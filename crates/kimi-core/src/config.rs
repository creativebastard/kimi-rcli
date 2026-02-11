//! Configuration types for the agent system

use crate::auth::OAuthRef;
use crate::types::{LoopControl, McpConfig, Services};
use crate::LlmModel;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_model: String,
    pub default_thinking: bool,
    pub default_yolo: bool,
    pub models: HashMap<String, LlmModel>,
    pub providers: HashMap<String, LlmProvider>,
    pub loop_control: LoopControl,
    pub services: Services,
    pub mcp: McpConfig,
    /// Whether the config was loaded from the default location
    #[serde(skip)]
    pub is_from_default_location: bool,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from a YAML file
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Save configuration to a YAML file
    pub fn to_yaml<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get a provider by name
    pub fn get_provider(&self, name: &str) -> Option<&LlmProvider> {
        self.providers.get(name)
    }

    /// Get a model by name
    pub fn get_model(&self, name: &str) -> Option<&LlmModel> {
        self.models.get(name)
    }
}

/// LLM Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProvider {
    pub provider_type: ProviderType,
    pub base_url: String,
    #[serde(skip_serializing, default = "default_secret")]
    pub api_key: SecretString,
    pub env: Option<HashMap<String, String>>,
    pub custom_headers: Option<HashMap<String, String>>,
    /// OAuth credential reference (do not store tokens here)
    pub oauth: Option<OAuthRef>,
}

fn default_secret() -> SecretString {
    SecretString::new(String::new())
}

impl LlmProvider {
    /// Create a new provider configuration
    pub fn new(
        provider_type: ProviderType,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            provider_type,
            base_url: base_url.into(),
            api_key: SecretString::new(api_key.into()),
            env: None,
            custom_headers: None,
            oauth: None,
        }
    }

    /// Set environment variables for the provider
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }

    /// Set custom headers for the provider
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.custom_headers = Some(headers);
        self
    }

    /// Set OAuth reference for the provider
    pub fn with_oauth(mut self, oauth: OAuthRef) -> Self {
        self.oauth = Some(oauth);
        self
    }
}

/// Provider type enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Kimi,
    OpenAiLegacy,
    OpenAiResponses,
    Anthropic,
    Gemini,
    VertexAi,
}

impl ProviderType {
    /// Get the default base URL for this provider type
    pub fn default_base_url(&self) -> &str {
        match self {
            ProviderType::Kimi => "https://api.moonshot.cn/v1",
            ProviderType::OpenAiLegacy => "https://api.openai.com/v1",
            ProviderType::OpenAiResponses => "https://api.openai.com/v1/responses",
            ProviderType::Anthropic => "https://api.anthropic.com/v1",
            ProviderType::Gemini => "https://generativelanguage.googleapis.com/v1",
            ProviderType::VertexAi => "https://aiplatform.googleapis.com/v1",
        }
    }
}

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

/// Load configuration from the default location or specified path
pub fn load_config(path: Option<&Path>) -> Result<Config, ConfigError> {
    use std::path::PathBuf;
    
    let default_path = dirs::config_dir()
        .map(|d| d.join("kimi").join("config.toml"))
        .unwrap_or_else(|| PathBuf::from(".kimi/config.toml"));
    
    let (path, is_default) = match path {
        Some(p) => (p.to_path_buf(), false),
        None => (default_path, true),
    };
    
    let mut config = if path.exists() {
        Config::from_file(&path)?
    } else {
        // Return default config
        Config {
            default_model: String::new(),
            default_thinking: false,
            default_yolo: false,
            models: HashMap::new(),
            providers: HashMap::new(),
            loop_control: LoopControl::default(),
            services: Services::default(),
            mcp: McpConfig::default(),
            is_from_default_location: is_default,
        }
    };
    
    config.is_from_default_location = is_default;
    Ok(config)
}

/// Save configuration to the default location or specified path
pub fn save_config(config: &Config, path: Option<&Path>) -> Result<(), ConfigError> {
    use std::path::PathBuf;
    use std::fs;
    
    let path = path.map(PathBuf::from).unwrap_or_else(|| {
        dirs::config_dir()
            .map(|d| d.join("kimi").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from(".kimi/config.toml"))
    });
    
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    config.to_file(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn test_provider_type_default_urls() {
        assert_eq!(
            ProviderType::Kimi.default_base_url(),
            "https://api.moonshot.cn/v1"
        );
        assert_eq!(
            ProviderType::OpenAiLegacy.default_base_url(),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            ProviderType::Anthropic.default_base_url(),
            "https://api.anthropic.com/v1"
        );
    }

    #[test]
    fn test_provider_builder() {
        let provider = LlmProvider::new(
            ProviderType::Kimi,
            "https://custom.api.com",
            "test-api-key",
        )
        .with_env(HashMap::from([("KEY".to_string(), "VALUE".to_string())]));

        assert!(matches!(provider.provider_type, ProviderType::Kimi));
        assert_eq!(provider.base_url, "https://custom.api.com");
        assert!(provider.env.is_some());
    }

    #[test]
    fn test_config_deserialization_without_api_key() {
        // This tests that OAuth-based providers can be loaded without an api_key field
        let config_str = r#"
default_model = "kimi-code/kimi-for-coding"
default_thinking = false
default_yolo = false

[models."kimi-code/kimi-for-coding"]
name = "kimi-code/kimi-for-coding"
provider = "managed:kimi-code"
max_tokens = 128000

[providers."managed:kimi-code"]
provider_type = "kimi"
base_url = "https://api.kimi.com/coding/v1"

[providers."managed:kimi-code".oauth]
storage = "file"
key = "oauth/kimi-code"

[loop_control]
max_iterations = 100
timeout_seconds = 300

[services]
enabled = []
config = {}

[mcp]
servers = []
"#;
        
        let result: Result<Config, _> = toml::from_str(config_str);
        assert!(result.is_ok(), "Failed to parse config: {:?}", result.err());
        
        let config = result.unwrap();
        assert_eq!(config.default_model, "kimi-code/kimi-for-coding");
        assert!(config.providers.contains_key("managed:kimi-code"));
        
        let provider = config.providers.get("managed:kimi-code").unwrap();
        assert!(provider.oauth.is_some());
        // api_key should default to empty string
        assert_eq!(provider.api_key.expose_secret(), "");
    }
}
