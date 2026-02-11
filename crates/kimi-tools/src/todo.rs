//! SetTodoList tool - manage a todo list for the agent.

use crate::{Tool, ToolError, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A single todo item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    /// The ID of the todo item.
    pub id: usize,
    /// The description of the todo item.
    pub description: String,
    /// Whether the todo item is completed.
    pub completed: bool,
    /// The priority of the todo item (low, medium, high).
    pub priority: Option<String>,
}

/// Parameters for the SetTodoList tool.
#[derive(Debug, Deserialize)]
pub struct SetTodoListParams {
    /// The list of todo items to set.
    pub items: Vec<TodoItemInput>,
}

/// Input for a single todo item.
#[derive(Debug, Deserialize)]
pub struct TodoItemInput {
    /// The ID of the todo item (optional, will be auto-assigned if not provided).
    pub id: Option<usize>,
    /// The description of the todo item.
    pub description: String,
    /// Whether the todo item is completed.
    #[serde(default)]
    pub completed: bool,
    /// The priority of the todo item.
    pub priority: Option<String>,
}

/// Tool for managing a todo list.
pub struct SetTodoListTool {
    items: Arc<RwLock<Vec<TodoItem>>>,
}

impl SetTodoListTool {
    /// Create a new SetTodoListTool instance.
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the current todo items.
    pub async fn get_items(&self) -> Vec<TodoItem> {
        self.items.read().await.clone()
    }

    /// Set the todo items.
    async fn set_items(&self, inputs: Vec<TodoItemInput>) -> Vec<TodoItem> {
        let mut items = self.items.write().await;
        let mut next_id = 1;

        *items = inputs
            .into_iter()
            .map(|input| {
                let id = input.id.unwrap_or(next_id);
                if id >= next_id {
                    next_id = id + 1;
                }

                TodoItem {
                    id,
                    description: input.description,
                    completed: input.completed,
                    priority: input.priority,
                }
            })
            .collect();

        items.clone()
    }

    /// Format the todo list as a string.
    fn format_todo_list(&self, items: &[TodoItem]) -> String {
        if items.is_empty() {
            return "No todo items.".to_string();
        }

        let mut lines = vec!["Todo List:".to_string()];
        lines.push("-".repeat(40));

        for item in items {
            let status = if item.completed { "[x]" } else { "[ ]" };
            let priority = item
                .priority
                .as_ref()
                .map(|p| format!(" ({p})"))
                .unwrap_or_default();
            lines.push(format!("{} {}{}: {}", status, item.id, priority, item.description));
        }

        lines.push("-".repeat(40));
        let completed = items.iter().filter(|i| i.completed).count();
        lines.push(format!("Progress: {}/{} completed", completed, items.len()));

        lines.join("\n")
    }
}

impl Default for SetTodoListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SetTodoListTool {
    fn name(&self) -> &str {
        "SetTodoList"
    }

    fn description(&self) -> &str {
        "Set the todo list for the agent. Use this to track tasks and progress."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "description": "The list of todo items to set",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "integer",
                                "description": "The ID of the todo item (optional, will be auto-assigned if not provided)"
                            },
                            "description": {
                                "type": "string",
                                "description": "The description of the todo item"
                            },
                            "completed": {
                                "type": "boolean",
                                "description": "Whether the todo item is completed"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["low", "medium", "high"],
                                "description": "The priority of the todo item"
                            }
                        },
                        "required": ["description"]
                    }
                }
            },
            "required": ["items"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: SetTodoListParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        // Set the items
        let items = self.set_items(params.items).await;

        // Format output
        let output = self.format_todo_list(&items);

        Ok(ToolOutput::new(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_todo_list() {
        let tool = SetTodoListTool::new();
        assert_eq!(tool.name(), "SetTodoList");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_set_and_get_items() {
        let tool = SetTodoListTool::new();

        let params = serde_json::json!({
            "items": [
                {
                    "description": "Task 1",
                    "completed": false,
                    "priority": "high"
                },
                {
                    "description": "Task 2",
                    "completed": true
                }
            ]
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());

        let output = result.unwrap().output;
        assert!(output.contains("Task 1"));
        assert!(output.contains("Task 2"));
        assert!(output.contains("high"));
    }
}
