//! DenwaRenji (D-Mail) - Time-travel debugging system
//!
//! Inspired by Steins;Gate, the D-Mail system allows the agent to "send messages
//! to the past" by checkpointing context and allowing rollback to previous states.

use std::sync::Arc;
use tokio::sync::Mutex;

/// A D-Mail (DeLorean Mail) represents a message sent "to the past"
/// to modify the timeline (context state).
#[derive(Debug, Clone)]
pub struct DMail {
    /// The checkpoint ID to rollback to
    pub checkpoint_id: usize,
    /// The message to append after rollback
    pub message: String,
}

impl DMail {
    /// Create a new D-Mail
    pub fn new(checkpoint_id: usize, message: impl Into<String>) -> Self {
        Self {
            checkpoint_id,
            message: message.into(),
        }
    }
}

/// DenwaRenji (Phone Microwave) - The D-Mail management system
///
/// This struct manages pending D-Mails that allow the agent to
/// rollback to previous checkpoints and continue from there.
#[derive(Debug, Clone)]
pub struct DenwaRenji {
    pending_dmail: Arc<Mutex<Option<DMail>>>,
}

impl DenwaRenji {
    /// Create a new DenwaRenji instance
    pub fn new() -> Self {
        Self {
            pending_dmail: Arc::new(Mutex::new(None)),
        }
    }

    /// Send a D-Mail (queue it for processing)
    pub async fn send_dmail(&self, dmail: DMail) {
        let mut pending = self.pending_dmail.lock().await;
        *pending = Some(dmail);
    }

    /// Check if there's a pending D-Mail
    pub async fn has_pending_dmail(&self) -> bool {
        let pending = self.pending_dmail.lock().await;
        pending.is_some()
    }

    /// Receive (consume) the pending D-Mail if any
    pub async fn receive_dmail(&self) -> Option<DMail> {
        let mut pending = self.pending_dmail.lock().await;
        pending.take()
    }

    /// Cancel any pending D-Mail
    pub async fn cancel_dmail(&self) {
        let mut pending = self.pending_dmail.lock().await;
        *pending = None;
    }
}

impl Default for DenwaRenji {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_denwarenji_new() {
        let dr = DenwaRenji::new();
        assert!(!dr.has_pending_dmail().await);
    }

    #[tokio::test]
    async fn test_send_and_receive_dmail() {
        let dr = DenwaRenji::new();
        
        let dmail = DMail::new(5, "This is a test message");
        dr.send_dmail(dmail).await;
        
        assert!(dr.has_pending_dmail().await);
        
        let received = dr.receive_dmail().await;
        assert!(received.is_some());
        
        let received = received.unwrap();
        assert_eq!(received.checkpoint_id, 5);
        assert_eq!(received.message, "This is a test message");
        
        assert!(!dr.has_pending_dmail().await);
    }

    #[tokio::test]
    async fn test_cancel_dmail() {
        let dr = DenwaRenji::new();
        
        let dmail = DMail::new(3, "Message to cancel");
        dr.send_dmail(dmail).await;
        
        assert!(dr.has_pending_dmail().await);
        
        dr.cancel_dmail().await;
        
        assert!(!dr.has_pending_dmail().await);
        assert!(dr.receive_dmail().await.is_none());
    }

    #[tokio::test]
    async fn test_receive_without_pending() {
        let dr = DenwaRenji::new();
        
        let received = dr.receive_dmail().await;
        assert!(received.is_none());
    }
}
