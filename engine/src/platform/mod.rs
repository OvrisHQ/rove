//! Platform-specific utilities
//!
//! This module provides utilities for handling platform-specific differences
//! in file paths, line endings, and other OS-specific behaviors.
//!
//! # Path Handling
//!
//! Rust's `std::path::Path` and `PathBuf` automatically handle platform-specific
//! path separators (/ on Unix, \ on Windows). This module provides additional
//! utilities for working with paths in a cross-platform manner.
//!
//! # Line Endings
//!
//! Different operating systems use different line ending conventions:
//! - Unix (Linux, macOS): LF (\n)
//! - Windows: CRLF (\r\n)
//!
//! This module provides utilities for normalizing and converting line endings
//! when reading and writing files.
//!
//! # Requirements
//!
//! - Requirement 25.2: Use platform-specific paths (/ on Unix, \ on Windows)
//! - Requirement 25.5: Handle platform-specific line endings (LF on Unix, CRLF on Windows)

#![allow(unused_imports)] // PathBuf is used in tests

use std::path::{Path, PathBuf};

/// Platform-specific line ending
///
/// On Unix systems (Linux, macOS), this is LF (\n).
/// On Windows, this is CRLF (\r\n).
#[cfg(unix)]
pub const LINE_ENDING: &str = "\n";

#[cfg(windows)]
pub const LINE_ENDING: &str = "\r\n";

/// Normalize line endings in text to the platform-specific format
///
/// This function converts all line endings in the input text to the
/// platform-specific format:
/// - On Unix: converts CRLF to LF
/// - On Windows: converts LF to CRLF
///
/// # Examples
///
/// ```
/// use rove_engine::platform::normalize_line_endings;
///
/// let text = "line1\r\nline2\nline3\r\n";
/// let normalized = normalize_line_endings(text);
///
/// #[cfg(unix)]
/// assert_eq!(normalized, "line1\nline2\nline3\n");
///
/// #[cfg(windows)]
/// assert_eq!(normalized, "line1\r\nline2\r\nline3\r\n");
/// ```
pub fn normalize_line_endings(text: &str) -> String {
    #[cfg(unix)]
    {
        // On Unix, convert CRLF to LF
        text.replace("\r\n", "\n")
    }

    #[cfg(windows)]
    {
        // On Windows, first normalize to LF, then convert to CRLF
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    }
}

/// Convert line endings to Unix format (LF)
///
/// This function converts all line endings to LF (\n), regardless of
/// the current platform. Useful for storing text in a canonical format.
///
/// # Examples
///
/// ```
/// use rove_engine::platform::to_unix_line_endings;
///
/// let text = "line1\r\nline2\nline3\r\n";
/// let unix = to_unix_line_endings(text);
/// assert_eq!(unix, "line1\nline2\nline3\n");
/// ```
pub fn to_unix_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Convert line endings to Windows format (CRLF)
///
/// This function converts all line endings to CRLF (\r\n), regardless of
/// the current platform. Useful for generating Windows-compatible files.
///
/// # Examples
///
/// ```
/// use rove_engine::platform::to_windows_line_endings;
///
/// let text = "line1\nline2\r\nline3\n";
/// let windows = to_windows_line_endings(text);
/// assert_eq!(windows, "line1\r\nline2\r\nline3\r\n");
/// ```
pub fn to_windows_line_endings(text: &str) -> String {
    // First normalize to LF, then convert to CRLF
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Get the platform-specific path separator
///
/// Returns "/" on Unix systems and "\\" on Windows.
/// Note: In most cases, you should use `std::path::Path` and `PathBuf`
/// which handle separators automatically. This function is provided for
/// cases where you need the separator as a string.
///
/// # Examples
///
/// ```
/// use rove_engine::platform::path_separator;
///
/// #[cfg(unix)]
/// assert_eq!(path_separator(), "/");
///
/// #[cfg(windows)]
/// assert_eq!(path_separator(), "\\");
/// ```
pub fn path_separator() -> &'static str {
    std::path::MAIN_SEPARATOR_STR
}

/// Join path components with the platform-specific separator
///
/// Note: In most cases, you should use `PathBuf::join()` instead, which
/// handles this automatically. This function is provided for cases where
/// you need to work with string paths.
///
/// # Examples
///
/// ```
/// use rove_engine::platform::join_path;
///
/// let path = join_path(&["home", "user", "file.txt"]);
///
/// #[cfg(unix)]
/// assert_eq!(path, "home/user/file.txt");
///
/// #[cfg(windows)]
/// assert_eq!(path, "home\\user\\file.txt");
/// ```
pub fn join_path(components: &[&str]) -> String {
    components.join(path_separator())
}

/// Display a path using the platform-specific separator
///
/// This function converts a `Path` to a string representation using
/// the platform-specific separator. Useful for displaying paths to users.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use rove_engine::platform::display_path;
///
/// let path = PathBuf::from("home").join("user").join("file.txt");
/// let display = display_path(&path);
///
/// #[cfg(unix)]
/// assert_eq!(display, "home/user/file.txt");
///
/// #[cfg(windows)]
/// assert_eq!(display, "home\\user\\file.txt");
/// ```
pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

