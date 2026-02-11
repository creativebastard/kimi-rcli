//! KimiSoul - The heart of the agent system
//!
//! This module contains the main agent loop implementation, including:
//! - KimiSoul: The main agent orchestrator
//! - Agent: The agent runtime and execution context
//! - Toolset: Tool management and execution
//! - Compaction: Context compaction strategies
//! - DenwaRenji: D-Mail system for time-travel debugging
//! - Slash commands: User command handling

pub mod agent;
pub mod compaction;
pub mod denwarenji;
pub mod kimisoul;
pub mod slash;
pub mod toolset;

pub use agent::{Agent, AgentConfig, AgentState, LaborMarket, Runtime};
pub use compaction::{Compaction, SimpleCompaction};
pub use denwarenji::{DenwaRenji, DMail};
pub use kimisoul::{KimiSoul, SoulError, TurnOutcome, StepOutcome};
pub use slash::{SlashCommand, SlashCommandRegistry};
pub use toolset::{KimiToolset, Tool, ToolError, ToolResult, McpServerInfo, ToolCall, ToolCallResult};

use crate::types::{Message, Role};

/// Wire protocol for soul-side communication
#[derive(Debug, Clone)]
pub struct WireSoulSide;

impl WireSoulSide {
    /// Create a new wire for soul-side communication
    pub fn new() -> Self {
        Self
    }

    /// Send a message through the wire
    pub async fn send(&self, _message: crate::wire::WireMessage) -> Result<(), SoulError> {
        // TODO: Implement actual wire communication
        Ok(())
    }
}

impl Default for WireSoulSide {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a user message from text
pub fn user_message(content: impl Into<String>) -> Message {
    Message {
        role: Role::User,
        content: content.into(),
        metadata: None,
    }
}

/// Create an assistant message from text
pub fn assistant_message(content: impl Into<String>) -> Message {
    Message {
        role: Role::Assistant,
        content: content.into(),
        metadata: None,
    }
}

/// Create a system message from text
pub fn system_message(content: impl Into<String>) -> Message {
    Message {
        role: Role::System,
        content: content.into(),
        metadata: None,
    }
}

/// Create a tool message from text
pub fn tool_message(content: impl Into<String>) -> Message {
    Message {
        role: Role::Tool,
        content: content.into(),
        metadata: None,
    }
}
