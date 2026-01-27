#![windows_subsystem = "windows"]

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

fn main() -> Result<()> {
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
    tracing::info!("Config loaded, service URL: {}", config.service.url);

    // Initialize audio recorder
    let recorder = Arc::new(audio::AudioRecorder::new(config.audio.sample_rate)?);
    let sample_rate = recorder.sample_rate();
    tracing::info!("Audio recorder initialized, sample rate: {}", sample_rate);

    // Initialize transcriber (HTTP client)
    let transcriber = Arc::new(transcribe::Transcriber::new(&config.service.url)?);
    tracing::info!("Transcriber initialized");

    // Create overlay state
    let overlay_state = overlay::OverlayState::new();
    let spectrum = overlay_state.spectrum.clone();
    let overlay_visible = overlay_state.visible.clone();
    let overlay_loading = overlay_state.loading.clone();
    let cursor_x = overlay_state.cursor_x.clone();
    let cursor_y = overlay_state.cursor_y.clone();

    // Recording state flag (for tray icon updates)
    let is_recording_icon = Arc::new(AtomicBool::new(false));

    // Create tray icon
    let tray = tray::TrayManager::new_simple()?;
    tracing::info!("Tray icon created");

    // Setup hotkey state
    let hotkey_state = hotkey::HotkeyState::new();
    let is_pressed = hotkey_state.is_pressed.clone();
    let is_recording = hotkey_state.is_recording.clone();

    // Install keyboard hook
    let _hook = hotkey::install_hook(hotkey_state)?;
    tracing::info!("Hotkey hook installed (Ctrl+Win)");

    // Channel for transcription results
    let (tx, rx) = std::sync::mpsc::channel::<String>();

    // Spawn processing task
    let recorder_clone = recorder.clone();
    let transcriber_clone = transcriber.clone();
    let is_pressed_clone = is_pressed.clone();
    let is_recording_clone = is_recording.clone();
    let is_recording_icon_clone = is_recording_icon.clone();
    let spectrum_clone = spectrum.clone();
    let overlay_visible_clone = overlay_visible.clone();
    let overlay_loading_clone = overlay_loading.clone();
    let cursor_x_clone = cursor_x.clone();
    let cursor_y_clone = cursor_y.clone();

    std::thread::spawn(move || {
        loop {
            // Wait for hotkey press
            while !is_pressed_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            tracing::info!("Recording started");
            is_recording_icon_clone.store(true, Ordering::SeqCst);
            overlay_visible_clone.store(true, Ordering::SeqCst);

            // Get cursor position at start of recording
            let (cx, cy) = get_cursor_position();
            *cursor_x_clone.lock().unwrap() = cx as f32;
            *cursor_y_clone.lock().unwrap() = cy as f32;

            // Record while pressed with spectrum callback
            let is_pressed_for_record = is_pressed_clone.clone();
            let spectrum_for_record = spectrum_clone.clone();

            let samples = recorder_clone
                .record_while_with_spectrum(
                    move || is_pressed_for_record.load(Ordering::SeqCst),
                    move |new_spectrum| {
                        let mut spec = spectrum_for_record.lock().unwrap();
                        *spec = new_spectrum;
                    },
                )
                .unwrap_or_default();

            overlay_visible_clone.store(false, Ordering::SeqCst);
            is_recording_icon_clone.store(false, Ordering::SeqCst);
            tracing::info!("Recording stopped, {} samples", samples.len());

            // Reset recording state
            is_recording_clone.store(false, Ordering::SeqCst);

            if samples.len() < 1600 {
                tracing::info!("Too short, skipping");
                continue;
            }

            // Show loading state
            overlay_loading_clone.store(true, Ordering::SeqCst);

            // Transcribe
            tracing::info!("Transcribing {} samples...", samples.len());
            let sample_rate = recorder_clone.sample_rate();
            let text = match transcriber_clone.transcribe(&samples, sample_rate) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Transcription failed: {}", e);
                    overlay_loading_clone.store(false, Ordering::SeqCst);
                    continue;
                }
            };

            overlay_loading_clone.store(false, Ordering::SeqCst);

            if text.is_empty() {
                tracing::info!("Empty transcription, skipping");
                continue;
            }

            tracing::info!("Transcribed: {}", text);
            let _ = tx.send(text);
        }
    });

    // Spawn text insertion task
    std::thread::spawn(move || {
        while let Ok(text) = rx.recv() {
            tracing::info!("Inserting text: {}", text);
            input::type_text(&text);
        }
    });

    // Spawn overlay in separate thread
    let overlay_state_for_thread = overlay::OverlayState {
        spectrum,
        visible: overlay_visible,
        loading: overlay_loading,
        cursor_x,
        cursor_y,
    };

    std::thread::spawn(move || {
        if let Err(e) = overlay::run_overlay(overlay_state_for_thread) {
            tracing::error!("Overlay error: {}", e);
        }
    });

    // Main message loop with tray
    tracing::info!("Flov running. Press Ctrl+Win to record.");

    let mut last_recording_state = false;

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

            // Update tray icon based on recording state
            let current_recording = is_recording_icon.load(Ordering::SeqCst);
            if current_recording != last_recording_state {
                tray.set_recording(current_recording);
                last_recording_state = current_recording;
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

fn get_cursor_position() -> (i32, i32) {
    unsafe {
        let mut point = windows::Win32::Foundation::POINT::default();
        if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point).is_ok() {
            (point.x, point.y)
        } else {
            (0, 0)
        }
    }
}
