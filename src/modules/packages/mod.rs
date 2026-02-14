//! Package Search module
//!
//! Fast fuzzy-searchable nixpkgs browser.
//! Auto-detects flakes vs channels, configurable in Settings.
//! Shows package name, version, description, and installed status.
//! Fun loading messages while nix search runs.

use crate::config::Language;
use crate::i18n;
use crate::types::FlashMessage;
use crate::ui::theme::Theme;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::sync::mpsc;
use std::time::Instant;

// ── Package search result ──

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub attr: String,
    pub pname: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
}

/// Detected nixpkgs source
#[derive(Debug, Clone)]
pub struct NixpkgsSource {
    pub display_name: String,
    pub is_flakes: bool,
    pub channel: String,
}

/// Status messages sent from search thread
#[derive(Debug)]
pub enum SearchStatus {
    Phase(String),
    Done(Vec<SearchResult>),
    Error(String),
}

// ── Module state ──

pub struct PackagesState {
    // Search
    pub search_active: bool,
    pub search_query: String,
    pub last_query: String,

    // Results
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub scroll_offset: usize,

    // Detail view
    pub detail_open: bool,

    // Background search
    pub loading: bool,
    pub loading_start: Option<Instant>,
    pub loading_phase: String,
    pub loading_joke_idx: usize,
    pub last_joke_change: Option<Instant>,
    search_rx: Option<mpsc::Receiver<SearchStatus>>,

    // Nixpkgs source (auto-detected or configured)
    pub source: Option<NixpkgsSource>,
    pub source_detected: bool,

    // Installed packages cache
    installed_packages: Vec<String>,
    installed_loaded: bool,

    // Flash / error
    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
    pub error_message: Option<String>,
    pub config_path: Option<String>,
}

impl PackagesState {
    pub fn new() -> Self {
        Self {
            search_active: false,
            search_query: String::new(),
            last_query: String::new(),
            results: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            detail_open: false,
            loading: false,
            loading_start: None,
            loading_phase: String::new(),
            loading_joke_idx: 0,
            last_joke_change: None,
            search_rx: None,
            source: None,
            source_detected: false,
            installed_packages: Vec::new(),
            installed_loaded: false,
            lang: Language::English,
            flash_message: None,
            error_message: None,
            config_path: None,
        }
    }

    /// Detect nixpkgs source (call once on first visit)
    pub fn ensure_source_detected(&mut self, config_channel: &str) {
        if self.source_detected {
            return;
        }
        self.source_detected = true;

        if config_channel != "auto" {
            let is_flakes = config_channel.contains('/') || config_channel == "nixpkgs";
            self.source = Some(NixpkgsSource {
                display_name: config_channel.to_string(),
                is_flakes,
                channel: config_channel.to_string(),
            });
        } else {
            self.source = detect_nixpkgs_source(self.lang, self.config_path.as_deref());
        }
    }

    /// Load installed packages list (call once on first visit)
    pub fn ensure_installed_loaded(&mut self) {
        if self.installed_loaded {
            return;
        }
        self.installed_loaded = true;
        self.installed_packages = load_installed_packages();
    }

    /// Reset source detection (when settings change)
    pub fn reset_source(&mut self) {
        self.source_detected = false;
        self.source = None;
    }

