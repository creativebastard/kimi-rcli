//! Chat processing for LLM interactions
//!
//! This module handles the actual chat processing between the user and the LLM,
//! including message building, streaming responses, and wire protocol integration.

use crate::context::Context;
use crate::soul::{KimiSoul, SoulError, WireSoulSide};
use crate::types::UserInput;
use crate::wire::WireMessage;
use futures::StreamExt;
use kosong_rs::{ChatProvider, Message as KosongMessage, Role as KosongRole};

/// Process a user message through the LLM
pub async fn process_message(
    soul: &KimiSoul,
    provider: &dyn ChatProvider,
    user_input: UserInput,
    wire: &WireSoulSide,
) -> Result<String, SoulError> {
    // Build messages from context
    let mut messages = build_messages(&soul.context);

    // Add user message
    messages.push(KosongMessage::user(user_input.text));

    // Get system prompt from agent
    let system_prompt = if soul.agent.system_prompt.is_empty() {
        None
    } else {
        Some(soul.agent.system_prompt.as_str())
    };

    // Generate response
    let mut stream = provider
        .generate(system_prompt, &messages)
        .await
        .map_err(|e| SoulError::Llm(e.to_string()))?;

    // Stream response back through wire and collect full text
    let mut full_response = String::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(text) => {
                wire.send(WireMessage::TextPart { text: text.clone() })
                    .await
                    .map_err(|e| SoulError::Wire(e.to_string()))?;
                full_response.push_str(&text);
            }
            Err(e) => {
                return Err(SoulError::Llm(e.to_string()));
            }
        }
    }

    Ok(full_response)
}

/// Build message history from context
fn build_messages(context: &Context) -> Vec<KosongMessage> {
    // Convert context messages to kosong messages
    context
        .messages()
        .iter()
        .map(|msg| {
            let role = match msg.role {
                crate::types::Role::User => KosongRole::User,
                crate::types::Role::Assistant => KosongRole::Assistant,
                crate::types::Role::System => KosongRole::System,
                crate::types::Role::Tool => KosongRole::Tool,
            };
            KosongMessage::new(role, msg.content.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Role;
    use std::path::PathBuf;

    fn create_test_context() -> Context {
        let mut context = Context::new(PathBuf::from("/tmp/test_chat_context.json"));
        context.add_message(crate::types::Message {
            role: Role::System,
            content: "You are a helpful assistant.".to_string(),
            metadata: None,
        });
        context.add_message(crate::types::Message {
            role: Role::User,
            content: "Hello".to_string(),
            metadata: None,
        });
        context.add_message(crate::types::Message {
            role: Role::Assistant,
            content: "Hi there!".to_string(),
            metadata: None,
        });
        context
    }

    #[test]
    fn test_build_messages() {
        let context = create_test_context();
        let messages = build_messages(&context);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, KosongRole::System);
        assert_eq!(messages[0].text(), Some("You are a helpful assistant.".to_string()));
        assert_eq!(messages[1].role, KosongRole::User);
        assert_eq!(messages[1].text(), Some("Hello".to_string()));
        assert_eq!(messages[2].role, KosongRole::Assistant);
        assert_eq!(messages[2].text(), Some("Hi there!".to_string()));
    }

    #[test]
    fn test_build_messages_empty_context() {
        let context = Context::new(PathBuf::from("/tmp/test_empty.json"));
        let messages = build_messages(&context);
        assert!(messages.is_empty());
    }
}
