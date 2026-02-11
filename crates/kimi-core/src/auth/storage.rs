//! Token storage implementation

use crate::auth::oauth::{OAuthError, OAuthToken};
// Note: KEYRING_SERVICE is defined in crate::auth but not used when keyring feature is disabled
// use crate::auth::KEYRING_SERVICE;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// OAuth reference in config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthRef {
    pub storage: String, // "file" or "keyring"
    pub key: String,
}

/// Get the share directory for the application
fn get_share_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kimi")
}

/// Get credentials directory
pub fn credentials_dir() -> PathBuf {
    let path = get_share_dir().join("credentials");
    fs::create_dir_all(&path).ok();
    path
}

/// Get credentials file path for a key
fn credentials_path(key: &str) -> PathBuf {
    let name = key
        .strip_prefix("oauth/")
        .and_then(|s| s.split('/').next_back())
        .unwrap_or(key);
    credentials_dir().join(format!("{}.json", name))
}

/// Ensure file has private permissions (0o600)
fn ensure_private_file(path: &PathBuf) -> Result<(), OAuthError> {
    #[cfg(unix)]
    {
        let metadata = fs::metadata(path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

/// Get device ID file path
fn device_id_path() -> PathBuf {
    get_share_dir().join("device_id")
}

/// Get device ID (generate if not exists)
pub fn get_device_id() -> String {
    let path = device_id_path();
    if path.exists() {
        fs::read_to_string(&path).unwrap_or_default().trim().to_string()
    } else {
        let device_id = uuid::Uuid::new_v4().to_string().replace("-", "");
        if let Err(e) = fs::write(&path, &device_id) {
            tracing::warn!("Failed to write device ID: {}", e);
        } else {
            ensure_private_file(&path).ok();
        }
        device_id
    }
}

// Load token from keyring (fallback for migration)
// Note: Keyring support is disabled by default.
// To enable, add `keyring` feature to Cargo.toml and uncomment the following:
//
// #[cfg(feature = "keyring")]
// fn load_from_keyring(key: &str) -> Option<OAuthToken> {
//     use keyring::Entry;
//     ...
// }

/// Load token from keyring (placeholder - keyring support disabled)
fn load_from_keyring(_key: &str) -> Option<OAuthToken> {
    None
}

/// Delete token from keyring (placeholder - keyring support disabled)
fn delete_from_keyring(_key: &str) {}

/// Load token from file
fn load_from_file(key: &str) -> Option<OAuthToken> {
    let path = credentials_path(key);
    if !path.exists() {
        return None;
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Failed to read token file: {}", e);
            return None;
        }
    };

    let payload: serde_json::Value = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!("Failed to parse token file: {}", e);
            return None;
        }
    };

    if !payload.is_object() {
        return None;
    }

    match OAuthToken::from_dict(payload) {
        Ok(token) => Some(token),
        Err(e) => {
            tracing::debug!("Failed to create token from file data: {}", e);
            None
        }
    }
}

/// Save token to file
fn save_to_file(key: &str, token: &OAuthToken) -> Result<(), OAuthError> {
    let path = credentials_path(key);
    let content = serde_json::to_string_pretty(&token.to_dict())?;
    fs::write(&path, content)?;
    ensure_private_file(&path)?;
    Ok(())
}

/// Delete token from file
fn delete_from_file(key: &str) {
    let path = credentials_path(key);
    if path.exists() {
        if let Err(e) = fs::remove_file(&path) {
            tracing::debug!("Failed to delete token file: {}", e);
        }
    }
}

/// Load token from storage
/// 
/// Tries file first, then keyring (for migration purposes).
/// If found in keyring, migrates to file.
pub fn load_token(ref_: &OAuthRef) -> Option<OAuthToken> {
    // Try file first
    let file_token = load_from_file(&ref_.key);
    if file_token.is_some() {
        return file_token;
    }

    // If not in file and keyring storage was requested, try keyring
    if ref_.storage == "keyring" {
        let keyring_token = load_from_keyring(&ref_.key);
        if let Some(ref token) = keyring_token {
            // Migrate from keyring to file
            if let Err(e) = save_to_file(&ref_.key, token) {
                tracing::warn!("Failed to migrate token from keyring to file: {}", e);
            } else {
                // Delete from keyring after successful migration
                delete_from_keyring(&ref_.key);
            }
        }
        return keyring_token;
    }

    None
}

/// Save token to storage
/// 
/// Always saves to file (keyring is deprecated).
pub fn save_token(ref_: &OAuthRef, token: &OAuthToken) -> OAuthRef {
    if ref_.storage == "keyring" {
        tracing::warn!("Keyring storage is deprecated; saving OAuth tokens to file.");
    }

    if let Err(e) = save_to_file(&ref_.key, token) {
        tracing::error!("Failed to save token: {}", e);
    }

    OAuthRef {
        storage: "file".to_string(),
        key: ref_.key.clone(),
    }
}

/// Delete token from storage
pub fn delete_token(ref_: &OAuthRef) {
    if ref_.storage == "keyring" {
        delete_from_keyring(&ref_.key);
    }
    delete_from_file(&ref_.key);
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_oauth_ref_serialization() {
        let ref_ = OAuthRef {
            storage: "file".to_string(),
            key: "oauth/test".to_string(),
        };

        let json = serde_json::to_string(&ref_).unwrap();
        let deserialized: OAuthRef = serde_json::from_str(&json).unwrap();

        assert_eq!(ref_.storage, deserialized.storage);
        assert_eq!(ref_.key, deserialized.key);
    }

    #[test]
    fn test_credentials_path() {
        let path = credentials_path("oauth/kimi-code");
        assert!(path.to_string_lossy().ends_with("kimi-code.json"));

        let path = credentials_path("kimi-code");
        assert!(path.to_string_lossy().ends_with("kimi-code.json"));
    }

    #[test]
    fn test_device_id_generation() {
        // This test might create a device_id file
        let id1 = get_device_id();
        let id2 = get_device_id();

        // Should be consistent
        assert_eq!(id1, id2);

        // Should be a valid UUID (32 hex chars without dashes)
        assert_eq!(id1.len(), 32);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
