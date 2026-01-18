#![windows_subsystem = "windows"]

mod audio;
mod config;
mod hotkey;
mod input;
mod transcribe;
mod tray;

use anyhow::Result;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let ipc_mode = args.contains(&"--ipc".to_string());

    if ipc_mode {
        run_ipc_mode()
    } else {
        run_standalone_mode()
    }
}

/// IPC mode - communicates with Electron via JSON on stdin/stdout
fn run_ipc_mode() -> Result<()> {
    eprintln!("Flov IPC mode starting...");

    // Load config
    let config = config::Config::load()?;
    eprintln!("Config loaded");

    // Initialize components
    let recorder = Arc::new(audio::AudioRecorder::new(config.audio.sample_rate)?);
    let transcriber = Arc::new(transcribe::Transcriber::new(
        &config.whisper.model_path,
        config.whisper.language.clone(),
    )?);

    // Setup hotkey state
    let hotkey_state = hotkey::HotkeyState::new();
    let is_pressed = hotkey_state.is_pressed.clone();
    let is_recording = hotkey_state.is_recording.clone();

    // Install keyboard hook
    let _hook = hotkey::install_hook(hotkey_state)?;
    eprintln!("Hotkey hook installed (Ctrl+Win)");

    // Spawn recording thread
    let recorder_clone = recorder.clone();
    let transcriber_clone = transcriber.clone();
    let is_pressed_clone = is_pressed.clone();
    let is_recording_clone = is_recording.clone();

    std::thread::spawn(move || {
        loop {
            // Wait for hotkey press
            while !is_pressed_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            // Send recording started
            send_json(&serde_json::json!({"type": "recording_started"}));

            // Record while pressed, sending frequency spectrum
            let is_pressed_for_record = is_pressed_clone.clone();

            let samples = recorder_clone
                .record_while_with_spectrum(
                    move || is_pressed_for_record.load(Ordering::SeqCst),
                    |spectrum| {
                        send_json(&serde_json::json!({"type": "spectrum", "values": spectrum}));
                    }
                )
                .unwrap_or_default();

            // Send recording stopped
            send_json(&serde_json::json!({"type": "recording_stopped"}));

            // Reset recording state
            is_recording_clone.store(false, Ordering::SeqCst);

            if samples.len() < 1600 {
                continue;
            }

            // Send transcribing status
            send_json(&serde_json::json!({"type": "transcribing"}));

            // Transcribe
            let text = match transcriber_clone.transcribe(&samples) {
                Ok(t) => t,
                Err(e) => {
                    send_json(&serde_json::json!({"type": "error", "message": e.to_string()}));
                    continue;
                }
            };

            if text.is_empty() {
                continue;
            }

            // Send transcription result
            send_json(&serde_json::json!({"type": "transcription", "text": text}));

            // Insert text
            input::type_text(&text);
        }
    });

    // Read commands from stdin in a separate thread
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(line) = line {
                if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&line) {
                    handle_command(&cmd);
                }
            }
        }
    });

    // Message pump for keyboard hook (required for low-level hooks to work smoothly)
    unsafe {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).as_bool() {
            windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }

    Ok(())
}

fn send_json(value: &serde_json::Value) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{}", value);
    let _ = stdout.flush();
}

fn handle_command(cmd: &serde_json::Value) {
    match cmd.get("type").and_then(|t| t.as_str()) {
        Some("ping") => {
            send_json(&serde_json::json!({"type": "pong"}));
        }
        Some("quit") => {
            std::process::exit(0);
        }
        _ => {}
    }
}

/// Standalone mode - runs with tray icon, no IPC
fn run_standalone_mode() -> Result<()> {
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

    // Recording state flag (for tray icon updates in main thread)
    let is_recording_icon = Arc::new(AtomicBool::new(false));

    // Create tray icon (without GLM)
    let tray = tray::TrayManager::new_simple()?;

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

    std::thread::spawn(move || {
        loop {
            // Wait for hotkey press
            while !is_pressed_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            tracing::info!("Recording started");
            is_recording_icon_clone.store(true, Ordering::SeqCst);

            // Record while pressed
            let is_pressed_for_record = is_pressed_clone.clone();
            let samples = recorder_clone
                .record_while(move || is_pressed_for_record.load(Ordering::SeqCst))
                .unwrap_or_default();

            is_recording_icon_clone.store(false, Ordering::SeqCst);
            tracing::info!("Recording stopped, {} samples", samples.len());

            // Reset recording state
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

    // Main message loop
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
