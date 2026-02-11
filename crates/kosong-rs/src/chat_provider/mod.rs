//! Chat provider abstractions for LLM interactions.
//!
//! This module defines the core [`ChatProvider`] trait and related types
//! for implementing LLM provider clients.

use crate::message::Message;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use thiserror::Error;

/// A tool definition for function calling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    /// The type of tool (typically "function").
    pub r#type: String,
    /// The function definition.
    pub function: FunctionDefinition,
}

/// A function definition for tool calling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionDefinition {
    /// The name of the function.
    pub name: String,
    /// A description of what the function does.
    pub description: String,
    /// The JSON schema for the function's parameters.
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition from a name, description, and parameters schema.
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: serde_json::Value) -> Self {
        Self {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

/// Errors that can occur during chat operations.
#[derive(Error, Debug)]
pub enum ChatError {
    /// An error occurred while making the HTTP request.
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// An error occurred while parsing the response.
    #[error("Failed to parse response: {0}")]
    Parse(String),

    /// The API returned an error response.
    #[error("API error: {message}")]
    Api {
        /// The HTTP status code.
        status: u16,
        /// The error message from the API.
        message: String,
    },

    /// An error occurred while serializing/deserializing JSON.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The stream ended unexpectedly.
    #[error("Stream ended unexpectedly")]
    StreamEnded,

    /// An invalid configuration was provided.
    #[error("Invalid configuration: {0}")]
    Config(String),

    /// A generic error with a message.
    #[error("{0}")]
    Other(String),
}

/// A stream of generated text chunks.
pub type GenerateStream = Pin<Box<dyn Stream<Item = Result<String, ChatError>> + Send>>;

/// Capabilities that a model may support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCapability {
    /// Support for streaming responses.
    Streaming,
    /// Support for function/tool calling.
    ToolCalling,
    /// Support for vision/image inputs.
    Vision,
    /// Support for JSON mode/structured outputs.
    JsonMode,
    /// Support for thinking/reasoning outputs.
    Thinking,
}

impl ModelCapability {
    /// Returns the string representation of the capability.
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelCapability::Streaming => "streaming",
            ModelCapability::ToolCalling => "tool_calling",
            ModelCapability::Vision => "vision",
            ModelCapability::JsonMode => "json_mode",
            ModelCapability::Thinking => "thinking",
        }
    }
}

impl std::fmt::Display for ModelCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The level of thinking effort to use for reasoning models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThinkingEffort {
    /// Minimal thinking, fastest response.
    Low,
    /// Balanced thinking (default).
    #[default]
    Medium,
    /// Maximum thinking, most thorough.
    High,
}

impl ThinkingEffort {
    /// Returns the string representation of the thinking effort.
    pub fn as_str(&self) -> &'static str {
        match self {
            ThinkingEffort::Low => "low",
            ThinkingEffort::Medium => "medium",
            ThinkingEffort::High => "high",
        }
    }
}

impl std::fmt::Display for ThinkingEffort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The core trait for LLM chat providers.
///
/// Implement this trait to add support for a new LLM provider.
/// All methods are thread-safe (Send + Sync).
#[async_trait]
pub trait ChatProvider: Send + Sync {
    /// Generates a streaming response from the model.
    ///
    /// # Arguments
    ///
    /// * `system_prompt` - Optional system instructions for the model.
    /// * `messages` - The conversation history.
    ///
    /// # Returns
    ///
    /// A stream of text chunks that can be consumed asynchronously.
    ///
    /// # Errors
    ///
    /// Returns a [`ChatError`] if the request fails or the response cannot be parsed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// let mut stream = provider.generate(None, &messages).await?;
    /// while let Some(chunk) = stream.next().await {
    ///     match chunk {
    ///         Ok(text) => print!("{}", text),
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// ```
    async fn generate(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
    ) -> Result<GenerateStream, ChatError> {
        // Default implementation delegates to generate_with_tools without tools
        self.generate_with_tools(system_prompt, messages, None).await
    }

