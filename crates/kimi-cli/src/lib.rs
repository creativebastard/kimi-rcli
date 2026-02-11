//! Kimi CLI - Your next CLI agent
//!
//! This crate provides the command-line interface and user interaction
//! layer for the Kimi agent system.

pub mod app;
pub mod cli;
pub mod commands;
pub mod ui;

pub use cli::{Cli, Commands, McpCommands};
