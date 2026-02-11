# Kimi CLI Rust Port Documentation

This document provides a comprehensive analysis of the original Python codebase and a roadmap for porting it to Rust.

## Overview

Kimi CLI is an AI agent that runs in the terminal, helping users complete software development tasks. It features:
- Interactive shell UI with slash commands
- Multiple UI modes (shell, print, ACP, wire)
- Tool system for file operations, shell execution, web search/fetch
- MCP (Model Context Protocol) support for external tools
- Subagent system for parallel task execution
- Session management with context persistence

## Architecture Analysis

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Entry (Typer)                        │
│                    src/kimi_cli/cli/__init__.py                  │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      KimiCLI (App Layer)                         │
│                      src/kimi_cli/app.py                         │
│  - Configuration loading                                         │
│  - LLM provider setup                                            │
│  - Runtime creation                                              │
│  - Session management                                            │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                     KimiSoul (Core Loop)                         │
│                  src/kimi_cli/soul/kimisoul.py                   │
│  - Main agent loop                                               │
│  - Turn/step management                                          │
│  - Slash command handling                                        │
│  - Context compaction                                            │
└─────────────────────────────────────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│    Agent      │      │    Context    │      │    Wire       │
│   (System     │      │  (Conversation│      │  (UI Comm)    │
│   Prompt +    │      │    History)   │      │               │
│   Tools)      │      │               │      │               │
└───────────────┘      └───────────────┘      └───────────────┘
```

### Core Modules

#### 1. CLI Layer (`src/kimi_cli/cli/`)
- **Entry Point**: `__init__.py` - Typer-based CLI with multiple commands
- **MCP Management**: `mcp.py` - MCP server management commands
- **Web Interface**: `web.py` - Web UI commands
- **Toad TUI**: `toad.py` - Terminal UI integration

**Key Commands:**
- `kimi` - Main interactive shell
- `kimi --print` - Non-interactive print mode
- `kimi acp` - Run as ACP server
- `kimi login/logout` - Authentication
- `kimi mcp` - MCP server management
- `kimi web` - Web UI

#### 2. App Layer (`src/kimi_cli/app.py`)

**KimiCLI Class:**
```python
class KimiCLI:
    @staticmethod
    async def create(session, config, model_name, ...) -> KimiCLI
    async def run(user_input, cancel_event) -> AsyncGenerator[WireMessage]
    async def run_shell(command) -> bool
    async def run_print(input_format, output_format, command) -> bool
    async def run_acp() -> None
    async def run_wire_stdio() -> None
```

**Responsibilities:**
- Configuration loading and validation
- LLM provider initialization
- Runtime creation
- UI mode dispatch

#### 3. Soul Layer (`src/kimi_cli/soul/`)

**Core Components:**

| File | Purpose |
|------|---------|
| `kimisoul.py` | Main agent loop, turn/step management |
| `agent.py` | Runtime, Agent, LaborMarket classes |
| `context.py` | Conversation history and checkpoints |
| `toolset.py` | Tool loading and execution |
| `approval.py` | User approval system |
| `compaction.py` | Context compaction when too long |
| `slash.py` | Slash command registry |
| `denwarenji.py` | D-Mail system for checkpointed replies |

**KimiSoul Flow:**
```
run(user_input)
  ├── Parse slash commands
  ├── _turn(user_message)
  │   ├── _checkpoint()
  │   ├── _context.append_message()
  │   └── _agent_loop()
  │       ├── _step() [repeated until done]
  │       │   ├── kosong.step() [LLM call]
  │       │   ├── Wait for tool results
  │       │   └── _grow_context()
  │       └── compact_context() [if needed]
  └── wire_send(TurnEnd)
```

#### 4. Wire Layer (`src/kimi_cli/wire/`)

**Purpose:** Communication channel between Soul and UI

**Key Types:**
- `TurnBegin/TurnEnd` - Turn lifecycle
- `StepBegin/StepInterrupted` - Step lifecycle
- `StatusUpdate` - Token usage, context usage
- `ContentPart` - Text, think, image, audio, video
- `ToolCall/ToolResult` - Tool execution
- `ApprovalRequest/ApprovalResponse` - User approval

**Wire Structure:**
```
Wire (spmc channel)
├── WireSoulSide (send messages)
├── WireUISide (receive messages)
└── WireRecorder (persist to file)
```

#### 5. Tools (`src/kimi_cli/tools/`)

**Built-in Tools:**

| Category | Tools |
|----------|-------|
| File | ReadFile, WriteFile, StrReplaceFile, Glob, Grep, ReadMediaFile |
| Shell | Shell (bash/powershell) |
| Web | SearchWeb, FetchURL |
| Multi-agent | Task (spawn subagents) |
| Todo | SetTodoList |
| Think | Think |
| DMail | SendDMail |

**Tool Architecture:**
```python
class SomeTool(CallableTool2[Params]):
    name: str = "ToolName"
    params: type[Params] = Params
    
    async def __call__(self, params: Params) -> ToolReturnValue:
        # Implementation
        return ToolOk(output="result") or ToolError(message="error")
