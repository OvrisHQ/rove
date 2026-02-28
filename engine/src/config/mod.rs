//! Configuration management
//!
//! This module handles loading, validation, and management of the Rove configuration.
//! Configuration is stored in TOML format at ~/.rove/config.toml.
//!
//! # Configuration Sections
//!
//! - **core**: Workspace path, log level, data directory
//! - **llm**: LLM provider settings and preferences
//! - **tools**: Core tool enablement flags
//! - **plugins**: Plugin enablement flags
//! - **security**: Risk tier and confirmation settings
//! - **brains**: Brains configuration (optional)
//!
//! # Path Expansion
//!
//! The configuration system automatically:
//! - Expands ~ to the user's home directory
//! - Canonicalizes paths to resolve symlinks and .. patterns
//! - Verifies workspace is a directory
//! - Creates workspace directory if it doesn't exist
//!
//! # Platform-Specific Path Handling
//!
//! This module uses Rust's `std::path::Path` and `PathBuf` types, which automatically
//! handle platform-specific path separators (/ on Unix, \ on Windows). The `canonicalize()`
//! method resolves paths to their absolute form using the platform-specific separator.
//!
//! **Requirements**: 25.2 - Use platform-specific paths (/ on Unix, \ on Windows)
//!
//! # Examples
//!
//! ```no_run
//! use rove_engine::config::Config;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load configuration from default location
//! let config = Config::load_or_create()?;
//!
//! // Access configuration values
//! println!("Workspace: {:?}", config.core.workspace);
//! println!("Default provider: {}", config.llm.default_provider);
//! # Ok(())
//! # }
//! ```

use sdk::errors::EngineError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure
///
/// This structure represents the complete Rove configuration loaded from
/// ~/.rove/config.toml. All sections are required except where noted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Core engine settings
    pub core: CoreConfig,

    /// LLM provider configuration
    pub llm: LLMConfig,

    /// Core tool enablement
    pub tools: ToolsConfig,

    /// Plugin enablement
    pub plugins: PluginsConfig,

    /// Security settings
    pub security: SecurityConfig,

    /// Memory configuration
    #[serde(default)]
    pub memory: MemoryConfig,

    /// Brains configuration (optional)
    #[serde(default)]
    pub brains: BrainsConfig,

    /// Steering system configuration
    #[serde(default)]
    pub steering: SteeringConfig,

    /// WebSocket client configuration
    #[serde(default)]
    pub ws_client: WsClientConfig,
}

/// Core engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Workspace directory path (supports ~ expansion)
    pub workspace: PathBuf,

    /// Log level (error, warn, info, debug, trace)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Enable auto-sync
    #[serde(default = "default_true")]
    pub auto_sync: bool,

    /// Data directory path (supports ~ expansion)
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Default LLM provider (ollama, openai, anthropic, gemini, nvidia_nim)
    pub default_provider: String,

    /// Sensitivity threshold for local provider preference (0.0-1.0)
    #[serde(default = "default_sensitivity_threshold")]
    pub sensitivity_threshold: f64,

    /// Complexity threshold for cloud provider preference (0.0-1.0)
    #[serde(default = "default_complexity_threshold")]
    pub complexity_threshold: f64,

    /// Ollama provider settings
    #[serde(default)]
    pub ollama: OllamaConfig,

    /// OpenAI provider settings
    #[serde(default)]
    pub openai: OpenAIConfig,

    /// Anthropic provider settings
    #[serde(default)]
    pub anthropic: AnthropicConfig,

    /// Gemini provider settings
    #[serde(default)]
    pub gemini: GeminiConfig,

    /// NVIDIA NIM provider settings
    #[serde(default)]
    pub nvidia_nim: NvidiaNimConfig,
}

/// Ollama provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Base URL for Ollama API
    #[serde(default = "default_ollama_base_url")]
    pub base_url: String,

    /// Model name
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

