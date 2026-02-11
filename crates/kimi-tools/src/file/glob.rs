//! Glob tool - find files using glob patterns.

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

/// Parameters for the Glob tool.
#[derive(Debug, Deserialize)]
pub struct GlobParams {
    /// The glob pattern to match files/directories.
    pub pattern: String,
    /// Absolute path to the directory to search in (defaults to working directory).
    #[serde(default)]
    pub directory: Option<String>,
    /// Whether to include directories in results.
    #[serde(default = "default_include_dirs")]
    pub include_dirs: bool,
}

fn default_include_dirs() -> bool {
    true
}

/// Tool for finding files using glob patterns.
#[derive(Debug)]
pub struct GlobTool;

impl GlobTool {
    /// Create a new GlobTool instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        "Find files and directories using glob patterns. Supports standard glob syntax like *, ?, and ** for recursive searches."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files/directories"
                },
                "directory": {
                    "type": "string",
                    "description": "Absolute path to the directory to search in (defaults to working directory)"
                },
                "include_dirs": {
                    "type": "boolean",
                    "description": "Whether to include directories in results",
                    "default": true
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: GlobParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        // Determine the base directory
        let base_dir = params
            .directory
            .as_deref()
            .map(Path::new)
            .unwrap_or_else(|| Path::new("."));

        // Validate that the base directory exists
        if !base_dir.exists() {
            return Err(ToolError::new(format!(
                "Directory does not exist: {}",
                base_dir.display()
            )));
        }

        // Build the full pattern
        let full_pattern = if let Some(dir) = &params.directory {
            format!("{}/{}", dir.trim_end_matches('/'), params.pattern)
        } else {
            params.pattern.clone()
        };

        // Perform the glob search
        let mut matches = Vec::new();

        let glob_matches = glob::glob(&full_pattern).map_err(|e| {
            ToolError::new(format!("Invalid glob pattern '{}': {e}", params.pattern))
        })?;

        for entry in glob_matches {
            match entry {
                Ok(path) => {
                    // Check if we should include this entry
                    let is_dir = path.is_dir();
                    if is_dir && !params.include_dirs {
                        continue;
                    }

                    // Convert to string, preferring absolute paths
                    let path_str = path.to_string_lossy().to_string();
                    matches.push(path_str);
                }
                Err(e) => {
                    // Log error but continue processing other matches
                    eprintln!("Error accessing path: {e}");
                }
            }
        }

        // Sort for consistent output
        matches.sort();

        // Format output
        let output = if matches.is_empty() {
            "No files found matching the pattern.".to_string()
        } else {
            matches.join("\n")
        };

        Ok(serde_json::json!(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_glob() {
        let tool = GlobTool::new();
        assert_eq!(tool.name(), "Glob");
        assert!(!tool.description().is_empty());
    }
}
