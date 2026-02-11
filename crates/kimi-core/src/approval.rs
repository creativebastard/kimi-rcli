//! Approval system for tool execution

use crate::types::{ApprovalKind, Request};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use tracing::{debug, info, warn};

/// Manages approval requests for tool execution
#[derive(Debug, Clone)]
pub struct Approval {
    yolo: bool,
    pending: Arc<Mutex<Option<PendingRequest>>>,
}

/// Internal structure for pending approval requests
#[derive(Debug)]
struct PendingRequest {
    request: Request,
    response_tx: oneshot::Sender<ApprovalKind>,
}

impl Approval {
    /// Create a new approval manager
    pub fn new() -> Self {
        Self {
            yolo: false,
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new approval manager in yolo mode (auto-approve)
    pub fn yolo() -> Self {
        Self {
            yolo: true,
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if in yolo mode
    pub fn is_yolo(&self) -> bool {
        self.yolo
    }

    /// Set yolo mode
    pub fn set_yolo(&mut self, yolo: bool) {
        self.yolo = yolo;
    }

    /// Request approval for a tool execution
    /// Returns ApprovalKind::Approve immediately if in yolo mode,
    /// otherwise waits for a response
    pub async fn request(&self, request: Request) -> ApprovalKind {
        if self.yolo {
            info!("Yolo mode active, auto-approving request {}", request.id);
            return ApprovalKind::Approve;
        }

        let (tx, rx) = oneshot::channel();
        
        {
            let mut pending = self.pending.lock().await;
            if pending.is_some() {
                warn!("Another approval request is already pending");
                // Return reject if there's already a pending request
                return ApprovalKind::Reject;
            }
            *pending = Some(PendingRequest {
                request,
                response_tx: tx,
            });
        }

        debug!("Waiting for approval response");
        match rx.await {
            Ok(response) => {
                let mut pending = self.pending.lock().await;
                *pending = None;
                response
            }
            Err(_) => {
                warn!("Approval response channel closed");
                let mut pending = self.pending.lock().await;
                *pending = None;
                ApprovalKind::Reject
            }
        }
    }

    /// Respond to a pending approval request
    pub async fn respond(&self, response: ApprovalKind) -> Result<(), ApprovalError> {
        let mut pending = self.pending.lock().await;
        
        if let Some(pending_request) = pending.take() {
            pending_request
                .response_tx
                .send(response)
                .map_err(|_| ApprovalError::SendFailed)?;
            Ok(())
        } else {
            Err(ApprovalError::NoPendingRequest)
        }
    }

    /// Check if there's a pending approval request
    pub async fn has_pending(&self) -> bool {
        let pending = self.pending.lock().await;
        pending.is_some()
    }

    /// Get the pending request if any
    pub async fn get_pending(&self) -> Option<Request> {
        let pending = self.pending.lock().await;
        pending.as_ref().map(|p| p.request.clone())
    }

    /// Cancel any pending request
    pub async fn cancel(&self) -> Result<(), ApprovalError> {
        let mut pending = self.pending.lock().await;
        
        if let Some(pending_request) = pending.take() {
            let _ = pending_request.response_tx.send(ApprovalKind::Reject);
            info!("Cancelled pending approval request");
        }
        
        Ok(())
    }
}

impl Default for Approval {
    fn default() -> Self {
        Self::new()
    }
}

/// Approval-related errors
#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("No pending approval request")]
    NoPendingRequest,
    #[error("Failed to send approval response")]
    SendFailed,
}

/// Approval request event for UI/transport layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestEvent {
    pub id: String,
    pub tool_call_id: String,
    pub sender: String,
    pub action: String,
    pub description: String,
}

impl From<Request> for ApprovalRequestEvent {
    fn from(req: Request) -> Self {
        Self {
            id: req.id,
            tool_call_id: req.tool_call_id,
            sender: req.sender,
            action: req.action,
            description: req.description,
        }
    }
}

/// Approval response event from UI/transport layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponseEvent {
    pub request_id: String,
    pub response: ApprovalKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> Request {
        Request {
            id: "req-123".to_string(),
            tool_call_id: "tool-456".to_string(),
            sender: "test-agent".to_string(),
            action: "write_file".to_string(),
            description: "Write to /tmp/test.txt".to_string(),
        }
    }

    #[tokio::test]
    async fn test_approval_yolo_mode() {
        let approval = Approval::yolo();
        assert!(approval.is_yolo());

        let request = create_test_request();
        let result = approval.request(request).await;
        assert!(matches!(result, ApprovalKind::Approve));
    }

    #[tokio::test]
    async fn test_approval_normal_mode() {
        let approval = Approval::new();
        assert!(!approval.is_yolo());
        assert!(!approval.has_pending().await);

        // Spawn a task to respond to the approval request
        let approval_clone = approval.clone();
        tokio::spawn(async move {
            // Wait a bit to ensure the request is pending
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            approval_clone.respond(ApprovalKind::Approve).await.unwrap();
        });

        let request = create_test_request();
        let result = approval.request(request).await;
        assert!(matches!(result, ApprovalKind::Approve));
        assert!(!approval.has_pending().await);
    }

    #[tokio::test]
    async fn test_approval_reject() {
        let approval = Approval::new();

        let approval_clone = approval.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            approval_clone.respond(ApprovalKind::Reject).await.unwrap();
        });

        let request = create_test_request();
        let result = approval.request(request).await;
        assert!(matches!(result, ApprovalKind::Reject));
    }

    #[tokio::test]
    async fn test_approval_cancel() {
        let approval = Approval::new();

        // Start a request in a separate task
        let approval_clone = approval.clone();
        let handle = tokio::spawn(async move {
            let request = create_test_request();
            approval_clone.request(request).await
        });

        // Wait a bit then cancel
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        approval.cancel().await.unwrap();

        let result = handle.await.unwrap();
        assert!(matches!(result, ApprovalKind::Reject));
    }

    #[tokio::test]
    async fn test_no_pending_request() {
        let approval = Approval::new();
        
        // Trying to respond without a pending request should fail
        let result = approval.respond(ApprovalKind::Approve).await;
        assert!(matches!(result, Err(ApprovalError::NoPendingRequest)));
    }
}
