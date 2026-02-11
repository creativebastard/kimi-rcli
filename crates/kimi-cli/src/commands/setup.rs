//! Setup command for manual provider configuration
//!
//! This module provides an interactive setup wizard for configuring
//! LLM providers with API keys, similar to the original kimi-cli setup.

use anyhow::Result;
use kimi_core::auth::platforms::{list_models, Platform};
use kimi_core::auth::KIMI_CODE_PLATFORM_ID;
use kimi_core::config::{load_config, save_config, LlmProvider, ProviderType};
use kimi_core::types::LlmModel;
use secrecy::SecretString;
use std::io::{self, Write};
use tracing::info;

/// Provider choice for setup
#[derive(Debug, Clone)]
pub enum ProviderChoice {
    /// Kimi Code OAuth (device flow)
    KimiCode,
    /// Moonshot AI China (api.moonshot.cn)
    MoonshotCN,
    /// Moonshot AI Overseas (api.moonshot.ai)
    MoonshotAI,
}

impl ProviderChoice {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            ProviderChoice::KimiCode => "Kimi Code (OAuth)",
            ProviderChoice::MoonshotCN => "Moonshot AI (China) - api.moonshot.cn",
            ProviderChoice::MoonshotAI => "Moonshot AI (Overseas) - api.moonshot.ai",
        }
    }

    /// Get platform ID
    pub fn platform_id(&self) -> &'static str {
        match self {
            ProviderChoice::KimiCode => KIMI_CODE_PLATFORM_ID,
            ProviderChoice::MoonshotCN => "moonshot-cn",
            ProviderChoice::MoonshotAI => "moonshot-ai",
        }
    }

    /// Whether this provider requires OAuth
    pub fn requires_oauth(&self) -> bool {
        matches!(self, ProviderChoice::KimiCode)
    }

    /// Get base URL
    pub fn base_url(&self) -> String {
        match self {
            ProviderChoice::KimiCode => {
                std::env::var("KIMI_CODE_BASE_URL")
                    .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".to_string())
            }
            ProviderChoice::MoonshotCN => "https://api.moonshot.cn/v1".to_string(),
            ProviderChoice::MoonshotAI => "https://api.moonshot.ai/v1".to_string(),
        }
    }
}

/// Execute the interactive setup wizard
pub async fn execute() -> Result<()> {
    info!("Starting setup wizard");

    println!("\n{}", "═".repeat(60));
    println!("  Welcome to Kimi CLI Setup");
    println!("{}", "═".repeat(60));
    println!();

    // Load existing config
    let mut config = load_config(None)?;

    // Step 1: Choose provider
    let provider_choice = select_provider()?;
    println!();

    // Step 2: Configure based on provider type
    if provider_choice.requires_oauth() {
        // OAuth flow - delegate to login command
        println!("Starting OAuth authentication for {}...", provider_choice.name());
        crate::commands::login::execute(true).await?;
    } else {
        // API key flow
        setup_api_key_provider(&mut config, &provider_choice).await?;
    }

    println!();
    println!("{}", "═".repeat(60));
    println!("  Setup complete!");
    println!("{}", "═".repeat(60));
    println!();
    println!("You can now start chatting with: kimi-cli");
    println!();

    Ok(())
}

/// Select provider interactively
fn select_provider() -> Result<ProviderChoice> {
    let choices = vec![
        ProviderChoice::KimiCode,
        ProviderChoice::MoonshotCN,
        ProviderChoice::MoonshotAI,
    ];

    println!("Select a provider:");
    println!();

    for (i, choice) in choices.iter().enumerate() {
        println!("  {}. {}", i + 1, choice.name());
    }
    println!();

    loop {
        print!("Enter your choice (1-{}): ", choices.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= choices.len() => {
                return Ok(choices[n - 1].clone());
            }
            _ => {
                println!("Invalid choice. Please enter a number between 1 and {}.", choices.len());
            }
        }
    }
}

