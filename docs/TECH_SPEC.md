# Kimi CLI Rust Port - Technical Specification

## Module Breakdown

### 1. Core Types (`kimi-core/src/types/`)

#### Wire Protocol Types
```rust
// Core message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WireMessage {
    // Lifecycle events
    TurnBegin { user_input: UserInput },
    TurnEnd,
    StepBegin { n: usize },
    StepInterrupted,
    CompactionBegin,
    CompactionEnd,
    
    // Content
    TextPart { text: String },
    ThinkPart { text: String },
    ImageUrlPart { url: String },
    AudioUrlPart { url: String },
    VideoUrlPart { url: String },
    
    // Tooling
    ToolCall(ToolCall),
    ToolCallPart { id: String, name: String, arguments: String },
    ToolResult(ToolResult),
    
    // Approval system
    ApprovalRequest(ApprovalRequest),
    ApprovalResponse { request_id: String, response: ApprovalKind },
    
    // Status
    StatusUpdate {
        context_usage: Option<f64>,
        token_usage: Option<TokenUsage>,
        message_id: Option<String>,
    },
    
    // Subagent
    SubagentEvent {
        task_tool_call_id: String,
        event: Box<WireMessage>,
    },
}

pub type UserInput = Either<String, Vec<ContentPart>>;

pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON
}

pub struct ToolResult {
    pub tool_call_id: String,
    pub return_value: ToolReturnValue,
}

pub enum ToolReturnValue {
    Ok { output: String, message: Option<String> },
    Error { message: String, brief: Option<String> },
}

pub struct ApprovalRequest {
    pub id: String,
    pub tool_call_id: String,
    pub sender: String,
    pub action: String,
    pub description: String,
    pub display: Vec<DisplayBlock>,
}

pub enum ApprovalKind {
    Approve,
    ApproveForSession,
    Reject,
}
```

#### Content Types
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    Think { text: String },
    ImageUrl { image_url: ImageUrl },
    AudioUrl { audio_url: AudioUrl },
    VideoUrl { video_url: VideoUrl },
}

pub struct ImageUrl {
    pub url: String,
}

pub struct AudioUrl {
    pub url: String,
}

pub struct VideoUrl {
    pub url: String,
}
```

#### Display Blocks
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisplayBlock {
    Brief { text: String },
    Unknown { data: serde_json::Value },
    Diff(DiffDisplayBlock),
    Todo(TodoDisplayBlock),
    Shell(ShellDisplayBlock),
}

pub struct DiffDisplayBlock {
    pub old_path: String,
    pub new_path: String,
    pub content: String,
}

pub struct TodoDisplayBlock {
    pub items: Vec<TodoItem>,
}

pub struct TodoItem {
    pub title: String,
    pub status: TodoStatus,
}

pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

pub struct ShellDisplayBlock {
    pub language: String,
    pub command: String,
}
```

