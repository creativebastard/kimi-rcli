//! Shared types for the kimi-core crate

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// User input for a turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInput {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

/// Attachment to user input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub path: PathBuf,
    pub mime_type: String,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

/// Approval kind for approval responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    Approve,
    Reject,
    ApproveOnce,
}

/// Message in the context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Role of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Checkpoint for context compaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub message_index: usize,
    pub token_count: usize,
    pub summary: Option<String>,
}

/// LLM Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModel {
    pub name: String,
    pub provider: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
}

/// Loop control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopControl {
    pub max_iterations: usize,
    pub timeout_seconds: u64,
}

impl Default for LoopControl {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            timeout_seconds: 300,
        }
    }
}

/// Services configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Services {
    pub enabled: Vec<String>,
    pub config: HashMap<String, serde_json::Value>,
}

/// MCP (Model Context Protocol) configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    pub servers: Vec<McpServer>,
    pub enabled_tools: Option<Vec<String>>,
}

/// MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
}

/// Request for approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub tool_call_id: String,
    pub sender: String,
    pub action: String,
    pub description: String,
}
