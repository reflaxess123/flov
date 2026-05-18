// Domain modules from existing flov.
pub mod audio;
pub mod config;
pub mod hotkey;
pub mod input;
pub mod models;
pub mod models_cmd;
pub mod paths;
pub mod postprocess;
pub mod state_cmd;
pub mod stats;
pub mod transcribe;

mod tray;
mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

/// Open `flov.log` next to the running exe (not CWD — CWD changes when
/// the app is launched from elsewhere) in append mode so logs survive
/// across restarts. Wrap the File in a Mutex so the writer is `Sync` and
/// every record actually flushes to the OS buffer (the previous
/// `with_writer(file)` setup was both truncating on every start AND
/// failing to write reliably). Defaults to INFO level when RUST_LOG is
/// unset.
fn init_logging() {
    use std::fs::OpenOptions;
    use std::path::PathBuf;
    use tracing_subscriber::EnvFilter;

    // Per-platform user data dir (see `paths.rs`): on Windows the
    // installer puts the exe under %LOCALAPPDATA% which is writable so
    // exe-relative would work too — but on macOS the .app bundle is
    // read-only after code-signing and we must write to
    // `~/Library/Application Support/com.flov.app/`. Falling back to
    // the binary's directory keeps behaviour sane if `HOME` is unset.
    let log_path: PathBuf = paths::log_path().unwrap_or_else(|_| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("flov.log")))
            .unwrap_or_else(|| PathBuf::from("flov.log"))
    });

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    match file {
        Ok(f) => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(Mutex::new(f))
                .with_ansi(false)
                .with_target(false)
                .try_init();
        }
        Err(_) => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .try_init();
        }
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
enum UiState {
    Idle,
    Recording,
    Transcribing,
    Polished,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    tracing::info!("flov starting (Tauri)");

    let cfg = config::Config::load().expect("config load failed");
    let recorder = Arc::new(
        audio::AudioRecorder::new(cfg.audio.sample_rate, cfg.audio.device.as_deref())
            .expect("audio init failed"),
    );

    // Shared mutable backend + model — written by the tray/Models window,
    // read by the Transcriber on every transcribe() call so a switch takes
    // effect on the next press of Ctrl+Win, no restart required.
    let backend_choice = Arc::new(Mutex::new(cfg.backend.choice.clone()));
    let model_path = Arc::new(Mutex::new(cfg.whisper.model_path.clone()));
    let available_backends = transcribe::available_backends();
    tracing::info!(
        "available backends: {:?}; configured choice: {}",
        available_backends,
        cfg.backend.choice
    );

    let transcriber = Arc::new(
        transcribe::Transcriber::new(
            model_path.clone(),
            cfg.whisper.language.clone(),
            backend_choice.clone(),
        )
        .expect("whisper init failed")
    );

    let model_state = models_cmd::ModelState {
        model_path: model_path.clone(),
        in_flight: Arc::new(Mutex::new(Vec::new())),
    };

    let initial_pp = if cfg.openrouter.api_key.is_empty() {
        None
    } else {
        Some(Arc::new(postprocess::PostProcessor::new(
            cfg.openrouter.api_key.clone(),
            cfg.openrouter.model.clone(),
            cfg.openrouter.system_prompt.clone(),
        )))
    };
    let post_processor = Arc::new(Mutex::new(initial_pp));
    let pp_settings = Arc::new(Mutex::new(state_cmd::PostprocessSettings {
        api_key: cfg.openrouter.api_key.clone(),
        model: cfg.openrouter.model.clone(),
        system_prompt: cfg.openrouter.system_prompt.clone(),
    }));
    let postprocess_enabled = Arc::new(AtomicBool::new(false));

    let stats = Arc::new(stats::Stats::open().expect("stats open failed"));

    let hotkey_state = hotkey::HotkeyState::new();
    let active_mode = hotkey_state.active_mode.clone();
    let is_recording = hotkey_state.is_recording.clone();
    let _hook = hotkey::install_hook(hotkey_state).expect("hotkey hook failed");

