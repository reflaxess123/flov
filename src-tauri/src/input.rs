// ─── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    pub fn type_text(text: &str) {
        if set_clipboard(text) {
            paste();
        }
    }

    pub fn get_clipboard() -> Option<String> {
        unsafe {
            if OpenClipboard(None).is_err() {
                return None;
            }

            // CF_UNICODETEXT = 13
            let handle = GetClipboardData(13);
            let result = if let Ok(handle) = handle {
                let ptr = GlobalLock(HGLOBAL(handle.0));
                if !ptr.is_null() {
                    let wide_ptr = ptr as *const u16;
                    let mut len = 0;
                    while *wide_ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(wide_ptr, len);
                    let text = String::from_utf16_lossy(slice);
                    let _ = GlobalUnlock(HGLOBAL(handle.0));
                    Some(text)
                } else {
                    None
                }
            } else {
                None
            };

            let _ = CloseClipboard();
            result
        }
    }

    fn set_clipboard(text: &str) -> bool {
        unsafe {
            if OpenClipboard(None).is_err() {
                return false;
            }

            let _ = EmptyClipboard();

            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let size = wide.len() * 2;

            let hmem = GlobalAlloc(GMEM_MOVEABLE, size);
            if hmem.is_err() {
                let _ = CloseClipboard();
                return false;
            }
            let hmem = hmem.unwrap();

            let ptr = GlobalLock(hmem);
            if ptr.is_null() {
                let _ = GlobalFree(Some(hmem));
                let _ = CloseClipboard();
                return false;
            }

            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
            let _ = GlobalUnlock(hmem);

            // CF_UNICODETEXT = 13
            let result = SetClipboardData(13, Some(HANDLE(hmem.0)));
            let _ = CloseClipboard();

            result.is_ok()
        }
    }

    fn paste() {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: KEYEVENTF_EXTENDEDKEY,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];

        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

// ─── Linux ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_impl {
    use std::process::Command;

    pub fn get_clipboard() -> Option<String> {
        let output = Command::new("wl-paste")
            .arg("--no-newline")
            .output()
            .ok()?;
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            None
        }
    }

    pub fn type_text(text: &str) {
        // Use wl-copy to set clipboard, then wtype to paste
        let copy_result = Command::new("wl-copy")
            .arg(text)
            .status();

        match copy_result {
            Ok(status) if status.success() => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                let paste_result = Command::new("wtype")
                    .arg("-M")
                    .arg("ctrl")
                    .arg("-k")
                    .arg("v")
                    .arg("-m")
                    .arg("ctrl")
                    .status();

                if let Err(e) = paste_result {
                    tracing::error!("wtype failed: {}", e);
                }
            }
            Ok(status) => {
                tracing::error!("wl-copy exited with: {}", status);
            }
            Err(e) => {
                tracing::error!("wl-copy failed: {} (install wl-clipboard)", e);
            }
        }
    }
}

// ─── Public API ─────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub use windows_impl::{get_clipboard, type_text};

#[cfg(target_os = "linux")]
pub use linux_impl::{get_clipboard, type_text};
