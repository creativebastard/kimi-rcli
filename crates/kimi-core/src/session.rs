//! Session management for agent conversations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};
use uuid::Uuid;

/// Represents an agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub work_dir: PathBuf,
    pub context_file: PathBuf,
    pub wire_file: PathBuf,
    pub created_at: DateTime<Utc>,
}

impl Session {
    /// Create a new session with the given working directory
    pub fn new(work_dir: PathBuf) -> Self {
        let id = Uuid::new_v4();
        let session_dir = work_dir.join(".kimi").join("sessions").join(id.to_string());
        
        Self {
            id,
            work_dir,
            context_file: session_dir.join("context.json"),
            wire_file: session_dir.join("wire.jsonl"),
            created_at: Utc::now(),
        }
    }

    /// Create a new session with a specific ID
    pub fn with_id(id: Uuid, work_dir: PathBuf) -> Self {
        let session_dir = work_dir.join(".kimi").join("sessions").join(id.to_string());
        
        Self {
            id,
            work_dir,
            context_file: session_dir.join("context.json"),
            wire_file: session_dir.join("wire.jsonl"),
            created_at: Utc::now(),
        }
    }

    /// Load a session from a directory
    pub fn load(work_dir: PathBuf, session_id: Uuid) -> Result<Self, SessionError> {
        let session_file = work_dir
            .join(".kimi")
            .join("sessions")
            .join(session_id.to_string())
            .join("session.json");

        if !session_file.exists() {
            return Err(SessionError::NotFound(session_id.to_string()));
        }

        let content = std::fs::read_to_string(&session_file)?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// Save session metadata to disk
    pub fn save(&self) -> Result<(), SessionError> {
        let session_dir = self.session_dir();
        std::fs::create_dir_all(&session_dir)?;

        let session_file = session_dir.join("session.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&session_file, content)?;

        debug!("Session saved to {:?}", session_file);
        Ok(())
    }

    /// Get the session directory
    pub fn session_dir(&self) -> PathBuf {
        self.work_dir
            .join(".kimi")
            .join("sessions")
            .join(self.id.to_string())
    }

    /// Get the session ID as a string
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }

    /// Get a shortened version of the session ID (first 8 chars)
    pub fn short_id(&self) -> String {
        self.id.to_string()[..8].to_string()
    }

    /// Check if the session directory exists
    pub fn exists(&self) -> bool {
        self.session_dir().exists()
    }

    /// Create the session directory structure
    pub fn initialize(&self) -> Result<(), SessionError> {
        let session_dir = self.session_dir();
        std::fs::create_dir_all(&session_dir)?;
        
        // Create empty context file with proper structure
        let empty_context = crate::context::Context::new(self.context_file.clone());
        let context_json = serde_json::to_string_pretty(&empty_context)?;
        std::fs::write(&self.context_file, context_json)?;
        
        // Create empty wire file
        std::fs::write(&self.wire_file, "")?;
        
        // Save session metadata
        self.save()?;
        
        info!("Session {} initialized at {:?}", self.id, session_dir);
        Ok(())
    }

    /// Delete the session and all its data
    pub fn delete(&self) -> Result<(), SessionError> {
        let session_dir = self.session_dir();
        if session_dir.exists() {
            std::fs::remove_dir_all(&session_dir)?;
            info!("Session {} deleted", self.id);
        }
        Ok(())
    }

    /// List all sessions in the working directory
    pub fn list_all(work_dir: &Path) -> Result<Vec<Session>, SessionError> {
        let sessions_dir = work_dir.join(".kimi").join("sessions");
        
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        
        for entry in std::fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                let session_file = path.join("session.json");
                if session_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&session_file) {
                        if let Ok(session) = serde_json::from_str::<Session>(&content) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }

        // Sort by creation time, newest first
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        Ok(sessions)
    }
}

/// Session-related errors
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Session not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let work_dir = PathBuf::from("/tmp/test");
        let session = Session::new(work_dir.clone());
        
        assert_eq!(session.work_dir, work_dir);
        assert!(session.context_file.to_string_lossy().contains("context.json"));
        assert!(session.wire_file.to_string_lossy().contains("wire.jsonl"));
    }

    #[test]
    fn test_session_with_id() {
        let id = Uuid::new_v4();
        let work_dir = PathBuf::from("/tmp/test");
        let session = Session::with_id(id, work_dir);
        
        assert_eq!(session.id, id);
    }

    #[test]
    fn test_short_id() {
        let session = Session::new(PathBuf::from("/tmp/test"));
        let short = session.short_id();
        assert_eq!(short.len(), 8);
    }

    #[test]
    fn test_session_dir() {
        let session = Session::new(PathBuf::from("/tmp/test"));
        let dir = session.session_dir();
        assert!(dir.to_string_lossy().contains(".kimi/sessions"));
        assert!(dir.to_string_lossy().contains(&session.id.to_string()));
    }
}
