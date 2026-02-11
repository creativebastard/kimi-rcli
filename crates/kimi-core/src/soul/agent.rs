//! Agent runtime and execution context
//!
//! This module provides the Agent struct which represents a running agent
//! instance, along with Runtime for managing execution and LaborMarket
//! for agent-to-agent task delegation.

use crate::types::{Message, Role};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Represents a running agent instance
#[derive(Debug, Clone)]
pub struct Agent {
    /// Unique agent ID
    pub id: String,
    /// Agent name
    pub name: String,
    /// Agent role/description
    pub role: String,
    /// System prompt
    pub system_prompt: String,
    /// Agent state
    state: Arc<RwLock<AgentState>>,
    /// Agent configuration
    config: AgentConfig,
}

/// Agent state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentState {
    /// Agent is idle, waiting for work
    Idle,
    /// Agent is working on a task
    Working { task_id: String, started_at: String },
    /// Agent has completed a task
    Completed { task_id: String, result: String },
    /// Agent encountered an error
    Error { error: String },
    /// Agent is paused
    Paused,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Maximum iterations per turn
    pub max_iterations: usize,
    /// Timeout in seconds
    pub timeout_seconds: u64,
    /// Whether to use thinking mode
    pub thinking: bool,
    /// Model to use
    pub model: String,
    /// Temperature for generation
    pub temperature: Option<f64>,
    /// Maximum tokens to generate
    pub max_tokens: Option<usize>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            timeout_seconds: 300,
            thinking: false,
            model: "default".to_string(),
            temperature: None,
            max_tokens: None,
        }
    }
}

impl Agent {
    /// Create a new agent
    pub fn new(name: impl Into<String>, role: impl Into<String>) -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            id: id.clone(),
            name: name.into(),
            role: role.into(),
            system_prompt: String::new(),
            state: Arc::new(RwLock::new(AgentState::Idle)),
            config: AgentConfig::default(),
        }
    }

    /// Create a new agent with a specific ID
    pub fn with_id(id: impl Into<String>, name: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role: role.into(),
            system_prompt: String::new(),
            state: Arc::new(RwLock::new(AgentState::Idle)),
            config: AgentConfig::default(),
        }
    }

    /// Set the system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Set the configuration
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the current state
    pub async fn state(&self) -> AgentState {
        let state = self.state.read().await;
        state.clone()
    }

    /// Set the agent state
    pub async fn set_state(&self, state: AgentState) {
        let mut current = self.state.write().await;
        *current = state;
    }

    /// Check if the agent is idle
    pub async fn is_idle(&self) -> bool {
        matches!(self.state().await, AgentState::Idle)
    }

    /// Check if the agent is working
    pub async fn is_working(&self) -> bool {
        matches!(self.state().await, AgentState::Working { .. })
    }

    /// Get the configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration
    pub fn config_mut(&mut self) -> &mut AgentConfig {
        &mut self.config
    }

    /// Build the system message from the system prompt
    pub fn build_system_message(&self) -> Message {
        Message {
            role: Role::System,
            content: self.system_prompt.clone(),
            metadata: None,
        }
    }
}

/// Runtime for managing agent execution
#[derive(Debug, Clone)]
pub struct Runtime {
    /// Running agents
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    /// Task queue
    task_queue: Arc<Mutex<Vec<Task>>>,
    /// Execution statistics
    stats: Arc<RwLock<RuntimeStats>>,
}

/// A task to be executed by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task ID
    pub id: String,
    /// Task description
    pub description: String,
    /// Assigned agent ID (if any)
    pub assigned_agent: Option<String>,
    /// Task priority (higher = more important)
    pub priority: i32,
    /// Task status
    pub status: TaskStatus,
    /// Created at timestamp
    pub created_at: String,
}

/// Task status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed(String),
    Failed(String),
    Cancelled,
}

/// Runtime statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeStats {
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub active_agents: usize,
}