/// OpenAI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// Base URL for OpenAI API
    #[serde(default = "default_openai_base_url")]
    pub base_url: String,

    /// Model name
    #[serde(default = "default_openai_model")]
    pub model: String,
    // Note: API key stored in OS keychain, not in config
}

/// Anthropic provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    /// Base URL for Anthropic API
    #[serde(default = "default_anthropic_base_url")]
    pub base_url: String,

    /// Model name
    #[serde(default = "default_anthropic_model")]
    pub model: String,
    // Note: API key stored in OS keychain, not in config
}

/// Gemini provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    /// Base URL for Gemini API
    #[serde(default = "default_gemini_base_url")]
    pub base_url: String,

    /// Model name
    #[serde(default = "default_gemini_model")]
    pub model: String,
    // Note: API key stored in OS keychain, not in config
}

/// NVIDIA NIM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaNimConfig {
    /// Base URL for NVIDIA NIM API
    #[serde(default = "default_nvidia_nim_base_url")]
    pub base_url: String,

    /// Model name
    #[serde(default = "default_nvidia_nim_model")]
    pub model: String,
    // Note: API key stored in OS keychain, not in config
}

/// Core tools enablement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// Enable Telegram bot controller
    #[serde(default, rename = "tg-controller")]
    pub tg_controller: bool,

    /// Enable UI server
    #[serde(default, rename = "ui-server")]
    pub ui_server: bool,

    /// Enable API server
    #[serde(default, rename = "api-server")]
    pub api_server: bool,
}

/// Plugins enablement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Enable file system editor plugin
    #[serde(default = "default_true", rename = "fs-editor")]
    pub fs_editor: bool,

    /// Enable terminal plugin
    #[serde(default = "default_true")]
    pub terminal: bool,

    /// Enable screenshot plugin
    #[serde(default)]
    pub screenshot: bool,

    /// Enable git plugin
    #[serde(default = "default_true")]
    pub git: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Maximum risk tier allowed (0, 1, or 2)
    #[serde(default = "default_max_risk_tier")]
    pub max_risk_tier: u8,

    /// Require confirmation for Tier 1 operations
    #[serde(default = "default_true")]
    pub confirm_tier1: bool,

    /// Countdown delay for Tier 1 operations (seconds)
    #[serde(default = "default_tier1_delay")]
    pub confirm_tier1_delay: u64,

    /// Require explicit confirmation for Tier 2 operations
    #[serde(default = "default_true")]
    pub require_explicit_tier2: bool,
}

/// Memory system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum tokens for short-term session memory
    #[serde(default = "default_max_session_tokens")]
    pub max_session_tokens: usize,

    /// Default number of days to keep episodic memories if active
    #[serde(default = "default_episodic_retention_days")]
    pub episodic_retention_days: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_session_tokens: default_max_session_tokens(),
            episodic_retention_days: default_episodic_retention_days(),
        }
    }
}

/// Brains configuration (optional)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrainsConfig {
    /// Enable brains feature
    #[serde(default)]
    pub enabled: bool,

    /// RAM limit in MB
    #[serde(default = "default_ram_limit")]
    pub ram_limit_mb: u64,

    /// Fallback provider when brains unavailable
    #[serde(default = "default_fallback_provider")]
    pub fallback: String,

    /// Auto-unload unused brains
    #[serde(default = "default_true")]
    pub auto_unload: bool,
}

/// Steering system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteeringConfig {
    /// Default skills always active
    #[serde(default)]
    pub default_skills: Vec<String>,

    /// Allow auto-activation based on task content
    #[serde(default = "default_true")]
    pub auto_detect: bool,

    /// Directory for steering skill files (supports ~ expansion)
    #[serde(default = "default_steering_dir")]
    pub skill_dir: PathBuf,
}

impl Default for SteeringConfig {
    fn default() -> Self {
        Self {
            default_skills: Vec::new(),
            auto_detect: true,
            skill_dir: default_steering_dir(),
        }
    }
}

