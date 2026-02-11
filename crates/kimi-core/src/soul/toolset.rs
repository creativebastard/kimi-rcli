//! Toolset implementation for the agent system
//!
//! Provides tool registration, schema generation, and execution capabilities
//! with support for both built-in tools and MCP (Model Context Protocol) servers.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info};

/// Errors that can occur during tool execution
#[derive(Debug, Error, Clone)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("Execution failed: {0}")]
    Execution(String),
    #[error("MCP server error: {0}")]
    McpServer(String),
    #[error("Timeout")]
    Timeout,
    #[error("Cancelled")]
    Cancelled,
}

/// Result type for tool execution
pub type ToolResult = Result<Value, ToolError>;

/// Tool trait for implementing tools
#[async_trait]
pub trait Tool: Send + Sync + Debug {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the JSON schema for the tool's parameters
    fn parameters_schema(&self) -> Value;

    /// Execute the tool with the given parameters
    async fn execute(&self, params: Value) -> ToolResult;
}

/// Information about an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// Server name
    pub name: String,
    /// Server command
    pub command: String,
    /// Server arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: Option<HashMap<String, String>>,
    /// Available tools from this server
    pub tools: Vec<String>,
    /// Whether the server is connected
    #[serde(skip)]
    pub connected: bool,
}

impl McpServerInfo {
    /// Create new MCP server info
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            env: None,
            tools: Vec::new(),
            connected: false,
        }
    }

    /// Add arguments to the server
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add environment variables to the server
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }
}

/// The main toolset for managing and executing tools
#[derive(Debug, Clone)]
pub struct KimiToolset {
    /// Built-in tools
    tools: HashMap<String, Arc<dyn Tool>>,
    /// MCP server information
    mcp_servers: HashMap<String, McpServerInfo>,
    /// Tool schemas cache
    schemas: Vec<Value>,
}

impl KimiToolset {
    /// Create a new empty toolset
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            mcp_servers: HashMap::new(),
            schemas: Vec::new(),
        }
    }

    /// Create a toolset with default tools
    pub fn with_defaults() -> Self {
        let mut toolset = Self::new();
        toolset.register_defaults();
        toolset
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        debug!("Registering tool: {}", name);
        self.tools.insert(name.clone(), tool);
        self.refresh_schemas();
    }

    /// Register multiple tools
    pub fn register_many(&mut self, tools: Vec<Arc<dyn Tool>>) {
        for tool in tools {
            self.register(tool);
        }
    }

    /// Unregister a tool
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn Tool>> {
        let tool = self.tools.remove(name);
        if tool.is_some() {
            self.refresh_schemas();
        }
        tool
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool exists
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Execute a tool by name with parameters
    pub async fn execute(&self, name: &str, params: Value) -> ToolResult {
        debug!("Executing tool: {} with params: {:?}", name, params);
        
        let tool = self.tools.get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        
        tool.execute(params).await
    }

    /// Get all tool schemas
    pub fn schemas(&self) -> &[Value] {
        &self.schemas
    }

    /// Get all tool names
    pub fn tool_names(&self) -> impl Iterator<Item = &String> {
        self.tools.keys()
    }

    /// Get tool count
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Register an MCP server
    pub fn register_mcp_server(&mut self, server: McpServerInfo) {
        info!("Registering MCP server: {}", server.name);
        self.mcp_servers.insert(server.name.clone(), server);
    }

    /// Get an MCP server by name
    pub fn get_mcp_server(&self, name: &str) -> Option<&McpServerInfo> {
        self.mcp_servers.get(name)
    }

    /// Get all MCP servers
    pub fn mcp_servers(&self) -> &HashMap<String, McpServerInfo> {
        &self.mcp_servers
    }

    /// Check if an MCP server exists
    pub fn has_mcp_server(&self, name: &str) -> bool {
        self.mcp_servers.contains_key(name)
    }

    /// Remove an MCP server
    pub fn remove_mcp_server(&mut self, name: &str) -> Option<McpServerInfo> {
        self.mcp_servers.remove(name)
    }

    /// Refresh the schemas cache
    fn refresh_schemas(&mut self) {
        self.schemas = self.tools.values()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters_schema(),
                    }
                })
            })
            .collect();
    }

    /// Register default built-in tools
    fn register_defaults(&mut self) {
        // Placeholder for default tools
        // These would be registered here in a full implementation
        debug!("Registering default tools (placeholder)");
    }
}

impl Default for KimiToolset {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool call representation from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,
    /// Tool name
    pub name: String,
    /// Tool arguments as JSON string
    pub arguments: String,
}

impl ToolCall {
    /// Parse the arguments as JSON
    pub fn parse_arguments(&self) -> Result<Value, ToolError> {
        serde_json::from_str(&self.arguments)
            .map_err(|e| ToolError::InvalidParameters(format!("Invalid JSON: {}", e)))
    }

