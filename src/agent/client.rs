//! OpenAI-compatible HTTP client (BYOK).
//!
//! Configuration via environment variables:
//!   - `LENS_AI_API_KEY` (required)
//!   - `LENS_AI_BASE_URL` (default: https://api.openai.com/v1)
//!   - `LENS_AI_MODEL` (default: gpt-4o)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Chat completion request (OpenAI-compatible).
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Chat completion response.
#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

/// Configuration for the AI client.
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl AiConfig {
    /// Load config from environment variables.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("LENS_AI_API_KEY")
            .context("LENS_AI_API_KEY environment variable is required for AI features")?;
        let base_url = std::env::var("LENS_AI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("LENS_AI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
        Ok(Self {
            api_key,
            base_url,
            model,
        })
    }

    /// Check if AI is configured (API key present).
    pub fn is_configured() -> bool {
        std::env::var("LENS_AI_API_KEY").is_ok()
    }
}

/// Send a chat completion request to an OpenAI-compatible API.
pub async fn chat(config: &AiConfig, system: &str, user: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let req = ChatRequest {
        model: config.model.clone(),
        messages: vec![
            Message {
                role: "system".into(),
                content: system.into(),
            },
            Message {
                role: "user".into(),
                content: user.into(),
            },
        ],
        temperature: 0.2, // Low temperature for deterministic code generation
        max_tokens: 4096,
    };

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&req)
        .send()
        .await
        .context("Failed to send request to AI API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("AI API error ({}): {}", status, body);
    }

    let chat_resp: ChatResponse = resp
        .json()
        .await
        .context("Failed to parse AI API response")?;

    chat_resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| anyhow::anyhow!("No response from AI API"))
}
