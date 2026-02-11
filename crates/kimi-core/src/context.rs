//! Context management for conversation history

use crate::types::{Checkpoint, Message};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Manages conversation context including messages and checkpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    messages: Vec<Message>,
    checkpoints: Vec<Checkpoint>,
    token_count: usize,
    context_file: PathBuf,
}

impl Context {
    /// Create a new context with the given context file path
    pub fn new(context_file: PathBuf) -> Self {
        Self {
            messages: Vec::new(),
            checkpoints: Vec::new(),
            token_count: 0,
            context_file,
        }
    }

    /// Load context from the context file
    pub fn load(context_file: PathBuf) -> Result<Self, ContextError> {
        if !context_file.exists() {
            info!("Context file does not exist, creating new context");
            return Ok(Self::new(context_file));
        }

        let content = std::fs::read_to_string(&context_file)?;
        let context: Context = serde_json::from_str(&content)?;
        Ok(context)
    }

    /// Save context to the context file
    pub fn save(&self) -> Result<(), ContextError> {
        if let Some(parent) = self.context_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.context_file, content)?;
        debug!("Context saved to {:?}", self.context_file);
        Ok(())
    }

    /// Add a message to the context
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Get all messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get mutable messages (use with caution)
    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Get the last message
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Clear all messages
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.token_count = 0;
    }

    /// Create a checkpoint at the current message index
    pub fn create_checkpoint(&mut self, summary: Option<String>) -> &Checkpoint {
        let checkpoint = Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            message_index: self.messages.len(),
            token_count: self.token_count,
            summary,
        };
        self.checkpoints.push(checkpoint);
        self.checkpoints.last().unwrap()
    }

    /// Get all checkpoints
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// Get the last checkpoint
    pub fn last_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    /// Compact context to the last checkpoint
    pub fn compact_to_last_checkpoint(&mut self) -> Option<usize> {
        let checkpoint = self.checkpoints.last()?;
        let removed = checkpoint.message_index;
        self.messages.truncate(removed);
        self.token_count = checkpoint.token_count;
        warn!("Context compacted to checkpoint, removed {} messages", removed);
        Some(removed)
    }

    /// Compact context to a specific checkpoint by ID
    pub fn compact_to_checkpoint(&mut self, checkpoint_id: &str) -> Option<usize> {
        let checkpoint = self.checkpoints.iter().find(|c| c.id == checkpoint_id)?;
        let removed = checkpoint.message_index;
        self.messages.truncate(removed);
        self.token_count = checkpoint.token_count;
        warn!(
            "Context compacted to checkpoint {}, removed {} messages",
            checkpoint_id, removed
        );
        Some(removed)
    }

    /// Get current token count
    pub fn token_count(&self) -> usize {
        self.token_count
    }

    /// Update token count
    pub fn set_token_count(&mut self, count: usize) {
        self.token_count = count;
    }

    /// Get context file path
    pub fn context_file(&self) -> &PathBuf {
        &self.context_file
    }

    /// Check if context needs compaction based on token limit
    pub fn needs_compaction(&self, max_tokens: usize) -> bool {
        self.token_count > max_tokens
    }

    /// Get messages since the last checkpoint
    pub fn messages_since_last_checkpoint(&self) -> &[Message] {
        let start_index = self
            .checkpoints
            .last()
            .map(|c| c.message_index)
            .unwrap_or(0);
        &self.messages[start_index..]
    }
}

/// Context-related errors
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Role;

    fn create_test_message(role: Role, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            metadata: None,
        }
    }

    #[test]
    fn test_context_new() {
        let context = Context::new(PathBuf::from("/tmp/test.json"));
        assert_eq!(context.message_count(), 0);
        assert_eq!(context.token_count(), 0);
    }

    #[test]
    fn test_add_message() {
        let mut context = Context::new(PathBuf::from("/tmp/test.json"));
        context.add_message(create_test_message(Role::User, "Hello"));
        assert_eq!(context.message_count(), 1);
    }

    #[test]
    fn test_checkpoint() {
        let mut context = Context::new(PathBuf::from("/tmp/test.json"));
        
        context.add_message(create_test_message(Role::User, "Message 1"));
        context.add_message(create_test_message(Role::Assistant, "Response 1"));
        
        context.create_checkpoint(Some("First exchange".to_string()));
        
        context.add_message(create_test_message(Role::User, "Message 2"));
        context.add_message(create_test_message(Role::Assistant, "Response 2"));
        
        assert_eq!(context.message_count(), 4);
        assert_eq!(context.checkpoints().len(), 1);
        
        let removed = context.compact_to_last_checkpoint();
        assert_eq!(removed, Some(2));
        assert_eq!(context.message_count(), 2);
    }

    #[test]
    fn test_needs_compaction() {
        let mut context = Context::new(PathBuf::from("/tmp/test.json"));
        context.set_token_count(1000);
        assert!(context.needs_compaction(500));
        assert!(!context.needs_compaction(2000));
    }
}
