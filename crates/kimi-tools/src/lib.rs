//! kimi-tools - Built-in tools for the Kimi CLI agent
//!
//! This crate provides the core tool implementations used by the Kimi CLI agent,
//! including file operations, shell execution, web requests, and task management.

pub mod file;
pub mod shell;
pub mod todo;
pub mod web;

// Re-export the Tool trait and types from kimi-core
pub use kimi_core::{Tool, ToolError, ToolResult};

// Re-export all tools
pub use file::{GlobTool, GrepTool, ReadFileTool, StrReplaceFileTool, WriteFileTool};
pub use shell::ShellTool;
pub use todo::SetTodoListTool;
pub use web::{FetchURLTool, SearchWebTool};

use serde_json::Value;

/// Tool output wrapper for returning results from tool execution.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// The main output content
    pub output: String,
    /// Optional message for the LLM
    pub message: Option<String>,
}

impl ToolOutput {
    /// Create a new ToolOutput with just the output string.
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            message: None,
        }
    }

    /// Create a new ToolOutput with output and a message.
    pub fn with_message(output: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            message: Some(message.into()),
        }
    }
}

impl From<ToolOutput> for Value {
    fn from(output: ToolOutput) -> Value {
        if let Some(msg) = output.message {
            serde_json::json!({
                "output": output.output,
                "message": msg
            })
        } else {
            Value::String(output.output)
        }
    }
}

// Implement the conversion from ToolOutput to Value for ToolResult
// This allows tools to return ToolOutput which gets converted to Value
impl From<ToolOutput> for ToolResult {
    fn from(output: ToolOutput) -> ToolResult {
        Ok(output.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_output_new() {
        let output = ToolOutput::new("test output");
        assert_eq!(output.output, "test output");
        assert!(output.message.is_none());
    }

    #[test]
    fn test_tool_output_with_message() {
        let output = ToolOutput::with_message("test output", "test message");
        assert_eq!(output.output, "test output");
        assert_eq!(output.message, Some("test message".to_string()));
    }

    #[test]
    fn test_tool_output_into_value() {
        let output = ToolOutput::new("test output");
        let value: Value = output.into();
        assert_eq!(value, Value::String("test output".to_string()));

        let output = ToolOutput::with_message("test output", "test message");
        let value: Value = output.into();
        assert_eq!(
            value,
            serde_json::json!({
                "output": "test output",
                "message": "test message"
            })
        );
    }
}
