# Pipe Mode

nixmate can receive build output via stdin pipe. This lets you pipe failed builds directly into the Error Translator for instant analysis.

---

## Basic Usage

```bash
# Pipe a rebuild's error output:
sudo nixos-rebuild switch 2>&1 | nixmate

# Pipe a nix build:
nix build .#mypackage 2>&1 | nixmate

# Pipe a flake check:
nix flake check 2>&1 | nixmate
```

**What happens:** nixmate reads the piped text, opens directly in the Error Translator with the output pre-loaded and auto-analyzed. No need to manually copy-paste errors.

> **The `2>&1` part** redirects stderr to stdout. Nix sends most error messages to stderr, so without this, nixmate wouldn't see them.

---

## Useful Aliases

Add these to your `~/.bashrc`, `~/.zshrc`, or NixOS shell config:

```bash
# Rebuild with auto-error-analysis
alias nrb="sudo nixos-rebuild switch 2>&1 | nixmate"

# Test rebuild (doesn't switch, just checks for errors)
alias nrt="sudo nixos-rebuild test 2>&1 | nixmate"

# Build check
alias nrc="sudo nixos-rebuild build 2>&1 | nixmate"

# Flake update + rebuild
alias nfu="nix flake update && sudo nixos-rebuild switch 2>&1 | nixmate"
```

Then just type `nrb` and if anything fails, you're already in nixmate looking at the explanation.

---

## NixOS Shell Alias (declarative)

Add to your `configuration.nix`:

```nix
environment.shellAliases = {
  nrb = "sudo nixos-rebuild switch 2>&1 | nixmate";
  nrt = "sudo nixos-rebuild test 2>&1 | nixmate";
};
```

---

## How It Works Technically

1. nixmate checks if stdin is a terminal (`isatty`). If not → pipe mode.
2. Reads all of stdin (up to 1MB) before starting the TUI.
3. Reattaches stdin to `/dev/tty` so keyboard input works again.
4. Opens the Error Translator with the piped text pre-loaded.
5. Auto-runs pattern matching on the piped text.

This is the same approach used by `fzf`, `bat`, and `less`.

---

## Limitations

- **Max input size:** 1MB. More than enough for any build log.
- **Binary input:** nixmate expects UTF-8 text. Binary data is ignored.
- **No live streaming:** nixmate reads ALL input first, then starts. It doesn't show build progress in real-time (use the Rebuild Dashboard for that).
- **sudo password:** If the build command needs sudo, the password prompt happens BEFORE nixmate starts. This is normal.

---

## Combining with the Rebuild Dashboard

Pipe mode is great for quick error analysis after a failed build. For live monitoring during a build, use the Rebuild Dashboard instead (press `7` in nixmate). It shows real-time progress, phase tracking, and educational explanations as the build runs.

**Workflow:**
1. Use Rebuild Dashboard (`7`) for your normal rebuilds
2. If it fails, the error is already visible in the Log tab
3. Use pipe mode for builds you run outside nixmate (CI, scripts, etc.)
