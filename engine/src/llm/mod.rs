//! LLM Provider Abstraction Layer
//!
//! This module provides a common interface for interacting with multiple LLM providers
//! (Ollama, OpenAI, Anthropic, Gemini, NVIDIA NIM). The LLMProvider trait defines
//! the contract that all providers must implement, enabling the LLM router to work
//! with multiple providers transparently.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod anthropic;
pub mod gemini;
pub mod nvidia_nim;
pub mod ollama;
pub mod openai;
pub mod router;

/// Result type for LLM operations
pub type Result<T> = std::result::Result<T, LLMError>;

/// Errors that can occur during LLM operations
#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    #[error("Provider unavailable: {0}")]
    ProviderUnavailable(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Message in a conversation history
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// Role of the message sender (user, assistant, system, tool)
    pub role: MessageRole,

    /// Content of the message
    pub content: String,

    /// Optional tool call ID for tool result messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
        }
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
        }
    }

    /// Create a new system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
        }
    }

    /// Create a new tool result message
    pub fn tool_result(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// User message
    User,

    /// Assistant message
    Assistant,

    /// System message
    System,

    /// Tool result message
    Tool,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::System => write!(f, "system"),
            MessageRole::Tool => write!(f, "tool"),
        }
    }
}

/// Response from an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LLMResponse {
    /// LLM wants to call a tool
    ToolCall(ToolCall),

    /// LLM has provided a final answer
    FinalAnswer(FinalAnswer),
}

/// Tool call request from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,

    /// Name of the tool to call
    pub name: String,

    /// Arguments to pass to the tool (JSON string)
    pub arguments: String,
}

impl ToolCall {
    /// Create a new tool call
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }
}

/// Final answer from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalAnswer {
    /// The answer content
    pub content: String,
}

impl FinalAnswer {
    /// Create a new final answer
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

/// LLM Provider trait that all providers must implement
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Returns the name of the provider (e.g., "ollama", "openai", "anthropic")
    fn name(&self) -> &str;

    /// Returns true if this is a local provider (e.g., Ollama), false for cloud providers
    fn is_local(&self) -> bool;

    /// Returns the estimated cost per 1K tokens in USD
    /// Local providers should return 0.0
    fn estimated_cost(&self, tokens: usize) -> f64;

    /// Generate a response from the LLM
    ///
    /// # Arguments
    /// * `messages` - Conversation history including system prompt, user messages, and tool results
    ///
    /// # Returns
    /// * `Ok(LLMResponse)` - Either a tool call or final answer
    /// * `Err(LLMError)` - If the request fails
    async fn generate(&self, messages: &[Message]) -> Result<LLMResponse>;

    /// Check if the provider is currently healthy and available
    /// Default implementation returns true.
    async fn check_health(&self) -> bool {
        true
    }
}

/// Helper function to parse tool calls from string content.
///
/// Handles multiple LLM output formats:
/// 1. Raw JSON: `{"function": "...", "arguments": {...}}`
/// 2. Fenced JSON (with or without trailing text): ` ```json\n{...}\n``` `
/// 3. `<tool_call>name({...})</tool_call>` XML markers
/// 4. JSON embedded in prose — scans for `{"function":` anywhere
pub fn parse_tool_calls(content: &str) -> Option<ToolCall> {
    let trimmed = content.trim();

    // Pattern 1: Raw JSON (entire content is valid JSON with "function" key)
    if let Some(tc) = try_parse_function_json(trimmed) {
        return Some(tc);
    }

    // Pattern 2: Extract from markdown code fences (even with trailing text)
    if let Some(inner) = extract_fenced_json(trimmed) {
        if let Some(tc) = try_parse_function_json(inner.trim()) {
            return Some(tc);
        }
    }

    // Pattern 3: <tool_call>name({...})</tool_call> XML markers
    if let Some(start) = trimmed.find("<tool_call>") {
        if let Some(end) = trimmed.find("</tool_call>") {
            let tool_content = &trimmed[start + 11..end];
            if let Some(paren_pos) = tool_content.find('(') {
                let tool_name = &tool_content[..paren_pos];
                let args_end = tool_content.rfind(')').unwrap_or(tool_content.len());
                let arguments = &tool_content[paren_pos + 1..args_end];

                return Some(ToolCall::new(
                    format!("call_{}", uuid::Uuid::new_v4()),
                    tool_name.trim(),
                    arguments,
                ));
            }
        }
    }

    // Pattern 4: Scan for {"function": anywhere in the content (LLM mixed prose + JSON)
    if let Some(pos) = trimmed.find("{\"function\"") {
        let candidate = &trimmed[pos..];
        // Find matching closing brace by counting depth
        if let Some(json_str) = extract_balanced_json(candidate) {
            if let Some(tc) = try_parse_function_json(json_str) {
                return Some(tc);
            }
        }
    }

    None
}

