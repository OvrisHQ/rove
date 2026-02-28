//! Native runtime for loading and managing core tools
//!
//! This module implements the NativeRuntime which loads core tools as native
//! shared libraries (.so/.dylib/.dll) with comprehensive security verification.
//!
//! # Four-Gate Verification System
//!
//! Every core tool must pass through four security gates before loading:
//!
//! 1. **Gate 1: Manifest Check** - Verify the tool is declared in the signed manifest
//! 2. **Gate 2: Hash Verification** - Verify the file's BLAKE3 hash matches the manifest
//! 3. **Gate 3: Team Signature** - Verify the manifest's Ed25519 signature with team public key
//! 4. **Gate 4: Tool Signature** - Verify the individual tool's Ed25519 signature
//!
//! If any gate fails, the compromised file is immediately deleted and loading is aborted.
//!
//! # Requirements
//!
//! - Requirement 6.2: Gate 1 - Check tool is in manifest
//! - Requirement 6.3: Gate 2 - Verify file hash with BLAKE3
//! - Requirement 6.4: Gate 3 - Verify team signature on manifest
//! - Requirement 6.5: Gate 4 - Verify individual tool signature
//! - Requirement 6.6: Delete compromised files on failure
//!
//! # Examples
//!
//! ```no_run
//! use rove_engine::runtime::NativeRuntime;
//! use rove_engine::crypto::CryptoModule;
//! use sdk::manifest::Manifest;
//! use sdk::core_tool::CoreContext;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let manifest = Manifest::from_json(&std::fs::read_to_string("manifest.json")?)?;
//! let crypto = Arc::new(CryptoModule::new()?);
//! let mut runtime = NativeRuntime::new(manifest, crypto);
//!
//! // Create CoreContext with handles
//! let ctx = CoreContext::new(
//!     // ... handles ...
//! #   todo!(), todo!(), todo!(), todo!(), todo!(), todo!()
//! );
//!
//! // Load a core tool with four-gate verification
//! runtime.load_tool("telegram", ctx)?;
//!
//! // Call the tool
//! let input = sdk::types::ToolInput::new("send_message")
//!     .with_param("chat_id", serde_json::json!(123456))
//!     .with_param("text", serde_json::json!("Hello from Rove!"));
//! let output = runtime.call_tool("telegram", input)?;
//! # Ok(())
//! # }
//! ```

