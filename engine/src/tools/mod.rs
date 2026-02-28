pub mod filesystem;
pub mod terminal;
pub mod vision;

pub use filesystem::FilesystemTool;
pub use terminal::TerminalTool;
pub use vision::VisionTool;

use tracing::{debug, warn};

/// Registry of available tools that can be dispatched by the agent.
///
/// Holds optional references to each core tool. Only tools that are `Some`
/// will be advertised in the system prompt and available for dispatch.
pub struct ToolRegistry {
    pub fs: Option<FilesystemTool>,
    pub terminal: Option<TerminalTool>,
    pub vision: Option<VisionTool>,
}

impl ToolRegistry {
    /// Create an empty registry with no tools enabled.
    pub fn empty() -> Self {
        Self {
            fs: None,
            terminal: None,
            vision: None,
        }
    }

    /// Dispatch a tool call by name, parsing arguments from JSON.
    ///
    /// Returns the tool output as a string. Errors are returned as `Ok(error_string)`
    /// so the LLM can see the error and self-correct.
    pub async fn dispatch(&self, name: &str, arguments_json: &str) -> String {
        debug!("Dispatching tool '{}' with args: {}", name, arguments_json);

        let args: serde_json::Value = match serde_json::from_str(arguments_json) {
            Ok(v) => v,
            Err(e) => {
                return format!("ERROR: Failed to parse arguments JSON: {}", e);
            }
        };

        match name {
            "read_file" => {
                let Some(ref fs) = self.fs else {
                    return "ERROR: read_file tool is not enabled".to_string();
                };
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                match fs.read_file(path).await {
                    Ok(content) => content,
                    Err(e) => format!("ERROR: {}", e),
                }
            }
            "write_file" => {
                let Some(ref fs) = self.fs else {
                    return "ERROR: write_file tool is not enabled".to_string();
                };
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                match fs.write_file(path, content).await {
                    Ok(msg) => msg,
                    Err(e) => format!("ERROR: {}", e),
                }
            }
            "list_dir" => {
                let Some(ref fs) = self.fs else {
                    return "ERROR: list_dir tool is not enabled".to_string();
                };
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                match fs.list_dir(path).await {
                    Ok(listing) => listing,
                    Err(e) => format!("ERROR: {}", e),
                }
            }
            "file_exists" => {
                let Some(ref fs) = self.fs else {
                    return "ERROR: file_exists tool is not enabled".to_string();
                };
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                match fs.file_exists(path).await {
                    Ok(exists) => {
                        if exists {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        }
                    }
                    Err(e) => format!("ERROR: {}", e),
                }
            }
            "run_command" => {
                let Some(ref terminal) = self.terminal else {
                    return "ERROR: run_command tool is not enabled".to_string();
                };
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                match terminal.execute(command).await {
                    Ok(output) => output,
                    Err(e) => format!("ERROR: {}", e),
                }
            }
            "capture_screen" => {
                let Some(ref vision) = self.vision else {
                    return "ERROR: capture_screen tool is not enabled".to_string();
                };
                let output_file = args
                    .get("output_file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("screenshot.png");
                match vision.capture_screen(output_file).await {
                    Ok(path) => format!("Screenshot saved to {}", path.display()),
                    Err(e) => format!("ERROR: {}", e),
                }
            }
            _ => {
                warn!("Unknown tool requested: {}", name);
                format!(
                    "ERROR: Unknown tool '{}'. Available tools: {}",
                    name,
                    self.available_tool_names().join(", ")
                )
            }
        }
    }

    /// Generate a system prompt describing the available tools.
    ///
    /// Only tools that are `Some` are included.
    pub fn system_prompt(&self) -> String {
        let mut parts = vec![
            "You are Rove, an AI agent that can use tools to accomplish tasks.".to_string(),
            String::new(),
            "IMPORTANT RULES:".to_string(),
            "1. To call a tool, your ENTIRE response must be ONLY the JSON object — nothing else. No explanation, no markdown fences, no text before or after.".to_string(),
            "2. When you have the final answer (after receiving tool results), respond with plain text only — no JSON.".to_string(),
            "3. Never guess or hallucinate tool output. Always call the tool and wait for the real result.".to_string(),
            String::new(),
            "Tool call format (your entire response must be exactly this):".to_string(),
            r#"{"function": "tool_name", "arguments": {"arg1": "value1"}}"#.to_string(),
            String::new(),
            "Available tools:".to_string(),
        ];

        if self.fs.is_some() {
            parts.push(String::new());
            parts.push("## read_file".to_string());
            parts.push("Read the contents of a file.".to_string());
            parts.push(r#"Arguments: {"path": "relative/or/absolute/path"}"#.to_string());

            parts.push(String::new());
            parts.push("## write_file".to_string());
            parts.push(
                "Write content to a file (creates parent directories if needed).".to_string(),
            );
            parts.push(
                r#"Arguments: {"path": "file/path", "content": "file contents"}"#.to_string(),
            );

            parts.push(String::new());
            parts.push("## list_dir".to_string());
            parts.push(
                "List files and directories at a path. Returns entries with type, size, and name."
                    .to_string(),
            );
            parts.push(r#"Arguments: {"path": "directory/path"}"#.to_string());

            parts.push(String::new());
            parts.push("## file_exists".to_string());
            parts.push(
                r#"Check if a file or directory exists. Returns "true" or "false"."#.to_string(),
            );
            parts.push(r#"Arguments: {"path": "file/path"}"#.to_string());
        }

        if self.terminal.is_some() {
            parts.push(String::new());
            parts.push("## run_command".to_string());
            parts.push("Execute a shell command and return its output.".to_string());
            parts.push(r#"Arguments: {"command": "shell command to run"}"#.to_string());
        }

        if self.vision.is_some() {
            parts.push(String::new());
            parts.push("## capture_screen".to_string());
            parts.push("Capture a screenshot and save it to a file.".to_string());
            parts.push(r#"Arguments: {"output_file": "screenshot.png"}"#.to_string());
        }

        parts.join("\n")
    }

    /// Return the names of all currently enabled tools.
    fn available_tool_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.fs.is_some() {
            names.extend_from_slice(&["read_file", "write_file", "list_dir", "file_exists"]);
        }
        if self.terminal.is_some() {
            names.push("run_command");
        }
        if self.vision.is_some() {
            names.push("capture_screen");
        }
        names
    }
}
