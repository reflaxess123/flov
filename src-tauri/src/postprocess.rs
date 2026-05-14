use anyhow::{Context, Result};

pub struct PostProcessor {
    api_key: String,
    model: String,
    system_prompt: String,
    reply_system_prompt: String,
}

impl PostProcessor {
    pub fn new(
        api_key: String,
        model: String,
        system_prompt: String,
        reply_system_prompt: String,
    ) -> Self {
        Self {
            api_key,
            model,
            system_prompt,
            reply_system_prompt,
        }
    }

    pub fn process(&self, text: &str) -> Result<String> {
        self.call_api(&self.system_prompt, text)
    }

    pub fn reply(&self, clipboard_context: &str, instruction: &str) -> Result<String> {
        let user_content = format!(
            "КОНТЕКСТ:\n{}\n\nИНСТРУКЦИЯ:\n{}",
            clipboard_context, instruction
        );
        self.call_api(&self.reply_system_prompt, &user_content)
    }

    fn call_api(&self, system_prompt: &str, user_content: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": user_content
                }
            ]
        });

        let response: serde_json::Value = ureq::post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .context("Failed to send request to OpenRouter")?
            .body_mut()
            .read_json()
            .context("Failed to parse OpenRouter response")?;

        let result = response["choices"][0]["message"]["content"]
            .as_str()
            .context("No content in OpenRouter response")?
            .trim()
            .to_string();

        Ok(result)
    }
}
