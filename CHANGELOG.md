# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial Rust port of kimi-cli
- OAuth device flow authentication
- Interactive shell with reedline
- Built-in tools: ReadFile, WriteFile, StrReplaceFile, Glob, Grep, Shell, SearchWeb, FetchURL, SetTodoList
- Context management with checkpoints
- Approval system for tool execution
- YOLO mode for auto-approval
- Session persistence
- Configuration management (TOML)
- Slash commands: /help, /login, /logout, /model, /models, /yolo, /compact, /clear, /exit
- Multi-platform support (macOS, Linux, Windows)

### Changed
- N/A (initial release)

### Deprecated
- N/A (initial release)

### Removed
- N/A (initial release)

### Fixed
- N/A (initial release)

### Security
- N/A (initial release)

## [0.1.0] - 2025-02-11

### Added
- Initial release
- Full OAuth authentication with Kimi Code platform
- Interactive shell with syntax highlighting
- All core tools implemented
- Configuration persistence
- Session management

[Unreleased]: https://github.com/creativebastard/kimi-rcli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/creativebastard/kimi-rcli/releases/tag/v0.1.0
