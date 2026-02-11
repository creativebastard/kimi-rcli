//! Shell tool - execute shell commands.

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::process::Stdio;
use tokio::process::Command;

/// Parameters for the Shell tool.
#[derive(Debug, Deserialize)]
pub struct ShellParams {
    /// The bash command to execute.
    pub command: String,
    /// The timeout in seconds for the command to execute.
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    60
}

/// Tool for executing shell commands.
#[derive(Debug)]
pub struct ShellTool {
    shell: String,
    shell_arg: String,
}

impl ShellTool {
    /// Create a new ShellTool with auto-detected shell.
    pub fn new() -> Self {
        // Auto-detect shell based on platform
        #[cfg(windows)]
        {
            Self {
                shell: "powershell".to_string(),
                shell_arg: "-Command".to_string(),
            }
        }
        #[cfg(not(windows))]
        {
            Self {
                shell: "/bin/bash".to_string(),
                shell_arg: "-c".to_string(),
            }
        }
    }

    /// Create a new ShellTool with a specific shell.
    pub fn with_shell(shell: impl Into<String>, shell_arg: impl Into<String>) -> Self {
        Self {
            shell: shell.into(),
            shell_arg: shell_arg.into(),
        }
    }

    /// Execute a command with timeout.
    async fn execute_command(&self, command: &str, timeout_secs: u64) -> Result<(String, String, i32), ToolError> {
        let mut cmd = Command::new(&self.shell);
        cmd.arg(&self.shell_arg)
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            ToolError::new(format!("Failed to spawn shell process: {e}"))
        })?;

        // Set up timeout
        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        let result = tokio::time::timeout(timeout, child.wait_with_output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                Ok((stdout, stderr, exit_code))
            }
            Ok(Err(e)) => Err(ToolError::new(format!("Failed to execute command: {e}"))),
            Err(_) => Err(ToolError::new(format!(
                "Command timed out after {timeout_secs} seconds"
            ))),
        }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "Shell"
    }

    fn description(&self) -> &str {
        "Execute a bash command. Use this tool to explore the filesystem, edit files, run scripts, get system information, etc."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "The timeout in seconds for the command to execute",
                    "default": 60,
                    "minimum": 1,
                    "maximum": 300
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: ShellParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        let (stdout, stderr, exit_code) = self
            .execute_command(&params.command, params.timeout)
            .await?;

        // Combine stdout and stderr
        let mut output = stdout;
        if !stderr.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&stderr);
        }

        // Check exit code
        if exit_code != 0 {
            return Err(ToolError::new(format!(
                "Command failed with exit code {exit_code}:\n{output}"
            )));
        }

        Ok(serde_json::json!(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell() {
        let tool = ShellTool::new();
        assert_eq!(tool.name(), "Shell");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_shell_echo() {
        let tool = ShellTool::new();
        let params = serde_json::json!({
            "command": "echo 'Hello World'",
            "timeout": 10
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        let output = value.as_str().unwrap_or("");
        assert!(output.contains("Hello World"));
    }
}
