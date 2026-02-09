# Security Policy

nixmate takes security seriously. Here's how the tool protects your system.

## Sudo & Privilege Escalation

nixmate never runs `sudo` commands silently. Every privileged operation (rebuild, garbage collection, service control, etc.) requires **explicit user confirmation** via an interactive prompt before execution.

## API Key Storage

API keys for AI providers (Claude, OpenAI, Ollama) are stored in:

```
~/.config/nixmate/config.toml
```

This file is created with `chmod 600` (owner read/write only). Never share this file or commit it to version control.

## No Shell Interpolation

nixmate does **not** pass user-provided data through shell interpolation. All user-visible commands use `Command::new()` with explicit argument lists. Internal health checks use `sh -c` with hardcoded command strings only.

## Pipe Input Limit

When using pipe mode (`nixos-rebuild switch 2>&1 | nixmate`), input is limited to **1 MB**. This prevents accidental memory exhaustion from unbounded input.

## Command Timeouts

All external commands (nix, systemctl, etc.) are executed with timeouts to prevent nixmate from hanging indefinitely on unresponsive processes.

## Responsible Disclosure

If you discover a security vulnerability in nixmate, please report it via [GitHub Issues](https://github.com/daskladas/nixmate/issues). Include as much detail as possible so we can reproduce and fix the issue quickly.

For sensitive vulnerabilities, please mark the issue as confidential or reach out to the maintainer directly before public disclosure.