/// Check if the current platform is Unix-like (Linux, macOS, BSD, etc.)
///
/// # Examples
///
/// ```
/// use rove_engine::platform::is_unix;
///
/// #[cfg(unix)]
/// assert!(is_unix());
///
/// #[cfg(windows)]
/// assert!(!is_unix());
/// ```
pub fn is_unix() -> bool {
    cfg!(unix)
}

/// Check if the current platform is Windows
///
/// # Examples
///
/// ```
/// use rove_engine::platform::is_windows;
///
/// #[cfg(windows)]
/// assert!(is_windows());
///
/// #[cfg(unix)]
/// assert!(!is_windows());
/// ```
pub fn is_windows() -> bool {
    cfg!(windows)
}

/// Get the platform name as a string
///
/// Returns one of: "linux", "macos", "windows", "unknown"
///
/// # Examples
///
/// ```
/// use rove_engine::platform::platform_name;
///
/// let name = platform_name();
/// assert!(["linux", "macos", "windows", "unknown"].contains(&name));
/// ```
pub fn platform_name() -> &'static str {
    #[cfg(target_os = "linux")]
    return "linux";

    #[cfg(target_os = "macos")]
    return "macos";

    #[cfg(target_os = "windows")]
    return "windows";

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return "unknown";
}

/// Get the platform-specific shared library extension
///
/// Returns the file extension used for shared libraries on the current platform:
/// - Linux: "so"
/// - macOS: "dylib"
/// - Windows: "dll"
///
/// This is used when loading core tools to ensure the correct library format
/// is loaded for the current platform.
///
/// # Requirements
///
/// - Requirement 25.4: Load Core_Tools with platform-specific extensions
/// - Requirement 25.6: Use #[cfg(target_os)] for platform-specific code
///
/// # Examples
///
/// ```
/// use rove_engine::platform::library_extension;
///
/// let ext = library_extension();
///
/// #[cfg(target_os = "linux")]
/// assert_eq!(ext, "so");
///
/// #[cfg(target_os = "macos")]
/// assert_eq!(ext, "dylib");
///
/// #[cfg(target_os = "windows")]
/// assert_eq!(ext, "dll");
/// ```
pub fn library_extension() -> &'static str {
    #[cfg(target_os = "linux")]
    return "so";

    #[cfg(target_os = "macos")]
    return "dylib";

    #[cfg(target_os = "windows")]
    return "dll";

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return "so"; // Default to .so for unknown platforms
}

/// Get the platform-specific shared library prefix
///
/// Returns the prefix used for shared libraries on the current platform:
/// - Unix (Linux, macOS): "lib"
/// - Windows: "" (no prefix)
///
/// This is used when constructing library file names.
///
/// # Requirements
///
/// - Requirement 25.4: Load Core_Tools with platform-specific extensions
/// - Requirement 25.6: Use #[cfg(target_os)] for platform-specific code
///
/// # Examples
///
/// ```
/// use rove_engine::platform::library_prefix;
///
/// let prefix = library_prefix();
///
/// #[cfg(unix)]
/// assert_eq!(prefix, "lib");
///
/// #[cfg(windows)]
/// assert_eq!(prefix, "");
/// ```
pub fn library_prefix() -> &'static str {
    #[cfg(unix)]
    return "lib";

    #[cfg(windows)]
    return "";
}

