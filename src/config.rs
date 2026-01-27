use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub audio: AudioConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_url")]
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_llm_enabled")]
    pub enabled: bool,
    #[serde(default = "default_llm_url")]
    pub url: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: default_llm_enabled(),
            url: default_llm_url(),
            model: default_llm_model(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
}

fn default_url() -> String {
    "http://localhost:8877/transcribe".to_string()
}

fn default_llm_enabled() -> bool {
    true
}

fn default_llm_url() -> String {
    "http://localhost:11435/api/generate".to_string()
}

fn default_llm_model() -> String {
    "gemma3:4b".to_string()
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
                llm: LlmConfig::default(),
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
