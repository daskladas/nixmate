//! Configuration management for nixmate
//!
//! Single global config for the entire multitool.
//! No per-module settings duplication – theme, language, layout
//! are set once and apply everywhere.
//!
//! Config file location: ~/.config/nixmate/config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Main configuration structure (global for all modules)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeName,
    pub language: Language,
    pub layout: LayoutMode,

    // First-run welcome screen flag
    #[serde(default)]
    pub welcome_shown: bool,

    // Error Translator settings
    #[serde(default)]
    pub ai_enabled: bool,
    #[serde(default = "default_ai_provider")]
    pub ai_provider: String,
    #[serde(default)]
    pub ai_api_key: Option<String>,
    #[serde(default)]
    /// Reserved for future GitHub API integration (planned for v0.8)
    pub github_token: Option<String>,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: Option<String>,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: Option<String>,

    // Package Search settings
    #[serde(default = "default_nixpkgs_channel")]
    pub nixpkgs_channel: String,
}

fn default_ai_provider() -> String {
    "claude".to_string()
}

fn default_nixpkgs_channel() -> String {
    "auto".to_string()
}

fn default_ollama_url() -> Option<String> {
    Some("http://localhost:11434".to_string())
}

fn default_ollama_model() -> Option<String> {
    Some("llama3".to_string())
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeName::Gruvbox,
            language: Language::English,
            layout: LayoutMode::Auto,
            welcome_shown: false,
            ai_enabled: false,
            ai_provider: "claude".to_string(),
            ai_api_key: None,
            github_token: None,
            ollama_url: Some("http://localhost:11434".to_string()),
            ollama_model: Some("llama3".to_string()),
            nixpkgs_channel: "auto".to_string(),
        }
    }
}

impl Config {
    /// Get the config file path
    pub fn path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("nixmate");
        Ok(config_dir.join("config.toml"))
    }

    /// Load config from file, or create default if not exists
    pub fn load() -> Result<Self> {
        let path = Self::path()?;

        if !path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config from {:?}", path))
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write config to {:?}", path))?;

        // Restrict config file permissions (may contain API keys)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    /// Check if AI can be used (including Ollama which needs no key)
    pub fn ai_available(&self) -> bool {
        if !self.ai_enabled {
            return false;
        }
        match self.ai_provider.as_str() {
            "ollama" => true,
            _ => self.ai_api_key.as_ref().map_or(false, |k| !k.is_empty()),
        }
    }

    /// Check if GitHub is configured
    pub fn has_github(&self) -> bool {
        self.github_token.as_ref().map_or(false, |t| !t.is_empty())
    }
}

/// Available theme names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Gruvbox,
    Nord,
    Catppuccin,
    Dracula,
    TokyoNight,
    RosePine,
    Everforest,
    Kanagawa,
    SolarizedDark,
    OneDark,
    Monokai,
    Hacker,
    Transparent,
}

impl ThemeName {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeName::Gruvbox => "Gruvbox",
            ThemeName::Nord => "Nord",
            ThemeName::Catppuccin => "Catppuccin",
            ThemeName::Dracula => "Dracula",
            ThemeName::TokyoNight => "Tokyo Night",
            ThemeName::RosePine => "Rosé Pine",
            ThemeName::Everforest => "Everforest",
            ThemeName::Kanagawa => "Kanagawa",
            ThemeName::SolarizedDark => "Solarized Dark",
            ThemeName::OneDark => "One Dark",
            ThemeName::Monokai => "Monokai",
            ThemeName::Hacker => "Hacker",
            ThemeName::Transparent => "Transparent",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ThemeName::Gruvbox => ThemeName::Nord,
            ThemeName::Nord => ThemeName::Catppuccin,
            ThemeName::Catppuccin => ThemeName::Dracula,
            ThemeName::Dracula => ThemeName::TokyoNight,
            ThemeName::TokyoNight => ThemeName::RosePine,
            ThemeName::RosePine => ThemeName::Everforest,
            ThemeName::Everforest => ThemeName::Kanagawa,
            ThemeName::Kanagawa => ThemeName::SolarizedDark,
            ThemeName::SolarizedDark => ThemeName::OneDark,
            ThemeName::OneDark => ThemeName::Monokai,
            ThemeName::Monokai => ThemeName::Hacker,
            ThemeName::Hacker => ThemeName::Transparent,
            ThemeName::Transparent => ThemeName::Gruvbox,
        }
    }
}

/// Available languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    English,
    German,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::German => "Deutsch",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Language::English => Language::German,
            Language::German => Language::English,
        }
    }
}

/// Layout mode for the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    #[default]
    Auto,
    SideBySide,
    TabsOnly,
}

impl LayoutMode {
    pub fn as_str(&self, lang: Language) -> &'static str {
        match lang {
            Language::English => match self {
                LayoutMode::Auto => "Auto (responsive)",
                LayoutMode::SideBySide => "Side-by-side",
                LayoutMode::TabsOnly => "Tabs only",
            },
            Language::German => match self {
                LayoutMode::Auto => "Auto (responsiv)",
                LayoutMode::SideBySide => "Nebeneinander",
                LayoutMode::TabsOnly => "Nur Tabs",
            },
        }
    }

    pub fn next(&self) -> Self {
        match self {
            LayoutMode::Auto => LayoutMode::SideBySide,
            LayoutMode::SideBySide => LayoutMode::TabsOnly,
            LayoutMode::TabsOnly => LayoutMode::Auto,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme, ThemeName::Gruvbox);
        assert_eq!(config.language, Language::English);
        assert_eq!(config.layout, LayoutMode::Auto);
    }

    #[test]
    fn test_theme_cycle() {
        let theme = ThemeName::Gruvbox;
        assert_eq!(theme.next(), ThemeName::Nord);
        assert_eq!(theme.next().next(), ThemeName::Catppuccin);
        // Full cycle should return to start
        let mut t = ThemeName::Gruvbox;
        for _ in 0..13 {
            t = t.next();
        }
        assert_eq!(t, ThemeName::Gruvbox);
    }

    #[test]
    fn test_language_cycle() {
        let lang = Language::English;
        assert_eq!(lang.next(), Language::German);
        assert_eq!(lang.next().next(), Language::English);
    }

    #[test]
    fn test_ai_available_without_key() {
        let config = Config::default();
        assert!(!config.ai_available());
    }
}
