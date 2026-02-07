use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub whisper: WhisperConfig,
    #[serde(default)]
    pub audio: AudioConfig,
}

#[derive(Debug, Deserialize)]
pub struct WhisperConfig {
    #[serde(default = "default_model_path")]
    pub model_path: PathBuf,
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_path: default_model_path(),
            language: default_language(),
        }
    }
}

fn default_language() -> String {
    "ru".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
}

fn default_model_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    exe_dir.join("ggml-large-v3-turbo.bin")
}

fn default_sample_rate() -> u32 {
    16000
}

impl Config {
    pub fn load() -> Result<Self> {
        let exe_dir = std::env::current_exe()
            .context("Failed to get executable path")?
            .parent()
            .context("Failed to get executable directory")?
            .to_path_buf();

        let config_path = exe_dir.join("flov.toml");

        if !config_path.exists() {
            return Ok(Config {
                whisper: WhisperConfig::default(),
                audio: AudioConfig::default(),
            });
        }

        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {:?}", config_path))?;

        let mut config: Config = toml::from_str(&config_str)
            .context("Failed to parse config")?;

        // Resolve relative model_path against exe directory
        if config.whisper.model_path.is_relative() {
            config.whisper.model_path = exe_dir.join(&config.whisper.model_path);
        }

        Ok(config)
    }
}
