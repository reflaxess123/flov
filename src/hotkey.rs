use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;

pub struct HotkeyState {
    pub is_pressed: Arc<AtomicBool>,
    pub is_recording: Arc<AtomicBool>,
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            is_pressed: Arc::new(AtomicBool::new(false)),
            is_recording: Arc::new(AtomicBool::new(false)),
        }
    }
}

static mut HOOK_STATE: Option<HotkeyState> = None;

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode as u16;

        let ctrl_pressed = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
        let win_pressed = vk == VK_LWIN || vk == VK_RWIN;

        if let Some(ref state) = HOOK_STATE {
            match wparam.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    // Only trigger if Ctrl+Win and NOT already recording
                    if win_pressed && ctrl_pressed && !state.is_recording.load(Ordering::SeqCst) {
                        state.is_pressed.store(true, Ordering::SeqCst);
                        state.is_recording.store(true, Ordering::SeqCst);
                        // Block Win key from opening Start menu
                        return LRESULT(1);
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => {
                    if win_pressed && state.is_recording.load(Ordering::SeqCst) {
                        state.is_pressed.store(false, Ordering::SeqCst);
                        // DON'T block keyup - let it pass through so Win key unsticks
                        // Just pass through to system
                    }
                }
                _ => {}
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

pub fn install_hook(state: HotkeyState) -> windows::core::Result<HHOOK> {
    unsafe {
        HOOK_STATE = Some(state);
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0)?;
        Ok(hook)
    }
}
