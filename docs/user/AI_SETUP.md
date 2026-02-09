# AI Setup

nixmate's Error Translator can use AI to analyze errors that don't match any built-in pattern. Three providers are supported: **Ollama** (local, free), **Claude**, and **OpenAI**.

---

## Option 1: Ollama (recommended)

Runs locally on your machine. Free, private, no API key needed.

### Setup on NixOS

Add to your `configuration.nix`:

```nix
services.ollama.enable = true;
```

Rebuild:

```bash
sudo nixos-rebuild switch
```

Pull a model (llama3 works well for error analysis):

```bash
ollama pull llama3
```

### Configure in nixmate

Press `,` to open Settings, then set:

| Setting | Value |
|---------|-------|
| AI Enabled | `true` |
| AI Provider | `ollama` |
| Ollama URL | `http://localhost:11434` (default) |
| Ollama Model | `llama3` (default) |

Or edit `~/.config/nixmate/config.toml`:

```toml
ai_enabled = true
ai_provider = "ollama"
ollama_url = "http://localhost:11434"
ollama_model = "llama3"
```

### Verify it works

```bash
# Check Ollama is running:
curl http://localhost:11434/api/tags
```

Then in nixmate: go to Error Translator (`2`), paste any error, press `a` for AI analysis.

### Other models

Any Ollama model works. Larger models give better results but are slower:

```bash
ollama pull mistral        # 7B, fast
ollama pull llama3         # 8B, good balance (default)
ollama pull llama3:70b     # 70B, best results, needs 40GB+ RAM
ollama pull codellama      # code-focused, good for build errors
```

Change the model in Settings or config.toml:

```toml
ollama_model = "codellama"
```

---

## Option 2: Claude (Anthropic)

Best analysis quality. Requires an API key ($).

### Get an API key

1. Go to [console.anthropic.com](https://console.anthropic.com)
2. Sign up / log in
3. Go to **API Keys** → **Create Key**
4. Copy the key (starts with `sk-ant-api03-...`)

### Configure in nixmate

Press `,` → navigate to the AI settings:

| Setting | Value |
|---------|-------|
| AI Enabled | `true` |
| AI Provider | `claude` |
| AI API Key | `sk-ant-api03-...` |

Or in config.toml:

```toml
ai_enabled = true
ai_provider = "claude"
ai_api_key = "sk-ant-api03-..."
```

### Cost

nixmate uses Claude Sonnet (`claude-sonnet-4-20250514`). Each error analysis costs roughly $0.003–$0.01 depending on error length. Casual use costs cents per month.

---

## Option 3: OpenAI

Good analysis quality. Requires an API key ($).

### Get an API key

1. Go to [platform.openai.com](https://platform.openai.com)
2. Sign up / log in
3. Go to **API Keys** → **Create new secret key**
4. Copy the key (starts with `sk-...`)

### Configure in nixmate

| Setting | Value |
|---------|-------|
| AI Enabled | `true` |
| AI Provider | `openai` |
| AI API Key | `sk-...` |

Or in config.toml:

```toml
ai_enabled = true
ai_provider = "openai"
ai_api_key = "sk-..."
```

### Cost

nixmate uses GPT-4o-mini. Very cheap — pennies per analysis.

---

## Troubleshooting

**"AI disabled"** — AI is turned off. Go to Settings (`,`) and set AI Enabled to `true`.

**"No API key configured"** — You selected Claude or OpenAI but didn't set a key. Go to Settings → AI API Key → Enter → type your key → Enter.

**"Ollama not reachable"** — The Ollama service isn't running. Check:

```bash
# Is the service running?
systemctl status ollama

# Start it:
sudo systemctl start ollama

# Enable on boot:
# Add to configuration.nix: services.ollama.enable = true;
```

**"Ollama error 404: model not found"** — You need to pull the model first:

```bash
ollama pull llama3
```

**AI response is slow** — Ollama depends on your hardware. On CPU-only machines, expect 10–30 seconds. With a GPU, 2–5 seconds. Claude and OpenAI are typically 3–8 seconds.

**AI gives wrong advice** — AI analysis is a fallback for unrecognized errors. It's not always correct. Use it as a starting point, not as gospel. If you know the right fix, consider [adding a pattern](../developer/ADDING_PATTERNS.md) so the next person gets an instant, accurate answer.

---

## Privacy

- **Ollama:** Everything stays on your machine. Nothing leaves your network.
- **Claude / OpenAI:** Your error text is sent to the provider's API. Don't paste errors containing secrets, passwords, or private paths if that concerns you.
- **API keys** are stored in plaintext in `~/.config/nixmate/config.toml`. Set appropriate file permissions if you're on a shared machine:
  ```bash
  chmod 600 ~/.config/nixmate/config.toml
  ```