/// Setup provider with API key
async fn setup_api_key_provider(
    config: &mut kimi_core::config::Config,
    choice: &ProviderChoice,
) -> Result<()> {
    let platform_id = choice.platform_id();
    let base_url = choice.base_url();

    println!("Setting up {}...", choice.name());
    println!();

    // Step 1: Get API key
    let api_key = prompt_api_key()?;
    if api_key.is_empty() {
        return Err(anyhow::anyhow!("API key is required"));
    }
    println!();

    // Step 2: Test connection and fetch models
    println!("Testing connection and fetching available models...");
    let platform = Platform {
        id: platform_id.to_string(),
        name: choice.name().to_string(),
        base_url: base_url.clone(),
        search_url: None,
        fetch_url: None,
        allowed_prefixes: Some(vec!["kimi-k".to_string()]),
    };

    let models = match list_models(&platform, &api_key).await {
        Ok(models) => models,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to connect to API or fetch models: {}. Please check your API key and try again.",
                e
            ));
        }
    };

    if models.is_empty() {
        return Err(anyhow::anyhow!("No models available for this API key"));
    }

    println!("  Found {} models", models.len());
    println!();

    // Step 3: Select default model
    let selected_model = select_model(&models)?;
    println!();

    // Step 4: Configure provider and models
    let provider_key = format!("user:{}", platform_id);

    // Remove existing models for this provider
    config.models.retain(|_, model| model.provider != provider_key);

    // Add provider
    config.providers.insert(
        provider_key.clone(),
        LlmProvider {
            provider_type: ProviderType::Kimi,
            base_url,
            api_key: SecretString::new(api_key),
            env: None,
            custom_headers: None,
            oauth: None,
        },
    );

    // Add models
    for model_info in &models {
        let model_key = format!("{}/{}", platform_id, model_info.id);
        let model = LlmModel {
            name: model_info.id.clone(),
            provider: provider_key.clone(),
            max_tokens: Some(model_info.context_length),
            temperature: None,
        };
        config.models.insert(model_key, model);
    }

    // Set default model
    let default_model_key = format!("{}/{}", platform_id, selected_model.id);
    config.default_model = default_model_key;

    // Save config
    save_config(config, None)?;

    println!("Configuration saved successfully!");
    println!();
    println!("  Provider: {}", choice.name());
    println!("  Default model: {}", selected_model.id);
    println!("  Available models: {}", models.len());

    Ok(())
}

/// Prompt for API key
fn prompt_api_key() -> Result<String> {
    println!("Please enter your API key.");
    println!("You can get one from the Moonshot AI Open Platform:");
    println!("  - China: https://platform.moonshot.cn");
    println!("  - Overseas: https://platform.moonshot.ai");
    println!();

    loop {
        print!("API Key: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let api_key = input.trim().to_string();

        if api_key.is_empty() {
            println!("API key cannot be empty. Please try again.");
            continue;
        }

        // Basic validation - Moonshot API keys typically start with specific prefixes
        if !api_key.starts_with("sk-") {
            println!();
            println!("Warning: The API key doesn't start with 'sk-' which is unusual for Moonshot AI keys.");
            println!("Do you want to continue anyway? (yes/no)");
            print!("> ");
            io::stdout().flush()?;

            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;

            if confirm.trim().to_lowercase() != "yes" {
                continue;
            }
        }

        return Ok(api_key);
    }
}

/// Select model interactively
fn select_model(models: &[kimi_core::auth::platforms::ModelInfo]) -> Result<&kimi_core::auth::platforms::ModelInfo> {
    println!("Available models:");
    println!();

    // Sort models by context length (descending)
    let mut sorted_models: Vec<_> = models.iter().collect();
    sorted_models.sort_by(|a, b| b.context_length.cmp(&a.context_length));

    for (i, model) in sorted_models.iter().enumerate() {
        let capabilities = format_capabilities(model);
        println!(
            "  {}. {} ({} tokens){}",
            i + 1,
            model.id,
            format_number(model.context_length),
            if capabilities.is_empty() {
                String::new()
            } else {
                format!(" - {}", capabilities)
            }
        );
    }
    println!();

    loop {
        print!("Select default model (1-{}): ", sorted_models.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= sorted_models.len() => {
                return Ok(sorted_models[n - 1]);
            }
            _ => {
                println!(
                    "Invalid choice. Please enter a number between 1 and {}.",
                    sorted_models.len()
                );
            }
        }
    }
}

/// Format model capabilities for display
fn format_capabilities(model: &kimi_core::auth::platforms::ModelInfo) -> String {
    let mut caps = Vec::new();

    if model.supports_reasoning {
        caps.push("reasoning");
    }
    if model.supports_image_in {
        caps.push("vision");
    }
    if model.supports_video_in {
        caps.push("video");
    }

    caps.join(", ")
}

/// Format number with commas
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_choice_properties() {
        let kimi = ProviderChoice::KimiCode;
        assert!(kimi.requires_oauth());
        assert_eq!(kimi.platform_id(), "kimi-code");

        let cn = ProviderChoice::MoonshotCN;
        assert!(!cn.requires_oauth());
        assert_eq!(cn.base_url(), "https://api.moonshot.cn/v1");

        let ai = ProviderChoice::MoonshotAI;
        assert!(!ai.requires_oauth());
        assert_eq!(ai.base_url(), "https://api.moonshot.ai/v1");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1000000), "1,000,000");
        assert_eq!(format_number(262144), "262,144");
    }
}