    /// Start a background search
    fn start_search(&mut self) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.results.clear();
            self.error_message = None;
            return;
        }
        if query == self.last_query {
            return;
        }

        self.last_query = query.clone();
        self.loading = true;
        self.loading_start = Some(Instant::now());
        self.loading_phase = String::new();
        self.loading_joke_idx = 0;
        self.last_joke_change = Some(Instant::now());
        self.error_message = None;

        let installed = self.installed_packages.clone();
        let source = self.source.clone();
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        let lang = self.lang;

        std::thread::spawn(move || {
            run_search_with_status(&query, &installed, source.as_ref(), tx, lang);
        });
    }

    /// Poll for search results (non-blocking)
    pub fn poll_search(&mut self) {
        if self.loading {
            if let Some(last) = self.last_joke_change {
                if last.elapsed().as_secs() >= 8 {
                    self.loading_joke_idx += 1;
                    self.last_joke_change = Some(Instant::now());
                }
            }
        }

        if let Some(rx) = &self.search_rx {
            loop {
                match rx.try_recv() {
                    Ok(SearchStatus::Phase(msg)) => {
                        self.loading_phase = msg;
                    }
                    Ok(SearchStatus::Done(results)) => {
                        if results.is_empty() && !self.last_query.is_empty() {
                            self.error_message =
                                Some(crate::i18n::get_strings(self.lang).pkg_no_found.to_string());
                        }
                        self.results = results;
                        self.selected = 0;
                        self.scroll_offset = 0;
                        self.loading = false;
                        self.search_rx = None;
                        return;
                    }
                    Ok(SearchStatus::Error(msg)) => {
                        self.error_message = Some(msg);
                        self.loading = false;
                        self.search_rx = None;
                        return;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.loading = false;
                        self.search_rx = None;
                        if self.results.is_empty() {
                            self.error_message = Some(
                                crate::i18n::get_strings(self.lang)
                                    .pkg_search_failed
                                    .to_string(),
                            );
                        }
                        return;
                    }
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.detail_open {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                    self.detail_open = false;
                }
                _ => {}
            }
            return Ok(true);
        }

        if self.search_active {
            match key.code {
                KeyCode::Enter => {
                    self.search_active = false;
                    self.start_search();
                }
                KeyCode::Esc => {
                    self.search_active = false;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                }
                _ => {}
            }
            return Ok(true);
        }

        match key.code {
            KeyCode::Char('/') | KeyCode::Char('i') => {
                self.search_active = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1).min(self.results.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('g') => {
                self.selected = 0;
            }
            KeyCode::Char('G') => {
                if !self.results.is_empty() {
                    self.selected = self.results.len() - 1;
                }
            }
            KeyCode::Enter => {
                if !self.results.is_empty() {
                    self.detail_open = true;
                }
            }
            KeyCode::Char('n') => {
                self.search_query.clear();
                self.last_query.clear();
                self.search_active = true;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

// ── NixOS loading jokes ──

// Loading hints are now accessed via i18n (see pkg_hint_* in i18n.rs)

// ── Nixpkgs source detection ──

fn detect_nixpkgs_source(
    lang: crate::config::Language,
    config_path: Option<&str>,
) -> Option<NixpkgsSource> {
    let s = crate::i18n::get_strings(lang);
    let uses_flakes = crate::nix::detect::detect_system(config_path)
        .map(|s| s.uses_flakes)
        .unwrap_or(false);

    if uses_flakes {
        let channel = detect_flake_nixpkgs(config_path).unwrap_or_else(|| "nixpkgs".to_string());
        return Some(NixpkgsSource {
            display_name: s.pkg_source_flakes.replace("{}", &channel),
            is_flakes: true,
            channel,
        });
    }

    if let Some(channel) = detect_channel_name() {
        return Some(NixpkgsSource {
            display_name: s.pkg_source_channel.replace("{}", &channel),
            is_flakes: false,
            channel,
        });
    }

    Some(NixpkgsSource {
        display_name: s.pkg_source_nixpkgs.to_string(),
        is_flakes: true,
        channel: "nixpkgs".to_string(),
    })
}

fn detect_flake_nixpkgs(config_path: Option<&str>) -> Option<String> {
    use std::process::Command;

    // Find flake.nix location
    let home = std::env::var("HOME").unwrap_or_default();
    let mut flake_dirs: Vec<String> = Vec::new();
    if let Some(p) = config_path {
        flake_dirs.push(p.to_string());
    }
    flake_dirs.extend([
        "/etc/nixos".to_string(),
        format!("{}/.config/nixos", home),
        format!("{}/nixos", home),
        format!("{}/.nixos", home),
    ]);

    let flake_dir = flake_dirs
        .iter()
        .find(|d| std::path::Path::new(d).join("flake.nix").exists())?;

    let output = Command::new("nix")
        .args(["flake", "metadata", "--json"])
        .current_dir(flake_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data: serde_json::Value = serde_json::from_str(&stdout).ok()?;

    // Check locks -> nodes -> nixpkgs -> original -> ref (branch name)
    if let Some(ref_str) = data
        .pointer("/locks/nodes/nixpkgs/original/ref")
        .and_then(|v| v.as_str())
    {
        return Some(ref_str.to_string());
    }

    // Fallback: check locked -> narHash exists = it's locked nixpkgs
    if data.pointer("/locks/nodes/nixpkgs").is_some() {
        return Some("nixpkgs".to_string());
    }

    None
}

fn detect_channel_name() -> Option<String> {
    use std::process::Command;

    // Try nix-channel --list
    if let Ok(output) = Command::new("nix-channel").args(["--list"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Some(name) = parts[1].rsplit('/').next() {
                        if name.starts_with("nixos-") || name.starts_with("nixpkgs-") {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fallback: /etc/os-release VERSION_ID
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(version) = line.strip_prefix("VERSION_ID=") {
                let version = version.trim_matches('"');
                return Some(format!("nixos-{}", version));
            }
        }
    }

    None
}

// ── Search with status messages ──

fn run_search_with_status(
    query: &str,
    installed: &[String],
    source: Option<&NixpkgsSource>,
    tx: mpsc::Sender<SearchStatus>,
    lang: Language,
) {
    let s = crate::i18n::get_strings(lang);
    let is_flakes = source.map(|s| s.is_flakes).unwrap_or(true);
    let channel = source.map(|s| s.channel.as_str()).unwrap_or("nixpkgs");

    let _ = tx.send(SearchStatus::Phase(s.pkg_source_label.replace(
        "{}",
        source.map(|s| s.display_name.as_str()).unwrap_or("nixpkgs"),
    )));

    std::thread::sleep(std::time::Duration::from_millis(200));

    let _ = tx.send(SearchStatus::Phase(
        s.pkg_searching_for.replace("{}", query),
    ));

    if is_flakes {
        match try_nix_search_flakes(query, installed, channel) {
            Some(results) => {
                let _ = tx.send(SearchStatus::Done(results));
            }
            None => {
                let _ = tx.send(SearchStatus::Phase(s.pkg_trying_alt.to_string()));
                match try_nix_env_search(query, installed) {
                    Some(results) => {
                        let _ = tx.send(SearchStatus::Done(results));
                    }
                    None => {
                        let _ = tx.send(SearchStatus::Error(s.pkg_search_fail_nix.to_string()));
                    }
                }
            }
        }
    } else {
        match try_nix_env_search(query, installed) {
            Some(results) => {
                let _ = tx.send(SearchStatus::Done(results));
            }
            None => {
                let _ = tx.send(SearchStatus::Phase(s.pkg_trying_alt.to_string()));
                match try_nix_search_flakes(query, installed, "nixpkgs") {
                    Some(results) => {
                        let _ = tx.send(SearchStatus::Done(results));
                    }
                    None => {
                        let _ = tx.send(SearchStatus::Error(s.pkg_search_fail_nix.to_string()));
                    }
                }
            }
        }
    }
}

fn try_nix_search_flakes(
    query: &str,
    installed: &[String],
    channel: &str,
) -> Option<Vec<SearchResult>> {
    use std::process::Command;

    let output = Command::new("nix")
        .args(["search", channel, query, "--json"])
        .env("NIX_CONFIG", "warn-dirty = false")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() || stdout.trim() == "{}" {
        return Some(Vec::new());
    }

    let data: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    let obj = data.as_object()?;

    let mut results: Vec<SearchResult> = obj
        .iter()
        .map(|(attr_path, info)| {
            let pname = info
                .get("pname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let version = info
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let description = info
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let attr = attr_path
                .rsplit('.')
                .next()
                .unwrap_or(attr_path)
                .to_string();
            let is_installed = installed.iter().any(|p| p == &pname || p == &attr);

            SearchResult {
                attr,
                pname,
                version,
                description,
                installed: is_installed,
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.installed
            .cmp(&a.installed)
            .then_with(|| a.pname.to_lowercase().cmp(&b.pname.to_lowercase()))
    });
    results.truncate(200);
    Some(results)
}

fn try_nix_env_search(query: &str, installed: &[String]) -> Option<Vec<SearchResult>> {
    use std::process::Command;

    let output = Command::new("nix-env")
        .args(["-qaP", "--description"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let query_lower = query.to_lowercase();

    let mut results: Vec<SearchResult> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, char::is_whitespace).collect();
            if parts.len() < 2 {
                return None;
            }

            let attr_full = parts[0];
            let name_version = parts[1].trim();
            let description = if parts.len() > 2 {
                parts[2].trim().to_string()
            } else {
                String::new()
            };

            let matches = attr_full.to_lowercase().contains(&query_lower)
                || name_version.to_lowercase().contains(&query_lower)
                || description.to_lowercase().contains(&query_lower);
            if !matches {
                return None;
            }

            let attr = attr_full
                .rsplit('.')
                .next()
                .unwrap_or(attr_full)
                .to_string();
            let (pname, version) = if let Some(pos) = name_version.rfind('-') {
                (
                    name_version[..pos].to_string(),
                    name_version[pos + 1..].to_string(),
                )
            } else {
                (name_version.to_string(), String::new())
            };

            let is_installed = installed.iter().any(|p| p == &pname || p == &attr);
            Some(SearchResult {
                attr,
                pname,
                version,
                description,
                installed: is_installed,
            })
        })
        .collect();

    results.sort_by(|a, b| {
        b.installed
            .cmp(&a.installed)
            .then_with(|| a.pname.to_lowercase().cmp(&b.pname.to_lowercase()))
    });
    results.truncate(200);
    Some(results)
}

fn load_installed_packages() -> Vec<String> {
    use std::process::Command;

    let path = std::path::Path::new("/run/current-system");
    if path.exists() {
        if let Ok(pkgs) = crate::nix::packages::get_packages(path) {
            return pkgs.into_iter().map(|p| p.name).collect();
        }
    }

    if let Ok(output) = Command::new("nix-env").args(["-q"]).output() {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|l| {
                    if let Some(pos) = l.rfind('-') {
                        if l[pos + 1..]
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_digit())
                        {
                            return l[..pos].to_string();
                        }
                    }
                    l.to_string()
                })
                .collect();
        }
    }
    Vec::new()
}

// ── Rendering ──

pub fn render(frame: &mut Frame, state: &PackagesState, theme: &Theme, lang: Language, area: Rect) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_packages))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(1), // Source
        Constraint::Length(2), // Search bar
        Constraint::Min(4),    // Results or loading
    ])
    .split(inner);

    render_source_line(frame, state, theme, chunks[0]);
    render_search_bar(frame, state, theme, lang, chunks[1]);

    if state.loading {
        render_loading(frame, state, theme, chunks[2]);
    } else if state.detail_open && !state.results.is_empty() {
        render_detail(frame, state, theme, lang, chunks[2]);
    } else {
        render_results(frame, state, theme, lang, chunks[2]);
    }
}

fn render_source_line(frame: &mut Frame, state: &PackagesState, theme: &Theme, area: Rect) {
    let source_text = if let Some(src) = &state.source {
        format!("  📦 {}", src.display_name)
    } else {
        "  📦 Detecting...".to_string()
    };

    frame.render_widget(
        Paragraph::new(Line::styled(source_text, Style::default().fg(theme.fg_dim)))
            .style(theme.block_style()),
        area,
    );
}

fn render_search_bar(
    frame: &mut Frame,
    state: &PackagesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let cursor_char = if state.search_active { "│" } else { "" };
    let query_display = if state.search_query.is_empty() && !state.search_active {
        s.pkg_search_hint.to_string()
    } else {
        format!("{}{}", state.search_query, cursor_char)
    };

    let search_style = if state.search_active {
        Style::default().fg(theme.accent)
    } else if state.search_query.is_empty() {
        Style::default().fg(theme.fg_dim)
    } else {
        theme.text()
    };

    let line = Line::from(vec![
        Span::styled(
            format!("  {} ", s.pkg_search_label),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(query_display, search_style),
    ]);

    frame.render_widget(Paragraph::new(line).style(theme.block_style()), area);

    // Result count on the right
    if !state.results.is_empty() {
        let count_text = format!("{} {} ", state.results.len(), s.pkg_results);
        if area.width > count_text.len() as u16 + 2 {
            let count_area = Rect {
                x: area.x + area.width - count_text.len() as u16 - 1,
                y: area.y,
                width: count_text.len() as u16 + 1,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(Line::styled(count_text, Style::default().fg(theme.fg_dim))),
                count_area,
            );
        }
    }
}

fn render_loading(frame: &mut Frame, state: &PackagesState, theme: &Theme, area: Rect) {
    let s = crate::i18n::get_strings(state.lang);
    let elapsed = state
        .loading_start
        .map(|s| s.elapsed().as_secs())
        .unwrap_or(0);
    let hints = [
        s.pkg_hint_1,
        s.pkg_hint_2,
        s.pkg_hint_3,
        s.pkg_hint_4,
        s.pkg_hint_5,
    ];
    let hint = hints[state.loading_joke_idx % hints.len()];

    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_idx = (elapsed as usize * 3 + state.loading_joke_idx) % spinner_frames.len();
    let spinner = spinner_frames[spinner_idx];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));
    lines.push(Line::raw(""));

    // Spinner + searching + elapsed on same line
    lines.push(Line::styled(
        format!("  {}  {} ({}s)", spinner, s.loading, elapsed),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    // Phase message
    if !state.loading_phase.is_empty() {
        lines.push(Line::styled(
            format!("  ✓ {}", state.loading_phase),
            Style::default().fg(theme.success),
        ));
        lines.push(Line::raw(""));
    }

    lines.push(Line::raw(""));

    lines.push(Line::styled(
        format!("  {}", hint),
        Style::default().fg(theme.fg_dim),
    ));

    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Left)
            .style(theme.block_style()),
        area,
    );
}

