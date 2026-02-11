//! Prompts for the agent system
//!
//! This module contains system prompts used by the agent for various tasks.

/// The INIT prompt used by the `/init` slash command.
/// This prompt instructs the agent to analyze the project and create an AGENTS.md file.
pub const INIT: &str = include_str!("init.md");

/// The DEFAULT_SYSTEM prompt used as the default system prompt for the agent.
pub const DEFAULT_SYSTEM: &str = "You are Kimi, a helpful AI assistant. \
You have access to various tools to help users with their tasks. \
Use the tools when appropriate to provide accurate and helpful responses.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_prompt_loaded() {
        assert!(!INIT.is_empty());
        assert!(INIT.contains("AGENTS.md"));
        assert!(INIT.contains("project structure"));
    }

    #[test]
    fn test_default_system_prompt() {
        assert!(!DEFAULT_SYSTEM.is_empty());
        assert!(DEFAULT_SYSTEM.contains("Kimi"));
    }
}
