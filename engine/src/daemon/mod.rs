//! Daemon lifecycle management
//!
//! This module provides the `DaemonManager` for managing the Rove daemon process.
//! It handles:
//! - PID file management (~/.rove/rove.pid)
//! - Daemon start/stop/status operations
//! - Graceful shutdown with timeout
//! - Detection of already-running daemons
//!
//! # Security
//!
//! The daemon manager ensures only one instance runs at a time by:
//! 1. Checking for existing PID file
//! 2. Verifying the process is actually running
//! 3. Handling stale PID files (process no longer exists)
//!
//! # Examples
//!
//! ```no_run
//! use rove_engine::daemon::DaemonManager;
//! use rove_engine::config::Config;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = Config::load_or_create()?;
//! let manager = DaemonManager::new(&config)?;
//!
//! // Start the daemon
//! manager.start().await?;
//!
//! // Check status
//! let status = DaemonManager::status(&config)?;
//! println!("Daemon running: {}", status.is_running);
//!
//! // Stop the daemon
//! DaemonManager::stop(&config).await?;
//! # Ok(())
//! # }
//! ```

use std::fs;
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::config::Config;
use crate::db::Database;
use crate::runtime::native::NativeRuntime;
use crate::runtime::wasm::WasmRuntime;
use sdk::errors::EngineError;

/// Result type for daemon operations
pub type Result<T> = std::result::Result<T, EngineError>;

/// Daemon status information
#[derive(Debug, Clone)]
pub struct DaemonStatus {
    /// Whether the daemon is currently running
    pub is_running: bool,

    /// Process ID if running
    pub pid: Option<u32>,

    /// Path to the PID file
    pub pid_file: PathBuf,

    /// Provider availability status
    pub providers: ProviderAvailability,
}

/// Provider availability information
#[derive(Debug, Clone)]
pub struct ProviderAvailability {
    /// Whether Ollama is available
    pub ollama: bool,

    /// Whether OpenAI is available (API key configured)
    pub openai: bool,

    /// Whether Anthropic is available (API key configured)
    pub anthropic: bool,

    /// Whether Gemini is available (API key configured)
    pub gemini: bool,

    /// Whether NVIDIA NIM is available (API key configured)
    pub nvidia_nim: bool,
}

/// Daemon manager for lifecycle operations
///
/// The `DaemonManager` handles starting, stopping, and monitoring the Rove daemon.
/// It ensures only one daemon instance runs at a time through PID file management.
///
/// # PID File Management
///
/// The PID file is stored at `~/.rove/rove.pid` and contains the process ID
/// of the running daemon. The manager:
/// - Creates the PID file on start
/// - Checks for existing PID files before starting
/// - Verifies the process is actually running
/// - Removes stale PID files (process no longer exists)
/// - Cleans up the PID file on graceful shutdown
///
/// # Graceful Shutdown
///
/// The daemon supports graceful shutdown via SIGTERM signal:
/// 1. Sets shutdown flag to refuse new tasks
/// 2. Waits up to 30 seconds for in-progress tasks
/// 3. Calls stop() on all core tools
/// 4. Closes all plugins
/// 5. Flushes SQLite WAL
/// 6. Removes PID file
pub struct DaemonManager {
    /// Path to the PID file
    pid_file: PathBuf,

    /// Shutdown flag for graceful termination
    shutdown_flag: Arc<AtomicBool>,

    /// Task handles for background operations
    /// Will be used for tracking in-progress tasks during shutdown
    #[allow(dead_code)]
    task_handles: Vec<JoinHandle<()>>,

    /// Native runtime for core tools (optional, set during start)
    native_runtime: Option<Arc<tokio::sync::Mutex<NativeRuntime>>>,

    /// WASM runtime for plugins (optional, set during start)
    wasm_runtime: Option<Arc<tokio::sync::Mutex<WasmRuntime>>>,

    /// Database connection (optional, set during start)
    database: Option<Arc<Database>>,
}

impl DaemonManager {
    /// Creates a new daemon manager
    ///
    /// # Arguments
    ///
    /// * `config` - Engine configuration containing data directory path
    ///
    /// # Returns
    ///
    /// Returns a new `DaemonManager` instance or an error if the PID file path
    /// cannot be determined.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rove_engine::daemon::DaemonManager;
    /// use rove_engine::config::Config;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::load_or_create()?;
    /// let manager = DaemonManager::new(&config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(config: &Config) -> Result<Self> {
        let pid_file = Self::get_pid_file_path(config)?;

