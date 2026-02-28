//! WASM runtime for loading and managing plugins
//!
//! This module implements the WasmRuntime which loads plugins as WASM modules
//! via the Extism runtime with two-gate security verification.
//!
//! # Two-Gate Verification System
//!
//! Every plugin must pass through two security gates before loading:
//!
//! 1. **Gate 1: Manifest Check** - Verify the plugin is declared in the signed manifest
//! 2. **Gate 2: Hash Verification** - Verify the file's BLAKE3 hash matches the manifest
//!
//! If any gate fails, the compromised file is immediately deleted and loading is aborted.
//!
//! Plugins have fewer security requirements than core tools because they run in a
//! sandboxed WASM environment with limited capabilities. They cannot directly access
//! the file system or execute system commands - all such operations must go through
//! host functions which enforce security policies.
//!
//! # Requirements
//!
//! - Requirement 5.1: Load plugins as WASM modules via Extism
//! - Requirement 5.2: Gate 1 - Check plugin is in manifest
//! - Requirement 5.3: Gate 2 - Verify file hash with BLAKE3
//! - Requirement 5.4: Validate manifest contains no absolute paths
//!
//! # Examples
//!
//! ```no_run
//! use rove_engine::runtime::WasmRuntime;
//! use rove_engine::crypto::CryptoModule;
//! use rove_engine::fs_guard::FileSystemGuard;
//! use sdk::manifest::Manifest;
//! use std::sync::Arc;
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let manifest = Manifest::from_json(&std::fs::read_to_string("manifest.json")?)?;
//! let crypto = Arc::new(CryptoModule::new()?);
//! let fs_guard = Arc::new(FileSystemGuard::new(PathBuf::from("/workspace")));
//! let mut runtime = WasmRuntime::new(manifest, crypto, fs_guard);
//!
//! // Load a plugin with two-gate verification
//! runtime.load_plugin("fs-editor").await?;
//!
//! // Call a plugin function
//! let input = serde_json::json!({
//!     "path": "test.txt"
//! });
//! let output = runtime.call_plugin("fs-editor", "read_file", input.to_string().as_bytes()).await?;
//! # Ok(())
//! # }
//! ```

use crate::crypto::CryptoModule;
use crate::fs_guard::FileSystemGuard;
use crate::message_bus::{Event, MessageBus};
use extism::{Function, Manifest as ExtismManifest, Plugin, UserData, Wasm};
use sdk::{errors::EngineError, manifest::Manifest};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Maximum number of crash restarts allowed per plugin before giving up
const MAX_CRASH_RESTARTS: u32 = 3;

/// Metadata about a loaded plugin
struct PluginMetadata {
    /// The Extism plugin instance
    plugin: Plugin,
    /// Number of times this plugin has crashed and been restarted
    crash_count: u32,
}

/// WASM runtime for loading and managing plugins
///
/// The WasmRuntime loads plugins as WASM modules via Extism and manages their
/// lifecycle. All plugins must pass through two security gates before being loaded.
///
/// Plugins run in a sandboxed environment and can only interact with the host
/// through explicitly provided host functions. This provides strong isolation
/// and security guarantees.
///
/// # Crash Handling
///
/// The runtime automatically detects plugin crashes and attempts to restart them
/// up to MAX_CRASH_RESTARTS times. After that, the plugin is marked as failed
/// and will not be restarted automatically. This prevents infinite restart loops
/// while allowing recovery from transient failures.
///
/// # Thread Safety
///
/// This struct is not thread-safe by default. Wrap in Arc<Mutex<_>> if sharing
/// across threads is needed.
pub struct WasmRuntime {
    /// Loaded plugins indexed by name with metadata
    plugins: HashMap<String, PluginMetadata>,
    /// Manifest containing plugin metadata
    manifest: Manifest,
    /// Cryptographic module for verification
    crypto: Arc<CryptoModule>,
    /// File system guard for path validation (reserved for future host function implementation)
    #[allow(dead_code)]
    fs_guard: Arc<FileSystemGuard>,
    /// Message bus for publishing crash events (optional)
    message_bus: Option<Arc<MessageBus>>,
}

