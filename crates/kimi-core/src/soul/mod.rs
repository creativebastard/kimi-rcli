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
pub mod chat;
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
use crate::wire::WireMessage;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Wire protocol for soul-side communication
/// 
/// This struct holds an optional mpsc sender for sending wire messages.
/// When no sender is configured, messages are silently dropped.
#[derive(Debug, Clone)]
pub struct WireSoulSide {
    sender: Option<Arc<mpsc::Sender<WireMessage>>>,
}

impl WireSoulSide {
    /// Create a new wire for soul-side communication with no sender
    pub fn new() -> Self {
        Self {
            sender: None,
        }
    }

    /// Create a new wire with an mpsc sender
    pub fn with_sender(sender: mpsc::Sender<WireMessage>) -> Self {
        Self {
            sender: Some(Arc::new(sender)),
        }
    }

    /// Send a message through the wire
    pub async fn send(&self, message: WireMessage) -> Result<(), SoulError> {
        if let Some(sender) = &self.sender {
            sender.send(message).await
                .map_err(|e| SoulError::Wire(format!("Failed to send message: {}", e)))?;
        }
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
