# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Kimi RCLI, please report it responsibly.

### How to Report

1. **Do NOT** open a public issue on GitHub
2. Email the maintainers directly at: [security@example.com] (replace with actual contact)
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 1 week
- **Fix Timeline**: Depends on severity
  - Critical: 1-2 weeks
  - High: 2-4 weeks
  - Medium/Low: Next release cycle

## Security Best Practices

### For Users

1. **Keep your token secure**
   - OAuth tokens are stored in `~/.kimi/credentials/` with restricted permissions (0o600)
   - Never share your credentials directory
   - Use `/logout` when done on shared machines

2. **Review tool executions**
   - The shell asks for approval before executing potentially dangerous operations
   - Use `/yolo` with caution - it auto-approves all actions

3. **Keep the binary updated**
   - Check for updates regularly
   - Review the changelog before updating

### For Developers

1. **Token Handling**
   - Never log or expose OAuth tokens
   - Use the `secrecy` crate for sensitive data
   - Clear memory after use when possible

2. **Shell Execution**
   - All shell commands are executed with user permissions
   - Commands are validated before execution
   - Timeout protection is in place

3. **File Operations**
   - Path validation prevents directory traversal
   - Files outside working directory require absolute paths
   - Binary files are handled appropriately

## Known Security Considerations

### OAuth Token Storage

Tokens are stored in `~/.kimi/credentials/` with file permissions 0o600 (read/write for owner only). The implementation:
- Uses atomic writes to prevent corruption
- Validates tokens before use
- Supports automatic refresh

### Shell Command Execution

The shell tool:
- Validates commands before execution
- Supports timeouts (default 60s, max 5min)
- Runs with user's environment
- Does not support interactive commands

### MCP (Model Context Protocol)

When using MCP servers:
- Verify server authenticity
- Review tool permissions
- Be cautious with sensitive data

## Security Features

- **Approval System**: Dangerous operations require user confirmation
- **YOLO Mode**: Can be toggled but defaults to safe mode
- **Path Validation**: Prevents access outside working directory
- **Timeout Protection**: Prevents hanging operations
- **Secure Token Storage**: Proper file permissions and encryption

## Vulnerability Disclosure Policy

We follow responsible disclosure:
1. Reporter submits vulnerability privately
2. We acknowledge and assess
3. We develop and test a fix
4. We coordinate disclosure timeline with reporter
5. We release fix and publicly disclose

## Credits

We appreciate security researchers who help keep Kimi RCLI secure. Responsible disclosures will be acknowledged in our changelog.
