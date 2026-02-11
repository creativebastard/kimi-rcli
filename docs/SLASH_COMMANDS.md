# Kimi CLI Slash Commands

This document lists all available slash commands in the Rust port of kimi-cli.

## General Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `/help` | `/h`, `/?` | Show help message |
| `/version` | - | Show version information |
| `/changelog` | `/release-notes` | Show release notes |
| `/feedback` | - | Submit feedback (opens GitHub issues) |
| `/exit` | `/quit`, `/q` | Exit the shell |

## Session Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `/session` | - | Set or show current session |
| `/sessions` | `/resume` | List and resume previous sessions |
| `/web` | - | Open Kimi Code Web UI |

## Authentication Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `/login` | `/setup` | Login to Kimi (OAuth device flow) |
| `/logout` | - | Logout and clear credentials |

## Context Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `/clear` | `/reset` | Clear the conversation context |
| `/compact` | - | Compact context to save tokens |
| `/yolo` | - | Toggle YOLO mode (auto-approve) |
| `/init` | - | Analyze codebase and generate AGENTS.md |

## Model Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `/model` | - | Set or show current model |
| `/tools` | - | List available tools |
| `/mcp` | - | Show MCP servers and tools |

## Usage Examples

```bash
# Start the shell
kimi-cli

# Inside the shell:
/login              # Login with OAuth
/setup              # Same as /login
/model              # Show current model
/model kimi-k2.5    # Switch model
/yolo               # Toggle auto-approve
/compact            # Compact context
/sessions           # List sessions
/web                # Open web UI
/exit               # Quit
```

## Implementation Notes

- All commands are available in both agent mode and shell mode
- Commands that require authentication will prompt for login if not authenticated
- The `/yolo` command toggles auto-approval for tool executions
- Context compaction removes older messages while preserving recent conversation
