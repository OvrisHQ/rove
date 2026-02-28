//! File System Editor Plugin
//!
//! WASM plugin for file system operations using Extism PDK.
//!
//! This plugin provides three main functions:
//! - `read_file`: Read the contents of a file
//! - `write_file`: Write content to a file
//! - `list_dir`: List the contents of a directory
//!
//! All file operations are performed through host functions provided by the
//! WasmRuntime, which ensures that FileSystemGuard security checks are applied.
//!
//! # Requirements
//!
//! - Requirement 19.4: Use host functions for file access

use extism_pdk::*;
use serde::{Deserialize, Serialize};

/// Input for read_file function
#[derive(Deserialize)]
struct ReadFileInput {
    path: String,
}

/// Output for read_file function
#[derive(Serialize)]
struct ReadFileOutput {
    content: String,
}

/// Input for write_file function
#[derive(Deserialize)]
struct WriteFileInput {
    path: String,
    content: String,
}

/// Output for write_file function
#[derive(Serialize)]
struct WriteFileOutput {
    success: bool,
    message: String,
}

/// Input for list_dir function
#[derive(Deserialize)]
struct ListDirInput {
    path: String,
}

/// Output for list_dir function
#[derive(Serialize)]
struct ListDirOutput {
    entries: Vec<String>,
}

/// Read a file from the file system
///
/// This function reads the contents of a file at the specified path.
/// The actual file reading is performed by the host function, which
/// applies FileSystemGuard security checks.
///
/// # Input JSON Format
///
/// ```json
/// {
///   "path": "path/to/file.txt"
/// }
/// ```
///
/// # Output JSON Format
///
/// ```json
/// {
///   "content": "file contents here"
/// }
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The input JSON is malformed
/// - The path is denied by FileSystemGuard
/// - The path is outside the workspace
/// - The file does not exist or cannot be read
#[plugin_fn]
pub fn read_file(input: String) -> FnResult<String> {
    let input: ReadFileInput = serde_json::from_str(&input)?;

    // Call host function for file access
    // The host function will apply all security checks via FileSystemGuard
    // SAFETY: Host function is provided by the trusted WasmRuntime
    let content = unsafe { host::read_file(&input.path)? };

    let output = ReadFileOutput { content };
    Ok(serde_json::to_string(&output)?)
}

/// Write a file to the file system
///
/// This function writes content to a file at the specified path.
/// The actual file writing is performed by the host function, which
/// applies FileSystemGuard security checks.
///
/// # Input JSON Format
///
/// ```json
/// {
///   "path": "path/to/file.txt",
///   "content": "content to write"
/// }
/// ```
///
/// # Output JSON Format
///
/// ```json
/// {
///   "success": true,
///   "message": "File written successfully"
/// }
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The input JSON is malformed
/// - The path is denied by FileSystemGuard
/// - The path is outside the workspace
/// - The file cannot be written
#[plugin_fn]
pub fn write_file(input: String) -> FnResult<String> {
    let input: WriteFileInput = serde_json::from_str(&input)?;

    // Call host function for file writing
    // The host function will apply all security checks via FileSystemGuard
    // SAFETY: Host function is provided by the trusted WasmRuntime
    unsafe { host::write_file(&input.path, &input.content)? };

    let output = WriteFileOutput {
        success: true,
        message: "File written successfully".to_string(),
    };
    Ok(serde_json::to_string(&output)?)
}

/// List directory contents
///
/// This function lists all entries in a directory at the specified path.
/// The actual directory listing is performed by the host function, which
/// applies FileSystemGuard security checks.
///
/// # Input JSON Format
///
/// ```json
/// {
///   "path": "path/to/directory"
/// }
/// ```
///
/// # Output JSON Format
///
/// ```json
/// {
///   "entries": ["file1.txt", "file2.txt", "subdir/"]
/// }
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The input JSON is malformed
/// - The path is denied by FileSystemGuard
/// - The path is outside the workspace
/// - The directory does not exist or cannot be read
#[plugin_fn]
pub fn list_dir(input: String) -> FnResult<String> {
    let input: ListDirInput = serde_json::from_str(&input)?;

    // Call host function for directory listing
    // The host function will apply all security checks via FileSystemGuard
    // SAFETY: Host function is provided by the trusted WasmRuntime
    let entries_json = unsafe { host::list_directory(&input.path)? };

    // Parse the JSON array returned by the host
    let entries: Vec<String> = serde_json::from_str(&entries_json)?;

    let output = ListDirOutput { entries };
    Ok(serde_json::to_string(&output)?)
}

/// Host functions provided by the WasmRuntime
///
/// These functions are implemented by the host (Rove engine) and provide
/// controlled access to file system operations. All operations go through
/// the FileSystemGuard for security validation.
///
/// # Security
///
/// The host functions enforce:
/// - Path canonicalization to prevent traversal attacks
/// - Deny list checking for sensitive paths
/// - Workspace boundary enforcement
/// - Plugin permission validation from manifest
mod host {
    use extism_pdk::*;

    #[host_fn]
    extern "ExtismHost" {
        pub fn read_file(path: &str) -> String;
        pub fn write_file(path: &str, content: &str);
        pub fn list_directory(path: &str) -> String;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_input_deserialization() {
        let json = r#"{"path": "test.txt"}"#;
        let input: ReadFileInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.path, "test.txt");
    }

    #[test]
    fn test_read_file_input_empty_path() {
        let json = r#"{"path": ""}"#;
        let input: ReadFileInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.path, "");
    }

    #[test]
    fn test_write_file_input_deserialization() {
        let json = r#"{"path": "out.txt", "content": "hello world"}"#;
        let input: WriteFileInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.path, "out.txt");
        assert_eq!(input.content, "hello world");
    }

    #[test]
    fn test_write_file_output_serialization() {
        let output = WriteFileOutput {
            success: true,
            message: "File written successfully".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("File written successfully"));
    }

    #[test]
    fn test_read_file_output_serialization() {
        let output = ReadFileOutput {
            content: "file contents".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("file contents"));
    }

    #[test]
    fn test_list_dir_input_deserialization() {
        let json = r#"{"path": "/tmp"}"#;
        let input: ListDirInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.path, "/tmp");
    }

    #[test]
    fn test_list_dir_output_serialization() {
        let output = ListDirOutput {
            entries: vec!["a.txt".to_string(), "b/".to_string()],
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("a.txt"));
        assert!(json.contains("b/"));
    }

    #[test]
    fn test_invalid_json_input() {
        let result = serde_json::from_str::<ReadFileInput>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_required_field() {
        let result = serde_json::from_str::<WriteFileInput>(r#"{"path": "x"}"#);
        assert!(result.is_err()); // missing content field
    }
}
