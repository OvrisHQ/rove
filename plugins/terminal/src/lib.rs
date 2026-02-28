//! Terminal Plugin
//!
//! WASM plugin for command execution

use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[allow(dead_code)]
struct ExecuteCommandInput {
    command: String,
    args: Vec<String>,
}

#[derive(Serialize)]
struct ExecuteCommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

/// Execute a command
#[plugin_fn]
pub fn execute_command(input: String) -> FnResult<String> {
    let _input: ExecuteCommandInput = serde_json::from_str(&input)?;

    // Call host function (to be implemented)
    let output = ExecuteCommandOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
    };

    Ok(serde_json::to_string(&output)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_command_input_deserialization() {
        let json = r#"{"command": "ls", "args": ["-la", "/tmp"]}"#;
        let input: ExecuteCommandInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.command, "ls");
        assert_eq!(input.args, vec!["-la", "/tmp"]);
    }

    #[test]
    fn test_execute_command_input_empty_args() {
        let json = r#"{"command": "pwd", "args": []}"#;
        let input: ExecuteCommandInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.command, "pwd");
        assert!(input.args.is_empty());
    }

    #[test]
    fn test_execute_command_output_serialization() {
        let output = ExecuteCommandOutput {
            stdout: "hello world\n".to_string(),
            stderr: String::new(),
            exit_code: 0,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("hello world"));
        assert!(json.contains("\"exit_code\":0"));
    }

    #[test]
    fn test_execute_command_output_with_error() {
        let output = ExecuteCommandOutput {
            stdout: String::new(),
            stderr: "command not found".to_string(),
            exit_code: 127,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("command not found"));
        assert!(json.contains("\"exit_code\":127"));
    }

    #[test]
    fn test_invalid_json_input() {
        let result = serde_json::from_str::<ExecuteCommandInput>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_required_fields() {
        let result = serde_json::from_str::<ExecuteCommandInput>(r#"{"command": "ls"}"#);
        assert!(result.is_err()); // missing args field
    }
}
