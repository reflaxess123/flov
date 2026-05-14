// Domain modules from existing flov.
pub mod audio;
pub mod config;
pub mod hotkey;
pub mod input;
pub mod models;
pub mod models_cmd;
pub mod postprocess;
pub mod state_cmd;
pub mod stats;
pub mod transcribe;

mod tray;
mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

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
    // Logging to file (mirrors current flov behavior)
    if let Ok(log_file) = std::fs::File::create("flov.log") {
        let _ = tracing_subscriber::fmt()
            .with_writer(log_file)
            .with_ansi(false)
            .try_init();
    }
    tracing::info!("flov starting (Tauri)");

    let cfg = config::Config::load().expect("config load failed");
    let recorder = Arc::new(audio::AudioRecorder::new(cfg.audio.sample_rate)
        .expect("audio init failed"));

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
    let initial_def = hotkey::HotkeyDef::parse(&cfg.hotkey.combo)
        .unwrap_or_else(|e| {
            tracing::warn!("invalid hotkey '{}': {}, falling back to Ctrl+Win", cfg.hotkey.combo, e);
            hotkey::HotkeyDef::parse("Ctrl+Win").unwrap()
        });
    tracing::info!("hotkey: {}", initial_def.combo);
    hotkey::set_hotkey_def(initial_def);
    let hotkey_combo = Arc::new(Mutex::new(cfg.hotkey.combo.clone()));

    let app_state = state_cmd::AppState {
        backend_choice: backend_choice.clone(),
        available_backends: available_backends.clone(),
        postprocess_enabled: postprocess_enabled.clone(),
        post_processor: post_processor.clone(),
        pp_settings: pp_settings.clone(),
        hotkey_combo: hotkey_combo.clone(),
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

        tracing::info!("recording start");
        let _ = active_mode.load(Ordering::SeqCst);

        // Reposition pill on the monitor of the cursor at recording-start.
        // Show the window only after positioning to avoid a flash on the
        // wrong monitor.
        if let Some(window) = app.get_webview_window("main") {
            #[cfg(target_os = "windows")]
            ui::position_at_cursor_monitor(&window);
            let _ = window.show();
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
                emit_state(&app, UiState::Idle);
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

        // Polished pill is only meaningful when AI transformed the text.
        // Without postprocess we skip the show — paste straight away.
        let used_postprocess =
            postprocess_enabled.load(Ordering::SeqCst) && pp_snapshot.is_some();

        if used_postprocess {
            // Hand off to frontend, which fires `polished-shown` after its
            // animation; meanwhile we wait for the matching command to paste.
            let _ = app.emit("polished-text", &final_text);
            emit_state(&app, UiState::Polished);
            // The actual paste is triggered by `polished_shown` invoke handler.
            // Hold the text in app state for that handler:
            *ui::PENDING_TEXT.lock().unwrap() = Some(final_text);
        } else {
            input::type_text(&final_text);
            emit_state(&app, UiState::Idle);
            tray::set_state(&app, tray::TrayState::Idle);
            // Frontend's morph-out transition fires hide_window when finished.
        }
    }
}

fn emit_state(app: &tauri::AppHandle, state: UiState) {
    let _ = app.emit("state-changed", state);
}
