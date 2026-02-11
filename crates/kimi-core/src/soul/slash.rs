//! Slash command system for user commands
//!
//! Provides a registry and handler system for slash commands that users
//! can invoke during conversations (e.g., /help, /compact, /reset).

use std::collections::HashMap;
use std::sync::Arc;

// Forward declarations to avoid circular dependencies
// These will be resolved when the modules are compiled together
use super::compaction::Compaction;
use super::kimisoul::{KimiSoul, SoulError};

/// A slash command handler
pub type SlashHandler = Arc<dyn Fn(&mut KimiSoul, &str) -> Result<(), SoulError> + Send + Sync>;

/// A slash command definition
#[derive(Clone)]
pub struct SlashCommand {
    /// Command name (without the leading /)
    pub name: String,
    /// Command description for help text
    pub description: String,
    /// Handler function
    pub handler: SlashHandler,
}

impl SlashCommand {
    /// Create a new slash command
    pub fn new<F>(name: impl Into<String>, description: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&mut KimiSoul, &str) -> Result<(), SoulError> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            handler: Arc::new(handler),
        }
    }

    /// Execute the command
    pub fn execute(&self, soul: &mut KimiSoul, args: &str) -> Result<(), SoulError> {
        (self.handler)(soul, args)
    }
}

impl std::fmt::Debug for SlashCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlashCommand")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

/// Registry for slash commands
#[derive(Debug, Clone)]
pub struct SlashCommandRegistry {
    commands: HashMap<String, SlashCommand>,
}

impl SlashCommandRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Create a registry with default commands
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register a command
    pub fn register(&mut self, command: SlashCommand) {
        self.commands.insert(command.name.clone(), command);
    }

    /// Get a command by name
    pub fn get(&self, name: &str) -> Option<&SlashCommand> {
        self.commands.get(name)
    }

    /// Check if a command exists
    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    /// Remove a command
    pub fn remove(&mut self, name: &str) -> Option<SlashCommand> {
        self.commands.remove(name)
    }

    /// Get all command names
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.commands.keys()
    }

    /// Get all commands
    pub fn commands(&self) -> &HashMap<String, SlashCommand> {
        &self.commands
    }

    /// Register default commands
    fn register_defaults(&mut self) {
        // Help command
        self.register(SlashCommand::new(
            "help",
            "Show available slash commands",
            |soul, _args| {
                let registry = &soul.slash_commands;
                let mut help_text = String::from("Available commands:\n");
                for (name, cmd) in registry.commands() {
                    help_text.push_str(&format!("  /{} - {}\n", name, cmd.description));
                }
                // TODO: Send help text to user via wire
                let _ = help_text;
                Ok(())
            },
        ));

        // Compact command
        self.register(SlashCommand::new(
            "compact",
            "Compact the conversation context",
            |soul, _args| {
                let _ = soul.compaction.compact(&mut soul.context)
                    .map_err(|e| SoulError::Compaction(e.to_string()))?;
                Ok(())
            },
        ));

        // Reset command
        self.register(SlashCommand::new(
            "reset",
            "Reset the conversation context",
            |soul, _args| {
                soul.context.clear_messages();
                Ok(())
            },
        ));

        // Note: The yolo command is not included in defaults because
        // modifying approval settings requires interior mutability.
        // It can be added manually if needed with proper synchronization.
    }
}

impl Default for SlashCommandRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Parse a slash command from input
///
/// Returns `Some((command_name, args))` if the input starts with `/`,
/// `None` otherwise.
pub fn parse_slash_command(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim();
    if let Some(without_slash) = trimmed.strip_prefix('/') {
        let mut parts = without_slash.splitn(2, ' ');
        let command = parts.next()?;
        let args = parts.next().unwrap_or("");
        Some((command, args))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_command_new() {
        let cmd = SlashCommand::new("test", "Test command", |_soul, _args| Ok(()));
        assert_eq!(cmd.name, "test");
        assert_eq!(cmd.description, "Test command");
    }

    #[test]
    fn test_registry_new() {
        let registry = SlashCommandRegistry::new();
        assert!(registry.commands().is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = SlashCommandRegistry::new();
        let cmd = SlashCommand::new("test", "Test command", |_soul, _args| Ok(()));
        
        registry.register(cmd);
        
        assert!(registry.contains("test"));
        assert!(!registry.contains("other"));
        
        let retrieved = registry.get("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test");
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = SlashCommandRegistry::new();
        let cmd = SlashCommand::new("test", "Test command", |_soul, _args| Ok(()));
        
        registry.register(cmd);
        assert!(registry.contains("test"));
        
        registry.remove("test");
        assert!(!registry.contains("test"));
    }

    #[test]
    fn test_parse_slash_command() {
        assert_eq!(
            parse_slash_command("/help"),
            Some(("help", ""))
        );
        assert_eq!(
            parse_slash_command("/compact all"),
            Some(("compact", "all"))
        );
        assert_eq!(
            parse_slash_command("  /yolo on  "),
            Some(("yolo", "on"))
        );
        assert_eq!(
            parse_slash_command("normal message"),
            None
        );
        assert_eq!(
            parse_slash_command("  normal  "),
            None
        );
    }

    #[test]
    fn test_parse_slash_command_with_multiple_spaces() {
        assert_eq!(
            parse_slash_command("/command arg1 arg2"),
            Some(("command", "arg1 arg2"))
        );
    }
}
