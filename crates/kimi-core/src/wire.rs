//! Wire protocol for agent communication

use crate::types::{ApprovalKind, TokenUsage, UserInput};
use serde::{Deserialize, Serialize};

/// Wire message types for agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WireMessage {
    /// Begin a new turn with user input
    TurnBegin { user_input: UserInput },
    /// End of a turn
    TurnEnd,
    /// Begin a step with iteration number
    StepBegin { n: usize },
    /// Step was interrupted
    StepInterrupted,
    /// Begin context compaction
    CompactionBegin,
    /// End context compaction
    CompactionEnd,
    /// Text content part
    TextPart { text: String },
    /// Thinking content part
    ThinkPart { text: String },
    /// Image URL content part
    ImageUrlPart { url: String },
    /// Audio URL content part
    AudioUrlPart { url: String },
    /// Video URL content part
    VideoUrlPart { url: String },
    /// Complete tool call
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    /// Partial tool call (streaming)
    ToolCallPart {
        id: String,
        name: String,
        arguments: String,
    },
    /// Tool execution result
    ToolResult {
        tool_call_id: String,
        output: String,
        is_error: bool,
    },
    /// Request for approval
    ApprovalRequest {
        id: String,
        tool_call_id: String,
        sender: String,
        action: String,
        description: String,
    },
    /// Response to an approval request
    ApprovalResponse {
        request_id: String,
        response: ApprovalKind,
    },
    /// Status update with usage statistics
    StatusUpdate {
        context_usage: Option<f64>,
        token_usage: Option<TokenUsage>,
        message_id: Option<String>,
    },
    /// Event from a subagent
    SubagentEvent {
        task_tool_call_id: String,
        event: Box<WireMessage>,
    },
}

impl WireMessage {
    /// Serialize the message to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize a message from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UserInput;

    #[test]
    fn test_wire_message_serialization() {
        let msg = WireMessage::TurnBegin {
            user_input: UserInput {
                text: "Hello".to_string(),
                attachments: vec![],
            },
        };

        let json = msg.to_json().unwrap();
        assert!(json.contains("TurnBegin"));
        assert!(json.contains("Hello"));

        let deserialized: WireMessage = WireMessage::from_json(&json).unwrap();
        match deserialized {
            WireMessage::TurnBegin { user_input } => {
                assert_eq!(user_input.text, "Hello");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_wire_message_tagged_serialization() {
        let msg = WireMessage::TextPart {
            text: "test content".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Check that the type tag is present
        assert_eq!(value.get("type").unwrap().as_str().unwrap(), "TextPart");
        // Check that the payload is nested
        assert_eq!(
            value.get("payload").unwrap().get("text").unwrap().as_str().unwrap(),
            "test content"
        );
    }
}
