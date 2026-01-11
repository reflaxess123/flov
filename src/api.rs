use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct ApiClient {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Option<Vec<Choice>>,
    error: Option<ApiError>,
}

#[derive(Deserialize, Debug)]
struct ApiError {
    message: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Message,
}

impl ApiClient {
    pub fn new(endpoint: String, api_key: String, model: String) -> Self {
        let client = reqwest::Client::new();
        Self {
            client,
            endpoint,
            api_key,
            model,
        }
    }

    pub async fn improve_text(&self, text: &str) -> Result<String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "Ты редактор текста. Улучши речь: исправь грамматику, добавь знаки препинания, замени мат на культурные выражения, сделай текст более официальным и вежливым. Верни ТОЛЬКО исправленный текст, без пояснений.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            max_tokens: 2048,
        };

        let response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request")?;

        let response_text = response.text().await.context("Failed to get response text")?;
        tracing::debug!("API response: {}", response_text);

        let chat_response: ChatResponse = serde_json::from_str(&response_text)
            .context("Failed to parse response")?;

        if let Some(error) = chat_response.error {
            anyhow::bail!("API error: {:?}", error.message);
        }

        chat_response
            .choices
            .and_then(|c| c.first().map(|choice| choice.message.content.clone()))
            .context("No response from API")
    }
}
