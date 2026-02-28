use async_trait::async_trait;
use reqwest::{header, Client};
use serde_json::{json, Value};
use std::time::Duration;

use super::{FinalAnswer, LLMError, LLMProvider, LLMResponse, Message, MessageRole, Result, ToolCall};
use crate::config::NvidiaNimConfig;

/// NVIDIA NIM provider configuration
#[derive(Debug, Clone)]
pub struct NvidiaNimProvider {
    config: NvidiaNimConfig,
    client: Client,
}

impl NvidiaNimProvider {
    pub fn new(config: NvidiaNimConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    fn get_api_key(&self) -> Result<String> {
        let entry = keyring::Entry::new("rove", "nim").map_err(|e| {
            LLMError::AuthenticationFailed(format!("Failed to initialize keyring: {}", e))
        })?;

        entry.get_password().map_err(|e| {
            LLMError::AuthenticationFailed(format!(
                "Failed to retrieve NIM API key (did you run `rove config set-secret nim`?): {}",
                e
            ))
        })
    }

    fn convert_message(msg: &Message) -> Value {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        if let Some(ref tool_id) = msg.tool_call_id {
            json!({
                "role": role,
                "tool_call_id": tool_id,
                "content": msg.content
            })
        } else {
            json!({
                "role": role,
                "content": msg.content
            })
        }
    }
}

#[async_trait]
impl LLMProvider for NvidiaNimProvider {
    fn name(&self) -> &str {
        "nvidia_nim"
    }

    fn is_local(&self) -> bool {
        false
    }

    fn estimated_cost(&self, tokens: usize) -> f64 {
        // Assume ~$0.001 per 1k tokens for Llama 3.1 70B via NIM
        (tokens as f64 / 1000.0) * 0.001
    }

    async fn generate(&self, messages: &[Message]) -> Result<LLMResponse> {
        let api_key = self.get_api_key()?;
        
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));

        let api_messages: Vec<Value> = messages.iter().map(Self::convert_message).collect();

        let payload = json!({
            "model": self.config.model,
            "messages": api_messages,
            "temperature": 0.0,
        });

        let resp = self
            .client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout
                } else if e.is_connect() {
                    LLMError::ProviderUnavailable(format!("Cannot connect to NVIDIA NIM: {}", e))
                } else {
                    LLMError::NetworkError(e.to_string())
                }
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_text = resp.text().await.unwrap_or_default();
            
            if status.as_u16() == 401 {
                return Err(LLMError::AuthenticationFailed("Invalid NIM API key.".to_string()));
            } else if status.as_u16() == 429 {
                return Err(LLMError::RateLimitExceeded);
            }
            
            return Err(LLMError::ProviderUnavailable(format!(
                "NIM API error ({}): {}",
                status, error_text
            )));
        }

        let body: Value = resp.json().await.map_err(|e| {
            LLMError::ParseError(format!("Failed to parse NIM response: {}", e))
        })?;

        let message = &body["choices"][0]["message"];
        let content = message["content"].as_str().unwrap_or_default().to_string();

        if let Some(tool_call) = super::parse_tool_calls(&content) {
            Ok(LLMResponse::ToolCall(tool_call))
        } else {
            Ok(LLMResponse::FinalAnswer(FinalAnswer::new(content)))
        }
    }
}
