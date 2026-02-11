use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Kimi CLI - Your next CLI agent
#[derive(Parser, Debug, Clone)]
#[command(name = "kimi")]
#[command(about = "Kimi, your next CLI agent")]
#[command(version)]
pub struct Cli {
    /// Working directory for the session
    #[arg(short, long, value_name = "DIR")]
    pub work_dir: Option<PathBuf>,

    /// Session name or ID to use
    #[arg(short, long, value_name = "NAME")]
    pub session: Option<String>,

    /// Continue the last session
    #[arg(long = "continue")]
    pub continue_: bool,

    /// Model to use for this session
    #[arg(short, long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Enable thinking mode for reasoning models
    #[arg(long)]
    pub thinking: bool,

    /// Enable YOLO mode (auto-execute without confirmation)
    #[arg(long)]
    pub yolo: bool,

    /// Single prompt to execute (non-interactive)
    #[arg(short, long, value_name = "TEXT")]
    pub prompt: Option<String>,

    /// Print mode - output to stdout without interactive UI
    #[arg(long)]
    pub print: bool,

    /// Path to configuration file
    #[arg(long, value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    /// Path to MCP configuration file(s)
    #[arg(long, value_name = "FILE")]
    pub mcp_config_file: Vec<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Authenticate with Kimi API
    Login,
    /// Manage MCP (Model Context Protocol) servers
    Mcp {
        #[command(subcommand)]
        subcommand: McpCommands,
    },
}

/// MCP management subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum McpCommands {
    /// List configured MCP servers
    List,
    /// Add a new MCP server
    Add {
        /// Name of the MCP server
        name: String,
        /// Command to run the MCP server
        #[arg(short, long)]
        command: String,
        /// Arguments for the command
        #[arg(short, long)]
        args: Vec<String>,
    },
    /// Remove an MCP server
    Remove {
        /// Name of the MCP server to remove
        name: String,
    },
    /// Test connection to an MCP server
    Test {
        /// Name of the MCP server to test
        name: String,
    },
}

impl Cli {
    /// Get the effective working directory
    pub fn effective_work_dir(&self) -> PathBuf {
        self.work_dir
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Check if we should use a specific session
    pub fn session_name(&self) -> Option<String> {
        self.session.clone()
    }

    /// Check if auto-execute (YOLO) mode is enabled
    pub fn is_yolo_mode(&self) -> bool {
        self.yolo
    }

    /// Check if thinking mode is enabled
    pub fn is_thinking_mode(&self) -> bool {
        self.thinking
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::parse_from(["kimi", "--yolo", "--thinking", "-p", "Hello"]);
        assert!(cli.yolo);
        assert!(cli.thinking);
        assert_eq!(cli.prompt, Some("Hello".to_string()));
    }

    #[test]
    fn test_session_parsing() {
        let cli = Cli::parse_from(["kimi", "-s", "my-session", "--continue"]);
        assert_eq!(cli.session, Some("my-session".to_string()));
        assert!(cli.continue_);
    }
}
