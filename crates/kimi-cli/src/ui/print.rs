use std::io::{self, Read};

use nu_ansi_term::{Color, Style};
use tokio::sync::mpsc;
use tracing::{debug, info};

use kimi_core::{
    ApprovalKind,
    soul::{KimiSoul, SoulError},
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
        _soul: &mut KimiSoul,
        prompt: &str,
    ) -> UIResult<()> {
        info!("Running print UI with prompt: {}", prompt);

        // Create channels for wire communication
        let (ui_tx, mut ui_rx) = mpsc::channel::<WireMessage>(100);
        let (approval_tx, mut approval_rx) = mpsc::channel::<(WireMessage, mpsc::Sender<ApprovalKind>)>(10);

        // Create user input
        let user_input = UserInput {
            text: prompt.to_string(),
            attachments: vec![],
        };

        // Spawn the soul processing in a separate task
        let soul_handle = tokio::spawn({
            let user_input = user_input.clone();
            async move {
                // TODO: In the actual implementation, we would pass the wire channels to the soul
                // For now, simulate the soul processing
                simulate_soul_processing_print(user_input, ui_tx, approval_tx).await
            }
        });

        // Run the UI loop to display responses
        self.run_ui_loop(&mut ui_rx, &mut approval_rx).await?;

        // Wait for soul to complete
        match soul_handle.await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(UIError::Core(e.to_string()));
            }
            Err(e) => {
                return Err(UIError::Core(format!("Soul task panicked: {}", e)));
            }
        }

        Ok(())
    }

    async fn run_ui_loop(
        &self,
        ui_rx: &mut mpsc::Receiver<WireMessage>,
        approval_rx: &mut mpsc::Receiver<(WireMessage, mpsc::Sender<ApprovalKind>)>,
    ) -> UIResult<()> {
        loop {
            tokio::select! {
                Some(msg) = ui_rx.recv() => {
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
                Some((msg, response_tx)) = approval_rx.recv() => {
                    if let WireMessage::ApprovalRequest { .. } = msg {
                        // In print mode, auto-approve or reject based on yolo setting
                        let response = if self.cli.yolo {
                            ApprovalKind::Approve
                        } else {
                            // In non-yolo print mode, reject to be safe
                            ApprovalKind::Reject
                        };
                        let _ = response_tx.send(response).await;
                    }
                }
                else => break,
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

/// Simulate soul processing for print mode (placeholder until full integration)
async fn simulate_soul_processing_print(
    user_input: UserInput,
    ui_tx: mpsc::Sender<WireMessage>,
    _approval_tx: mpsc::Sender<(WireMessage, mpsc::Sender<ApprovalKind>)>,
) -> Result<(), SoulError> {
    // Send turn begin
    let _ = ui_tx.send(WireMessage::TurnBegin { user_input: user_input.clone() }).await;
    
    // Simulate processing delay
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    // Send step begin
    let _ = ui_tx.send(WireMessage::StepBegin { n: 1 }).await;
    
    // Simulate text response (more concise for print mode)
    let response = format!("Processed: {}", user_input.text);
    let _ = ui_tx.send(WireMessage::TextPart { text: response }).await;
    
    // Send turn end
    let _ = ui_tx.send(WireMessage::TurnEnd).await;
    
    Ok(())
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