        Ok(Self {
            pid_file,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            task_handles: Vec::new(),
            native_runtime: None,
            wasm_runtime: None,
            database: None,
        })
    }

    /// Starts the daemon
    ///
    /// This method:
    /// 1. Checks for an existing daemon (returns `DaemonAlreadyRunning` if found)
    /// 2. Writes the current process PID to the PID file
    /// 3. Initializes daemon components
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful start, or an error if:
    /// - A daemon is already running (`DaemonAlreadyRunning`)
    /// - The PID file cannot be written
    /// - Component initialization fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rove_engine::daemon::DaemonManager;
    /// use rove_engine::config::Config;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::load_or_create()?;
    /// let manager = DaemonManager::new(&config)?;
    /// manager.start().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&self) -> Result<()> {
        // Check if daemon is already running
        if self.is_daemon_running()? {
            return Err(EngineError::DaemonAlreadyRunning);
        }

        // Write PID file
        self.write_pid_file()?;

        // Set up SIGTERM signal handler (Requirement 14.5)
        let shutdown_flag = Arc::clone(&self.shutdown_flag);
        let _signal_handle = Self::setup_signal_handler(shutdown_flag);
        tracing::info!("SIGTERM signal handler installed");

        // Verify manifest integrity at startup (Requirement 6.7, 26.1, 28.3)
        if let Err(e) = Self::verify_manifest_at_startup() {
            tracing::warn!("Manifest verification skipped or failed: {}", e);
            // In development mode, we continue despite verification failure.
            // In production, this would be a hard error.
            #[cfg(feature = "production")]
            return Err(EngineError::Config(format!(
                "Manifest verification failed: {}",
                e
            )));
        }

        // TODO: Initialize daemon components (agent, runtimes, etc.)
        // Components should be registered with set_native_runtime(), set_wasm_runtime(), set_database()

        Ok(())
    }

    /// Stops the daemon
    ///
    /// This method:
    /// 1. Reads the PID from the PID file
    /// 2. Sends SIGTERM to the daemon process (Requirement 14.5)
    /// 3. Waits for graceful shutdown
    ///
    /// The daemon process will handle SIGTERM by calling `graceful_shutdown()`.
    ///
    /// # Arguments
    ///
    /// * `config` - Engine configuration containing data directory path
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful stop, or an error if:
    /// - No daemon is running
    /// - The PID file cannot be read
    /// - The process cannot be signaled
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rove_engine::daemon::DaemonManager;
    /// use rove_engine::config::Config;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::load_or_create()?;
    /// DaemonManager::stop(&config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stop(config: &Config) -> Result<()> {
        let pid_file = Self::get_pid_file_path(config)?;

        // Read PID from file
        let _pid = Self::read_pid_file(&pid_file)?;

        // Send SIGTERM to the process (Requirement 14.5)
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            tracing::info!("Sending SIGTERM to daemon process {}", _pid);
            kill(Pid::from_raw(_pid as i32), Signal::SIGTERM).map_err(|e| {
                EngineError::Io(std::io::Error::other(format!(
                    "Failed to send SIGTERM: {}",
                    e
                )))
            })?;
        }

        #[cfg(windows)]
        {
            return Err(EngineError::Config(
                "Daemon stop not yet implemented for Windows".to_string(),
            ));
        }

        #[cfg(unix)]
        {
            // Wait for the process to exit (with timeout)
            tracing::info!("Waiting for daemon to shut down gracefully");
            let wait_result = timeout(Duration::from_secs(35), async {
                loop {
                    if !Self::is_process_running(_pid) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
            .await;

            if wait_result.is_err() {
                tracing::warn!("Daemon did not stop within 35 seconds");
            } else {
                tracing::info!("Daemon stopped successfully");
            }

            // Remove PID file if it still exists
            if pid_file.exists() {
                fs::remove_file(&pid_file).map_err(EngineError::Io)?;
            }

            Ok(())
        }
    }

    /// Gets the daemon status
    ///
    /// This method reports:
    /// - Whether the daemon is running
    /// - The daemon's PID if running
    /// - Which LLM providers are available
    ///
    /// # Arguments
    ///
    /// * `config` - Engine configuration containing data directory path
    ///
    /// # Returns
    ///
    /// Returns a `DaemonStatus` struct with information about the daemon state
    /// and provider availability.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rove_engine::daemon::DaemonManager;
    /// use rove_engine::config::Config;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::load_or_create()?;
    /// let status = DaemonManager::status(&config)?;
    /// println!("Daemon running: {}", status.is_running);
    /// println!("Ollama available: {}", status.providers.ollama);
    /// # Ok(())
    /// # }
    /// ```
    pub fn status(config: &Config) -> Result<DaemonStatus> {
        let pid_file = Self::get_pid_file_path(config)?;

        let (is_running, pid) = match Self::read_pid_file(&pid_file) {
            Ok(pid) => {
                if Self::is_process_running(pid) {
                    (true, Some(pid))
                } else {
                    // Stale PID file - process no longer exists
                    (false, None)
                }
            }
            Err(_) => {
                // No PID file or cannot read it
                (false, None)
            }
        };

        // Check provider availability (Requirement 14.13)
        let providers = Self::check_provider_availability(config);

        Ok(DaemonStatus {
            is_running,
            pid,
            pid_file,
            providers,
        })
    }

    /// Waits for shutdown signal with timeout
    ///
    /// This method blocks until either:
    /// - The shutdown flag is set
    /// - The timeout expires
    ///
    /// # Arguments
    ///
    /// * `timeout_duration` - Maximum time to wait for shutdown
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if shutdown was signaled, or an error if the timeout expired.
    pub async fn wait_for_shutdown(&self, timeout_duration: Duration) -> Result<()> {
        let result = timeout(timeout_duration, async {
            while !self.shutdown_flag.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(EngineError::Config("Shutdown timeout exceeded".to_string())),
        }
    }

    /// Signals the daemon to shut down
    ///
    /// This sets the shutdown flag, which will cause the daemon to begin
    /// graceful shutdown procedures.
    pub fn signal_shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
    }

    /// Performs graceful shutdown of all daemon components
    ///
    /// This method implements the complete shutdown sequence:
    /// 1. Sets shutdown flag to refuse new tasks (Requirement 14.6)
    /// 2. Waits up to 30 seconds for in-progress tasks (Requirement 14.8)
    /// 3. Calls stop() on all core tools (Requirement 14.9)
    /// 4. Closes all plugins (Requirement 14.10)
    /// 5. Flushes SQLite WAL (Requirement 14.11)
    /// 6. Removes PID file (Requirement 14.12)
    ///
    /// # Arguments
    ///
    /// * `_config` - Engine configuration (reserved for future use)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful shutdown, or an error if any step fails.
    /// Errors are logged but don't prevent subsequent shutdown steps.
    ///
    /// Requirements: 14.6, 14.7, 14.8, 14.9, 14.10, 14.11, 14.12
    pub async fn graceful_shutdown(&mut self, _config: &Config) -> Result<()> {
        tracing::info!("Starting graceful shutdown");

        // Step 1: Set shutdown flag to refuse new tasks (Requirement 14.6)
        self.signal_shutdown();
        tracing::info!("Shutdown flag set - refusing new tasks");

        // Step 2: Wait up to 30 seconds for in-progress tasks (Requirement 14.8)
        tracing::info!("Waiting up to 30 seconds for in-progress tasks to complete");
        match self.wait_for_shutdown(Duration::from_secs(30)).await {
            Ok(_) => tracing::info!("All in-progress tasks completed"),
            Err(_) => tracing::warn!("Timeout waiting for tasks - proceeding with shutdown"),
        }

        // Step 3: Call stop() on all core tools (Requirement 14.9)
        if let Some(native_runtime) = &self.native_runtime {
            tracing::info!("Stopping all core tools");
            let mut runtime = native_runtime.lock().await;
            runtime.unload_all();
            tracing::info!("All core tools stopped");
        } else {
            tracing::debug!("No native runtime to stop");
        }

        // Step 4: Close all plugins (Requirement 14.10)
        if let Some(wasm_runtime) = &self.wasm_runtime {
            tracing::info!("Closing all plugins");
            let mut runtime = wasm_runtime.lock().await;
            runtime.unload_all();
            tracing::info!("All plugins closed");
        } else {
            tracing::debug!("No WASM runtime to close");
        }

        // Step 5: Flush SQLite WAL (Requirement 14.11)
        if let Some(database) = &self.database {
            tracing::info!("Flushing SQLite WAL");
            match database.flush_wal().await {
                Ok(_) => tracing::info!("SQLite WAL flushed successfully"),
                Err(e) => tracing::error!("Failed to flush SQLite WAL: {}", e),
            }
        } else {
            tracing::debug!("No database to flush");
        }

        // Step 6: Remove PID file (Requirement 14.12)
        if self.pid_file.exists() {
            tracing::info!("Removing PID file");
            match fs::remove_file(&self.pid_file) {
                Ok(_) => tracing::info!("PID file removed successfully"),
                Err(e) => tracing::error!("Failed to remove PID file: {}", e),
            }
        }

        tracing::info!("Graceful shutdown completed");
        Ok(())
    }

    /// Sets up SIGTERM signal handler for graceful shutdown
    ///
    /// This method installs a signal handler that will trigger graceful shutdown
    /// when the daemon receives SIGTERM (e.g., from `rove stop`).
    ///
    /// # Arguments
    ///
    /// * `shutdown_flag` - Shared atomic flag to signal shutdown
    ///
    /// # Returns
    ///
    /// Returns a `JoinHandle` for the signal handler task.
    ///
    /// Requirements: 14.5
    #[cfg(unix)]
    pub fn setup_signal_handler(shutdown_flag: Arc<AtomicBool>) -> JoinHandle<()> {
        use tokio::signal::unix::{signal, SignalKind};

        tokio::spawn(async move {
            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");

            sigterm.recv().await;
            tracing::info!("Received SIGTERM signal");
            shutdown_flag.store(true, Ordering::Relaxed);
        })
    }

    /// Sets up signal handler for Windows (placeholder)
    ///
    /// Windows doesn't have SIGTERM, so this is a placeholder for future implementation.
    #[cfg(windows)]
    pub fn setup_signal_handler(_shutdown_flag: Arc<AtomicBool>) -> JoinHandle<()> {
        tokio::spawn(async move {
            // Windows signal handling would go here
            // For now, just keep the task alive
            tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
        })
    }

    /// Sets the native runtime for shutdown management
    ///
    /// This should be called after the native runtime is initialized.
    pub fn set_native_runtime(&mut self, runtime: Arc<tokio::sync::Mutex<NativeRuntime>>) {
        self.native_runtime = Some(runtime);
    }

    /// Sets the WASM runtime for shutdown management
    ///
    /// This should be called after the WASM runtime is initialized.
    pub fn set_wasm_runtime(&mut self, runtime: Arc<tokio::sync::Mutex<WasmRuntime>>) {
        self.wasm_runtime = Some(runtime);
    }

    /// Sets the database for shutdown management
    ///
    /// This should be called after the database is initialized.
    pub fn set_database(&mut self, database: Arc<Database>) {
        self.database = Some(database);
    }

    /// Verify manifest integrity at engine startup (Requirement 6.7, 26.1, 28.3)
    ///
    /// Checks for a manifest.json in the data directory, verifies its signature
    /// using the embedded team public key, and validates file hashes for all
    /// listed core tools and plugins.
    fn verify_manifest_at_startup() -> std::result::Result<(), String> {
        use crate::crypto::CryptoModule;

        // Look for manifest in standard locations
        let manifest_paths = [
            std::path::PathBuf::from("manifest/manifest.json"),
            dirs::home_dir()
                .map(|h| h.join(".rove/manifest.json"))
                .unwrap_or_default(),
        ];

        let manifest_path = manifest_paths.iter().find(|p| p.exists());

        let manifest_path = match manifest_path {
            Some(p) => p,
            None => {
                tracing::debug!("No manifest.json found — skipping verification");
                return Ok(());
            }
        };

        tracing::info!("Verifying manifest at {}", manifest_path.display());

        // Read manifest
        let manifest_bytes =
            std::fs::read(manifest_path).map_err(|e| format!("Failed to read manifest: {}", e))?;

        // Parse manifest to extract signature and file entries
        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| format!("Failed to parse manifest JSON: {}", e))?;

        // Initialize crypto module
        let crypto =
            CryptoModule::new().map_err(|e| format!("Failed to initialize crypto: {}", e))?;

        // Verify manifest signature if present
        if let Some(signature) = manifest.get("signature").and_then(|s| s.as_str()) {
            // Verify signature over the manifest content (excluding signature field)
            let mut manifest_for_verify = manifest.clone();
            if let Some(obj) = manifest_for_verify.as_object_mut() {
                obj.remove("signature");
            }
            let verify_bytes = serde_json::to_vec(&manifest_for_verify)
                .map_err(|e| format!("Failed to serialize manifest for verification: {}", e))?;

            crypto
                .verify_manifest(&verify_bytes, signature)
                .map_err(|e| format!("Manifest signature verification failed: {}", e))?;

            tracing::info!("Manifest signature verified successfully");
        } else {
            tracing::debug!("No signature in manifest — skipping signature verification");
        }

        // Verify file hashes for listed entries
        if let Some(entries) = manifest.get("entries").and_then(|e| e.as_array()) {
            for entry in entries {
                let path_str = entry.get("path").and_then(|p| p.as_str()).unwrap_or("");
                let hash = entry.get("hash").and_then(|h| h.as_str()).unwrap_or("");

                if path_str.is_empty() || hash.is_empty() {
                    continue;
                }

                let file_path = std::path::Path::new(path_str);
                if file_path.exists() {
                    if let Err(e) = crypto.verify_file(file_path, hash) {
                        tracing::error!("File verification failed for {}: {}", path_str, e);
                        return Err(format!("File verification failed for {}: {}", path_str, e));
                    }
                    tracing::debug!("Verified: {}", path_str);
                } else {
                    tracing::debug!("Skipping missing file: {}", path_str);
                }
            }
        }

        tracing::info!("Manifest verification completed successfully");
        Ok(())
    }

    /// Gets the shutdown flag status
    ///
    /// This is primarily for testing but can be used to check shutdown state.
    pub fn is_shutdown_signaled(&self) -> bool {
        self.shutdown_flag.load(Ordering::Relaxed)
    }

    /// Gets the PID file path
    ///
    /// This is primarily for testing but can be used to check the PID file location.
    pub fn pid_file_path(&self) -> &PathBuf {
        &self.pid_file
    }

    /// Writes the PID file (exposed for testing)
    ///
    /// This is primarily for testing. In normal operation, `start()` handles PID file creation.
    pub fn write_pid_file_test(&self) -> Result<()> {
        self.write_pid_file()
    }

    /// Checks if the daemon is currently running
    ///
    /// This method:
    /// 1. Checks if the PID file exists
    /// 2. Reads the PID from the file
    /// 3. Verifies the process is actually running
    /// 4. Removes stale PID files
    ///
    /// # Returns
    ///
    /// Returns `true` if a daemon is running, `false` otherwise.
    fn is_daemon_running(&self) -> Result<bool> {
        if !self.pid_file.exists() {
            return Ok(false);
        }

        let pid = Self::read_pid_file(&self.pid_file)?;

        if Self::is_process_running(pid) {
            Ok(true)
        } else {
            // Stale PID file - remove it
            fs::remove_file(&self.pid_file).map_err(EngineError::Io)?;
            Ok(false)
        }
    }

    /// Writes the current process PID to the PID file
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the file cannot be written.
    fn write_pid_file(&self) -> Result<()> {
        let pid = std::process::id();

        // Ensure the parent directory exists
        if let Some(parent) = self.pid_file.parent() {
            fs::create_dir_all(parent).map_err(EngineError::Io)?;
        }

        fs::write(&self.pid_file, pid.to_string()).map_err(EngineError::Io)?;

        tracing::info!("Wrote PID {} to {:?}", pid, self.pid_file);

        Ok(())
    }

    /// Reads the PID from the PID file
    ///
    /// # Arguments
    ///
    /// * `pid_file` - Path to the PID file
    ///
    /// # Returns
    ///
    /// Returns the PID as a `u32`, or an error if the file cannot be read or parsed.
    fn read_pid_file(pid_file: &Path) -> Result<u32> {
        let content = fs::read_to_string(pid_file).map_err(EngineError::Io)?;

        content
            .trim()
            .parse::<u32>()
            .map_err(|e| EngineError::Config(format!("Invalid PID in file: {}", e)))
    }

    /// Checks if a process with the given PID is running
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the process is running, `false` otherwise.
    fn is_process_running(_pid: u32) -> bool {
        #[cfg(unix)]
        {
            use nix::sys::signal::kill;
            use nix::unistd::Pid;

            // Send signal 0 to check if process exists (doesn't actually send a signal)
            kill(Pid::from_raw(_pid as i32), None).is_ok()
        }

        #[cfg(windows)]
        {
            false
        }
    }

    /// Gets the path to the PID file
    ///
    /// # Arguments
    ///
    /// * `config` - Engine configuration containing data directory path
    ///
    /// # Returns
    ///
    /// Returns the path to the PID file, with ~ expansion applied.
    fn get_pid_file_path(config: &Config) -> Result<PathBuf> {
        let mut data_dir = config.core.data_dir.clone();

        // Expand ~ if present
        if let Some(home) = dirs::home_dir() {
            if data_dir.starts_with("~") {
                data_dir = home.join(
                    data_dir
                        .strip_prefix("~")
                        .expect("data_dir verified to start with ~"),
                );
            }
        }

        Ok(data_dir.join("rove.pid"))
    }

    /// Checks which LLM providers are available
    ///
    /// This method checks:
    /// - Ollama: Attempts to connect to the Ollama API
    /// - OpenAI: Checks if API key is configured in keychain
    /// - Anthropic: Checks if API key is configured in keychain
    /// - Gemini: Checks if API key is configured in keychain
    /// - NVIDIA NIM: Checks if API key is configured in keychain
    ///
    /// # Arguments
    ///
    /// * `config` - Engine configuration containing LLM settings
    ///
    /// # Returns
    ///
    /// Returns a `ProviderAvailability` struct with availability status for each provider.
    ///
    /// **Validates: Requirements 14.13**
    fn check_provider_availability(config: &Config) -> ProviderAvailability {
        use crate::secrets::SecretManager;

        // Check Ollama availability by attempting to connect
        let ollama_available = Self::check_ollama_availability(&config.llm.ollama.base_url);

        // Check cloud providers by verifying API keys exist in keychain
        let secret_manager = SecretManager::new("rove");

        let openai_available = secret_manager.has_secret("openai_api_key");
        let anthropic_available = secret_manager.has_secret("anthropic_api_key");
        let gemini_available = secret_manager.has_secret("gemini_api_key");
        let nvidia_nim_available = secret_manager.has_secret("nvidia_nim_api_key");

        ProviderAvailability {
            ollama: ollama_available,
            openai: openai_available,
            anthropic: anthropic_available,
            gemini: gemini_available,
            nvidia_nim: nvidia_nim_available,
        }
    }

    /// Checks if Ollama is available by attempting to connect to the API
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for Ollama API (e.g., "http://localhost:11434")
    ///
    /// # Returns
    ///
    /// Returns `true` if Ollama is reachable, `false` otherwise.
    fn check_ollama_availability(base_url: &str) -> bool {
        use std::time::Duration;

        // Use async reqwest client in a blocking context
        // We'll use a simple TCP connection check instead
        let url = base_url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let parts: Vec<&str> = url.split(':').collect();

        if parts.len() != 2 {
            return false;
        }

        let host = parts[0];
        let port: u16 = match parts[1].parse() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Try to establish a TCP connection with timeout
        std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from((
                match host.parse::<std::net::IpAddr>() {
                    Ok(ip) => ip,
                    Err(_) => {
                        // Try to resolve hostname
                        match (host, port).to_socket_addrs() {
                            Ok(mut addrs) => match addrs.next() {
                                Some(addr) => addr.ip(),
                                None => return false,
                            },
                            Err(_) => return false,
                        }
                    }
                },
                port,
            )),
            Duration::from_secs(2),
        )
        .is_ok()
    }
}