    /// Generates a streaming response from the model with optional tool support.
    ///
    /// # Arguments
    ///
    /// * `system_prompt` - Optional system instructions for the model.
    /// * `messages` - The conversation history.
    /// * `tools` - Optional list of tool definitions for function calling.
    ///
    /// # Returns
    ///
    /// A stream of text chunks that can be consumed asynchronously.
    /// When the model makes tool calls, they will be included in the message stream.
    ///
    /// # Errors
    ///
    /// Returns a [`ChatError`] if the request fails or the response cannot be parsed.
    async fn generate_with_tools(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<GenerateStream, ChatError>;

    /// Returns the model name used by this provider.
    fn model_name(&self) -> &str;

    /// Returns a new provider instance with the specified thinking effort.
    ///
    /// This is useful for reasoning models that support different levels
    /// of thinking effort (low, medium, high).
    fn with_thinking(&self, effort: ThinkingEffort) -> Box<dyn ChatProvider>;

    /// Returns the capabilities supported by this model.
    fn capabilities(&self) -> &[ModelCapability];

    /// Returns true if the model supports the given capability.
    fn has_capability(&self, capability: ModelCapability) -> bool {
        self.capabilities().contains(&capability)
    }

    /// Returns true if the model supports streaming.
    fn supports_streaming(&self) -> bool {
        self.has_capability(ModelCapability::Streaming)
    }

    /// Returns true if the model supports tool calling.
    fn supports_tools(&self) -> bool {
        self.has_capability(ModelCapability::ToolCalling)
    }

    /// Returns true if the model supports vision inputs.
    fn supports_vision(&self) -> bool {
        self.has_capability(ModelCapability::Vision)
    }
}

/// Configuration options for chat completion requests.
#[derive(Debug, Clone)]
pub struct ChatOptions {
    /// The maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// The sampling temperature (0.0 to 2.0).
    pub temperature: Option<f32>,
    /// The nucleus sampling parameter (0.0 to 1.0).
    pub top_p: Option<f32>,
    /// Penalty for repeating tokens.
    pub frequency_penalty: Option<f32>,
    /// Penalty for new tokens based on presence.
    pub presence_penalty: Option<f32>,
    /// Stop sequences to end generation.
    pub stop: Option<Vec<String>>,
    /// Whether to stream the response.
    pub stream: bool,
    /// Response format (e.g., "json_object").
    pub response_format: Option<ResponseFormat>,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            max_tokens: None,
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: true,
            response_format: None,
        }
    }
}

impl ChatOptions {
    /// Creates a new ChatOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Sets the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets the top_p parameter.
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Sets whether to stream the response.
    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    /// Sets the response format.
    pub fn with_response_format(mut self, format: ResponseFormat) -> Self {
        self.response_format = Some(format);
        self
    }
}

/// Response format options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseFormat {
    /// Standard text response.
    Text,
    /// JSON object response.
    JsonObject,
    /// JSON schema-constrained response.
    JsonSchema {
        /// The name of the schema.
        name: String,
        /// The JSON schema definition.
        schema: serde_json::Value,
        /// Whether to strictly enforce the schema.
        strict: bool,
    },
}

impl ResponseFormat {
    /// Returns the type string for the response format.
    pub fn type_str(&self) -> &'static str {
        match self {
            ResponseFormat::Text => "text",
            ResponseFormat::JsonObject => "json_object",
            ResponseFormat::JsonSchema { .. } => "json_schema",
        }
    }
}

/// Get a unique device ID for this installation.
/// 
/// This generates a persistent device ID that is stored in the user's data directory.
/// The device ID is used for API authentication and tracking.
pub fn get_device_id() -> String {
    use std::fs;
    use std::path::PathBuf;
    
    // Try to get the data directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kimi");
    
    let device_id_file = data_dir.join("device_id");
    
    // Try to read existing device ID
    if let Ok(existing) = fs::read_to_string(&device_id_file) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    
    // Generate a new device ID (32 hex characters, no dashes)
    let new_id = uuid::Uuid::new_v4().to_string().replace("-", "");
    
    // Try to save it
    if let Err(e) = fs::create_dir_all(&data_dir) {
        eprintln!("Warning: Failed to create data directory: {}", e);
        return new_id;
    }
    
    if let Err(e) = fs::write(&device_id_file, &new_id) {
        eprintln!("Warning: Failed to save device ID: {}", e);
    }
    
    new_id
}

pub mod kimi;
pub mod openai;

// Re-export provider implementations
pub use kimi::KimiProvider;
pub use openai::OpenAiProvider;