### 2. Configuration (`kimi-core/src/config/`)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_model: String,
    #[serde(default)]
    pub default_thinking: bool,
    #[serde(default)]
    pub default_yolo: bool,
    pub models: HashMap<String, LlmModel>,
    pub providers: HashMap<String, LlmProvider>,
    #[serde(default)]
    pub loop_control: LoopControl,
    #[serde(default)]
    pub services: Services,
    #[serde(default)]
    pub mcp: McpConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProvider {
    pub provider_type: ProviderType,
    pub base_url: String,
    #[serde(skip_serializing)]
    pub api_key: SecretString,
    pub env: Option<HashMap<String, String>>,
    pub custom_headers: Option<HashMap<String, String>>,
    pub oauth: Option<OAuthRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Kimi,
    OpenAiLegacy,
    OpenAiResponses,
    Anthropic,
    Gemini,
    VertexAi,
    #[serde(rename = "_echo")]
    Echo,
    #[serde(rename = "_scripted_echo")]
    ScriptedEcho,
    #[serde(rename = "_chaos")]
    Chaos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModel {
    pub provider: String,
    pub model: String,
    pub max_context_size: usize,
    pub capabilities: Option<Vec<ModelCapability>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    ImageIn,
    VideoIn,
    Thinking,
    AlwaysThinking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopControl {
    #[serde(default = "default_max_steps")]
    pub max_steps_per_turn: usize,
    #[serde(default = "default_max_retries")]
    pub max_retries_per_step: usize,
    #[serde(default = "default_max_ralph")]
    pub max_ralph_iterations: i32,
    #[serde(default = "default_reserved_context")]
    pub reserved_context_size: usize,
}

fn default_max_steps() -> usize { 100 }
fn default_max_retries() -> usize { 3 }
fn default_max_ralph() -> i32 { 0 }
fn default_reserved_context_size() -> usize { 50000 }
```

### 3. LLM Abstraction (`kosong-rs/src/`)

```rust
// Core trait for LLM providers
#[async_trait]
pub trait ChatProvider: Send + Sync {
    /// Generate a completion stream
    async fn generate(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
    ) -> Result<GenerateStream, ChatError>;
    
    /// Get the model name
    fn model_name(&self) -> &str;
    
    /// Clone with thinking enabled/disabled
    fn with_thinking(&self, effort: ThinkingEffort) -> Box<dyn ChatProvider>;
    
    /// Get capabilities
    fn capabilities(&self) -> &[ModelCapability];
}

pub enum ThinkingEffort {
    Off,
    Low,
    Medium,
    High,
}

/// Streaming response
pub struct GenerateStream {
    pub stream: Pin<Box<dyn Stream<Item = Result<StreamedPart, ChatError>> + Send>>,
}

pub enum StreamedPart {
    Content(ContentPart),
    ToolCall(ToolCall),
    Usage(TokenUsage),
    Finish(FinishReason),
}

pub struct TokenUsage {
    pub input: usize,
    pub output: usize,
    pub total: usize,
}

pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
}

/// Message for LLM conversation
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentPart>,
}

pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}
```

### 4. Tool System (`kimi-tools/src/`)

```rust
/// Tool trait - all tools implement this
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (must be unique)
    fn name(&self) -> &str;
    
    /// Tool description for LLM
    fn description(&self) -> &str;
    
    /// JSON schema for parameters
    fn parameters_schema(&self) -> serde_json::Value;
    
    /// Execute the tool
    async fn execute(&self, params: serde_json::Value) -> ToolResult;
}

pub type ToolResult = Result<ToolOutput, ToolError>;

pub struct ToolOutput {
    pub output: String,
    pub message: Option<String>,
}

pub struct ToolError {
    pub message: String,
    pub brief: Option<String>,
}

/// Tool registry
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }
    
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
    
    pub fn list(&self) -> Vec<&dyn Tool> {
        self.tools.values().map(|t| t.as_ref()).collect()
    }
}
```

#### Example Tool Implementation
```rust
// ReadFile tool
pub struct ReadFile {
    work_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct ReadFileParams {
    path: String,
    #[serde(default = "default_line_offset")]
    line_offset: usize,
    #[serde(default = "default_n_lines")]
    n_lines: usize,
}

fn default_line_offset() -> usize { 1 }
fn default_n_lines() -> usize { 1000 }

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "ReadFile"
    }
    
    fn description(&self) -> &str {
        include_str!("./read_file_desc.md")
    }
    
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "line_offset": { "type": "integer", "minimum": 1 },
                "n_lines": { "type": "integer", "minimum": 1 }
            },
            "required": ["path"]
        })
    }
    
    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: ReadFileParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid params: {}", e)))?;
        
        // Validate path
        let path = self.validate_path(&params.path)?;
        
        // Read file
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| ToolError::new(format!("Failed to read file: {}", e)))?;
        
        // Process lines
        let lines: Vec<&str> = content.lines()
            .skip(params.line_offset - 1)
            .take(params.n_lines)
            .collect();
        
        let output = lines.iter()
            .enumerate()
            .map(|(i, line)| format!("{:6}\t{}", params.line_offset + i, line))
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(ToolOutput {
            output,
            message: Some(format!("Read {} lines", lines.len())),
        })
    }
}
```

### 5. Context Management (`kimi-core/src/context.rs`)

```rust
/// Conversation context with checkpoint support
pub struct Context {
    messages: Vec<Message>,
    checkpoints: Vec<Checkpoint>,
    token_count: usize,
    context_file: PathBuf,
}

