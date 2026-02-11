//! Kimi API provider implementation.
//!
//! This module provides a [`ChatProvider`] implementation for Moonshot AI's Kimi API.

use crate::chat_provider::{
    ChatError, ChatOptions, ChatProvider, GenerateStream, ModelCapability, ThinkingEffort,
};
use crate::message::{Message, ToolCall};
use async_trait::async_trait;
use futures::{stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};


/// Default base URL for the Kimi API.
const KIMI_API_BASE: HeaderValue = HeaderValue::from_static("https://api.moonshot.cn/v1");

/// The Kimi API provider.
#[derive(Debug, Clone)]
pub struct KimiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
    options: ChatOptions,
    thinking_effort: ThinkingEffort,
    capabilities: Vec<ModelCapability>,
}

/// Request body for the Kimi API.
#[derive(Debug, Serialize)]
struct KimiRequest {
    model: String,
    messages: Vec<KimiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<super::ToolDefinition>>,
}

/// Response format for structured outputs.
#[derive(Debug, Serialize)]
struct ResponseFormat {
    r#type: String,
}

/// A message in the Kimi API format.
#[derive(Debug, Serialize, Deserialize)]
struct KimiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// Response from the Kimi API (non-streaming).
#[derive(Debug, Deserialize)]
struct KimiResponse {
    choices: Vec<KimiChoice>,
}

/// A choice in the Kimi API response.
#[derive(Debug, Deserialize)]
struct KimiChoice {
    message: KimiMessage,
    finish_reason: Option<String>,
}

/// A streaming chunk from the Kimi API.
#[derive(Debug, Deserialize)]
struct KimiStreamChunk {
    choices: Vec<KimiStreamChoice>,
}

/// A choice within a streaming chunk.
#[derive(Debug, Deserialize)]
struct KimiStreamChoice {
    delta: KimiDelta,
    finish_reason: Option<String>,
}

/// The delta content in a streaming chunk.
#[derive(Debug, Deserialize, Default)]
struct KimiDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

impl KimiProvider {
    /// Creates a new Kimi provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Moonshot AI API key.
    /// * `model` - The model name (e.g., "kimi-k2-0711-preview").
    /// * `base_url` - Optional custom base URL for the API.
    ///
    /// # Example
    ///
    /// ```
    /// use kosong_rs::KimiProvider;
    ///
    /// // With default base URL
    /// let provider = KimiProvider::new("your-api-key", "kimi-k2-0711-preview", None::<&str>).unwrap();
    ///
    /// // With custom base URL
    /// let provider = KimiProvider::new("your-api-key", "kimi-k2-0711-preview", Some("https://custom.api.com/v1")).unwrap();
    /// ```
    pub fn new<S: Into<String>>(
        api_key: S,
        model: S,
        base_url: Option<S>,
    ) -> Result<Self, ChatError> {
        let mut provider = Self::with_options(api_key, model, ChatOptions::default())?;
        if let Some(url) = base_url {
            provider.base_url = url.into();
        }
        Ok(provider)
    }