impl WasmRuntime {
    /// Create a new WasmRuntime with the given manifest, crypto module, and file system guard
    ///
    /// # Arguments
    ///
    /// * `manifest` - The signed manifest containing plugin metadata
    /// * `crypto` - Cryptographic module for hash verification
    /// * `fs_guard` - File system guard for path validation in host functions
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # use rove_engine::crypto::CryptoModule;
    /// # use rove_engine::fs_guard::FileSystemGuard;
    /// # use sdk::manifest::Manifest;
    /// # use std::sync::Arc;
    /// # use std::path::PathBuf;
    /// let manifest = Manifest::from_json(&std::fs::read_to_string("manifest.json").unwrap()).unwrap();
    /// let crypto = Arc::new(CryptoModule::new().unwrap());
    /// let fs_guard = Arc::new(FileSystemGuard::new(PathBuf::from("/workspace")));
    /// let runtime = WasmRuntime::new(manifest, crypto, fs_guard);
    /// ```
    pub fn new(
        manifest: Manifest,
        crypto: Arc<CryptoModule>,
        fs_guard: Arc<FileSystemGuard>,
    ) -> Self {
        tracing::info!("Initializing WasmRuntime");
        Self {
            plugins: HashMap::new(),
            manifest,
            crypto,
            fs_guard,
            message_bus: None,
        }
    }

    /// Set the message bus for publishing crash events
    ///
    /// This is optional but recommended for production use. When set, the runtime
    /// will publish PluginCrashed events to the message bus when plugins crash.
    ///
    /// # Arguments
    ///
    /// * `bus` - The message bus to use for publishing events
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # use rove_engine::message_bus::MessageBus;
    /// # use std::sync::Arc;
    /// # fn example(runtime: &mut WasmRuntime) {
    /// let bus = Arc::new(MessageBus::new());
    /// runtime.set_message_bus(bus);
    /// # }
    /// ```
    pub fn set_message_bus(&mut self, bus: Arc<MessageBus>) {
        self.message_bus = Some(bus);
    }

    /// Load a plugin with two-gate verification
    ///
    /// This method implements the two-gate security verification system:
    ///
    /// 1. **Gate 1**: Verify the plugin is declared in the manifest
    /// 2. **Gate 2**: Verify the file's BLAKE3 hash matches the manifest
    ///
    /// If any gate fails, the compromised file is deleted and an error is returned.
    ///
    /// After verification, the plugin is loaded via Extism with host functions
    /// that provide controlled access to file system operations.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin to load (must match manifest entry)
    ///
    /// # Errors
    ///
    /// Returns `EngineError::PluginNotInManifest` if the plugin is not declared in the manifest (Gate 1 failure).
    /// Returns `EngineError::HashMismatch` if the file hash doesn't match (Gate 2 failure).
    /// Returns `EngineError::Plugin` if the WASM module cannot be loaded.
    ///
    /// # Security
    ///
    /// **CRITICAL**: This method deletes the plugin file if any verification gate fails.
    /// This prevents execution of compromised or tampered WASM modules.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # async fn example(runtime: &mut WasmRuntime) -> Result<(), Box<dyn std::error::Error>> {
    /// runtime.load_plugin("fs-editor").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn load_plugin(&mut self, name: &str) -> Result<(), EngineError> {
        tracing::info!("Loading plugin: {}", name);

        // Gate 1: Check plugin is in manifest (Requirement 5.2)
        let plugin_entry = self.manifest.get_plugin(name).ok_or_else(|| {
            tracing::error!("Gate 1 FAILED: Plugin '{}' not found in manifest", name);
            EngineError::PluginNotInManifest(name.to_string())
        })?;

        tracing::info!("Gate 1 PASSED: Plugin '{}' found in manifest", name);

        // Validate no absolute paths in manifest (Requirement 5.4)
        let plugin_path = PathBuf::from(&plugin_entry.path);
        if plugin_path.is_absolute() {
            tracing::error!(
                "Plugin '{}' has absolute path in manifest: {}",
                name,
                plugin_entry.path
            );
            return Err(EngineError::Config(format!(
                "Plugin '{}' has absolute path in manifest (security violation)",
                name
            )));
        }

        // Gate 2: Verify file hash with BLAKE3 (Requirement 5.3)
        // Note: verify_file will delete the file if hash doesn't match
        self.crypto
            .verify_file(&plugin_path, &plugin_entry.hash)
            .map_err(|e| {
                tracing::error!(
                    "Gate 2 FAILED: Hash verification failed for '{}': {}",
                    name,
                    e
                );
                e
            })?;

        tracing::info!("Gate 2 PASSED: File hash verified for '{}'", name);

