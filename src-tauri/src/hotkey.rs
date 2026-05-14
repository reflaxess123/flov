use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// Active mode: 0 = idle, 1 = recording (the hotkey is held).
pub const MODE_IDLE: u8 = 0;
pub const MODE_TRANSCRIBE: u8 = 1;

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

/// Parsed hotkey definition. The combo's last token is the trigger key
/// (whose KEYDOWN starts recording / KEYUP stops it). All earlier tokens
/// must be held at trigger time.
#[derive(Debug, Clone, Default)]
pub struct HotkeyDef {
    /// Virtual-key codes that must be held (use the `_LEFT`/`_RIGHT`-agnostic
    /// VK constants, e.g. VK_CONTROL, so GetAsyncKeyState matches either).
    pub modifier_vks: Vec<u16>,
    /// Virtual-key codes that fire the trigger (any of them counts).
    pub trigger_vks: Vec<u16>,
    /// Original combo string, kept so the UI / config can read it back.
    pub combo: String,
}

impl HotkeyDef {
    /// Parse "Ctrl+Win" / "Ctrl+Alt+Space" / "Ctrl+Shift+K" etc.
    pub fn parse(combo: &str) -> Result<Self, String> {
        let parts: Vec<&str> = combo
            .split('+')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            return Err("empty hotkey combo".into());
        }
        let trigger_token = *parts.last().unwrap();
        let modifier_tokens = &parts[..parts.len() - 1];

        let mut modifier_vks = Vec::new();
        for m in modifier_tokens {
            modifier_vks.extend(modifier_vk_codes(m).ok_or_else(|| {
                format!("unknown modifier '{}' in '{}'", m, combo)
            })?);
        }

        let trigger_vks = trigger_vk_codes(trigger_token)
            .ok_or_else(|| format!("unknown key '{}' in '{}'", trigger_token, combo))?;

        Ok(Self {
            modifier_vks,
            trigger_vks,
            combo: combo.to_string(),
        })
    }
}

#[allow(dead_code)] // Linux build doesn't use these but the parser needs the table
fn modifier_vk_codes(token: &str) -> Option<Vec<u16>> {
    // Generic tokens use the "either left or right" composite VK
    // (VK_CONTROL = 0x11 etc.) — GetAsyncKeyState handles both sides.
    // L/R-specific tokens map to their side-specific VK so a binding like
    // "RCtrl" can mean "right ctrl only".
    match token.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(vec![0x11]),
        "lctrl"            => Some(vec![0xA2]),
        "rctrl"            => Some(vec![0xA3]),
        "alt" | "menu"     => Some(vec![0x12]),
        "lalt"             => Some(vec![0xA4]),
        "ralt"             => Some(vec![0xA5]),
        "shift"            => Some(vec![0x10]),
        "lshift"           => Some(vec![0xA0]),
        "rshift"           => Some(vec![0xA1]),
        "win" | "meta" | "super" | "cmd" => Some(vec![0x5B, 0x5C]),
        "lwin"             => Some(vec![0x5B]),
        "rwin"             => Some(vec![0x5C]),
        _ => None,
    }
}

#[allow(dead_code)]
fn trigger_vk_codes(token: &str) -> Option<Vec<u16>> {
    // Trigger keys can be any modifier OR a regular key — we normalise here.
    // The low-level keyboard hook reports L/R-specific VKs in
    // KBDLLHOOKSTRUCT.vkCode, so for the *generic* names we expand to both
    // sides; for the side-specific names we match exactly one VK.
    if let Some(mods) = modifier_vk_codes(token) {
        return Some(match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => vec![0xA2, 0xA3],
            "lctrl"            => vec![0xA2],
            "rctrl"            => vec![0xA3],
            "alt" | "menu"     => vec![0xA4, 0xA5],
            "lalt"             => vec![0xA4],
            "ralt"             => vec![0xA5],
            "shift"            => vec![0xA0, 0xA1],
            "lshift"           => vec![0xA0],
            "rshift"           => vec![0xA1],
            _ => mods, // win/lwin/rwin already correct
        });
    }
    let lower = token.to_ascii_lowercase();
    match lower.as_str() {
        "space" => Some(vec![0x20]),
        "enter" | "return" => Some(vec![0x0D]),
        "tab" => Some(vec![0x09]),
        "esc" | "escape" => Some(vec![0x1B]),
        "backspace" => Some(vec![0x08]),
        "delete" | "del" => Some(vec![0x2E]),
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap().to_ascii_uppercase();
            if c.is_ascii_alphanumeric() {
                Some(vec![c as u16])
            } else {
                None
            }
        }
        _ => None,
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

    static mut HOOK_STATE: Option<HotkeyState> = None;
    /// Active hotkey definition. Updated live by the Tauri command — the
    /// hook callback re-reads it on every keystroke so changes apply
    /// immediately, no re-registration needed.
    static HOOK_DEF: Mutex<Option<HotkeyDef>> = Mutex::new(None);

    pub fn set_hotkey_def(def: HotkeyDef) {
        *HOOK_DEF.lock().unwrap() = Some(def);
    }

    fn modifier_held(vk: u16) -> bool {
        unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode as u16;
            let evt = wparam.0 as u32;

            let def_opt = HOOK_DEF.lock().unwrap().clone();
            if let (Some(def), Some(state)) = (def_opt, &HOOK_STATE) {
                let is_trigger = def.trigger_vks.contains(&vk);
                if is_trigger {
                    match evt {
                        WM_KEYDOWN | WM_SYSKEYDOWN => {
                            let all_held = def
                                .modifier_vks
                                .iter()
                                .all(|&m| modifier_held(m));
                            if all_held && !state.is_recording.load(Ordering::SeqCst) {
                                state.active_mode.store(MODE_TRANSCRIBE, Ordering::SeqCst);
                                state.is_recording.store(true, Ordering::SeqCst);
                                // Swallow so e.g. Win doesn't open Start menu,
                                // Space doesn't insert a space, etc.
                                return LRESULT(1);
                            }
                        }
                        WM_KEYUP | WM_SYSKEYUP => {
                            if state.active_mode.load(Ordering::SeqCst) == MODE_TRANSCRIBE {
                                state.active_mode.store(MODE_IDLE, Ordering::SeqCst);
                            }
                        }
                        _ => {}
                    }
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

    /// Linux doesn't yet honour the configurable hotkey — it stays on
    /// Ctrl+Super (the original combo). Wire it up if/when we ship Linux.
    pub fn set_hotkey_def(_def: HotkeyDef) {}

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
pub use windows_impl::{install_hook, set_hotkey_def};

#[cfg(target_os = "linux")]
pub use linux_impl::{install_hook, set_hotkey_def};
