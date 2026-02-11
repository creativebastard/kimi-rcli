//! kimi-core - Core types and wire protocol for the agent system

pub mod approval;
pub mod auth;
pub mod config;
pub mod context;
pub mod llm;
pub mod prompts;
pub mod session;
pub mod skill;
pub mod soul;
pub mod types;
pub mod wire;

pub use approval::{Approval, ApprovalError};
pub use config::{Config, ConfigError, LlmProvider, ProviderType};
pub use context::{Context, ContextError};
pub use session::{Session, SessionError};
pub use types::*;
pub use wire::WireMessage;

// Re-export soul types for convenience
pub use soul::{
    kimisoul::{KimiSoul, SoulError, TurnOutcome, StepOutcome},
    agent::{Agent, AgentState, AgentConfig, Runtime, RuntimeStats, Task, TaskStatus, LaborMarket, MarketTask},
    compaction::{Compaction, SimpleCompaction, CompactionError, AggressiveCompaction, SmartCompaction},
    denwarenji::{DenwaRenji, DMail},
    slash::{SlashCommand, SlashCommandRegistry, parse_slash_command},
    toolset::{KimiToolset, Tool, ToolError, ToolResult, McpServerInfo, ToolCall, ToolCallResult, SimpleTool},
};
