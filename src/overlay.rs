use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const OVERLAY_WIDTH: i32 = 240;
const OVERLAY_HEIGHT: i32 = 60;
const BAR_COUNT: usize = 9;
const BAR_WIDTH: i32 = 8;
const BAR_GAP: i32 = 18;
const CLASS_NAME: &str = "FlovOverlay";

// Global state for animation
static mut ANIMATION_PHASE: f32 = 0.0;
static mut ANIMATION_RUNNING: bool = false;

pub struct Overlay {
    hwnd: isize,
    is_visible: Arc<AtomicBool>,
}

unsafe impl Send for Overlay {}
unsafe impl Sync for Overlay {}

impl Overlay {
    pub fn new() -> windows::core::Result<Self> {
        let is_visible = Arc::new(AtomicBool::new(false));
        let hwnd = unsafe { create_overlay_window()? };

        Ok(Self {
            hwnd: hwnd.0 as isize,
            is_visible,
        })
    }

    fn get_hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut std::ffi::c_void)
    }

    pub fn show(&self) {
        self.is_visible.store(true, Ordering::SeqCst);
        unsafe {
            ANIMATION_RUNNING = true;
            ANIMATION_PHASE = 0.0;
            let _ = ShowWindow(self.get_hwnd(), SW_SHOWNOACTIVATE);
            let _ = SetTimer(Some(self.get_hwnd()), 1, 33, None);
        }
    }

    pub fn hide(&self) {
        self.is_visible.store(false, Ordering::SeqCst);
        unsafe {
            ANIMATION_RUNNING = false;
            let _ = KillTimer(Some(self.get_hwnd()), 1);
            let _ = ShowWindow(self.get_hwnd(), SW_HIDE);
        }
    }
}

unsafe fn create_overlay_window() -> windows::core::Result<HWND> {
    let instance = GetModuleHandleW(None)?;
    let class_name: Vec<u16> = CLASS_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hbrBackground: HBRUSH(std::ptr::null_mut()),
        lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };

    RegisterClassExW(&wc);

    let screen_width = GetSystemMetrics(SM_CXSCREEN);
    let screen_height = GetSystemMetrics(SM_CYSCREEN);

    let x = (screen_width - OVERLAY_WIDTH) / 2;
    let y = (screen_height - OVERLAY_HEIGHT) / 2;

    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
        windows::core::PCWSTR(class_name.as_ptr()),
        windows::core::PCWSTR::null(),
        WS_POPUP,
        x,
        y,
        OVERLAY_WIDTH,
        OVERLAY_HEIGHT,
        None,
        None,
        Some(instance.into()),
        None,
    )?;

    // Use color key for transparency (black = transparent)
    SetLayeredWindowAttributes(hwnd, COLORREF(0x000000), 255, LWA_COLORKEY)?;

    Ok(hwnd)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            if ANIMATION_RUNNING {
                ANIMATION_PHASE += 0.12;
                if ANIMATION_PHASE > std::f32::consts::PI * 2.0 {
                    ANIMATION_PHASE -= std::f32::consts::PI * 2.0;
                }
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            // Prevent flicker by not erasing background
            LRESULT(1)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            // Create memory DC for double buffering
            let mem_dc = CreateCompatibleDC(Some(hdc));
            let mem_bitmap = CreateCompatibleBitmap(hdc, rect.right, rect.bottom);
            let old_bitmap = SelectObject(mem_dc, mem_bitmap.into());

            // Fill with black (transparent)
            let bg_brush = CreateSolidBrush(COLORREF(0x000000));
            FillRect(mem_dc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush.into());

            // Draw wave bars
            let center_y = rect.bottom / 2;
            let max_height = center_y - 4;
            let total_width = (BAR_COUNT as i32) * BAR_WIDTH + (BAR_COUNT as i32 - 1) * (BAR_GAP - BAR_WIDTH);
            let start_x = (rect.right - total_width) / 2;

            // No pen for clean edges
            let null_pen = CreatePen(PS_NULL, 0, COLORREF(0));
            let old_pen = SelectObject(mem_dc, null_pen.into());

            for i in 0..BAR_COUNT {
                let phase_offset = (i as f32) * 0.7;
                let wave = ((ANIMATION_PHASE + phase_offset).sin() + 1.0) / 2.0;
                let height = ((wave * 0.8 + 0.2) * max_height as f32) as i32;

                let x = start_x + (i as i32) * BAR_GAP;
                let top = center_y - height;
                let bottom = center_y + height;

                // Gradient from cyan to pink
                let t = i as f32 / (BAR_COUNT - 1) as f32;
                let r = (100.0 + 155.0 * t) as u8;
                let g = (220.0 - 80.0 * t) as u8;
                let b = 255u8;
                let color = COLORREF((b as u32) << 16 | (g as u32) << 8 | (r as u32));

                let brush = CreateSolidBrush(color);
                let old_brush = SelectObject(mem_dc, brush.into());

                // Draw rounded rectangle (pill shape)
                let _ = RoundRect(mem_dc, x, top, x + BAR_WIDTH, bottom, BAR_WIDTH, BAR_WIDTH);

                SelectObject(mem_dc, old_brush);
                let _ = DeleteObject(brush.into());
            }

            SelectObject(mem_dc, old_pen);
            let _ = DeleteObject(null_pen.into());

            // Copy to screen
            let _ = BitBlt(hdc, 0, 0, rect.right, rect.bottom, Some(mem_dc), 0, 0, SRCCOPY);

            // Cleanup
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(mem_bitmap.into());
            let _ = DeleteDC(mem_dc);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