/// Construct a platform-specific library filename
///
/// Given a library name, constructs the full filename with the appropriate
/// prefix and extension for the current platform.
///
/// # Arguments
///
/// * `name` - The base name of the library (without prefix or extension)
///
/// # Returns
///
/// The full library filename for the current platform:
/// - Linux: `lib{name}.so`
/// - macOS: `lib{name}.dylib`
/// - Windows: `{name}.dll`
///
/// # Requirements
///
/// - Requirement 25.4: Load Core_Tools with platform-specific extensions
/// - Requirement 25.6: Use #[cfg(target_os)] for platform-specific code
///
/// # Examples
///
/// ```
/// use rove_engine::platform::library_filename;
///
/// let filename = library_filename("telegram");
///
/// #[cfg(target_os = "linux")]
/// assert_eq!(filename, "libtelegram.so");
///
/// #[cfg(target_os = "macos")]
/// assert_eq!(filename, "libtelegram.dylib");
///
/// #[cfg(target_os = "windows")]
/// assert_eq!(filename, "telegram.dll");
/// ```
pub fn library_filename(name: &str) -> String {
    format!("{}{}.{}", library_prefix(), name, library_extension())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_ending_constant() {
        #[cfg(unix)]
        assert_eq!(LINE_ENDING, "\n");

        #[cfg(windows)]
        assert_eq!(LINE_ENDING, "\r\n");
    }

    #[test]
    fn test_normalize_line_endings_mixed() {
        let text = "line1\r\nline2\nline3\r\n";
        let normalized = normalize_line_endings(text);

        #[cfg(unix)]
        assert_eq!(normalized, "line1\nline2\nline3\n");

        #[cfg(windows)]
        assert_eq!(normalized, "line1\r\nline2\r\nline3\r\n");
    }

    #[test]
    fn test_normalize_line_endings_already_normalized() {
        #[cfg(unix)]
        {
            let text = "line1\nline2\nline3\n";
            let normalized = normalize_line_endings(text);
            assert_eq!(normalized, text);
        }

        #[cfg(windows)]
        {
            let text = "line1\r\nline2\r\nline3\r\n";
            let normalized = normalize_line_endings(text);
            assert_eq!(normalized, text);
        }
    }

    #[test]
    fn test_to_unix_line_endings() {
        let text = "line1\r\nline2\nline3\r\n";
        let unix = to_unix_line_endings(text);
        assert_eq!(unix, "line1\nline2\nline3\n");
    }

    #[test]
    fn test_to_windows_line_endings() {
        let text = "line1\nline2\r\nline3\n";
        let windows = to_windows_line_endings(text);
        assert_eq!(windows, "line1\r\nline2\r\nline3\r\n");
    }

    #[test]
    fn test_path_separator() {
        let sep = path_separator();

        #[cfg(unix)]
        assert_eq!(sep, "/");

        #[cfg(windows)]
        assert_eq!(sep, "\\");
    }

    #[test]
    fn test_join_path() {
        let path = join_path(&["home", "user", "file.txt"]);

        #[cfg(unix)]
        assert_eq!(path, "home/user/file.txt");

        #[cfg(windows)]
        assert_eq!(path, "home\\user\\file.txt");
    }

    #[test]
    fn test_display_path() {
        let path = PathBuf::from("home").join("user").join("file.txt");
        let display = display_path(&path);

        // The display should contain the platform-specific separator
        #[cfg(unix)]
        assert!(display.contains('/'));

        #[cfg(windows)]
        assert!(display.contains('\\'));
    }

    #[test]
    fn test_is_unix() {
        #[cfg(unix)]
        assert!(is_unix());

        #[cfg(windows)]
        assert!(!is_unix());
    }

    #[test]
    fn test_is_windows() {
        #[cfg(windows)]
        assert!(is_windows());

        #[cfg(unix)]
        assert!(!is_windows());
    }

    #[test]
    fn test_platform_name() {
        let name = platform_name();
        assert!(["linux", "macos", "windows", "unknown"].contains(&name));

        #[cfg(target_os = "linux")]
        assert_eq!(name, "linux");

        #[cfg(target_os = "macos")]
        assert_eq!(name, "macos");

        #[cfg(target_os = "windows")]
        assert_eq!(name, "windows");
    }

    #[test]
    fn test_library_extension() {
        let ext = library_extension();

        #[cfg(target_os = "linux")]
        assert_eq!(ext, "so");

        #[cfg(target_os = "macos")]
        assert_eq!(ext, "dylib");

        #[cfg(target_os = "windows")]
        assert_eq!(ext, "dll");
    }

    #[test]
    fn test_library_prefix() {
        let prefix = library_prefix();

        #[cfg(unix)]
        assert_eq!(prefix, "lib");

        #[cfg(windows)]
        assert_eq!(prefix, "");
    }

    #[test]
    fn test_library_filename() {
        let filename = library_filename("telegram");

        #[cfg(target_os = "linux")]
        assert_eq!(filename, "libtelegram.so");

        #[cfg(target_os = "macos")]
        assert_eq!(filename, "libtelegram.dylib");

        #[cfg(target_os = "windows")]
        assert_eq!(filename, "telegram.dll");
    }

    #[test]
    fn test_library_filename_with_special_chars() {
        let filename = library_filename("ui-server");

        #[cfg(target_os = "linux")]
        assert_eq!(filename, "libui-server.so");

        #[cfg(target_os = "macos")]
        assert_eq!(filename, "libui-server.dylib");

        #[cfg(target_os = "windows")]
        assert_eq!(filename, "ui-server.dll");
    }

    #[test]
    fn test_round_trip_line_endings() {
        let original = "line1\nline2\nline3\n";

        // Convert to Windows and back to Unix
        let windows = to_windows_line_endings(original);
        let back_to_unix = to_unix_line_endings(&windows);

        assert_eq!(back_to_unix, original);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(normalize_line_endings(""), "");
        assert_eq!(to_unix_line_endings(""), "");
        assert_eq!(to_windows_line_endings(""), "");
    }

    #[test]
    fn test_no_line_endings() {
        let text = "single line with no ending";
        assert_eq!(normalize_line_endings(text), text);
        assert_eq!(to_unix_line_endings(text), text);
        assert_eq!(to_windows_line_endings(text), text);
    }
}