```

#### 6. UI Layer (`src/kimi_cli/ui/`)

**Modes:**
- **Shell** (`ui/shell/`) - Interactive TUI with prompt-toolkit
- **Print** (`ui/print/`) - Non-interactive output
- **ACP** (`ui/acp/`) - Agent Client Protocol server
- **Wire** (`wire/server.py`) - Wire protocol server

#### 7. Configuration (`src/kimi_cli/config.py`)

**Config Structure:**
```python
class Config(BaseModel):
    default_model: str
    default_thinking: bool
    default_yolo: bool
    models: dict[str, LLMModel]
    providers: dict[str, LLMProvider]
    loop_control: LoopControl
    services: Services
    mcp: MCPConfig
```

#### 8. LLM Layer (`src/kimi_cli/llm.py`)

**Supported Providers:**
- `kimi` - Moonshot AI Kimi models
- `openai_legacy` - OpenAI compatible
- `openai_responses` - OpenAI Responses API
- `anthropic` - Claude models
- `gemini` - Google Gemini
- `vertexai` - Google Vertex AI

#### 9. Session Management (`src/kimi_cli/session.py`)

**Session Structure:**
```python
@dataclass
class Session:
    id: UUID
    work_dir: KaosPath
    context_file: Path
    wire_file: Path
    created_at: datetime
```

### External Dependencies (Workspace Packages)

#### Kosong (`packages/kosong/`)
LLM abstraction layer for agent applications.

**Key Components:**
- `ChatProvider` - Unified interface for LLM providers
- `Message` - Standardized message format
- `Toolset/Tool` - Tool abstraction
- `generate()` - Streaming completion
- `step()` - Tool-enabled LLM step

#### Kaos (`packages/kaos/`)
OS abstraction layer for file operations and command execution.

**Key Components:**
- `KaosPath` - Async path operations
- `exec()` - Async subprocess execution
- `AsyncReadable/AsyncWritable` - Stream protocols
- Local and SSH backends

## Data Flow

### Normal Turn Flow
```
1. User Input → KimiSoul.run()
2. Parse slash commands (if any)
3. _turn(user_message)
   - Checkpoint context
   - Append user message
   - _agent_loop()
4. _agent_loop()
   - For each step:
     - Check context size → compact if needed
     - _step()
       - kosong.step() → LLM call
       - Stream response parts via Wire
       - Wait for tool results
       - _grow_context()
     - Check stop conditions
5. TurnEnd
```

### Tool Execution Flow
```
1. LLM returns tool_calls
2. kosong.step() returns StepResult
3. Tool calls executed concurrently
4. Each tool:
   - Check approval (if needed)
   - Execute
   - Return ToolResult
