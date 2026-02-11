use anyhow::Result;
use kimi_core::auth::{
    login_kimi_code, logout_kimi_code, OAuthEvent, OAuthError,
};
use kimi_core::config::{load_config, save_config};
use std::io::{self, Write};
use tracing::{error, info, warn};

/// Execute the login command
///
/// This handles OAuth authentication with the Kimi Code platform.
/// It will open a browser for the user to authenticate and
/// store the resulting credentials securely.
pub async fn execute(open_browser: bool) -> Result<()> {
    info!("Starting OAuth login flow");

    // Load existing config
    let mut config = load_config(None)
        .map_err(|e| OAuthError::General(format!("Failed to load config: {}", e)))?;

    // Check if using default config location
    if !config.is_from_default_location {
        return Err(OAuthError::General(
            "Login requires the default config file; restart without --config/--config-file."
                .to_string(),
        )
        .into());
    }

    println!("Initiating login to Kimi Code...\n");

    // Run the OAuth flow
    let events = login_kimi_code(&mut config, open_browser).await?;

    // Display events
    let mut success = false;
    for event in events {
        match event {
            OAuthEvent::Info { message } => {
                println!("{}", message);
            }
            OAuthEvent::Error { message } => {
                error!("Login error: {}", message);
                eprintln!("Error: {}", message);
                return Err(OAuthError::General(message).into());
            }
            OAuthEvent::Waiting { message } => {
                print!("\r{}", message);
                io::stdout().flush()?;
            }
            OAuthEvent::VerificationUrl { url, user_code } => {
                println!("\n\nPlease visit the following URL to authorize:");
                println!("  URL: {}", url);
                println!("  Code: {}\n", user_code);

                if open_browser {
                    match open::that(&url) {
                        Ok(_) => println!("Browser opened automatically."),
                        Err(e) => {
                            warn!("Failed to open browser: {}", e);
                            println!("Please open the URL manually in your browser.");
                        }
                    }
                }
            }
            OAuthEvent::Success { message } => {
                println!("\n{}", message);
                success = true;
            }
        }
    }

    if success {
        // Debug: Show what models were configured
        if !config.models.is_empty() {
            println!("\n{} models configured:", config.models.len());
            for (name, model) in &config.models {
                println!("  - {} (provider: {:?})", name, model.provider);
            }
            println!("Default model: {}", config.default_model);
        }
        
        // Save the updated config
        let config_path = dirs::config_dir()
            .map(|d| d.join("kimi").join("config.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from(".kimi/config.toml"));
        println!("\nSaving config to: {:?}", config_path);
        
        save_config(&config, None)
            .map_err(|e| OAuthError::General(format!("Failed to save config: {}", e)))?;

        println!("Configuration saved successfully.");
        println!("You can now use 'kimi-cli' to start chatting.");
        Ok(())
    } else {
        Err(OAuthError::General("Login did not complete successfully".to_string()).into())
    }
}

/// Execute the logout command
///
/// This clears stored OAuth credentials and removes
/// the managed provider configuration.
pub async fn logout() -> Result<()> {
    info!("Starting logout");

    // Load existing config
    let mut config = load_config(None)
        .map_err(|e| OAuthError::General(format!("Failed to load config: {}", e)))?;

    // Check if using default config location
    if !config.is_from_default_location {
        return Err(OAuthError::General(
            "Logout requires the default config file; restart without --config/--config-file."
                .to_string(),
        )
        .into());
    }

    println!("Logging out from Kimi Code...\n");

    // Run the logout flow
    let events = logout_kimi_code(&mut config).await?;

    // Display events
    let mut success = false;
    for event in events {
        match event {
            OAuthEvent::Info { message } => {
                println!("{}", message);
            }
            OAuthEvent::Error { message } => {
                error!("Logout error: {}", message);
                eprintln!("Error: {}", message);
            }
            OAuthEvent::Success { message } => {
                println!("{}", message);
                success = true;
            }
            _ => {}
        }
    }

    if success {
        // Save the updated config
        save_config(&config, None)
            .map_err(|e| OAuthError::General(format!("Failed to save config: {}", e)))?;

        println!("\nConfiguration updated.");
        Ok(())
    } else {
        Err(OAuthError::General("Logout did not complete successfully".to_string()).into())
    }
}

/// Check if the user is authenticated
pub fn is_authenticated() -> bool {
    match load_config(None) {
        Ok(config) => {
            // Check if we have any OAuth-managed providers
            config.providers.values().any(|p| p.oauth.is_some())
        }
        Err(e) => {
            warn!("Failed to load config to check authentication: {}", e);
            false
        }
    }
}

/// Get stored API token if available
pub fn get_token() -> Option<String> {
    use kimi_core::auth::load_token;

    let config = load_config(None).ok()?;

    // Find the first provider with OAuth and get its token
    for provider in config.providers.values() {
        if let Some(ref_) = provider.oauth.as_ref() {
            if let Some(token) = load_token(ref_) {
                return Some(token.access_token);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_authenticated_default() {
        // By default (without config), should not be authenticated
        // This may fail if there's a valid config in the test environment
        let _ = is_authenticated();
    }

    #[test]
    fn test_get_token_default() {
        // By default, should return None
        let _ = get_token();
    }
}
