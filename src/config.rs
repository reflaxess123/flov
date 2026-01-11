use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub whisper: WhisperConfig,
    pub audio: AudioConfig,
    #[serde(default)]
    pub api: Option<ApiConfig>,
}

#[derive(Debug, Deserialize)]
pub struct WhisperConfig {
    pub model_path: PathBuf,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiConfig {
    pub endpoint: String,
    pub key: String,
    pub model: String,
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
        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {:?}", config_path))?;

        let config: Config = toml::from_str(&config_str)
            .context("Failed to parse config")?;

        Ok(config)
    }
}