/// WebSocket client configuration for connecting to external UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsClientConfig {
    /// Enable WebSocket client
    #[serde(default)]
    pub enabled: bool,

    /// WebSocket server URL to connect to
    #[serde(default = "default_ws_url")]
    pub url: String,

    /// Authentication token for the WebSocket connection
    #[serde(default)]
    pub auth_token: Option<String>,

    /// Delay in seconds before reconnecting after disconnect
    #[serde(default = "default_ws_reconnect_delay")]
    pub reconnect_delay_secs: u64,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: default_ws_url(),
            auth_token: None,
            reconnect_delay_secs: default_ws_reconnect_delay(),
        }
    }
}

// Default value functions
fn default_log_level() -> String {
    "info".to_string()
}

fn default_true() -> bool {
    true
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("~/.rove")
}

fn default_sensitivity_threshold() -> f64 {
    0.7
}

fn default_complexity_threshold() -> f64 {
    0.8
}

fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_openai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_anthropic_base_url() -> String {
    "https://api.anthropic.com/v1".to_string()
}

fn default_gemini_base_url() -> String {
    "https://generativelanguage.googleapis.com/v1beta".to_string()
}

fn default_ollama_model() -> String {
    "llama3.1:8b".to_string()
}

fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_anthropic_model() -> String {
    "claude-3-5-sonnet-20241022".to_string()
}

fn default_gemini_model() -> String {
    "gemini-1.5-pro".to_string()
}

fn default_nvidia_nim_base_url() -> String {
    "https://integrate.api.nvidia.com/v1".to_string()
}

fn default_nvidia_nim_model() -> String {
    "meta/llama-3.1-70b-instruct".to_string()
}

fn default_max_risk_tier() -> u8 {
    2
}

fn default_tier1_delay() -> u64 {
    10
}

fn default_ram_limit() -> u64 {
    512
}

fn default_fallback_provider() -> String {
    "openai".to_string()
}

fn default_max_session_tokens() -> usize {
    8192
}

fn default_steering_dir() -> PathBuf {
    PathBuf::from("~/.rove/steering")
}

fn default_episodic_retention_days() -> u32 {
    30
}

fn default_ws_url() -> String {
    "ws://localhost:9090/rove".to_string()
}

fn default_ws_reconnect_delay() -> u64 {
    5
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: default_ollama_base_url(),
            model: default_ollama_model(),
        }
    }
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            base_url: default_openai_base_url(),
            model: default_openai_model(),
        }
    }
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            base_url: default_anthropic_base_url(),
            model: default_anthropic_model(),
        }
    }
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            base_url: default_gemini_base_url(),
            model: default_gemini_model(),
        }
    }
}

impl Default for NvidiaNimConfig {
    fn default() -> Self {
        Self {
            base_url: default_nvidia_nim_base_url(),
            model: default_nvidia_nim_model(),
        }
    }
}

impl Config {
    /// Load configuration from the default location (~/.rove/config.toml)
    ///
    /// If the configuration file doesn't exist, creates a default configuration.
    /// Validates the configuration after loading and returns descriptive errors
    /// if validation fails.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration file cannot be read
    /// - TOML parsing fails
    /// - Validation fails (invalid paths, missing required fields)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rove_engine::config::Config;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::load_or_create()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_or_create() -> Result<Self, EngineError> {
        let config_path = Self::default_config_path()?;

