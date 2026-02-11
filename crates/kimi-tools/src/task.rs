//! Task tool - spawn a subagent to perform a specific task.
//!
//! This tool allows the agent to delegate work to subagents, which run in isolated
//! contexts without access to the parent's conversation history.

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Parameters for the Task tool.
#[derive(Debug, Deserialize)]
pub struct TaskParams {
    /// A short (3-5 word) description of the task.
    pub description: String,
    /// The detailed task for the subagent to perform.
    pub prompt: String,
    /// The name of the specialized subagent to use (e.g., "coder", "searcher").
    #[serde(default)]
    pub subagent_name: Option<String>,
}

/// A subagent that can execute tasks.
#[derive(Debug, Clone)]
pub struct Subagent {
    /// Subagent name
    pub name: String,
    /// System prompt for the subagent
    pub system_prompt: String,
}

impl Subagent {
    /// Create a new subagent with the given name and system prompt.
    pub fn new(name: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            system_prompt: system_prompt.into(),
        }
    }

    /// Get a subagent by name with predefined configurations.
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "coder" => Some(Self::new(
                "coder",
                "You are a skilled software engineer. Write clean, well-documented code. \
                 Follow best practices and explain your reasoning.",
            )),
            "searcher" => Some(Self::new(
                "searcher",
                "You are a research assistant. Find accurate information and provide \
                 well-sourced answers. Be thorough but concise.",
            )),
            "fixer" => Some(Self::new(
                "fixer",
                "You are a debugging expert. Analyze code, identify issues, and provide \
                 minimal fixes. Explain what was wrong and why your fix works.",
            )),
            _ => None,
        }
    }
}

/// Tool for spawning subagents.
#[derive(Debug)]
pub struct TaskTool {
    /// Available subagents
    subagents: Arc<RwLock<Vec<Subagent>>>,
}

impl TaskTool {
    /// Create a new TaskTool instance.
    pub fn new() -> Self {
        let default_subagents = vec![
            Subagent::by_name("coder").unwrap(),
            Subagent::by_name("searcher").unwrap(),
            Subagent::by_name("fixer").unwrap(),
        ];
        
        Self {
            subagents: Arc::new(RwLock::new(default_subagents)),
        }
    }

    /// Create a TaskTool with custom subagents.
    pub fn with_subagents(subagents: Vec<Subagent>) -> Self {
        Self {
            subagents: Arc::new(RwLock::new(subagents)),
        }
    }

    /// Register a new subagent.
    pub async fn register_subagent(&self, subagent: Subagent) {
        let mut subagents = self.subagents.write().await;
        subagents.push(subagent);
    }

    /// Get a subagent by name.
    pub async fn get_subagent(&self, name: &str) -> Option<Subagent> {
        // First check predefined subagents
        if let Some(subagent) = Subagent::by_name(name) {
            return Some(subagent);
        }
        
        // Then check registered subagents
        let subagents = self.subagents.read().await;
        subagents.iter().find(|s| s.name == name).cloned()
    }

    /// List available subagents.
    pub async fn list_subagents(&self) -> Vec<Subagent> {
        let mut all = vec![
            Subagent::by_name("coder").unwrap(),
            Subagent::by_name("searcher").unwrap(),
            Subagent::by_name("fixer").unwrap(),
        ];
        
        let subagents = self.subagents.read().await;
        all.extend(subagents.iter().cloned());
        all
    }

    /// Execute the task with a subagent.
    async fn execute_task(&self, params: &TaskParams) -> Result<String, ToolError> {
        // Determine which subagent to use
        let subagent_name = params.subagent_name.as_deref().unwrap_or("coder");
        let subagent = self.get_subagent(subagent_name).await
            .ok_or_else(|| ToolError::new(format!("Unknown subagent: {}", subagent_name)))?;

        // In a real implementation, this would:
        // 1. Create a new agent instance with the subagent's system prompt
        // 2. Run the task in isolation
        // 3. Return the result
        
        // For now, we return a placeholder response
        let result = format!(
            "Task '{}' executed by subagent '{}'\n\
             System prompt: {}\n\
             Task prompt: {}\n\
             \n\
             [Note: This is a placeholder. In a full implementation, \
             the subagent would process the task and return actual results.]",
            params.description,
            subagent.name,
            subagent.system_prompt,
            params.prompt
        );

        Ok(result)
    }
}

impl Default for TaskTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    fn description(&self) -> &str {
        "Spawn a subagent to perform a specific task. Use this tool to delegate work \
         to specialized subagents that run in isolated contexts without access to \
         the parent's conversation history."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The detailed task for the subagent to perform"
                },
                "subagent_name": {
                    "type": "string",
                    "description": "The name of the specialized subagent to use (e.g., 'coder', 'searcher', 'fixer')",
                    "enum": ["coder", "searcher", "fixer"]
                }
            },
            "required": ["description", "prompt"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: TaskParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        let result = self.execute_task(&params).await?;
        Ok(serde_json::json!(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_tool() {
        let tool = TaskTool::new();
        assert_eq!(tool.name(), "Task");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_subagent_by_name() {
        let coder = Subagent::by_name("coder");
        assert!(coder.is_some());
        assert_eq!(coder.unwrap().name, "coder");

        let unknown = Subagent::by_name("unknown");
        assert!(unknown.is_none());
    }

    #[tokio::test]
    async fn test_list_subagents() {
        let tool = TaskTool::new();
        let subagents = tool.list_subagents().await;
        assert!(!subagents.is_empty());
        
        let names: Vec<_> = subagents.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"coder".to_string()));
        assert!(names.contains(&"searcher".to_string()));
        assert!(names.contains(&"fixer".to_string()));
    }

    #[tokio::test]
    async fn test_execute_task() {
        let tool = TaskTool::new();
        let params = serde_json::json!({
            "description": "Test task",
            "prompt": "Write a hello world program",
            "subagent_name": "coder"
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());
        
        let value = result.unwrap();
        let output = value.as_str().unwrap_or("");
        assert!(output.contains("coder"));
        assert!(output.contains("Test task"));
    }

    #[tokio::test]
    async fn test_execute_task_default_subagent() {
        let tool = TaskTool::new();
        let params = serde_json::json!({
            "description": "Test task",
            "prompt": "Do something"
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());
    }
}
