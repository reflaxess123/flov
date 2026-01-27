use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    #[serde(default)]
    pub audio: AudioConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_url")]
    pub url: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
}

fn default_url() -> String {
    "http://localhost:8877/transcribe".to_string()
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

        // If no config exists, use defaults
        if !config_path.exists() {
            return Ok(Config {
                service: ServiceConfig {
                    url: default_url(),
                },
                audio: AudioConfig::default(),
            });
        }

        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {:?}", config_path))?;

        let config: Config = toml::from_str(&config_str)
            .context("Failed to parse config")?;

        Ok(config)
    }
}