/// Try to parse a string as a `{"function": "...", "arguments": {...}}` tool call.
fn try_parse_function_json(s: &str) -> Option<ToolCall> {
    let json: serde_json::Value = serde_json::from_str(s).ok()?;
    let function = json.get("function")?.as_str()?;
    let arguments = json.get("arguments")?;
    Some(ToolCall::new(
        format!("call_{}", uuid::Uuid::new_v4()),
        function,
        arguments.to_string(),
    ))
}

/// Extract the body of the first markdown code fence in the text.
///
/// Works even when there is trailing prose after the closing ```.
/// Returns `None` if no fenced block is found.
fn extract_fenced_json(content: &str) -> Option<&str> {
    // Find opening fence
    let fence_start = content.find("```")?;
    let after_opening = &content[fence_start + 3..];

    // Skip the language tag line (e.g. "json\n")
    let body_start_rel = after_opening.find('\n')? + 1;
    let body_start = fence_start + 3 + body_start_rel;

    // Find closing fence after the body starts
    let closing = content[body_start..].find("```")?;
    let body_end = body_start + closing;

    if body_start >= body_end {
        return None;
    }

    Some(&content[body_start..body_end])
}

/// Extract a balanced JSON object starting at position 0 of `s`.
///
/// Counts `{` / `}` depth, respecting string literals, to find the
/// matching close brace.
fn extract_balanced_json(s: &str) -> Option<&str> {
    if !s.starts_with('{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in s.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello");
        assert_eq!(user_msg.role, MessageRole::User);
        assert_eq!(user_msg.content, "Hello");
        assert_eq!(user_msg.tool_call_id, None);

        let assistant_msg = Message::assistant("Hi there");
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
        assert_eq!(assistant_msg.content, "Hi there");

        let system_msg = Message::system("You are a helpful assistant");
        assert_eq!(system_msg.role, MessageRole::System);

        let tool_msg = Message::tool_result("result", "call_123");
        assert_eq!(tool_msg.role, MessageRole::Tool);
        assert_eq!(tool_msg.tool_call_id, Some("call_123".to_string()));
    }

    #[test]
    fn test_tool_call_creation() {
        let tool_call = ToolCall::new("call_123", "read_file", r#"{"path": "test.txt"}"#);
        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.name, "read_file");
        assert_eq!(tool_call.arguments, r#"{"path": "test.txt"}"#);
    }

    #[test]
    fn test_final_answer_creation() {
        let answer = FinalAnswer::new("The answer is 42");
        assert_eq!(answer.content, "The answer is 42");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_llm_response_serialization() {
        let tool_call = LLMResponse::ToolCall(ToolCall::new("id", "name", "{}"));
        let json = serde_json::to_string(&tool_call).unwrap();
        assert!(json.contains(r#""type":"tool_call"#));

        let final_answer = LLMResponse::FinalAnswer(FinalAnswer::new("answer"));
        let json = serde_json::to_string(&final_answer).unwrap();
        assert!(json.contains(r#""type":"final_answer"#));
    }
}
