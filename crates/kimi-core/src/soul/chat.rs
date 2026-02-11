//! Chat processing for LLM interactions
//!
//! This module handles the actual chat processing between the user and the LLM,
//! including message building, streaming responses, tool calling, and wire protocol integration.

use crate::context::Context;
use crate::soul::{KimiSoul, SoulError, WireSoulSide};
use crate::types::UserInput;
use crate::wire::WireMessage;
use futures::StreamExt;
use kosong_rs::{ChatProvider, Message as KosongMessage, Role as KosongRole};
use kosong_rs::chat_provider::ToolDefinition;
use tracing::{debug, info, warn};

/// Process a user message through the LLM with tool support
pub async fn process_message(
    soul: &mut KimiSoul,
    provider: &dyn ChatProvider,
    user_input: UserInput,
    wire: &WireSoulSide,
) -> Result<String, SoulError> {
    // Add user message to context
    soul.context.add_message(crate::types::Message {
        role: crate::types::Role::User,
        content: user_input.text.clone(),
        metadata: None,
    });

    // Process with potential tool call loops (max 5 iterations)
    let max_iterations = 5;
    for iteration in 0..max_iterations {
        let result = process_single_turn(soul, provider, wire).await?;
        
        match result {
            TurnResult::Complete(response) => {
                return Ok(response);
            }
            TurnResult::ToolCallsExecuted => {
                // Continue to next iteration to get final response
                debug!("Tool calls executed, continuing to iteration {}", iteration + 1);
                continue;
            }
        }
    }
    
    Err(SoulError::MaxIterations)
}

/// Result of a single turn
enum TurnResult {
    /// Turn completed with final response
    Complete(String),
    /// Tool calls were executed, need another turn
    ToolCallsExecuted,
}

/// Process a single turn (one LLM call)
async fn process_single_turn(
    soul: &mut KimiSoul,
    provider: &dyn ChatProvider,
    wire: &WireSoulSide,
) -> Result<TurnResult, SoulError> {
    // Build messages from context
    let messages = build_messages(&soul.context);

    // Get system prompt from agent
    let system_prompt = if soul.agent.system_prompt.is_empty() {
        None
    } else {
        Some(soul.agent.system_prompt.as_str())
    };

    // Convert toolset to ToolDefinitions
    let tools = if soul.toolset.tool_count() > 0 {
        Some(build_tool_definitions(&soul.toolset))
    } else {
        None
    };

    // Generate response with tools
    let mut stream = provider
        .generate_with_tools(system_prompt, &messages, tools.as_deref())
        .await
        .map_err(|e| SoulError::Llm(e.to_string()))?;

    // Stream response back through wire and collect full text
    let mut full_response = String::new();
    let mut pending_tool_calls = Vec::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(kosong_rs::StreamChunk::Text(text)) => {
                wire.send(WireMessage::TextPart { text: text.clone() })
                    .await
                    .map_err(|e| SoulError::Wire(e.to_string()))?;
                full_response.push_str(&text);
            }
            Ok(kosong_rs::StreamChunk::ToolCall(tool_call)) => {
                debug!("Received tool call: {:?}", tool_call);
                pending_tool_calls.push(tool_call);
            }
            Ok(kosong_rs::StreamChunk::ToolCallPart(_part)) => {
                // Tool call parts are accumulated by the provider
                // We only receive complete ToolCalls, so we can ignore parts here
                debug!("Received tool call part (accumulated by provider)");
            }
            Err(e) => {
                return Err(SoulError::Llm(e.to_string()));
            }
        }
    }

    // Process any tool calls
    if !pending_tool_calls.is_empty() {
        info!("Processing {} tool calls", pending_tool_calls.len());
        
        // Add assistant message with tool calls to context
        soul.context.add_message(crate::types::Message {
            role: crate::types::Role::Assistant,
            content: full_response.clone(),
            metadata: Some({
                let mut map = std::collections::HashMap::new();
                map.insert("tool_calls".to_string(), serde_json::json!(pending_tool_calls));
                map
            }),
        });

        // Execute tool calls and collect results
        for tool_call in pending_tool_calls {
            let result = execute_tool_call(soul, &tool_call, wire).await?;
            
            // Add tool result to context
            soul.context.add_message(crate::types::Message {
                role: crate::types::Role::Tool,
                content: result,
                metadata: None,
            });
        }

        return Ok(TurnResult::ToolCallsExecuted);
    }

    // Add assistant message to context
    soul.context.add_message(crate::types::Message {
        role: crate::types::Role::Assistant,
        content: full_response.clone(),
        metadata: None,
    });

    Ok(TurnResult::Complete(full_response))
}

