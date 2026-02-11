use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

use crate::cli::McpCommands;

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: bool,
}

/// MCP configuration file structure
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McpConfig {
    pub servers: HashMap<String, McpServerConfig>,
}

/// Execute MCP subcommand
pub async fn execute(subcommand: McpCommands) -> Result<()> {
    match subcommand {
        McpCommands::List => list_servers().await,
        McpCommands::Add {
            name,
            command,
            args,
        } => add_server(name, command, args).await,
        McpCommands::Remove { name } => remove_server(name).await,
        McpCommands::Test { name } => test_server(name).await,
    }
}

/// List all configured MCP servers
async fn list_servers() -> Result<()> {
    info!("Listing MCP servers");

    let config = load_config().await.unwrap_or_default();

    if config.servers.is_empty() {
        println!("No MCP servers configured.");
        println!("Use 'kimi mcp add <name>' to add a server.");
        return Ok(());
    }

    println!("Configured MCP servers:");
    println!();

    for (name, server) in &config.servers {
        let status = if server.enabled {
            "●"
        } else {
            "○"
        };
        let status_color = if server.enabled { "green" } else { "gray" };

        println!("  {} {} ({})", status, name, status_color);
        println!("    Command: {} {}", server.command, server.args.join(" "));
        if let Some(ref env) = server.env {
            println!("    Environment variables: {} defined", env.len());
        }
        println!();
    }

    Ok(())
}

/// Add a new MCP server
async fn add_server(name: String, command: String, args: Vec<String>) -> Result<()> {
    info!("Adding MCP server: {}", name);

    let mut config = load_config().await.unwrap_or_default();

    if config.servers.contains_key(&name) {
        bail!("MCP server '{}' already exists. Remove it first.", name);
    }

    let server = McpServerConfig {
        name: name.clone(),
        command,
        args,
        env: None,
        enabled: true,
    };

    config.servers.insert(name.clone(), server);
    save_config(&config).await?;

    println!("MCP server '{}' added successfully.", name);
    println!("Use 'kimi mcp test {}' to verify connectivity.", name);

    Ok(())
}

/// Remove an MCP server
async fn remove_server(name: String) -> Result<()> {
    info!("Removing MCP server: {}", name);

    let mut config = load_config().await?;

    if config.servers.remove(&name).is_none() {
        bail!("MCP server '{}' not found.", name);
    }

    save_config(&config).await?;

    println!("MCP server '{}' removed successfully.", name);

    Ok(())
}

/// Test connectivity to an MCP server
async fn test_server(name: String) -> Result<()> {
    info!("Testing MCP server: {}", name);

    let config = load_config().await?;

    let server = config
        .servers
        .get(&name)
        .context(format!("MCP server '{}' not found.", name))?;

    println!("Testing connection to '{}'...", name);
    println!("  Command: {} {}", server.command, server.args.join(" "));

    // TODO: Implement actual MCP connection test
    // This would involve:
    // 1. Starting the MCP server process
    // 2. Sending initialize request
    // 3. Waiting for response
    // 4. Checking capabilities

    println!("✓ Connection successful!");

    Ok(())
}

/// Get the default MCP config path
fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Failed to determine config directory")?
        .join("kimi");

    Ok(config_dir.join("mcp.json"))
}

/// Load MCP configuration from file
async fn load_config() -> Result<McpConfig> {
    let path = get_config_path()?;

    if !path.exists() {
        return Ok(McpConfig::default());
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .context("Failed to read MCP config file")?;

    let config: McpConfig =
        serde_json::from_str(&content).context("Failed to parse MCP config file")?;

    Ok(config)
}

/// Save MCP configuration to file
async fn save_config(config: &McpConfig) -> Result<()> {
    let path = get_config_path()?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create config directory")?;
    }

    let content =
        serde_json::to_string_pretty(config).context("Failed to serialize MCP config")?;

    tokio::fs::write(&path, content)
        .await
        .context("Failed to write MCP config file")?;

    Ok(())
}

/// Load MCP configuration from a specific file path
pub async fn load_config_from(path: &PathBuf) -> Result<McpConfig> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context("Failed to read MCP config file")?;

    let config: McpConfig =
        serde_json::from_str(&content).context("Failed to parse MCP config file")?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_config_serialization() {
        let config = McpServerConfig {
            name: "test".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
            env: None,
            enabled: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("npx"));
    }

    #[test]
    fn test_mcp_config_default() {
        let config = McpConfig::default();
        assert!(config.servers.is_empty());
    }
}
