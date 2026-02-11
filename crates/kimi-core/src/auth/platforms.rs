//! Platform configuration and model management

use crate::auth::oauth::OAuthError;
use secrecy::ExposeSecret;
use crate::auth::KIMI_CODE_PLATFORM_ID;
use reqwest::Client;
use serde::Deserialize;

/// Platform definition
#[derive(Debug, Clone)]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub search_url: Option<String>,
    pub fetch_url: Option<String>,
    pub allowed_prefixes: Option<Vec<String>>,
}

/// Model info from API
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub context_length: usize,
    pub supports_reasoning: bool,
    pub supports_image_in: bool,
    pub supports_video_in: bool,
}

impl ModelInfo {
    /// Get capabilities from model info
    pub fn capabilities(&self) -> Vec<crate::auth::ModelCapability> {
        use crate::auth::ModelCapability;
        let mut caps = Vec::new();

        if self.supports_reasoning {
            caps.push(ModelCapability::Thinking);
        }

        // Models with "thinking" in name are always-thinking
        if self.id.to_lowercase().contains("thinking") {
            caps.push(ModelCapability::Thinking);
            caps.push(ModelCapability::AlwaysThinking);
        }

        if self.supports_image_in {
            caps.push(ModelCapability::ImageIn);
        }

        if self.supports_video_in {
            caps.push(ModelCapability::VideoIn);
        }

        // Special case for kimi-k2.5
        if self.id.to_lowercase().contains("kimi-k2.5") {
            caps.push(ModelCapability::Thinking);
            caps.push(ModelCapability::ImageIn);
            caps.push(ModelCapability::VideoIn);
        }

        // Deduplicate
        caps.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
        caps.dedup_by(|a, b| format!("{:?}", a) == format!("{:?}", b));

        caps
    }
}

/// Get Kimi Code base URL from environment or default
fn kimi_code_base_url() -> String {
    std::env::var("KIMI_CODE_BASE_URL")
        .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".to_string())
}

/// List of supported platforms
fn platforms() -> Vec<Platform> {
    vec![
        Platform {
            id: KIMI_CODE_PLATFORM_ID.to_string(),
            name: "Kimi Code".to_string(),
            base_url: kimi_code_base_url(),
            search_url: Some(format!("{}/search", kimi_code_base_url())),
            fetch_url: Some(format!("{}/fetch", kimi_code_base_url())),
            allowed_prefixes: None,
        },
        Platform {
            id: "moonshot-cn".to_string(),
            name: "Moonshot AI Open Platform (moonshot.cn)".to_string(),
            base_url: "https://api.moonshot.cn/v1".to_string(),
            search_url: None,
            fetch_url: None,
            allowed_prefixes: Some(vec!["kimi-k".to_string()]),
        },
        Platform {
            id: "moonshot-ai".to_string(),
            name: "Moonshot AI Open Platform (moonshot.ai)".to_string(),
            base_url: "https://api.moonshot.ai/v1".to_string(),
            search_url: None,
            fetch_url: None,
            allowed_prefixes: Some(vec!["kimi-k".to_string()]),
        },
    ]
}

/// Get platform by ID
pub fn get_platform_by_id(id: &str) -> Option<Platform> {
    platforms().into_iter().find(|p| p.id == id)
}

/// List all platforms
pub fn list_platforms() -> Vec<Platform> {
    platforms()
}

/// List models for a platform
pub async fn list_models(platform: &Platform, api_key: &str) -> Result<Vec<ModelInfo>, OAuthError> {
    let client = Client::new();
    let url = format!("{}/models", platform.base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    response.error_for_status_ref()?;

    let resp_json: serde_json::Value = response.json().await?;
    let data = resp_json
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| OAuthError::General("Unexpected models response".to_string()))?;

    let mut result = Vec::new();
    for item in data {
        let model_id = item.get("id").and_then(|v| v.as_str());
        if model_id.is_none() {
            continue;
        }

        result.push(ModelInfo {
            id: model_id.unwrap().to_string(),
            context_length: item
                .get("context_length")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize,
            supports_reasoning: item
                .get("supports_reasoning")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            supports_image_in: item
                .get("supports_image_in")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            supports_video_in: item
                .get("supports_video_in")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        });
    }

    // Filter by allowed prefixes if specified
    if let Some(ref prefixes) = platform.allowed_prefixes {
        result.retain(|model| prefixes.iter().any(|prefix| model.id.starts_with(prefix)));
    }

    Ok(result)
}

