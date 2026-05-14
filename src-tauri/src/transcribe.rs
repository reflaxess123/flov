// Transcription via a swappable sidecar binary.
//
// The previous in-process whisper-rs link is replaced by a Command::spawn
// of `flov-whisper-<backend>.exe` that lives next to flov.exe. The protocol
// is documented in crates/flov-whisper-cpu/src/main.rs — args carry model
// + language, stdin carries raw f32 LE PCM, stdout returns text.
//
// Each transcription resolves the active sidecar fresh, so the tray menu
// can switch backends at runtime without recreating the Transcriber.

use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Backends tried, in order, when the user picked "auto" (or did not pick).
/// First one whose binary exists next to flov.exe wins.
pub const BACKEND_PRIORITY: &[&str] = &["cuda", "vulkan", "metal", "cpu"];

fn backend_bin_name(backend: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("flov-whisper-{}.exe", backend)
    } else {
        format!("flov-whisper-{}", backend)
    }
}

fn exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("current_exe failed")?;
    Ok(exe
        .parent()
        .context("current_exe has no parent")?
        .to_path_buf())
}

/// Names of backends whose sidecar binary exists next to flov.exe right now.
/// Used by the tray menu to grey-out unavailable choices.
pub fn available_backends() -> Vec<String> {
    let dir = match exe_dir() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    BACKEND_PRIORITY
        .iter()
        .filter(|b| dir.join(backend_bin_name(b)).exists())
        .map(|b| (*b).to_string())
        .collect()
}

pub struct Transcriber {
    model_path: Arc<Mutex<PathBuf>>,
    language: String,
    /// Shared with the tray menu; updated when the user picks a backend.
    backend_choice: Arc<Mutex<String>>,
}

impl Transcriber {
    pub fn new(
        model_path: Arc<Mutex<PathBuf>>,
        language: String,
        backend_choice: Arc<Mutex<String>>,
    ) -> Result<Self> {
        // Existence is checked per-transcribe so the user can launch the app
        // before downloading a model and pick one from the Models window.
        let preview = model_path.lock().unwrap().clone();
        tracing::info!("Whisper model: {:?}", preview);
        Ok(Self {
            model_path,
            language,
            backend_choice,
        })
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        let total_start = std::time::Instant::now();
        let choice = self.backend_choice.lock().unwrap().clone();
        let (backend, sidecar) = resolve_sidecar(&choice)?;
        let model_path = self.model_path.lock().unwrap().clone();
        if !model_path.exists() {
            anyhow::bail!(
                "model file not found: {:?} — open Models from the tray to download one",
                model_path
            );
        }
        tracing::info!(
            "transcribe via {} | model={:?} | sidecar={:?}",
            backend,
            model_path,
            sidecar
        );

        let mut cmd = Command::new(&sidecar);
        cmd.arg("--model")
            .arg(&model_path)
            .arg("--language")
            .arg(&self.language)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn sidecar: {:?}", sidecar))?;

        // Write raw f32 LE PCM to stdin in one shot, then close to signal EOF.
        {
            let mut stdin = child.stdin.take().context("sidecar stdin missing")?;
            let mut buf = Vec::with_capacity(samples.len() * 4);
            for s in samples {
                buf.extend_from_slice(&s.to_le_bytes());
            }
            stdin
                .write_all(&buf)
                .context("failed to write samples to sidecar")?;
            // Dropping stdin closes the pipe → sidecar's read_to_end returns.
        }

        // Drain stderr in a thread so the sidecar can't block on a full pipe.
        let mut stderr = child.stderr.take().context("sidecar stderr missing")?;
        let stderr_thread = std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf);
            buf
        });

        let mut stdout_buf = String::new();
        if let Some(mut out) = child.stdout.take() {
            out.read_to_string(&mut stdout_buf)
                .context("failed to read sidecar stdout")?;
        }

        let status = child.wait().context("sidecar wait failed")?;
        let stderr_text = stderr_thread.join().unwrap_or_default();

        if !status.success() {
            anyhow::bail!(
                "sidecar exited with {:?}; stderr: {}",
                status.code(),
                stderr_text.trim()
            );
        }
        if !stderr_text.trim().is_empty() {
            tracing::debug!("sidecar stderr: {}", stderr_text.trim());
        }
        tracing::info!("transcription took {:?}", total_start.elapsed());
        Ok(stdout_buf.trim().to_string())
    }
}

/// Picks the sidecar to spawn given a user choice.
/// - "auto" → first available in BACKEND_PRIORITY
/// - "cuda" / "vulkan" / "metal" / "cpu" → that one specifically (errors if
///   missing — the menu greys out missing ones, so this only fires if the
///   binary was deleted between startup and the click).
/// - FLOV_BACKEND env var, when set, overrides the choice — useful for
///   one-off comparisons without touching the menu.
fn resolve_sidecar(choice: &str) -> Result<(String, PathBuf)> {
    let dir = exe_dir()?;
    let effective = std::env::var("FLOV_BACKEND").unwrap_or_else(|_| choice.to_string());

    if effective != "auto" {
        let candidate = dir.join(backend_bin_name(&effective));
        if !candidate.exists() {
            anyhow::bail!(
                "backend '{}' selected but {:?} not found",
                effective,
                candidate
            );
        }
        return Ok((effective, candidate));
    }

    let mut tried = Vec::new();
    for backend in BACKEND_PRIORITY {
        let candidate = dir.join(backend_bin_name(backend));
        if candidate.exists() {
            return Ok(((*backend).to_string(), candidate));
        }
        tried.push(candidate);
    }
    anyhow::bail!("no whisper sidecar found in {:?}; tried {:?}", dir, tried);
}
