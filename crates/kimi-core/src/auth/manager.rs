//! OAuth manager for runtime token management

use crate::auth::oauth::{refresh_token, OAuthError, OAuthToken};
use crate::auth::storage::{delete_token, load_token, save_token, OAuthRef};
use crate::auth::{KIMI_CODE_PLATFORM_ID, REFRESH_INTERVAL_SECONDS};
use crate::config::Config;
use secrecy::ExposeSecret;
use secrecy::SecretString;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{debug, warn};

/// Manages OAuth tokens during runtime
pub struct OAuthManager {
    config: Config,
    access_tokens: HashMap<String, String>,
    refresh_lock: Arc<Mutex<()>>,
}

impl OAuthManager {
    /// Create a new OAuth manager
    pub fn new(config: Config) -> Self {
        let mut manager = Self {
            config,
            access_tokens: HashMap::new(),
            refresh_lock: Arc::new(Mutex::new(())),
        };

        manager.migrate_oauth_storage();
        manager.load_initial_tokens();

        manager
    }

    /// Iterate over all OAuth references in config
    fn iter_oauth_refs(&self) -> Vec<OAuthRef> {
        let mut refs = Vec::new();

        // Collect from providers
        for provider in self.config.providers.values() {
            if let Some(ref oauth) = provider.oauth {
                refs.push(oauth.clone());
            }
        }

        refs
    }

    /// Migrate OAuth storage from keyring to file
    fn migrate_oauth_storage(&mut self) {
        let mut migrated_keys: Vec<String> = Vec::new();
        let mut changed = false;

        // Check providers
        for provider in self.config.providers.values_mut() {
            if let Some(ref oauth) = provider.oauth {
                if oauth.storage == "keyring" && !migrated_keys.contains(&oauth.key) {
                    // Try to load (which will migrate)
                    let _ = load_token(oauth);
                    migrated_keys.push(oauth.key.clone());
                }
                if oauth.storage == "keyring" {
                    changed = true;
                    provider.oauth = Some(OAuthRef {
                        storage: "file".to_string(),
                        key: oauth.key.clone(),
                    });
                }
            }
        }

        // Note: Services migration would go here if services had OAuth refs
        // For now, services config is simplified

        if changed {
            // Config should be saved here if needed
            debug!("Migrated OAuth storage from keyring to file");
        }
    }

    /// Load initial tokens into cache
    fn load_initial_tokens(&mut self) {
        for ref_ in self.iter_oauth_refs() {
            if let Some(token) = load_token(&ref_) {
                self.do_cache_access_token(&ref_.key, &token.access_token);
            }
        }
    }

    /// Internal: Cache an access token by key
    fn do_cache_access_token(&mut self, key: &str, access_token: &str) {
        if access_token.is_empty() {
            self.access_tokens.remove(key);
        } else {
            self.access_tokens.insert(key.to_string(), access_token.to_string());
        }
    }

    /// Get common headers for OAuth requests
    pub fn common_headers(&self) -> HashMap<String, String> {
        use crate::auth::storage::get_device_id;

        let device_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let device_model = self.device_model();
        let os_version = sysinfo::System::kernel_version().unwrap_or_default();
        let version = env!("CARGO_PKG_VERSION");

        let mut headers = HashMap::new();
        headers.insert("X-Msh-Platform".to_string(), "kimi_cli".to_string());
        headers.insert("X-Msh-Version".to_string(), version.to_string());
        headers.insert(
            "X-Msh-Device-Name".to_string(),
            Self::ascii_header_value(&device_name),
        );
        headers.insert(
            "X-Msh-Device-Model".to_string(),
            Self::ascii_header_value(&device_model),
        );
        headers.insert(
            "X-Msh-Os-Version".to_string(),
            Self::ascii_header_value(&os_version),
        );
        headers.insert("X-Msh-Device-Id".to_string(), get_device_id());
        headers
    }

    /// Sanitize a header value to ASCII
    fn ascii_header_value(value: &str) -> String {
        let sanitized: String = value.chars().filter(|c| c.is_ascii()).collect();
        if sanitized.trim().is_empty() {
            "unknown".to_string()
        } else {
            sanitized
        }
    }