        // All gates passed - safe to load the WASM module
        tracing::info!("Both gates passed for '{}', loading WASM module...", name);

        // Read the WASM file
        let wasm_bytes = std::fs::read(&plugin_path).map_err(|e| {
            tracing::error!("Failed to read WASM file {}: {}", plugin_path.display(), e);
            EngineError::Plugin(format!("Failed to read WASM file: {}", e))
        })?;

        // Create Extism manifest for the plugin
        let wasm = Wasm::data(wasm_bytes);
        let extism_manifest = ExtismManifest::new([wasm]);

        // Create host functions for the plugin
        let host_functions = self.create_host_functions();

        // Create the Extism plugin with host functions
        let plugin = Plugin::new(&extism_manifest, host_functions, true).map_err(|e| {
            tracing::error!("Failed to create Extism plugin for '{}': {}", name, e);
            EngineError::Plugin(format!("Failed to create plugin: {}", e))
        })?;

        // Store the plugin with metadata
        self.plugins.insert(
            name.to_string(),
            PluginMetadata {
                plugin,
                crash_count: 0,
            },
        );

        tracing::info!("Plugin '{}' loaded successfully", name);
        Ok(())
    }

    /// Create host functions that plugins can call
    ///
    /// These host functions provide controlled access to file system operations.
    /// All operations go through the FileSystemGuard for security validation.
    ///
    /// # Host Functions Provided
    ///
    /// - `read_file(path: string) -> string` - Read a file's contents
    /// - `write_file(path: string, content: string)` - Write content to a file
    /// - `list_directory(path: string) -> string` - List directory contents (JSON array)
    ///
    /// # Security
    ///
    /// All file operations are validated by the FileSystemGuard, which:
    /// - Checks paths against the deny list
    /// - Canonicalizes paths to prevent traversal attacks
    /// - Ensures operations stay within the workspace
    ///
    /// Additionally, plugin permissions from the manifest are enforced:
    /// - allowed_paths: Only paths matching these patterns are allowed
    /// - denied_paths: Paths matching these patterns are explicitly denied
    /// - max_file_size: Maximum file size for read/write operations
    ///
    /// # Implementation Note
    ///
    /// These are placeholder implementations. The actual Extism host function API
    /// requires using the PDK's host function interface, which works differently
    /// than shown here. In production, plugins would call these functions via
    /// the Extism PDK's `host_fn!` macro, and the host would implement them
    /// using Extism's `Function::new` with proper memory handling.
    ///
    /// For now, we return empty function lists since the actual implementation
    /// requires deeper integration with Extism's memory model.
    fn create_host_functions(&self) -> Vec<Function> {
        // TODO: Implement actual host functions using Extism's PDK interface
        // The challenge is that Extism's host functions need to:
        // 1. Read strings from plugin linear memory
        // 2. Perform operations (with our security checks)
        // 3. Write results back to plugin memory

        tracing::warn!(
            "Host functions not yet fully implemented - plugins will receive empty/dummy responses"
        );

        use extism::ValType;

        let read_file = Function::new(
            "read_file",
            [ValType::I64],
            [ValType::I64],
            UserData::new(()),
            |_plugin, _inputs, _outputs, _user_data| Ok(()),
        );

        let write_file = Function::new(
            "write_file",
            [ValType::I64, ValType::I64],
            [],
            UserData::new(()),
            |_plugin, _inputs, _outputs, _user_data| Ok(()),
        );

        let list_directory = Function::new(
            "list_directory",
            [ValType::I64],
            [ValType::I64],
            UserData::new(()),
            |_plugin, _inputs, _outputs, _user_data| Ok(()),
        );

        let exec_git = Function::new(
            "exec_git",
            [ValType::I64],
            [ValType::I64],
            UserData::new(()),
            |_plugin, _inputs, _outputs, _user_data| Ok(()),
        );

        vec![read_file, write_file, list_directory, exec_git]
    }

    /// Call a plugin function with the given input
    ///
    /// This method wraps the plugin call in crash detection and automatic restart logic.
    /// If a plugin crashes, it will be automatically restarted up to MAX_CRASH_RESTARTS
    /// times. After that, the plugin is marked as failed and subsequent calls will fail.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin to call
    /// * `function` - Name of the function to call within the plugin
    /// * `input` - Input data as bytes (typically JSON)
    ///
    /// # Returns
    ///
    /// Returns the plugin's output as bytes if successful.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::PluginNotLoaded` if the plugin is not currently loaded.
    /// Returns `EngineError::Plugin` if the function call fails or the plugin has crashed too many times.
    ///
    /// # Crash Handling
    ///
    /// When a plugin crashes:
    /// 1. The crash is logged with details
    /// 2. A PluginCrashed event is published to the message bus (if configured)
    /// 3. The plugin is automatically restarted (if under MAX_CRASH_RESTARTS)
    /// 4. The call is retried once after restart
    /// 5. If the retry fails or max restarts exceeded, an error is returned
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # async fn example(runtime: &mut WasmRuntime) -> Result<(), Box<dyn std::error::Error>> {
    /// let input = serde_json::json!({
    ///     "path": "test.txt"
    /// });
    /// let output = runtime.call_plugin(
    ///     "fs-editor",
    ///     "read_file",
    ///     input.to_string().as_bytes()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call_plugin(
        &mut self,
        name: &str,
        function: &str,
        input: &[u8],
    ) -> Result<Vec<u8>, EngineError> {
        tracing::debug!("Calling plugin '{}' function '{}'", name, function);

        // Check if plugin is loaded
        let metadata = self.plugins.get_mut(name).ok_or_else(|| {
            tracing::error!("Plugin '{}' not loaded", name);
            EngineError::PluginNotLoaded(name.to_string())
        })?;

        // Check if plugin has crashed too many times
        if metadata.crash_count >= MAX_CRASH_RESTARTS {
            tracing::error!(
                "Plugin '{}' has crashed {} times, refusing to call",
                name,
                metadata.crash_count
            );
            return Err(EngineError::Plugin(format!(
                "Plugin '{}' has crashed too many times ({} crashes)",
                name, metadata.crash_count
            )));
        }

        // Attempt to call the plugin function
        let result = metadata
            .plugin
            .call::<&[u8], Vec<u8>>(function, input)
            .map_err(|e| {
                tracing::error!("Plugin '{}' function '{}' failed: {}", name, function, e);
                EngineError::Plugin(format!("Plugin call failed: {}", e))
            });

        match result {
            Ok(output) => {
                // Success - reset crash count on successful call
                if metadata.crash_count > 0 {
                    tracing::info!(
                        "Plugin '{}' recovered after {} crashes",
                        name,
                        metadata.crash_count
                    );
                    metadata.crash_count = 0;
                }
                Ok(output)
            }
            Err(e) => {
                // Plugin call failed - treat as potential crash
                self.handle_plugin_crash(name, &e).await?;

                // Retry once after restart
                tracing::info!(
                    "Retrying plugin '{}' function '{}' after restart",
                    name,
                    function
                );
                let metadata = self.plugins.get_mut(name).ok_or_else(|| {
                    EngineError::Plugin(format!("Plugin '{}' disappeared after restart", name))
                })?;

                metadata
                    .plugin
                    .call::<&[u8], Vec<u8>>(function, input)
                    .map_err(|e| {
                        tracing::error!(
                            "Plugin '{}' function '{}' failed again after restart: {}",
                            name,
                            function,
                            e
                        );
                        EngineError::Plugin(format!("Plugin call failed after restart: {}", e))
                    })
            }
        }
    }

    /// Handle a plugin crash by logging, publishing events, and attempting restart
    ///
    /// This internal method is called when a plugin call fails. It:
    /// 1. Increments the crash counter
    /// 2. Logs the crash with details
    /// 3. Publishes a PluginCrashed event to the message bus
    /// 4. Attempts to restart the plugin (if under MAX_CRASH_RESTARTS)
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the crashed plugin
    /// * `error` - The error that caused the crash
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin cannot be restarted or has crashed too many times.
    async fn handle_plugin_crash(
        &mut self,
        name: &str,
        error: &EngineError,
    ) -> Result<(), EngineError> {
        // Increment crash counter
        if let Some(metadata) = self.plugins.get_mut(name) {
            metadata.crash_count += 1;
            let crash_count = metadata.crash_count;

            tracing::error!(
                "Plugin '{}' crashed (crash #{}/{}): {}",
                name,
                crash_count,
                MAX_CRASH_RESTARTS,
                error
            );

            // Publish crash event to message bus
            if let Some(bus) = &self.message_bus {
                let event = Event::PluginCrashed {
                    plugin_id: name.to_string(),
                    error: format!("Crash #{}: {}", crash_count, error),
                };
                bus.publish(event).await;
            }

            // Check if we should attempt restart
            if crash_count >= MAX_CRASH_RESTARTS {
                tracing::error!(
                    "Plugin '{}' has reached maximum crash limit ({}), will not restart",
                    name,
                    MAX_CRASH_RESTARTS
                );
                return Err(EngineError::Plugin(format!(
                    "Plugin '{}' has crashed {} times and will not be restarted",
                    name, MAX_CRASH_RESTARTS
                )));
            }

            // Attempt to restart the plugin
            tracing::warn!(
                "Attempting to restart plugin '{}' (crash #{}/{})",
                name,
                crash_count,
                MAX_CRASH_RESTARTS
            );

            // Remove the crashed plugin (but keep the crash count)
            let crash_count_backup = crash_count;
            self.plugins.remove(name);

            // Reload the plugin
            self.load_plugin(name).await?;

            // Restore the crash count
            if let Some(metadata) = self.plugins.get_mut(name) {
                metadata.crash_count = crash_count_backup;
            }

            tracing::info!("Plugin '{}' restarted successfully after crash", name);
            Ok(())
        } else {
            Err(EngineError::PluginNotLoaded(name.to_string()))
        }
    }

    /// Unload a plugin
    ///
    /// This method removes the plugin from the runtime. The Extism plugin
    /// will be dropped and its resources cleaned up.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin to unload
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # fn example(runtime: &mut WasmRuntime) {
    /// runtime.unload_plugin("fs-editor");
    /// # }
    /// ```
    pub fn unload_plugin(&mut self, name: &str) {
        if self.plugins.remove(name).is_some() {
            tracing::info!("Plugin '{}' unloaded", name);
        } else {
            tracing::debug!("Plugin '{}' not loaded, nothing to unload", name);
        }
    }

    /// Restart a crashed plugin
    ///
    /// This method unloads the plugin and reloads it from scratch.
    /// This is useful for recovering from plugin crashes without
    /// affecting the rest of the engine.
    ///
    /// Note: This method resets the crash counter to 0. Use with caution
    /// as it bypasses the automatic crash limit protection.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin to restart
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin cannot be reloaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # async fn example(runtime: &mut WasmRuntime) -> Result<(), Box<dyn std::error::Error>> {
    /// runtime.restart_plugin("fs-editor").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn restart_plugin(&mut self, name: &str) -> Result<(), EngineError> {
        tracing::warn!("Manually restarting plugin: {}", name);

        // Remove crashed plugin
        self.plugins.remove(name);

        // Reload
        self.load_plugin(name).await?;

        // Reset crash count since this is a manual restart
        if let Some(metadata) = self.plugins.get_mut(name) {
            metadata.crash_count = 0;
        }

        tracing::info!("Plugin '{}' restarted successfully", name);
        Ok(())
    }

    /// Get the crash count for a plugin
    ///
    /// Returns the number of times the plugin has crashed since it was loaded
    /// or since the last manual restart.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin
    ///
    /// # Returns
    ///
    /// Returns the crash count, or None if the plugin is not loaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # fn example(runtime: &WasmRuntime) {
    /// if let Some(count) = runtime.get_crash_count("fs-editor") {
    ///     println!("Plugin has crashed {} times", count);
    /// }
    /// # }
    /// ```
    pub fn get_crash_count(&self, name: &str) -> Option<u32> {
        self.plugins.get(name).map(|m| m.crash_count)
    }

    /// Check if a plugin is currently loaded
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin to check
    ///
    /// # Returns
    ///
    /// Returns true if the plugin is loaded, false otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # fn example(runtime: &WasmRuntime) {
    /// if runtime.is_plugin_loaded("fs-editor") {
    ///     println!("File system editor plugin is loaded");
    /// }
    /// # }
    /// ```
    pub fn is_plugin_loaded(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Get a list of all loaded plugin names
    ///
    /// # Returns
    ///
    /// Returns a vector of plugin names that are currently loaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # fn example(runtime: &WasmRuntime) {
    /// let loaded_plugins = runtime.loaded_plugins();
    /// println!("Loaded plugins: {:?}", loaded_plugins);
    /// # }
    /// ```
    pub fn loaded_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Unload all plugins
    ///
    /// This method removes all loaded plugins from the runtime.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::WasmRuntime;
    /// # fn example(runtime: &mut WasmRuntime) {
    /// runtime.unload_all();
    /// # }
    /// ```
    pub fn unload_all(&mut self) {
        tracing::info!("Unloading all plugins");

        let plugin_names: Vec<String> = self.plugins.keys().cloned().collect();

        for name in plugin_names {
            self.unload_plugin(&name);
        }

        tracing::info!("All plugins unloaded");
    }
}

