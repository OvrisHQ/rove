//! Manifest types for plugin and core tool metadata

use serde::{Deserialize, Serialize};

/// Main manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub team_public_key: String,
    pub signature: String,
    pub generated_at: String,
    pub core_tools: Vec<CoreToolEntry>,
    pub plugins: Vec<PluginEntry>,
}

impl Manifest {
    /// Get a core tool entry by name
    pub fn get_core_tool(&self, name: &str) -> Option<&CoreToolEntry> {
        self.core_tools.iter().find(|tool| tool.name == name)
    }

    /// Get a plugin entry by name
    pub fn get_plugin(&self, name: &str) -> Option<&PluginEntry> {
        self.plugins.iter().find(|plugin| plugin.name == name)
    }

    /// Parse manifest from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize manifest to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Serialize manifest to JSON bytes (for signature verification)
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

/// Core tool entry in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreToolEntry {
    pub name: String,
    pub version: String,
    pub path: String,
    pub hash: String,
    pub signature: String,
    pub platform: String,
}

impl CoreToolEntry {
    /// Check if this core tool entry is for the current platform
    pub fn is_current_platform(&self) -> bool {
        let current = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        self.platform == current
    }
}

/// Plugin entry in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub name: String,
    pub version: String,
    pub path: String,
    pub hash: String,
    pub permissions: PluginPermissions,
}

impl PluginEntry {
    /// Check if a path is allowed by this plugin's permissions
    pub fn is_path_allowed(&self, path: &str) -> bool {
        // Check denied paths first
        for denied in &self.permissions.denied_paths {
            if path.contains(denied) {
                return false;
            }
        }

        // Check allowed paths
        if self.permissions.allowed_paths.is_empty() {
            return true; // No restrictions
        }

        self.permissions
            .allowed_paths
            .iter()
            .any(|allowed| path.starts_with(allowed) || allowed == "workspace")
    }

    /// Check if a command is allowed by this plugin's permissions
    pub fn is_command_allowed(&self, command: &str) -> bool {
        if !self.permissions.can_execute {
            return false;
        }

        // Check denied flags
        if let Some(denied_flags) = &self.permissions.denied_flags {
            for flag in denied_flags {
                if command.contains(flag) {
                    return false;
                }
            }
        }

        // Check allowed commands
        if let Some(allowed_commands) = &self.permissions.allowed_commands {
            if allowed_commands.is_empty() {
                return true; // No restrictions
            }
            return allowed_commands
                .iter()
                .any(|allowed| command.starts_with(allowed));
        }

        true // No command restrictions
    }
}

