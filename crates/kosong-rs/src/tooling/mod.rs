//! Tool system for LLM function calling.
//!
//! This module provides abstractions for defining and executing tools
//! that can be called by LLMs. Tools are functions with a schema that
//! describes their parameters.
//!
//! # Example
//!
//! ```rust
//! use kosong_rs::tooling::{Tool, ToolError};
//! use async_trait::async_trait;
//! use serde_json::json;
//!
//! struct CalculatorTool;
//!
//! #[async_trait]
//! impl Tool for CalculatorTool {
//!     fn name(&self) -> &str {
//!         "calculator"
//!     }
//!
//!     fn description(&self) -> &str {
//!         "Perform basic arithmetic operations"
//!     }
//!
//!     fn parameters_schema(&self) -> serde_json::Value {
//!         json!({
//!             "type": "object",
//!             "properties": {
//!                 "operation": {
//!                     "type": "string",
//!                     "enum": ["add", "subtract", "multiply", "divide"]
//!                 },
//!                 "a": { "type": "number" },
//!                 "b": { "type": "number" }
//!             },
//!             "required": ["operation", "a", "b"]
//!         })
//!     }
//!
//!     async fn execute(&self, params: serde_json::Value) -> Result<String, ToolError> {
//!         let op = params["operation"].as_str().ok_or_else(|| {
//!             ToolError::InvalidParameters("Missing 'operation' parameter".to_string())
//!         })?;
//!         
//!         let a = params["a"].as_f64().ok_or_else(|| {
//!             ToolError::InvalidParameters("Missing or invalid 'a' parameter".to_string())
//!         })?;
//!         
//!         let b = params["b"].as_f64().ok_or_else(|| {
//!             ToolError::InvalidParameters("Missing or invalid 'b' parameter".to_string())
//!         })?;
//!
//!         let result = match op {
//!             "add" => a + b,
//!             "subtract" => a - b,
//!             "multiply" => a * b,
//!             "divide" => {
//!                 if b == 0.0 {
//!                     return Err(ToolError::Execution("Division by zero".to_string()));
//!                 }
//!                 a / b
//!             }
//!             _ => return Err(ToolError::InvalidParameters(format!("Unknown operation: {}", op))),
//!         };
//!
//!         Ok(result.to_string())
//!     }
//! }
//! ```

use crate::message::ToolResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during tool operations.
#[derive(Error, Debug, Clone)]
pub enum ToolError {
    /// The provided parameters are invalid or missing.
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// An error occurred during tool execution.
    #[error("Execution failed: {0}")]
    Execution(String),

    /// The tool is not available or not found.
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// A timeout occurred during execution.
    #[error("Tool execution timed out")]
    Timeout,

    /// A generic error with a message.
    #[error("{0}")]
    Other(String),
}

/// The result of executing a tool.
pub type ToolExecutionResult = Result<String, ToolError>;

/// A trait for tools that can be called by LLMs.
///
/// Implement this trait to create custom tools that LLMs can invoke.
/// The tool's schema describes its parameters to the LLM.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the unique name of this tool.
    ///
    /// This name is used by the LLM to identify which tool to call.
    fn name(&self) -> &str;

    /// Returns a description of what this tool does.
    ///
    /// This description helps the LLM understand when and how to use the tool.
    fn description(&self) -> &str;

    /// Returns the JSON schema for this tool's parameters.
    ///
    /// The schema should follow the JSON Schema specification and describe
    /// all parameters the tool accepts, including their types and which are required.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Executes the tool with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - The parameters for the tool call, as a JSON value.
    ///
    /// # Returns
    ///
    /// The result of the tool execution as a string, or an error if execution fails.
    async fn execute(&self, params: serde_json::Value) -> ToolExecutionResult;

    /// Returns the tool definition as a JSON object.
    ///
    /// This is the format expected by OpenAI-compatible APIs.
    fn to_definition(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": self.parameters_schema()
            }
        })
    }
}

/// A collection of tools that can be used together.
///
/// This trait allows grouping multiple tools and provides methods
/// for looking up tools by name.
pub trait Toolset: Send + Sync {
    /// Returns all tools in this toolset.
    fn tools(&self) -> &[Box<dyn Tool>];

    /// Finds a tool by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to find.
    ///
    /// # Returns
    ///
    /// Some reference to the tool if found, None otherwise.
    fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools()
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    /// Checks if a tool with the given name exists in this toolset.
    fn has_tool(&self, name: &str) -> bool {
        self.tools().iter().any(|t| t.name() == name)
    }

    /// Returns the number of tools in this toolset.
    fn len(&self) -> usize {
        self.tools().len()
    }

    /// Returns true if this toolset contains no tools.
    fn is_empty(&self) -> bool {
        self.tools().is_empty()
    }

    /// Returns all tool definitions as a JSON array.
    fn to_definitions(&self) -> Vec<serde_json::Value> {
        self.tools().iter().map(|t| t.to_definition()).collect()
    }

}

/// A simple in-memory toolset.
#[derive(Default)]
pub struct SimpleToolset {
    tools: Vec<Box<dyn Tool>>,
}

impl std::fmt::Debug for SimpleToolset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleToolset")
            .field("tool_count", &self.tools.len())
            .field("tool_names", &self.tools.iter().map(|t| t.name()).collect::<Vec<_>>())
            .finish()
    }
}

impl SimpleToolset {
    /// Creates a new empty toolset.
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Creates a new toolset with the given tools.
    pub fn with_tools(tools: Vec<Box<dyn Tool>>) -> Self {
        Self { tools }
    }

