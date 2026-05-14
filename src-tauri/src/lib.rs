// Domain modules from existing flov.
pub mod audio;
pub mod config;
pub mod hotkey;
pub mod input;
pub mod postprocess;
pub mod transcribe;

mod tray;
mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    let transcriber = Arc::new(
        transcribe::Transcriber::new(&cfg.whisper.model_path, cfg.whisper.language.clone())
            .expect("whisper init failed (check model_path in flov.toml)")
    );

    let postprocess_available = !cfg.openrouter.api_key.is_empty();
    let post_processor = if postprocess_available {
        Some(Arc::new(postprocess::PostProcessor::new(
            cfg.openrouter.api_key.clone(),
            cfg.openrouter.model.clone(),
            cfg.openrouter.system_prompt.clone(),
            cfg.openrouter.reply_system_prompt.clone(),
        )))
    } else {
        None
    };
    let postprocess_enabled = Arc::new(AtomicBool::new(false));

    let hotkey_state = hotkey::HotkeyState::new();
    let active_mode = hotkey_state.active_mode.clone();
    let is_recording = hotkey_state.is_recording.clone();
    let _hook = hotkey::install_hook(hotkey_state).expect("hotkey hook failed");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let window = app.get_webview_window("main").expect("main window missing");

            // Click-through is set both via Tauri (works on Windows) and as a
            // belt-and-suspenders Win32 fallback, mirroring the verified POC.
            let _ = window.set_ignore_cursor_events(true);
            #[cfg(target_os = "windows")]
            ui::force_click_through(&window);

            tray::setup(&app_handle, postprocess_available, postprocess_enabled.clone())?;

            // Spawn the recording orchestration thread; it owns the recorder
            // loop and emits state/amplitude events to the webview.
            let recorder = recorder.clone();
            let transcriber = transcriber.clone();
            let active_mode = active_mode.clone();
            let is_recording = is_recording.clone();
            let post_processor = post_processor.clone();
            let postprocess_enabled = postprocess_enabled.clone();
            let app_for_thread = app_handle.clone();

            std::thread::spawn(move || {
                recording_loop(
                    app_for_thread,
                    recorder,
                    transcriber,
                    active_mode,
                    is_recording,
                    post_processor,
                    postprocess_enabled,
                );
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ui::polished_shown, ui::hide_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn recording_loop(
    app: tauri::AppHandle,
    recorder: Arc<audio::AudioRecorder>,
    transcriber: Arc<transcribe::Transcriber>,
    active_mode: Arc<std::sync::atomic::AtomicU8>,
    is_recording: Arc<AtomicBool>,
    post_processor: Option<Arc<postprocess::PostProcessor>>,
    postprocess_enabled: Arc<AtomicBool>,
) {
    loop {
        // Idle until a hotkey arms us
        while active_mode.load(Ordering::SeqCst) == hotkey::MODE_IDLE {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let mode = active_mode.load(Ordering::SeqCst);
        tracing::info!("recording start (mode={})", mode);

        // For reply mode, snapshot clipboard before we start
        let clipboard_context = if mode == hotkey::MODE_REPLY {
            input::get_clipboard()
        } else {
            None
        };

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

        // Mode → final text
        let final_text = match mode {
            hotkey::MODE_REPLY => {
                if let Some(p) = post_processor.as_ref() {
                    let ctx = clipboard_context.as_deref().unwrap_or("");
                    p.reply(ctx, &raw_text).unwrap_or(raw_text)
                } else {
                    raw_text
                }
            }
            _ => {
                if postprocess_enabled.load(Ordering::SeqCst) {
                    if let Some(p) = post_processor.as_ref() {
                        match p.process(&raw_text) {
                            Ok(t) => t,
                            Err(_) => raw_text,
                        }
                    } else {
                        raw_text
                    }
                } else {
                    raw_text
                }
            }
        };

        // Polished pill is only meaningful when AI transformed the text.
        // Without postprocess we skip the show — paste straight away.
        let used_postprocess = match mode {
            hotkey::MODE_REPLY => post_processor.is_some(),
            _ => postprocess_enabled.load(Ordering::SeqCst) && post_processor.is_some(),
        };

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
