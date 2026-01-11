#![windows_subsystem = "windows"]

mod api;
mod audio;
mod config;
mod hotkey;
mod input;
mod overlay;
mod transcribe;
mod tray;

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging to file
    let log_file = std::fs::File::create("flov.log").ok();
    if let Some(file) = log_file {
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .init();
    }

    tracing::info!("Flov starting...");

    // Load config
    let config = config::Config::load()?;
    tracing::info!("Config loaded");

    // Initialize components
    let recorder = Arc::new(audio::AudioRecorder::new(config.audio.sample_rate)?);
    let transcriber = Arc::new(transcribe::Transcriber::new(
        &config.whisper.model_path,
        config.whisper.language.clone(),
    )?);

    // API client (optional)
    let api_client = config.api.as_ref().map(|api_config| {
        Arc::new(api::ApiClient::new(
            api_config.endpoint.clone(),
            api_config.key.clone(),
            api_config.model.clone(),
        ))
    });

    // GLM enabled flag (shared with tray)
    let glm_enabled = Arc::new(AtomicBool::new(false));

    // Create overlay
    let overlay = Arc::new(overlay::Overlay::new()?);

    // Create tray icon
    let tray = tray::TrayManager::new(glm_enabled.clone())?;

    // Setup hotkey state
    let hotkey_state = hotkey::HotkeyState::new();
    let is_pressed = hotkey_state.is_pressed.clone();
    let is_recording = hotkey_state.is_recording.clone();

    // Install keyboard hook
    let _hook = hotkey::install_hook(hotkey_state)?;
    tracing::info!("Hotkey hook installed (Ctrl+Win)");

    // Channel for processing results
    let (tx, mut rx) = mpsc::channel::<(String, bool)>(1);

    // Spawn processing task
    let recorder_clone = recorder.clone();
    let transcriber_clone = transcriber.clone();
    let overlay_clone = overlay.clone();
    let is_pressed_clone = is_pressed.clone();
    let is_recording_clone = is_recording.clone();
    let glm_enabled_clone = glm_enabled.clone();

    std::thread::spawn(move || {
        loop {
            // Wait for hotkey press
            while !is_pressed_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            tracing::info!("Recording started");
            overlay_clone.show();

            // Record while pressed
            let is_pressed_for_record = is_pressed_clone.clone();
            let samples = recorder_clone
                .record_while(move || is_pressed_for_record.load(Ordering::SeqCst))
                .unwrap_or_default();

            overlay_clone.hide();
            tracing::info!("Recording stopped, {} samples", samples.len());

            // Reset recording state so hotkey can trigger again
            is_recording_clone.store(false, Ordering::SeqCst);

            if samples.len() < 1600 {
                tracing::info!("Too short, skipping");
                continue;
            }

            // Transcribe
            tracing::info!("Transcribing {} samples...", samples.len());
            let text = match transcriber_clone.transcribe(&samples) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Transcription failed: {}", e);
                    continue;
                }
            };

            if text.is_empty() {
                tracing::info!("Empty transcription, skipping");
                continue;
            }

            tracing::info!("Transcribed: {}", text);

            let use_glm = glm_enabled_clone.load(Ordering::SeqCst);
            let tx_clone = tx.clone();
            let _ = tx_clone.blocking_send((text, use_glm));
        }
    });

    // Spawn text insertion task
    let api_client_clone = api_client.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            while let Some((text, use_glm)) = rx.recv().await {
                let final_text = if use_glm {
                    if let Some(ref api) = api_client_clone {
                        tracing::info!("Processing with GLM...");
                        match api.improve_text(&text).await {
                            Ok(improved) => {
                                tracing::info!("GLM improved: {}", improved);
                                improved
                            }
                            Err(e) => {
                                tracing::error!("GLM error: {}", e);
                                text
                            }
                        }
                    } else {
                        tracing::warn!("GLM enabled but no API config");
                        text
                    }
                } else {
                    text
                };

                tracing::info!("Inserting text: {}", final_text);
                input::type_text(&final_text);
            }
        });
    });

    // Main message loop
    tracing::info!("Flov running. Press Ctrl+Win to record.");

    unsafe {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        loop {
            while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                &mut msg,
                None,
                0,
                0,
                windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
            )
            .as_bool()
            {
                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
            }

            if tray.check_events() {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    tracing::info!("Flov shutting down");
    Ok(())
}
