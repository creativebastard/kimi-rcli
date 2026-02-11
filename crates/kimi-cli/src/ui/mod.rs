//! UI module for Kimi CLI
//!
//! Provides different user interface modes:
//! - ShellUI: Interactive shell with readline support
//! - PrintUI: Non-interactive mode for scripts and automation

mod print;
mod shell;

pub use print::PrintUI;
pub use shell::ShellUI;

use thiserror::Error;

/// Errors that can occur in the UI
#[derive(Error, Debug)]
pub enum UIError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Shell error: {0}")]
    Shell(String),

    #[error("Core error: {0}")]
    Core(String),

    #[error("User interrupted")]
    Interrupted,

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Result type for UI operations
pub type UIResult<T> = Result<T, UIError>;

/// Common trait for UI implementations
#[async_trait::async_trait]
pub trait UI: Send {
    /// Run the UI until completion
    async fn run(&mut self) -> UIResult<()>
    where
        Self: Sized;

    /// Display a message to the user
    fn message(&self, msg: &str);

    /// Display an error message
    fn error(&self, err: &str);
}
