//! OpenAI-compatible chat provider implementation.
//!
//! This module provides a [`ChatProvider`] implementation for OpenAI-compatible APIs,
//! including OpenAI's official API and compatible endpoints like Azure OpenAI,
//! LocalAI, Ollama, etc.
//!
//! # Example
//!
//! ```rust,no_run
//! use kosong_rs::{OpenAiProvider, ChatProvider, Message, Role};
//! use futures::StreamExt;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let provider = OpenAiProvider::new("your-api-key", "gpt-4o")?;
//!
//! let messages = vec![Message::user("Hello!")];
//! let mut stream = provider.generate(None, &messages).await?;
//!
//! while let Some(chunk) = stream.next().await {
//!     print!("{}", chunk?);
//! }
//! # Ok(())
//! # }
//! ```

use super::{ChatError, ChatProvider, ChatOptions, GenerateStream, ModelCapability, ThinkingEffort};
use crate::message::{Message, ToolCall};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use crate::tooling::Tool;

/// The base URL for the OpenAI API.
pub const OPENAI_API_BASE: &str = "https://api.openai.com/v1";

/// OpenAI-compatible chat provider.
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
    options: ChatOptions,
    thinking_effort: ThinkingEffort,
    capabilities: Vec<ModelCapability>,
    tools: Option<Vec<ToolDefinition>>,
}

/// Definition of a tool for the OpenAI API.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolDefinition {
    /// The type of tool (typically "function").
    #[serde(rename = "type")]
    tool_type: String,
    /// The function definition.
    function: FunctionDefinition,
}

/// Definition of a function for the OpenAI API.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionDefinition {
    /// The name of the function.
    name: String,
    /// A description of what the function does.
    description: String,
    /// The JSON schema for the function parameters.
    parameters: serde_json::Value,
}

impl OpenAiProvider {
    /// Creates a new OpenAI provider with the given API key and model.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your OpenAI API key.
    /// * `model` - The model name (e.g., "gpt-4o", "gpt-4o-mini").
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new<S: Into<String>>(api_key: S, model: S) -> Result<Self, ChatError> {
        Self::with_options(api_key, model, ChatOptions::default())
    }

    /// Creates a new OpenAI provider with custom options.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your OpenAI API key.
    /// * `model` - The model name.
    /// * `options` - Additional chat options.
    pub fn with_options<S: Into<String>>(
        api_key: S,
        model: S,
        options: ChatOptions,
    ) -> Result<Self, ChatError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| ChatError::Config(format!("Failed to create HTTP client: {}", e)))?;

        let model_str = model.into();
        let capabilities = Self::infer_capabilities(&model_str);

        Ok(Self {
            client,
            api_key: api_key.into(),
            model: model_str,
            base_url: OPENAI_API_BASE.to_string(),
            options,
            thinking_effort: ThinkingEffort::default(),
            capabilities,
            tools: None,
        })
    }

    /// Creates a new OpenAI provider with a custom base URL.
    ///
    /// This is useful for using Azure OpenAI, LocalAI, Ollama, or other
    /// OpenAI-compatible endpoints.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use kosong_rs::OpenAiProvider;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = OpenAiProvider::with_base_url(
    ///     "your-api-key",
    ///     "llama2",
    ///     "http://localhost:11434/v1"
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_base_url<S: Into<String>>(
        api_key: S,
        model: S,
        base_url: S,
    ) -> Result<Self, ChatError> {
        let mut provider = Self::new(api_key, model)?;
        provider.base_url = base_url.into();
        Ok(provider)
    }

    /// Infers model capabilities based on the model name.
    fn infer_capabilities(model: &str) -> Vec<ModelCapability> {
        let mut caps = vec![
            ModelCapability::Streaming,
            ModelCapability::ToolCalling,
        ];

        // Vision models
        if model.contains("gpt-4o")
            || model.contains("gpt-4-turbo")
            || model.contains("vision")
            || model.contains("claude-3")
        {
            caps.push(ModelCapability::Vision);
        }

        // Models with JSON mode support
        if model.contains("gpt-4")
            || model.contains("gpt-3.5-turbo")
            || model.contains("claude-3")
        {
            caps.push(ModelCapability::JsonMode);
        }

        // o1 models support thinking
        if model.starts_with("o1") || model.starts_with("o3") {
            caps.push(ModelCapability::Thinking);
        }

        caps
    }

    /// Returns the API base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Sets the chat options.
    pub fn set_options(&mut self, options: ChatOptions) {
        self.options = options;
    }

    /// Adds tools to the provider for function calling.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use kosong_rs::{OpenAiProvider, tooling::Tool};
    /// use async_trait::async_trait;
    /// use serde_json::json;
    ///
    /// struct WeatherTool;
    ///
    /// #[async_trait]
    /// impl Tool for WeatherTool {
    ///     fn name(&self) -> &str { "get_weather" }
    ///     fn description(&self) -> &str { "Get weather for a location" }
    ///     fn parameters_schema(&self) -> serde_json::Value {
    ///         json!({
    ///             "type": "object",
    ///             "properties": {
    ///                 "location": {"type": "string"}
    ///             },
    ///             "required": ["location"]
    ///         })
    ///     }
    ///     async fn execute(&self, params: serde_json::Value) -> Result<String, kosong_rs::tooling::ToolError> {
    ///         Ok("Sunny".to_string())
    ///     }
    /// }
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut provider = OpenAiProvider::new("key", "gpt-4")?;
    /// provider.with_tools(vec![Box::new(WeatherTool)]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_tools<T: Tool>(&mut self, tools: Vec<Box<T>>) {
        let tool_defs: Vec<ToolDefinition> = tools
            .iter()
            .map(|tool| ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    parameters: tool.parameters_schema(),
                },
            })
            .collect();
        
        self.tools = Some(tool_defs);
    }

    /// Sets tools from a slice of tool trait objects.
    pub fn set_tools(&mut self, tools: &[&dyn Tool]) {
        let tool_defs: Vec<ToolDefinition> = tools
            .iter()
            .map(|tool| ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    parameters: tool.parameters_schema(),
                },
            })
            .collect();
        
        self.tools = Some(tool_defs);
    }

    fn build_headers(&self) -> Result<HeaderMap, ChatError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .map_err(|e| ChatError::Config(format!("Invalid API key: {}", e)))?,
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        Ok(headers)
    }

    fn build_request_body(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
        tools: Option<&[super::ToolDefinition]>,
    ) -> serde_json::Value {
        let mut msgs = Vec::new();

        // Add system message if provided
        if let Some(prompt) = system_prompt {
            msgs.push(serde_json::json!({
                "role": "system",
                "content": prompt
            }));
        }

        // Add conversation messages
        for msg in messages {
            msgs.push(serde_json::to_value(msg).unwrap());
        }

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": msgs,
            "stream": self.options.stream,
        });

        // Add optional parameters
        if let Some(max_tokens) = self.options.max_tokens {
            body["max_tokens"] = max_tokens.into();
        }

        if let Some(temperature) = self.options.temperature {
            body["temperature"] = temperature.into();
        }

        if let Some(top_p) = self.options.top_p {
            body["top_p"] = top_p.into();
        }

        if let Some(freq_penalty) = self.options.frequency_penalty {
            body["frequency_penalty"] = freq_penalty.into();
        }

        if let Some(pres_penalty) = self.options.presence_penalty {
            body["presence_penalty"] = pres_penalty.into();
        }

        if let Some(stop) = &self.options.stop {
            body["stop"] = stop.clone().into();
        }

        if let Some(format) = &self.options.response_format {
            body["response_format"] = serde_json::json!({
                "type": format.type_str()
            });
        }

        // Add tools if provided or configured
        if let Some(tools) = tools {
            body["tools"] = serde_json::to_value(tools).unwrap();
        } else if let Some(tools) = &self.tools {
            body["tools"] = serde_json::to_value(tools).unwrap();
        }

        body
    }
}

