use std::io::{self, Read};

use nu_ansi_term::{Color, Style};
use tokio::sync::mpsc;
use tracing::{debug, info};

use kimi_core::{
    llm,
    soul::{KimiSoul, WireSoulSide},
    types::UserInput,
    wire::WireMessage,
};

use crate::cli::Cli;
use crate::ui::{UIError, UIResult, UI};

/// Non-interactive print UI for scripts and automation
pub struct PrintUI {
    cli: Cli,
}

impl PrintUI {
    /// Create a new print UI instance
    pub fn new(cli: Cli) -> UIResult<Self> {
        info!("Initializing print UI");
        Ok(Self { cli })
    }

    /// Run the print UI with a KimiSoul for processing
    pub async fn run_with_soul(
        &mut self,
        soul: &mut KimiSoul,
        prompt: &str,
    ) -> UIResult<()> {
        info!("Running print UI with prompt: {}", prompt);

        // Create LLM provider
        let config = kimi_core::config::load_config(None)
            .map_err(|e| UIError::Shell(format!("Failed to load config: {}", e)))?;
        let provider = llm::create_provider(&config).await
            .map_err(|e| UIError::Core(format!("Failed to create provider: {}", e)))?;

        // Create channels for wire communication
        let (ui_tx, mut ui_rx) = mpsc::channel::<WireMessage>(100);
        let wire = WireSoulSide::with_sender(ui_tx.clone());

        // Create user input
        let user_input = UserInput {
            text: prompt.to_string(),
            attachments: vec![],
        };

        // Run LLM processing and UI loop concurrently
        let llm_future = async {
            match soul.process_with_llm(provider.as_ref(), user_input, &wire).await {
                Ok(_) => {
                    let _ = ui_tx.send(WireMessage::TurnEnd).await;
                    Ok(())
                }
                Err(e) => {
                    let _ = ui_tx.send(WireMessage::TextPart { 
                        text: format!("Error: {}", e) 
                    }).await;
                    let _ = ui_tx.send(WireMessage::TurnEnd).await;
                    Err(UIError::Core(e.to_string()))
                }
            }
        };

        // Run both futures concurrently
        tokio::select! {
            result = llm_future => {
                result?;
            }
            result = self.run_ui_loop(&mut ui_rx) => {
                result?;
            }
        }

        Ok(())
    }

    async fn run_ui_loop(
        &self,
        ui_rx: &mut mpsc::Receiver<WireMessage>,
    ) -> UIResult<()> {
        loop {
            match ui_rx.recv().await {
                Some(msg) => {
                    match msg {
                        WireMessage::TextPart { text } => {
                            print!("{}", text);
                            std::io::Write::flush(&mut std::io::stdout()).map_err(UIError::Io)?;
                        }
                        WireMessage::ThinkPart { text } => {
                            // In print mode, we might want to suppress thinking
                            if self.cli.verbose {
                                eprintln!("[Thinking: {}]", text);
                            }
                        }
                        WireMessage::ToolCall { name, arguments, .. } => {
                            if self.cli.verbose {
                                eprintln!("[Tool: {}] {}", name, arguments);
                            }
                        }
                        WireMessage::ToolResult { output, is_error, .. } => {
                            if self.cli.verbose {
                                if is_error {
                                    eprintln!("[Tool Error: {}]", output);
                                } else {
                                    eprintln!("[Tool Result: {}]", output);
                                }
                            }
                        }
                        WireMessage::TurnEnd => {
                            println!(); // New line after response
                            break;
                        }
                        WireMessage::StatusUpdate { context_usage, token_usage, .. } => {
                            if self.cli.verbose {
                                if let Some(usage) = context_usage {
                                    eprintln!("[Context: {:.1}%]", usage * 100.0);
                                }
                                if let Some(tokens) = token_usage {
                                    eprintln!("[Tokens: {} in / {} out]", 
                                        tokens.input_tokens, tokens.output_tokens);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                None => break,
            }
        }

        Ok(())
    }

    async fn execute_prompt(&self, prompt: &str) -> UIResult<()> {
        debug!("Executing prompt: {}", prompt);

        // Placeholder implementation
        let mut output = String::new();

        // Add header if verbose
        if self.cli.verbose {
            output.push_str(&format!(
                "{} Processing prompt...\n",
                Style::new().fg(Color::Cyan).paint("[INFO]")
            ));
        }

        // Simulated response
        output.push_str(&format!(
            "Response to: {}\n",
            Style::new().bold().paint(prompt)
        ));

        // Add footer if verbose
        if self.cli.verbose {
            output.push_str(&format!(
                "{} Done.\n",
                Style::new().fg(Color::Cyan).paint("[INFO]")
            ));
        }

        println!("{}", output);

        Ok(())
    }

    async fn continue_session(&self) -> UIResult<()> {
        debug!("Continuing last session");

        println!("Continuing previous session...");

        // If there's a prompt, execute it
        if let Some(ref prompt) = self.cli.prompt {
            self.execute_prompt(prompt).await?;
        } else {
            println!("No prompt provided for session continuation.");
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl UI for PrintUI {
    async fn run(&mut self) -> UIResult<()>
    where
        Self: Sized,
    {
        info!("Running print UI (legacy mode)");

        // Handle continue mode
        if self.cli.continue_ {
            return self.continue_session().await;
        }

        // Handle single prompt
        if let Some(ref prompt) = self.cli.prompt {
            return self.execute_prompt(prompt).await;
        }

        // If no prompt and not continuing, read from stdin
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(UIError::Io)?;

        let input = input.trim();
        if input.is_empty() {
            return Err(UIError::InvalidInput(
                "No input provided. Use -p/--prompt or pipe input.".to_string(),
            ));
        }

        self.execute_prompt(input).await
    }

    fn message(&self, msg: &str) {
        println!("{}", msg);
    }

    fn error(&self, err: &str) {
        eprintln!("{}", Style::new().fg(Color::Red).paint(err));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[tokio::test]
    async fn test_print_ui_creation() {
        let cli = Cli::parse_from(["kimi", "--print", "-p", "test"]);
        let ui = PrintUI::new(cli);
        assert!(ui.is_ok());
    }
}
