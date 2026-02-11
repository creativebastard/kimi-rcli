//! OAuth authentication module for Kimi CLI
//!
//! This module provides OAuth2 device authorization flow support for
//! authenticating with the Kimi Code platform.

pub mod manager;
pub mod oauth;
pub mod platforms;
pub mod storage;

pub use manager::OAuthManager;
pub use oauth::{
    login_kimi_code, logout_kimi_code, poll_device_token, refresh_token,
    request_device_authorization, DeviceAuthorization, OAuthError, OAuthEvent, OAuthToken,
};
pub use platforms::{
    get_platform_by_id, is_managed_provider_key, list_platforms, list_models,
    managed_model_key, managed_provider_key, parse_managed_provider_key, ModelInfo, Platform,
};
pub use storage::{
    delete_token, get_device_id, load_token, save_token, OAuthRef,
};

/// Platform ID for Kimi Code
pub const KIMI_CODE_PLATFORM_ID: &str = "kimi-code";

/// OAuth client ID for Kimi Code
pub const KIMI_CODE_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";

/// Default OAuth host URL
pub const DEFAULT_OAUTH_HOST: &str = "https://auth.kimi.com";

/// Keyring service name
pub const KEYRING_SERVICE: &str = "kimi-code";

/// OAuth key for Kimi Code
pub const KIMI_CODE_OAUTH_KEY: &str = "oauth/kimi-code";

/// Refresh interval in seconds (background refresh task)
pub const REFRESH_INTERVAL_SECONDS: u64 = 60;

/// Refresh threshold in seconds (refresh if token expires within this time)
pub const REFRESH_THRESHOLD_SECONDS: f64 = 300.0;

/// Model capability derived from model info
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    /// Model supports thinking/reasoning
    Thinking,
    /// Model is always in thinking mode
    AlwaysThinking,
    /// Model supports image input
    ImageIn,
    /// Model supports video input
    VideoIn,
}

use serde::{Deserialize, Serialize};
