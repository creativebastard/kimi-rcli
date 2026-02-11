//! OAuth2 device authorization flow implementation

use crate::auth::{
    storage::{delete_token, get_device_id, save_token, OAuthRef},
    ModelCapability, KIMI_CODE_CLIENT_ID, KIMI_CODE_OAUTH_KEY, KIMI_CODE_PLATFORM_ID,
    REFRESH_THRESHOLD_SECONDS,
};
use crate::config::Config;
use crate::types::{LlmModel, Services};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use tracing::{debug, warn};

/// OAuth token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: f64,
    pub scope: String,
    pub token_type: String,
}

impl OAuthToken {
    /// Create an OAuthToken from a JSON response
    pub fn from_response(payload: serde_json::Value) -> Result<Self, OAuthError> {
        let expires_in = payload
            .get("expires_in")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| OAuthError::General("Missing expires_in in response".to_string()))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Ok(Self {
            access_token: payload
                .get("access_token")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OAuthError::General("Missing access_token".to_string()))?
                .to_string(),
            refresh_token: payload
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OAuthError::General("Missing refresh_token".to_string()))?
                .to_string(),
            expires_at: now + expires_in,
            scope: payload
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            token_type: payload
                .get("token_type")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// Convert to JSON value
    pub fn to_dict(&self) -> serde_json::Value {
        json!({
            "access_token": self.access_token,
            "refresh_token": self.refresh_token,
            "expires_at": self.expires_at,
            "scope": self.scope,
            "token_type": self.token_type,
        })
    }

    /// Create from JSON value
    pub fn from_dict(payload: serde_json::Value) -> Result<Self, OAuthError> {
        let expires_at = payload
            .get("expires_at")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        Ok(Self {
            access_token: payload
                .get("access_token")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            refresh_token: payload
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            expires_at,
            scope: payload
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            token_type: payload
                .get("token_type")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        self.expires_at <= now
    }

    /// Check if the token needs refresh (expires within threshold)
    pub fn needs_refresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        self.expires_at - now < REFRESH_THRESHOLD_SECONDS
    }
}

/// Device authorization response
#[derive(Debug, Clone)]
pub struct DeviceAuthorization {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: Option<i64>,
    pub interval: u64,
}

/// OAuth event for UI updates
#[derive(Debug, Clone)]
pub enum OAuthEvent {
    /// Informational message
    Info { message: String },
    /// Error message
    Error { message: String },
    /// Waiting for user action
    Waiting { message: String },
    /// Verification URL available
    VerificationUrl { url: String, user_code: String },
    /// Success message
    Success { message: String },
}

impl OAuthEvent {
    /// Create an info event
    pub fn info(message: impl Into<String>) -> Self {
        Self::Info {
            message: message.into(),
        }
    }

    /// Create an error event
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    /// Create a waiting event
    pub fn waiting(message: impl Into<String>) -> Self {
        Self::Waiting {
            message: message.into(),
        }
    }

    /// Create a verification URL event
    pub fn verification_url(url: impl Into<String>, user_code: impl Into<String>) -> Self {
        Self::VerificationUrl {
            url: url.into(),
            user_code: user_code.into(),
        }
    }

    /// Create a success event
    pub fn success(message: impl Into<String>) -> Self {
        Self::Success {
            message: message.into(),
        }
    }

    /// Get the message from the event
    pub fn message(&self) -> &str {
        match self {
            Self::Info { message } => message,
            Self::Error { message } => message,
            Self::Waiting { message } => message,
            Self::VerificationUrl { url, .. } => url,
            Self::Success { message } => message,
        }
    }
}

/// OAuth error types
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("OAuth flow error: {0}")]
    General(String),
    #[error("OAuth credentials rejected")]
    Unauthorized,
    #[error("Device authorization expired")]
    DeviceExpired,
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Get the OAuth host URL from environment or default
fn oauth_host() -> String {
    env::var("KIMI_CODE_OAUTH_HOST")
        .or_else(|_| env::var("KIMI_OAUTH_HOST"))
        .unwrap_or_else(|_| crate::auth::DEFAULT_OAUTH_HOST.to_string())
}

/// Build common headers for OAuth requests
fn common_headers() -> HashMap<String, String> {
    use std::ffi::OsStr;

    let device_name = hostname::get()
        .unwrap_or_else(|_| OsStr::new("unknown").to_os_string())
        .to_string_lossy()
        .to_string();

    let device_model = device_model();
    let os_version = sysinfo::System::kernel_version().unwrap_or_default();
    let version = env!("CARGO_PKG_VERSION");

    let mut headers = HashMap::new();
    headers.insert("X-Msh-Platform".to_string(), "kimi_cli".to_string());
    headers.insert("X-Msh-Version".to_string(), version.to_string());
    headers.insert("X-Msh-Device-Name".to_string(), ascii_header_value(&device_name));
    headers.insert("X-Msh-Device-Model".to_string(), ascii_header_value(&device_model));
    headers.insert("X-Msh-Os-Version".to_string(), ascii_header_value(&os_version));
    headers.insert("X-Msh-Device-Id".to_string(), get_device_id());
    headers
}

