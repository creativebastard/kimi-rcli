# Kimi RCLI

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

A Rust implementation of the Kimi CLI agent for software engineering tasks. This is a community port of the original [kimi-cli](https://github.com/MoonshotAI/kimi-cli) from Python to Rust, offering improved performance and native binary distribution.

## Features

- 🤖 **AI-Powered Coding Assistant** - Get help with code, debugging, architecture, and more
- 🔧 **Built-in Tools** - File operations, shell execution, web search, and more
- 💬 **Interactive Shell** - Rich terminal UI with syntax highlighting and autocompletion
- 🔐 **OAuth Authentication** - Secure login with Kimi Code platform
- 🎯 **Multiple UI Modes** - Interactive shell, print mode for scripts, and ACP support
- 🚀 **High Performance** - Native Rust binary starts instantly
- 📦 **Single Binary** - Easy distribution, no Python dependencies

## Installation

### From Source

Requires Rust 1.85 or later:

```bash
git clone https://github.com/creativebastard/kimi-rcli.git
cd kimi-rcli
cargo build --release
```

The binary will be available at `target/release/kimi-cli`.

### Pre-built Binaries

Download pre-built binaries from the [releases page](https://github.com/creativebastard/kimi-rcli/releases).

## Quick Start

1. **Login to Kimi**:
   ```bash
   kimi-cli login
   ```

2. **Start the interactive shell**:
   ```bash
   kimi-cli
   ```

3. **Or run a single command**:
   ```bash
   kimi-cli --print -p "Explain this code"
   ```

## Usage

### Interactive Mode

```bash
# Start interactive shell
kimi-cli

# With specific model
kimi-cli --model kimi-k2.5

# Enable thinking mode
kimi-cli --thinking
```

### Non-Interactive Mode

```bash
# Single prompt
kimi-cli --print -p "What is Rust?"

# Read from stdin
echo "Explain lifetimes" | kimi-cli --print
```

### Slash Commands

Inside the interactive shell, use these commands:

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/login` | Authenticate with Kimi |
| `/logout` | Clear credentials |
| `/model` | Show or set current model |
| `/models` | List available models |
| `/yolo` | Toggle auto-approve mode |
| `/compact` | Compact conversation context |
| `/clear` | Clear conversation |
| `/exit` | Quit the shell |

## Configuration

Configuration is stored in `~/.config/kimi/config.toml`:

```toml
default_model = "kimi-code/kimi-k2-5"
default_thinking = false
default_yolo = false

[providers.kimi-code]
type = "kimi"
base_url = "https://api.kimi.com/coding/v1"

[models.kimi-code-kimi-k2-5]
provider = "kimi-code"
model = "kimi-k2-5"
max_tokens = 128000
```

## Architecture

```
kimi-rcli/
├── crates/
│   ├── kosong-rs/     # LLM abstraction layer
│   ├── kaos-rs/       # OS abstraction layer
│   ├── kimi-core/     # Core agent system
│   ├── kimi-tools/    # Built-in tools
│   └── kimi-cli/      # CLI and UI
```

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test --all

# Check code
cargo clippy --all -- -D warnings
```

### Project Structure

- **kosong-rs** - LLM provider abstraction (Kimi, OpenAI, etc.)
- **kaos-rs** - Async file and process operations
- **kimi-core** - Agent loop, context management, wire protocol
- **kimi-tools** - Built-in tools (file, shell, web)
- **kimi-cli** - Interactive shell and command-line interface

## Comparison with Original

| Feature | Python kimi-cli | Rust kimi-rcli |
|---------|-----------------|----------------|
| Startup Time | ~1-2 seconds | Instant |
| Binary Size | ~50MB (with Python) | ~3.3MB |
| Dependencies | Python runtime | None (static) |
| Memory Usage | Higher | Lower |
| Performance | Good | Excellent |

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Security

Please report security vulnerabilities to the maintainers. See [SECURITY.md](SECURITY.md) for details.

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

This is a community port of the original [kimi-cli](https://github.com/MoonshotAI/kimi-cli) by Moonshot AI, also licensed under Apache 2.0.

## Acknowledgments

- Original [kimi-cli](https://github.com/MoonshotAI/kimi-cli) by Moonshot AI
- [Kimi](https://www.kimi.com/) - The AI assistant platform
- Rust community for excellent tooling and libraries

## Disclaimer

This is a community project and is not officially affiliated with Moonshot AI. For the official kimi-cli, please use the [original repository](https://github.com/MoonshotAI/kimi-cli).