fn render_results(
    frame: &mut Frame,
    state: &PackagesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if state.results.is_empty() {
        let msg = if let Some(err) = &state.error_message {
            err.clone()
        } else if state.last_query.is_empty() {
            s.pkg_empty_hint.to_string()
        } else {
            s.pkg_no_results.to_string()
        };

        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(""),
                Line::raw(""),
                Line::styled(msg, Style::default().fg(theme.fg_dim)),
            ])
            .alignment(Alignment::Center)
            .style(theme.block_style()),
            area,
        );
        return;
    }

    let visible_height = area.height as usize;
    let mut scroll = state.scroll_offset;
    if state.selected >= scroll + visible_height {
        scroll = state.selected + 1 - visible_height;
    }
    if state.selected < scroll {
        scroll = state.selected;
    }

    let name_width = 28usize.min(area.width as usize / 3);
    let version_width = 14usize.min(area.width as usize / 5);

    let items: Vec<ListItem> = state
        .results
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, pkg)| {
            let is_selected = i == state.selected;
            let installed_marker = if pkg.installed { "✓ " } else { "  " };
            let name = if pkg.attr != pkg.pname && !pkg.attr.is_empty() {
                format!("{:<width$}", pkg.attr, width = name_width)
            } else {
                format!("{:<width$}", pkg.pname, width = name_width)
            };
            let version = format!("{:<width$}", pkg.version, width = version_width);

            let desc_width = (area.width as usize).saturating_sub(name_width + version_width + 6);
            let description: String = if pkg.description.len() > desc_width {
                format!("{}…", &pkg.description[..desc_width.saturating_sub(1)])
            } else {
                pkg.description.clone()
            };

            let style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };
            let installed_style = if pkg.installed {
                Style::default().fg(theme.success)
            } else {
                Style::default().fg(theme.fg_dim)
            };

            ListItem::new(Line::from(vec![
                Span::styled(installed_marker.to_string(), installed_style),
                Span::styled(
                    name,
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.accent)
                    },
                ),
                Span::styled(format!(" {} ", version), style),
                Span::styled(
                    description,
                    if is_selected {
                        style
                    } else {
                        Style::default().fg(theme.fg_dim)
                    },
                ),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items).style(theme.block_style()), area);
}