        if config_path.exists() {
            Self::load_from_path(&config_path)
        } else {
            Self::create_default(&config_path)
        }
    }

    /// Load configuration from a specific path
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be read
    /// - TOML parsing fails
    /// - Validation fails
    pub fn load_from_path(path: &Path) -> Result<Self, EngineError> {
        let contents = fs::read_to_string(path)
            .map_err(|e| EngineError::Config(format!("Failed to read config file: {}", e)))?;

        let mut config: Config = toml::from_str(&contents)
            .map_err(|e| EngineError::Config(format!("Failed to parse config: {}", e)))?;

        // Validate and process configuration
        config.validate_and_process()?;

        Ok(config)
    }

    /// Create default configuration and save to path
    ///
    /// Creates the configuration directory if it doesn't exist, generates
    /// a default configuration, and saves it to the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Directory creation fails
    /// - File write fails
    /// - Path validation fails
    fn create_default(path: &Path) -> Result<Self, EngineError> {
        // Create config directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EngineError::Config(format!("Failed to create config directory: {}", e))
            })?;
        }

        // Create default configuration
        let mut config = Self::default_config();

        // Validate and process
        config.validate_and_process()?;

        // Serialize to TOML
        let toml_string = toml::to_string_pretty(&config)
            .map_err(|e| EngineError::Config(format!("Failed to serialize config: {}", e)))?;

        // Write to file
        fs::write(path, toml_string)
            .map_err(|e| EngineError::Config(format!("Failed to write config file: {}", e)))?;

        Ok(config)
    }

    /// Get the default configuration file path (~/.rove/config.toml)
    fn default_config_path() -> Result<PathBuf, EngineError> {
        let home = dirs::home_dir()
            .ok_or_else(|| EngineError::Config("Could not determine home directory".to_string()))?;

        Ok(home.join(".rove").join("config.toml"))
    }

    /// Create a default configuration
    fn default_config() -> Self {
        Self {
            core: CoreConfig {
                workspace: PathBuf::from("~/projects"),
                log_level: default_log_level(),
                auto_sync: true,
                data_dir: default_data_dir(),
            },
            llm: LLMConfig {
                default_provider: "ollama".to_string(),
                sensitivity_threshold: default_sensitivity_threshold(),
                complexity_threshold: default_complexity_threshold(),
                ollama: OllamaConfig::default(),
                openai: OpenAIConfig::default(),
                anthropic: AnthropicConfig::default(),
                gemini: GeminiConfig::default(),
                nvidia_nim: NvidiaNimConfig::default(),
            },
            tools: ToolsConfig {
                tg_controller: false,
                ui_server: false,
                api_server: false,
            },
            plugins: PluginsConfig {
                fs_editor: true,
                terminal: true,
                screenshot: false,
                git: true,
            },
            security: SecurityConfig {
                max_risk_tier: default_max_risk_tier(),
                confirm_tier1: true,
                confirm_tier1_delay: default_tier1_delay(),
                require_explicit_tier2: true,
            },
            memory: MemoryConfig::default(),
            brains: BrainsConfig::default(),
            steering: SteeringConfig::default(),
            ws_client: WsClientConfig::default(),
        }
    }

    /// Validate and process configuration
    ///
    /// This method:
    /// - Validates required fields
    /// - Expands ~ in paths
    /// - Canonicalizes paths
    /// - Verifies workspace is a directory
    /// - Creates workspace if it doesn't exist
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required fields are missing or invalid
    /// - Path expansion fails
    /// - Path canonicalization fails
    /// - Workspace creation fails
    fn validate_and_process(&mut self) -> Result<(), EngineError> {
        // Validate log level
        let valid_log_levels = ["error", "warn", "info", "debug", "trace"];
        if !valid_log_levels.contains(&self.core.log_level.as_str()) {
            return Err(EngineError::Config(format!(
                "Invalid log level '{}'. Must be one of: {}",
                self.core.log_level,
                valid_log_levels.join(", ")
            )));
        }

        // Validate default provider
        let valid_providers = ["ollama", "openai", "anthropic", "gemini", "nvidia_nim"];
        if !valid_providers.contains(&self.llm.default_provider.as_str()) {
            return Err(EngineError::Config(format!(
                "Invalid default provider '{}'. Must be one of: {}",
                self.llm.default_provider,
                valid_providers.join(", ")
            )));
        }

        // Validate thresholds
        if self.llm.sensitivity_threshold < 0.0 || self.llm.sensitivity_threshold > 1.0 {
            return Err(EngineError::Config(
                "sensitivity_threshold must be between 0.0 and 1.0".to_string(),
            ));
        }
        if self.llm.complexity_threshold < 0.0 || self.llm.complexity_threshold > 1.0 {
            return Err(EngineError::Config(
                "complexity_threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Validate max risk tier
        if self.security.max_risk_tier > 2 {
            return Err(EngineError::Config(
                "max_risk_tier must be 0, 1, or 2".to_string(),
            ));
        }

        // Expand and validate workspace path
        self.core.workspace = expand_path(&self.core.workspace)?;
        self.core.workspace = canonicalize_or_create(&self.core.workspace)?;

        // Verify workspace is a directory
        if !self.core.workspace.is_dir() {
            return Err(EngineError::Config(format!(
                "Workspace path is not a directory: {:?}",
                self.core.workspace
            )));
        }

        // Expand and validate data directory
        self.core.data_dir = expand_path(&self.core.data_dir)?;

        // Create data directory if it doesn't exist
        if !self.core.data_dir.exists() {
            fs::create_dir_all(&self.core.data_dir).map_err(|e| {
                EngineError::Config(format!("Failed to create data directory: {}", e))
            })?;
        }

        Ok(())
    }
}

/// Expand ~ in path to user's home directory
///
/// # Examples
///
/// ```ignore
/// let path = PathBuf::from("~/projects");
/// let expanded = expand_path(&path)?;
/// // expanded is now /home/user/projects (on Unix)
/// ```
fn expand_path(path: &Path) -> Result<PathBuf, EngineError> {
    let path_str = path
        .to_str()
        .ok_or_else(|| EngineError::Config("Invalid UTF-8 in path".to_string()))?;

    if let Some(rest) = path_str.strip_prefix("~/") {
        let home = dirs::home_dir()
            .ok_or_else(|| EngineError::Config("Could not determine home directory".to_string()))?;

        Ok(home.join(rest))
    } else if path_str == "~" {
        dirs::home_dir()
            .ok_or_else(|| EngineError::Config("Could not determine home directory".to_string()))
    } else {
        Ok(path.to_path_buf())
    }
}

/// Canonicalize path, creating it if it doesn't exist
///
/// This function attempts to canonicalize the path. If the path doesn't exist,
/// it creates the directory and then canonicalizes it.
fn canonicalize_or_create(path: &Path) -> Result<PathBuf, EngineError> {
    if path.exists() {
        path.canonicalize()
            .map_err(|e| EngineError::PathCanonicalization(path.to_path_buf(), e.to_string()))
    } else {
        // Create directory
        fs::create_dir_all(path).map_err(|e| {
            EngineError::Config(format!("Failed to create directory {:?}: {}", path, e))
        })?;

        // Now canonicalize
        path.canonicalize()
            .map_err(|e| EngineError::PathCanonicalization(path.to_path_buf(), e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_creation() {
        let config = Config::default_config();

        assert_eq!(config.core.log_level, "info");
        assert_eq!(config.llm.default_provider, "ollama");
        assert_eq!(config.security.max_risk_tier, 2);
        assert!(config.plugins.fs_editor);
        assert!(config.plugins.terminal);
        assert!(config.plugins.git);
    }

    #[test]
    fn test_expand_path_with_tilde() {
        let path = PathBuf::from("~/test");
        let expanded = expand_path(&path).unwrap();

        let home = dirs::home_dir().unwrap();
        assert_eq!(expanded, home.join("test"));
    }

    #[test]
    fn test_expand_path_without_tilde() {
        let path = PathBuf::from("/absolute/path");
        let expanded = expand_path(&path).unwrap();

        assert_eq!(expanded, path);
    }

    #[test]
    fn test_expand_path_tilde_only() {
        let path = PathBuf::from("~");
        let expanded = expand_path(&path).unwrap();

        let home = dirs::home_dir().unwrap();
        assert_eq!(expanded, home);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default_config();
        let toml_string = toml::to_string(&config).unwrap();

        // Verify it can be deserialized back
        let deserialized: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(config.core.log_level, deserialized.core.log_level);
        assert_eq!(
            config.llm.default_provider,
            deserialized.llm.default_provider
        );
    }
}