#[async_trait]
impl ChatProvider for OpenAiProvider {
    async fn generate_with_tools(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
        tools: Option<&[super::ToolDefinition]>,
    ) -> Result<GenerateStream, ChatError> {
        let url = format!("{}/chat/completions", self.base_url);
        let headers = self.build_headers()?;
        let body = self.build_request_body(system_prompt, messages, tools);

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChatError::Api {
                status,
                message: error_text,
            });
        }

        let stream = response.bytes_stream();
        let stream = process_stream(stream);

        Ok(Box::pin(stream))
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn with_thinking(&self, effort: ThinkingEffort) -> Box<dyn ChatProvider> {
        let mut new_provider = self.clone();
        new_provider.thinking_effort = effort;
        Box::new(new_provider)
    }

    fn capabilities(&self) -> &[ModelCapability] {
        &self.capabilities
    }
}

/// Processes the SSE stream from OpenAI API.
fn process_stream(
    stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<String, ChatError>> + Send + 'static {
    stream
        .map(|result| {
            result.map_err(ChatError::Request).and_then(|bytes| {
                String::from_utf8(bytes.to_vec())
                    .map_err(|e| ChatError::Parse(format!("Invalid UTF-8: {}", e)))
            })
        })
        .flat_map(|text_result| {
            let chunks = match text_result {
                Ok(text) => parse_sse_chunks(&text),
                Err(e) => vec![Err(e)],
            };
            futures::stream::iter(chunks)
        })
}

/// Parses SSE (Server-Sent Events) chunks from the OpenAI API.
fn parse_sse_chunks(text: &str) -> Vec<Result<String, ChatError>> {
    let mut results = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        // Parse data lines
        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();

            // Check for stream end
            if data == "[DONE]" {
                break;
            }

            // Parse the JSON chunk
            match parse_chunk_json(data) {
                Ok(Some(content)) => results.push(Ok(content)),
                Ok(None) => {} // No content in this chunk
                Err(e) => results.push(Err(e)),
            }
        }
    }

    results
}

