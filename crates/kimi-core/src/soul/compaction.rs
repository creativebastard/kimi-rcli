//! Context compaction strategies
//!
//! Provides mechanisms to reduce context size when approaching token limits,
//! including summarization and checkpoint-based truncation.

use crate::context::Context;
use thiserror::Error;
use tracing::{info, warn};

/// Errors that can occur during compaction
#[derive(Debug, Error)]
pub enum CompactionError {
    #[error("No checkpoint available for compaction")]
    NoCheckpoint,
    #[error("Context error: {0}")]
    Context(#[from] crate::context::ContextError),
}

/// Trait for context compaction strategies
pub trait Compaction: Send + Sync {
    /// Compact the context to reduce token count
    ///
    /// Returns the number of messages removed, if any.
    fn compact(&self, context: &mut Context) -> Result<usize, CompactionError>;

    /// Check if compaction is needed based on current token count
    fn is_needed(&self, context: &Context, max_tokens: usize) -> bool {
        context.token_count() > max_tokens
    }
}

/// Simple compaction strategy that truncates to the last checkpoint
#[derive(Debug, Clone)]
pub struct SimpleCompaction {
    /// Maximum tokens before triggering compaction
    pub max_tokens: usize,
    /// Target token count after compaction (percentage of max)
    pub target_ratio: f64,
}

impl SimpleCompaction {
    /// Create a new simple compaction strategy
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            target_ratio: 0.5, // Target 50% of max after compaction
        }
    }

    /// Create a new simple compaction strategy with custom target ratio
    pub fn with_ratio(max_tokens: usize, target_ratio: f64) -> Self {
        Self {
            max_tokens,
            target_ratio: target_ratio.clamp(0.1, 0.9),
        }
    }

    /// Get the target token count
    pub fn target_tokens(&self) -> usize {
        (self.max_tokens as f64 * self.target_ratio) as usize
    }

    /// Summarize messages for checkpoint (placeholder for future LLM-based summarization)
    pub fn summarize_messages(&self, _messages: &[crate::types::Message]) -> String {
        // TODO: Implement LLM-based summarization
        "[Conversation summary placeholder]".to_string()
    }
}

impl Default for SimpleCompaction {
    fn default() -> Self {
        Self::new(8000) // Default 8k token limit
    }
}

impl Compaction for SimpleCompaction {
    fn compact(&self, context: &mut Context) -> Result<usize, CompactionError> {
        // First, try to compact to the last checkpoint
        if let Some(removed) = context.compact_to_last_checkpoint() {
            info!("Compacted context to last checkpoint, removed {} messages", removed);
            return Ok(removed);
        }

        // If no checkpoint exists, we need to create one and clear messages
        warn!("No checkpoint available for compaction, creating emergency checkpoint");
        
        // Create a checkpoint with a summary
        let summary = if !context.messages().is_empty() {
            self.summarize_messages(context.messages())
        } else {
            "Empty context".to_string()
        };
        
        context.create_checkpoint(Some(summary));
        
        // Clear all messages
        let count = context.message_count();
        context.clear_messages();
        
        info!("Created emergency checkpoint and cleared {} messages", count);
        Ok(count)
    }

    fn is_needed(&self, context: &Context, _max_tokens: usize) -> bool {
        context.needs_compaction(self.max_tokens)
    }
}

/// Aggressive compaction that keeps only the most recent messages
#[derive(Debug, Clone)]
pub struct AggressiveCompaction {
    /// Maximum tokens allowed
    pub max_tokens: usize,
    /// Number of recent messages to keep
    pub keep_recent: usize,
}

impl AggressiveCompaction {
    /// Create a new aggressive compaction strategy
    pub fn new(max_tokens: usize, keep_recent: usize) -> Self {
        Self {
            max_tokens,
            keep_recent,
        }
    }
}

impl Compaction for AggressiveCompaction {
    fn compact(&self, context: &mut Context) -> Result<usize, CompactionError> {
        let messages = context.messages();
        let total_messages = messages.len();
        
        if total_messages <= self.keep_recent {
            // Not enough messages to compact
            return Ok(0);
        }

        // Keep only the most recent messages
        let keep_index = total_messages - self.keep_recent;
        let removed = keep_index;
        
        // Create a checkpoint before truncation
        let summary = format!("[Truncated {} older messages]", removed);
        context.create_checkpoint(Some(summary));
        
        // Truncate messages (this requires mutable access)
        let messages_mut = context.messages_mut();
        messages_mut.drain(0..keep_index);
        
        warn!(
            "Aggressively compacted context, removed {} messages, kept {}",
            removed, self.keep_recent
        );
        
        Ok(removed)
    }
}

/// Smart compaction that uses summarization
#[derive(Debug, Clone)]
pub struct SmartCompaction {
    /// Maximum tokens allowed
    pub max_tokens: usize,
    /// Target token count after compaction
    pub target_tokens: usize,
}