struct Checkpoint {
    message_count: usize,
    token_count: usize,
}

impl Context {
    pub async fn load(context_file: PathBuf) -> Result<Self, ContextError> {
        // Load from disk if exists
    }
    
    pub async fn append(&mut self, message: Message) -> Result<(), ContextError> {
        // Update token count
        self.token_count += estimate_tokens(&message);
        self.messages.push(message);
        Ok(())
    }
    
    pub async fn checkpoint(&mut self) -> Result<usize, ContextError> {
        let id = self.checkpoints.len();
        self.checkpoints.push(Checkpoint {
            message_count: self.messages.len(),
            token_count: self.token_count,
        });
        Ok(id)
    }
    
    pub async fn revert_to(&mut self, checkpoint_id: usize) -> Result<(), ContextError> {
        let checkpoint = self.checkpoints.get(checkpoint_id)
            .ok_or(ContextError::InvalidCheckpoint)?;
        self.messages.truncate(checkpoint.message_count);
        self.token_count = checkpoint.token_count;
        Ok(())
    }
    
    pub async fn clear(&mut self) {
        self.messages.clear();
        self.checkpoints.clear();
        self.token_count = 0;
    }
    
    pub fn history(&self) -> &[Message] {
        &self.messages
    }
    
    pub fn token_count(&self) -> usize {
        self.token_count
    }
    
    pub async fn save(&self) -> Result<(), ContextError> {
        // Persist to disk
    }
}

fn estimate_tokens(message: &Message) -> usize {
    // Simple estimation: 4 chars ≈ 1 token
    // More accurate would use tiktoken or similar
}
```

### 6. Soul / Agent Loop (`kimi-core/src/soul/`)

```rust
/// Main agent loop
pub struct KimiSoul {
    agent: Agent,
    context: Context,
    approval: Arc<Approval>,
    denwa_renji: Arc<DenwaRenji>,
    loop_control: LoopControl,
    compaction: Box<dyn CompactionStrategy>,
}

pub struct Agent {
    name: String,
    system_prompt: String,
    tools: ToolRegistry,
    runtime: Runtime,
}

pub struct Runtime {
    config: Config,
    llm: Option<Arc<dyn ChatProvider>>,
    session: Session,
    builtin_args: BuiltinArgs,
    labor_market: Arc<LaborMarket>,
    environment: Environment,
    skills: HashMap<String, Skill>,
}

impl KimiSoul {
    pub async fn run(
        &mut self,
        user_input: UserInput,
        wire: &WireSoulSide,
        cancel: &CancellationToken,
    ) -> Result<(), SoulError> {
        wire.send(WireMessage::TurnBegin { user_input: user_input.clone() });
        
        // Check for slash commands
        if let Some(cmd) = self.parse_slash_command(&user_input) {
            self.handle_slash_command(cmd, wire).await?;
        } else {
            self.run_turn(user_input, wire, cancel).await?;
        }
        
        wire.send(WireMessage::TurnEnd);
        Ok(())
    }
    
