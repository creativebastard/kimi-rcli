//! Wire channel - Single-producer multi-consumer channel for Soul/UI communication

use crate::wire::WireMessage;
use tokio::sync::broadcast;
use tracing::{debug, trace, warn};

/// Default channel capacity for broadcast channels
const DEFAULT_CAPACITY: usize = 1024;

/// Content part types that can be merged
#[derive(Debug, Clone)]
enum ContentPart {
    Text(String),
}

/// Main Wire channel for communication between Soul and UI
///
/// The Wire provides two broadcast channels:
/// - `raw_tx`: Unmerged messages for consumers that need every individual message
/// - `merged_tx`: Merged messages where consecutive TextParts are combined
#[derive(Debug, Clone)]
pub struct Wire {
    raw_tx: broadcast::Sender<WireMessage>,
    merged_tx: broadcast::Sender<WireMessage>,
}

impl Wire {
    /// Create a new Wire channel with default capacity
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a new Wire channel with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let (raw_tx, _) = broadcast::channel(capacity);
        let (merged_tx, _) = broadcast::channel(capacity);

        debug!("Created Wire channel with capacity {}", capacity);

        Self {
            raw_tx,
            merged_tx,
        }
    }

    /// Get the Soul side of the wire for sending messages
    pub fn soul_side(&self) -> WireSoulSide {
        WireSoulSide {
            raw_tx: self.raw_tx.clone(),
            merged_tx: self.merged_tx.clone(),
            merge_buffer: None,
        }
    }

    /// Get the UI side of the wire for receiving messages
    ///
    /// If `merge` is true, receives merged messages (consecutive TextParts combined).
    /// If `merge` is false, receives raw unmerged messages.
    pub fn ui_side(&self, merge: bool) -> WireUISide {
        let rx = if merge {
            self.merged_tx.subscribe()
        } else {
            self.raw_tx.subscribe()
        };

        WireUISide { rx }
    }

    /// Shut down the wire by closing all senders
    ///
    /// This will cause all receivers to eventually return `None`
    pub fn shutdown(&self) {
        debug!("Shutting down Wire channel");
        // Dropping all senders will close the channel
        // The senders are dropped when the Wire and all WireSoulSides are dropped
    }
}

impl Default for Wire {
    fn default() -> Self {
        Self::new()
    }
}

/// Soul side of the Wire - responsible for sending messages
///
/// The Soul side handles message merging for the merged channel,
/// combining consecutive TextParts to reduce message overhead.
pub struct WireSoulSide {
    raw_tx: broadcast::Sender<WireMessage>,
    merged_tx: broadcast::Sender<WireMessage>,
    merge_buffer: Option<ContentPart>,
}

impl WireSoulSide {
    /// Send a message through the wire
    ///
    /// The message is sent on the raw channel immediately.
    /// For the merged channel, consecutive TextParts are buffered and merged.
    pub fn send(&mut self, msg: WireMessage) {
        trace!("Sending message: {:?}", msg);

        // Always send raw message immediately
        if self.raw_tx.send(msg.clone()).is_err() {
            warn!("Failed to send to raw channel: no active receivers");
        }

        // Handle merging for the merged channel
        match &msg {
            WireMessage::TextPart { text } => {
                self.buffer_for_merge(ContentPart::Text(text.clone()));
            }
            WireMessage::ThinkPart { text: _ } => {
                self.flush();
                if self.merged_tx.send(msg).is_err() {
                    warn!("Failed to send to merged channel: no active receivers");
                }
            }
            other => {
                self.flush();
                if self.merged_tx.send(other.clone()).is_err() {
                    warn!("Failed to send to merged channel: no active receivers");
                }
            }
        }
    }

    /// Flush any buffered content to the merged channel
    ///
    /// This should be called when the Soul wants to ensure all
    /// buffered content is sent, such as at the end of a turn.
    pub fn flush(&mut self) {
        if let Some(part) = self.merge_buffer.take() {
            let msg = match part {
                ContentPart::Text(text) => WireMessage::TextPart { text },
            };

            trace!("Flushing buffered message: {:?}", msg);

            if self.merged_tx.send(msg).is_err() {
                warn!("Failed to send flushed message: no active receivers");
            }
        }
    }

    /// Buffer content for merging, flushing previous buffer if types don't match
    fn buffer_for_merge(&mut self, part: ContentPart) {
        match (&mut self.merge_buffer, &part) {
            (Some(ContentPart::Text(existing)), ContentPart::Text(new)) => {
                // Merge consecutive text parts
                existing.push_str(new);
                trace!("Merged text part, new length: {}", existing.len());
            }
            _ => {
                // Different type or empty buffer - flush and start new buffer
                self.flush();
                self.merge_buffer = Some(part);
            }
        }
    }
}

impl Drop for WireSoulSide {
    fn drop(&mut self) {
        // Ensure any buffered content is sent before dropping
        self.flush();
    }
}

/// UI side of the Wire - responsible for receiving messages
pub struct WireUISide {
    rx: broadcast::Receiver<WireMessage>,
}

impl WireUISide {
    /// Receive the next message from the wire
    ///
    /// Returns `Some(WireMessage)` if a message is received,
    /// or `None` if the channel is closed.
    pub async fn recv(&mut self) -> Option<WireMessage> {
        loop {
            match self.rx.recv().await {
                Ok(msg) => {
                    trace!("Received message: {:?}", msg);
                    return Some(msg);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Wire channel closed");
                    return None;
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    warn!("UI side lagged behind by {} messages", count);
                    // Continue to try to receive the next available message
                    continue;
                }
            }
        }
    }

