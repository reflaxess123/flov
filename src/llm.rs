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
r#"Исправь и отформатируй следующий текст, полученный из голосового ввода:

1. Исправь орфографические и грамматические ошибки
2. Добавь правильную пунктуацию (точки, запятые, вопросительные знаки)
3. Раздели на предложения
4. Если текст длинный, раздели на абзацы по смыслу
5. Сохрани исходный смысл и стиль речи
6. НЕ добавляй ничего от себя, только форматируй

Текст: {raw_text}

Отформатированный текст:"#
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

        let processed = result.response.trim().to_string();
        tracing::info!("LLM processed: {}", processed);

        Ok(processed)
    }
}