    // Set the initial hotkey definition; Tauri command can swap it later.
    // Fallback combo is per-platform so Mac users don't get Ctrl+Cmd (=
    // collides with system shortcuts like Ctrl+Cmd+Q lock screen).
    #[cfg(target_os = "macos")]
    let fallback_combo = "Cmd+Alt";
    #[cfg(not(target_os = "macos"))]
    let fallback_combo = "Ctrl+Win";
    let initial_def = hotkey::HotkeyDef::parse(&cfg.hotkey.combo)
        .unwrap_or_else(|e| {
            tracing::warn!(
                "invalid hotkey '{}': {}, falling back to {}",
                cfg.hotkey.combo, e, fallback_combo
            );
            hotkey::HotkeyDef::parse(fallback_combo).unwrap()
        });
    tracing::info!("hotkey: {}", initial_def.combo);
    hotkey::set_hotkey_def(initial_def);
    let hotkey_combo = Arc::new(Mutex::new(cfg.hotkey.combo.clone()));
    let audio_device = Arc::new(Mutex::new(cfg.audio.device.clone()));

    let app_state = state_cmd::AppState {
        backend_choice: backend_choice.clone(),
        available_backends: available_backends.clone(),
        postprocess_enabled: postprocess_enabled.clone(),
        post_processor: post_processor.clone(),
        pp_settings: pp_settings.clone(),
        hotkey_combo: hotkey_combo.clone(),
        audio_device: audio_device.clone(),
        stats: stats.clone(),
    };

    let stats_for_loop = stats.clone();
    let sample_rate_for_loop = cfg.audio.sample_rate;

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(model_state)
        .manage(app_state)
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let window = app.get_webview_window("main").expect("main window missing");

            // Click-through is set both via Tauri (works on Windows) and as a
            // belt-and-suspenders Win32 fallback, mirroring the verified POC.
            let _ = window.set_ignore_cursor_events(true);
            #[cfg(target_os = "windows")]
            ui::force_click_through(&window);

            // Re-use the tray PNG as the Settings window's title-bar /
            // taskbar icon so the app has a consistent brand without us
            // needing a separate .ico.
            if let Some(settings) = app.get_webview_window("settings") {
                let icon = tray::load_themed_icon_for_window();
                let _ = settings.set_icon(icon);
                #[cfg(target_os = "windows")]
                ui::disable_native_window_rounding(&settings);
            }

            tray::setup(&app_handle)?;

            // Spawn the recording orchestration thread; it owns the recorder
            // loop and emits state/amplitude events to the webview.
            let recorder = recorder.clone();
            let transcriber = transcriber.clone();
            let active_mode = active_mode.clone();
            let is_recording = is_recording.clone();
            let post_processor = post_processor.clone();
            let postprocess_enabled = postprocess_enabled.clone();
            let stats = stats_for_loop.clone();
            let app_for_thread = app_handle.clone();
            let sample_rate = sample_rate_for_loop;

            // Watchdog: if the recorder loop crashes mid-iteration the
            // hook keeps setting `is_recording=true` on every keypress
            // but nothing clears it, so the next press silently no-ops.
            // This thread spots that wedge and resets the flag so the
            // user is never stuck without a manual restart.
            {
                let watch_recording = is_recording.clone();
                let watch_mode = active_mode.clone();
                std::thread::Builder::new()
                    .name("flov-state-watchdog".into())
                    .spawn(move || {
                        let mut stuck_ticks = 0u8;
                        loop {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            let recording = watch_recording.load(Ordering::SeqCst);
                            let mode = watch_mode.load(Ordering::SeqCst);
                            if recording && mode == hotkey::MODE_IDLE {
                                stuck_ticks = stuck_ticks.saturating_add(1);
                                if stuck_ticks >= 3 {
                                    tracing::warn!(
                                        "watchdog: is_recording stuck true with mode=IDLE for ~6s, resetting"
                                    );
                                    watch_recording.store(false, Ordering::SeqCst);
                                    stuck_ticks = 0;
                                }
                            } else {
                                stuck_ticks = 0;
                            }
                        }
                    })
                    .expect("spawn watchdog thread");
            }