    /// Try to receive a message without waiting
    ///
    /// Returns `Ok(Some(WireMessage))` if a message is available,
    /// `Ok(None)` if no message is available,
    /// or `Err(RecvError)` if the channel is closed.
    pub fn try_recv(&mut self) -> Result<Option<WireMessage>, broadcast::error::RecvError> {
        loop {
            match self.rx.try_recv() {
                Ok(msg) => {
                    trace!("Try-received message: {:?}", msg);
                    return Ok(Some(msg));
                }
                Err(broadcast::error::TryRecvError::Empty) => return Ok(None),
                Err(broadcast::error::TryRecvError::Closed) => {
                    return Err(broadcast::error::RecvError::Closed);
                }
                Err(broadcast::error::TryRecvError::Lagged(count)) => {
                    warn!("UI side lagged behind by {} messages", count);
                    // Try again after lag
                    continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UserInput;

    #[tokio::test]
    async fn test_wire_basic_send_recv() {
        let wire = Wire::new();
        let mut soul = wire.soul_side();
        let mut ui = wire.ui_side(false);

        soul.send(WireMessage::TurnBegin {
            user_input: UserInput {
                text: "Hello".to_string(),
                attachments: vec![],
            },
        });

        let msg = ui.recv().await;
        assert!(matches!(msg, Some(WireMessage::TurnBegin { .. })));
    }

    #[tokio::test]
    async fn test_wire_multiple_consumers() {
        let wire = Wire::new();
        let mut soul = wire.soul_side();
        let mut ui1 = wire.ui_side(false);
        let mut ui2 = wire.ui_side(false);

        soul.send(WireMessage::TextPart {
            text: "Hello".to_string(),
        });

        let msg1 = ui1.recv().await;
        let msg2 = ui2.recv().await;

        assert!(matches!(msg1, Some(WireMessage::TextPart { text }) if text == "Hello"));
        assert!(matches!(msg2, Some(WireMessage::TextPart { text }) if text == "Hello"));
    }

    #[tokio::test]
    async fn test_wire_text_merging() {
        let wire = Wire::new();
        let mut soul = wire.soul_side();
        let mut ui_raw = wire.ui_side(false);
        let mut ui_merged = wire.ui_side(true);

        // Send multiple text parts
        soul.send(WireMessage::TextPart {
            text: "Hello ".to_string(),
        });
        soul.send(WireMessage::TextPart {
            text: "World".to_string(),
        });
        soul.send(WireMessage::TextPart {
            text: "!".to_string(),
        });
        soul.flush();

        // Raw channel should have 3 separate messages
        let msg1 = ui_raw.recv().await;
        let msg2 = ui_raw.recv().await;
        let msg3 = ui_raw.recv().await;

        assert!(matches!(msg1, Some(WireMessage::TextPart { text }) if text == "Hello "));
        assert!(matches!(msg2, Some(WireMessage::TextPart { text }) if text == "World"));
        assert!(matches!(msg3, Some(WireMessage::TextPart { text }) if text == "!"));

        // Merged channel should have 1 combined message
        let merged = ui_merged.recv().await;
        assert!(
            matches!(merged, Some(WireMessage::TextPart { text }) if text == "Hello World!")
        );
    }

    #[tokio::test]
    async fn test_wire_non_text_flushes_buffer() {
        let wire = Wire::new();
        let mut soul = wire.soul_side();
        let mut ui_merged = wire.ui_side(true);

        // Send text then a non-text message
        soul.send(WireMessage::TextPart {
            text: "Hello ".to_string(),
        });
        soul.send(WireMessage::StepBegin { n: 1 });
        soul.send(WireMessage::TextPart {
            text: "World".to_string(),
        });
        soul.flush();

        // Should receive: TextPart, StepBegin, TextPart (not merged)
        let msg1 = ui_merged.recv().await;
        let msg2 = ui_merged.recv().await;
        let msg3 = ui_merged.recv().await;

        assert!(matches!(msg1, Some(WireMessage::TextPart { text }) if text == "Hello "));
        assert!(matches!(msg2, Some(WireMessage::StepBegin { n: 1 })));
        assert!(matches!(msg3, Some(WireMessage::TextPart { text }) if text == "World"));
    }

    #[tokio::test]
    async fn test_wire_drop_flushes() {
        let wire = Wire::new();
        let mut soul = wire.soul_side();
        let mut ui_merged = wire.ui_side(true);

        // Send text without flushing
        soul.send(WireMessage::TextPart {
            text: "Hello".to_string(),
        });

        // Drop the soul side
        drop(soul);

        // Should still receive the buffered message
        let msg = ui_merged.recv().await;
        assert!(matches!(msg, Some(WireMessage::TextPart { text }) if text == "Hello"));
    }

    #[tokio::test]
    async fn test_wire_channel_closed() {
        let wire = Wire::new();
        let soul = wire.soul_side();
        let mut ui = wire.ui_side(false);

        // Drop the soul side (which holds the senders)
        drop(soul);
        drop(wire);

        // Should receive None when channel is closed
        let msg = ui.recv().await;
        assert!(msg.is_none());
    }
}
