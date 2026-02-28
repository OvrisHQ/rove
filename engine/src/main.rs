// Rove AI Agent Engine
// Main entry point for the Rove binary

use clap::Parser;
use rove_engine::agent::SteeringEngine;
use rove_engine::cli::{Cli, Command, PluginAction, SkillAction};
use rove_engine::config::Config;
use rove_engine::daemon::DaemonManager;
use rove_engine::handlers::{
    handle_doctor, handle_history, handle_plugins_list, handle_replay, handle_run, handle_update,
    OutputFormat,
};
use rove_engine::telemetry::{init_telemetry, init_telemetry_with_level};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize basic telemetry first (before config is loaded)
    init_telemetry();

    let version = env!("CARGO_PKG_VERSION");
    let commit = env!("GIT_COMMIT_HASH");
    let timestamp = env!("BUILD_TIMESTAMP");

    tracing::info!("Rove Engine v{} ({} - {})", version, commit, timestamp);

    // Determine output format
    let format = if cli.json {
        OutputFormat::Json
    } else {
        OutputFormat::Text
    };

    // Load configuration (or use custom path if provided)
    let config = if let Some(config_path) = &cli.config {
        Config::load_from_path(config_path)?
    } else {
        Config::load_or_create()?
    };

    // Re-initialize telemetry with config-driven log level
    // (only takes effect if RUST_LOG env var is not set)
    init_telemetry_with_level(&config.core.log_level);

    // Handle commands
    match cli.command {
        Command::Setup => {
            tracing::info!("Running setup wizard...");
            rove_engine::handlers::handle_setup().await
        }

        Command::Start => {
            tracing::info!("Starting daemon...");
            let manager = DaemonManager::new(&config)?;
            manager.start().await?;
            println!("Rove daemon started (PID {})", std::process::id());

            // Keep the process alive — wait for shutdown signal
            manager
                .wait_for_shutdown(std::time::Duration::from_secs(u64::MAX))
                .await
                .ok();
            Ok(())
        }

        Command::Stop => {
            tracing::info!("Stopping daemon...");
            DaemonManager::stop(&config).await?;
            println!("Rove daemon stopped.");
            Ok(())
        }

        Command::Status => {
            tracing::info!("Checking daemon status...");
            let status = DaemonManager::status(&config)?;
            if status.is_running {
                println!("Rove daemon is running (PID {})", status.pid.unwrap_or(0));
            } else {
                println!("Rove daemon is not running.");
            }
            println!("Providers:");
            println!(
                "  Ollama:     {}",
                if status.providers.ollama {
                    "available"
                } else {
                    "unavailable"
                }
            );
            println!(
                "  OpenAI:     {}",
                if status.providers.openai {
                    "available"
                } else {
                    "unavailable"
                }
            );
            println!(
                "  Anthropic:  {}",
                if status.providers.anthropic {
                    "available"
                } else {
                    "unavailable"
                }
            );
            println!(
                "  Gemini:     {}",
                if status.providers.gemini {
                    "available"
                } else {
                    "unavailable"
                }
            );
            println!(
                "  NVIDIA NIM: {}",
                if status.providers.nvidia_nim {
                    "available"
                } else {
                    "unavailable"
                }
            );
            Ok(())
        }

        Command::Run { task } => {
            tracing::info!("Executing task: {}", task);
            handle_run(task, &config, format).await
        }

        Command::History { limit } => {
            tracing::info!("Showing last {} tasks", limit);
            handle_history(limit, &config, format).await
        }

        Command::Replay { task_id } => {
            tracing::info!("Replaying task: {}", task_id);
            handle_replay(task_id, &config, format).await
        }

        Command::Plugins { action } => {
            tracing::info!("Plugin management: {:?}", action);
            match action {
                PluginAction::List => handle_plugins_list(&config, format).await,
                _ => {
                    println!("Plugin management actions (enable/disable/info) - to be implemented");
                    Ok(())
                }
            }
        }

        Command::Modules { action } => {
            tracing::info!("Module management: {:?}", action);
            println!("Module management - to be implemented");
            Ok(())
        }

        Command::Config { action } => {
            tracing::info!("Config management: {:?}", action);
            println!("Config management - to be implemented");
            Ok(())
        }

        Command::Doctor => {
            tracing::info!("Running diagnostics...");
            handle_doctor(&config, format).await
        }

        Command::Update { check } => {
            tracing::info!("Checking for updates...");
            handle_update(check, format).await
        }

        Command::Bot { action } => {
            tracing::info!("Bot management: {:?}", action);
            println!("Bot management - to be implemented");
            Ok(())
        }

        Command::Skill { action } => {
            tracing::info!("Skill management: {:?}", action);

            // Determine skills directory
            let skills_dir = {
                let dir = &config.steering.skill_dir;
                let dir_str = dir.to_str().unwrap_or("");
                if let Some(rest) = dir_str.strip_prefix("~/") {
                    dirs::home_dir()
                        .map(|h| h.join(rest))
                        .unwrap_or_else(|| dir.clone())
                } else {
                    dir.clone()
                }
            };

            match action {
                SkillAction::List { dir } => {
                    let dir = dir.unwrap_or(skills_dir);
                    let engine = SteeringEngine::new(&dir).await?;
                    let skills = engine.list_skills();

                    if skills.is_empty() {
                        println!("No skills found in {}", dir.display());
                        println!("Add .toml or .md skill files to this directory.");
                    } else {
                        println!("Available skills ({}):", dir.display());
                        println!();
                        for skill in &skills {
                            let kind = if skill.config.is_some() { "toml" } else { "md" };
                            let priority = skill
                                .config
                                .as_ref()
                                .map(|c| c.activation.priority)
                                .unwrap_or(0);
                            println!("  {} [{}] (priority: {})", skill.name, kind, priority);
                            if !skill.description.is_empty() {
                                println!("    {}", skill.description);
                            }
                        }
                        println!();
                        println!("{} skill(s) loaded.", skills.len());
                    }
                    Ok(())
                }

                SkillAction::Status => {
                    let engine = SteeringEngine::new(&skills_dir).await?;
                    let all = engine.list_skills();
                    let active = engine.active_skills();
                    let defaults = &config.steering.default_skills;

                    println!("Steering Status:");
                    println!("  Skills directory: {}", skills_dir.display());
                    println!("  Auto-detect:      {}", config.steering.auto_detect);
                    println!("  Total skills:     {}", all.len());
                    println!("  Active skills:    {}", active.len());

                    if !defaults.is_empty() {
                        println!("  Default skills:   {}", defaults.join(", "));
                    }

                    if active.is_empty() {
                        println!("\n  No skills currently active.");
                    } else {
                        println!("\n  Active:");
                        for id in active {
                            println!("    - {}", id);
                        }
                    }
                    Ok(())
                }

                SkillAction::On { name } => {
                    let mut engine = SteeringEngine::new(&skills_dir).await?;
                    match engine.activate(&name) {
                        Ok(()) => println!("Skill '{}' activated.", name),
                        Err(e) => println!("Failed to activate '{}': {}", name, e),
                    }
                    Ok(())
                }

                SkillAction::Off { name } => {
                    let mut engine = SteeringEngine::new(&skills_dir).await?;
                    engine.deactivate(&name);
                    println!("Skill '{}' deactivated.", name);
                    Ok(())
                }

                SkillAction::Add { name, description } => {
                    let desc = description.unwrap_or_else(|| format!("{} skill", name));
                    let template = format!(
                        r#"[meta]
id = "{name}"
name = "{name}"
description = "{desc}"
version = "1.0.0"
tags = []

[activation]
manual = true
priority = 50
conflicts_with = []

[directives]
system_prefix = ""
system_suffix = ""

[routing]
preferred_providers = []
avoid_providers = []
always_verify = false
"#
                    );

                    let file_path = skills_dir.join(format!("{}.toml", name));
                    if file_path.exists() {
                        println!("Skill '{}' already exists at {}", name, file_path.display());
                    } else {
                        tokio::fs::create_dir_all(&skills_dir).await?;
                        tokio::fs::write(&file_path, template).await?;
                        println!("Created skill '{}' at {}", name, file_path.display());
                    }
                    Ok(())
                }

                SkillAction::Edit { name } => {
                    let file_path = skills_dir.join(format!("{}.toml", name));
                    if !file_path.exists() {
                        // Try .md
                        let md_path = skills_dir.join(format!("{}.md", name));
                        if md_path.exists() {
                            println!("Opening {} in editor...", md_path.display());
                            let editor =
                                std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                            std::process::Command::new(editor).arg(&md_path).status()?;
                        } else {
                            println!(
                                "Skill '{}' not found. Use 'skill add {}' to create it.",
                                name, name
                            );
                        }
                    } else {
                        println!("Opening {} in editor...", file_path.display());
                        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                        std::process::Command::new(editor)
                            .arg(&file_path)
                            .status()?;
                    }
                    Ok(())
                }
            }
        }
    }
}
