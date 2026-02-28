//! Read-only File System Plugin
//!
//! This plugin provides read-only access to the file system for log files.
//! It follows the Rove plugin architecture with proper input/output types.
//!
//! Tools provided:
//! - read_file: Read complete file contents
//! - list_directory: List files in a directory
//! - find_system_logs: Find common system log files
//!
//! Note: Currently uses mock data as host functions are not yet fully implemented.
//! In production, this would call host functions for actual file system access.

use extism_pdk::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// Input/Output Types
// ============================================================================

#[derive(Deserialize)]
struct ReadFileInput {
    path: String,
}

#[derive(Serialize)]
struct ReadFileOutput {
    content: String,
    size: usize,
    path: String,
}

#[derive(Deserialize)]
struct ListDirectoryInput {
    path: String,
}

#[derive(Serialize)]
struct DirEntry {
    name: String,
    is_dir: bool,
    size: Option<u64>,
}

#[derive(Serialize)]
struct ListDirectoryOutput {
    path: String,
    entries: Vec<DirEntry>,
    count: usize,
}

#[derive(Deserialize)]
struct FindSystemLogsInput {
    #[serde(default)]
    filter: Option<String>,
}

#[derive(Serialize)]
struct LogFileInfo {
    path: String,
    name: String,
    size: u64,
    description: String,
}

#[derive(Serialize)]
struct FindSystemLogsOutput {
    logs: Vec<LogFileInfo>,
    count: usize,
}

// ============================================================================
// Plugin Functions
// ============================================================================

/// Read the complete contents of a file
///
/// Use this when you need to examine an existing file. The file path must be
/// absolute. This is a read-only operation (Tier 0).
///
/// Input: JSON with "path" field
/// Output: JSON with "content", "size", and "path" fields
///
/// Example input:
/// ```json
/// { "path": "/var/log/system.log" }
/// ```
#[plugin_fn]
pub fn read_file(Json(input): Json<ReadFileInput>) -> FnResult<Json<ReadFileOutput>> {
    // Validate input
    if input.path.is_empty() {
        return Err(WithReturnCode::new(Error::msg("Path cannot be empty"), 1));
    }

    // Mock implementation - in production would call host function
    // For demonstration, return mock content based on path
    let content = if input.path.contains("system.log") {
        generate_mock_system_log()
    } else if input.path.contains("error.log") {
        generate_mock_error_log()
    } else if input.path.contains("access.log") {
        generate_mock_access_log()
    } else {
        format!("Mock content of file: {}\n\nThis is a demonstration plugin.\nIn production, this would read actual file contents via host functions.", input.path)
    };

    let output = ReadFileOutput {
        size: content.len(),
        path: input.path.clone(),
        content,
    };

    Ok(Json(output))
}

/// List the contents of a directory
///
/// Returns a list of files and subdirectories in the specified path.
/// Use this to explore directory structure before reading specific files.
///
/// Input: JSON with "path" field
/// Output: JSON with "path", "entries" array, and "count"
///
/// Example input:
/// ```json
/// { "path": "/var/log" }
/// ```
#[plugin_fn]
pub fn list_directory(
    Json(input): Json<ListDirectoryInput>,
) -> FnResult<Json<ListDirectoryOutput>> {
    // Validate input
    if input.path.is_empty() {
        return Err(WithReturnCode::new(Error::msg("Path cannot be empty"), 1));
    }

    // Mock implementation - return common log directory structure
    let entries = if input.path.contains("/var/log") || input.path.contains("log") {
        vec![
            DirEntry {
                name: "system.log".to_string(),
                is_dir: false,
                size: Some(15234),
            },
            DirEntry {
                name: "error.log".to_string(),
                is_dir: false,
                size: Some(8192),
            },
            DirEntry {
                name: "access.log".to_string(),
                is_dir: false,
                size: Some(45678),
            },
            DirEntry {
                name: "debug.log".to_string(),
                is_dir: false,
                size: Some(2048),
            },
            DirEntry {
                name: "archive".to_string(),
                is_dir: true,
                size: None,
            },
        ]
    } else {
        vec![
            DirEntry {
                name: "file1.txt".to_string(),
                is_dir: false,
                size: Some(1024),
            },
            DirEntry {
                name: "file2.txt".to_string(),
                is_dir: false,
                size: Some(2048),
            },
        ]
    };

    let count = entries.len();
    let output = ListDirectoryOutput {
        path: input.path.clone(),
        entries,
        count,
    };

    Ok(Json(output))
}