            // Independent webview reload thread.
            //
            // WebView2's renderer process leaks DOM/JS state over
            // multi-hour sessions and eventually stops painting our
            // layered, transparent, always-on-top pill — the HWND stays
            // "visible" from Windows' point of view but DWM shows
            // nothing. This used to be tied to the recording loop, but
            // on long idle sessions the user wouldn't trigger a
            // recording for hours, so the reload never fired and the
            // webview had already rotted by the time it was needed.
            // Doing it on its own timer guarantees we refresh
            // periodically regardless of activity.
            //
            // We skip the reload while the pill is visible so we don't
            // yank it from under a live recording. The interval is
            // 30 min, which is well inside the empirical "starts
            // failing somewhere after a couple of hours" window.
            {
                let app_for_reload = app_handle.clone();
                let watch_recording = is_recording.clone();
                std::thread::Builder::new()
                    .name("flov-webview-reloader".into())
                    .spawn(move || {
                        const RELOAD_INTERVAL: std::time::Duration =
                            std::time::Duration::from_secs(30 * 60);
                        loop {
                            std::thread::sleep(RELOAD_INTERVAL);
                            if watch_recording.load(Ordering::SeqCst) {
                                tracing::info!(
                                    "webview reload skipped: recording in progress"
                                );
                                continue;
                            }
                            let Some(w) = app_for_reload.get_webview_window("main") else {
                                tracing::warn!("webview reload: main window missing");
                                continue;
                            };
                            // Belt-and-suspenders: even when idle, the
                            // OS-level visibility might be true if the
                            // morph-out hide hasn't fired yet. Skip
                            // this tick to keep the reload entirely
                            // invisible.
                            if matches!(w.is_visible(), Ok(true)) {
                                tracing::info!("webview reload skipped: pill visible");
                                continue;
                            }
                            match w.eval("location.reload()") {
                                Ok(_) => tracing::info!("webview reload triggered"),
                                Err(e) => tracing::warn!("webview reload failed: {}", e),
                            }
                        }
                    })
                    .expect("spawn webview reloader thread");
            }

