//! CLI interface for Rove
//!
//! This module provides the command-line interface using clap's derive API.
//! It defines all commands and global flags for controlling the Rove daemon.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Rove AI Agent Engine
///
/// A local-first AI agent that runs on your machine, controls your system through
/// sandboxed plugins, and communicates with LLM providers.
#[derive(Parser, Debug)]
#[command(name = "rove")]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Set log level (error, warn, info, debug, trace)
    #[arg(long, global = true, value_name = "LEVEL")]
    pub log: Option<String>,

    /// Specify alternate configuration file
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run interactive setup wizard
    Setup,

    /// Start the daemon in the background
    Start,

    /// Stop the running daemon
    Stop,

    /// Show daemon status and provider availability
    Status,

    /// Execute a task immediately
    Run {
        /// The task to execute
        task: String,
    },

    /// Show task history
    History {
        /// Number of tasks to show (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Replay and show all steps for a task
    Replay {
        /// Task ID to replay
        task_id: String,
    },

    /// Manage plugins
    Plugins {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Manage core modules
    Modules {
        #[command(subcommand)]
        action: ModuleAction,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Run system diagnostics
    Doctor,

    /// Update Rove to the latest version
    Update {
        /// Only check if an update is available, do not download
        #[arg(long)]
        check: bool,
    },

    /// Manage Telegram bot
    Bot {
        #[command(subcommand)]
        action: BotAction,
    },

    /// Manage Agent Skills (Steering)
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
}

/// Agent Skill management actions
#[derive(Subcommand, Debug)]
pub enum SkillAction {
    /// List all loaded Agent Skills
    List {
        /// Optional skills directory path override
        #[arg(long)]
        dir: Option<PathBuf>,
    },

    /// Show active steering skills
    Status,

    /// Activate a steering skill
    On {
        /// Skill name to activate
        name: String,
    },

    /// Deactivate a steering skill
    Off {
        /// Skill name to deactivate
        name: String,
    },

    /// Create a new empty skill
    Add {
        /// Name of the new skill
        name: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Edit an existing skill in the default editor
    Edit {
        /// Name of the skill to edit
        name: String,
    },
}

/// Plugin management actions
#[derive(Subcommand, Debug)]
pub enum PluginAction {
    /// List all installed plugins
    List,

    /// Enable a plugin
    Enable {
        /// Plugin name
        name: String,
    },

    /// Disable a plugin
    Disable {
        /// Plugin name
        name: String,
    },

    /// Show plugin details
    Info {
        /// Plugin name
        name: String,
    },
}

/// Core module management actions
#[derive(Subcommand, Debug)]
pub enum ModuleAction {
    /// List all core modules
    List,

    /// Enable a core module
    Enable {
        /// Module name
        name: String,
    },

    /// Disable a core module
    Disable {
        /// Module name
        name: String,
    },

    /// Show module details
    Info {
        /// Module name
        name: String,
    },
}

/// Configuration management actions
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show,

    /// Get a configuration value
    Get {
        /// Configuration key (e.g., "core.workspace")
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., "core.workspace")
        key: String,
        /// Configuration value
        value: String,
    },

    /// Edit configuration file in default editor
    Edit,

    /// Validate configuration file
    Validate,
}

/// Telegram bot management actions
#[derive(Subcommand, Debug)]
pub enum BotAction {
    /// Start the Telegram bot
    Start,

    /// Stop the Telegram bot
    Stop,

    /// Show bot status
    Status,

    /// Set bot token
    SetToken {
        /// Bot token from @BotFather
        token: String,
    },

    /// Add allowed user ID
    AddUser {
        /// Telegram user ID
        user_id: i64,
    },

    /// Remove allowed user ID
    RemoveUser {
        /// Telegram user ID
        user_id: i64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test basic command parsing
        let cli = Cli::parse_from(["rove", "status"]);
        assert!(matches!(cli.command, Command::Status));
        assert!(!cli.json);
        assert!(cli.log.is_none());
        assert!(cli.config.is_none());
    }

    #[test]
    fn test_global_flags() {
        // Test global flags
        let cli = Cli::parse_from(["rove", "--json", "--log", "debug", "status"]);
        assert!(cli.json);
        assert_eq!(cli.log, Some("debug".to_string()));
    }

    #[test]
    fn test_run_command() {
        // Test run command with task
        let cli = Cli::parse_from(["rove", "run", "list files in current directory"]);
        if let Command::Run { task } = cli.command {
            assert_eq!(task, "list files in current directory");
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_history_command() {
        // Test history command with limit
        let cli = Cli::parse_from(["rove", "history", "--limit", "20"]);
        if let Command::History { limit } = cli.command {
            assert_eq!(limit, 20);
        } else {
            panic!("Expected History command");
        }
    }

    #[test]
    fn test_plugins_list() {
        // Test plugins list subcommand
        let cli = Cli::parse_from(["rove", "plugins", "list"]);
        if let Command::Plugins { action } = cli.command {
            assert!(matches!(action, PluginAction::List));
        } else {
            panic!("Expected Plugins command");
        }
    }

    #[test]
    fn test_config_get() {
        // Test config get subcommand
        let cli = Cli::parse_from(["rove", "config", "get", "core.workspace"]);
        if let Command::Config { action } = cli.command {
            if let ConfigAction::Get { key } = action {
                assert_eq!(key, "core.workspace");
            } else {
                panic!("Expected ConfigAction::Get");
            }
        } else {
            panic!("Expected Config command");
        }
    }

    #[test]
    fn test_bot_add_user() {
        // Test bot add user subcommand
        let cli = Cli::parse_from(["rove", "bot", "add-user", "123456789"]);
        if let Command::Bot { action } = cli.command {
            if let BotAction::AddUser { user_id } = action {
                assert_eq!(user_id, 123456789);
            } else {
                panic!("Expected BotAction::AddUser");
            }
        } else {
            panic!("Expected Bot command");
        }
    }

    #[test]
    fn test_skill_list() {
        let cli = Cli::parse_from(["rove", "skill", "list"]);
        if let Command::Skill { action } = cli.command {
            assert!(matches!(action, SkillAction::List { dir: None }));
        } else {
            panic!("Expected Skill command");
        }
    }

    #[test]
    fn test_skill_add() {
        let cli = Cli::parse_from([
            "rove",
            "skill",
            "add",
            "my-new-skill",
            "--description",
            "A test skill",
        ]);
        if let Command::Skill { action } = cli.command {
            if let SkillAction::Add { name, description } = action {
                assert_eq!(name, "my-new-skill");
                assert_eq!(description, Some("A test skill".to_string()));
            } else {
                panic!("Expected SkillAction::Add");
            }
        } else {
            panic!("Expected Skill command");
        }
    }
}
