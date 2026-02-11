//! WriteFile tool - writes content to a file.

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

/// Parameters for the WriteFile tool.
#[derive(Debug, Deserialize)]
pub struct WriteFileParams {
    /// The path to the file to write.
    pub path: String,
    /// The content to write to the file.
    pub content: String,
    /// The mode to use: "overwrite" (default) or "append".
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "overwrite".to_string()
}

/// Tool for writing files.
#[derive(Debug)]
pub struct WriteFileTool;

impl WriteFileTool {
    /// Create a new WriteFileTool instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "WriteFile"
    }

    fn description(&self) -> &str {
        "Write content to a file. Supports overwrite and append modes."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                },
                "mode": {
                    "type": "string",
                    "enum": ["overwrite", "append"],
                    "description": "The mode to use: 'overwrite' (default) or 'append'"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: WriteFileParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        let path = Path::new(&params.path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    ToolError::new(format!(
                        "Failed to create directory '{}': {e}",
                        parent.display()
                    ))
                })?;
            }
        }

        // Write content based on mode
        match params.mode.as_str() {
            "append" => {
                tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await
                    .map_err(|e| {
                        ToolError::new(format!("Failed to open file '{}': {e}", params.path))
                    })?;

                tokio::fs::write(path, &params.content).await.map_err(|e| {
                    ToolError::new(format!("Failed to write to file '{}': {e}", params.path))
                })?;
            }
            _ => {
                // Default to overwrite
                tokio::fs::write(path, &params.content).await.map_err(|e| {
                    ToolError::new(format!("Failed to write to file '{}': {e}", params.path))
                })?;
            }
        }

        let message = format!("Successfully wrote {} bytes to {}", params.content.len(), params.path);
        Ok(serde_json::json!({
            "output": "",
            "message": message
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_file() {
        let tool = WriteFileTool::new();
        assert_eq!(tool.name(), "WriteFile");
        assert!(!tool.description().is_empty());
    }
}
