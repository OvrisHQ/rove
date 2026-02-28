use std::fmt;

/// A wrapper for sensitive string data that prevents accidental logging.
///
/// It implements `Debug` and `Display` to always print `[REDACTED]`.
/// To access the actual secret value, use the `unsecure()` method.
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
