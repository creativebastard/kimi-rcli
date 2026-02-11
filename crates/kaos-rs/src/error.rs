//! Error types for kaos-rs.

use std::io;
use thiserror::Error;

/// The main error type for kaos-rs operations.
#[derive(Error, Debug)]
pub enum KaosError {
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// The path is not valid UTF-8.
    #[error("path contains invalid UTF-8")]
    InvalidUtf8,

    /// The path does not exist.
    #[error("path does not exist: {0}")]
    NotFound(String),

    /// The path is not a file.
    #[error("not a file: {0}")]
    NotAFile(String),

    /// The path is not a directory.
    #[error("not a directory: {0}")]
    NotADirectory(String),

    /// A process execution error occurred.
    #[error("process error: {0}")]
    Process(String),

    /// The process was terminated by a signal.
    #[error("process terminated by signal")]
    TerminatedBySignal,

    /// A generic error with a message.
    #[error("{0}")]
    Other(String),
}

/// A specialized result type for kaos-rs operations.
pub type Result<T> = std::result::Result<T, KaosError>;