impl Runtime {
    /// Create a new runtime
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            task_queue: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(RwLock::new(RuntimeStats::default())),
        }
    }

    /// Register an agent with the runtime
    pub async fn register_agent(&self, agent: Agent) {
        let agent_id = agent.id.clone();
        let mut agents = self.agents.write().await;
        agents.insert(agent_id.clone(), agent);
        debug!("Registered agent: {}", agent_id);
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &str) -> Option<Agent> {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id)
    }

    /// Get an agent by ID
    pub async fn get_agent(&self, agent_id: &str) -> Option<Agent> {
        let agents = self.agents.read().await;
        agents.get(agent_id).cloned()
    }

    /// Get all agents
    pub async fn list_agents(&self) -> Vec<Agent> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Get idle agents
    pub async fn get_idle_agents(&self) -> Vec<Agent> {
        let agents = self.agents.read().await;
        let mut idle = Vec::new();
        for agent in agents.values() {
            if agent.is_idle().await {
                idle.push(agent.clone());
            }
        }
        idle
    }

    /// Submit a task to the queue
    pub async fn submit_task(&self, description: impl Into<String>) -> String {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            description: description.into(),
            assigned_agent: None,
            priority: 0,
            status: TaskStatus::Pending,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        
        let task_id = task.id.clone();
        let mut queue = self.task_queue.lock().await;
        queue.push(task);
        
        // Sort by priority (descending)
        queue.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        info!("Submitted task: {}", task_id);
        task_id
    }

    /// Submit a task with priority
    pub async fn submit_task_with_priority(
        &self,
        description: impl Into<String>,
        priority: i32,
    ) -> String {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            description: description.into(),
            assigned_agent: None,
            priority,
            status: TaskStatus::Pending,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        
        let task_id = task.id.clone();
        let mut queue = self.task_queue.lock().await;
        queue.push(task);
        
        // Sort by priority (descending)
        queue.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        info!("Submitted task with priority {}: {}", priority, task_id);
        task_id
    }

    /// Get the next pending task
    pub async fn next_task(&self) -> Option<Task> {
        let mut queue = self.task_queue.lock().await;
        // Find first pending task
        queue
            .iter()
            .position(|t| matches!(t.status, TaskStatus::Pending))
            .map(|index| queue.remove(index))
    }

    /// Get task queue length
    pub async fn queue_length(&self) -> usize {
        let queue = self.task_queue.lock().await;
        queue.len()
    }

    /// Get runtime statistics
    pub async fn stats(&self) -> RuntimeStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Update statistics
    pub async fn update_stats<F>(&self, f: F)
    where
        F: FnOnce(&mut RuntimeStats),
    {
        let mut stats = self.stats.write().await;
        f(&mut stats);
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Labor market for agent-to-agent task delegation
///
/// This implements a simple marketplace where agents can offer and accept tasks
#[derive(Debug, Clone)]
pub struct LaborMarket {
    /// Available tasks (offered by agents)
    available_tasks: Arc<RwLock<HashMap<String, MarketTask>>>,
    /// Task assignments
    assignments: Arc<RwLock<HashMap<String, String>>>, // task_id -> agent_id
}

/// A task available on the labor market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTask {
    /// Task ID
    pub id: String,
    /// Task description
    pub description: String,
    /// Offering agent ID
    pub offered_by: String,
    /// Required skills (tags)
    pub required_skills: Vec<String>,
    /// Reward/priority
    pub priority: i32,
    /// Offered at timestamp
    pub offered_at: String,
}

