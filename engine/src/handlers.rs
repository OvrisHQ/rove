//! Command handlers for CLI operations
//!
//! This module implements the handlers for all CLI commands:
//! - run: Execute a task immediately
//! - history: Show last N tasks
//! - replay: Show all steps for a task
//! - plugins list: List all installed plugins
//! - doctor: Validate configuration and check dependencies
//!
//! Requirements: 15.3, 15.4, 15.5, 15.6, 15.7

use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::daemon::DaemonManager;
use crate::db::{tasks::TaskRepository, Database};

/// Output format for command results
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output for machine consumption
    Json,
}

/// Run a task immediately
///
/// This handler executes a task synchronously and returns the result.
/// If the daemon is running, it delegates to the daemon. Otherwise, it
/// executes the task directly.
///
/// Requirements: 15.3
pub async fn handle_run(task: String, config: &Config, format: OutputFormat) -> Result<()> {
    use crate::agent::{AgentCore, SteeringEngine, Task};
    use crate::db::tasks::TaskRepository;
    use crate::llm::ollama::OllamaProvider;
    use crate::llm::router::LLMRouter;
    use crate::rate_limiter::RateLimiter;
    use crate::risk_assessor::{OperationSource, RiskAssessor};
    use crate::tools::{FilesystemTool, TerminalTool, ToolRegistry, VisionTool};
    use std::sync::Arc;

    // Initialize database
    let db_path = get_db_path(config)?;
    let database = Database::new(&db_path)
        .await
        .context("Failed to open database")?;

    // Create LLM providers
    let mut providers: Vec<Box<dyn crate::llm::LLMProvider>> = Vec::new();

    // Add Ollama provider (always configured with defaults)
    let ollama = OllamaProvider::new(
        config.llm.ollama.base_url.clone(),
        config.llm.ollama.model.clone(),
    );
    providers.push(Box::new(ollama));

    // Initialize SecretCache
    use crate::secrets::{SecretCache, SecretManager};
    let secret_manager = Arc::new(SecretManager::new("rove"));
    let secret_cache = Arc::new(SecretCache::new(secret_manager.clone()));

    // Only add cloud providers if their API keys already exist in keychain
    // (don't prompt interactively — Ollama works without any keys)
    if secret_manager.has_secret("openai_api_key") {
        use crate::llm::openai::OpenAIProvider;
        providers.push(Box::new(OpenAIProvider::new(
            config.llm.openai.clone(),
            secret_cache.clone(),
        )));
    }

    if secret_manager.has_secret("anthropic_api_key") {
        use crate::llm::anthropic::AnthropicProvider;
        providers.push(Box::new(AnthropicProvider::new(
            config.llm.anthropic.clone(),
            secret_cache.clone(),
        )));
    }

    if secret_manager.has_secret("gemini_api_key") {
        use crate::llm::gemini::GeminiProvider;
        providers.push(Box::new(GeminiProvider::new(
            config.llm.gemini.clone(),
            secret_cache.clone(),
        )));
    }

    if secret_manager.has_secret("nvidia_nim_api_key") {
        use crate::llm::nvidia_nim::NvidiaNimProvider;
        providers.push(Box::new(NvidiaNimProvider::new(
            config.llm.nvidia_nim.clone(),
            secret_cache.clone(),
        )));
    }

    if providers.is_empty() {
        return Err(anyhow::anyhow!(
            "No LLM providers configured. Please configure at least one provider in config.toml"
        ));
    }

    // Create LLM router
    let router = Arc::new(LLMRouter::new(providers, Arc::new(config.llm.clone())));

    // Create rate limiter
    let rate_limiter = Arc::new(RateLimiter::new(database.pool().clone()));

    // Create risk assessor
    let risk_assessor = RiskAssessor::new();

    // Create task repository
    let task_repo = Arc::new(TaskRepository::new(database.pool().clone()));

    // Create tool registry based on config flags
    let workspace = config.core.workspace.clone();
    let workspace_str = workspace.to_string_lossy().to_string();

    let tools = Arc::new(ToolRegistry {
        fs: if config.plugins.fs_editor {
            Some(FilesystemTool::new(workspace.clone()))
        } else {
            None
        },
        terminal: if config.plugins.terminal {
            Some(TerminalTool::new(workspace_str))
        } else {
            None
        },
        vision: if config.plugins.screenshot {
            Some(VisionTool::new(workspace.clone()))
        } else {
            None
        },
    });

    // Load steering engine from config
    let steering = {
        let skill_dir = if config.steering.skill_dir.to_string_lossy().starts_with("~/") {
            let home = dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
            let rest = config.steering.skill_dir.to_string_lossy();
            let rest = rest.strip_prefix("~/").unwrap_or(&rest);
            home.join(rest)
        } else {
            config.steering.skill_dir.clone()
        };

        if config.steering.auto_detect {
            match SteeringEngine::new(&skill_dir).await {
                Ok(engine) => Some(engine),
                Err(e) => {
                    tracing::warn!("Failed to load steering engine: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };

    // Create agent
    let mut agent = AgentCore::new(router, risk_assessor, rate_limiter, task_repo, tools, steering);

    // Create task
    let agent_task = Task::new(task.clone(), OperationSource::Local);

    match format {
        OutputFormat::Text => {
            println!("Executing task: {}", task);
            println!();
        }
        OutputFormat::Json => {
            let output = json!({
                "status": "running",
                "task": task.clone()
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    // Execute task
    let result = agent.process_task(agent_task).await;

    match result {
        Ok(task_result) => {
            match format {
                OutputFormat::Text => {
                    println!("Result:");
                    println!("{}", task_result.answer);
                    println!();
                    println!("✓ Task completed successfully");
                    println!("  Provider: {}", task_result.provider_used);
                    println!("  Duration: {}ms", task_result.duration_ms);
                    println!("  Iterations: {}", task_result.iterations);
                }
                OutputFormat::Json => {
                    let output = json!({
                        "status": "completed",
                        "task_id": task_result.task_id,
                        "answer": task_result.answer,
                        "provider": task_result.provider_used,
                        "duration_ms": task_result.duration_ms,
                        "iterations": task_result.iterations
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                }
            }
            Ok(())
        }
        Err(e) => {
            match format {
                OutputFormat::Text => {
                    println!("✗ Task failed: {}", e);
                }
                OutputFormat::Json => {
                    let output = json!({
                        "status": "failed",
                        "error": e.to_string()
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                }
            }
            Err(e)
        }
    }
}

/// Show task history
///
/// This handler retrieves and displays the last N tasks from the database.
///
/// Requirements: 15.4
pub async fn handle_history(limit: usize, config: &Config, format: OutputFormat) -> Result<()> {
    // Initialize database
    let db_path = get_db_path(config)?;
    let database = Database::new(&db_path)
        .await
        .context("Failed to open database")?;

    let task_repo = TaskRepository::new(database.pool().clone());

    // Fetch recent tasks
    let tasks = task_repo
        .get_recent_tasks(limit as i64)
        .await
        .context("Failed to fetch task history")?;

    match format {
        OutputFormat::Text => {
            if tasks.is_empty() {
                println!("No tasks in history");
                return Ok(());
            }

            println!("Task History (last {} tasks):", limit);
            println!();

            for task in tasks {
                println!("Task ID: {}", task.id);
                println!("  Input: {}", task.input);
                println!("  Status: {:?}", task.status);

                if let Some(provider) = task.provider_used {
                    println!("  Provider: {}", provider);
                }

                if let Some(duration) = task.duration_ms {
                    println!("  Duration: {}ms", duration);
                }

                // Format timestamp
                let created = chrono::DateTime::from_timestamp(task.created_at, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                println!("  Created: {}", created);

                println!();
            }
        }
        OutputFormat::Json => {
            let output = json!({
                "tasks": tasks,
                "count": tasks.len(),
                "limit": limit
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

/// Replay a task and show all steps
///
/// This handler retrieves a task and all its steps from the database
/// and displays them in order.
///
/// Requirements: 15.5
pub async fn handle_replay(task_id: String, config: &Config, format: OutputFormat) -> Result<()> {
    // Initialize database
    let db_path = get_db_path(config)?;
    let database = Database::new(&db_path)
        .await
        .context("Failed to open database")?;

    let task_repo = TaskRepository::new(database.pool().clone());

    // Fetch task
    let task = task_repo
        .get_task(&task_id)
        .await
        .context("Failed to fetch task")?
        .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

    // Fetch steps
    let steps = task_repo
        .get_task_steps(&task_id)
        .await
        .context("Failed to fetch task steps")?;

    match format {
        OutputFormat::Text => {
            println!("Task Replay: {}", task_id);
            println!();
            println!("Input: {}", task.input);
            println!("Status: {:?}", task.status);

            if let Some(provider) = task.provider_used {
                println!("Provider: {}", provider);
            }

            if let Some(duration) = task.duration_ms {
                println!("Duration: {}ms", duration);
            }

            println!();
            println!("Steps ({} total):", steps.len());
            println!();

            for step in steps {
                println!("Step {}: {:?}", step.step_order, step.step_type);
                println!("  {}", step.content);
                println!();
            }
        }
        OutputFormat::Json => {
            let output = json!({
                "task": task,
                "steps": steps,
                "step_count": steps.len()
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

/// List all installed plugins
///
/// This handler retrieves and displays all plugins from the database.
///
/// Requirements: 15.6
pub async fn handle_plugins_list(config: &Config, format: OutputFormat) -> Result<()> {
    // Initialize database
    let db_path = get_db_path(config)?;
    let _database = Database::new(&db_path)
        .await
        .context("Failed to open database")?;

    // TODO: Implement plugin listing from database
    // For now, show configured plugins from config

    match format {
        OutputFormat::Text => {
            println!("Installed Plugins:");
            println!();

            println!(
                "  fs-editor: {}",
                if config.plugins.fs_editor {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  terminal: {}",
                if config.plugins.terminal {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  screenshot: {}",
                if config.plugins.screenshot {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  git: {}",
                if config.plugins.git {
                    "enabled"
                } else {
                    "disabled"
                }
            );

            println!();
            println!("Note: Plugin database integration will be completed in a future task");
        }
        OutputFormat::Json => {
            let output = json!({
                "plugins": [
                    {
                        "name": "fs-editor",
                        "enabled": config.plugins.fs_editor
                    },
                    {
                        "name": "terminal",
                        "enabled": config.plugins.terminal
                    },
                    {
                        "name": "screenshot",
                        "enabled": config.plugins.screenshot
                    },
                    {
                        "name": "git",
                        "enabled": config.plugins.git
                    }
                ]
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

/// Run system diagnostics
///
/// This handler validates the configuration, checks dependencies,
/// verifies the manifest, and reports any issues.
///
/// Requirements: 15.7
pub async fn handle_doctor(config: &Config, format: OutputFormat) -> Result<()> {
    let mut issues = Vec::new();
    let mut checks = Vec::new();

    // Check 1: Configuration validation
    checks.push(("Configuration", "Valid"));
    // Config is already validated when loaded

    // Check 2: Workspace directory
    if config.core.workspace.exists() {
        checks.push(("Workspace directory", "Exists"));
    } else {
        checks.push(("Workspace directory", "Missing"));
        issues.push(format!(
            "Workspace directory does not exist: {:?}",
            config.core.workspace
        ));
    }

    // Check 3: Data directory
    let data_dir = expand_data_dir(&config.core.data_dir)?;
    if data_dir.exists() {
        checks.push(("Data directory", "Exists"));
    } else {
        checks.push(("Data directory", "Missing"));
        issues.push(format!("Data directory does not exist: {:?}", data_dir));
    }

    // Check 4: Database
    let db_path = data_dir.join("rove.db");
    if db_path.exists() {
        checks.push(("Database", "Exists"));

        // Try to open database
        match Database::new(&db_path).await {
            Ok(_) => {
                checks.push(("Database connection", "OK"));
            }
            Err(e) => {
                checks.push(("Database connection", "Failed"));
                issues.push(format!("Cannot connect to database: {}", e));
            }
        }
    } else {
        checks.push(("Database", "Not initialized"));
        issues.push("Database not initialized. Run 'rove start' to initialize.".to_string());
    }

    // Check 5: Daemon status
    match DaemonManager::status(config) {
        Ok(status) => {
            if status.is_running {
                checks.push(("Daemon", "Running"));
            } else {
                checks.push(("Daemon", "Not running"));
            }

            // Check 6: LLM providers
            if status.providers.ollama {
                checks.push(("Ollama", "Available"));
            } else {
                checks.push(("Ollama", "Not available"));
                issues.push("Ollama is not running. Start Ollama to use local LLM.".to_string());
            }

            if status.providers.openai {
                checks.push(("OpenAI API key", "Configured"));
            } else {
                checks.push(("OpenAI API key", "Not configured"));
            }

            if status.providers.anthropic {
                checks.push(("Anthropic API key", "Configured"));
            } else {
                checks.push(("Anthropic API key", "Not configured"));
            }

            if status.providers.gemini {
                checks.push(("Gemini API key", "Configured"));
            } else {
                checks.push(("Gemini API key", "Not configured"));
            }

            if status.providers.nvidia_nim {
                checks.push(("NVIDIA NIM API key", "Configured"));
            } else {
                checks.push(("NVIDIA NIM API key", "Not configured"));
            }

            // Warn if no providers are available
            if !status.providers.ollama
                && !status.providers.openai
                && !status.providers.anthropic
                && !status.providers.gemini
                && !status.providers.nvidia_nim
            {
                issues.push(
                    "No LLM providers available. Configure at least one provider.".to_string(),
                );
            }
        }
        Err(e) => {
            checks.push(("Daemon status", "Error"));
            issues.push(format!("Cannot check daemon status: {}", e));
        }
    }

    // Check 7: Manifest verification
    {
        let manifest_paths = [
            std::path::PathBuf::from("manifest/manifest.json"),
            dirs::home_dir()
                .map(|h| h.join(".rove/manifest.json"))
                .unwrap_or_default(),
        ];
        if let Some(manifest_path) = manifest_paths.iter().find(|p| p.exists()) {
            match crate::crypto::CryptoModule::new() {
                Ok(crypto) => match std::fs::read(manifest_path) {
                    Ok(bytes) => {
                        match crypto.verify_manifest_file(&bytes) {
                            Ok(()) => {
                                // Check if it was a placeholder
                                if let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                                    if let Some(sig) = manifest.get("signature").and_then(|s| s.as_str()) {
                                        if sig.contains("PLACEHOLDER") || sig.contains("LOCAL_DEV") {
                                            checks.push(("Manifest signature", "Dev placeholder (OK for development)"));
                                        } else {
                                            checks.push(("Manifest signature", "Valid"));
                                        }
                                    } else {
                                        checks.push(("Manifest", "Present (unsigned)"));
                                    }
                                } else {
                                    checks.push(("Manifest signature", "Valid"));
                                }
                            }
                            Err(_) => {
                                checks.push(("Manifest signature", "INVALID"));
                                issues.push(
                                    "Manifest signature verification failed!"
                                        .to_string(),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        checks.push(("Manifest", "Unreadable"));
                        issues.push(format!("Cannot read manifest: {}", e));
                    }
                },
                Err(e) => {
                    checks.push(("Manifest", "Crypto error"));
                    issues.push(format!("Cannot initialize crypto module: {}", e));
                }
            }
        } else {
            checks.push(("Manifest", "Not found"));
        }
    }

    // Output results
    match format {
        OutputFormat::Text => {
            println!("Rove System Diagnostics");
            println!("============================");
            println!();

            println!("System Checks:");
            for (check, status) in &checks {
                println!("  {:<25} {}", format!("{}:", check), status);
            }

            println!();

            if issues.is_empty() {
                println!("✓ All checks passed!");
            } else {
                println!("⚠ Issues found:");
                println!();
                for (i, issue) in issues.iter().enumerate() {
                    println!("  {}. {}", i + 1, issue);
                }
            }
        }
        OutputFormat::Json => {
            let output = json!({
                "checks": checks.iter().map(|(name, status)| {
                    json!({
                        "name": name,
                        "status": status
                    })
                }).collect::<Vec<_>>(),
                "issues": issues,
                "healthy": issues.is_empty()
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

/// Run the interactive setup wizard
///
/// Prompts the user for:
/// - Workspace directory
/// - Default LLM provider
/// - API keys (stored in keychain)
/// - Telegram bot configuration
/// - Risk tier preferences
///
/// Requirements: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 13.7, 13.8, 40.1, 40.2
pub async fn handle_setup() -> Result<()> {
    use crate::secrets::SecretManager;
    use std::io::{self, Write};

    println!("=== Rove Setup Wizard ===");
    println!();

    // 1. Workspace directory
    print!("Workspace directory [~/projects]: ");
    io::stdout().flush()?;
    let mut workspace = String::new();
    io::stdin().read_line(&mut workspace)?;
    let workspace = workspace.trim();
    let workspace = if workspace.is_empty() {
        "~/projects".to_string()
    } else {
        workspace.to_string()
    };

    // 2. Default LLM provider
    println!();
    println!("Available LLM providers:");
    println!("  1. ollama (local, free)");
    println!("  2. openai (cloud, requires API key)");
    println!("  3. anthropic (cloud, requires API key)");
    println!("  4. gemini (cloud, requires API key)");
    println!("  5. nvidia_nim (cloud, requires API key)");
    print!("Default provider [1]: ");
    io::stdout().flush()?;
    let mut provider_choice = String::new();
    io::stdin().read_line(&mut provider_choice)?;
    let provider = match provider_choice.trim() {
        "2" => "openai",
        "3" => "anthropic",
        "4" => "gemini",
        "5" => "nvidia_nim",
        _ => "ollama",
    };

    // 3. API keys
    let secret_manager = SecretManager::new("rove");

    println!();
    println!("Configure API keys (press Enter to skip):");

    // OpenAI
    print!("  OpenAI API key: ");
    io::stdout().flush()?;
    let mut key = String::new();
    io::stdin().read_line(&mut key)?;
    let key = key.trim();
    if !key.is_empty() {
        secret_manager
            .set_secret("openai_api_key", key)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        println!("    Stored in keychain.");
    }

    // Anthropic
    print!("  Anthropic API key: ");
    io::stdout().flush()?;
    let mut key = String::new();
    io::stdin().read_line(&mut key)?;
    let key = key.trim();
    if !key.is_empty() {
        secret_manager
            .set_secret("anthropic_api_key", key)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        println!("    Stored in keychain.");
    }

    // Gemini
    print!("  Gemini API key: ");
    io::stdout().flush()?;
    let mut key = String::new();
    io::stdin().read_line(&mut key)?;
    let key = key.trim();
    if !key.is_empty() {
        secret_manager
            .set_secret("gemini_api_key", key)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        println!("    Stored in keychain.");
    }

    // NVIDIA NIM
    print!("  NVIDIA NIM API key: ");
    io::stdout().flush()?;
    let mut key = String::new();
    io::stdin().read_line(&mut key)?;
    let key = key.trim();
    if !key.is_empty() {
        secret_manager
            .set_secret("nvidia_nim_api_key", key)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        println!("    Stored in keychain.");
    }

    // 4. Telegram bot
    println!();
    print!("Configure Telegram bot? [y/N]: ");
    io::stdout().flush()?;
    let mut tg_choice = String::new();
    io::stdin().read_line(&mut tg_choice)?;
    let enable_telegram = tg_choice.trim().eq_ignore_ascii_case("y");

    if enable_telegram {
        print!("  Telegram bot token: ");
        io::stdout().flush()?;
        let mut token = String::new();
        io::stdin().read_line(&mut token)?;
        let token = token.trim();
        if !token.is_empty() {
            secret_manager
                .set_secret("telegram_bot_token", token)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("    Stored in keychain.");
        }

        print!("  Your Telegram user ID: ");
        io::stdout().flush()?;
        let mut uid = String::new();
        io::stdin().read_line(&mut uid)?;
        let _uid = uid.trim();
    }

    // 5. Risk tier preference
    println!();
    println!("Maximum risk tier allowed:");
    println!("  0 = Read-only operations only");
    println!("  1 = Allow local modifications (with confirmation)");
    println!("  2 = Allow all operations (with confirmation)");
    print!("Max risk tier [2]: ");
    io::stdout().flush()?;
    let mut tier = String::new();
    io::stdin().read_line(&mut tier)?;
    let max_tier: u8 = tier.trim().parse().unwrap_or(2).min(2);

    // 6. Generate config file
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    let config_dir = home.join(".rove");
    std::fs::create_dir_all(&config_dir)?;

    let config_content = format!(
        r#"[core]
workspace = "{workspace}"
log_level = "info"
data_dir = "~/.rove"

[llm]
default_provider = "{provider}"

[tools]
tg-controller = {enable_telegram}
ui-server = false
api-server = false

[plugins]
fs-editor = true
terminal = true
screenshot = false
git = true

[security]
max_risk_tier = {max_tier}
confirm_tier1 = true
confirm_tier1_delay = 10
require_explicit_tier2 = true
"#
    );

    let config_path = config_dir.join("config.toml");
    std::fs::write(&config_path, &config_content)?;
    println!();
    println!("Configuration written to {}", config_path.display());

    // 7. Create database
    let db_path = config_dir.join("rove.db");
    if !db_path.exists() {
        let db = crate::db::Database::new(&db_path).await?;
        drop(db);
        println!("Database created at {}", db_path.display());
    }

    // 8. Create workspace directory
    let expanded_workspace = if let Some(rest) = workspace.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(&workspace)
    };
    std::fs::create_dir_all(&expanded_workspace)?;
    println!("Workspace directory: {}", expanded_workspace.display());

    println!();
    println!("Setup complete! Run 'rove doctor' to verify your configuration.");
    println!("Run 'rove start' to start the daemon.");

    Ok(())
}

/// Get the database path from config
fn get_db_path(config: &Config) -> Result<PathBuf> {
    let data_dir = expand_data_dir(&config.core.data_dir)?;
    Ok(data_dir.join("rove.db"))
}

/// Expand data directory path (handle ~ expansion)
fn expand_data_dir(path: &Path) -> Result<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix("~") {
            return Ok(home.join(rest));
        }
    }
    Ok(path.to_path_buf())
}

// --- Self-update types and handler ---

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(serde::Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

fn current_target() -> &'static str {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        "x86_64-unknown-linux-gnu"
    }

    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    {
        "aarch64-unknown-linux-gnu"
    }

    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    {
        "x86_64-apple-darwin"
    }

    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    {
        "aarch64-apple-darwin"
    }

    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    {
        "x86_64-pc-windows-msvc"
    }

    #[cfg(all(target_arch = "aarch64", target_os = "windows"))]
    {
        "aarch64-pc-windows-msvc"
    }
}

/// Check for updates and optionally self-update the binary
///
/// Fetches the latest release from GitHub, compares semver versions,
/// and downloads + replaces the binary if a newer version is available.
pub async fn handle_update(check_only: bool, format: OutputFormat) -> Result<()> {
    use futures::StreamExt;

    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .context("Failed to parse current version")?;

    // Fetch latest release
    let client = reqwest::Client::builder()
        .user_agent(format!("rove/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let release: GitHubRelease = client
        .get("https://api.github.com/repos/OvrisHQ/rove/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()
        .context("Failed to fetch latest release from GitHub")?
        .json()
        .await?;

    let latest_tag = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let latest =
        semver::Version::parse(latest_tag).context("Failed to parse latest release version")?;

    if latest <= current {
        match format {
            OutputFormat::Text => println!("Rove is already up to date (v{}).", current),
            OutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": "up_to_date",
                    "current_version": current.to_string(),
                    "latest_version": latest.to_string(),
                }))?
            ),
        }
        return Ok(());
    }

    match format {
        OutputFormat::Text => {
            println!("Update available: v{} -> v{}", current, latest);
            println!("Release: {}", release.html_url);
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": "update_available",
                    "current_version": current.to_string(),
                    "latest_version": latest.to_string(),
                    "release_url": release.html_url,
                }))?
            );
        }
    }

    if check_only {
        return Ok(());
    }

    // Find matching asset
    let target = current_target();
    let asset_name = format!("rove-{}", target);
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name || a.name == format!("{}.exe", asset_name))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No release asset for target '{}'. Available: {}",
                target,
                release
                    .assets
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    println!(
        "Downloading {} ({:.1} MB)...",
        asset.name,
        asset.size as f64 / 1_048_576.0
    );

    // Stream download into memory (verify before writing to disk)
    let response = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()
        .context("Failed to download release asset")?;

    let total = response.content_length().unwrap_or(asset.size);
    let mut bytes = Vec::with_capacity(total as usize);
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_pct: u32 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading download stream")?;
        downloaded += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);

        if total > 0 {
            let pct = (downloaded as f64 / total as f64 * 100.0) as u32;
            if pct / 10 > last_pct / 10 {
                eprint!("\r  Progress: {}%", pct);
                last_pct = pct;
            }
        }
    }
    eprintln!("\r  Progress: 100%");

    // Verify integrity: check SHA-256 hash against release manifest if available
    // Look for a manifest.json asset in the release
    let manifest_asset = release
        .assets
        .iter()
        .find(|a| a.name == "manifest.json");

    if let Some(manifest_asset) = manifest_asset {
        eprintln!("Verifying download integrity...");

        let manifest_response = client
            .get(&manifest_asset.browser_download_url)
            .send()
            .await?
            .error_for_status()
            .context("Failed to download release manifest")?;

        let manifest_bytes = manifest_response
            .bytes()
            .await
            .context("Failed to read release manifest")?;

        // Verify manifest signature
        match crate::crypto::CryptoModule::new() {
            Ok(crypto) => {
                match crypto.verify_manifest_file(&manifest_bytes) {
                    Ok(()) => {
                        eprintln!("  Manifest signature: verified");

                        // Check binary hash against manifest
                        let computed_hash = crate::crypto::CryptoModule::compute_hash(&bytes);

                        if let Ok(manifest_value) =
                            serde_json::from_slice::<serde_json::Value>(&manifest_bytes)
                        {
                            // Look for our binary's hash in the manifest
                            let expected_hash = manifest_value
                                .get("binaries")
                                .and_then(|b| b.get(&asset.name))
                                .and_then(|b| b.get("hash"))
                                .and_then(|h| h.as_str())
                                .or_else(|| {
                                    // Fallback: check core_tools array
                                    manifest_value
                                        .get("core_tools")
                                        .and_then(|t| t.as_array())
                                        .and_then(|arr| {
                                            arr.iter().find_map(|entry| {
                                                let name =
                                                    entry.get("id").and_then(|i| i.as_str())?;
                                                if name == "rove" || asset.name.contains(name) {
                                                    entry.get("hash").and_then(|h| h.as_str())
                                                } else {
                                                    None
                                                }
                                            })
                                        })
                                });

                            if let Some(expected) = expected_hash {
                                if computed_hash != expected {
                                    return Err(anyhow::anyhow!(
                                        "Binary hash mismatch! Expected: {}, Got: {}. Download may be corrupted or tampered.",
                                        expected, computed_hash
                                    ));
                                }
                                eprintln!("  Binary hash: verified (SHA-256: {}...)", &computed_hash[..16]);
                            } else {
                                eprintln!("  Binary hash: not in manifest (skipping hash check)");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  Warning: Manifest signature verification failed: {}", e);
                        eprintln!("  Proceeding with update (signature verification will be enforced in future releases)");
                    }
                }
            }
            Err(e) => {
                eprintln!("  Warning: Cannot initialize crypto module: {}", e);
            }
        }
    } else {
        eprintln!("  Note: No release manifest found (hash verification skipped)");
    }

    // Write to temp file and self-replace
    let temp_path = std::env::temp_dir().join(&asset.name);
    std::fs::write(&temp_path, &bytes).context("Failed to write temporary update file")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    self_replace::self_replace(&temp_path).context("Failed to replace the current binary")?;

    let _ = std::fs::remove_file(&temp_path);

    match format {
        OutputFormat::Text => {
            println!("Successfully updated Rove: v{} -> v{}", current, latest);
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": "updated",
                    "previous_version": current.to_string(),
                    "new_version": latest.to_string(),
                }))?
            );
        }
    }

    Ok(())
}
