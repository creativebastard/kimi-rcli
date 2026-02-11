//! # kaos-rs
//!
//! An OS abstraction layer for async file operations and command execution.
//!
//! This crate provides a high-level, async-friendly interface for common
//! operating system operations like file I/O and process execution. It is
//! built on top of Tokio and follows Rust best practices.
//!
//! ## Features
//!
//! - **Path Abstraction**: [`KaosPath`] provides async methods for file operations
//! - **Process Execution**: [`Command`] and [`Process`] for running external commands
//! - **Stream Abstractions**: [`LineReader`], [`CountingWriter`], and stream extensions
//! - **Error Handling**: Comprehensive error types via [`KaosError`]
//!
//! ## Quick Start
//!
//! ```
//! use kaos_rs::{KaosPath, Command};
//!
//! # async fn example() -> kaos_rs::Result<()> {
//! // File operations
//! let path = KaosPath::cwd().join("example.txt");
//! path.write_file("Hello, World!").await?;
//! let content = path.read_file().await?;
//!
//! // Process execution
//! let output = Command::new("echo")
//!     .arg("Hello from kaos!")
//!     .output()
//!     .await?;
//!
//! if output.success() {
//!     println!("{}", output.stdout_str().unwrap());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Modules
//!
//! - [`path`]: Path abstraction and file operations
//! - [`exec`]: Process execution and command running
//! - [`stream`]: Async stream utilities and extensions
//! - [`error`]: Error types and results

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod error;
pub mod exec;
pub mod path;
pub mod stream;

// Re-export main types for convenience
pub use error::{KaosError, Result};
pub use exec::{Command, CommandOutput, Output, Process};
pub use path::KaosPath;
pub use stream::{AsyncReadable, AsyncWritable, CountingWriter, LineReader, StreamExt};

// Re-export stream extension traits
pub use stream::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

/// Prelude module for convenient imports.
///
/// This module re-exports the most commonly used types and traits.
/// Import it with `use kaos_rs::prelude::*;`
pub mod prelude {
    pub use crate::error::{KaosError, Result};
    pub use crate::exec::{Command, CommandOutput, Output, Process};
    pub use crate::path::KaosPath;
    pub use crate::stream::{AsyncReadable, AsyncWritable, CountingWriter, LineReader, StreamExt};
    pub use crate::stream::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_path_operations() {
        let temp_dir = std::env::temp_dir();
        let test_path = KaosPath::from(temp_dir).join("kaos_test.txt");

        // Test write and read
        test_path.write_file("test content").await.unwrap();
        assert!(test_path.exists().await);
        assert!(test_path.is_file().await);

        let content = test_path.read_file().await.unwrap();
        assert_eq!(content, "test content");

        // Cleanup
        tokio::fs::remove_file(test_path.as_path()).await.unwrap();
    }

    #[tokio::test]
    async fn test_command_execution() {
        let output = Command::new("echo")
            .arg("hello")
            .output()
            .await
            .unwrap();

        assert!(output.success());
        assert!(output.stdout_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_path_join_and_parent() {
        let path = KaosPath::from("/tmp").join("test").join("file.txt");
        assert_eq!(path.as_path(), std::path::Path::new("/tmp/test/file.txt"));

        let parent = path.parent().unwrap();
        assert_eq!(parent.as_path(), std::path::Path::new("/tmp/test"));

        let file_name = path.file_name().unwrap();
        assert_eq!(file_name, std::ffi::OsStr::new("file.txt"));
    }

    #[tokio::test]
    async fn test_command_with_env() {
        // Use `env` command to check environment variables
        let output = Command::new("sh")
            .arg("-c")
            .arg("echo $TEST_VAR")
            .env("TEST_VAR", "test_value")
            .output()
            .await
            .unwrap();

        assert!(output.success());
        assert!(output.stdout_str().unwrap().contains("test_value"));
    }
}