            std::thread::spawn(move || {
                recording_loop(
                    app_for_thread,
                    recorder,
                    transcriber,
                    active_mode,
                    is_recording,
                    post_processor,
                    postprocess_enabled,
                    stats,
                    sample_rate,
                );
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ui::polished_shown,
            ui::hide_window,
            models_cmd::list_models,
            models_cmd::download_model,
            models_cmd::delete_model,
            models_cmd::set_active_model,
            models_cmd::show_models_window,
            state_cmd::get_backend_state,
            state_cmd::set_backend_choice,
            state_cmd::get_postprocess_state,
            state_cmd::set_postprocess_enabled,
            state_cmd::get_postprocess_config,
            state_cmd::set_postprocess_config,
            state_cmd::get_hotkey,
            state_cmd::set_hotkey,
            state_cmd::list_audio_inputs,
            state_cmd::set_audio_input,
            state_cmd::get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn recording_loop(
    app: tauri::AppHandle,
    recorder: Arc<audio::AudioRecorder>,
    transcriber: Arc<transcribe::Transcriber>,
    active_mode: Arc<std::sync::atomic::AtomicU8>,
    is_recording: Arc<AtomicBool>,
    post_processor: Arc<Mutex<Option<Arc<postprocess::PostProcessor>>>>,
    postprocess_enabled: Arc<AtomicBool>,
    stats: Arc<stats::Stats>,
    sample_rate: u32,
) {
    loop {
        // Idle until a hotkey arms us
        while active_mode.load(Ordering::SeqCst) == hotkey::MODE_IDLE {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Now that we've actually picked up the hotkey, mark ourselves
        // as recording. The hook used to do this on KEYDOWN, but that
        // created a wedge if the user pressed during the previous
        // transcribe — see HotkeyState docs.
        is_recording.store(true, Ordering::SeqCst);
        tracing::info!("recording start");

        // Pre-flight: no model → don't even start recording. Show the
        // error pill at the cursor and wait for the user to release the
        // hotkey before going idle (otherwise the moment they release
        // the loop would try again instantly).
        let model_present = transcriber.has_model();
        if !model_present {
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                ui::position_at_cursor_monitor(&window);
                let _ = window.show();
            }
            let _ = app.emit("transcribe-error", "Скачай модель: Settings → Models");
            tracing::warn!("hotkey pressed but no model is configured");
            is_recording.store(false, Ordering::SeqCst);
            // Drain the held hotkey so we don't immediately re-fire.
            while active_mode.load(Ordering::SeqCst) != hotkey::MODE_IDLE {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            continue;
        }

        // Reposition pill on the monitor of the cursor at recording-start.
        // Show the window only after positioning to avoid a flash on the
        // wrong monitor.
        match app.get_webview_window("main") {
            Some(window) => {
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                ui::position_at_cursor_monitor(&window);
                if let Err(e) = window.show() {
                    tracing::warn!("pill window.show() failed: {}", e);
                }
                // Force DWM to repaint our layered/transparent pill —
                // without this, on long sessions the HWND is visible
                // but DWM keeps showing the previous (blank) layered
                // surface, so the user sees nothing even though
                // `show()` and the subsequent `emit` both succeeded.
                // See `ui::force_repaint` docs for the full story.
                // macOS WKWebView doesn't have the equivalent DWM
                // layered-surface staleness issue, so the call is
                // Windows-only.
                #[cfg(target_os = "windows")]
                ui::force_repaint(&window);
            }
            None => tracing::warn!("pill window missing — webview may have crashed"),
        }
        emit_state(&app, UiState::Recording);
        tray::set_state(&app, tray::TrayState::Recording);

        // Drive the recorder; emits a 20-band FFT spectrum (~30 Hz). Frontend
        // renders bars at fixed positions with heights that pulse with the
        // matching frequency band.
        let active_for_record = active_mode.clone();
        let app_for_spec = app.clone();
        let samples = recorder
            .record_while_with_spectrum(
                move || active_for_record.load(Ordering::SeqCst) != hotkey::MODE_IDLE,
                move |spec| {
                    let _ = app_for_spec.emit("audio-spectrum", spec);
                },
            )
            .unwrap_or_default();

        is_recording.store(false, Ordering::SeqCst);
        tracing::info!("recording stop, samples={}", samples.len());

        // Too short — skip transcribe entirely (frontend hides after morph-out)
        if samples.len() < 1600 {
            emit_state(&app, UiState::Idle);
            tray::set_state(&app, tray::TrayState::Idle);
            continue;
        }

        emit_state(&app, UiState::Transcribing);
        tray::set_state(&app, tray::TrayState::Transcribing);

        let raw_text = match transcriber.transcribe(&samples) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("transcribe failed: {}", e);
                let msg = if e.to_string().contains("model file not found") {
                    "Скачай модель: Settings → Models".to_string()
                } else {
                    format!("Transcribe error: {}", e)
                };
                let _ = app.emit("transcribe-error", &msg);
                tray::set_state(&app, tray::TrayState::Idle);
                continue;
            }
        };
        if raw_text.is_empty() {
            emit_state(&app, UiState::Idle);
            tray::set_state(&app, tray::TrayState::Idle);
            continue;
        }
        tracing::info!("transcript: {}", raw_text);

        // Stats: count this recording. `chars` is char count of the raw
        // transcript (before postprocess); seconds is derived from the
        // captured sample count and the recorder's native rate.
        let chars = raw_text.chars().count() as u64;
        let seconds = samples.len() as f64 / sample_rate as f64;
        stats.record(chars, seconds);
        // Push update to any open Settings window so its counters and
        // heatmap reflect immediately, without polling.
        let _ = app.emit("stats-updated", ());

        // Snapshot the current PostProcessor (Settings UI may have swapped
        // it). We keep the Arc only for this iteration.
        let pp_snapshot: Option<Arc<postprocess::PostProcessor>> =
            post_processor.lock().unwrap().clone();

        let final_text = if postprocess_enabled.load(Ordering::SeqCst) {
            if let Some(p) = pp_snapshot.as_ref() {
                match p.process(&raw_text) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!(
                            "postprocess failed, falling back to raw transcript: {:#}",
                            e
                        );
                        raw_text
                    }
                }
            } else {
                tracing::warn!("postprocess enabled but no API key configured");
                raw_text
            }
        } else {
            raw_text
        };

        // Pill stays on the "transcribing" sine wave the entire time the
        // postprocess HTTP call is in flight — once the final text is in
        // hand we paste immediately and let the pill morph out, identical
        // to the no-postprocess path. The previous "polished" stage caused
        // a ~360 ms blank pill (out-fade + in-fade delay) before the paste,
        // and it gave nothing the user couldn't see in the editor itself.
        input::type_text(&final_text);
        emit_state(&app, UiState::Idle);
        tray::set_state(&app, tray::TrayState::Idle);
        // Frontend's morph-out transition fires hide_window when finished.
        // Webview reload happens on an independent timer (see the
        // "flov-webview-reloader" thread in setup()) — putting it here
        // meant idle users could go many hours without ever hitting
        // it.
    }
}

fn emit_state(app: &tauri::AppHandle, state: UiState) {
    let _ = app.emit("state-changed", state);
}
