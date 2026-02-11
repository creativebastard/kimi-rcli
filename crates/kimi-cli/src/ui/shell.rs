use std::borrow::Cow;

use nu_ansi_term::{Color, Style};
use reedline::{
    default_emacs_keybindings, ColumnarMenu, Completer, DefaultCompleter, DefaultHinter,
    Emacs, FileBackedHistory, Highlighter, KeyCode, KeyModifiers, MenuBuilder, Prompt,
    PromptHistorySearch, PromptHistorySearchStatus, Reedline, ReedlineEvent, ReedlineMenu, Signal,
    ValidationResult, Validator, StyledText,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use kimi_core::{
    ApprovalKind,
    soul::{KimiSoul, SoulError, Compaction},
    types::UserInput,
    wire::WireMessage,
    Session,
    config::{load_config, save_config, Config},
};

use crate::cli::Cli;
use crate::ui::{UIError, UIResult, UI};

/// Interactive shell UI using reedline
pub struct ShellUI {
    editor: Reedline,
    prompt: Box<dyn Prompt>,
    #[allow(dead_code)]
    cli: Cli,
    config: Config,
}

/// Custom highlighter for the shell
struct ShellHighlighter;

impl Highlighter for ShellHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled_text = StyledText::new();
        styled_text.push((Style::new().fg(Color::White), line.to_string()));
        styled_text
    }
}

/// Custom prompt for Kimi
struct KimiPrompt;

impl Prompt for KimiPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Owned(Style::new().bold().fg(Color::Green).paint("kimi").to_string())
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _prompt_mode: reedline::PromptEditMode) -> Cow<'_, str> {
        Cow::Owned(Style::new().fg(Color::Cyan).paint(" ❯ ").to_string())
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Owned(Style::new().fg(Color::Cyan).paint("... ").to_string())
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!(
            "({}reverse-search: {})",
            prefix, history_search.term
        ))
    }
}

/// Input validator for multi-line support
struct KimiValidator;

impl Validator for KimiValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if line.trim().is_empty() {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

/// Command completer
struct KimiCompleter {
    inner: DefaultCompleter,
}

impl KimiCompleter {
    fn new() -> Self {
        let commands = vec![
            "/help".to_string(),
            "/h".to_string(),
            "/?".to_string(),
            "/exit".to_string(),
            "/quit".to_string(),
            "/q".to_string(),
            "/clear".to_string(),
            "/reset".to_string(),
            "/model".to_string(),
            "/models".to_string(),
            "/session".to_string(),
            "/yolo".to_string(),
            "/compact".to_string(),
            "/tools".to_string(),
            "/version".to_string(),
            "/changelog".to_string(),
            "/release-notes".to_string(),
            "/feedback".to_string(),
            "/sessions".to_string(),
            "/resume".to_string(),
            "/web".to_string(),
            "/mcp".to_string(),
            "/login".to_string(),
            "/setup".to_string(),
            "/logout".to_string(),
            "/init".to_string(),
        ];
        let inner = DefaultCompleter::new_with_wordlen(commands, 1);
        Self { inner }
    }
}

impl Completer for KimiCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<reedline::Suggestion> {
        self.inner.complete(line, pos)
    }
}

impl ShellUI {
    /// Create a new shell UI instance
    pub async fn new(cli: Cli) -> UIResult<Self> {
        info!("Initializing interactive shell UI");

        let editor = Self::create_editor()?;
        let prompt = Box::new(KimiPrompt);
        
        // Load config
        let config = load_config(None).map_err(|e| {
            UIError::Shell(format!("Failed to load config: {}", e))
        })?;
        
        // Debug: Show loaded config info
        if !config.default_model.is_empty() {
            println!("Loaded config with default model: {}", config.default_model);
        } else if !config.models.is_empty() {
            println!("Loaded config with {} models (no default set)", config.models.len());
        } else {
            println!("No models configured. Use /login to authenticate.");
        }

        Ok(Self {
            editor,
            prompt,
            cli,
            config,
        })
    }

