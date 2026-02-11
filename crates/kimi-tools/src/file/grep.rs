//! Grep tool - search file contents using regex.

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Output mode for grep results.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    /// Show matching lines.
    #[default]
    Content,
    /// Show file paths only.
    FilesWithMatches,
    /// Show count of matches.
    CountMatches,
}

/// Parameters for the Grep tool.
#[derive(Debug, Deserialize)]
pub struct GrepParams {
    /// The regular expression pattern to search for.
    pub pattern: String,
    /// File or directory to search in (defaults to current working directory).
    #[serde(default)]
    pub path: Option<String>,
    /// Glob pattern to filter files (e.g. *.js, *.{ts,tsx}).
    #[serde(default)]
    pub glob: Option<String>,
    /// Number of lines to show before each match.
    #[serde(default)]
    pub before_context: Option<usize>,
    /// Number of lines to show after each match.
    #[serde(default)]
    pub after_context: Option<usize>,
    /// Number of lines to show before and after each match.
    #[serde(default)]
    pub context: Option<usize>,
    /// Case insensitive search.
    #[serde(default)]
    pub case_insensitive: bool,
    /// Limit output to first N lines.
    #[serde(default)]
    pub head_limit: Option<usize>,
    /// Output mode.
    #[serde(default)]
    pub output_mode: OutputMode,
    /// File type to search (e.g., py, rust, js).
    #[serde(default)]
    pub file_type: Option<String>,
}

/// A single match result.
#[derive(Debug)]
struct MatchResult {
    file_path: String,
    line_number: usize,
    line_content: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
}

/// Tool for searching file contents using regex.
#[derive(Debug)]
pub struct GrepTool;

impl GrepTool {
    /// Create a new GrepTool instance.
    pub fn new() -> Self {
        Self
    }

    /// Get the glob pattern from file type or provided glob.
    fn get_glob_pattern(&self, params: &GrepParams) -> Option<String> {
        if let Some(ref glob) = params.glob {
            return Some(glob.clone());
        }

        params.file_type.as_ref().map(|ft| match ft.as_str() {
            "rs" | "rust" => "*.rs".to_string(),
            "py" | "python" => "*.py".to_string(),
            "js" => "*.js".to_string(),
            "ts" | "typescript" => "*.{ts,tsx}".to_string(),
            "go" => "*.go".to_string(),
            "java" => "*.java".to_string(),
            _ => format!("*.{ft}"),
        })
    }

    /// Search a single file for matches.
    async fn search_file(
        &self,
        file_path: &Path,
        regex: &Regex,
        params: &GrepParams,
    ) -> Result<Vec<MatchResult>, ToolError> {
        let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
            ToolError::new(format!("Failed to read file '{}': {e}", file_path.display()))
        })?;

        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();

        let before_ctx = params.context.or(params.before_context).unwrap_or(0);
        let after_ctx = params.context.or(params.after_context).unwrap_or(0);

        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                let line_number = i + 1;

                // Get context lines
                let before_start = i.saturating_sub(before_ctx);
                let before_lines: Vec<(usize, String)> = (before_start..i)
                    .map(|j| (j + 1, lines[j].to_string()))
                    .collect();

                let after_end = (i + 1 + after_ctx).min(lines.len());
                let after_lines: Vec<(usize, String)> = (i + 1..after_end)
                    .map(|j| (j + 1, lines[j].to_string()))
                    .collect();

                matches.push(MatchResult {
                    file_path: file_path.to_string_lossy().to_string(),
                    line_number,
                    line_content: line.to_string(),
                    before_lines,
                    after_lines,
                });
            }
        }

        Ok(matches)
    }

    /// Collect all files to search.
    async fn collect_files(&self, params: &GrepParams) -> Result<Vec<std::path::PathBuf>, ToolError> {
        let search_path = params
            .path
            .as_deref()
            .map(Path::new)
            .unwrap_or_else(|| Path::new("."));

        let mut files = Vec::new();

        if search_path.is_file() {
            files.push(search_path.to_path_buf());
        } else if search_path.is_dir() {
            let glob_pattern = self.get_glob_pattern(params);

            let pattern = if let Some(g) = glob_pattern {
                format!("{}/{}", search_path.display(), g)
            } else {
                format!("{}/**/*", search_path.display())
            };

            let glob_matches = glob::glob(&pattern).map_err(|e| {
                ToolError::new(format!("Invalid glob pattern: {e}"))
            })?;

            for entry in glob_matches {
                if let Ok(path) = entry {
                    if path.is_file() {
                        files.push(path);
                    }
                }
            }
        }

        Ok(files)
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regular expressions. Based on ripgrep with support for context lines, file filtering, and multiple output modes."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regular expression pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (defaults to current working directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. *.js, *.{ts,tsx})"
                },
                "before_context": {
                    "type": "integer",
                    "description": "Number of lines to show before each match"
                },
                "after_context": {
                    "type": "integer",
                    "description": "Number of lines to show after each match"
                },
                "context": {
                    "type": "integer",
                    "description": "Number of lines to show before and after each match"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case insensitive search"
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Limit output to first N lines"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count_matches"],
                    "description": "Output mode"
                },
                "file_type": {
                    "type": "string",
                    "description": "File type to search (e.g., py, rust, js)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: GrepParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        // Build the regex
        let mut regex_builder = regex::RegexBuilder::new(&params.pattern);
        regex_builder.case_insensitive(params.case_insensitive);

        let regex = regex_builder.build().map_err(|e| {
            ToolError::new(format!("Invalid regex pattern '{}': {e}", params.pattern))
        })?;

        // Collect files to search
        let files = self.collect_files(&params).await?;

        // Search all files
        let mut all_matches: Vec<MatchResult> = Vec::new();
        let mut file_match_counts: HashMap<String, usize> = HashMap::new();

        for file in files {
            match self.search_file(&file, &regex, &params).await {
                Ok(matches) => {
                    if !matches.is_empty() {
                        file_match_counts.insert(file.to_string_lossy().to_string(), matches.len());
                        all_matches.extend(matches);
                    }
                }
                Err(_) => {
                    // Skip files that can't be read (e.g., binary files)
                    continue;
                }
            }
        }

        // Apply head limit if specified
        if let Some(limit) = params.head_limit {
            all_matches.truncate(limit);
        }

        // Format output based on mode
        let output = match params.output_mode {
            OutputMode::FilesWithMatches => {
                let mut files: Vec<String> = file_match_counts.keys().cloned().collect();
                files.sort();
                files.join("\n")
            }
            OutputMode::CountMatches => {
                let mut entries: Vec<(String, usize)> = file_match_counts.into_iter().collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                entries
                    .into_iter()
                    .map(|(file, count)| format!("{count}:{file}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            OutputMode::Content => {
                let mut lines = Vec::new();
                let mut current_file: Option<&str> = None;

                for m in &all_matches {
                    // Print file header if changed
                    if current_file != Some(&m.file_path) {
                        if current_file.is_some() {
                            lines.push(String::new());
                        }
                        lines.push(format!("{}", m.file_path));
                        current_file = Some(&m.file_path);
                    }

                    // Print before context
                    for (line_num, content) in &m.before_lines {
                        lines.push(format!("{line_num}-{content}"));
                    }

                    // Print match
                    lines.push(format!("{}:{}", m.line_number, m.line_content));

                    // Print after context
                    for (line_num, content) in &m.after_lines {
                        lines.push(format!("{line_num}-{content}"));
                    }
                }

                lines.join("\n")
            }
        };

        Ok(serde_json::json!(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grep() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "Grep");
        assert!(!tool.description().is_empty());
    }
}