fn render_detail(
    frame: &mut Frame,
    state: &PackagesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let Some(pkg) = state.results.get(state.selected) else {
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));

    let fields: Vec<(&str, String, bool)> = vec![
        (s.pkg_detail_name, pkg.pname.clone(), true),
        (s.pkg_detail_attr, pkg.attr.clone(), false),
        (s.pkg_detail_version, pkg.version.clone(), false),
    ];

    for (label, value, bold) in fields {
        let val_style = if bold {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            theme.text()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", label), Style::default().fg(theme.fg_dim)),
            Span::styled(value, val_style),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled(
            format!("  {} ", s.pkg_detail_status),
            Style::default().fg(theme.fg_dim),
        ),
        if pkg.installed {
            Span::styled(
                s.pkg_installed,
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(s.pkg_not_installed, Style::default().fg(theme.fg_dim))
        },
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  {} ", s.pkg_detail_desc),
        Style::default().fg(theme.fg_dim),
    ));
    lines.push(Line::raw(""));

    let wrap_width = (area.width as usize).saturating_sub(6);
    for chunk in pkg.description.as_bytes().chunks(wrap_width.max(1)) {
        if let Ok(text) = std::str::from_utf8(chunk) {
            lines.push(Line::styled(format!("    {}", text), theme.text()));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  {}", s.pkg_install_hint),
        Style::default().fg(theme.fg_dim),
    ));
    lines.push(Line::styled(
        format!("  nix-env -iA nixpkgs.{}", pkg.attr),
        Style::default().fg(theme.accent),
    ));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  [Esc/Enter] {}", s.back),
        Style::default().fg(theme.fg_dim),
    ));

    frame.render_widget(Paragraph::new(lines).style(theme.block_style()), area);
}