    /// Adds a tool to this toolset.
    pub fn add_tool<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.push(Box::new(tool));
    }

    /// Adds multiple tools to this toolset.
    pub fn add_tools(&mut self, tools: Vec<Box<dyn Tool>>) {
        self.tools.extend(tools);
    }

    /// Removes a tool by name.
    pub fn remove_tool(&mut self, name: &str) -> Option<Box<dyn Tool>> {
        if let Some(index) = self.tools.iter().position(|t| t.name() == name) {
            Some(self.tools.remove(index))
        } else {
            None
        }
    }

    /// Executes a tool by name with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to execute.
    /// * `params` - The parameters for the tool call.
    ///
    /// # Returns
    ///
    /// The result of the tool execution, or an error if the tool is not found
    /// or execution fails.
    pub async fn execute_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> ToolExecutionResult {
        match self.get_tool(name) {
            Some(tool) => tool.execute(params).await,
            None => Err(ToolError::NotFound(name.to_string())),
        }
    }
}

impl Toolset for SimpleToolset {
    fn tools(&self) -> &[Box<dyn Tool>] {
        &self.tools
    }
}

/// A tool call request from an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// The unique identifier for this tool call.
    pub id: String,
    /// The type of tool call (typically "function").
    #[serde(rename = "type")]
    pub call_type: String,
    /// The function call details.
    pub function: FunctionCallRequest,
}

/// A function call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallRequest {
    /// The name of the function to call.
    pub name: String,
    /// The JSON-encoded arguments.
    pub arguments: String,
}

impl ToolCallRequest {
    /// Parses the arguments as JSON.
    pub fn parse_arguments(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.function.arguments)
    }

    /// Executes this tool call against a SimpleToolset.
    ///
    /// # Arguments
    ///
    /// * `toolset` - The toolset to execute against.
    ///
    /// # Returns
    ///
    /// A ToolResult containing the execution result.
    pub async fn execute(&self, toolset: &SimpleToolset) -> ToolResult {
        let params = match self.parse_arguments() {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::error(
                    &self.id,
                    &self.function.name,
                    &format!("Failed to parse arguments: {}", e),
                );
            }
        };

        match toolset.execute_tool(&self.function.name, params).await {
            Ok(content) => ToolResult::new(&self.id, &self.function.name, &content),
            Err(e) => ToolResult::error(&self.id, &self.function.name, &e.to_string()),
        }
    }
}

/// A builder for creating tool definitions.
#[derive(Debug, Default)]
pub struct ToolBuilder {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

impl ToolBuilder {
    /// Creates a new tool builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the tool name.
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = name.into();
        self
    }

    /// Sets the tool description.
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = description.into();
        self
    }

    /// Sets the tool parameters schema.
    pub fn parameters(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }

    /// Builds the tool definition as JSON.
    pub fn build(self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A simple test tool
    struct TestTool;

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test_tool"
        }

        fn description(&self) -> &str {
            "A test tool"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "value": { "type": "string" }
                }
            })
        }

        async fn execute(&self, params: serde_json::Value) -> ToolExecutionResult {
            let value = params["value"].as_str().unwrap_or("default");
            Ok(format!("Result: {}", value))
        }
    }

    #[test]
    fn test_tool_definition() {
        let tool = TestTool;
        let def = tool.to_definition();

        assert_eq!(def["type"], "function");
        assert_eq!(def["function"]["name"], "test_tool");
        assert_eq!(def["function"]["description"], "A test tool");
    }

    #[test]
    fn test_simple_toolset() {
        let mut toolset = SimpleToolset::new();
        toolset.add_tool(TestTool);

        assert_eq!(toolset.len(), 1);
        assert!(toolset.has_tool("test_tool"));
        assert!(!toolset.has_tool("nonexistent"));

        let tool = toolset.get_tool("test_tool");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "test_tool");
    }

    #[test]
    fn test_tool_builder() {
        let def = ToolBuilder::new()
            .name("my_tool")
            .description("Does something")
            .parameters(serde_json::json!({
                "type": "object",
                "properties": {}
            }))
            .build();

        assert_eq!(def["function"]["name"], "my_tool");
        assert_eq!(def["function"]["description"], "Does something");
    }

    #[test]
    fn test_tool_call_request() {
        let request = ToolCallRequest {
            id: "call_123".to_string(),
            call_type: "function".to_string(),
            function: FunctionCallRequest {
                name: "test_tool".to_string(),
                arguments: r#"{"value": "hello"}"#.to_string(),
            },
        };

        let args = request.parse_arguments().unwrap();
        assert_eq!(args["value"], "hello");
    }

    #[test]
    fn test_tool_error() {
        let err = ToolError::InvalidParameters("missing field".to_string());
        assert!(err.to_string().contains("Invalid parameters"));

        let err = ToolError::Execution("something went wrong".to_string());
        assert!(err.to_string().contains("Execution failed"));

        let err = ToolError::NotFound("my_tool".to_string());
        assert!(err.to_string().contains("Tool not found"));
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let tool = TestTool;
        let params = serde_json::json!({"value": "test"});
        
        let result = tool.execute(params).await;
        assert_eq!(result.unwrap(), "Result: test");
    }

    #[tokio::test]
    async fn test_toolset_execution() {
        let mut toolset = SimpleToolset::new();
        toolset.add_tool(TestTool);

        let params = serde_json::json!({"value": "from_toolset"});
        let result = toolset.execute_tool("test_tool", params).await;
        
        assert_eq!(result.unwrap(), "Result: from_toolset");

        let result = toolset.execute_tool("nonexistent", serde_json::json!({})).await;
        assert!(matches!(result, Err(ToolError::NotFound(_))));
    }
}
