//! Tauri UI glue: window positioning, click-through, polished-pill handshake.

use std::sync::Mutex;

/// Buffered final text waiting for the polished-pill animation to finish
/// (set by recording loop, drained by `polished_shown`).
pub static PENDING_TEXT: Mutex<Option<String>> = Mutex::new(None);

#[cfg(target_os = "windows")]
pub fn position_at_cursor_monitor(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, HMONITOR, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_err() {
            return;
        }
    }

    let hmon: HMONITOR = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let ok = unsafe { GetMonitorInfoW(hmon, &mut info) };
    if !ok.as_bool() {
        return;
    }

    let mon_x = info.rcMonitor.left;
    let mon_y = info.rcMonitor.top;
    let mon_w = info.rcMonitor.right - info.rcMonitor.left;
    let mon_h = info.rcMonitor.bottom - info.rcMonitor.top;

    // tauri.conf width/height are LOGICAL; monitor rect is PHYSICAL.
    let scale = window.scale_factor().unwrap_or(1.0);
    let pill_w = (800.0 * scale) as i32;
    let pill_h = (200.0 * scale) as i32;
    let margin_bottom = (24.0 * scale) as i32;
    let x = mon_x + (mon_w - pill_w) / 2;
    let y = mon_y + mon_h - pill_h - margin_bottom;

    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Disable Win11's native window-corner rounding so the OS doesn't paint a
/// rounded mask on top of our CSS border-radius (which produced ugly Win11
/// corner pixels showing through the frameless transparent window).
#[cfg(target_os = "windows")]
pub fn disable_native_window_rounding(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };

    let raw = match window.hwnd() {
        Ok(h) => h.0,
        Err(_) => return,
    };
    let hwnd = HWND(raw);
    let pref = DWMWCP_DONOTROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
    }
}

#[cfg(target_os = "windows")]
pub fn force_click_through(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };

    let raw = match window.hwnd() {
        Ok(h) => h.0,
        Err(_) => return,
    };
    let hwnd = HWND(raw);

    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new = ex | (WS_EX_LAYERED.0 as isize) | (WS_EX_TRANSPARENT.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
    }
}

/// Frontend tells us its polished-pill animation is done — paste the buffered
/// text. Window is hidden separately by `hide_window` after morph-out plays.
#[tauri::command]
pub fn polished_shown() {
    let text = PENDING_TEXT.lock().unwrap().take();
    if let Some(t) = text {
        crate::input::type_text(&t);
    }
}

/// Frontend calls this after the pill's exit transition finishes — only then
/// do we actually hide the OS window, so the morph-out is visible.
#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}