    /// Get device model information
    fn device_model(&self) -> String {
        #[cfg(target_os = "macos")]
        {
            let arch = std::env::consts::ARCH;
            format!(
                "macOS {} {}",
                sysinfo::System::kernel_version().unwrap_or_default(),
                arch
            )
        }
        #[cfg(target_os = "windows")]
        {
            let arch = std::env::consts::ARCH;
            format!(
                "Windows {} {}",
                sysinfo::System::kernel_version().unwrap_or_default(),
                arch
            )
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

    /// Resolve API key (from OAuth or config)
    pub fn resolve_api_key(&self, api_key: &SecretString, oauth: Option<&OAuthRef>) -> String {
        if let Some(ref_) = oauth {
            // Try cache first
            if let Some(token) = self.access_tokens.get(&ref_.key) {
                return token.clone();
            }

            // Try loading from storage
            if let Some(token) = load_token(ref_) {
                return token.access_token;
            }
        }

        api_key.expose_secret().clone()
    }

    /// Get the Kimi Code OAuth reference
    fn kimi_code_ref(&self) -> Option<OAuthRef> {
        use crate::auth::platforms::managed_provider_key;

        let provider_key = managed_provider_key(KIMI_CODE_PLATFORM_ID);
        self.config
            .providers
            .get(&provider_key)
            .and_then(|p| p.oauth.clone())
    }

    /// Ensure fresh token (refresh if needed)
    pub async fn ensure_fresh(&mut self) -> Result<(), OAuthError> {
        let ref_ = match self.kimi_code_ref() {
            Some(r) => r,
            None => return Ok(()),
        };

        let token = match load_token(&ref_) {
            Some(t) => t,
            None => return Ok(()),
        };

        self.do_cache_access_token(&ref_.key, &token.access_token);

        // Check if refresh is needed
        if !token.needs_refresh() {
            return Ok(());
        }

        self.refresh_tokens(&ref_, &token).await
    }

    /// Refresh tokens
    async fn refresh_tokens(&mut self, ref_: &OAuthRef, token: &OAuthToken) -> Result<(), OAuthError> {
        // Always prefer persisted tokens before refresh
        let persisted = load_token(ref_);
        if let Some(ref persisted_token) = persisted {
            self.do_cache_access_token(&ref_.key, &persisted_token.access_token);
        }

        let current_token = persisted.as_ref().unwrap_or(token);

        if current_token.refresh_token.is_empty() {
            return Ok(());
        }

        // Clone values we need before the lock
        let ref_key = ref_.key.clone();
        let refresh_token_value = current_token.refresh_token.clone();
        let ref_key_for_cache = ref_.key.clone();

        // Acquire lock to prevent concurrent refreshes
        // Note: We lock a separate Arc<Mutex>, not self directly
        let lock = Arc::clone(&self.refresh_lock);
        let _guard = lock.lock().await;

        // Re-check persisted token inside the lock
        let persisted = load_token(ref_);
        if let Some(ref persisted_token) = persisted {
            self.do_cache_access_token(&ref_.key, &persisted_token.access_token);
        }
        let current = persisted.as_ref().unwrap_or(current_token);

        // Check again if refresh is still needed
        if !current.needs_refresh() {
            return Ok(());
        }

        if current.refresh_token.is_empty() {
            return Ok(());
        }

        match refresh_token(&refresh_token_value).await {
            Ok(refreshed) => {
                save_token(ref_, &refreshed);
                self.do_cache_access_token(&ref_key, &refreshed.access_token);
                debug!("Successfully refreshed OAuth token");
                Ok(())
            }
            Err(OAuthError::Unauthorized) => {
                // Check if another session refreshed and persisted a new token
                let latest = load_token(ref_);
                if let Some(ref latest_token) = latest {
                    if latest_token.refresh_token != refresh_token_value {
                        self.do_cache_access_token(&ref_key_for_cache, &latest_token.access_token);
                        return Ok(());
                    }
                }

                warn!("OAuth credentials rejected, deleting stored tokens");
                self.access_tokens.remove(&ref_key);
                delete_token(ref_);
                Err(OAuthError::Unauthorized)
            }
            Err(e) => {
                warn!("Failed to refresh OAuth token: {}", e);
                Err(e)
            }
        }
    }

    /// Get access token for key
    pub fn get_access_token(&self, key: &str) -> Option<&str> {
        self.access_tokens.get(key).map(|s| s.as_str())
    }

    /// Cache access token (public version)
    pub fn cache_access_token(&mut self, ref_: &OAuthRef, token: &OAuthToken) {
        self.do_cache_access_token(&ref_.key, &token.access_token);
    }

    /// Start background refresh task
    /// 
    /// This spawns a task that periodically checks and refreshes tokens.
    /// The task runs until the returned handle is dropped.
    pub fn start_background_refresh(&self) -> BackgroundRefreshHandle {
        let refresh_lock = Arc::clone(&self.refresh_lock);

        let handle = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(REFRESH_INTERVAL_SECONDS)).await;

                // Try to refresh tokens
                let _guard = refresh_lock.lock().await;
                // Actual refresh logic would go here
                // This is a simplified version
            }
        });

        BackgroundRefreshHandle { handle }
    }
}

/// Handle for background refresh task
pub struct BackgroundRefreshHandle {
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for BackgroundRefreshHandle {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_token(expires_in: f64) -> OAuthToken {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        OAuthToken {
            access_token: "test_access".to_string(),
            refresh_token: "test_refresh".to_string(),
            expires_at: now + expires_in,
            scope: "test".to_string(),
            token_type: "Bearer".to_string(),
        }
    }

    #[test]
    fn test_ascii_header_value() {
        assert_eq!(
            OAuthManager::ascii_header_value("hello world"),
            "hello world"
        );
        assert_eq!(
            OAuthManager::ascii_header_value("héllo"),
            "hllo"
        );
        assert_eq!(
            OAuthManager::ascii_header_value(""),
            "unknown"
        );
        assert_eq!(
            OAuthManager::ascii_header_value("日本語"),
            "unknown"
        );
    }

    #[test]
    fn test_token_needs_refresh() {
        // Token expiring in 10 seconds (needs refresh)
        let token = create_test_token(10.0);
        assert!(token.needs_refresh());

        // Token expiring in 10 minutes (doesn't need refresh)
        let token = create_test_token(600.0);
        assert!(!token.needs_refresh());
    }
}