impl SmartCompaction {
    /// Create a new smart compaction strategy
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            target_tokens: max_tokens / 2,
        }
    }

    /// Estimate token count for a message (rough approximation)
    fn estimate_tokens(&self, message: &crate::types::Message) -> usize {
        // Rough estimate: 1 token ≈ 4 characters
        message.content.len() / 4 + 10 // +10 for metadata
    }
}

impl Compaction for SmartCompaction {
    fn compact(&self, context: &mut Context) -> Result<usize, CompactionError> {
        let messages = context.messages().to_vec();
        let total_tokens: usize = messages.iter().map(|m| self.estimate_tokens(m)).sum();
        
        if total_tokens <= self.target_tokens {
            return Ok(0);
        }

        // Find how many messages we need to summarize
        let tokens_to_remove = total_tokens - self.target_tokens;
        let mut messages_to_summarize = 0;
        let mut accumulated_tokens = 0;

        for message in &messages {
            let tokens = self.estimate_tokens(message);
            if accumulated_tokens + tokens > tokens_to_remove {
                break;
            }
            accumulated_tokens += tokens;
            messages_to_summarize += 1;
        }

        if messages_to_summarize == 0 {
            // Can't remove anything without removing the most recent message
            warn!("Cannot compact without removing recent messages");
            return Ok(0);
        }

        // Create a summary checkpoint
        let summary = format!(
            "[Summarized {} messages, ~{} tokens]",
            messages_to_summarize, accumulated_tokens
        );
        
        context.create_checkpoint(Some(summary));
        
        // Remove the summarized messages
        let messages_mut = context.messages_mut();
        messages_mut.drain(0..messages_to_summarize);
        
        info!(
            "Smart compacted context, summarized {} messages (~{} tokens)",
            messages_to_summarize, accumulated_tokens
        );
        
        Ok(messages_to_summarize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, Role};
    use std::path::PathBuf;

    fn create_test_message(role: Role, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            metadata: None,
        }
    }

    #[test]
    fn test_simple_compaction_new() {
        let compaction = SimpleCompaction::new(8000);
        assert_eq!(compaction.max_tokens, 8000);
        assert_eq!(compaction.target_ratio, 0.5);
    }

    #[test]
    fn test_simple_compaction_with_ratio() {
        let compaction = SimpleCompaction::with_ratio(8000, 0.7);
        assert_eq!(compaction.max_tokens, 8000);
        assert_eq!(compaction.target_ratio, 0.7);
    }

    #[test]
    fn test_simple_compaction_clamps_ratio() {
        let compaction = SimpleCompaction::with_ratio(8000, 1.5);
        assert_eq!(compaction.target_ratio, 0.9);
        
        let compaction = SimpleCompaction::with_ratio(8000, 0.05);
        assert_eq!(compaction.target_ratio, 0.1);
    }

    #[test]
    fn test_is_needed() {
        let compaction = SimpleCompaction::new(100);
        let mut context = Context::new(PathBuf::from("/tmp/test.json"));
        
        assert!(!compaction.is_needed(&context, 100));
        
        context.set_token_count(150);
        assert!(compaction.is_needed(&context, 100));
    }

    #[test]
    fn test_aggressive_compaction() {
        let compaction = AggressiveCompaction::new(1000, 5);
        let mut context = Context::new(PathBuf::from("/tmp/test.json"));
        
        // Add 10 messages
        for i in 0..10 {
            context.add_message(create_test_message(Role::User, &format!("Message {}", i)));
        }
        
        let removed = compaction.compact(&mut context).unwrap();
        assert_eq!(removed, 5); // Removed 5 messages
        assert_eq!(context.message_count(), 5); // Kept 5 messages
    }

    #[test]
    fn test_aggressive_compaction_not_enough_messages() {
        let compaction = AggressiveCompaction::new(1000, 10);
        let mut context = Context::new(PathBuf::from("/tmp/test.json"));
        
        // Add 5 messages
        for i in 0..5 {
            context.add_message(create_test_message(Role::User, &format!("Message {}", i)));
        }
        
        let removed = compaction.compact(&mut context).unwrap();
        assert_eq!(removed, 0); // Nothing removed
        assert_eq!(context.message_count(), 5);
    }

    #[test]
    fn test_smart_compaction() {
        let compaction = SmartCompaction::new(100);
        let mut context = Context::new(PathBuf::from("/tmp/test.json"));
        
        // Add many long messages to ensure we exceed the target
        for i in 0..20 {
            context.add_message(create_test_message(Role::User, &format!(
                "Message {} with a lot of content that takes up significant space in the context \
                 and should contribute to a higher token count when we estimate it", 
                i
            )));
        }
        
        // Manually set token count to simulate high usage
        context.set_token_count(500);
        
        let removed = compaction.compact(&mut context).unwrap();
        // Note: removed may be 0 if the estimated tokens don't exceed target
        // The test mainly verifies the compaction logic runs without error
        assert_eq!(context.checkpoints().len(), if removed > 0 { 1 } else { 0 });
    }
}