/// Parses a single SSE data chunk.
fn parse_chunk_json(data: &str) -> Result<Option<String>, ChatError> {
    let chunk: StreamChunk = serde_json::from_str(data)
        .map_err(|e| ChatError::Parse(format!("Failed to parse chunk: {} - {}", e, data)))?;

    // Extract content from delta
    if let Some(choice) = chunk.choices.first() {
        if let Some(delta) = &choice.delta {
            if let Some(content) = &delta.content {
                return Ok(Some(content.clone()));
            }
        }
    }

    Ok(None)
}

/// A chunk from the streaming response.
#[derive(Debug, Deserialize)]
struct StreamChunk {
    /// The chunk ID.
    #[allow(dead_code)]
    id: String,
    /// The object type.
    #[allow(dead_code)]
    object: String,
    /// The choices in this chunk.
    choices: Vec<StreamChoice>,
}

/// A choice within a stream chunk.
#[derive(Debug, Deserialize)]
struct StreamChoice {
    /// The index of this choice.
    #[allow(dead_code)]
    index: u32,
    /// The delta content.
    delta: Option<StreamDelta>,
    /// The finish reason if this is the last chunk.
    #[serde(rename = "finish_reason")]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

/// The delta content within a choice.
#[derive(Debug, Deserialize)]
struct StreamDelta {
    /// The role (typically only in first chunk).
    #[allow(dead_code)]
    role: Option<String>,
    /// The content text.
    content: Option<String>,
    /// Tool calls if present.
    #[serde(rename = "tool_calls")]
    #[allow(dead_code)]
    tool_calls: Option<Vec<ToolCall>>,
}

/// Non-streaming response from the OpenAI API.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChatCompletionResponse {
    /// The response ID.
    #[allow(dead_code)]
    id: String,
    /// The object type.
    #[allow(dead_code)]
    object: String,
    /// The Unix timestamp of the response.
    #[allow(dead_code)]
    created: u64,
    /// The model used.
    #[allow(dead_code)]
    model: String,
    /// The completion choices.
    choices: Vec<CompletionChoice>,
    /// Token usage information.
    #[allow(dead_code)]
    usage: Option<Usage>,
}

/// A completion choice in the non-streaming response.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CompletionChoice {
    /// The index of this choice.
    #[allow(dead_code)]
    index: u32,
    /// The generated message.
    message: ResponseMessage,
    /// The finish reason.
    #[serde(rename = "finish_reason")]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

/// A message in the API response.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ResponseMessage {
    /// The role of the message.
    #[allow(dead_code)]
    role: String,
    /// The content of the message.
    content: Option<String>,
    /// Tool calls if present.
    #[serde(rename = "tool_calls")]
    #[allow(dead_code)]
    tool_calls: Option<Vec<ToolCall>>,
}

/// Token usage information.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Usage {
    /// Tokens in the prompt.
    #[allow(dead_code)]
    prompt_tokens: u32,
    /// Tokens in the completion.
    #[allow(dead_code)]
    completion_tokens: u32,
    /// Total tokens used.
    #[allow(dead_code)]
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_capabilities_gpt4o() {
        let caps = OpenAiProvider::infer_capabilities("gpt-4o");
        assert!(caps.contains(&ModelCapability::Streaming));
        assert!(caps.contains(&ModelCapability::ToolCalling));
        assert!(caps.contains(&ModelCapability::Vision));
        assert!(caps.contains(&ModelCapability::JsonMode));
    }

    #[test]
    fn test_infer_capabilities_gpt35() {
        let caps = OpenAiProvider::infer_capabilities("gpt-3.5-turbo");
        assert!(caps.contains(&ModelCapability::Streaming));
        assert!(caps.contains(&ModelCapability::ToolCalling));
        assert!(!caps.contains(&ModelCapability::Vision));
        assert!(caps.contains(&ModelCapability::JsonMode));
    }

    #[test]
    fn test_infer_capabilities_o1() {
        let caps = OpenAiProvider::infer_capabilities("o1-preview");
        assert!(caps.contains(&ModelCapability::Streaming));
        assert!(caps.contains(&ModelCapability::ToolCalling));
        assert!(caps.contains(&ModelCapability::Thinking));
    }

    #[test]
    fn test_parse_sse_chunks() {
        let sse_data = r#"data: {"id":"chat-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}

data: {"id":"chat-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: [DONE]"#;

        let results = parse_sse_chunks(sse_data);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].as_ref().unwrap(), "Hello");
        assert_eq!(results[1].as_ref().unwrap(), " world");
    }

    #[test]
    fn test_build_request_body() {
        let provider = OpenAiProvider::new("test-key", "gpt-4o").unwrap();
        let messages = vec![Message::user("Hello")];
        
        let body = provider.build_request_body(Some("Be helpful"), &messages, None);
        
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], true);
        assert!(body["messages"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn test_with_base_url() {
        let provider = OpenAiProvider::with_base_url(
            "test-key",
            "llama2",
            "http://localhost:11434/v1"
        ).unwrap();
        
        assert_eq!(provider.base_url(), "http://localhost:11434/v1");
    }

    #[test]
    fn test_thinking_effort() {
        let provider = OpenAiProvider::new("test-key", "gpt-4o").unwrap();
        let thinking_provider = provider.with_thinking(ThinkingEffort::High);
        
        assert_eq!(thinking_provider.model_name(), "gpt-4o");
    }
}