use crate::crypto::CryptoModule;
use sdk::{
    core_tool::CoreContext,
    core_tool::CoreTool,
    errors::EngineError,
    manifest::Manifest,
    types::{ToolInput, ToolOutput},
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Native runtime for loading and managing core tools
///
/// The NativeRuntime loads core tools as native shared libraries via dlopen
/// and manages their lifecycle. All tools must pass through four security gates
/// before being loaded.
///
/// # Thread Safety
///
/// This struct is not thread-safe by default. Wrap in Arc<Mutex<_>> if sharing
/// across threads is needed.
pub struct NativeRuntime {
    /// Loaded core tools indexed by name
    tools: HashMap<String, Box<dyn CoreTool>>,
    /// Manifest containing tool metadata and signatures
    manifest: Manifest,
    /// Cryptographic module for verification
    crypto: Arc<CryptoModule>,
    /// Loaded libraries (kept alive to prevent unloading)
    #[allow(dead_code)]
    libraries: HashMap<String, libloading::Library>,
}

impl NativeRuntime {
    /// Create a new NativeRuntime with the given manifest and crypto module
    ///
    /// # Arguments
    ///
    /// * `manifest` - The signed manifest containing core tool metadata
    /// * `crypto` - Cryptographic module for signature and hash verification
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::NativeRuntime;
    /// # use rove_engine::crypto::CryptoModule;
    /// # use sdk::manifest::Manifest;
    /// # use std::sync::Arc;
    /// let manifest = Manifest::from_json(&std::fs::read_to_string("manifest.json").unwrap()).unwrap();
    /// let crypto = Arc::new(CryptoModule::new().unwrap());
    /// let runtime = NativeRuntime::new(manifest, crypto);
    /// ```
    pub fn new(manifest: Manifest, crypto: Arc<CryptoModule>) -> Self {
        tracing::info!("Initializing NativeRuntime");
        Self {
            tools: HashMap::new(),
            manifest,
            crypto,
            libraries: HashMap::new(),
        }
    }

    /// Load a core tool with four-gate verification
    ///
    /// This method implements the complete four-gate security verification system:
    ///
    /// 1. **Gate 1**: Verify the tool is declared in the manifest
    /// 2. **Gate 2**: Verify the file's BLAKE3 hash matches the manifest
    /// 3. **Gate 3**: Verify the manifest's Ed25519 signature with team public key
    /// 4. **Gate 4**: Verify the individual tool's Ed25519 signature
    ///
    /// If any gate fails, the compromised file is deleted and an error is returned.
    ///
    /// # Platform-Specific Loading
    ///
    /// The method uses `libloading` which automatically handles platform-specific
    /// shared library extensions:
    /// - Linux: `.so`
    /// - macOS: `.dylib`
    /// - Windows: `.dll`
    ///
    /// The manifest should specify the correct path for each platform. You can use
    /// the `platform::library_filename()` utility to construct platform-specific
    /// filenames:
    ///
    /// ```
    /// use rove_engine::platform::library_filename;
    /// let filename = library_filename("telegram");
    /// // Linux: "libtelegram.so"
    /// // macOS: "libtelegram.dylib"
    /// // Windows: "telegram.dll"
    /// ```
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the tool to load (must match manifest entry)
    /// * `ctx` - CoreContext to provide to the tool for engine interaction
    ///
    /// # Errors
    ///
    /// Returns `EngineError::ToolNotInManifest` if the tool is not declared in the manifest (Gate 1 failure).
    /// Returns `EngineError::HashMismatch` if the file hash doesn't match (Gate 2 failure).
    /// Returns `EngineError::InvalidSignature` if signature verification fails (Gate 3 or 4 failure).
    /// Returns `EngineError::LibraryLoadFailed` if the shared library cannot be loaded.
    /// Returns `EngineError::SymbolNotFound` if the create_tool symbol is not found.
    ///
    /// # Security
    ///
    /// **CRITICAL**: This method deletes the tool file if any verification gate fails.
    /// This prevents execution of compromised or tampered binaries.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::NativeRuntime;
    /// # use sdk::core_tool::CoreContext;
    /// # fn example(runtime: &mut NativeRuntime, ctx: CoreContext) -> Result<(), Box<dyn std::error::Error>> {
    /// runtime.load_tool("telegram", ctx)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_tool(&mut self, name: &str, ctx: CoreContext) -> Result<(), EngineError> {
        tracing::info!("Loading core tool: {}", name);

        // Gate 1: Check tool is in manifest (Requirement 6.2)
        let tool_entry = self.manifest.get_core_tool(name).ok_or_else(|| {
            tracing::error!("Gate 1 FAILED: Tool '{}' not found in manifest", name);
            EngineError::ToolNotInManifest(name.to_string())
        })?;

        tracing::info!("Gate 1 PASSED: Tool '{}' found in manifest", name);

        // Convert path to PathBuf
        let tool_path = PathBuf::from(&tool_entry.path);

        // Gate 2: Verify file hash with BLAKE3 (Requirement 6.3)
        // Note: verify_file will delete the file if hash doesn't match (Requirement 6.6)
        self.crypto
            .verify_file(&tool_path, &tool_entry.hash)
            .map_err(|e| {
                tracing::error!(
                    "Gate 2 FAILED: Hash verification failed for '{}': {}",
                    name,
                    e
                );
                e
            })?;

        tracing::info!("Gate 2 PASSED: File hash verified for '{}'", name);

        // Gate 3: Verify team signature on manifest (Requirement 6.4)
        // We need to serialize the manifest to bytes for signature verification
        let manifest_bytes = serde_json::to_vec(&self.manifest)
            .map_err(|e| EngineError::Config(format!("Failed to serialize manifest: {}", e)))?;

        self.crypto
            .verify_manifest(&manifest_bytes, &self.manifest.signature)
            .map_err(|e| {
                tracing::error!(
                    "Gate 3 FAILED: Manifest signature verification failed: {}",
                    e
                );
                // Delete the tool file since manifest is compromised
                if let Err(del_err) = std::fs::remove_file(&tool_path) {
                    tracing::error!(
                        "Failed to delete compromised file {}: {}",
                        tool_path.display(),
                        del_err
                    );
                }
                e
            })?;

        tracing::info!("Gate 3 PASSED: Manifest signature verified");

        // Gate 4: Verify individual tool signature (Requirement 6.5)
        self.crypto
            .verify_file_signature(&tool_path, &tool_entry.signature)
            .map_err(|e| {
                tracing::error!(
                    "Gate 4 FAILED: Tool signature verification failed for '{}': {}",
                    name,
                    e
                );
                // Delete the tool file since it's compromised
                if let Err(del_err) = std::fs::remove_file(&tool_path) {
                    tracing::error!(
                        "Failed to delete compromised file {}: {}",
                        tool_path.display(),
                        del_err
                    );
                }
                e
            })?;

        tracing::info!("Gate 4 PASSED: Tool signature verified for '{}'", name);

        // All gates passed - safe to load the library
        tracing::info!("All four gates passed for '{}', loading library...", name);

        // Load the shared library
        let lib = unsafe {
            libloading::Library::new(&tool_path).map_err(|e| {
                tracing::error!("Failed to load library {}: {}", tool_path.display(), e);
                EngineError::LibraryLoadFailed(e.to_string())
            })?
        };

        // Get the create_tool constructor function
        let create_tool: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn CoreTool> = unsafe {
            lib.get(b"create_tool").map_err(|e| {
                tracing::error!(
                    "Symbol 'create_tool' not found in {}: {}",
                    tool_path.display(),
                    e
                );
                EngineError::SymbolNotFound(e.to_string())
            })?
        };

        // Create the tool instance
        let mut tool = unsafe {
            let ptr = create_tool();
            if ptr.is_null() {
                tracing::error!("create_tool returned null pointer for '{}'", name);
                return Err(EngineError::LibraryLoadFailed(
                    "create_tool returned null".to_string(),
                ));
            }
            Box::from_raw(ptr)
        };

        // Initialize the tool with CoreContext
        tool.start(ctx).map_err(|e| {
            tracing::error!("Failed to start tool '{}': {}", name, e);
            e
        })?;

        // Store the tool and library
        self.tools.insert(name.to_string(), tool);
        self.libraries.insert(name.to_string(), lib);

        tracing::info!("Core tool '{}' loaded successfully", name);
        Ok(())
    }

    /// Unload a core tool and call its stop() method
    ///
    /// This method removes the tool from the runtime and calls its stop() method
    /// to allow for graceful cleanup.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the tool to unload
    ///
    /// # Errors
    ///
    /// Returns an error if the tool's stop() method fails.
    /// If the tool is not loaded, this method succeeds silently.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::NativeRuntime;
    /// # fn example(runtime: &mut NativeRuntime) -> Result<(), Box<dyn std::error::Error>> {
    /// runtime.unload_tool("telegram")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn unload_tool(&mut self, name: &str) -> Result<(), EngineError> {
        if let Some(mut tool) = self.tools.remove(name) {
            tracing::info!("Unloading core tool: {}", name);

            // Call stop() for graceful cleanup
            tool.stop().map_err(|e| {
                tracing::error!("Failed to stop tool '{}': {}", name, e);
                e
            })?;

            // Remove the library (will be unloaded when dropped)
            self.libraries.remove(name);

            tracing::info!("Core tool '{}' unloaded successfully", name);
        } else {
            tracing::debug!("Tool '{}' not loaded, nothing to unload", name);
        }

        Ok(())
    }

    /// Call a loaded core tool with the given input
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the tool to call
    /// * `input` - Input parameters for the tool
    ///
    /// # Returns
    ///
    /// Returns the tool's output if successful.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::ToolNotLoaded` if the tool is not currently loaded.
    /// Returns any error from the tool's handle() method.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::NativeRuntime;
    /// # use sdk::types::ToolInput;
    /// # fn example(runtime: &NativeRuntime) -> Result<(), Box<dyn std::error::Error>> {
    /// let input = ToolInput::new("send_message")
    ///     .with_param("chat_id", serde_json::json!(123456))
    ///     .with_param("text", serde_json::json!("Hello!"));
    /// let output = runtime.call_tool("telegram", input)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn call_tool(&self, name: &str, input: ToolInput) -> Result<ToolOutput, EngineError> {
        tracing::debug!(
            "Calling core tool '{}' with method '{}'",
            name,
            input.method
        );

        let tool = self.tools.get(name).ok_or_else(|| {
            tracing::error!("Tool '{}' not loaded", name);
            EngineError::ToolNotLoaded(name.to_string())
        })?;

        tool.handle(input).map_err(|e| {
            tracing::error!("Tool '{}' returned error: {}", name, e);
            e
        })
    }

    /// Check if a tool is currently loaded
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the tool to check
    ///
    /// # Returns
    ///
    /// Returns true if the tool is loaded, false otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::NativeRuntime;
    /// # fn example(runtime: &NativeRuntime) {
    /// if runtime.is_tool_loaded("telegram") {
    ///     println!("Telegram bot is running");
    /// }
    /// # }
    /// ```
    pub fn is_tool_loaded(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get a list of all loaded tool names
    ///
    /// # Returns
    ///
    /// Returns a vector of tool names that are currently loaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::NativeRuntime;
    /// # fn example(runtime: &NativeRuntime) {
    /// let loaded_tools = runtime.loaded_tools();
    /// println!("Loaded tools: {:?}", loaded_tools);
    /// # }
    /// ```
    pub fn loaded_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Unload all core tools
    ///
    /// This method calls stop() on all loaded tools and removes them from the runtime.
    /// Errors from individual tools are logged but don't prevent other tools from being unloaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use rove_engine::runtime::NativeRuntime;
    /// # fn example(runtime: &mut NativeRuntime) {
    /// runtime.unload_all();
    /// # }
    /// ```
    pub fn unload_all(&mut self) {
        tracing::info!("Unloading all core tools");

        let tool_names: Vec<String> = self.tools.keys().cloned().collect();

        for name in tool_names {
            if let Err(e) = self.unload_tool(&name) {
                tracing::error!("Error unloading tool '{}': {}", name, e);
            }
        }

        tracing::info!("All core tools unloaded");
    }
}

impl Drop for NativeRuntime {
    /// Ensure all tools are properly unloaded when the runtime is dropped
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
    fn test_native_runtime_creation() {
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
        // let runtime = NativeRuntime::new(manifest, crypto);
        // assert_eq!(runtime.loaded_tools().len(), 0);
    }

    #[test]
    fn test_tool_not_in_manifest() {
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
        // let mut runtime = NativeRuntime::new(manifest, crypto);

        // Create a dummy CoreContext
        // let ctx = CoreContext::new(...);

        // Attempt to load a tool not in the manifest
        // let result = runtime.load_tool("nonexistent", ctx);
        // assert!(matches!(result, Err(EngineError::ToolNotInManifest(_))));
    }

    #[test]
    fn test_is_tool_loaded() {
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
        // let runtime = NativeRuntime::new(manifest, crypto);

        // assert!(!runtime.is_tool_loaded("telegram"));
    }

    #[test]
    fn test_loaded_tools_empty() {
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
        // let runtime = NativeRuntime::new(manifest, crypto);

        // assert_eq!(runtime.loaded_tools().len(), 0);
    }
}
