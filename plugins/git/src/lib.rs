//! Git Plugin
//!
//! WASM plugin for git operations.
//! Delegates actual execution to the Rove host daemon.

use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GitOutput {
    success: bool,
    output: String,
}

#[derive(Deserialize)]
struct GitCommitInput {
    message: String,
}

/// Get git status
#[plugin_fn]
pub fn git_status(_input: String) -> FnResult<String> {
    let output = unsafe { host::exec_git("status --short")? };
    let result = GitOutput {
        success: true,
        output,
    };
    Ok(serde_json::to_string(&result)?)
}

/// Get git log
#[plugin_fn]
pub fn git_log(_input: String) -> FnResult<String> {
    let output = unsafe { host::exec_git("log -n 5 --oneline")? };
    let result = GitOutput {
        success: true,
        output,
    };
    Ok(serde_json::to_string(&result)?)
}

/// Git commit
#[plugin_fn]
pub fn git_commit(input: String) -> FnResult<String> {
    let input: GitCommitInput = serde_json::from_str(&input)?;

    // First add all
    unsafe { host::exec_git("add -A")? };

    // Then commit with message
    let cmd = format!("commit -m \"{}\"", input.message.replace('"', "\\\""));
    let output = unsafe { host::exec_git(&cmd)? };

    let result = GitOutput {
        success: true,
        output,
    };
    Ok(serde_json::to_string(&result)?)
}

/// Git push
#[plugin_fn]
pub fn git_push(_input: String) -> FnResult<String> {
    let output = unsafe { host::exec_git("push")? };
    let result = GitOutput {
        success: true,
        output,
    };
    Ok(serde_json::to_string(&result)?)
}

mod host {
    use extism_pdk::*;

    #[host_fn]
    extern "ExtismHost" {
        /// Execute a git command via the host
        pub fn exec_git(args: &str) -> String;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_output_serialization() {
        let output = GitOutput {
            success: true,
            output: "M src/main.rs".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("M src/main.rs"));
    }

    #[test]
    fn test_git_commit_input_deserialization() {
        let json = r#"{"message": "fix: resolve build error"}"#;
        let input: GitCommitInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.message, "fix: resolve build error");
    }

    #[test]
    fn test_git_commit_input_with_quotes() {
        let json = r#"{"message": "feat: add \"new\" feature"}"#;
        let input: GitCommitInput = serde_json::from_str(json).unwrap();
        assert!(input.message.contains("new"));
    }

    #[test]
    fn test_git_output_failure() {
        let output = GitOutput {
            success: false,
            output: "fatal: not a git repository".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("not a git repository"));
    }

    #[test]
    fn test_invalid_commit_input() {
        let result = serde_json::from_str::<GitCommitInput>(r#"{}"#);
        assert!(result.is_err()); // missing message field
    }

    #[test]
    fn test_empty_commit_message() {
        let json = r#"{"message": ""}"#;
        let input: GitCommitInput = serde_json::from_str(json).unwrap();
        assert!(input.message.is_empty());
    }
}
