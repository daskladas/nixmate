//! AI fallback analysis for unrecognized Nix errors.
//!
//! Supports Claude, OpenAI, and Ollama (local).
//! All calls are blocking — ALWAYS run in a background thread!
//! Uses `ureq` for HTTP with timeouts on every request.

use anyhow::{Context, Result};
use std::time::Duration;

const TIMEOUT_SECS: u64 = 60;

/// Run AI analysis against the given provider.
/// This blocks — caller MUST run in a background thread.
pub fn analyze_with_ai(
    provider: &str,
    api_key: &str,
    ollama_url: &str,
    ollama_model: &str,
    error_text: &str,
    lang: &str,
) -> Result<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build();

    let prompt = build_prompt(error_text, lang);

    match provider {
        "claude" => call_claude(&agent, api_key, &prompt),
        "openai" => call_openai(&agent, api_key, &prompt),
        "ollama" => call_ollama(&agent, ollama_url, ollama_model, &prompt),
        _ => anyhow::bail!("Unknown AI provider: {}", provider),
    }
}

/// Get a display-friendly name for the provider.
pub fn provider_display_name(provider: &str) -> &str {
    match provider {
        "claude" => "Claude",
        "openai" => "OpenAI",
        "ollama" => "Ollama",
        _ => provider,
    }
}

fn build_prompt(error_text: &str, lang: &str) -> String {
    let lang_instruction = match lang {
        "de" => "Antworte auf Deutsch.",
        _ => "Respond in English.",
    };

    format!(
        r#"You are a NixOS error analysis expert. Analyze the following Nix/NixOS error and provide:

1. **Problem**: What went wrong (1-2 sentences)
2. **Solution**: Concrete commands or configuration changes to fix it
3. **Explanation**: Why this error happens and how Nix works in this context

Be concise and practical. Focus on actionable solutions.
{lang_instruction}

Error:
```
{error_text}
```"#,
    )
}

// ═══════════════════════════════════════
//  CLAUDE
// ═══════════════════════════════════════

fn call_claude(agent: &ureq::Agent, api_key: &str, prompt: &str) -> Result<String> {
    let body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 2048,
        "messages": [{"role": "user", "content": prompt}]
    });

    let resp = agent
        .post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", api_key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_string(&serde_json::to_string(&body)?);

    match resp {
        Ok(resp) => {
            let json: serde_json::Value = serde_json::from_reader(resp.into_reader())
                .context("Failed to parse Claude response")?;
            json["content"][0]["text"]
                .as_str()
                .map(|s| s.to_string())
                .context("Unexpected Claude response format")
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            let msg: String = body.chars().take(200).collect();
            anyhow::bail!("Claude API error {}: {}", code, msg)
        }
        Err(ureq::Error::Transport(e)) => {
            anyhow::bail!("Network error (Claude): {}", e)
        }
    }
}

// ═══════════════════════════════════════
//  OPENAI
// ═══════════════════════════════════════

fn call_openai(agent: &ureq::Agent, api_key: &str, prompt: &str) -> Result<String> {
    let body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 2048
    });

    let resp = agent
        .post("https://api.openai.com/v1/chat/completions")
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("content-type", "application/json")
        .send_string(&serde_json::to_string(&body)?);

    match resp {
        Ok(resp) => {
            let json: serde_json::Value = serde_json::from_reader(resp.into_reader())
                .context("Failed to parse OpenAI response")?;
            json["choices"][0]["message"]["content"]
                .as_str()
                .map(|s| s.to_string())
                .context("Unexpected OpenAI response format")
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            let msg: String = body.chars().take(200).collect();
            anyhow::bail!("OpenAI API error {}: {}", code, msg)
        }
        Err(ureq::Error::Transport(e)) => {
            anyhow::bail!("Network error (OpenAI): {}", e)
        }
    }
}

// ═══════════════════════════════════════
//  OLLAMA (local)
// ═══════════════════════════════════════

fn call_ollama(agent: &ureq::Agent, base_url: &str, model: &str, prompt: &str) -> Result<String> {
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false
    });

    let resp = agent
        .post(&url)
        .set("content-type", "application/json")
        .send_string(&serde_json::to_string(&body)?);

    match resp {
        Ok(resp) => {
            let json: serde_json::Value = serde_json::from_reader(resp.into_reader())
                .context("Failed to parse Ollama response")?;
            json["response"]
                .as_str()
                .map(|s| s.to_string())
                .context("Unexpected Ollama response format")
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            let msg: String = body.chars().take(200).collect();
            anyhow::bail!("Ollama error {}: {}", code, msg)
        }
        Err(ureq::Error::Transport(e)) => {
            anyhow::bail!(
                "Ollama not reachable at {}\n\n\
                 Setup on NixOS:\n\
                 1. Add to configuration.nix:\n\
                    services.ollama.enable = true;\n\
                 2. sudo nixos-rebuild switch\n\
                 3. ollama pull {}\n\n\
                 Error: {}",
                base_url, model, e
            )
        }
    }
}
