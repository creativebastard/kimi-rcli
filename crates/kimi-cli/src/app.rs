//! Application orchestration for Kimi CLI
//!
//! This module provides the main application structure that coordinates
//! between the CLI, core systems, and UI layers.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing::{debug, info};

use kimi_core::{
    Approval, Config, Context, Session,
    config::ConfigError,
    context::ContextError,
    session::SessionError,
    soul::{KimiSoul, SoulError, Agent, SimpleCompaction},
    types::LoopControl,
};

use crate::cli::Cli;
use crate::ui::{ShellUI, PrintUI, UIError};

/// Default agent file path
#[allow(dead_code)]
const DEFAULT_AGENT_FILE: &str = "agent.toml";

/// Application errors
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Session error: {0}")]
    Session(#[from] SessionError),
    
    #[error("Context error: {0}")]
    Context(#[from] ContextError),
    
    #[error("Soul error: {0}")]
    Soul(#[from] SoulError),
    
    #[error("UI error: {0}")]
    Ui(#[from] UIError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Provider error: {0}")]
    Provider(String),
    
    #[error("Agent not found: {0}")]
    AgentNotFound(PathBuf),
}

/// Main application structure
pub struct App {
    config: Config,
    #[allow(dead_code)]
    session: Session,
    context: Context,
    approval: Arc<Approval>,
    agent: Option<Agent>,
    cli: Cli,
}

impl App {
    /// Create a new application instance from CLI arguments
    pub async fn create(cli: &Cli) -> Result<Self, AppError> {
        info!("Creating application instance");

        // Load configuration
        let config = load_config(cli.config_file.as_ref()).await?;
        debug!("Configuration loaded successfully");

        // Create or load session
        let session: Session = create_session(cli).await?;
        debug!("Session created: {}", session.id_string());

        // Initialize context
        let context = Context::load(session.context_file.clone())?;
        debug!("Context loaded with {} messages", context.message_count());

        // Create approval manager
        let approval = if cli.yolo {
            Approval::yolo()
        } else {
            Approval::new()
        };

        Ok(Self {
            config,
            session,
            context,
            approval: Arc::new(approval),
            agent: None,
            cli: cli.clone(),
        })
    }

    /// Initialize the agent
    pub async fn initialize(&mut self) -> Result<(), AppError> {
        info!("Initializing agent");

        // Create agent
        let agent = Agent::new(
            "kimi",
            "A helpful AI assistant",
        );
        
        self.agent = Some(agent);
        debug!("Agent initialized");

        Ok(())
    }

    /// Run the interactive shell mode
    pub async fn run_shell(mut self) -> Result<(), AppError> {
        info!("Starting shell mode");

        // Ensure agent is initialized
        if self.agent.is_none() {
            self.initialize().await?;
        }

        // Create KimiSoul
        let agent = self.agent.take().unwrap();
        let denwa_renji = Arc::new(kimi_core::soul::DenwaRenji::new());
        let loop_control = self.config.loop_control.clone();
        let compaction = SimpleCompaction::new(4000);
        
        let mut soul = KimiSoul::new(
            agent,
            self.context,
            self.approval.clone(),
            denwa_renji,
            loop_control,
            compaction,
        );

        // Create and run shell UI
        let mut shell = ShellUI::new(self.cli).await?;
        shell.run_with_soul(&mut soul).await?;

        Ok(())
    }

    /// Run the print mode (non-interactive)
    pub async fn run_print(mut self, prompt: &str) -> Result<(), AppError> {
        info!("Starting print mode with prompt: {}", prompt);

        // Ensure agent is initialized
        if self.agent.is_none() {
            self.initialize().await?;
        }

        // Create KimiSoul
        let agent = self.agent.take().unwrap();
        let denwa_renji = Arc::new(kimi_core::soul::DenwaRenji::new());
        let loop_control = self.config.loop_control.clone();
        let compaction = SimpleCompaction::new(4000);
        
        let mut soul = KimiSoul::new(
            agent,
            self.context,
            self.approval.clone(),
            denwa_renji,
            loop_control,
            compaction,
        );

        // Create and run print UI
        let mut print_ui = PrintUI::new(self.cli)?;
        print_ui.run_with_soul(&mut soul, prompt).await?;

        Ok(())
    }

    /// Continue an existing session
    pub async fn run_continue(mut self) -> Result<(), AppError> {
        info!("Continuing existing session");

        // Ensure agent is initialized
        if self.agent.is_none() {
            self.initialize().await?;
        }

        // Create KimiSoul
        let agent = self.agent.take().unwrap();
        let denwa_renji = Arc::new(kimi_core::soul::DenwaRenji::new());
        let loop_control = self.config.loop_control.clone();
        let compaction = SimpleCompaction::new(4000);
        
        let mut soul = KimiSoul::new(
            agent,
            self.context,
            self.approval.clone(),
            denwa_renji,
            loop_control,
            compaction,
        );

        // If there's a prompt, run print mode; otherwise, run shell mode
        let cli = self.cli.clone();
        if let Some(ref prompt) = self.cli.prompt {
            let mut print_ui = PrintUI::new(cli)?;
            print_ui.run_with_soul(&mut soul, prompt).await?;
        } else {
            let mut shell = ShellUI::new(cli).await?;
            shell.run_with_soul(&mut soul).await?;
        }

        Ok(())
    }
}

/// Load configuration from file or create default
async fn load_config(config_path: Option<&PathBuf>) -> Result<Config, ConfigError> {
    if let Some(path) = config_path {
        info!("Loading configuration from: {:?}", path);
        if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
            Config::from_yaml(path)
        } else {
            Config::from_file(path)
        }
    } else {
        // Try to load from default locations
        let default_paths = [
            PathBuf::from("kimi.toml"),
            PathBuf::from(".kimi/config.toml"),
            dirs::config_dir()
                .map(|d| d.join("kimi/config.toml"))
                .unwrap_or_else(|| PathBuf::from("/etc/kimi/config.toml")),
        ];

        for path in &default_paths {
            if path.exists() {
                info!("Loading configuration from: {:?}", path);
                return Config::from_file(path);
            }
        }

        // Create default configuration
        info!("No configuration file found, using defaults");
        create_default_config()
    }
}

/// Create a default configuration
fn create_default_config() -> Result<Config, ConfigError> {
    use std::collections::HashMap;
    use kimi_core::{LlmModel, LlmProvider, ProviderType, types::{Services, McpConfig}};

    let mut models = HashMap::new();
    models.insert(
        "kimi-k2".to_string(),
        LlmModel {
            name: "kimi-k2-0711-preview".to_string(),
            provider: "kimi".to_string(),
            max_tokens: Some(8192),
            temperature: Some(0.7),
        },
    );

    let mut providers = HashMap::new();
    providers.insert(
        "kimi".to_string(),
        LlmProvider::new(
            ProviderType::Kimi,
            ProviderType::Kimi.default_base_url(),
            std::env::var("KIMI_API_KEY").unwrap_or_default(),
        ),
    );

    Ok(Config {
        default_model: "kimi-k2".to_string(),
        default_thinking: false,
        default_yolo: false,
        models,
        providers,
        loop_control: LoopControl {
            max_iterations: 50,
            timeout_seconds: 300,
        },
        services: Services {
            enabled: vec![],
            config: HashMap::new(),
        },
        mcp: McpConfig {
            servers: vec![],
            enabled_tools: None,
        },
        is_from_default_location: true,
    })
}

/// Create or load a session based on CLI arguments
async fn create_session(cli: &Cli) -> Result<Session, SessionError> {
    let work_dir = cli.effective_work_dir();

    if cli.continue_ {
        // Try to load the most recent session
        let sessions = Session::list_all(&work_dir)?;
        if let Some(session) = sessions.into_iter().next() {
            info!("Continuing session: {}", session.id_string());
            return Ok(session);
        }
        // No existing session, create a new one
        info!("No existing session found, creating new one");
    }

    if let Some(session_name) = cli.session_name() {
        // Try to load the named session
        if let Ok(session_id) = uuid::Uuid::parse_str(&session_name) {
            let session = Session::load(work_dir, session_id)?;
            info!("Loaded session: {}", session.id_string());
            return Ok(session);
        }
        // If not a UUID, create a new session with that as identifier
        // (This is a simplified approach - in practice you might want to map names to IDs)
    }

    // Create a new session
    let session = Session::new(work_dir);
    session.initialize()?;
    info!("Created new session: {}", session.id_string());
    
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::env;

    #[tokio::test]
    async fn test_app_creation() {
        // Create a temp directory for the test
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();
        
        // Create the .kimi/sessions directory structure
        let kimi_dir = temp_path.join(".kimi");
        let sessions_dir = kimi_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        
        // Create a minimal valid config file
        let config_content = r#"
default_model = "kimi-k2"
default_thinking = false
default_yolo = false

[models.kimi-k2]
name = "kimi-k2"
provider = "kimi"
max_tokens = 8192

[providers.kimi]
provider_type = "kimi"
base_url = "https://api.moonshot.cn/v1"
api_key = ""

[loop_control]
max_iterations = 100
timeout_seconds = 300

[services]
enabled = []
config = {}

[mcp]
servers = []
enabled_tools = []
"#;
        std::fs::write(kimi_dir.join("config.toml"), config_content).unwrap();
        
        // Change to temp directory for session creation
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_path).unwrap();
        
        let cli = Cli::parse_from(["kimi", "--yolo"]);
        let result = App::create(&cli).await;
        
        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
        
        // Print error for debugging if it fails
        if let Err(ref e) = result {
            eprintln!("App creation failed: {}", e);
        }
        
        // Should succeed with valid config file
        assert!(result.is_ok());
    }
}