/// Build tool definitions from the soul's toolset
fn build_tool_definitions(toolset: &crate::soul::KimiToolset) -> Vec<ToolDefinition> {
    toolset
        .schemas()
        .iter()
        .filter_map(|schema| {
            // Parse the schema which should be in the format:
            // { "type": "function", "function": { "name": "...", "description": "...", "parameters": {...} } }
            let function = schema.get("function")?;
            let name = function.get("name")?.as_str()?.to_string();
            let description = function.get("description")?.as_str()?.to_string();
            let parameters = function.get("parameters")?.clone();

            Some(ToolDefinition::new(name, description, parameters))
        })
        .collect()
}

/// Execute a tool call and return the result
async fn execute_tool_call(
    soul: &mut KimiSoul,
    tool_call: &kosong_rs::ToolCall,
    wire: &WireSoulSide,
) -> Result<String, SoulError> {
    let tool_name = &tool_call.function.name;
    info!("Executing tool: {} (id: {})", tool_name, tool_call.id);

    // Check if tool exists
    if !soul.toolset.contains(tool_name) {
        let error_msg = format!("Tool not found: {}", tool_name);
        warn!("{}", error_msg);
        return Ok(error_msg);
    }

    // Parse arguments for approval description
    let params: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
        .map_err(|e| SoulError::Tool(format!("Invalid tool arguments: {}", e)))?;

    // Build approval description based on tool and params
    let description = build_approval_description(tool_name, &params);

    // Request approval (unless in yolo mode)
    let request_id = uuid::Uuid::new_v4().to_string();
    let approval_request = crate::types::Request {
        id: request_id.clone(),
        tool_call_id: tool_call.id.clone(),
        sender: "kimi".to_string(),
        action: tool_name.clone(),
        description: description.clone(),
    };

    let approval_kind = soul.approval.request(approval_request).await;
    
    match approval_kind {
        crate::types::ApprovalKind::Reject => {
            info!("Tool {} rejected by user", tool_name);
            return Ok(format!("Tool '{}' was rejected by user approval", tool_name));
        }
        crate::types::ApprovalKind::ApproveOnce => {
            // ApproveOnce is treated as Approve for a single tool call
        }
        crate::types::ApprovalKind::Approve => {
            // Continue with execution
        }
    }

    // Send tool begin message
    wire.send(WireMessage::ToolBegin {
        name: tool_name.clone(),
        arguments: tool_call.function.arguments.clone(),
    }).await.map_err(|e| SoulError::Wire(e.to_string()))?;

    // Execute the tool
    let result = match soul.toolset.execute(tool_name, params).await {
        Ok(output) => {
            let output_str = serde_json::to_string(&output)
                .unwrap_or_else(|_| output.to_string());
            info!("Tool {} executed successfully", tool_name);
            output_str
        }
        Err(e) => {
            let error_msg = format!("Tool execution failed: {}", e);
            warn!("{}", error_msg);
            error_msg
        }
    };

    // Send tool end message
    wire.send(WireMessage::ToolEnd {
        name: tool_name.clone(),
        result: result.clone(),
    }).await.map_err(|e| SoulError::Wire(e.to_string()))?;

    Ok(result)
}

/// Build a human-readable description for approval request
fn build_approval_description(tool_name: &str, params: &serde_json::Value) -> String {
    match tool_name {
        "WriteFile" => {
            let path = params.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
            let mode = params.get("mode").and_then(|m| m.as_str()).unwrap_or("overwrite");
            format!("Write file '{}' ({})", path, mode)
        }
        "StrReplaceFile" => {
            let path = params.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
            format!("Edit file '{}'", path)
        }
        "Shell" => {
            let command = params.get("command").and_then(|c| c.as_str()).unwrap_or("unknown");
            // Truncate long commands
            let cmd_display = if command.len() > 60 {
                format!("{}...", &command[..60])
            } else {
                command.to_string()
            };
            format!("Execute shell command: {}", cmd_display)
        }
        "ReadFile" => {
            let path = params.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
            format!("Read file '{}'", path)
        }
        "Glob" => {
            let pattern = params.get("pattern").and_then(|p| p.as_str()).unwrap_or("unknown");
            format!("Search files matching '{}'", pattern)
        }
        "Grep" => {
            let pattern = params.get("pattern").and_then(|p| p.as_str()).unwrap_or("unknown");
            format!("Search for pattern '{}'", pattern)
        }
        "Task" => {
            let desc = params.get("description").and_then(|d| d.as_str()).unwrap_or("unknown");
            format!("Spawn subagent task: {}", desc)
        }
        _ => format!("Execute tool '{}'", tool_name),
    }
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