    /// Create a new tool call
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }
}

/// Tool call result for returning to the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Tool call ID
    pub tool_call_id: String,
    /// Result output
    pub output: String,
    /// Whether the result is an error
    pub is_error: bool,
}

impl ToolCallResult {
    /// Create a successful result
    pub fn success(tool_call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            output: output.into(),
            is_error: false,
        }
    }

    /// Create an error result
    pub fn error(tool_call_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            output: error.into(),
            is_error: true,
        }
    }
}

/// A simple tool implementation for testing
pub struct SimpleTool {
    name: String,
    description: String,
    parameters: Value,
    handler: Arc<dyn Fn(Value) -> ToolResult + Send + Sync>,
}

impl std::fmt::Debug for SimpleTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("parameters", &self.parameters)
            .finish_non_exhaustive()
    }
}

impl SimpleTool {
    /// Create a new simple tool
    pub fn new<F>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> ToolResult + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            handler: Arc::new(handler),
        }
    }
}

#[async_trait]
impl Tool for SimpleTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.parameters.clone()
    }

    async fn execute(&self, params: Value) -> ToolResult {
        (self.handler)(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_toolset_new() {
        let toolset = KimiToolset::new();
        assert_eq!(toolset.tool_count(), 0);
        assert!(toolset.schemas().is_empty());
    }

    #[tokio::test]
    async fn test_toolset_register() {
        let mut toolset = KimiToolset::new();
        
        let tool = Arc::new(SimpleTool::new(
            "test_tool",
            "A test tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }),
            |params| {
                let input = params.get("input")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                Ok(serde_json::json!({"result": input}))
            },
        ));
        
        toolset.register(tool);
        
        assert_eq!(toolset.tool_count(), 1);
        assert!(toolset.contains("test_tool"));
        assert!(!toolset.contains("other_tool"));
    }

    #[tokio::test]
    async fn test_toolset_execute() {
        let mut toolset = KimiToolset::new();
        
        let tool = Arc::new(SimpleTool::new(
            "echo",
            "Echo the input",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                }
            }),
            |params| {
                let msg = params.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(serde_json::json!({"echo": msg}))
            },
        ));
        
        toolset.register(tool);
        
        let result = toolset.execute("echo", serde_json::json!({"message": "hello"})).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!({"echo": "hello"}));
    }

    #[tokio::test]
    async fn test_toolset_execute_not_found() {
        let toolset = KimiToolset::new();
        
        let result = toolset.execute("nonexistent", serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_toolset_unregister() {
        let mut toolset = KimiToolset::new();
        
        let tool = Arc::new(SimpleTool::new(
            "temp_tool",
            "A temporary tool",
            serde_json::json!({"type": "object"}),
            |_params| Ok(serde_json::json!({})),
        ));
        
        toolset.register(tool);
        assert!(toolset.contains("temp_tool"));
        
        toolset.unregister("temp_tool");
        assert!(!toolset.contains("temp_tool"));
    }

    #[test]
    fn test_tool_call() {
        let call = ToolCall::new("call-1", "test_tool", r#"{"input": "hello"}"#);
        assert_eq!(call.id, "call-1");
        assert_eq!(call.name, "test_tool");
        
        let args = call.parse_arguments().unwrap();
        assert_eq!(args["input"], "hello");
    }

    #[test]
    fn test_tool_call_invalid_json() {
        let call = ToolCall::new("call-1", "test_tool", "invalid json");
        let result = call.parse_arguments();
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_call_result() {
        let success = ToolCallResult::success("call-1", "success output");
        assert_eq!(success.tool_call_id, "call-1");
        assert_eq!(success.output, "success output");
        assert!(!success.is_error);
        
        let error = ToolCallResult::error("call-2", "error message");
        assert_eq!(error.tool_call_id, "call-2");
        assert_eq!(error.output, "error message");
        assert!(error.is_error);
    }

    #[test]
    fn test_mcp_server_info() {
        let server = McpServerInfo::new("test-server", "test-command")
            .with_args(vec!["arg1".to_string(), "arg2".to_string()])
            .with_env(HashMap::from([("KEY".to_string(), "VALUE".to_string())]));
        
        assert_eq!(server.name, "test-server");
        assert_eq!(server.command, "test-command");
        assert_eq!(server.args, vec!["arg1", "arg2"]);
        assert!(server.env.is_some());
    }

    #[test]
    fn test_toolset_mcp_server() {
        let mut toolset = KimiToolset::new();
        
        let server = McpServerInfo::new("test-server", "test-command");
        toolset.register_mcp_server(server);
        
        assert!(toolset.has_mcp_server("test-server"));
        assert!(!toolset.has_mcp_server("other-server"));
        
        let retrieved = toolset.get_mcp_server("test-server");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-server");
    }
}
