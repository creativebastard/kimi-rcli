//! Skills system - modular extensions for specialized knowledge

use std::path::PathBuf;

pub mod discovery;
pub mod flow;
pub mod frontmatter;

pub use discovery::SkillDiscovery;
pub use flow::{Flow, FlowEdge, FlowNode, NodeType};
pub use frontmatter::Frontmatter;

/// A discovered skill
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub skill_type: SkillType,
    pub dir: PathBuf,
    pub flow: Option<Flow>,
}

/// Type of skill
#[derive(Debug, Clone, PartialEq)]
pub enum SkillType {
    /// Standard skill with documentation
    Standard,
    /// Flow-based skill with nodes and edges
    Flow,
}

/// Errors that can occur when parsing skills
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid flow diagram: {0}")]
    InvalidFlow(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
