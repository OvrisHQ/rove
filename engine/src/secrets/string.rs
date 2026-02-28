use std::fmt;
use zeroize::Zeroize;

/// A wrapper for sensitive string data that prevents accidental logging
/// and zeroes memory on drop.
///
/// It implements `Debug` and `Display` to always print `[REDACTED]`.
/// To access the actual secret value, use the `unsecure()` method.
///
/// The inner string is zeroed from heap memory when this value is dropped,
/// preventing secrets from lingering in freed memory.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// Create a new SecretString
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Access the raw underlying string
    pub fn unsecure(&self) -> &str {
        &self.0
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretString([REDACTED])")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SecretString {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_string_redacted_debug() {
        let s = SecretString::new("my-secret-key");
        assert_eq!(format!("{:?}", s), "SecretString([REDACTED])");
    }

    #[test]
    fn test_secret_string_redacted_display() {
        let s = SecretString::new("my-secret-key");
        assert_eq!(format!("{}", s), "[REDACTED]");
    }

    #[test]
    fn test_secret_string_unsecure() {
        let s = SecretString::new("my-secret-key");
        assert_eq!(s.unsecure(), "my-secret-key");
    }

    #[test]
    fn test_secret_string_from_string() {
        let s: SecretString = "hello".to_string().into();
        assert_eq!(s.unsecure(), "hello");
    }

    #[test]
    fn test_secret_string_from_str() {
        let s: SecretString = "hello".into();
        assert_eq!(s.unsecure(), "hello");
    }
}