    async fn run_turn(
        &mut self,
        user_message: Message,
        wire: &WireSoulSide,
        cancel: &CancellationToken,
    ) -> Result<(), SoulError> {
        self.context.checkpoint().await?;
        self.context.append(user_message).await?;
        
        // Main agent loop
        let mut step_no = 0;
        loop {
            step_no += 1;
            if step_no > self.loop_control.max_steps_per_turn {
                return Err(SoulError::MaxStepsReached);
            }
            
            wire.send(WireMessage::StepBegin { n: step_no });
            
            // Check context size
            if self.should_compact() {
                self.compact_context(wire).await?;
            }
            
            // Run step
            match self.step(wire, cancel).await {
                Ok(StepOutcome::Continue) => continue,
                Ok(StepOutcome::Stop { final_message }) => {
                    if let Some(msg) = final_message {
                        self.context.append(msg).await?;
                    }
                    break;
                }
                Err(e) => {
                    wire.send(WireMessage::StepInterrupted);
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn step(
        &mut self,
        wire: &WireSoulSide,
        cancel: &CancellationToken,
    ) -> Result<StepOutcome, SoulError> {
        let llm = self.agent.runtime.llm.as_ref()
            .ok_or(SoulError::LlmNotSet)?;
        
        // Run LLM step with retry
        let result = self.run_with_retry(|| {
            self.run_llm_step(llm, wire, cancel)
        }).await?;
        
        // Wait for tool results
        let tool_results = result.wait_for_tools().await;
        
        // Handle D-Mail if present
        if let Some(dmail) = self.denwa_renji.fetch_pending() {
            return Err(SoulError::BackToTheFuture { checkpoint_id: dmail.checkpoint_id });
        }
        
        // Grow context
        self.grow_context(&result, &tool_results).await?;
        
        // Determine outcome
        if tool_results.iter().any(|r| r.is_rejected()) {
            Ok(StepOutcome::Stop { final_message: Some(result.message) })
        } else if result.tool_calls.is_empty() {
            Ok(StepOutcome::Stop { final_message: Some(result.message) })
        } else {
            Ok(StepOutcome::Continue)
        }
    }
}
```

### 7. Wire Protocol (`kimi-core/src/wire/`)

```rust
/// Single-producer multi-consumer channel for soul/UI communication
pub struct Wire {
    raw_tx: broadcast::Sender<WireMessage>,
    merged_tx: broadcast::Sender<WireMessage>,
    merge_buffer: Arc<Mutex<Option<ContentPart>>>,
}

impl Wire {
    pub fn new() -> Self {
        let (raw_tx, _) = broadcast::channel(256);
        let (merged_tx, _) = broadcast::channel(256);
        Self {
            raw_tx,
            merged_tx,
            merge_buffer: Arc::new(Mutex::new(None)),
        }
    }
    
    pub fn soul_side(&self) -> WireSoulSide {
        WireSoulSide {
            raw_tx: self.raw_tx.clone(),
            merged_tx: self.merged_tx.clone(),
            merge_buffer: self.merge_buffer.clone(),
        }
    }
    
    pub fn ui_side(&self) -> WireUISide {
        WireUISide {
            raw_rx: self.raw_tx.subscribe(),
        }
    }
}

pub struct WireSoulSide {
    raw_tx: broadcast::Sender<WireMessage>,
    merged_tx: broadcast::Sender<WireMessage>,
    merge_buffer: Arc<Mutex<Option<ContentPart>>>,
}

impl WireSoulSide {
    pub fn send(&self, msg: WireMessage) {
        // Send raw message
        let _ = self.raw_tx.send(msg.clone());
        
        // Handle merging for content parts
        match msg {
            WireMessage::TextPart { text } => {
                // Merge consecutive text parts
                let mut buf = self.merge_buffer.lock().unwrap();
                match buf.as_mut() {
                    Some(ContentPart::Text { text: buf_text }) => {
                        buf_text.push_str(&text);
                    }
                    _ => {
                        self.flush_merged();
                        *buf = Some(ContentPart::Text { text });
                    }
                }
            }
            _ => {
                self.flush_merged();
                let _ = self.merged_tx.send(msg);
            }
        }
    }
    
    fn flush_merged(&self) {
        let mut buf = self.merge_buffer.lock().unwrap();
        if let Some(part) = buf.take() {
            let _ = self.merged_tx.send(WireMessage::from(part));
        }
    }
}

pub struct WireUISide {
    raw_rx: broadcast::Receiver<WireMessage>,
}

impl WireUISide {
    pub async fn recv(&mut self) -> Option<WireMessage> {
        self.raw_rx.recv().await.ok()
    }
}
```

### 8. Approval System (`kimi-core/src/approval.rs`)

```rust
pub struct Approval {
    yolo: bool,
    pending: Arc<Mutex<Option<Request>>>,
}

pub struct Request {
    id: String,
    tool_call_id: String,
    sender: String,
    action: String,
    description: String,
    display: Vec<DisplayBlock>,
    response_tx: oneshot::Sender<ApprovalKind>,
}

impl Approval {
    pub fn new(yolo: bool) -> Self {
        Self {
            yolo,
            pending: Arc::new(Mutex::new(None)),
        }
    }
    
    pub async fn request(
        &self,
        tool_call_id: &str,
        sender: &str,
        action: &str,
        description: &str,
        display: Vec<DisplayBlock>,
    ) -> Result<ApprovalKind, ApprovalError> {
        if self.yolo {
            return Ok(ApprovalKind::Approve);
        }
        
        let (tx, rx) = oneshot::channel();
        let request = Request {
            id: uuid::Uuid::new_v4().to_string(),
            tool_call_id: tool_call_id.to_string(),
            sender: sender.to_string(),
            action: action.to_string(),
            description: description.to_string(),
            display,
            response_tx: tx,
        };
        
        {
            let mut pending = self.pending.lock().unwrap();
            *pending = Some(request);
        }
        
        // Wait for response via wire
        rx.await.map_err(|_| ApprovalError::Cancelled)
    }
    
    pub fn resolve(&self, request_id: &str, response: ApprovalKind) -> Result<(), ApprovalError> {
        let mut pending = self.pending.lock().unwrap();
        if let Some(req) = pending.take() {
            if req.id == request_id {
                let _ = req.response_tx.send(response);
                Ok(())
            } else {
                Err(ApprovalError::InvalidRequest)
            }
        } else {
            Err(ApprovalError::NoPendingRequest)
        }
    }
}
```

### 9. Session Management (`kimi-core/src/session.rs`)

```rust
pub struct Session {
    pub id: Uuid,
    pub work_dir: PathBuf,
    pub context_file: PathBuf,
    pub wire_file: PathBuf,
    pub created_at: DateTime<Utc>,
}

pub struct SessionManager {
    data_dir: PathBuf,
}

impl SessionManager {
    pub async fn create(&self, work_dir: PathBuf) -> Result<Session, SessionError> {
        let id = Uuid::new_v4();
        let session_dir = self.data_dir.join("sessions").join(id.to_string());
        tokio::fs::create_dir_all(&session_dir).await?;
        
        let session = Session {
            id,
            work_dir,
            context_file: session_dir.join("context.jsonl"),
            wire_file: session_dir.join("wire.jsonl"),
            created_at: Utc::now(),
        };
        
        // Save metadata
        self.save_metadata(&session).await?;
        
        Ok(session)
    }
    
    pub async fn find(&self, work_dir: &Path, session_id: &str) -> Result<Option<Session>, SessionError> {
        // Load session by ID
    }
    
    pub async fn continue_last(&self, work_dir: &Path) -> Result<Option<Session>, SessionError> {
        // Find most recent session for work_dir
    }
}
```

### 10. Shell UI (`kimi-cli/src/ui/shell.rs`)

```rust
pub struct Shell {
    soul: KimiSoul,
    welcome_info: Vec<WelcomeInfoItem>,
    editor: Reedline,
}

impl Shell {
    pub async fn run(&mut self, initial_command: Option<String>) -> Result<bool, ShellError> {
        // Print welcome
        self.print_welcome();
        
        // Main input loop
        loop {
            let prompt = self.render_prompt();
            
            match self.editor.read_line(&prompt) {
                Ok(Signal::Success(line)) => {
                    if let Some(result) = self.handle_input(&line).await? {
                        return Ok(result);
                    }
                }
                Ok(Signal::CtrlC) => continue,
                Ok(Signal::CtrlD) => return Ok(true),
                Err(e) => return Err(ShellError::Readline(e)),
            }
        }
    }
    
    async fn handle_input(&mut self, input: &str) -> Result<Option<bool>, ShellError> {
        let input = input.trim();
        
        if input.is_empty() {
            return Ok(None);
        }
        
        // Handle shell mode toggle (Ctrl-X equivalent)
        if input.starts_with("!") {
            return self.run_shell_command(&input[1..]).await.map(|_| None);
        }
        
        // Handle slash commands
        if input.starts_with("/") {
            return self.handle_slash_command(&input[1..]).await;
        }
        
        // Normal agent input
        let wire = Wire::new();
        let cancel = CancellationToken::new();
        
        // Spawn UI task
        let ui_handle = tokio::spawn(self.run_ui_loop(wire.ui_side()));
        
        // Run soul
        self.soul.run(
            UserInput::Left(input.to_string()),
            &wire.soul_side(),
            &cancel,
        ).await?;
        
        // Wait for UI
        ui_handle.await??;
        
        Ok(None)
    }
}
```

## File Structure

```
kimi-rcli/
├── Cargo.toml
├── crates/
│   ├── kimi-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types/
│   │       │   ├── mod.rs
│   │       │   ├── wire.rs
│   │       │   ├── content.rs
│   │       │   └── display.rs
│   │       ├── config/
│   │       │   ├── mod.rs
│   │       │   ├── model.rs
│   │       │   └── loader.rs
│   │       ├── soul/
│   │       │   ├── mod.rs
│   │       │   ├── kimisoul.rs
│   │       │   ├── agent.rs
│   │       │   ├── context.rs
│   │       │   ├── toolset.rs
│   │       │   ├── approval.rs
│   │       │   └── compaction.rs
│   │       ├── wire/
│   │       │   ├── mod.rs
│   │       │   ├── channel.rs
│   │       │   └── file.rs
│   │       ├── session.rs
│   │       └── skill/
│   │           ├── mod.rs
│   │           ├── discovery.rs
│   │           └── flow.rs
│   │
│   ├── kimi-tools/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── file/
│   │       │   ├── mod.rs
│   │       │   ├── read.rs
│   │       │   ├── write.rs
│   │       │   ├── replace.rs
│   │       │   ├── glob.rs
│   │       │   └── grep.rs
│   │       ├── shell.rs
│   │       ├── web/
│   │       │   ├── mod.rs
│   │       │   ├── search.rs
│   │       │   └── fetch.rs
│   │       ├── multiagent/
│   │       │   ├── mod.rs
│   │       │   └── task.rs
│   │       └── todo.rs
│   │
│   ├── kimi-cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── cli/
│   │       │   ├── mod.rs
│   │       │   ├── commands.rs
│   │       │   └── mcp.rs
│   │       └── ui/
│   │           ├── mod.rs
│   │           ├── shell.rs
│   │           ├── print.rs
│   │           └── acp.rs
│   │
│   ├── kosong-rs/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── chat_provider/
│   │       │   ├── mod.rs
│   │       │   ├── kimi.rs
│   │       │   ├── openai.rs
│   │       │   └── anthropic.rs
│   │       ├── message.rs
│   │       └── tooling/
│   │           ├── mod.rs
│   │           ├── tool.rs
│   │           └── toolset.rs
│   │
│   └── kaos-rs/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── path.rs
│           └── exec.rs
│
└── docs/
    ├── PORTING.md
    └── TECH_SPEC.md
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_read_file_tool() {
        let tool = ReadFile::new(PathBuf::from("/tmp"));
        let result = tool.execute(json!({
            "path": "test.txt"
        })).await;
        assert!(result.is_ok());
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_full_turn() {
    let (mut soul, wire, cancel) = setup_test_soul().await;
    
    soul.run(
        UserInput::Left("Hello".to_string()),
        &wire.soul_side(),
        &cancel,
    ).await.unwrap();
    
    // Verify wire messages
}
```

## Performance Considerations

1. **Async I/O**: Use tokio for all async operations
2. **Streaming**: Stream LLM responses, don't buffer
3. **Context Compaction**: Compact before hitting token limit
4. **File Operations**: Use async file I/O
5. **Memory**: Avoid cloning large contexts; use Arc where appropriate

## Security Considerations

1. **Path Validation**: All file paths validated against work_dir
2. **Approval System**: Dangerous operations require user approval
3. **Secret Handling**: API keys in keyring or env vars, never logged
4. **Sandboxing**: Shell commands run with restricted env