/// Sanitize a header value to ASCII
fn ascii_header_value(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .filter(|c| c.is_ascii())
        .collect();
    if sanitized.trim().is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

/// Get device model information
fn device_model() -> String {
    #[cfg(target_os = "macos")]
    {
        let arch = std::env::consts::ARCH;
        format!("macOS {} {}", sysinfo::System::kernel_version().unwrap_or_default(), arch)
    }
    #[cfg(target_os = "windows")]
    {
        let arch = std::env::consts::ARCH;
        format!("Windows {} {}", sysinfo::System::kernel_version().unwrap_or_default(), arch)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let arch = std::env::consts::ARCH;
        format!(
            "{} {} {}",
            std::env::consts::OS,
            sysinfo::System::kernel_version().unwrap_or_default(),
            arch
        )
    }
}

/// Request device authorization from OAuth server
pub async fn request_device_authorization() -> Result<DeviceAuthorization, OAuthError> {
    let client = Client::new();
    let url = format!("{}/api/oauth/device_authorization", oauth_host().trim_end_matches('/'));

    let headers = common_headers();

    let response = client
        .post(&url)
        .form(&[("client_id", KIMI_CODE_CLIENT_ID)])
        .header("X-Msh-Platform", headers.get("X-Msh-Platform").unwrap())
        .header("X-Msh-Version", headers.get("X-Msh-Version").unwrap())
        .header("X-Msh-Device-Name", headers.get("X-Msh-Device-Name").unwrap())
        .header("X-Msh-Device-Model", headers.get("X-Msh-Device-Model").unwrap())
        .header("X-Msh-Os-Version", headers.get("X-Msh-Os-Version").unwrap())
        .header("X-Msh-Device-Id", headers.get("X-Msh-Device-Id").unwrap())
        .send()
        .await?;

    let status = response.status();
    let data: serde_json::Value = response.json().await?;

    if status.as_u16() != 200 {
        return Err(OAuthError::General(format!(
            "Device authorization failed: {}",
            data
        )));
    }

    Ok(DeviceAuthorization {
        user_code: data
            .get("user_code")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        device_code: data
            .get("device_code")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        verification_uri: data
            .get("verification_uri")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        verification_uri_complete: data
            .get("verification_uri_complete")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        expires_in: data.get("expires_in").and_then(|v| v.as_i64()),
        interval: data
            .get("interval")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .max(1),
    })
}

/// Internal function to request device token
async fn request_device_token(
    auth: &DeviceAuthorization,
) -> Result<(u16, serde_json::Value), OAuthError> {
    let client = Client::new();
    let url = format!("{}/api/oauth/token", oauth_host().trim_end_matches('/'));

    let headers = common_headers();

    let response = client
        .post(&url)
        .form(&[
            ("client_id", KIMI_CODE_CLIENT_ID),
            ("device_code", &auth.device_code),
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code",
            ),
        ])
        .header("X-Msh-Platform", headers.get("X-Msh-Platform").unwrap())
        .header("X-Msh-Version", headers.get("X-Msh-Version").unwrap())
        .header("X-Msh-Device-Name", headers.get("X-Msh-Device-Name").unwrap())
        .header("X-Msh-Device-Model", headers.get("X-Msh-Device-Model").unwrap())
        .header("X-Msh-Os-Version", headers.get("X-Msh-Os-Version").unwrap())
        .header("X-Msh-Device-Id", headers.get("X-Msh-Device-Id").unwrap())
        .send()
        .await?;

    let status = response.status().as_u16();
    let data: serde_json::Value = response.json().await?;

    if status >= 500 {
        return Err(OAuthError::General(format!(
            "Token polling server error: {}",
            status
        )));
    }

    Ok((status, data))
}

/// Poll for device token
pub async fn poll_device_token(auth: &DeviceAuthorization) -> Result<OAuthToken, OAuthError> {
    let interval = Duration::from_secs(auth.interval);
    let mut printed_wait = false;

    loop {
        let (status, data) = request_device_token(auth).await?;

        if status == 200 && data.get("access_token").is_some() {
            return OAuthToken::from_response(data);
        }

        let error_code = data
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_error");

        if error_code == "expired_token" {
            return Err(OAuthError::DeviceExpired);
        }

        let error_description = data
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if !printed_wait {
            debug!(
                "Waiting for authorization... error: {}, description: {}",
                error_code, error_description
            );
            printed_wait = true;
        }

        sleep(interval).await;
    }
}

