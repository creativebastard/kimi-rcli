//! LLM provider factory and integration
//!
//! This module provides a factory function to create LLM providers from configuration,
//! with support for OAuth token resolution.

use kosong_rs::{ChatProvider, KimiProvider};
use crate::auth::{load_token, OAuthRef};
use crate::config::{Config, ProviderType};
use secrecy::ExposeSecret;

/// Error type for LLM operations
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// No provider configured
    #[error("No provider configured")]
    NoProvider,
    /// OAuth token not found
    #[error("OAuth token not found")]
    MissingToken,
    /// Unsupported provider type
    #[error("Unsupported provider type: {0}")]
    UnsupportedProvider(String),
    /// Provider error
    #[error("Provider error: {0}")]
    ProviderError(String),
}

/// Create a chat provider from configuration
///
/// # Arguments
///
/// * `config` - The configuration containing provider and model settings
///
/// # Returns
///
/// A boxed ChatProvider if successful, or an LlmError if the provider cannot be created.
///
/// # Errors
///
/// Returns `LlmError::NoProvider` if the default model or its provider is not found.
/// Returns `LlmError::MissingToken` if OAuth is configured but the token cannot be loaded.
/// Returns `LlmError::UnsupportedProvider` if the provider type is not supported.
/// Returns `LlmError::ProviderError` if the provider fails to initialize.
pub async fn create_provider(
    config: &Config,
) -> Result<Box<dyn ChatProvider>, LlmError> {
    // Get the default model's provider
    let model = config.models.get(&config.default_model)
        .ok_or(LlmError::NoProvider)?;
    
    let provider_config = config.providers.get(&model.provider)
        .ok_or(LlmError::NoProvider)?;
    
    // Resolve API key (from OAuth or direct)
    let api_key = if let Some(oauth_ref) = &provider_config.oauth {
        let token = load_token(oauth_ref)
            .ok_or(LlmError::MissingToken)?;
        token.access_token
    } else {
        provider_config.api_key.expose_secret().to_string()
    };
    
    // Create provider based on type
    match provider_config.provider_type {
        ProviderType::Kimi => {
            let provider = KimiProvider::with_base_url(
                api_key,
                model.name.clone(),  // model name
                provider_config.base_url.clone(),
            ).map_err(|e| LlmError::ProviderError(e.to_string()))?;
            
            Ok(Box::new(provider))
        }
        _ => Err(LlmError::UnsupportedProvider(
            format!("{:?}", provider_config.provider_type)
        )),
    }
}

/// Create a chat provider for a specific model
///
/// # Arguments
///
/// * `config` - The configuration containing provider and model settings
/// * `model_name` - The name of the model to use
///
/// # Returns
///
/// A boxed ChatProvider if successful, or an LlmError if the provider cannot be created.
pub async fn create_provider_for_model(
    config: &Config,
    model_name: &str,
) -> Result<Box<dyn ChatProvider>, LlmError> {
    // Get the specified model's provider
    let model = config.models.get(model_name)
        .ok_or(LlmError::NoProvider)?;
    
    let provider_config = config.providers.get(&model.provider)
        .ok_or(LlmError::NoProvider)?;
    
    // Resolve API key (from OAuth or direct)
    let api_key = if let Some(oauth_ref) = &provider_config.oauth {
        let token = load_token(oauth_ref)
            .ok_or(LlmError::MissingToken)?;
        token.access_token
    } else {
        provider_config.api_key.expose_secret().to_string()
    };
    
    // Create provider based on type
    match provider_config.provider_type {
        ProviderType::Kimi => {
            let provider = KimiProvider::with_base_url(
                api_key,
                model.name.clone(),  // model name
                provider_config.base_url.clone(),
            ).map_err(|e| LlmError::ProviderError(e.to_string()))?;
            
            Ok(Box::new(provider))
        }
        _ => Err(LlmError::UnsupportedProvider(
            format!("{:?}", provider_config.provider_type)
        )),
    }
}

/// Get the OAuth reference for a provider if configured
///
/// # Arguments
///
/// * `config` - The configuration
/// * `provider_name` - The name of the provider
///
/// # Returns
///
/// The OAuth reference if configured, None otherwise.
pub fn get_oauth_ref<'a>(config: &'a Config, provider_name: &str) -> Option<&'a OAuthRef> {
    config.providers
        .get(provider_name)
        .and_then(|p| p.oauth.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmProvider;
    use crate::types::LlmModel;
    use secrecy::SecretString;
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        let mut models = HashMap::new();
        models.insert(
            "test-model".to_string(),
            LlmModel {
                name: "kimi-test-model".to_string(),
                provider: "test-provider".to_string(),
                max_tokens: Some(128000),
                temperature: None,
            },
        );

        let mut providers = HashMap::new();
        providers.insert(
            "test-provider".to_string(),
            LlmProvider {
                provider_type: ProviderType::Kimi,
                base_url: "https://api.moonshot.cn/v1".to_string(),
                api_key: SecretString::new("test-api-key".to_string()),
                env: None,
                custom_headers: None,
                oauth: None,
            },
        );

        Config {
            default_model: "test-model".to_string(),
            default_thinking: false,
            default_yolo: false,
            models,
            providers,
            loop_control: crate::types::LoopControl::default(),
            services: crate::types::Services::default(),
            mcp: crate::types::McpConfig::default(),
            is_from_default_location: false,
        }
    }

    #[test]
    fn test_get_oauth_ref() {
        let config = create_test_config();
        
        // Provider without OAuth
        let oauth_ref = get_oauth_ref(&config, "test-provider");
        assert!(oauth_ref.is_none());
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::NoProvider;
        assert_eq!(err.to_string(), "No provider configured");

        let err = LlmError::MissingToken;
        assert_eq!(err.to_string(), "OAuth token not found");

        let err = LlmError::UnsupportedProvider("custom".to_string());
        assert_eq!(err.to_string(), "Unsupported provider type: custom");

        let err = LlmError::ProviderError("connection failed".to_string());
        assert_eq!(err.to_string(), "Provider error: connection failed");
    }
}
