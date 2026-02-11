//! ReadFile tool - reads text content from a file.

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

/// Parameters for the ReadFile tool.
#[derive(Debug, Deserialize)]
pub struct ReadFileParams {
    /// The path to the file to read.
    pub path: String,
    /// The line number to start reading from (1-indexed).
    #[serde(default)]
    pub line_offset: Option<usize>,
    /// The number of lines to read.
    #[serde(default)]
    pub n_lines: Option<usize>,
}

/// Tool for reading files.
#[derive(Debug)]
pub struct ReadFileTool;

impl ReadFileTool {
    /// Create a new ReadFileTool instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "ReadFile"
    }

    fn description(&self) -> &str {
        "Read text content from a file. Supports reading specific line ranges."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read"
                },
                "line_offset": {
                    "type": "integer",
                    "description": "The line number to start reading from (1-indexed)"
                },
                "n_lines": {
                    "type": "integer",
                    "description": "The number of lines to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: ReadFileParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        let path = Path::new(&params.path);

        // Check if file exists
        if !path.exists() {
            return Err(ToolError::new(format!(
                "File does not exist: {}",
                params.path
            )));
        }

        // Check if it's a file
        if !path.is_file() {
            return Err(ToolError::new(format!(
                "Path is not a file: {}",
                params.path
            )));
        }

        // Read the file content
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            ToolError::new(format!("Failed to read file '{}': {e}", params.path))
        })?;

        // Handle line range if specified
        let output = if params.line_offset.is_some() || params.n_lines.is_some() {
            let lines: Vec<&str> = content.lines().collect();
            let start = params.line_offset.unwrap_or(1).saturating_sub(1);
            let end = if let Some(n) = params.n_lines {
                (start + n).min(lines.len())
            } else {
                lines.len()
            };

            if start >= lines.len() {
                return Err(ToolError::new(format!(
                    "Line offset {} exceeds file length of {} lines",
                    start + 1,
                    lines.len()
                )));
            }

            lines[start..end].join("\n")
        } else {
            content
        };

        Ok(serde_json::json!(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file() {
        let tool = ReadFileTool::new();
        assert_eq!(tool.name(), "ReadFile");
        assert!(!tool.description().is_empty());
    }
}
