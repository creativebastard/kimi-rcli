//! StrReplaceFile tool - replace strings within a file.

use crate::{Tool, ToolError, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

/// A single edit operation.
#[derive(Debug, Deserialize)]
pub struct Edit {
    /// The old string to replace.
    pub old: String,
    /// The new string to replace with.
    pub new: String,
    /// Whether to replace all occurrences.
    #[serde(default)]
    pub replace_all: bool,
}

/// Parameters for the StrReplaceFile tool.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StrReplaceFileParams {
    /// Single edit operation.
    Single {
        /// The path to the file to edit.
        path: String,
        /// The old string to replace.
        old: String,
        /// The new string to replace with.
        new: String,
        /// Whether to replace all occurrences.
        #[serde(default)]
        replace_all: bool,
    },
    /// Multiple edit operations.
    Multiple {
        /// The path to the file to edit.
        path: String,
        /// The list of edits to apply.
        edit: Vec<Edit>,
    },
}

/// Tool for replacing strings in files.
pub struct StrReplaceFileTool;

impl StrReplaceFileTool {
    /// Create a new StrReplaceFileTool instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for StrReplaceFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for StrReplaceFileTool {
    fn name(&self) -> &str {
        "StrReplaceFile"
    }

    fn description(&self) -> &str {
        "Replace specific strings within a file. Supports multiple edits in one call."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old": {
                    "type": "string",
                    "description": "The old string to replace"
                },
                "new": {
                    "type": "string",
                    "description": "The new string to replace with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Whether to replace all occurrences"
                },
                "edit": {
                    "type": "array",
                    "description": "List of edits to apply",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old": { "type": "string" },
                            "new": { "type": "string" },
                            "replace_all": { "type": "boolean" }
                        },
                        "required": ["old", "new"]
                    }
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: StrReplaceFileParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        let (path, edits) = match params {
            StrReplaceFileParams::Single {
                path,
                old,
                new,
                replace_all,
            } => (
                path,
                vec![Edit {
                    old,
                    new,
                    replace_all,
                }],
            ),
            StrReplaceFileParams::Multiple { path, edit } => (path, edit),
        };

        let file_path = Path::new(&path);

        // Check if file exists
        if !file_path.exists() {
            return Err(ToolError::new(format!(
                "File does not exist: {path}"
            )));
        }

        // Read the file content
        let mut content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
            ToolError::new(format!("Failed to read file '{path}': {e}"))
        })?;

        // Apply edits
        let mut total_replacements = 0;
        for edit in edits {
            let old_str = &edit.old;
            let new_str = &edit.new;

            if !content.contains(old_str) {
                return Err(ToolError::new(format!(
                    "Could not find the string to replace in file: {old_str}"
                )));
            }

            if edit.replace_all {
                let count = content.matches(old_str).count();
                content = content.replace(old_str, new_str);
                total_replacements += count;
            } else {
                // Replace only first occurrence
                if let Some(pos) = content.find(old_str) {
                    content.replace_range(pos..pos + old_str.len(), new_str);
                    total_replacements += 1;
                }
            }
        }

        // Write the modified content back
        tokio::fs::write(file_path, &content).await.map_err(|e| {
            ToolError::new(format!("Failed to write to file '{path}': {e}"))
        })?;

        let message = format!(
            "Successfully made {total_replacements} replacement(s) in {path}"
        );
        Ok(ToolOutput::with_message("", message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_replace_file() {
        let tool = StrReplaceFileTool::new();
        assert_eq!(tool.name(), "StrReplaceFile");
        assert!(!tool.description().is_empty());
    }
}