/// Refresh OAuth token
pub async fn refresh_token(refresh_token_str: &str) -> Result<OAuthToken, OAuthError> {
    let client = Client::new();
    let url = format!("{}/api/oauth/token", oauth_host().trim_end_matches('/'));

    let headers = common_headers();

    let response = client
        .post(&url)
        .form(&[
            ("client_id", KIMI_CODE_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token_str),
        ])
        .header("X-Msh-Platform", headers.get("X-Msh-Platform").unwrap())
        .header("X-Msh-Version", headers.get("X-Msh-Version").unwrap())
        .header("X-Msh-Device-Name", headers.get("X-Msh-Device-Name").unwrap())
        .header("X-Msh-Device-Model", headers.get("X-Msh-Device-Model").unwrap())
        .header("X-Msh-Os-Version", headers.get("X-Msh-Os-Version").unwrap())
        .header("X-Msh-Device-Id", headers.get("X-Msh-Device-Id").unwrap())
        .send()
        .await?;

    let status = response.status();
    let data: serde_json::Value = response.json().await?;

    let status_code = status.as_u16();
    if status_code == 401 || status_code == 403 {
        return Err(OAuthError::Unauthorized);
    }

    if status_code != 200 {
        let error_desc = data
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("Token refresh failed");
        return Err(OAuthError::General(error_desc.to_string()));
    }

    OAuthToken::from_response(data)
}

/// Select default model and thinking mode from list of models
fn select_default_model_and_thinking(
    models: &[crate::auth::ModelInfo],
) -> Option<(&crate::auth::ModelInfo, bool)> {
    let selected_model = models.first()?;
    let capabilities = selected_model.capabilities();
    let thinking = capabilities.contains(&ModelCapability::Thinking)
        || capabilities.contains(&ModelCapability::AlwaysThinking);
    Some((selected_model, thinking))
}

/// Apply Kimi Code configuration after login
fn apply_kimi_code_config(
    config: &mut Config,
    models: Vec<crate::auth::ModelInfo>,
    selected_model: &crate::auth::ModelInfo,
    _thinking: bool,
    oauth_ref: OAuthRef,
) -> Result<(), OAuthError> {
    use crate::auth::platforms::{get_platform_by_id, managed_model_key, managed_provider_key};
    use secrecy::SecretString;

    let platform = get_platform_by_id(KIMI_CODE_PLATFORM_ID)
        .ok_or_else(|| OAuthError::General("Kimi Code platform not found".to_string()))?;

    let provider_key = managed_provider_key(&platform.id);

    // Add or update provider
    let provider = crate::config::LlmProvider {
        provider_type: crate::config::ProviderType::Kimi,
        base_url: platform.base_url.clone(),
        api_key: SecretString::new(String::new()),
        env: None,
        custom_headers: None,
        oauth: Some(oauth_ref.clone()),
    };
    config.providers.insert(provider_key.clone(), provider);

    // Remove existing models for this provider
    config.models.retain(|_, model| model.provider != provider_key);

    // Add new models
    for model_info in &models {
        let capabilities = model_info.capabilities();
        let _caps_vec: Vec<String> = capabilities
            .iter()
            .map(|c| format!("{:?}", c).to_lowercase())
            .collect();

        let model = LlmModel {
            name: model_info.id.clone(),  // Use the actual model ID, not the managed key
            provider: provider_key.clone(),
            max_tokens: Some(model_info.context_length),
            temperature: None,
        };
        config
            .models
            .insert(managed_model_key(&platform.id, &model_info.id), model);
    }

    // Set default model
    config.default_model = managed_model_key(&platform.id, &selected_model.id);

    // Update services
    if platform.search_url.is_some() {
        config.services = Services {
            enabled: vec!["moonshot_search".to_string()],
            config: HashMap::new(),
        };
    }

    Ok(())
}

