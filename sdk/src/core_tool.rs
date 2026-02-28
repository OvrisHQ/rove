//! Core tool trait and context types
//!
//! This module defines the CoreTool trait that all native core tools must implement,
//! and the CoreContext that provides limited, controlled access to engine functionality.

use crate::errors::EngineError;
use crate::types::{ToolInput, ToolOutput};
use std::sync::Arc;

/// Trait that all core tools must implement
pub trait CoreTool: Send + Sync {
    /// Returns the name of the tool
    fn name(&self) -> &str;

    /// Returns the version of the tool
    fn version(&self) -> &str;

    /// Called when the tool is loaded, provides CoreContext for engine interaction
    fn start(&mut self, ctx: CoreContext) -> Result<(), EngineError>;

    /// Called when the tool is being unloaded
    fn stop(&mut self) -> Result<(), EngineError>;

    /// Handle a tool invocation
    fn handle(&self, input: ToolInput) -> Result<ToolOutput, EngineError>;
}

/// Context provided to core tools for engine interaction.
///
/// CoreContext is the sole API surface for core tools to interact with the engine.
/// It provides controlled access through handles that expose specific operations
/// without revealing internal engine state.
///
/// **Requirements: 1.4, 1.5, 6.10**
#[derive(Clone)]
pub struct CoreContext {
    /// Handle for agent operations (task submission, status queries)
    pub agent: AgentHandle,

    /// Handle for database access (read-only queries)
    pub db: DbHandle,

    /// Handle for configuration reading
    pub config: ConfigHandle,

    /// Handle for cryptographic operations (signing, verification, secrets)
    pub crypto: CryptoHandle,

    /// Handle for network calls (HTTP requests)
    pub network: NetworkHandle,

    /// Handle for message bus subscriptions and publishing
    pub bus: BusHandle,
}

impl CoreContext {
    /// Create a new CoreContext with all handles
    pub fn new(
        agent: AgentHandle,
        db: DbHandle,
        config: ConfigHandle,
        crypto: CryptoHandle,
        network: NetworkHandle,
        bus: BusHandle,
    ) -> Self {
        Self {
            agent,
            db,
            config,
            crypto,
            network,
            bus,
        }
    }
}

/// Handle for agent operations
///
/// Provides methods to submit tasks and query task status.
#[derive(Clone)]
pub struct AgentHandle {
    inner: Arc<dyn AgentHandleImpl>,
}

impl AgentHandle {
    /// Create a new AgentHandle with the given implementation
    pub fn new(inner: Arc<dyn AgentHandleImpl>) -> Self {
        Self { inner }
    }

    /// Submit a task to the agent for execution
    pub fn submit_task(&self, task_input: String) -> Result<String, EngineError> {
        self.inner.submit_task(task_input)
    }

    /// Get the status of a task by ID
    pub fn get_task_status(&self, task_id: &str) -> Result<String, EngineError> {
        self.inner.get_task_status(task_id)
    }
}

/// Trait for agent handle implementation (to be implemented by engine)
pub trait AgentHandleImpl: Send + Sync {
    /// Submit a task and return task ID
    fn submit_task(&self, task_input: String) -> Result<String, EngineError>;

    /// Get task status by ID
    fn get_task_status(&self, task_id: &str) -> Result<String, EngineError>;
}

/// Handle for database access
///
/// Provides read-only access to the database for querying task history and other data.
#[derive(Clone)]
pub struct DbHandle {
    inner: Arc<dyn DbHandleImpl>,
}

impl DbHandle {
    /// Create a new DbHandle with the given implementation
    pub fn new(inner: Arc<dyn DbHandleImpl>) -> Self {
        Self { inner }
    }

    /// Execute a read-only SQL query
    ///
    /// Only SELECT queries are allowed. Write operations will return an error.
    pub fn query(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, EngineError> {
        self.inner.query(sql, params)
    }
}

/// Trait for database handle implementation (to be implemented by engine)
pub trait DbHandleImpl: Send + Sync {
    /// Execute a read-only query
    fn query(
        &self,
        sql: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, EngineError>;
}

/// Handle for configuration access
///
/// Provides read-only access to configuration values.
#[derive(Clone)]
pub struct ConfigHandle {
    inner: Arc<dyn ConfigHandleImpl>,
}

impl ConfigHandle {
    /// Create a new ConfigHandle with the given implementation
    pub fn new(inner: Arc<dyn ConfigHandleImpl>) -> Self {
        Self { inner }
    }

