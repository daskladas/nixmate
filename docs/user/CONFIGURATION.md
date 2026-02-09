# Configuration

nixmate stores its configuration at:

```
~/.config/nixmate/config.toml
```

This file is created automatically on first run. You can edit it manually or change everything through the Settings tab (press `,` in nixmate).

---

## All Options

Here's a complete `config.toml` with every option explained:

```toml
# ── Appearance ──

# Color theme. Options: gruvbox, nord, catppuccin, dracula, tokyonight,
# rosepine, everforest, kanagawa, solarizeddark, onedark, monokai, hacker,
# transparent
theme = "gruvbox"

# Language. Options: english, german
language = "english"

# Layout mode.
#   auto       = sidebar on wide terminals, tabs on narrow
#   sidebyside = always show sidebar
#   tabsonly   = never show sidebar
layout = "auto"

# ── First-run ──

# Set to true after dismissing the welcome screen.
# Set to false to see the welcome screen again.
welcome_shown = true

# ── Package Search ──

# Which nixpkgs source to use for package search.
#   auto     = auto-detect (checks flake.lock, then channels)
#   Or a specific channel: nixos-unstable, nixos-24.11, etc.
nixpkgs_channel = "auto"

# ── AI Error Analysis ──

# Enable AI fallback in the Error Translator.
# When a pattern doesn't match, send the error to an AI for analysis.
ai_enabled = false

# AI provider. Options: claude, openai, ollama
ai_provider = "claude"

# API key for Claude or OpenAI. Not needed for Ollama.
# You can also set this in the Settings tab (press , → navigate to API Key → Enter).
ai_api_key = "sk-ant-..."

# Ollama settings (only used when ai_provider = "ollama")
ollama_url = "http://localhost:11434"
ollama_model = "llama3"

# ── GitHub ──

# GitHub personal access token. Used for higher API rate limits
# when checking flake inputs. Optional.
github_token = "ghp_..."
```

---

## Themes

nixmate ships with 13 themes. Change via Settings (`,`) or edit `theme` in config.toml:

| Name | Style |
|------|-------|
| `gruvbox` | Warm retro (default) |
| `nord` | Cool Arctic blue |
| `catppuccin` | Pastel mocha |
| `dracula` | Purple/pink dark |
| `tokyonight` | Deep blue neon |
| `rosepine` | Muted rose tones |
| `everforest` | Soft green forest |
| `kanagawa` | Japanese wave tones |
| `solarizeddark` | Ethan Schoonover's classic |
| `onedark` | Atom editor style |
| `monokai` | Sublime Text style |
| `hacker` | Matrix green on black |
| `transparent` | Uses your terminal's background |

> **Tip:** The `transparent` theme works great with terminals that have blur or background images — nixmate becomes see-through.

---

## Language

nixmate is fully localized in English and German. Every string, every module, every intro page.

Change via Settings or:

```toml
language = "german"
```

---

## Nixpkgs Channel

The Package Search module needs to know where your packages come from. By default it auto-detects:

1. Checks `flake.lock` for a nixpkgs input → uses that
2. Falls back to `nix-channel --list` → uses the first NixOS channel
3. Falls back to `nixos-unstable`

If auto-detection picks wrong, override it:

```toml
nixpkgs_channel = "nixos-24.11"
```

---

## AI Setup

See [AI_SETUP.md](AI_SETUP.md) for detailed setup instructions for each provider.

**Quick version:**

```toml
# For Claude:
ai_enabled = true
ai_provider = "claude"
ai_api_key = "sk-ant-api03-..."

# For OpenAI:
ai_enabled = true
ai_provider = "openai"
ai_api_key = "sk-..."

# For Ollama (local, free, no key needed):
ai_enabled = true
ai_provider = "ollama"
ollama_url = "http://localhost:11434"
ollama_model = "llama3"
```

---

## Config Location

The config directory is:

```
~/.config/nixmate/
├── config.toml           # Main config
└── rebuild_history.json  # Rebuild Dashboard history (auto-generated)
```

If you ever need a fresh start:

```bash
rm -rf ~/.config/nixmate
```

nixmate will recreate defaults on next launch.