/// Login to Kimi Code platform
pub async fn login_kimi_code(
    config: &mut Config,
    open_browser: bool,
) -> Result<Vec<OAuthEvent>, OAuthError> {
    use crate::auth::platforms::{get_platform_by_id, list_models};
    use std::process::Command;

    let mut events = Vec::new();

    // Check if config is from default location
    // Note: This would need to be tracked in the Config struct
    // For now, we assume it's valid

    let platform = match get_platform_by_id(KIMI_CODE_PLATFORM_ID) {
        Some(p) => p,
        None => {
            events.push(OAuthEvent::error("Kimi Code platform is unavailable"));
            return Ok(events);
        }
    };

    let mut token: Option<OAuthToken> = None;

    // Loop to handle device code expiration and restart
    loop {
        let auth = match request_device_authorization().await {
            Ok(a) => a,
            Err(e) => {
                events.push(OAuthEvent::error(format!("Login failed: {}", e)));
                return Ok(events);
            }
        };

        events.push(OAuthEvent::info(
            "Please visit the following URL to finish authorization.",
        ));
        events.push(OAuthEvent::verification_url(
            &auth.verification_uri_complete,
            &auth.user_code,
        ));

        // Open browser if requested
        if open_browser {
            let url = auth.verification_uri_complete.clone();
            let result = if cfg!(target_os = "macos") {
                Command::new("open").arg(&url).output()
            } else if cfg!(target_os = "windows") {
                Command::new("cmd").args(["/C", "start", &url]).output()
            } else {
                Command::new("xdg-open").arg(&url).output()
            };

            if let Err(e) = result {
                warn!("Failed to open browser: {}", e);
            }
        }

        // Poll for token
        let interval = Duration::from_secs(auth.interval.max(1));
        let mut printed_wait = false;

        loop {
            match request_device_token(&auth).await {
                Ok((status, data)) => {
                    if status == 200 && data.get("access_token").is_some() {
                        match OAuthToken::from_response(data) {
                            Ok(t) => {
                                token = Some(t);
                                break;
                            }
                            Err(e) => {
                                events.push(OAuthEvent::error(format!("Login failed: {}", e)));
                                return Ok(events);
                            }
                        }
                    }

                    let error_code = data
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown_error");

                    if error_code == "expired_token" {
                        events.push(OAuthEvent::info("Device code expired, restarting login..."));
                        break; // Break inner loop to restart outer loop
                    }

                    let error_description = data
                        .get("error_description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !printed_wait {
                        events.push(OAuthEvent::waiting(format!(
                            "Waiting for user authorization...: {}",
                            error_description.trim()
                        )));
                        printed_wait = true;
                    }

                    sleep(interval).await;
                }
                Err(e) => {
                    events.push(OAuthEvent::error(format!("Login failed: {}", e)));
                    return Ok(events);
                }
            }
        }

        if token.is_some() {
            break;
        }
        // Otherwise, loop continues to restart the device authorization
    }

    let token = token.unwrap();

    // Save token
    let oauth_ref = OAuthRef {
        storage: "file".to_string(),
        key: KIMI_CODE_OAUTH_KEY.to_string(),
    };
    let oauth_ref = save_token(&oauth_ref, &token);

    // Fetch models
    let models = match list_models(&platform, &token.access_token).await {
        Ok(m) => m,
        Err(e) => {
            events.push(OAuthEvent::error(format!("Failed to get models: {}", e)));
            return Ok(events);
        }
    };

    if models.is_empty() {
        events.push(OAuthEvent::error("No models available for the selected platform"));
        return Ok(events);
    }

    let (selected_model, thinking) = match select_default_model_and_thinking(&models) {
        Some((m, t)) => (m.clone(), t),
        None => {
            events.push(OAuthEvent::error("Failed to select default model"));
            return Ok(events);
        }
    };

    // Apply configuration
    if let Err(e) = apply_kimi_code_config(
        config,
        models,
        &selected_model,
        thinking,
        oauth_ref.clone(),
    ) {
        events.push(OAuthEvent::error(format!("Failed to apply config: {}", e)));
        return Ok(events);
    }

    events.push(OAuthEvent::success("Logged in successfully."));
    Ok(events)
}

/// Logout from Kimi Code platform
pub async fn logout_kimi_code(config: &mut Config) -> Result<Vec<OAuthEvent>, OAuthError> {
    use crate::auth::platforms::managed_provider_key;

    let mut events = Vec::new();

    // Delete tokens from both keyring and file
    let keyring_ref = OAuthRef {
        storage: "keyring".to_string(),
        key: KIMI_CODE_OAUTH_KEY.to_string(),
    };
    let file_ref = OAuthRef {
        storage: "file".to_string(),
        key: KIMI_CODE_OAUTH_KEY.to_string(),
    };
    delete_token(&keyring_ref);
    delete_token(&file_ref);

    // Remove provider
    let provider_key = managed_provider_key(KIMI_CODE_PLATFORM_ID);
    config.providers.remove(&provider_key);

    // Remove models and track if default was removed
    let mut removed_default = false;
    let models_to_remove: Vec<String> = config
        .models
        .iter()
        .filter(|(_, m)| m.provider == provider_key)
        .map(|(k, _)| k.clone())
        .collect();

    for key in models_to_remove {
        if config.default_model == key {
            removed_default = true;
        }
        config.models.remove(&key);
    }

    if removed_default {
        config.default_model = String::new();
    }

    // Clear services
    config.services = Services {
        enabled: Vec::new(),
        config: HashMap::new(),
    };

    events.push(OAuthEvent::success("Logged out successfully."));
    Ok(events)
}
