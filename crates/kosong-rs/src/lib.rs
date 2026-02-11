//! # kosong-rs
//!
//! An LLM abstraction layer for AI agent applications.
//!
//! This crate provides a unified interface for interacting with various LLM providers
//! including Moonshot AI's Kimi API and OpenAI-compatible endpoints.
//!
//! ## Features
//!
//! - **Unified ChatProvider trait** - Abstract interface for LLM providers
//! - **Streaming responses** - Real-time token streaming support
//! - **Tool calling** - Function calling capabilities for agents
//! - **Multiple providers** - Kimi and OpenAI-compatible implementations
//!
//! ## Example
//!
//! ```rust,no_run
//! use kosong_rs::{ChatProvider, KimiProvider, Message, Role, StreamChunk};
//! use futures::StreamExt;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let provider = KimiProvider::new("your-api-key", "kimi-k2-0711-preview", None::<&str>)?;
//!
//! let messages = vec![
//!     Message::new(Role::User, "Hello, how are you?"),
//! ];
//!
//! let mut stream = provider.generate(None, &messages).await?;
//! while let Some(chunk) = stream.next().await {
//!     match chunk? {
//!         StreamChunk::Text(text) => print!("{}", text),
//!         StreamChunk::ToolCall(tool_call) => println!("Tool call: {:?}", tool_call),
//!         StreamChunk::ToolCallPart(_) => {}, // Parts are accumulated by the provider
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod chat_provider;
pub mod message;
pub mod tooling;

// Re-export main types for convenience
pub use chat_provider::{ChatProvider, ChatError, GenerateStream, StreamChunk, ModelCapability, ThinkingEffort};
pub use chat_provider::kimi::KimiProvider;
pub use chat_provider::openai::OpenAiProvider;
pub use message::{ContentPart, Message, Role, ToolCall, ToolCallPart, ToolResult};
pub use tooling::{Tool, Toolset, ToolError as ToolingError};

// Re-export async_trait for users implementing custom providers
pub use async_trait::async_trait;