    /// Get a configuration value by key
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.inner.get(key)
    }

    /// Get a configuration value as a string
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str().map(String::from))
    }

    /// Get a configuration value as an integer
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    /// Get a configuration value as a boolean
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }
}

/// Trait for config handle implementation (to be implemented by engine)
pub trait ConfigHandleImpl: Send + Sync {
    /// Get a configuration value by key
    fn get(&self, key: &str) -> Option<serde_json::Value>;
}

/// Handle for cryptographic operations
///
/// Provides access to signing, verification, and secret management.
#[derive(Clone)]
pub struct CryptoHandle {
    inner: Arc<dyn CryptoHandleImpl>,
}

impl CryptoHandle {
    /// Create a new CryptoHandle with the given implementation
    pub fn new(inner: Arc<dyn CryptoHandleImpl>) -> Self {
        Self { inner }
    }

    /// Sign data with the engine's private key
    pub fn sign_data(&self, data: &[u8]) -> Result<Vec<u8>, EngineError> {
        self.inner.sign_data(data)
    }

    /// Verify a signature on data
    pub fn verify_signature(&self, data: &[u8], signature: &[u8]) -> Result<(), EngineError> {
        self.inner.verify_signature(data, signature)
    }

    /// Get a secret from the OS keychain
    pub fn get_secret(&self, key: &str) -> Result<String, EngineError> {
        self.inner.get_secret(key)
    }

    /// Scrub secrets from text (replace with [REDACTED])
    pub fn scrub_secrets(&self, text: &str) -> String {
        self.inner.scrub_secrets(text)
    }
}

/// Trait for crypto handle implementation (to be implemented by engine)
pub trait CryptoHandleImpl: Send + Sync {
    /// Sign data
    fn sign_data(&self, data: &[u8]) -> Result<Vec<u8>, EngineError>;

    /// Verify signature
    fn verify_signature(&self, data: &[u8], signature: &[u8]) -> Result<(), EngineError>;

    /// Get secret from keychain
    fn get_secret(&self, key: &str) -> Result<String, EngineError>;

    /// Scrub secrets from text
    fn scrub_secrets(&self, text: &str) -> String;
}

/// Handle for network operations
///
/// Provides HTTP client functionality for making network requests.
#[derive(Clone)]
pub struct NetworkHandle {
    inner: Arc<dyn NetworkHandleImpl>,
}

impl NetworkHandle {
    /// Create a new NetworkHandle with the given implementation
    pub fn new(inner: Arc<dyn NetworkHandleImpl>) -> Self {
        Self { inner }
    }

    /// Perform an HTTP GET request
    pub fn http_get(&self, url: &str) -> Result<Vec<u8>, EngineError> {
        self.inner.http_get(url)
    }

    /// Perform an HTTP POST request
    pub fn http_post(&self, url: &str, body: Vec<u8>) -> Result<Vec<u8>, EngineError> {
        self.inner.http_post(url, body)
    }
}

/// Trait for network handle implementation (to be implemented by engine)
pub trait NetworkHandleImpl: Send + Sync {
    /// HTTP GET request
    fn http_get(&self, url: &str) -> Result<Vec<u8>, EngineError>;

    /// HTTP POST request
    fn http_post(&self, url: &str, body: Vec<u8>) -> Result<Vec<u8>, EngineError>;
}

/// Handle for message bus operations
///
/// Provides pub/sub functionality for inter-component communication.
#[derive(Clone)]
pub struct BusHandle {
    inner: Arc<dyn BusHandleImpl>,
}

impl BusHandle {
    /// Create a new BusHandle with the given implementation
    pub fn new(inner: Arc<dyn BusHandleImpl>) -> Self {
        Self { inner }
    }

    /// Subscribe to events of a specific type
    ///
    /// Returns a channel receiver for receiving events.
    pub fn subscribe(&self, event_type: &str) -> Result<(), EngineError> {
        self.inner.subscribe(event_type)
    }

    /// Publish an event to the message bus
    pub fn publish(&self, event_type: &str, payload: serde_json::Value) -> Result<(), EngineError> {
        self.inner.publish(event_type, payload)
    }
}

/// Trait for bus handle implementation (to be implemented by engine)
pub trait BusHandleImpl: Send + Sync {
    /// Subscribe to event type
    fn subscribe(&self, event_type: &str) -> Result<(), EngineError>;

    /// Publish event
    fn publish(&self, event_type: &str, payload: serde_json::Value) -> Result<(), EngineError>;
}
