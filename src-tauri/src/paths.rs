// User-writable data directory used for flov.toml, stats.json, the
// downloaded Whisper models and the log file.
//
// On Windows the installer drops everything next to the binary in a
// per-user folder (`%LOCALAPPDATA%\flov\`), so "exe dir" is writable
// and we keep the original layout — no migration needed for existing
// installs.
//
// On macOS the .app bundle (`/Applications/flov.app/Contents/MacOS/flov`)
// is sealed once Gatekeeper has signed it; writing inside the bundle
// breaks the signature. Use `~/Library/Application Support/com.flov.app/`.
//
// On Linux we follow XDG: `$XDG_DATA_HOME/flov/` or the documented
// fallback `~/.local/share/flov/`. The current Linux build doesn't
// ship as an installed package yet but this future-proofs paths.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::OnceLock;

// XDG (Linux) layout puts user data under `<XDG>/flov/`. macOS uses
// the bundle identifier, Windows reuses the installer's exe dir, so
// the constant is only referenced inside the Linux fallback.
#[cfg(all(unix, not(target_os = "macos")))]
const APP_DIR_NAME: &str = "flov";
#[cfg(target_os = "macos")]
const MACOS_BUNDLE_ID: &str = "com.flov.app";

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Returns the user-writable directory where flov stores its config,
/// stats, logs and downloaded models. Created on first call.
pub fn user_data_dir() -> Result<PathBuf> {
    if let Some(d) = DATA_DIR.get() {
        return Ok(d.clone());
    }
    let dir = compute_data_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("create data dir {:?}", dir))?;
    let _ = DATA_DIR.set(dir.clone());
    Ok(dir)
}

#[cfg(target_os = "windows")]
fn compute_data_dir() -> Result<PathBuf> {
    // Keep the historical layout — installer-managed exe dir is writable
    // under %LOCALAPPDATA%\flov\.
    let exe = std::env::current_exe().context("current_exe failed")?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .context("current_exe has no parent")
}

#[cfg(target_os = "macos")]
fn compute_data_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join(MACOS_BUNDLE_ID))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn compute_data_dir() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        let p = PathBuf::from(xdg);
        if p.is_absolute() {
            return Ok(p.join(APP_DIR_NAME));
        }
    }
    let home = std::env::var_os("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join(APP_DIR_NAME))
}

/// Path to `flov.toml`. Created lazily by writers if missing.
pub fn config_path() -> Result<PathBuf> {
    Ok(user_data_dir()?.join("flov.toml"))
}

/// Path to `stats.json`. Created on first transcription.
pub fn stats_path() -> Result<PathBuf> {
    Ok(user_data_dir()?.join("stats.json"))
}

/// Path to the rolling log file (single-file, overwritten each run —
/// matches the previous Windows behaviour).
pub fn log_path() -> Result<PathBuf> {
    Ok(user_data_dir()?.join("flov.log"))
}

/// `<data_dir>/models/<family>/` — directory for downloaded Whisper
/// models. Created if missing.
pub fn models_dir(family: &str) -> Result<PathBuf> {
    let dir = user_data_dir()?.join("models").join(family);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {:?}", dir))?;
    Ok(dir)
}
