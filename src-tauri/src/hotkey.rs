use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

/// Active mode: 0 = idle, 1 = Ctrl+Win (transcribe), 2 = Ctrl+Alt (reply)
pub const MODE_IDLE: u8 = 0;
pub const MODE_TRANSCRIBE: u8 = 1;
pub const MODE_REPLY: u8 = 2;

pub struct HotkeyState {
    pub active_mode: Arc<AtomicU8>,
    pub is_recording: Arc<AtomicBool>,
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            active_mode: Arc::new(AtomicU8::new(MODE_IDLE)),
            is_recording: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Opaque guard — drop stops the hook/grab
pub struct HookGuard {
    #[cfg(target_os = "windows")]
    _hook: windows::Win32::UI::WindowsAndMessaging::HHOOK,
    #[cfg(target_os = "linux")]
    _handle: std::thread::JoinHandle<()>,
}

// ─── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    const VK_LWIN: u16 = 0x5B;
    const VK_RWIN: u16 = 0x5C;
    const VK_LMENU: u16 = 0xA4;
    const VK_RMENU: u16 = 0xA5;

    static mut HOOK_STATE: Option<HotkeyState> = None;

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode as u16;

            let ctrl_pressed = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
            let win_pressed = vk == VK_LWIN || vk == VK_RWIN;
            let alt_pressed = vk == VK_LMENU || vk == VK_RMENU;

            if let Some(ref state) = HOOK_STATE {
                match wparam.0 as u32 {
                    WM_KEYDOWN | WM_SYSKEYDOWN => {
                        if ctrl_pressed && !state.is_recording.load(Ordering::SeqCst) {
                            if win_pressed {
                                state.active_mode.store(MODE_TRANSCRIBE, Ordering::SeqCst);
                                state.is_recording.store(true, Ordering::SeqCst);
                                return LRESULT(1);
                            }
                            if alt_pressed {
                                state.active_mode.store(MODE_REPLY, Ordering::SeqCst);
                                state.is_recording.store(true, Ordering::SeqCst);
                                return LRESULT(1);
                            }
                        }
                    }
                    WM_KEYUP | WM_SYSKEYUP => {
                        let mode = state.active_mode.load(Ordering::SeqCst);
                        if win_pressed && mode == MODE_TRANSCRIBE {
                            state.active_mode.store(MODE_IDLE, Ordering::SeqCst);
                        }
                        if alt_pressed && mode == MODE_REPLY {
                            state.active_mode.store(MODE_IDLE, Ordering::SeqCst);
                        }
                    }
                    _ => {}
                }
            }
        }

        CallNextHookEx(None, code, wparam, lparam)
    }

    pub fn install_hook(state: HotkeyState) -> anyhow::Result<HookGuard> {
        unsafe {
            HOOK_STATE = Some(state);
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0)?;
            Ok(HookGuard { _hook: hook })
        }
    }
}

// ─── Linux ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use evdev::{Device, InputEventKind, Key};

    fn find_keyboards() -> Vec<Device> {
        let mut keyboards = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/dev/input") {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with("event") {
                        continue;
                    }
                }
                if let Ok(dev) = Device::open(&path) {
                    if dev.supported_keys().is_some_and(|keys| keys.contains(Key::KEY_A)) {
                        tracing::info!("Found keyboard: {} ({})", dev.name().unwrap_or("?"), path.display());
                        keyboards.push(dev);
                    }
                }
            }
        }
        keyboards
    }

    pub fn install_hook(state: HotkeyState) -> anyhow::Result<HookGuard> {
        let active_mode = state.active_mode.clone();
        let is_recording = state.is_recording.clone();

        let handle = std::thread::spawn(move || {
            let keyboards = find_keyboards();
            if keyboards.is_empty() {
                tracing::error!("No keyboard devices found in /dev/input/");
                return;
            }

            let ctrl_held = Arc::new(AtomicBool::new(false));

            let mut handles = Vec::new();
            for mut dev in keyboards {
                let ctrl = ctrl_held.clone();
                let mode = active_mode.clone();
                let recording = is_recording.clone();

                let h = std::thread::spawn(move || {
                    loop {
                        match dev.fetch_events() {
                            Ok(events) => {
                                for ev in events {
                                    if let InputEventKind::Key(key) = ev.kind() {
                                        let value = ev.value();
                                        match key {
                                            Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => {
                                                ctrl.store(value != 0, Ordering::SeqCst);
                                            }
                                            Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => {
                                                if value == 1 {
                                                    if ctrl.load(Ordering::SeqCst)
                                                        && !recording.load(Ordering::SeqCst)
                                                    {
                                                        tracing::info!("Hotkey: Ctrl+Super (transcribe)");
                                                        mode.store(MODE_TRANSCRIBE, Ordering::SeqCst);
                                                        recording.store(true, Ordering::SeqCst);
                                                    }
                                                } else if value == 0 {
                                                    if mode.load(Ordering::SeqCst) == MODE_TRANSCRIBE {
                                                        mode.store(MODE_IDLE, Ordering::SeqCst);
                                                    }
                                                }
                                            }
                                            Key::KEY_LEFTALT | Key::KEY_RIGHTALT => {
                                                if value == 1 {
                                                    if ctrl.load(Ordering::SeqCst)
                                                        && !recording.load(Ordering::SeqCst)
                                                    {
                                                        tracing::info!("Hotkey: Ctrl+Alt (reply)");
                                                        mode.store(MODE_REPLY, Ordering::SeqCst);
                                                        recording.store(true, Ordering::SeqCst);
                                                    }
                                                } else if value == 0 {
                                                    if mode.load(Ordering::SeqCst) == MODE_REPLY {
                                                        mode.store(MODE_IDLE, Ordering::SeqCst);
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("evdev read error: {}", e);
                                break;
                            }
                        }
                    }
                });
                handles.push(h);
            }

            for h in handles {
                let _ = h.join();
            }
        });

        Ok(HookGuard { _handle: handle })
    }
}

// ─── Public API ─────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub use windows_impl::install_hook;

#[cfg(target_os = "linux")]
pub use linux_impl::install_hook;
