use anyhow::{Context, Result};
use std::time::Duration;

pub struct PostProcessor {
    api_key: String,
    model: String,
    system_prompt: String,
    agent: ureq::Agent,
}

impl PostProcessor {
    pub fn new(api_key: String, model: String, system_prompt: String) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .build();
        Self {
            api_key,
            model,
            system_prompt,
            agent: config.into(),
        }
    }

    pub fn process(&self, text: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": self.system_prompt },
                { "role": "user",   "content": text }
            ]
        });

        let key_tail = self
            .api_key
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        tracing::info!(
            "openrouter: POST chat/completions model={} key=…{} chars={}",
            self.model,
            key_tail,
            text.chars().count()
        );

        let started = std::time::Instant::now();
        let mut resp = match self
            .agent
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("openrouter: send failed: {:?}", e);
                return Err(e).context("Failed to send request to OpenRouter");
            }
        };

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp
                .body_mut()
                .read_to_string()
                .unwrap_or_else(|_| "<no body>".into());
            tracing::error!(
                "openrouter: HTTP {} after {:?} body={}",
                status.as_u16(),
                started.elapsed(),
                body_text
            );
            anyhow::bail!(
                "OpenRouter returned HTTP {}: {}",
                status.as_u16(),
                body_text
            );
        }

        let response: serde_json::Value = resp
            .body_mut()
            .read_json()
            .context("Failed to parse OpenRouter response")?;

        let content = response["choices"][0]["message"]["content"]
            .as_str()
            .context("No content in OpenRouter response")?
            .trim()
            .to_string();
        tracing::info!(
            "openrouter: ok in {:?} ({} chars in → {} chars out)",
            started.elapsed(),
            text.chars().count(),
            content.chars().count()
        );
        Ok(content)
    }
}