    /// Creates a new Kimi provider with custom options.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Moonshot AI API key.
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
            base_url: KIMI_API_BASE.to_str().unwrap().to_string(),
            options,
            thinking_effort: ThinkingEffort::default(),
            capabilities,
        })
    }

    /// Creates a new Kimi provider with a custom base URL.
    ///
    /// This is useful for using a proxy or a compatible endpoint.
    pub fn with_base_url<S: Into<String>>(
        api_key: S,
        model: S,
        base_url: S,
    ) -> Result<Self, ChatError> {
        let mut provider = Self::with_options(api_key, model, ChatOptions::default())?;
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
        if model.contains("kimi-k2")
            || model.contains("vision")
        {
            caps.push(ModelCapability::Vision);
        }

        // K2 models support thinking
        if model.contains("kimi-k2") {
            caps.push(ModelCapability::Thinking);
        }

        // K2 models support JSON mode
        if model.contains("kimi-k2") {
            caps.push(ModelCapability::JsonMode);
        }

        caps
    }

    /// Converts a generic message to the Kimi API format.
    fn convert_message(msg: &Message) -> KimiMessage {
        KimiMessage {
            role: msg.role.as_str().to_string(),
            content: msg.text(),
            tool_calls: msg.tool_calls.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            name: msg.name.clone(),
        }
    }

    /// Builds the request headers.
    fn build_headers(&self) -> Result<HeaderMap, ChatError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let auth_value = format!("Bearer {}", self.api_key);
        let auth_header = HeaderValue::from_str(&auth_value)
            .map_err(|e| ChatError::Config(format!("Invalid API key: {}", e)))?;
        headers.insert(AUTHORIZATION, auth_header);

        // User-Agent is critical for Kimi Code API access
        // Format: KimiCLI/{VERSION}
        let user_agent = format!("KimiCLI/{}", env!("CARGO_PKG_VERSION"));
        headers.insert("User-Agent", HeaderValue::from_str(&user_agent).unwrap_or_else(|_| HeaderValue::from_static("KimiCLI/0.1.0")));

        // Add Kimi CLI identification headers for API access
        headers.insert("X-Msh-Platform", HeaderValue::from_static("kimi_cli"));
        headers.insert("X-Msh-Version", HeaderValue::from_static(env!("CARGO_PKG_VERSION")));
        
        // Device identification headers - use actual system info
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());
        
        if let Ok(device_name) = HeaderValue::from_str(&hostname) {
            headers.insert("X-Msh-Device-Name", device_name);
        }
        
        // Device model: OS + Architecture
        let device_model = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
        if let Ok(device_model_header) = HeaderValue::from_str(&device_model) {
            headers.insert("X-Msh-Device-Model", device_model_header);
        }
        
        // OS Version
        if let Ok(os_version) = HeaderValue::from_str(&sysinfo::System::kernel_version().unwrap_or_default()) {
            headers.insert("X-Msh-Os-Version", os_version);
        }
        
        // Device ID - use the stored device ID
        let device_id = crate::chat_provider::get_device_id();
        if let Ok(device_id_header) = HeaderValue::from_str(&device_id) {
            headers.insert("X-Msh-Device-Id", device_id_header);
        }

        Ok(headers)
    }

    /// Builds the request body.
    fn build_request_body(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
        tools: Option<&[super::ToolDefinition]>,
    ) -> KimiRequest {
        let mut kimi_messages: Vec<KimiMessage> = messages
            .iter()
            .map(|msg| Self::convert_message(msg))
            .collect();

        // Prepend system message if provided
        if let Some(system) = system_prompt {
            kimi_messages.insert(
                0,
                KimiMessage {
                    role: "system".to_string(),
                    content: Some(system.to_string()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
            );
        }

        KimiRequest {
            model: self.model.clone(),
            messages: kimi_messages,
            max_tokens: self.options.max_tokens,
            temperature: self.options.temperature,
            top_p: self.options.top_p,
            frequency_penalty: self.options.frequency_penalty,
            presence_penalty: self.options.presence_penalty,
            stop: self.options.stop.clone(),
            stream: self.options.stream,
            response_format: self.options.response_format.as_ref().map(|f| ResponseFormat {
                r#type: f.type_str().to_string(),
            }),
            tools: tools.map(|t| t.to_vec()),
        }
    }
}

#[async_trait]
impl ChatProvider for KimiProvider {
    async fn generate_with_tools(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
        tools: Option<&[super::ToolDefinition]>,
    ) -> Result<GenerateStream, ChatError> {
        let headers = self.build_headers()?;
        let body = self.build_request_body(system_prompt, messages, tools);

        let url = format!("{}/chat/completions", self.base_url);

        tracing::debug!("Sending request to {}", url);

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(ChatError::Request)?;
        
        tracing::debug!("Response status: {}", response.status());

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ChatError::Api {
                status: status.as_u16(),
                message: error_text,
            });
        }

        // For non-streaming, we'd parse the full response
        // For streaming, we process the SSE stream
        if !self.options.stream {
            let kimi_response: KimiResponse = response.json().await.map_err(ChatError::Request)?;
            let text = kimi_response
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .unwrap_or_default();

            // Create a single-item stream
            let stream = stream::once(async move { Ok(text) });
            return Ok(Box::pin(stream));
        }

        // Handle streaming response with proper SSE parsing
        tracing::trace!("Processing streaming response...");
        
        // Use unfold to maintain state (buffer) across chunks
        let stream = futures::stream::unfold(
            (response.bytes_stream(), String::new()),
            |(mut byte_stream, mut buffer)| async move {
                loop {
                    // Try to process any complete lines in the buffer first
                    if let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer.drain(..=newline_pos).collect::<String>();
                        let line = line.trim_end();
                        
                        tracing::trace!("Processing line: {}", line);
                        
                        if line.starts_with("data:") {
                            let data = line[5..].trim_start();
                            if data == "[DONE]" {
                                tracing::debug!("Received [DONE]");
                                return None; // End of stream
                            }
                            
                            tracing::trace!("Parsing JSON chunk");
                            match serde_json::from_str::<KimiStreamChunk>(data) {
                                Ok(chunk) => {
                                    if let Some(choice) = chunk.choices.into_iter().next() {
                                        // Check for content first
                                        if let Some(content) = choice.delta.content {
                                            if !content.is_empty() {
                                                return Some((Ok(content), (byte_stream, buffer)));
                                            }
                                        }
                                        // TODO: Handle tool_calls in delta
                                        // Skip reasoning_content for now
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to parse chunk: {}", e);
                                    // Continue to next line on parse error
                                }
                            }
                        }
                        // Continue to process more lines
                        continue;
                    }
                    
                    // No complete line in buffer, fetch more data
                    match byte_stream.next().await {
                        Some(Ok(bytes)) => {
                            let text = String::from_utf8_lossy(&bytes);
                            tracing::trace!("Received bytes: {}", text);
                            buffer.push_str(&text);
                            // Continue the loop to try processing again
                        }
                        Some(Err(e)) => {
                            tracing::error!("Stream error: {}", e);
                            return Some((Err(ChatError::Request(e)), (byte_stream, buffer)));
                        }
                        None => {
                            // End of byte stream
                            return None;
                        }
                    }
                }
            }
        );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kimi_provider_new() {
        let provider = KimiProvider::new("test-key", "kimi-k2-0711-preview", None::<&str>);
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.model, "kimi-k2-0711-preview");
        assert_eq!(provider.api_key, "test-key");
    }

    #[test]
    fn test_kimi_provider_with_base_url() {
        let provider = KimiProvider::with_base_url(
            "test-key",
            "kimi-k2-0711-preview",
            "https://custom.api.com/v1",
        );
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.base_url, "https://custom.api.com/v1");
    }

    #[test]
    fn test_infer_capabilities_k2() {
        let caps = KimiProvider::infer_capabilities("kimi-k2-0711-preview");
        assert!(caps.contains(&ModelCapability::Streaming));
        assert!(caps.contains(&ModelCapability::ToolCalling));
        assert!(caps.contains(&ModelCapability::Vision));
        assert!(caps.contains(&ModelCapability::Thinking));
        assert!(caps.contains(&ModelCapability::JsonMode));
    }

    #[test]
    fn test_infer_capabilities_basic() {
        let caps = KimiProvider::infer_capabilities("kimi-basic");
        assert!(caps.contains(&ModelCapability::Streaming));
        assert!(caps.contains(&ModelCapability::ToolCalling));
        assert!(!caps.contains(&ModelCapability::Vision));
        assert!(!caps.contains(&ModelCapability::Thinking));
    }

    #[test]
    fn test_convert_message() {
        let msg = Message::user("Hello");
        let kimi_msg = KimiProvider::convert_message(&msg);
        assert_eq!(kimi_msg.role, "user");
        assert_eq!(kimi_msg.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_build_request_body() {
        let provider = KimiProvider::new("test-key", "kimi-k2-0711-preview", None::<&str>).unwrap();
        let messages = vec![Message::user("Hello")];
        let body = provider.build_request_body(Some("You are helpful"), &messages, None);

        assert_eq!(body.model, "kimi-k2-0711-preview");
        assert_eq!(body.messages.len(), 2); // system + user
        assert_eq!(body.messages[0].role, "system");
        assert_eq!(body.messages[1].role, "user");
    }

    #[test]
    fn test_build_request_body_with_tools() {
        use crate::chat_provider::ToolDefinition;
        
        let provider = KimiProvider::new("test-key", "kimi-k2-0711-preview", None::<&str>).unwrap();
        let messages = vec![Message::user("Hello")];
        let tools = vec![
            ToolDefinition::new(
                "read_file",
                "Read a file from disk",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                })
            )
        ];
        let body = provider.build_request_body(Some("You are helpful"), &messages, Some(&tools));

        assert_eq!(body.model, "kimi-k2-0711-preview");
        assert!(body.tools.is_some());
        assert_eq!(body.tools.unwrap().len(), 1);
    }
}
