use crate::secrets::string::SecretString;
use crate::secrets::SecretManager;
use sdk::errors::EngineError;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// An in-memory cache for secrets retrieved from the OS keychain.
///
/// This avoids hitting the OS keychain repeatedly during operations.
/// It works in tandem with `SecretManager`.
#[derive(Clone)]
pub struct SecretCache {
    manager: Arc<SecretManager>,
    cache: Arc<RwLock<HashMap<String, SecretString>>>,
}

impl SecretCache {
    /// Creates a new SecretCache wrapping the provided SecretManager
    pub fn new(manager: Arc<SecretManager>) -> Self {
        Self {
            manager,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Retrieves a secret. It checks the memory cache first.
    /// If not found, it asks the SecretManager (which may prompt the user),
    /// caches the result, and returns it.
    pub fn get_secret(&self, key: &str) -> Result<SecretString, EngineError> {
        // Read lock
        {
            let cache = self.cache.read().expect("SecretCache lock poisoned");
            if let Some(secret) = cache.get(key) {
                return Ok(secret.clone());
            }
        }

        // Cache miss, hit the SecretManager
        let raw_secret = self.manager.get_secret(key)?;
        let secret = SecretString::new(raw_secret);

        // Write lock
        {
            let mut cache = self.cache.write().expect("SecretCache lock poisoned");
            cache.insert(key.to_string(), secret.clone());
        }

        Ok(secret)
    }

    /// Pre-loads a set of keys. This ensures any interactive prompts happen early.
    pub fn preload(&self, keys: &[&str]) -> Result<(), EngineError> {
        for key in keys {
            self.get_secret(key)?;
        }
        Ok(())
    }
}
