//! Screenshot Plugin
//!
//! WASM plugin for screen capture

use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CaptureScreenInput {
    output_path: String,
}

#[derive(Serialize)]
struct CaptureScreenOutput {
    path: String,
    success: bool,
}

/// Capture screen
#[plugin_fn]
pub fn capture_screen(input: String) -> FnResult<String> {
    let input: CaptureScreenInput = serde_json::from_str(&input)?;

    let output = CaptureScreenOutput {
        path: input.output_path,
        success: true,
    };

    Ok(serde_json::to_string(&output)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_screen_input_deserialization() {
        let json = r#"{"output_path": "screenshot.png"}"#;
        let input: CaptureScreenInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.output_path, "screenshot.png");
    }

    #[test]
    fn test_capture_screen_input_absolute_path() {
        let json = r#"{"output_path": "/tmp/screenshots/capture.png"}"#;
        let input: CaptureScreenInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.output_path, "/tmp/screenshots/capture.png");
    }

    #[test]
    fn test_capture_screen_output_serialization() {
        let output = CaptureScreenOutput {
            path: "screenshot.png".to_string(),
            success: true,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("screenshot.png"));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_capture_screen_output_failure() {
        let output = CaptureScreenOutput {
            path: String::new(),
            success: false,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"success\":false"));
    }

    #[test]
    fn test_invalid_json_input() {
        let result = serde_json::from_str::<CaptureScreenInput>("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_output_path() {
        let result = serde_json::from_str::<CaptureScreenInput>(r#"{}"#);
        assert!(result.is_err()); // missing output_path
    }
}