    fn create_editor() -> UIResult<Reedline> {
        // Set up history
        let history_path = dirs::cache_dir()
            .ok_or_else(|| UIError::Shell("Failed to determine cache directory".to_string()))?
            .join("kimi")
            .join("history.txt");

        // Ensure parent directory exists
        if let Some(parent) = history_path.parent() {
            std::fs::create_dir_all(parent).map_err(UIError::Io)?;
        }

        let history = Box::new(
            FileBackedHistory::with_file(1000, history_path).map_err(|e| {
                UIError::Shell(format!("Failed to initialize history: {}", e))
            })?,
        );

        // Set up completer
        let completer = Box::new(KimiCompleter::new());

        // Set up highlighter
        let highlighter = Box::new(ShellHighlighter);

        // Set up hinter
        let hinter = Box::new(
            DefaultHinter::default()
                .with_style(Style::new().fg(Color::DarkGray))
        );

        // Set up validator
        let validator = Box::new(KimiValidator);

        // Set up menu
        let completion_menu = Box::new(
            ColumnarMenu::default()
                .with_name("completion_menu")
                .with_marker("❯ "),
        );

        // Build reedline
        let mut editor = Reedline::create()
            .with_history(history)
            .with_completer(completer)
            .with_highlighter(highlighter)
            .with_hinter(hinter)
            .with_validator(validator)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu));

        // Set up keybindings
        let mut keybindings = default_emacs_keybindings();
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('d'),
            ReedlineEvent::Edit(vec![reedline::EditCommand::Delete]),
        );
        editor = editor.with_edit_mode(Box::new(Emacs::new(keybindings)));

        Ok(editor)
    }

    /// Run the shell with a KimiSoul for processing
    pub async fn run_with_soul(&mut self, soul: &mut KimiSoul) -> UIResult<()> {
        info!("Starting interactive shell with soul");

        println!(
            "\n{}",
            Style::new()
                .bold()
                .fg(Color::Cyan)
                .paint("Welcome to Kimi CLI!")
        );
        println!(
            "{}",
            Style::new()
                .fg(Color::DarkGray)
                .paint("Type /help for commands, /exit to quit.\n")
        );

        loop {
            match self.editor.read_line(self.prompt.as_ref()) {
                Ok(Signal::Success(input)) => {
                    match self.handle_input_with_soul(input, soul).await {
                        Ok(true) => continue,
                        Ok(false) => break,
                        Err(e) => {
                            eprintln!("{} {}", 
                                Style::new().fg(Color::Red).paint("Error:"),
                                e
                            );
                            continue;
                        }
                    }
                }
                Ok(Signal::CtrlC) => {
                    println!("\nInterrupted. Press Ctrl+D or type /exit to quit.");
                    continue;
                }
                Ok(Signal::CtrlD) => {
                    println!("\nGoodbye!");
                    break;
                }
                Err(e) => {
                    return Err(UIError::Shell(format!("Readline error: {}", e)));
                }
            }
        }

        Ok(())
    }

    async fn handle_input_with_soul(
        &mut self,
        input: String,
        soul: &mut KimiSoul,
    ) -> UIResult<bool> {
        let input = input.trim();

        if input.is_empty() {
            return Ok(true);
        }

        // Handle special commands
        if input.starts_with('/') {
            return self.handle_command(input, soul).await;
        }

        // Process as regular message through the soul
        debug!("Processing user input through soul: {}", input);
        self.process_message_with_soul(input, soul).await?;

        Ok(true)
    }

    async fn handle_command(&mut self, input: &str, soul: &mut KimiSoul) -> UIResult<bool> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(true);
        }

        let cmd = parts[0];
        let _args = parts.get(1..).map(|v| v.join(" ")).unwrap_or_default();

        match cmd {
            // General commands
            "/exit" | "/quit" | "/q" => {
                println!("Goodbye!");
                Ok(false)
            }
            "/help" | "/h" | "/?" => {
                self.print_help();
                Ok(true)
            }
            "/version" => {
                self.print_version();
                Ok(true)
            }
            "/changelog" | "/release-notes" => {
                self.print_changelog();
                Ok(true)
            }
            "/feedback" => {
                self.open_feedback().await;
                Ok(true)
            }

            // Session commands
            "/sessions" | "/resume" => {
                self.list_sessions().await;
                Ok(true)
            }
            "/session" => {
                if parts.len() > 1 {
                    println!("Session: {}", parts[1]);
                } else {
                    println!("Current session: {}", soul.context.message_count());
                }
                Ok(true)
            }

            // Authentication commands
            "/login" | "/setup" => {
                if let Err(e) = crate::commands::login::execute(true).await {
                    eprintln!("{} {}", 
                        Style::new().fg(Color::Red).paint("Login failed:"),
                        e
                    );
                }
                Ok(true)
            }
            "/logout" => {
                if let Err(e) = crate::commands::login::logout().await {
                    eprintln!("{} {}", 
                        Style::new().fg(Color::Red).paint("Logout failed:"),
                        e
                    );
                }
                Ok(true)
            }

            // Context commands
            "/clear" | "/reset" => {
                soul.context.clear_messages();
                print!("\x1B[2J\x1B[1;1H");
                println!("{}", Style::new().fg(Color::Green).paint("Context cleared."));
                Ok(true)
            }
            "/compact" => {
                println!("Compacting context...");
                match soul.compaction.compact(&mut soul.context) {
                    Ok(removed) => {
                        println!("{} Removed {} messages.", 
                            Style::new().fg(Color::Green).paint("Context compacted."),
                            removed
                        );
                    }
                    Err(e) => {
                        eprintln!("{} {}", 
                            Style::new().fg(Color::Red).paint("Compaction failed:"),
                            e
                        );
                    }
                }
                Ok(true)
            }
            "/yolo" => {
                // Toggle YOLO mode in the soul's approval
                let current = soul.approval.is_yolo();
                let _new_state = !current;
                
                // We need to get a mutable reference to the approval
                // Since it's wrapped in Arc, we need to use interior mutability
                // For now, print a message indicating the limitation
                // Note: Since approval is wrapped in Arc, we can't easily modify it at runtime
                // without interior mutability. For now, inform the user about the limitation.
                println!("{}", Style::new().fg(Color::Yellow).paint("Note: YOLO mode state cannot be changed at runtime in this version."));
                println!("Current state: {}", if current { "enabled" } else { "disabled" });
                println!("Restart with --yolo flag to enable YOLO mode.");
                Ok(true)
            }

            // Other commands
            "/model" => {
                if parts.len() > 1 {
                    let model_name = parts[1];
                    // Validate model exists
                    if self.config.models.contains_key(model_name) {
                        self.config.default_model = model_name.to_string();
                        // Save config
                        if let Err(e) = save_config(&self.config, None) {
                            eprintln!("Failed to save config: {}", e);
                        } else {
                            println!("Model set to: {}", model_name);
                        }
                    } else {
                        eprintln!("Unknown model: {}", model_name);
                        eprintln!("Available models: {}", 
                            self.config.models.keys().cloned().collect::<Vec<_>>().join(", "));
                    }
                } else if self.config.default_model.is_empty() {
                    println!("No default model set. Use /login to authenticate or /model <name> to set one.");
                } else {
                    println!("Current model: {}", self.config.default_model);
                    if let Some(model) = self.config.models.get(&self.config.default_model) {
                        println!("  Provider: {}", model.provider);
                        if let Some(max_tokens) = model.max_tokens {
                            println!("  Max tokens: {}", max_tokens);
                        }
                    }
                }
                Ok(true)
            }
            "/models" => {
                if self.config.models.is_empty() {
                    println!("No models configured. Use /login to authenticate.");
                } else {
                    println!("\n{}", Style::new().bold().fg(Color::Cyan).paint("Available Models:"));
                    for (name, model) in &self.config.models {
                        let marker = if name == &self.config.default_model {
                            Style::new().fg(Color::Green).paint(" (default)")
                        } else {
                            Style::new().fg(Color::DarkGray).paint("")
                        };
                        println!("  {}{}", name, marker);
                        println!("    Provider: {}", model.provider);
                        if let Some(max_tokens) = model.max_tokens {
                            println!("    Max tokens: {}", max_tokens);
                        }
                    }
                }
                Ok(true)
            }
            "/tools" => {
                println!("Available tools:");
                for name in soul.toolset.tool_names() {
                    println!("  - {}", name);
                }
                Ok(true)
            }
            "/mcp" => {
                if let Err(e) = self.show_mcp_servers().await {
                    eprintln!("{} {}", 
                        Style::new().fg(Color::Red).paint("Error:"),
                        e
                    );
                }
                Ok(true)
            }
            "/web" => {
                println!("{}", Style::new().fg(Color::Cyan).paint("Web UI is not available in this version."));
                println!("Visit https://kimi.moonshot.cn for the web interface.");
                Ok(true)
            }
            "/init" => {
                println!("{}", Style::new().fg(Color::Cyan).paint("Initializing codebase analysis..."));
                println!("This would analyze the codebase and generate AGENTS.md");
                println!("(Full implementation pending)");
                Ok(true)
            }
            _ => {
                println!("Unknown command: {}. Type /help for available commands.", cmd);
                Ok(true)
            }
        }
    }

    async fn process_message_with_soul(
        &mut self,
        message: &str,
        _soul: &mut KimiSoul,
    ) -> UIResult<()> {
        // Create channels for wire communication
        let (ui_tx, mut ui_rx) = mpsc::channel::<WireMessage>(100);
        let (approval_tx, mut approval_rx) = mpsc::channel::<(WireMessage, mpsc::Sender<ApprovalKind>)>(10);

        // Create user input
        let user_input = UserInput {
            text: message.to_string(),
            attachments: vec![],
        };

        // Spawn the soul processing in a separate task
        let soul_handle = tokio::spawn({
            let user_input = user_input.clone();
            async move {
                // TODO: In the actual implementation, we would pass the wire channels to the soul
                // For now, simulate the soul processing
                simulate_soul_processing(user_input, ui_tx, approval_tx).await
            }
        });

        // Run the UI loop to display responses
        self.run_ui_loop(&mut ui_rx, &mut approval_rx).await?;

        // Wait for soul to complete
        match soul_handle.await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                warn!("Soul processing error: {}", e);
                return Err(UIError::Core(e.to_string()));
            }
            Err(e) => {
                warn!("Soul task panicked: {}", e);
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
        // Print assistant prefix
        print!("\n{} ", Style::new().bold().fg(Color::Blue).paint("Kimi:"));
        
        loop {
            tokio::select! {
                Some(msg) = ui_rx.recv() => {
                    match msg {
                        WireMessage::TextPart { text } => {
                            print!("{}", text);
                            std::io::Write::flush(&mut std::io::stdout()).map_err(UIError::Io)?;
                        }
                        WireMessage::ThinkPart { text } => {
                            // Display thinking in dimmed color
                            print!("{}", Style::new().fg(Color::DarkGray).paint(text));
                            std::io::Write::flush(&mut std::io::stdout()).map_err(UIError::Io)?;
                        }
                        WireMessage::ToolCall { name, arguments, .. } => {
                            println!("\n{} {}", 
                                Style::new().fg(Color::Yellow).paint("[Tool Call:]"),
                                Style::new().bold().paint(&name)
                            );
                            if !arguments.is_empty() {
                                println!("  {}: {}",
                                    Style::new().fg(Color::DarkGray).paint("Arguments"),
                                    arguments
                                );
                            }
                        }
                        WireMessage::ToolResult { output, is_error, .. } => {
                            if is_error {
                                println!("{} {}",
                                    Style::new().fg(Color::Red).paint("[Tool Error:]"),
                                    output
                                );
                            } else {
                                println!("{} {}",
                                    Style::new().fg(Color::Green).paint("[Tool Result:]"),
                                    output
                                );
                            }
                        }
                        WireMessage::TurnEnd => {
                            println!(); // New line after response
                            break;
                        }
                        WireMessage::StepBegin { n } => {
                            debug!("Step {} began", n);
                        }
                        WireMessage::StepInterrupted => {
                            println!("\n{}", 
                                Style::new().fg(Color::Yellow).paint("[Step interrupted]")
                            );
                        }
                        WireMessage::StatusUpdate { context_usage, token_usage, .. } => {
                            if let Some(usage) = context_usage {
                                debug!("Context usage: {:.1}%", usage * 100.0);
                            }
                            if let Some(tokens) = token_usage {
                                debug!("Tokens: {} in / {} out", tokens.input_tokens, tokens.output_tokens);
                            }
                        }
                        _ => {}
                    }
                }
                Some((msg, response_tx)) = approval_rx.recv() => {
                    if let WireMessage::ApprovalRequest { action, description, .. } = msg {
                        let approved = self.handle_approval_request(&action, &description).await?;
                        let _ = response_tx.send(approved).await;
                    }
                }
                else => break,
            }
        }

        Ok(())
    }

    async fn handle_approval_request(
        &self,
        action: &str,
        description: &str,
    ) -> UIResult<ApprovalKind> {
        println!("\n{}", Style::new().bold().fg(Color::Yellow).paint("=== Approval Request ==="));
        println!("{}: {}", Style::new().bold().paint("Action"), action);
        println!("{}: {}", Style::new().bold().paint("Description"), description);
        println!("{}", Style::new().fg(Color::DarkGray).paint("(y)es / (n)o / (o)nce"));
        
        // For now, auto-approve in this skeleton implementation
        // In a real implementation, we'd read user input here
        println!("{}", Style::new().fg(Color::Green).paint("Auto-approved (skeleton implementation)"));
        
        Ok(ApprovalKind::Approve)
    }

    fn print_help(&self) {
        println!("\n{}", Style::new().bold().fg(Color::Cyan).paint("Available Commands:"));
        
        println!("\n{}", Style::new().bold().fg(Color::Yellow).paint("General:"));
        println!("  {} - Show this help message", Style::new().fg(Color::Green).paint("/help, /h, /?"));
        println!("  {} - Exit the shell", Style::new().fg(Color::Green).paint("/exit, /quit, /q"));
        println!("  {} - Show version information", Style::new().fg(Color::Green).paint("/version"));
        println!("  {} - Show release notes", Style::new().fg(Color::Green).paint("/changelog, /release-notes"));
        println!("  {} - Submit feedback (open GitHub issues)", Style::new().fg(Color::Green).paint("/feedback"));
        
        println!("\n{}", Style::new().bold().fg(Color::Yellow).paint("Session:"));
        println!("  {} - List and resume sessions", Style::new().fg(Color::Green).paint("/sessions, /resume"));
        println!("  {} - Set or show current session", Style::new().fg(Color::Green).paint("/session [name]"));
        
        println!("\n{}", Style::new().bold().fg(Color::Yellow).paint("Authentication:"));
        println!("  {} - Login to Kimi (OAuth)", Style::new().fg(Color::Green).paint("/login, /setup"));
        println!("  {} - Logout from Kimi", Style::new().fg(Color::Green).paint("/logout"));
        
        println!("\n{}", Style::new().bold().fg(Color::Yellow).paint("Context:"));
        println!("  {} - Clear the screen and context", Style::new().fg(Color::Green).paint("/clear, /reset"));
        println!("  {} - Compact conversation context", Style::new().fg(Color::Green).paint("/compact"));
        println!("  {} - Toggle YOLO mode (auto-execute)", Style::new().fg(Color::Green).paint("/yolo"));
        
        println!("\n{}", Style::new().bold().fg(Color::Yellow).paint("Other:"));
        println!("  {} - Set or show current model", Style::new().fg(Color::Green).paint("/model [name]"));
        println!("  {} - List all available models", Style::new().fg(Color::Green).paint("/models"));
        println!("  {} - List available tools", Style::new().fg(Color::Green).paint("/tools"));
        println!("  {} - Show MCP servers and tools", Style::new().fg(Color::Green).paint("/mcp"));
        println!("  {} - Open Web UI (info only)", Style::new().fg(Color::Green).paint("/web"));
        println!("  {} - Analyze codebase and generate AGENTS.md", Style::new().fg(Color::Green).paint("/init"));
        
        println!();
        println!(
            "{}",
            Style::new()
                .fg(Color::DarkGray)
                .paint("Type your message and press Enter to chat with Kimi.")
        );
        println!();
    }

    fn print_version(&self) {
        println!("\n{}", Style::new().bold().fg(Color::Cyan).paint("Kimi CLI"));
        println!("  Version: {}", env!("CARGO_PKG_VERSION"));
        println!("  Repository: https://github.com/moonshot-ai/kimi-cli");
        println!();
    }

    fn print_changelog(&self) {
        println!("\n{}", Style::new().bold().fg(Color::Cyan).paint("Release Notes"));
        println!("  See the latest releases at:");
        println!("  https://github.com/moonshot-ai/kimi-cli/releases");
        println!();
    }

    async fn open_feedback(&self) {
        println!("\n{}", Style::new().bold().fg(Color::Cyan).paint("Submit Feedback"));
        println!("  Opening GitHub issues page...");
        
        let url = "https://github.com/moonshot-ai/kimi-cli/issues";
        match open::that(url) {
            Ok(_) => println!("  {}", Style::new().fg(Color::Green).paint("Browser opened successfully.")),
            Err(e) => {
                println!("  {}: {}", Style::new().fg(Color::Yellow).paint("Could not open browser"), e);
                println!("  Please visit: {}", url);
            }
        }
        println!();
    }

    async fn list_sessions(&self) {
        println!("\n{}", Style::new().bold().fg(Color::Cyan).paint("Sessions"));
        
        let work_dir = self.cli.effective_work_dir();
        match Session::list_all(&work_dir) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    println!("  No sessions found.");
                    println!("  Start chatting to create a new session.");
                } else {
                    println!("  Available sessions (newest first):");
                    println!();
                    for session in sessions {
                        let short_id = session.short_id();
                        let created = session.created_at.format("%Y-%m-%d %H:%M:%S");
                        println!("  {} - Created: {}", 
                            Style::new().fg(Color::Green).paint(&short_id),
                            created
                        );
                    }
                    println!();
                    println!("  Use {} to resume a session.", 
                        Style::new().fg(Color::Yellow).paint("kimi --continue")
                    );
                }
            }
            Err(e) => {
                eprintln!("  {}: {}", 
                    Style::new().fg(Color::Red).paint("Error listing sessions"),
                    e
                );
            }
        }
        println!();
    }

    async fn show_mcp_servers(&self) -> anyhow::Result<()> {
        println!("\n{}", Style::new().bold().fg(Color::Cyan).paint("MCP Servers"));
        
        // Load MCP config
        let config = crate::commands::mcp::load_config_from(
            &dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Failed to determine config directory"))?
                .join("kimi")
                .join("mcp.json")
        ).await?;
        
        if config.servers.is_empty() {
            println!("  No MCP servers configured.");
            println!("  Use {} to add a server.", 
                Style::new().fg(Color::Yellow).paint("kimi mcp add <name>")
            );
        } else {
            println!("  Configured servers:");
            println!();
            for (name, server) in &config.servers {
                let status = if server.enabled {
                    Style::new().fg(Color::Green).paint("● enabled")
                } else {
                    Style::new().fg(Color::DarkGray).paint("○ disabled")
                };
                println!("  {} - {}", 
                    Style::new().bold().paint(name),
                    status
                );
                println!("    Command: {} {}", server.command, server.args.join(" "));
            }
        }
        println!();
        Ok(())
    }
}