impl LaborMarket {
    /// Create a new labor market
    pub fn new() -> Self {
        Self {
            available_tasks: Arc::new(RwLock::new(HashMap::new())),
            assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Offer a task on the market
    pub async fn offer_task(
        &self,
        description: impl Into<String>,
        offered_by: impl Into<String>,
        required_skills: Vec<String>,
        priority: i32,
    ) -> String {
        let task = MarketTask {
            id: Uuid::new_v4().to_string(),
            description: description.into(),
            offered_by: offered_by.into(),
            required_skills,
            priority,
            offered_at: chrono::Utc::now().to_rfc3339(),
        };
        
        let task_id = task.id.clone();
        let mut tasks = self.available_tasks.write().await;
        tasks.insert(task_id.clone(), task);
        
        info!("Task offered on labor market: {}", task_id);
        task_id
    }

    /// Accept a task from the market
    pub async fn accept_task(&self, task_id: &str, agent_id: &str) -> bool {
        let mut tasks = self.available_tasks.write().await;
        
        if tasks.contains_key(task_id) {
            tasks.remove(task_id);
            
            let mut assignments = self.assignments.write().await;
            assignments.insert(task_id.to_string(), agent_id.to_string());
            
            info!("Task {} accepted by agent {}", task_id, agent_id);
            true
        } else {
            warn!("Task {} not found on labor market", task_id);
            false
        }
    }

    /// Get available tasks
    pub async fn get_available_tasks(&self) -> Vec<MarketTask> {
        let tasks = self.available_tasks.read().await;
        tasks.values().cloned().collect()
    }

    /// Get tasks matching certain skills
    pub async fn get_tasks_matching_skills(&self, skills: &[String]) -> Vec<MarketTask> {
        let tasks = self.available_tasks.read().await;
        tasks
            .values()
            .filter(|t| skills.iter().any(|s| t.required_skills.contains(s)))
            .cloned()
            .collect()
    }

    /// Complete a task
    pub async fn complete_task(&self, task_id: &str) -> bool {
        let mut assignments = self.assignments.write().await;
        assignments.remove(task_id).is_some()
    }

    /// Get task assignment
    pub async fn get_assignment(&self, task_id: &str) -> Option<String> {
        let assignments = self.assignments.read().await;
        assignments.get(task_id).cloned()
    }
}

impl Default for LaborMarket {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_new() {
        let agent = Agent::new("TestAgent", "A test agent");
        assert_eq!(agent.name, "TestAgent");
        assert_eq!(agent.role, "A test agent");
        assert!(agent.is_idle().await);
    }

    #[tokio::test]
    async fn test_agent_state() {
        let agent = Agent::new("TestAgent", "A test agent");
        
        agent.set_state(AgentState::Working {
            task_id: "task-1".to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
        }).await;
        
        assert!(agent.is_working().await);
        assert!(!agent.is_idle().await);
    }

    #[tokio::test]
    async fn test_agent_with_system_prompt() {
        let agent = Agent::new("TestAgent", "A test agent")
            .with_system_prompt("You are a helpful assistant.");
        
        let msg = agent.build_system_message();
        assert_eq!(msg.content, "You are a helpful assistant.");
        assert!(matches!(msg.role, Role::System));
    }

    #[tokio::test]
    async fn test_runtime_register_agent() {
        let runtime = Runtime::new();
        let agent = Agent::new("TestAgent", "A test agent");
        let agent_id = agent.id.clone();
        
        runtime.register_agent(agent).await;
        
        let retrieved = runtime.get_agent(&agent_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "TestAgent");
    }

    #[tokio::test]
    async fn test_runtime_submit_task() {
        let runtime = Runtime::new();
        
        let task_id = runtime.submit_task("Test task").await;
        assert!(!task_id.is_empty());
        
        let queue_len = runtime.queue_length().await;
        assert_eq!(queue_len, 1);
    }

    #[tokio::test]
    async fn test_runtime_next_task() {
        let runtime = Runtime::new();
        
        runtime.submit_task("Task 1").await;
        runtime.submit_task("Task 2").await;
        
        let task = runtime.next_task().await;
        assert!(task.is_some());
        assert_eq!(task.unwrap().description, "Task 1");
        
        let queue_len = runtime.queue_length().await;
        assert_eq!(queue_len, 1);
    }

    #[tokio::test]
    async fn test_runtime_priority_tasks() {
        let runtime = Runtime::new();
        
        runtime.submit_task_with_priority("Low priority", 1).await;
        runtime.submit_task_with_priority("High priority", 10).await;
        runtime.submit_task_with_priority("Medium priority", 5).await;
        
        // High priority task should come first
        let task = runtime.next_task().await;
        assert_eq!(task.unwrap().description, "High priority");
    }

    #[tokio::test]
    async fn test_labor_market_offer_and_accept() {
        let market = LaborMarket::new();
        
        let task_id = market.offer_task(
            "Complex calculation",
            "agent-1",
            vec!["math".to_string(), "calculation".to_string()],
            5,
        ).await;
        
        let available = market.get_available_tasks().await;
        assert_eq!(available.len(), 1);
        
        let accepted = market.accept_task(&task_id, "agent-2").await;
        assert!(accepted);
        
        let available = market.get_available_tasks().await;
        assert_eq!(available.len(), 0);
        
        let assignment = market.get_assignment(&task_id).await;
        assert_eq!(assignment, Some("agent-2".to_string()));
    }

    #[tokio::test]
    async fn test_labor_market_skill_matching() {
        let market = LaborMarket::new();
        
        market.offer_task(
            "Math task",
            "agent-1",
            vec!["math".to_string()],
            5,
        ).await;
        
        market.offer_task(
            "Code task",
            "agent-1",
            vec!["coding".to_string()],
            5,
        ).await;
        
        let math_tasks = market.get_tasks_matching_skills(&["math".to_string()]).await;
        assert_eq!(math_tasks.len(), 1);
        assert_eq!(math_tasks[0].description, "Math task");
    }
}
