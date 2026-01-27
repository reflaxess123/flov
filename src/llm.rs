use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

pub struct TextProcessor {
    url: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl TextProcessor {
    pub fn new(url: &str, model: &str) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .no_proxy()  // Bypass system proxy for localhost
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            url: url.to_string(),
            model: model.to_string(),
            client,
        })
    }

    pub fn process(&self, raw_text: &str) -> Result<String> {
        let prompt = format!(
r#"Ты - корректор текста. Исправь текст и верни ТОЛЬКО исправленный результат без пояснений.

Правила:
- Исправь ошибки
- Добавь пунктуацию
- Раздели на предложения
- Не добавляй ничего от себя

Входной текст: "{raw_text}"

Исправленный текст:"#
        );

        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
        };

        tracing::info!("Sending text to LLM for processing");

        let response = self.client
            .post(&self.url)
            .json(&request)
            .send()
            .context("Failed to send request to LLM")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!("LLM service returned {}: {}", status, body);
        }

        let result: OllamaResponse = response
            .json()
            .context("Failed to parse LLM response")?;

        // Clean up the response
        let mut processed = result.response.trim().to_string();

        // Remove quotes if wrapped
        if processed.starts_with('"') && processed.ends_with('"') {
            processed = processed[1..processed.len()-1].to_string();
        }

        // If response is empty or too short, return original
        if processed.len() < 2 {
            tracing::warn!("LLM returned empty response, using original");
            return Ok(raw_text.to_string());
        }

        tracing::info!("LLM processed: {}", processed);

        Ok(processed)
    }
}