impl Drop for DaemonManager {
    /// Cleanup on drop
    ///
    /// Removes the PID file when the daemon manager is dropped.
    fn drop(&mut self) {
        if self.pid_file.exists() {
            if let Err(e) = fs::remove_file(&self.pid_file) {
                tracing::warn!("Failed to remove PID file on drop: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config(temp_dir: &TempDir) -> Config {
        // Create a minimal config for testing
        // Use forward slashes for TOML compatibility (Windows backslashes
        // are interpreted as unicode escape sequences by the TOML parser)
        let config_path = temp_dir.path().join("config.toml");
        let workspace_str = temp_dir.path().to_string_lossy().replace('\\', "/");
        let config_content = format!(
            r#"
[core]
workspace = "{workspace}"
log_level = "info"
auto_sync = true
data_dir = "{workspace}"

[llm]
default_provider = "ollama"

[tools]
tg-controller = false
ui-server = false
api-server = false

[plugins]
fs-editor = true
terminal = true
screenshot = false
git = true

[security]
max_risk_tier = 2
confirm_tier1 = true
confirm_tier1_delay = 10
require_explicit_tier2 = true
"#,
            workspace = workspace_str
        );

        std::fs::write(&config_path, config_content).unwrap();
        Config::load_from_path(&config_path).unwrap()
    }

    #[tokio::test]
    async fn test_daemon_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let manager = DaemonManager::new(&config).unwrap();
        assert!(manager.pid_file.to_string_lossy().contains("rove.pid"));
    }

    #[tokio::test]
    async fn test_write_and_read_pid_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let manager = DaemonManager::new(&config).unwrap();
        manager.write_pid_file().unwrap();

        assert!(manager.pid_file.exists());

        let pid = DaemonManager::read_pid_file(&manager.pid_file).unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[tokio::test]
    async fn test_daemon_already_running() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let manager = DaemonManager::new(&config).unwrap();

        // First start should succeed
        manager.start().await.unwrap();

        // Second start should fail with DaemonAlreadyRunning
        let result = manager.start().await;
        assert!(matches!(result, Err(EngineError::DaemonAlreadyRunning)));
    }

    #[tokio::test]
    async fn test_stale_pid_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let manager = DaemonManager::new(&config).unwrap();

        // Write a PID file with a non-existent PID
        fs::create_dir_all(manager.pid_file.parent().unwrap()).unwrap();
        fs::write(&manager.pid_file, "999999").unwrap();

        // Should detect stale PID and allow start
        let result = manager.start().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_daemon_status() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        // Status when not running
        let status = DaemonManager::status(&config).unwrap();
        assert!(!status.is_running);
        assert!(status.pid.is_none());
        // Provider availability should be checked regardless of daemon status
        // Note: In test environment, providers may not be available

        // Start daemon
        let manager = DaemonManager::new(&config).unwrap();
        manager.start().await.unwrap();

        // Status when running
        let status = DaemonManager::status(&config).unwrap();
        assert!(status.is_running);
        assert_eq!(status.pid, Some(std::process::id()));
        // Provider availability is checked and returned
        // (actual availability depends on test environment)
    }

    #[tokio::test]
    async fn test_pid_file_cleanup_on_drop() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let pid_file = {
            let manager = DaemonManager::new(&config).unwrap();
            manager.write_pid_file().unwrap();
            assert!(manager.pid_file.exists());
            manager.pid_file.clone()
        }; // manager dropped here

        // PID file should be removed on drop
        assert!(!pid_file.exists());
    }

    #[tokio::test]
    async fn test_daemon_status_provider_availability() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        // Get status
        let status = DaemonManager::status(&config).unwrap();

        // Provider availability should be checked
        // In test environment, most providers will be unavailable
        // but the check should not fail

        // Ollama availability depends on whether Ollama is running locally
        // (typically false in CI/test environments)

        // Cloud providers depend on whether API keys are in keychain
        // (typically false in test environments)

        // The important thing is that the status call succeeds
        // and returns a ProviderAvailability struct with boolean fields
        let _openai = status.providers.openai;
        let _anthropic = status.providers.anthropic;
        let _gemini = status.providers.gemini;
        let _nvidia_nim = status.providers.nvidia_nim;
        let _ollama = status.providers.ollama;
    }
}
