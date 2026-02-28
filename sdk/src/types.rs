//! Tool input/output types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Input to a tool function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    pub method: String,
    pub params: HashMap<String, serde_json::Value>,
}

impl ToolInput {
    /// Create a new ToolInput
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            params: HashMap::new(),
        }
    }

    /// Add a parameter
    pub fn with_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    /// Get a string parameter
    pub fn param_str(&self, key: &str) -> Result<String, ToolError> {
        self.params
            .get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))
    }

    /// Get an i64 parameter
    pub fn param_i64(&self, key: &str) -> Result<i64, ToolError> {
        self.params
            .get(key)
            .and_then(|v| v.as_i64())
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))
    }

    /// Get a bool parameter
    pub fn param_bool(&self, key: &str) -> Result<bool, ToolError> {
        self.params
            .get(key)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))
    }

    /// Get an optional string parameter
    pub fn param_str_opt(&self, key: &str) -> Option<String> {
        self.params
            .get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Get an optional i64 parameter
    pub fn param_i64_opt(&self, key: &str) -> Option<i64> {
        self.params.get(key).and_then(|v| v.as_i64())
    }

    /// Get an optional bool parameter
    pub fn param_bool_opt(&self, key: &str) -> Option<bool> {
        self.params.get(key).and_then(|v| v.as_bool())
    }

    /// Get a parameter as a JSON value
    pub fn param_json(&self, key: &str) -> Result<&serde_json::Value, ToolError> {
        self.params
            .get(key)
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))
    }
}

/// Output from a tool function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

impl ToolOutput {
    /// Create a successful output with text
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            success: true,
            data: serde_json::json!({ "text": text.into() }),
            error: None,
        }
    }

    /// Create a successful output with JSON data
    pub fn json(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    /// Create an error output
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(error.into()),
        }
    }

    /// Create an empty successful output
    pub fn empty() -> Self {
        Self {
            success: true,
            data: serde_json::Value::Null,
            error: None,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Tool-specific errors
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Missing parameter: {0}")]
    MissingParameter(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Unknown method: {0}")]
    UnknownMethod(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_input_new() {
        let input = ToolInput::new("test_method");
        assert_eq!(input.method, "test_method");
        assert!(input.params.is_empty());
    }

    #[test]
    fn test_tool_input_with_param() {
        let input = ToolInput::new("test_method")
            .with_param("key1", json!("value1"))
            .with_param("key2", json!(42));

        assert_eq!(input.params.len(), 2);
        assert_eq!(input.params.get("key1").unwrap(), &json!("value1"));
        assert_eq!(input.params.get("key2").unwrap(), &json!(42));
    }

    #[test]
    fn test_param_str_success() {
        let input = ToolInput::new("test").with_param("name", json!("Alice"));

        let result = input.param_str("name");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Alice");
    }

    #[test]
    fn test_param_str_missing() {
        let input = ToolInput::new("test");
        let result = input.param_str("missing");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::MissingParameter(_)
        ));
    }

    #[test]
    fn test_param_i64_success() {
        let input = ToolInput::new("test").with_param("count", json!(42));

        let result = input.param_i64("count");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_param_i64_missing() {
        let input = ToolInput::new("test");
        let result = input.param_i64("missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_param_bool_success() {
        let input = ToolInput::new("test").with_param("enabled", json!(true));

        let result = input.param_bool("enabled");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_param_bool_missing() {
        let input = ToolInput::new("test");
        let result = input.param_bool("missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_param_str_opt_some() {
        let input = ToolInput::new("test").with_param("name", json!("Bob"));

        let result = input.param_str_opt("name");
        assert_eq!(result, Some("Bob".to_string()));
    }

    #[test]
    fn test_param_str_opt_none() {
        let input = ToolInput::new("test");
        let result = input.param_str_opt("missing");
        assert_eq!(result, None);
    }

    #[test]
    fn test_param_i64_opt_some() {
        let input = ToolInput::new("test").with_param("count", json!(100));

        let result = input.param_i64_opt("count");
        assert_eq!(result, Some(100));
    }

    #[test]
    fn test_param_i64_opt_none() {
        let input = ToolInput::new("test");
        let result = input.param_i64_opt("missing");
        assert_eq!(result, None);
    }

    #[test]
    fn test_param_bool_opt_some() {
        let input = ToolInput::new("test").with_param("flag", json!(false));

        let result = input.param_bool_opt("flag");
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_param_bool_opt_none() {
        let input = ToolInput::new("test");
        let result = input.param_bool_opt("missing");
        assert_eq!(result, None);
    }

    #[test]
    fn test_param_json_success() {
        let input = ToolInput::new("test").with_param("data", json!({"nested": "value"}));

        let result = input.param_json("data");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), &json!({"nested": "value"}));
    }

    #[test]
    fn test_param_json_missing() {
        let input = ToolInput::new("test");
        let result = input.param_json("missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_input_serialization() {
        let input = ToolInput::new("test_method").with_param("key", json!("value"));

        let serialized = serde_json::to_string(&input).unwrap();
        let deserialized: ToolInput = serde_json::from_str(&serialized).unwrap();

        assert_eq!(input.method, deserialized.method);
        assert_eq!(input.params, deserialized.params);
    }

    #[test]
    fn test_tool_output_text() {
        let output = ToolOutput::text("Hello, World!");
        assert!(output.success);
        assert_eq!(output.data, json!({"text": "Hello, World!"}));
        assert!(output.error.is_none());
    }

    #[test]
    fn test_tool_output_json() {
        let data = json!({"result": "success", "count": 42});
        let output = ToolOutput::json(data.clone());
        assert!(output.success);
        assert_eq!(output.data, data);
        assert!(output.error.is_none());
    }

    #[test]
    fn test_tool_output_error() {
        let output = ToolOutput::error("Something went wrong");
        assert!(!output.success);
        assert_eq!(output.data, serde_json::Value::Null);
        assert_eq!(output.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_tool_output_empty() {
        let output = ToolOutput::empty();
        assert!(output.success);
        assert_eq!(output.data, serde_json::Value::Null);
        assert!(output.error.is_none());
    }

    #[test]
    fn test_tool_output_to_json() {
        let output = ToolOutput::text("test");
        let json_str = output.to_json();
        assert!(json_str.contains("\"success\":true"));
        assert!(json_str.contains("\"text\":\"test\""));
    }

    #[test]
    fn test_tool_output_serialization() {
        let output = ToolOutput::json(json!({"key": "value"}));
        let serialized = serde_json::to_string(&output).unwrap();
        let deserialized: ToolOutput = serde_json::from_str(&serialized).unwrap();

        assert_eq!(output.success, deserialized.success);
        assert_eq!(output.data, deserialized.data);
        assert_eq!(output.error, deserialized.error);
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::MissingParameter("test_param".to_string());
        assert_eq!(err.to_string(), "Missing parameter: test_param");

        let err = ToolError::InvalidParameter("bad_value".to_string());
        assert_eq!(err.to_string(), "Invalid parameter: bad_value");

        let err = ToolError::UnknownMethod("unknown".to_string());
        assert_eq!(err.to_string(), "Unknown method: unknown");
    }
}