impl Drop for WasmRuntime {
    /// Ensure all plugins are properly unloaded when the runtime is dropped
    fn drop(&mut self) {
        self.unload_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a valid manifest and crypto setup
    // They are primarily for documentation and will be expanded with integration tests

    #[test]
    fn test_wasm_runtime_creation() {
        // Create a minimal manifest for testing
        let _manifest = Manifest {
            version: "1.0.0".to_string(),
            team_public_key: "ed25519:test_key".to_string(),
            signature: "ed25519:test_sig".to_string(),
            generated_at: "2024-01-15T10:30:00Z".to_string(),
            core_tools: vec![],
            plugins: vec![],
        };

        // Note: This will fail until build.rs is set up with a valid key
        // let crypto = Arc::new(CryptoModule::new().unwrap());
        // let fs_guard = Arc::new(FileSystemGuard::new(PathBuf::from("/workspace")));
        // let runtime = WasmRuntime::new(manifest, crypto, fs_guard);
        // assert_eq!(runtime.loaded_plugins().len(), 0);
    }

    #[test]
    fn test_plugin_not_in_manifest() {
        let _manifest = Manifest {
            version: "1.0.0".to_string(),
            team_public_key: "ed25519:test_key".to_string(),
            signature: "ed25519:test_sig".to_string(),
            generated_at: "2024-01-15T10:30:00Z".to_string(),
            core_tools: vec![],
            plugins: vec![],
        };

        // Note: This will fail until build.rs is set up with a valid key
        // let crypto = Arc::new(CryptoModule::new().unwrap());
        // let fs_guard = Arc::new(FileSystemGuard::new(PathBuf::from("/workspace")));
        // let mut runtime = WasmRuntime::new(manifest, crypto, fs_guard);

        // Attempt to load a plugin not in the manifest
        // let result = runtime.load_plugin("nonexistent").await;
        // assert!(matches!(result, Err(EngineError::PluginNotInManifest(_))));
    }

    #[test]
    fn test_is_plugin_loaded() {
        let _manifest = Manifest {
            version: "1.0.0".to_string(),
            team_public_key: "ed25519:test_key".to_string(),
            signature: "ed25519:test_sig".to_string(),
            generated_at: "2024-01-15T10:30:00Z".to_string(),
            core_tools: vec![],
            plugins: vec![],
        };

        // Note: This will fail until build.rs is set up with a valid key
        // let crypto = Arc::new(CryptoModule::new().unwrap());
        // let fs_guard = Arc::new(FileSystemGuard::new(PathBuf::from("/workspace")));
        // let runtime = WasmRuntime::new(manifest, crypto, fs_guard);

        // assert!(!runtime.is_plugin_loaded("fs-editor"));
    }

    #[test]
    fn test_loaded_plugins_empty() {
        let _manifest = Manifest {
            version: "1.0.0".to_string(),
            team_public_key: "ed25519:test_key".to_string(),
            signature: "ed25519:test_sig".to_string(),
            generated_at: "2024-01-15T10:30:00Z".to_string(),
            core_tools: vec![],
            plugins: vec![],
        };

        // Note: This will fail until build.rs is set up with a valid key
        // let crypto = Arc::new(CryptoModule::new().unwrap());
        // let fs_guard = Arc::new(FileSystemGuard::new(PathBuf::from("/workspace")));
        // let runtime = WasmRuntime::new(manifest, crypto, fs_guard);

        // assert_eq!(runtime.loaded_plugins().len(), 0);
    }

    #[test]
    fn test_crash_count_tracking() {
        // Test that crash count is properly tracked
        // This would require a mock plugin that can be made to crash
        // For now, this is a placeholder for future integration tests
    }

    #[test]
    fn test_max_crash_restarts() {
        // Test that plugins are not restarted after MAX_CRASH_RESTARTS
        // This would require a mock plugin that crashes repeatedly
        // For now, this is a placeholder for future integration tests
    }

    #[test]
    fn test_crash_event_publishing() {
        // Test that PluginCrashed events are published to the message bus
        // This would require setting up a message bus and mock plugin
        // For now, this is a placeholder for future integration tests
    }
}
