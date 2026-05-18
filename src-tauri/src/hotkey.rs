use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

/// Active mode: 0 = idle, 1 = recording (the hotkey is held).
pub const MODE_IDLE: u8 = 0;
pub const MODE_TRANSCRIBE: u8 = 1;

pub struct HotkeyState {
    pub active_mode: Arc<AtomicU8>,
    /// Used by the recording_loop to track whether it currently has the
    /// recorder open. The hook does NOT consult this any more — see the
    /// `trigger_held` field for that — because checking it from the
    /// hook caused a stuck state when the user tapped the hotkey during
    /// the previous transcription (the new press set `is_recording=true`
    /// but recording_loop was still in the previous iteration's
    /// transcribe step, so when it eventually looped back and saw
    /// `mode=IDLE` the flag stayed true and every subsequent hotkey
    /// press was ignored). The state-watchdog now relies on it as a
    /// last resort but the trigger_held flag means it should rarely
    /// fire.
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
    _handle: std::thread::JoinHandle<()>,
    #[cfg(target_os = "linux")]
    _handle: std::thread::JoinHandle<()>,
}

// ─── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use std::sync::{OnceLock, RwLock};
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Set once on `install_hook`. `OnceLock` instead of `static mut` to
    /// avoid the rust-2024 UB warnings around shared references to
    /// mutable statics.
    static HOOK_STATE: OnceLock<HotkeyState> = OnceLock::new();
    /// Suppresses Windows' KEYDOWN auto-repeat: once a trigger key is
    /// down we ignore further KEYDOWN events for it until we see the
    /// matching KEYUP. Lives only in the hook, separate from
    /// `is_recording` so the recording_loop's progress can't desync the
    /// hook's idea of "is the key physically held right now".
    static TRIGGER_HELD: AtomicBool = AtomicBool::new(false);
    /// Active hotkey definition. `RwLock` (not `Mutex`) so the hook
    /// callback can `try_read` without ever blocking — the UI command
    /// that swaps the hotkey holds the write lock for microseconds, but
    /// in a low-level keyboard hook even *microseconds* of waiting can
    /// trip Windows' `LowLevelHooksTimeout` and get the whole hook
    /// silently uninstalled. `try_read` falls through to the cached
    /// `CallNextHookEx` instead of waiting.
    static HOOK_DEF: RwLock<Option<HotkeyDef>> = RwLock::new(None);

    /// `KBDLLHOOKSTRUCT.flags` bit set when the event was synthesised by
    /// `SendInput` rather than typed by a human. We must filter these:
    /// every paste fires Ctrl-down/V-down/V-up/Ctrl-up via SendInput,
    /// and if those traversed our trigger logic we'd both (a) waste
    /// hook callback time on every paste and (b) risk re-arming the
    /// recorder mid-paste when somebody binds Ctrl as the trigger.
    const LLKHF_INJECTED: u32 = 0x10;

    pub fn set_hotkey_def(def: HotkeyDef) {
        if let Ok(mut g) = HOOK_DEF.write() {
            *g = Some(def);
        }
    }

    fn modifier_held(vk: u16) -> bool {
        unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // Hot path — keep it short. Anything that can wait or allocate
        // belongs OUTSIDE this function, otherwise Windows will quietly
        // disable the hook on the first slow callback.
        if code < 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);

        // Skip events we generated ourselves (paste's synthetic Ctrl+V).
        if (kb.flags.0 & LLKHF_INJECTED) != 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        // Don't block waiting on the writer — fall through cleanly if
        // the UI is currently swapping the hotkey definition.
        let def_guard = match HOOK_DEF.try_read() {
            Ok(g) => g,
            Err(_) => return CallNextHookEx(None, code, wparam, lparam),
        };
        let Some(def) = def_guard.as_ref() else {
            return CallNextHookEx(None, code, wparam, lparam);
        };
        let Some(state) = HOOK_STATE.get() else {
            return CallNextHookEx(None, code, wparam, lparam);
        };

        let vk = kb.vkCode as u16;
        let evt = wparam.0 as u32;
        let is_trigger = def.trigger_vks.contains(&vk);
        if is_trigger {
            match evt {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    // `TRIGGER_HELD` collapses Windows' KEYDOWN auto-repeat
                    // (held key = ~30 KEYDOWNs/s) into a single arming
                    // event. We deliberately do NOT consult
                    // `is_recording` here: that flag belongs to the
                    // recording_loop, and using it as a hook gate caused
                    // a wedge when a press during the previous
                    // transcribe left it stuck true forever.
                    if !TRIGGER_HELD.load(Ordering::SeqCst) {
                        let all_held = def.modifier_vks.iter().all(|&m| modifier_held(m));
                        if all_held {
                            TRIGGER_HELD.store(true, Ordering::SeqCst);
                            state.active_mode.store(MODE_TRANSCRIBE, Ordering::SeqCst);
                            // Swallow so e.g. Win doesn't open Start menu,
                            // Space doesn't insert a space, etc.
                            return LRESULT(1);
                        }
                    } else {
                        // Auto-repeat for an already-held trigger — still
                        // swallow so the keystroke doesn't reach the
                        // foreground app (otherwise repeating Space would
                        // type spaces while the user is dictating).
                        return LRESULT(1);
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => {
                    if TRIGGER_HELD.load(Ordering::SeqCst) {
                        TRIGGER_HELD.store(false, Ordering::SeqCst);
                        state.active_mode.store(MODE_IDLE, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    /// Install the hook on a dedicated thread that does nothing but pump
    /// messages.
    ///
    /// Why a dedicated thread: Windows enforces `LowLevelHooksTimeout`
    /// (~300ms by default) and silently uninstalls any LL hook whose
    /// owning thread is slow to dispatch — and "the owning thread" means
    /// the one that called `SetWindowsHookExW`, not the one running the
    /// callback. If the hook lived on the main thread, any spike on it
    /// (Tauri command handler, blocked .lock(), GC-like pause from
    /// transcribe activity on other cores) could let Windows decide the
    /// hook is unresponsive and quietly nuke it. The user then sees the
    /// hotkey "just stop working" with no error.
    ///
    /// On its own thread with a tight `GetMessage` loop, the hook
    /// thread is always responsive, regardless of what the rest of the
    /// app is doing. This is the standard fix for LL hooks on Windows.
    pub fn install_hook(state: HotkeyState) -> anyhow::Result<HookGuard> {
        use std::sync::mpsc;

        let _ = HOOK_STATE.set(state);

        let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);

        let handle = std::thread::Builder::new()
            .name("flov-keyhook".into())
            .spawn(move || unsafe {
                let mut hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0) {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let _ = tx.send(Ok(()));
                tracing::info!("keyboard hook installed on dedicated thread");

                // Self-healing watchdog: re-install the hook every 30s.
                // Empirically Windows quietly disables LL hooks under
                // certain conditions (reportedly Sleep/Wake, certain
                // anti-cheat/security drivers, long idles, the timeout
                // heuristic firing once for any reason). The cost of a
                // periodic reinstall is microseconds, and it guarantees
                // that the hook is always alive instead of relying on
                // the user to notice and restart the app.
                //
                // SetTimer with NULL hWnd posts WM_TIMER to this
                // thread's message queue every `REINSTALL_MS` ms.
                const REINSTALL_MS: u32 = 30_000;
                let timer_id = SetTimer(None, 0, REINSTALL_MS, None);
                if timer_id == 0 {
                    tracing::warn!("SetTimer for hook watchdog failed — running without periodic reinstall");
                }

                let mut msg = MSG::default();
                loop {
                    let r = GetMessageW(&mut msg, None, 0, 0);
                    if r.0 <= 0 { break; }

                    if msg.message == WM_TIMER {
                        let _ = UnhookWindowsHookEx(hook);
                        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0) {
                            Ok(h) => { hook = h; }
                            Err(e) => {
                                tracing::error!("hook reinstall failed: {} — retrying next tick", e);
                                // Fall back to a sentinel handle; next
                                // WM_TIMER will retry.
                                hook = windows::Win32::UI::WindowsAndMessaging::HHOOK::default();
                            }
                        }
                        continue;
                    }

                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                if timer_id != 0 {
                    let _ = KillTimer(None, timer_id);
                }
                let _ = UnhookWindowsHookEx(hook);
            })?;

        match rx.recv() {
            Ok(Ok(())) => Ok(HookGuard { _handle: handle }),
            Ok(Err(e)) => anyhow::bail!("SetWindowsHookExW failed: {}", e),
            Err(e) => anyhow::bail!("hook thread vanished: {}", e),
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