5. Results appended to context
6. Next step begins
```

### Approval Flow
```
1. Tool calls approval.request()
2. Approval creates Request
3. _pipe_approval_to_wire() sends ApprovalRequest
4. UI displays request, waits for user
5. User responds → ApprovalResponse
6. Request resolved, tool continues/rejects
```

## Rust Port Strategy

### Phase 1: Core Infrastructure

1. **Project Structure**
   ```
   kimi-rcli/
   ├── Cargo.toml
   ├── crates/
   │   ├── kimi-core/       # Soul, context, wire
   │   ├── kimi-cli/        # CLI entry, UI
   │   ├── kimi-tools/      # Built-in tools
   │   ├── kosong-rs/       # LLM abstraction
   │   └── kaos-rs/         # OS abstraction
   ```

2. **Async Runtime**
   - Use `tokio` for async runtime
   - Replace asyncio patterns with tokio equivalents

3. **Configuration**
   - Use `serde` + `toml`/`json` for config
   - Replace pydantic with custom validation or `validator` crate

### Phase 2: Core Modules

1. **Wire Protocol**
   - Define message types as Rust enums/structs
   - Use `serde_json` for serialization
   - Implement async channels with `tokio::sync::mpsc`

2. **Context Management**
   - Message history with efficient storage
   - Checkpoint system with serialization
   - Token counting (approximate)

3. **Tool System**
   - Trait-based tool definition
   - Async tool execution
   - Dynamic tool registration

4. **LLM Integration**
   - Port kosong's ChatProvider trait
   - Implement providers for each backend
   - Streaming response handling

### Phase 3: UI Layer

1. **Shell UI**
   - Use `rustyline` or `reedline` for readline
   - Implement autocomplete for slash commands
   - Syntax highlighting with `syntect`

2. **Print UI**
   - Simple stdout output
   - JSON streaming support

3. **ACP Server**
   - HTTP server with `axum` or `actix-web`
   - WebSocket support for real-time updates

### Phase 4: Tools

Port tools in order of priority:
1. ReadFile, WriteFile, StrReplaceFile
2. Shell
3. Glob, Grep
4. SearchWeb, FetchURL
5. Task (subagents)
6. Todo, Think, DMail

### Phase 5: Advanced Features

1. **MCP Support**
   - MCP client implementation
   - Tool translation layer

2. **Session Management**
   - Persistence with SQLite or files
   - Session restoration

3. **OAuth Authentication**
   - HTTP server for callback
   - Token storage with keyring

## Key Design Decisions

### 1. Error Handling
- Use `thiserror` for error definitions
- Structured errors with context
- Propagate errors via Wire protocol

### 2. Concurrency
- Spawn tasks with `tokio::spawn`
- Use channels for communication
- Cancellation via `tokio_util::sync::CancellationToken`

### 3. Trait Design
```rust
// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    async fn execute(&self, params: Value) -> ToolResult;
}

// ChatProvider trait
#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn generate(&self, messages: &[Message]) -> Result<GenerateResult, ChatError>;
    fn with_thinking(&self, effort: ThinkingEffort) -> Box<dyn ChatProvider>;
}
```

### 4. Message Types
```rust
pub enum WireMessage {
    TurnBegin { user_input: UserInput },
    TurnEnd,
    StepBegin { n: usize },
    StepInterrupted,
    ContentPart(ContentPart),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
    ApprovalRequest(ApprovalRequest),
    ApprovalResponse(ApprovalResponse),
    // ...
}
```

## Dependencies to Replace

| Python Package | Rust Equivalent |
|----------------|-----------------|
| typer | clap |
| pydantic | serde + validator |
| aiohttp | reqwest + tokio |
| prompt-toolkit | rustyline/reedline |
| rich | ratatui + syntect |
| loguru | tracing + tracing-subscriber |
| pyyaml | serde_yaml |
| jinja2 | minijinja |
| tenacity | tokio-retry |
| fastmcp | custom MCP implementation |
| kosong | kosong-rs (port) |
| kaos | kaos-rs (port) |

## Testing Strategy

1. **Unit Tests**: Each module with `cargo test`
2. **Integration Tests**: CLI behavior with `assert_cmd`
3. **E2E Tests**: Full agent workflows
4. **Mock LLM**: Scripted responses for deterministic tests

## Migration Checklist

- [ ] Project scaffolding and CI setup
- [ ] Core types (Wire messages, ContentPart, ToolCall)
- [ ] Wire protocol (channels, serialization)
- [ ] Configuration (TOML/JSON parsing)
- [ ] Context management (history, checkpoints)
- [ ] Tool trait and registry
- [ ] Basic tools (ReadFile, WriteFile, Shell)
- [ ] LLM abstraction (kosong-rs)
- [ ] KimiSoul main loop
- [ ] Shell UI
- [ ] Print UI
- [ ] Session management
- [ ] ACP server
- [ ] MCP support
- [ ] Advanced tools (Task, DMail)
- [ ] OAuth authentication
- [ ] Web UI
- [ ] Documentation

## Notes

1. **Python-specific features to handle:**
   - Dynamic tool loading via import paths
   - Jinja2 templating for system prompts
   - Python code execution in shell mode
   - PyInstaller binary builds

2. **Rust advantages:**
   - Better performance
   - Type safety
   - Easier distribution (single binary)
   - Memory safety

3. **Challenges:**
   - Async ecosystem differences
   - Trait object limitations
   - No runtime reflection
   - MCP protocol implementation
