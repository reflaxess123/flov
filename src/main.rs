#![windows_subsystem = "windows"]

mod audio;
mod config;
mod hotkey;
mod input;
mod llm;
mod overlay;
mod transcribe;
mod tray;

use anyhow::Result;
use std::sync::atomic::Ordering;
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
    tracing::info!("LLM enabled: {}, URL: {}, model: {}",
        config.llm.enabled, config.llm.url, config.llm.model);

    // Initialize audio recorder
    let recorder = Arc::new(audio::AudioRecorder::new(config.audio.sample_rate)?);
    let sample_rate = recorder.sample_rate();
    tracing::info!("Audio recorder initialized, sample rate: {}", sample_rate);

    // Initialize transcriber (HTTP client)
    let transcriber = Arc::new(transcribe::Transcriber::new(&config.service.url)?);
    tracing::info!("Transcriber initialized");

    // Initialize LLM text processor (optional)
    let text_processor: Option<Arc<llm::TextProcessor>> = if config.llm.enabled {
        match llm::TextProcessor::new(&config.llm.url, &config.llm.model) {
            Ok(p) => {
                tracing::info!("LLM text processor initialized");
                Some(Arc::new(p))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize LLM processor: {}", e);
                None
            }
        }
    } else {
        tracing::info!("LLM text processing disabled");
        None
    };

    // Create overlay state
    let overlay_state = overlay::OverlayState::new();
    let spectrum = overlay_state.spectrum.clone();
    let overlay_visible = overlay_state.visible.clone();
    let overlay_loading = overlay_state.loading.clone();
    let cursor_x = overlay_state.cursor_x.clone();
    let cursor_y = overlay_state.cursor_y.clone();

    // Tray state for icon updates
    let tray_state = Arc::new(std::sync::Mutex::new(tray::TrayState::Idle));

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
    let text_processor_clone = text_processor.clone();
    let is_pressed_clone = is_pressed.clone();
    let is_recording_clone = is_recording.clone();
    let tray_state_clone = tray_state.clone();
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
            *tray_state_clone.lock().unwrap() = tray::TrayState::Recording;
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
            tracing::info!("Recording stopped, {} samples", samples.len());

            // Reset recording state
            is_recording_clone.store(false, Ordering::SeqCst);

            if samples.len() < 1600 {
                tracing::info!("Too short, skipping");
                *tray_state_clone.lock().unwrap() = tray::TrayState::Idle;
                continue;
            }

            // Show loading state and yellow icon for transcription
            overlay_loading_clone.store(true, Ordering::SeqCst);
            *tray_state_clone.lock().unwrap() = tray::TrayState::Transcribing;

            // Transcribe (samples are always resampled to 16000 Hz)
            tracing::info!("Transcribing {} samples...", samples.len());
            let raw_text = match transcriber_clone.transcribe(&samples, 16000) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Transcription failed: {}", e);
                    overlay_loading_clone.store(false, Ordering::SeqCst);
                    *tray_state_clone.lock().unwrap() = tray::TrayState::Idle;
                    continue;
                }
            };

            if raw_text.is_empty() {
                tracing::info!("Empty transcription, skipping");
                overlay_loading_clone.store(false, Ordering::SeqCst);
                *tray_state_clone.lock().unwrap() = tray::TrayState::Idle;
                continue;
            }

            tracing::info!("Raw transcription: {}", raw_text);

            // Process with LLM if available
            let final_text = if let Some(ref processor) = text_processor_clone {
                // Set blue icon for LLM processing
                *tray_state_clone.lock().unwrap() = tray::TrayState::LlmProcessing;
                match processor.process(&raw_text) {
                    Ok(processed) => {
                        tracing::info!("LLM processed: {}", processed);
                        processed
                    }
                    Err(e) => {
                        tracing::warn!("LLM processing failed, using raw text: {}", e);
                        raw_text
                    }
                }
            } else {
                raw_text
            };

            overlay_loading_clone.store(false, Ordering::SeqCst);
            *tray_state_clone.lock().unwrap() = tray::TrayState::Idle;

            if final_text.is_empty() {
                continue;
            }

            let _ = tx.send(final_text);
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

    let mut last_tray_state = tray::TrayState::Idle;

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

            // Update tray icon based on current state
            let current_state = *tray_state.lock().unwrap();
            if current_state != last_tray_state {
                tray.set_state(current_state);
                last_tray_state = current_state;
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
