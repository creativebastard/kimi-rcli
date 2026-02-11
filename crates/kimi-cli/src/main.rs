use std::process;

use anyhow::Result;
use tracing::{error, info};

use clap::Parser;
use kimi_cli::{Cli, Commands};
use kimi_cli::app::App;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        error!("Error: {:#}", e);
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose);

    info!("Starting kimi-cli v{}", env!("CARGO_PKG_VERSION"));

    // Handle subcommands first
    if let Some(command) = cli.command {
        match command {
            Commands::Login => {
                kimi_cli::commands::login::execute(true).await?;
                return Ok(());
            }
            Commands::Setup => {
                kimi_cli::commands::setup::execute().await?;
                return Ok(());
            }
            Commands::Mcp { subcommand } => {
                kimi_cli::commands::mcp::execute(subcommand).await?;
                return Ok(());
            }
        }
    }

    // Create the application
    let app = App::create(&cli).await?;

    // Run based on mode
    if cli.print {
        // Print mode - non-interactive output
        if let Some(ref prompt) = cli.prompt {
            app.run_print(prompt).await?;
        } else {
            // Read from stdin if no prompt provided
            use std::io::{self, Read};
            let mut input = String::new();
            io::stdin().read_to_string(&mut input)?;
            let input = input.trim();
            if input.is_empty() {
                anyhow::bail!("No input provided. Use -p/--prompt or pipe input.");
            }
            app.run_print(input).await?;
        }
    } else if cli.continue_ {
        // Continue existing session
        app.run_continue().await?;
    } else if cli.prompt.is_some() {
        // Single prompt mode
        let prompt = cli.prompt.clone().unwrap();
        app.run_print(&prompt).await?;
    } else {
        // Interactive shell mode (default)
        app.run_shell().await?;
    }

    Ok(())
}

fn init_logging(verbose: bool) {
    let filter = if verbose {
        "debug"
    } else {
        "info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}