/// Managed provider key format
pub fn managed_provider_key(platform_id: &str) -> String {
    format!("managed:{}", platform_id)
}

/// Managed model key format
pub fn managed_model_key(platform_id: &str, model_id: &str) -> String {
    format!("{}/{}", platform_id, model_id)
}

/// Check if provider key is managed
pub fn is_managed_provider_key(key: &str) -> bool {
    key.starts_with("managed:")
}

/// Parse managed provider key
pub fn parse_managed_provider_key(key: &str) -> Option<String> {
    key.strip_prefix("managed:").map(|s| s.to_string())
}

/// Get platform name for provider key
pub fn get_platform_name_for_provider(provider_key: &str) -> Option<String> {
    let platform_id = parse_managed_provider_key(provider_key)?;
    get_platform_by_id(&platform_id).map(|p| p.name)
}

/// Refresh managed models in config
pub async fn refresh_managed_models(config: &mut Config) -> Result<bool, OAuthError> {
    use crate::auth::storage::load_token;
    use crate::config::LlmProvider;

    let managed_providers: Vec<(String, LlmProvider)> = config
        .providers
        .iter()
        .filter(|(k, _)| is_managed_provider_key(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    if managed_providers.is_empty() {
        return Ok(false);
    }

    let mut changed = false;

    for (provider_key, provider) in managed_providers {
        let platform_id = match parse_managed_provider_key(&provider_key) {
            Some(id) => id,
            None => continue,
        };

        let platform = match get_platform_by_id(&platform_id) {
            Some(p) => p,
            None => {
                tracing::warn!("Managed platform not found: {}", platform_id);
                continue;
            }
        };

        // Get API key from OAuth or config
        let api_key = if let Some(ref oauth) = provider.oauth {
            match load_token(oauth) {
                Some(token) => token.access_token,
                None => continue,
            }
        } else {
            provider.api_key.expose_secret().to_string()
        };

        if api_key.is_empty() {
            tracing::warn!("Missing API key for managed provider: {}", provider_key);
            continue;
        }

        let models = match list_models(&platform, &api_key).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(
                    "Failed to refresh models for {}: {}",
                    platform_id,
                    e
                );
                continue;
            }
        };

        if apply_models(config, &provider_key, &platform_id, &models) {
            changed = true;
        }
    }

    Ok(changed)
}

/// Apply models to config
fn apply_models(
    config: &mut Config,
    provider_key: &str,
    platform_id: &str,
    models: &[ModelInfo],
) -> bool {
    use crate::types::LlmModel;

    let mut changed = false;
    let mut model_keys: Vec<String> = Vec::new();

    for model in models {
        let model_key = managed_model_key(platform_id, &model.id);
        model_keys.push(model_key.clone());

        let existing = config.models.get(&model_key);
        let capabilities = model.capabilities();
        let _caps_vec: Vec<String> = capabilities
            .iter()
            .map(|c| format!("{:?}", c).to_lowercase())
            .collect();

        match existing {
            None => {
                let new_model = LlmModel {
                    name: model.id.clone(),
                    provider: provider_key.to_string(),
                    max_tokens: Some(model.context_length),
                    temperature: None,
                };
                config.models.insert(model_key, new_model);
                changed = true;
            }
            Some(existing_model) => {
                if existing_model.provider != provider_key {
                    changed = true;
                }
                if existing_model.name != model.id {
                    changed = true;
                }
                if existing_model.max_tokens != Some(model.context_length) {
                    changed = true;
                }
            }
        }
    }

    // Remove models that no longer exist
    let model_keys_set: std::collections::HashSet<_> = model_keys.iter().cloned().collect();
    let to_remove: Vec<String> = config
        .models
        .iter()
        .filter(|(_, m)| m.provider == provider_key)
        .filter(|(k, _)| !model_keys_set.contains(*k))
        .map(|(k, _)| k.clone())
        .collect();

    let mut removed_default = false;
    for key in to_remove {
        if config.default_model == key {
            removed_default = true;
        }
        config.models.remove(&key);
        changed = true;
    }

    // Update default model if needed
    if removed_default {
        config.default_model = model_keys.first().cloned().unwrap_or_default();
        changed = true;
    }

    if !config.default_model.is_empty() && !config.models.contains_key(&config.default_model) {
        config.default_model = config.models.keys().next().cloned().unwrap_or_default();
        changed = true;
    }

    changed
}

// Need to import Config for refresh_managed_models
use crate::config::Config;
