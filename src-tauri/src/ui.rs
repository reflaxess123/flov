//! Tauri UI glue: window positioning, click-through, polished-pill handshake.

#![allow(dead_code)]

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

/// Force DWM to recomposite our layered+transparent+frameless pill
/// window after we call `show()`.
///
/// The bug this exists to fix: on long sessions the user calls the
/// hotkey, the backend logs `recording start` and the OS reports the
/// HWND visible, but the pill never visually appears. Backend keeps
/// working — recordings get transcribed and pasted — only the visual
/// is stuck.
///
/// The mechanism is documented in WebView2 issues
/// [#3674](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3674)
/// and [#1130](https://github.com/MicrosoftEdge/WebView2Feedback/issues/1130):
/// for a `WS_EX_LAYERED` window, the standard WM_PAINT path doesn't
/// touch the composited surface — DWM keeps showing whatever was on
/// it last. Without a tickle, the surface can stay blank indefinitely,
/// and on long sessions (or after sleep/wake) this is what users see.
///
/// The cheapest reliable tickle is `RedrawWindow(INVALIDATE | UPDATENOW
/// | ERASE | FRAME)` plus a no-op `SetWindowPos` with
/// `SWP_FRAMECHANGED`, which together force the compositor to
/// re-evaluate the layered content. No flicker because we don't move,
/// resize, or restack.
#[cfg(target_os = "windows")]
pub fn force_repaint(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        RedrawWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    };

    let raw = match window.hwnd() {
        Ok(h) => h.0,
        Err(_) => return,
    };
    let hwnd = HWND(raw);

    unsafe {
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN | RDW_ERASE | RDW_FRAME,
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
}

/// Position the pill at the bottom-center of the screen the cursor is
/// currently on. macOS coordinate space matches what Tauri's
/// LogicalPosition expects (top-left origin, points) — no Y flip
/// needed because CGEventGetLocation and CGDisplayBounds both use
/// global display coordinates anchored to the upper-left corner.
#[cfg(target_os = "macos")]
pub fn position_at_cursor_monitor(window: &tauri::WebviewWindow) {
    use core_graphics::display::CGDisplay;
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let src = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s,
        Err(_) => return,
    };
    let cursor = match CGEvent::new(src) {
        Ok(e) => e.location(),
        Err(_) => return,
    };

    let displays = match CGDisplay::active_displays() {
        Ok(d) => d,
        Err(_) => return,
    };

    let containing = displays
        .into_iter()
        .map(CGDisplay::new)
        .find(|d| {
            let b = d.bounds();
            cursor.x >= b.origin.x
                && cursor.x < b.origin.x + b.size.width
                && cursor.y >= b.origin.y
                && cursor.y < b.origin.y + b.size.height
        })
        .unwrap_or_else(|| CGDisplay::new(CGDisplay::main().id));
    let mon = containing.bounds();

    // Pill dimensions in *logical* units (= macOS points). Tauri's
    // window config (tauri.conf.json) lists 800×200 in the same units,
    // so no scale-factor multiplication is needed here — unlike the
    // Windows path where rcMonitor is in physical pixels.
    let pill_w: f64 = 800.0;
    let pill_h: f64 = 200.0;
    let margin_bottom: f64 = 24.0;
    let x = mon.origin.x + (mon.size.width - pill_w) / 2.0;
    let y = mon.origin.y + mon.size.height - pill_h - margin_bottom;
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
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