/// Plugin permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPermissions {
    /// Paths the plugin is allowed to access (e.g., ["workspace", "/tmp"])
    pub allowed_paths: Vec<String>,
    /// Paths the plugin is explicitly denied from accessing (e.g., [".ssh", ".env"])
    pub denied_paths: Vec<String>,
    /// Maximum file size the plugin can read/write (in bytes)
    pub max_file_size: Option<u64>,
    /// Whether the plugin can execute commands
    pub can_execute: bool,
    /// Commands the plugin is allowed to execute (e.g., ["git", "ls"])
    pub allowed_commands: Option<Vec<String>>,
    /// Command flags that are denied (e.g., ["--force", "-rf"])
    pub denied_flags: Option<Vec<String>>,
    /// Maximum execution time for commands (in seconds)
    pub max_execution_time: Option<u64>,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self {
            allowed_paths: vec!["workspace".to_string()],
            denied_paths: vec![
                ".ssh".to_string(),
                ".env".to_string(),
                "credentials".to_string(),
                "id_rsa".to_string(),
                "id_ed25519".to_string(),
            ],
            max_file_size: Some(10 * 1024 * 1024), // 10MB default
            can_execute: false,
            allowed_commands: None,
            denied_flags: Some(vec![
                "--force".to_string(),
                "-rf".to_string(),
                "--delete".to_string(),
                "--hard".to_string(),
            ]),
            max_execution_time: Some(30), // 30 seconds default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialization() {
        let manifest = Manifest {
            version: "1.0.0".to_string(),
            team_public_key: "ed25519:test_key".to_string(),
            signature: "ed25519:test_sig".to_string(),
            generated_at: "2024-01-15T10:30:00Z".to_string(),
            core_tools: vec![],
            plugins: vec![],
        };

        let json = manifest.to_json().unwrap();
        let parsed = Manifest::from_json(&json).unwrap();

        assert_eq!(manifest.version, parsed.version);
        assert_eq!(manifest.team_public_key, parsed.team_public_key);
        assert_eq!(manifest.signature, parsed.signature);
    }

    #[test]
    fn test_get_core_tool() {
        let manifest = Manifest {
            version: "1.0.0".to_string(),
            team_public_key: "ed25519:test_key".to_string(),
            signature: "ed25519:test_sig".to_string(),
            generated_at: "2024-01-15T10:30:00Z".to_string(),
            core_tools: vec![CoreToolEntry {
                name: "telegram".to_string(),
                version: "0.1.0".to_string(),
                path: "core-tools/telegram.so".to_string(),
                hash: "sha256:abc123".to_string(),
                signature: "ed25519:sig123".to_string(),
                platform: "linux-x86_64".to_string(),
            }],
            plugins: vec![],
        };

        assert!(manifest.get_core_tool("telegram").is_some());
        assert!(manifest.get_core_tool("nonexistent").is_none());
    }

    #[test]
    fn test_get_plugin() {
        let manifest = Manifest {
            version: "1.0.0".to_string(),
            team_public_key: "ed25519:test_key".to_string(),
            signature: "ed25519:test_sig".to_string(),
            generated_at: "2024-01-15T10:30:00Z".to_string(),
            core_tools: vec![],
            plugins: vec![PluginEntry {
                name: "fs-editor".to_string(),
                version: "0.1.0".to_string(),
                path: "plugins/fs-editor.wasm".to_string(),
                hash: "sha256:def456".to_string(),
                permissions: PluginPermissions::default(),
            }],
        };

        assert!(manifest.get_plugin("fs-editor").is_some());
        assert!(manifest.get_plugin("nonexistent").is_none());
    }

    #[test]
    fn test_plugin_path_permissions() {
        let plugin = PluginEntry {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            path: "test.wasm".to_string(),
            hash: "sha256:test".to_string(),
            permissions: PluginPermissions {
                allowed_paths: vec!["workspace".to_string(), "/tmp".to_string()],
                denied_paths: vec![".ssh".to_string(), ".env".to_string()],
                max_file_size: Some(1024),
                can_execute: false,
                allowed_commands: None,
                denied_flags: None,
                max_execution_time: None,
            },
        };

        // Allowed paths
        assert!(plugin.is_path_allowed("workspace/file.txt"));
        assert!(plugin.is_path_allowed("/tmp/test.txt"));

        // Denied paths
        assert!(!plugin.is_path_allowed("/home/user/.ssh/id_rsa"));
        assert!(!plugin.is_path_allowed("workspace/.env"));
    }

    #[test]
    fn test_plugin_command_permissions() {
        let plugin = PluginEntry {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            path: "test.wasm".to_string(),
            hash: "sha256:test".to_string(),
            permissions: PluginPermissions {
                allowed_paths: vec![],
                denied_paths: vec![],
                max_file_size: None,
                can_execute: true,
                allowed_commands: Some(vec!["git".to_string(), "ls".to_string()]),
                denied_flags: Some(vec!["--force".to_string(), "-rf".to_string()]),
                max_execution_time: Some(30),
            },
        };

        // Allowed commands
        assert!(plugin.is_command_allowed("git status"));
        assert!(plugin.is_command_allowed("ls -la"));

        // Denied commands
        assert!(!plugin.is_command_allowed("rm -rf /"));
        assert!(!plugin.is_command_allowed("git push --force"));

        // Not in allowed list
        assert!(!plugin.is_command_allowed("rm file.txt"));
    }

    #[test]
    fn test_plugin_no_execute_permission() {
        let plugin = PluginEntry {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            path: "test.wasm".to_string(),
            hash: "sha256:test".to_string(),
            permissions: PluginPermissions {
                allowed_paths: vec![],
                denied_paths: vec![],
                max_file_size: None,
                can_execute: false,
                allowed_commands: Some(vec!["git".to_string()]),
                denied_flags: None,
                max_execution_time: None,
            },
        };

        // Should deny all commands if can_execute is false
        assert!(!plugin.is_command_allowed("git status"));
        assert!(!plugin.is_command_allowed("ls"));
    }

    #[test]
    fn test_plugin_permissions_default() {
        let perms = PluginPermissions::default();

        // Default should allow workspace
        assert_eq!(perms.allowed_paths, vec!["workspace"]);

        // Default should deny sensitive paths
        assert!(perms.denied_paths.contains(&".ssh".to_string()));
        assert!(perms.denied_paths.contains(&".env".to_string()));

        // Default should not allow execution
        assert!(!perms.can_execute);

        // Default should have file size limit
        assert_eq!(perms.max_file_size, Some(10 * 1024 * 1024));

        // Default should have execution time limit
        assert_eq!(perms.max_execution_time, Some(30));
    }
}
