//! KimiSoul - The heart of the agent system
//!
//! This module implements the main agent loop that orchestrates:
//! - User input processing
//! - Slash command handling
//! - LLM interaction via kosong-rs
//! - Tool execution and approval
//! - Context management and compaction
//! - D-Mail (time-travel debugging) support

use crate::approval::Approval;
use crate::context::Context;
use crate::types::{ApprovalKind, LoopControl, Message, Request, UserInput};
use crate::wire::WireMessage;

// Import from sibling modules directly to avoid circular dependencies
use super::agent::Agent;
use super::chat;
use super::compaction::{Compaction, SimpleCompaction};
use super::denwarenji::DenwaRenji;
use super::slash::{parse_slash_command, SlashCommandRegistry};
use super::toolset::{KimiToolset, ToolCall, ToolCallResult};
use super::{user_message, WireSoulSide};
use kosong_rs::ChatProvider;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

use tracing::{debug, error, info, warn};

/// Errors that can occur in the soul
#[derive(Debug, Error)]
pub enum SoulError {
    #[error("Context error: {0}")]
    Context(#[from] crate::context::ContextError),
    #[error("Wire error: {0}")]
    Wire(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Approval error: {0}")]
    Approval(String),
    #[error("Compaction error: {0}")]
    Compaction(String),
    #[error("Timeout")]
    Timeout,
    #[error("Cancelled")]
    Cancelled,
    #[error("Max iterations reached")]
    MaxIterations,
    #[error("Slash command error: {0}")]
    SlashCommand(String),
    #[error("D-Mail error: {0}")]
    DMail(String),
}

/// Outcome of a turn
#[derive(Debug, Clone)]
pub enum TurnOutcome {
    /// Turn completed successfully with response
    Completed(String),
    /// Turn was interrupted
    Interrupted,
    /// Turn encountered an error
    Error(String),
    /// Turn was handled by a slash command
    SlashCommandHandled,
    /// Turn triggered a D-Mail rollback
    DMailRollback,
}

/// Outcome of a single step
#[derive(Debug, Clone)]
pub enum StepOutcome {
    /// Step completed, continue to next
    Continue,
    /// Step produced a final response, stop
    Complete(String),
    /// Step requires tool execution
    ToolCalls(Vec<ToolCall>),
    /// Step was interrupted
    Interrupted,
    /// Step timed out
    Timeout,
    /// Step reached iteration limit
    MaxIterations,
}

/// The main KimiSoul struct - the heart of the agent system
pub struct KimiSoul {
    /// The agent runtime
    pub agent: Agent,
    /// Conversation context
    pub context: Context,
    /// Approval system for tool execution
    pub approval: Arc<Approval>,
    /// D-Mail system for time-travel debugging
    pub denwa_renji: Arc<DenwaRenji>,
    /// Loop control configuration
    pub loop_control: LoopControl,
    /// Context compaction strategy
    pub compaction: SimpleCompaction,
    /// Slash command registry
    pub slash_commands: SlashCommandRegistry,
    /// Toolset for tool execution
    pub toolset: KimiToolset,
    /// Current iteration count
    iteration: usize,
    /// Turn start time
    turn_start: Option<Instant>,
    /// Pending tool calls
    pending_tool_calls: Vec<ToolCall>,
    /// Whether the loop should stop
    should_stop: bool,
}

impl KimiSoul {
    /// Create a new KimiSoul instance
    pub fn new(
        agent: Agent,
        context: Context,
        approval: Arc<Approval>,
        denwa_renji: Arc<DenwaRenji>,
        loop_control: LoopControl,
        compaction: SimpleCompaction,
    ) -> Self {
        Self {
            agent,
            context,
            approval,
            denwa_renji,
            loop_control,
            compaction,
            slash_commands: SlashCommandRegistry::with_defaults(),
            toolset: KimiToolset::new(),
            iteration: 0,
            turn_start: None,
            pending_tool_calls: Vec::new(),
            should_stop: false,
        }
    }

    /// Run a complete turn with user input
    pub async fn run(
        &mut self,
        user_input: UserInput,
        wire: &WireSoulSide,
    ) -> Result<TurnOutcome, SoulError> {
        info!("Starting new turn with user input");
        
        // Send TurnBegin
        self.send_wire(wire, WireMessage::TurnBegin { user_input: user_input.clone() }).await?;
        
        // Check for slash commands
        if let Some((command, args)) = parse_slash_command(&user_input.text) {
            info!("Detected slash command: /{}", command);
            
            if let Some(cmd) = self.slash_commands.get(command).cloned() {
                match cmd.execute(self, args) {
                    Ok(()) => {
                        self.send_wire(wire, WireMessage::TurnEnd).await?;
                        return Ok(TurnOutcome::SlashCommandHandled);
                    }
                    Err(e) => {
                        error!("Slash command error: {}", e);
                        self.send_wire(wire, WireMessage::TurnEnd).await?;
                        return Ok(TurnOutcome::Error(format!("Command error: {}", e)));
                    }
                }
            } else {
                let error_msg = format!("Unknown command: /{}", command);
                warn!("{}", error_msg);
                self.send_wire(wire, WireMessage::TurnEnd).await?;
                return Ok(TurnOutcome::Error(error_msg));
            }
        }
        
        // Normal flow: create checkpoint and append user message
        self.context.create_checkpoint(Some("User input".to_string()));
        
        let message = user_message(user_input.text);
        self.context.add_message(message);
        
        // Run the agent loop
        let outcome = self.agent_loop(wire).await;
        
        // Send TurnEnd
        self.send_wire(wire, WireMessage::TurnEnd).await?;
        
        // Reset turn state
        self.iteration = 0;
        self.turn_start = None;
        self.pending_tool_calls.clear();
        self.should_stop = false;
        
        outcome
    }

    /// Process a single turn (internal)
    pub async fn turn(
        &mut self,
        message: Message,
        wire: &WireSoulSide,
    ) -> Result<TurnOutcome, SoulError> {
        self.context.add_message(message);
        self.agent_loop(wire).await
    }

    /// Execute a single step in the agent loop
    pub async fn step(&mut self, wire: &WireSoulSide) -> Result<StepOutcome, SoulError> {
        // Check for D-Mail
        if self.denwa_renji.has_pending_dmail().await {
            info!("D-Mail detected, handling rollback");
            if let Some(dmail) = self.denwa_renji.receive_dmail().await {
                // Rollback to checkpoint
                if self.context.compact_to_checkpoint(&dmail.checkpoint_id.to_string()).is_none() {
                    // If checkpoint not found, try to compact to last checkpoint
                    self.context.compact_to_last_checkpoint();
                }
                // Append the D-Mail message
                self.context.add_message(user_message(dmail.message));
                return Ok(StepOutcome::Continue);
            }
        }
        
        // Check context size and compact if needed
        if self.compaction.is_needed(&self.context, self.compaction.max_tokens) {
            info!("Context needs compaction, compacting...");
            self.send_wire(wire, WireMessage::CompactionBegin).await?;
            
            match self.compaction.compact(&mut self.context) {
                Ok(removed) => {
                    info!("Compacted {} messages", removed);
                }
                Err(e) => {
                    warn!("Compaction failed: {}", e);
                }
            }
            
            self.send_wire(wire, WireMessage::CompactionEnd).await?;
        }
        
        // Check iteration limit
        if self.iteration >= self.loop_control.max_iterations {
            warn!("Max iterations reached: {}", self.iteration);
            return Ok(StepOutcome::MaxIterations);
        }
        
        // Check timeout
        if let Some(start) = self.turn_start {
            let elapsed = start.elapsed();
            if elapsed.as_secs() > self.loop_control.timeout_seconds {
                warn!("Turn timeout after {:?}", elapsed);
                return Ok(StepOutcome::Timeout);
            }
        }
        
        // Send StepBegin
        self.send_wire(wire, WireMessage::StepBegin { n: self.iteration }).await?;
        
        // TODO: Call LLM via kosong-rs
        // For now, this is a placeholder that would be replaced with actual LLM integration
        let llm_response = self.call_llm(wire).await?;
        
        self.iteration += 1;
        
        // Process LLM response
        match llm_response {
            LlmResponse::Text(text) => {
                // Add assistant message to context
                self.context.add_message(super::assistant_message(text.clone()));
                Ok(StepOutcome::Complete(text))
            }
            LlmResponse::ToolCalls(calls) => {
                Ok(StepOutcome::ToolCalls(calls))
            }
            LlmResponse::Interrupted => {
                Ok(StepOutcome::Interrupted)
            }
        }
    }

    /// The main agent loop
    async fn agent_loop(&mut self, wire: &WireSoulSide) -> Result<TurnOutcome, SoulError> {
        info!("Starting agent loop");
        
        self.turn_start = Some(Instant::now());
        
        loop {
            // Check if we should stop
            if self.should_stop {
                debug!("Agent loop stopping (should_stop=true)");
                break;
            }
            
            // Execute a step
            let outcome = self.step(wire).await?;
            
            match outcome {
                StepOutcome::Continue => {
                    // Continue to next iteration
                    continue;
                }
                StepOutcome::Complete(response) => {
                    // Turn completed successfully
                    return Ok(TurnOutcome::Completed(response));
                }
                StepOutcome::ToolCalls(calls) => {
                    // Execute tool calls
                    let results = self.execute_tool_calls(calls, wire).await?;
                    
                    // Add tool results to context
                    for result in results {
                        let content = if result.is_error {
                            format!("Error: {}", result.output)
                        } else {
                            result.output
                        };
                        self.context.add_message(super::tool_message(content));
                    }
                    
                    // Continue to next iteration
                    continue;
                }
                StepOutcome::Interrupted => {
                    self.send_wire(wire, WireMessage::StepInterrupted).await?;
                    return Ok(TurnOutcome::Interrupted);
                }
                StepOutcome::Timeout => {
                    return Ok(TurnOutcome::Error("Turn timeout".to_string()));
                }
                StepOutcome::MaxIterations => {
                    return Ok(TurnOutcome::Error("Max iterations reached".to_string()));
                }
            }
        }
        
        Ok(TurnOutcome::Interrupted)
    }

    /// Execute tool calls with approval
    async fn execute_tool_calls(
        &mut self,
        calls: Vec<ToolCall>,
        wire: &WireSoulSide,
    ) -> Result<Vec<ToolCallResult>, SoulError> {
        let mut results = Vec::new();
        
        for call in calls {
            // Check if tool exists
            if !self.toolset.contains(&call.name) {
                results.push(ToolCallResult::error(
                    &call.id,
                    format!("Tool '{}' not found", call.name),
                ));
                continue;
            }
            
            // Request approval if not in yolo mode
            if !self.approval.is_yolo() {
                let request = Request {
                    id: uuid::Uuid::new_v4().to_string(),
                    tool_call_id: call.id.clone(),
                    sender: self.agent.name.clone(),
                    action: call.name.clone(),
                    description: format!("Execute {} with args: {}", call.name, call.arguments),
                };
                
                // Send approval request
                self.send_wire(
                    wire,
                    WireMessage::ApprovalRequest {
                        id: request.id.clone(),
                        tool_call_id: call.id.clone(),
                        sender: request.sender.clone(),
                        action: request.action.clone(),
                        description: request.description.clone(),
                    },
                ).await?;
                
                // Wait for approval
                let approval = self.approval.request(request).await;
                
                match approval {
                    ApprovalKind::Approve => {
                        // Execute the tool
                        let result = self.execute_tool(&call).await?;
                        results.push(result);
                    }
                    ApprovalKind::ApproveOnce => {
                        // Execute once
                        let result = self.execute_tool(&call).await?;
                        results.push(result);
                    }
                    ApprovalKind::Reject => {
                        results.push(ToolCallResult::error(&call.id, "Tool execution rejected by user"));
                    }
                }
            } else {
                // Yolo mode - execute without approval
                let result = self.execute_tool(&call).await?;
                results.push(result);
            }
        }
        
        Ok(results)
    }

    /// Execute a single tool
    async fn execute_tool(&self, call: &ToolCall) -> Result<ToolCallResult, SoulError> {
        let params = call.parse_arguments()
            .map_err(|e| SoulError::Tool(format!("Invalid arguments: {}", e)))?;
        
        match self.toolset.execute(&call.name, params).await {
            Ok(output) => {
                let output_str = serde_json::to_string(&output)
                    .unwrap_or_else(|_| output.to_string());
                Ok(ToolCallResult::success(&call.id, output_str))
            }
            Err(e) => {
                Ok(ToolCallResult::error(&call.id, e.to_string()))
            }
        }
    }

    /// Call the LLM (placeholder for kosong-rs integration)
    async fn call_llm(&self, _wire: &WireSoulSide) -> Result<LlmResponse, SoulError> {
        // TODO: Integrate with kosong-rs
        // This is a placeholder implementation
        
        // For now, return a simple text response
        // In the real implementation, this would:
        // 1. Build the request with context messages
        // 2. Call the LLM via kosong-rs
        // 3. Stream the response and send chunks via wire
        // 4. Parse tool calls if present
        
        debug!("Calling LLM (placeholder)");
        
        // Placeholder: return a simple response
        Ok(LlmResponse::Text(
            "This is a placeholder response. LLM integration via kosong-rs is pending.".to_string()
        ))
    }

    /// Send a wire message
    async fn send_wire(&self, wire: &WireSoulSide, message: WireMessage) -> Result<(), SoulError> {
        wire.send(message).await
            .map_err(|e| SoulError::Wire(e.to_string()))
    }

    /// Stop the agent loop
    pub fn stop(&mut self) {
        self.should_stop = true;
    }

    /// Get current iteration count
    pub fn iteration(&self) -> usize {
        self.iteration
    }

    /// Check if the loop should stop
    pub fn should_stop(&self) -> bool {
        self.should_stop
    }

    /// Process a message with the LLM provider
    pub async fn process_with_llm(
        &self,
        provider: &dyn ChatProvider,
        user_input: UserInput,
        wire: &WireSoulSide,
    ) -> Result<String, SoulError> {
        chat::process_message(self, provider, user_input, wire).await
    }
}

/// LLM response types
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum LlmResponse {
    /// Plain text response
    Text(String),
    /// Tool calls to execute
    ToolCalls(Vec<ToolCall>),
    /// Response was interrupted
    Interrupted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::agent::Agent;
    use std::path::PathBuf;

    fn create_test_soul() -> KimiSoul {
        let agent = Agent::new("TestAgent", "A test agent");
        let context = Context::new(PathBuf::from("/tmp/test_context.json"));
        let approval = Arc::new(Approval::yolo());
        let denwa_renji = Arc::new(DenwaRenji::new());
        let loop_control = LoopControl {
            max_iterations: 10,
            timeout_seconds: 60,
        };
        let compaction = SimpleCompaction::new(8000);

        KimiSoul::new(agent, context, approval, denwa_renji, loop_control, compaction)
    }

    #[tokio::test]
    async fn test_kimisoul_new() {
        let soul = create_test_soul();
        
        assert_eq!(soul.agent.name, "TestAgent");
        assert_eq!(soul.iteration(), 0);
        assert!(!soul.should_stop());
    }

    #[tokio::test]
    async fn test_kimisoul_stop() {
        let mut soul = create_test_soul();
        
        assert!(!soul.should_stop());
        soul.stop();
        assert!(soul.should_stop());
    }

    #[tokio::test]
    async fn test_kimisoul_iteration() {
        let mut soul = create_test_soul();
        
        assert_eq!(soul.iteration(), 0);
        soul.iteration = 5;
        assert_eq!(soul.iteration(), 5);
    }

    #[test]
    fn test_turn_outcome_debug() {
        let outcome = TurnOutcome::Completed("Hello".to_string());
        assert!(format!("{:?}", outcome).contains("Completed"));
        
        let outcome = TurnOutcome::Interrupted;
        assert!(format!("{:?}", outcome).contains("Interrupted"));
    }

    #[test]
    fn test_step_outcome_debug() {
        let outcome = StepOutcome::Complete("Done".to_string());
        assert!(format!("{:?}", outcome).contains("Complete"));
        
        let outcome = StepOutcome::Continue;
        assert!(format!("{:?}", outcome).contains("Continue"));
    }

    #[test]
    fn test_llm_response() {
        let resp = LlmResponse::Text("Hello".to_string());
        assert!(matches!(resp, LlmResponse::Text(_)));
        
        let resp = LlmResponse::ToolCalls(vec![]);
        assert!(matches!(resp, LlmResponse::ToolCalls(_)));
    }

    #[test]
    fn test_soul_error_display() {
        let err = SoulError::Timeout;
        assert_eq!(err.to_string(), "Timeout");
        
        let err = SoulError::MaxIterations;
        assert_eq!(err.to_string(), "Max iterations reached");
        
        let err = SoulError::Wire("connection failed".to_string());
        assert!(err.to_string().contains("connection failed"));
    }
}