/// Simulate soul processing (placeholder until full integration)
async fn simulate_soul_processing(
    user_input: UserInput,
    ui_tx: mpsc::Sender<WireMessage>,
    _approval_tx: mpsc::Sender<(WireMessage, mpsc::Sender<ApprovalKind>)>,
) -> Result<(), SoulError> {
    // Send turn begin
    let _ = ui_tx.send(WireMessage::TurnBegin { user_input: user_input.clone() }).await;
    
    // Simulate processing delay
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Send step begin
    let _ = ui_tx.send(WireMessage::StepBegin { n: 1 }).await;
    
    // Simulate text response
    let response = format!("I received your message: '{}'", user_input.text);
    for word in response.split_whitespace() {
        let _ = ui_tx.send(WireMessage::TextPart { 
            text: format!("{} ", word) 
        }).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
    
    // Send turn end
    let _ = ui_tx.send(WireMessage::TurnEnd).await;
    
    Ok(())
}

#[async_trait::async_trait]
impl UI for ShellUI {
    async fn run(&mut self) -> UIResult<()>
    where
        Self: Sized,
    {
        // This is the old interface - we now use run_with_soul instead
        // This implementation just shows a message directing to use run_with_soul
        info!("Starting interactive shell (legacy mode)");

        println!(
            "\n{}",
            Style::new()
                .bold()
                .fg(Color::Cyan)
                .paint("Welcome to Kimi CLI!")
        );
        println!(
            "{}",
            Style::new()
                .fg(Color::DarkGray)
                .paint("Type /help for commands, /exit to quit.\n")
        );

        loop {
            match self.editor.read_line(self.prompt.as_ref()) {
                Ok(Signal::Success(input)) => {
                    let input = input.trim();
                    
                    if input.is_empty() {
                        continue;
                    }

                    // Handle special commands
                    if input == "/exit" || input == "/quit" || input == "/q" {
                        println!("Goodbye!");
                        break;
                    }
                    
                    if input == "/help" || input == "/h" || input == "/?" {
                        self.print_help();
                        continue;
                    }
                    
                    if input == "/clear" || input == "/reset" {
                        print!("\x1B[2J\x1B[1;1H");
                        continue;
                    }

                    // Placeholder response
                    println!("{}", Style::new().fg(Color::Blue).paint("Kimi: "));
                    println!(
                        "  {}",
                        Style::new()
                            .fg(Color::White)
                            .paint(format!("Received: {}", input))
                    );
                }
                Ok(Signal::CtrlC) => {
                    println!("\nInterrupted. Press Ctrl+D or type /exit to quit.");
                    continue;
                }
                Ok(Signal::CtrlD) => {
                    println!("\nGoodbye!");
                    break;
                }
                Err(e) => {
                    return Err(UIError::Shell(format!("Readline error: {}", e)));
                }
            }
        }

        Ok(())
    }

    fn message(&self, msg: &str) {
        println!("{}", msg);
    }

    fn error(&self, err: &str) {
        eprintln!("{}", Style::new().fg(Color::Red).paint(err));
    }
}
