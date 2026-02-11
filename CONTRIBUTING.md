# Contributing to Kimi RCLI

Thank you for your interest in contributing to Kimi RCLI! This document provides guidelines for contributing to the project.

## Code of Conduct

Be respectful, inclusive, and constructive in all interactions.

## How to Contribute

### Reporting Bugs

1. Check if the issue already exists in the [issue tracker](https://github.com/creativebastard/kimi-rcli/issues)
2. If not, create a new issue with:
   - Clear title and description
   - Steps to reproduce
   - Expected vs actual behavior
   - System information (OS, Rust version)
   - Relevant logs or screenshots

### Suggesting Features

1. Open an issue with the "feature request" label
2. Describe the feature and its use case
3. Discuss implementation approach

### Pull Requests

1. **Fork** the repository
2. **Create a branch** for your changes (`git checkout -b feature/my-feature`)
3. **Make your changes** following our coding standards
4. **Test** your changes (`cargo test --all`)
5. **Commit** with clear messages following [Conventional Commits](https://www.conventionalcommits.org/)
6. **Push** to your fork
7. **Open a Pull Request** with:
   - Clear description of changes
   - Reference to related issues
   - Screenshots/logs if applicable

## Development Setup

### Prerequisites

- Rust 1.85 or later
- Git

### Building

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/kimi-rcli.git
cd kimi-rcli

# Build debug version
cargo build

# Build release version
cargo build --release

# Run tests
cargo test --all

# Run clippy
cargo clippy --all -- -D warnings

# Format code
cargo fmt
```

## Coding Standards

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` and fix all warnings
- Write documentation for public APIs

### Code Quality

- **No compiler warnings** - Code must compile cleanly
- **No clippy warnings** - All lints must pass
- **Tests** - Add tests for new functionality
- **Documentation** - Document public APIs with examples

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Test changes
- `chore`: Build/tooling changes

Examples:
```
feat(tools): add grep tool for file search

Add a new grep tool that supports regex patterns
and file filtering options.

fix(shell): handle ctrl-c gracefully

Prevent panic when user interrupts with Ctrl-C.
```

## Project Structure

```
crates/
├── kosong-rs/     # LLM abstraction
├── kaos-rs/       # OS abstraction
├── kimi-core/     # Core agent system
├── kimi-tools/    # Built-in tools
└── kimi-cli/      # CLI and UI
```

### Adding a New Tool

1. Create tool in `crates/kimi-tools/src/`
2. Implement the `Tool` trait
3. Add to tool registry
4. Write tests
5. Update documentation

### Adding a New Slash Command

1. Add command to completer list in `crates/kimi-cli/src/ui/shell.rs`
2. Add handler in `handle_command` method
3. Add to help text
4. Test the command

## Testing

### Unit Tests

```bash
# Run all tests
cargo test --all

# Run tests for specific crate
cargo test -p kimi-core

# Run with output
cargo test --all -- --nocapture
```

### Integration Tests

Integration tests are in the `tests/` directory (if any).

### Manual Testing

Test common workflows:
1. Login/logout
2. Interactive shell
3. Single prompt mode
4. Tool execution
5. Session management

## Documentation

- Update README.md if adding user-facing features
- Update crate-level documentation in `lib.rs`
- Add doc comments to public APIs
- Update CHANGELOG.md

## Release Process

1. Update version in `Cargo.toml` files
2. Update CHANGELOG.md
3. Create git tag
4. Build release binaries
5. Create GitHub release

## Questions?

- Open an issue for questions
- Join discussions in existing issues
- Check existing documentation

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
