//! Tauri 2 native tray with Windows-theme-aware tray icon.
//!
//! - Single source PNG (`<exe_dir>/icons/tray.png`, dark glyph).
//! - On dark Windows theme we invert RGB so the glyph becomes light, keeping
//!   the alpha channel intact.
//! - We poll the theme registry key every few seconds to react to user theme
//!   switches without an extra Win32 message hook.
//! - State is conveyed via tooltip — pill UI is the primary signal.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::AppHandle;

pub const TRAY_ID: &str = "flov-tray";

#[derive(Clone, Copy)]
pub enum TrayState {
    Idle,
    Recording,
    Transcribing,
}

impl TrayState {
    fn tooltip(self) -> &'static str {
        match self {
            TrayState::Idle => "flov — hold Ctrl+Win to dictate",
            TrayState::Recording => "flov — recording…",
            TrayState::Transcribing => "flov — transcribing…",
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_uses_dark_theme() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_VALUE_TYPE,
    };

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let subkey = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value_name = to_wide("SystemUsesLightTheme");

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), Some(0), KEY_READ, &mut hkey)
            != ERROR_SUCCESS
        {
            return false;
        }
        let mut data: u32 = 1;
        let mut size: u32 = std::mem::size_of::<u32>() as u32;
        let mut kind = REG_VALUE_TYPE::default();
        let res = RegQueryValueExW(
            hkey,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut kind),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);
        if res != ERROR_SUCCESS {
            return false;
        }
        // SystemUsesLightTheme: 1 = light, 0 = dark
        data == 0
    }
}

#[cfg(not(target_os = "windows"))]
fn windows_uses_dark_theme() -> bool {
    false
}

/// Loads tray.png and (if dark theme) inverts its RGB channels so the dark
/// glyph becomes light. Falls back to a flat dark square if the file is
/// missing or undecodeable.
fn load_themed_icon(dark_theme: bool) -> Image<'static> {
    let candidate = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("icons/tray.png")));

    if let Some(path) = candidate {
        if let Ok(img) = image::open(&path) {
            let mut rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if dark_theme {
                for px in rgba.pixels_mut() {
                    px[0] = 255 - px[0];
                    px[1] = 255 - px[1];
                    px[2] = 255 - px[2];
                    // alpha untouched
                }
            }
            return Image::new_owned(rgba.into_raw(), w, h);
        }
    }

    const SIZE: u32 = 32;
    let fill: [u8; 4] = if dark_theme {
        [0xF5, 0xF5, 0xF7, 0xFF]
    } else {
        [0x1C, 0x1C, 0x1E, 0xFF]
    };
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for _ in 0..(SIZE * SIZE) {
        data.extend_from_slice(&fill);
    }
    Image::new_owned(data, SIZE, SIZE)
}

pub fn setup(
    app: &AppHandle,
    postprocess_available: bool,
    postprocess_enabled: Arc<AtomicBool>,
) -> tauri::Result<()> {
    let postprocess_item = CheckMenuItem::with_id(
        app,
        "toggle_postprocess",
        "Post-process via OpenRouter",
        postprocess_available,
        false,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&postprocess_item, &quit_item])?;

    let initial_dark = windows_uses_dark_theme();
    let pp_enabled = postprocess_enabled.clone();

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(load_themed_icon(initial_dark))
        .tooltip(TrayState::Idle.tooltip())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app: &AppHandle, event: MenuEvent| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "toggle_postprocess" => {
                let was = pp_enabled.load(Ordering::SeqCst);
                pp_enabled.store(!was, Ordering::SeqCst);
                tracing::info!("postprocess toggled: {}", !was);
            }
            _ => {}
        })
        .build(app)?;

    // Poll the theme key. Windows broadcasts WM_SETTINGCHANGE on theme switch
    // but we don't (yet) hook the message loop, so a 3 s poll is the cheap
    // path to "tray icon recolors when theme changes".
    let app_for_poll = app.clone();
    std::thread::spawn(move || {
        let mut current = initial_dark;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let now = windows_uses_dark_theme();
            if now != current {
                current = now;
                if let Some(tray) = app_for_poll.tray_by_id(TRAY_ID) {
                    let _ = tray.set_icon(Some(load_themed_icon(now)));
                }
            }
        }
    });

    Ok(())
}

pub fn set_state(app: &AppHandle, state: TrayState) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(state.tooltip()));
    }
}