/// Find common system log files
///
/// Searches for and returns information about common system log files.
/// This is useful for quickly locating logs without knowing exact paths.
/// Optionally filter by log type (e.g., "error", "system", "access").
///
/// Input: JSON with optional "filter" field
/// Output: JSON with "logs" array and "count"
///
/// Example input:
/// ```json
/// { "filter": "error" }
/// ```
#[plugin_fn]
pub fn find_system_logs(
    Json(input): Json<FindSystemLogsInput>,
) -> FnResult<Json<FindSystemLogsOutput>> {
    // Mock implementation - return common system log locations
    let mut logs = vec![
        LogFileInfo {
            path: "/var/log/system.log".to_string(),
            name: "system.log".to_string(),
            size: 15234,
            description: "Main system log file".to_string(),
        },
        LogFileInfo {
            path: "/var/log/error.log".to_string(),
            name: "error.log".to_string(),
            size: 8192,
            description: "System error log".to_string(),
        },
        LogFileInfo {
            path: "/var/log/access.log".to_string(),
            name: "access.log".to_string(),
            size: 45678,
            description: "Access log for services".to_string(),
        },
        LogFileInfo {
            path: "/var/log/auth.log".to_string(),
            name: "auth.log".to_string(),
            size: 12345,
            description: "Authentication and authorization log".to_string(),
        },
        LogFileInfo {
            path: "/var/log/kern.log".to_string(),
            name: "kern.log".to_string(),
            size: 23456,
            description: "Kernel log messages".to_string(),
        },
    ];

    // Apply filter if provided
    if let Some(filter) = input.filter {
        let filter_lower = filter.to_lowercase();
        logs.retain(|log| {
            log.name.to_lowercase().contains(&filter_lower)
                || log.description.to_lowercase().contains(&filter_lower)
        });
    }

    let count = logs.len();
    let output = FindSystemLogsOutput { logs, count };

    Ok(Json(output))
}

// ============================================================================
// Mock Data Generators
// ============================================================================

fn generate_mock_system_log() -> String {
    r#"2026-02-24 10:30:15 [INFO] System startup complete
2026-02-24 10:30:16 [INFO] Loading configuration from /etc/config
2026-02-24 10:30:17 [INFO] Network interface eth0 initialized
2026-02-24 10:30:18 [INFO] Service daemon started successfully
2026-02-24 10:31:22 [INFO] User session started for user: admin
2026-02-24 10:32:45 [INFO] Database connection established
2026-02-24 10:33:10 [WARN] High memory usage detected: 85%
2026-02-24 10:35:00 [INFO] Scheduled backup completed successfully
2026-02-24 10:36:30 [INFO] Rove agent task started
2026-02-24 10:36:55 [INFO] Rove agent task completed
"#
    .to_string()
}

fn generate_mock_error_log() -> String {
    r#"2026-02-24 09:15:23 [ERROR] Failed to connect to database: connection timeout
2026-02-24 09:15:24 [ERROR] Retry attempt 1/3 failed
2026-02-24 09:15:25 [ERROR] Retry attempt 2/3 failed
2026-02-24 09:15:26 [INFO] Retry attempt 3/3 succeeded
2026-02-24 10:22:10 [ERROR] File not found: /tmp/missing_file.txt
2026-02-24 10:33:10 [WARN] Memory usage above threshold: 85% (threshold: 80%)
2026-02-24 11:05:45 [ERROR] Permission denied accessing /root/secure_data
"#
    .to_string()
}

fn generate_mock_access_log() -> String {
    r#"192.168.1.100 - - [24/Feb/2026:10:30:15 +0000] "GET /api/status HTTP/1.1" 200 1234
192.168.1.101 - - [24/Feb/2026:10:30:16 +0000] "POST /api/tasks HTTP/1.1" 201 567
192.168.1.100 - - [24/Feb/2026:10:30:17 +0000] "GET /api/tasks/123 HTTP/1.1" 200 2345
192.168.1.102 - - [24/Feb/2026:10:30:18 +0000] "GET /api/health HTTP/1.1" 200 89
192.168.1.100 - - [24/Feb/2026:10:31:22 +0000] "DELETE /api/tasks/456 HTTP/1.1" 204 0
192.168.1.103 - - [24/Feb/2026:10:32:45 +0000] "GET /api/logs HTTP/1.1" 200 15678
"#
    .to_string()
}
