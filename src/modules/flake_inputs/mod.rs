//! Flake Input Manager module
//!
//! Manage, update, and inspect flake inputs individually.
//! Sub-tabs:
//!   Overview — all inputs with revision, age, status
//!   Update   — selective per-input updates with checkboxes
//!   History  — diff of last update (old vs new revisions)
//!   Details  — full info for the selected input
//!
//! Data source: flake.lock (JSON) + flake.nix parsing.
//! Updates via `nix flake lock --update-input <name>`.

use crate::config::Language;
use crate::i18n;
use crate::types::FlashMessage;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};
use std::collections::HashMap;
use std::sync::mpsc;

// ── Sub-tabs ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlakeSubTab {
    #[default]
    Overview,
    Update,
    History,
    Details,
}

impl FlakeSubTab {
    pub fn all() -> &'static [FlakeSubTab] {
        &[
            FlakeSubTab::Overview,
            FlakeSubTab::Update,
            FlakeSubTab::History,
            FlakeSubTab::Details,
        ]
    }

    pub fn index(&self) -> usize {
        match self {
            FlakeSubTab::Overview => 0,
            FlakeSubTab::Update => 1,
            FlakeSubTab::History => 2,
            FlakeSubTab::Details => 3,
        }
    }

    pub fn next(&self) -> Self {
        let tabs = Self::all();
        let idx = (self.index() + 1) % tabs.len();
        tabs[idx]
    }

    pub fn prev(&self) -> Self {
        let tabs = Self::all();
        let idx = if self.index() == 0 {
            tabs.len() - 1
        } else {
            self.index() - 1
        };
        tabs[idx]
    }
}

// ── Flake input data ──

#[derive(Debug, Clone)]
pub struct FlakeInput {
    pub name: String,
    pub input_type: String, // github, git, path, indirect, etc.
    pub url: String,        // display URL (e.g. "github:NixOS/nixpkgs")
    #[allow(dead_code)] // Parsed from flake.lock, reserved for detail view
    pub owner: String,
    #[allow(dead_code)] // Parsed from flake.lock, reserved for detail view
    pub repo: String,
    pub branch: String,    // ref/branch if set
    pub revision: String,  // full rev hash
    pub rev_short: String, // first 7 chars
    pub nar_hash: String,
    pub last_modified: i64, // unix timestamp
    pub age_text: String,   // "3 days ago", "2 months ago"
    pub age_days: u64,
    pub follows: Vec<String>, // what this input's sub-inputs follow
    #[allow(dead_code)] // Parsed from flake.lock, reserved for detail view
    pub is_indirect: bool, // flake registry reference
}

// ── Update result ──

#[derive(Debug)]
pub struct UpdateResult {
    pub input_name: String,
    pub old_rev: String,
    pub new_rev: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug)]
pub enum UpdateStatus {
    Progress(String),
    InputDone(UpdateResult),
    AllDone,
    #[allow(dead_code)] // Reserved for granular error reporting
    Error(String),
}

// ── Popup state ──

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlakePopup {
    None,
    ConfirmUpdate,
    Updating,
}

// ── Module state ──

pub struct FlakeInputsState {
    pub sub_tab: FlakeSubTab,

    // Data
    pub inputs: Vec<FlakeInput>,
    pub flake_path: Option<String>,
    pub loaded: bool,
    pub loading: bool,
    pub error_message: Option<String>,
    load_rx: Option<mpsc::Receiver<LoadResult>>,

    // Overview tab
    pub selected: usize,
    pub scroll_offset: usize,

    // Update tab
    pub update_checked: Vec<bool>,
    pub update_selected: usize,
    pub update_scroll: usize,
    pub popup: FlakePopup,

    // Update process
    pub updating: bool,
    pub update_log: Vec<String>,
    pub update_results: Vec<UpdateResult>,
    update_rx: Option<mpsc::Receiver<UpdateStatus>>,

    // History (diffs from last update)
    pub history: Vec<UpdateResult>,
    pub history_selected: usize,
    pub history_scroll: usize,

    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

#[derive(Debug)]
enum LoadResult {
    Done {
        inputs: Vec<FlakeInput>,
        flake_path: String,
    },
    Error(String),
}

impl FlakeInputsState {
    pub fn new() -> Self {
        Self {
            sub_tab: FlakeSubTab::Overview,
            inputs: Vec::new(),
            flake_path: None,
            loaded: false,
            loading: false,
            error_message: None,
            load_rx: None,
            selected: 0,
            scroll_offset: 0,
            update_checked: Vec::new(),
            update_selected: 0,
            update_scroll: 0,
            popup: FlakePopup::None,
            updating: false,
            update_log: Vec::new(),
            update_results: Vec::new(),
            update_rx: None,
            history: Vec::new(),
            history_selected: 0,
            history_scroll: 0,
            lang: Language::English,
            flash_message: None,
        }
    }

    /// Lazy load on first tab visit
    pub fn ensure_loaded(&mut self) {
        if self.loaded || self.loading {
            return;
        }
        self.loading = true;
        self.error_message = None;

        let (tx, rx) = mpsc::channel();
        self.load_rx = Some(rx);
        let lang = self.lang;

        std::thread::spawn(move || {
            let result = load_flake_inputs(lang);
            let _ = tx.send(result);
        });
    }

    /// Poll background loaders
    pub fn poll_load(&mut self) {
        // Poll initial load
        if let Some(rx) = &self.load_rx {
            match rx.try_recv() {
                Ok(LoadResult::Done { inputs, flake_path }) => {
                    self.update_checked = vec![false; inputs.len()];
                    self.inputs = inputs;
                    self.flake_path = Some(flake_path);
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
                Ok(LoadResult::Error(msg)) => {
                    self.error_message = Some(msg);
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.loaded = true;
                    self.load_rx = None;
                    if self.inputs.is_empty() && self.error_message.is_none() {
                        self.error_message = Some(
                            crate::i18n::get_strings(self.lang)
                                .fi_error_load_failed
                                .to_string(),
                        );
                    }
                }
            }
        }

        // Poll update process
        if let Some(rx) = &self.update_rx {
            loop {
                match rx.try_recv() {
                    Ok(UpdateStatus::Progress(msg)) => {
                        self.update_log.push(msg);
                    }
                    Ok(UpdateStatus::InputDone(result)) => {
                        self.update_log.push(format!(
                            "{}: {} → {}",
                            result.input_name, result.old_rev, result.new_rev
                        ));
                        self.history.push(UpdateResult {
                            input_name: result.input_name.clone(),
                            old_rev: result.old_rev.clone(),
                            new_rev: result.new_rev.clone(),
                            success: result.success,
                            message: result.message.clone(),
                        });
                        self.update_results.push(result);
                    }
                    Ok(UpdateStatus::AllDone) => {
                        self.updating = false;
                        self.popup = FlakePopup::None;
                        self.update_rx = None;
                        // Reload inputs to get fresh data
                        self.loaded = false;
                        self.loading = false;
                        self.ensure_loaded();
                        let s = crate::i18n::get_strings(self.lang);
                        self.flash_message = Some(FlashMessage::new(
                            format!("{} {}", self.update_results.len(), s.flk_inputs_updated),
                            true,
                        ));
                        return;
                    }
                    Ok(UpdateStatus::Error(msg)) => {
                        self.updating = false;
                        self.popup = FlakePopup::None;
                        self.update_rx = None;
                        self.flash_message = Some(FlashMessage::new(msg, false));
                        return;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.updating = false;
                        self.popup = FlakePopup::None;
                        self.update_rx = None;
                        return;
                    }
                }
            }
        }
    }

    /// Start updating selected inputs
    fn start_update(&mut self) {
        let flake_path = match &self.flake_path {
            Some(p) => p.clone(),
            None => return,
        };

        // Collect selected input names and their current revisions
        let selected: Vec<(String, String)> = self
            .inputs
            .iter()
            .enumerate()
            .filter(|(i, _)| self.update_checked.get(*i).copied().unwrap_or(false))
            .map(|(_, input)| (input.name.clone(), input.rev_short.clone()))
            .collect();

        if selected.is_empty() {
            return;
        }

        self.updating = true;
        self.popup = FlakePopup::Updating;
        self.update_log.clear();
        self.update_results.clear();

        let (tx, rx) = mpsc::channel();
        self.update_rx = Some(rx);
        let lang = self.lang;

        std::thread::spawn(move || {
            run_selective_update(&flake_path, &selected, tx, lang);
        });
    }

    /// Reload flake data
    fn reload(&mut self) {
        self.loaded = false;
        self.loading = false;
        self.load_rx = None;
        self.inputs.clear();
        self.update_checked.clear();
        self.error_message = None;
        self.ensure_loaded();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Popup handling
        match &self.popup {
            FlakePopup::ConfirmUpdate => {
                match key.code {
                    KeyCode::Enter | KeyCode::Char('y') => {
                        self.start_update();
                    }
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => {
                        self.popup = FlakePopup::None;
                    }
                    _ => {}
                }
                return Ok(true);
            }
            FlakePopup::Updating => {
                // Absorb all keys while updating
                return Ok(true);
            }
            FlakePopup::None => {}
        }

        // Sub-tab switching with [ / ]
        match key.code {
            KeyCode::Char('[') => {
                self.sub_tab = self.sub_tab.prev();
                return Ok(true);
            }
            KeyCode::Char(']') => {
                self.sub_tab = self.sub_tab.next();
                return Ok(true);
            }
            _ => {}
        }

        match self.sub_tab {
            FlakeSubTab::Overview => self.handle_overview_key(key),
            FlakeSubTab::Update => self.handle_update_key(key),
            FlakeSubTab::History => self.handle_history_key(key),
            FlakeSubTab::Details => self.handle_details_key(key),
        }
    }

    fn handle_overview_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.inputs.is_empty() {
                    self.selected = (self.selected + 1).min(self.inputs.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('g') => self.selected = 0,
            KeyCode::Char('G') => {
                if !self.inputs.is_empty() {
                    self.selected = self.inputs.len() - 1;
                }
            }
            KeyCode::Enter => {
                // Switch to details for selected input
                self.sub_tab = FlakeSubTab::Details;
            }
            KeyCode::Char('r') => {
                self.reload();
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn handle_update_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.inputs.is_empty() {
                    self.update_selected = (self.update_selected + 1).min(self.inputs.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.update_selected > 0 {
                    self.update_selected -= 1;
                }
            }
            KeyCode::Char(' ') => {
                // Toggle checkbox
                if self.update_selected < self.update_checked.len() {
                    let val = self.update_checked[self.update_selected];
                    self.update_checked[self.update_selected] = !val;
                }
            }
            KeyCode::Char('a') => {
                // Select all
                for v in &mut self.update_checked {
                    *v = true;
                }
            }
            KeyCode::Char('n') => {
                // Select none
                for v in &mut self.update_checked {
                    *v = false;
                }
            }
            KeyCode::Enter => {
                let any_selected = self.update_checked.iter().any(|&v| v);
                if any_selected {
                    self.popup = FlakePopup::ConfirmUpdate;
                }
            }
            KeyCode::Char('g') => self.update_selected = 0,
            KeyCode::Char('G') => {
                if !self.inputs.is_empty() {
                    self.update_selected = self.inputs.len() - 1;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn handle_history_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.history.is_empty() {
                    self.history_selected = (self.history_selected + 1).min(self.history.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.history_selected > 0 {
                    self.history_selected -= 1;
                }
            }
            KeyCode::Char('g') => self.history_selected = 0,
            KeyCode::Char('G') => {
                if !self.history.is_empty() {
                    self.history_selected = self.history.len() - 1;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn handle_details_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.inputs.is_empty() {
                    self.selected = (self.selected + 1).min(self.inputs.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('r') => {
                self.reload();
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

// ── Data loading ──

fn find_flake_dir() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        "/etc/nixos".to_string(),
        format!("{}/.config/nixos", home),
        format!("{}/nixos", home),
        format!("{}/.nixos", home),
    ];

    for dir in &candidates {
        let flake_nix = format!("{}/flake.nix", dir);
        let flake_lock = format!("{}/flake.lock", dir);
        if std::path::Path::new(&flake_nix).exists() && std::path::Path::new(&flake_lock).exists() {
            return Some(dir.clone());
        }
    }

    // Also check: flake.nix exists but no lock yet
    for dir in &candidates {
        let flake_nix = format!("{}/flake.nix", dir);
        if std::path::Path::new(&flake_nix).exists() {
            return Some(dir.clone());
        }
    }

    None
}

fn load_flake_inputs(lang: Language) -> LoadResult {
    let s = crate::i18n::get_strings(lang);
    let flake_dir = match find_flake_dir() {
        Some(d) => d,
        None => {
            return LoadResult::Error(s.flk_no_flake.to_string());
        }
    };

    let lock_path = format!("{}/flake.lock", flake_dir);
    let lock_content = match std::fs::read_to_string(&lock_path) {
        Ok(c) => c,
        Err(_) => {
            return LoadResult::Error(format!("{} ({})", s.flk_no_lock, lock_path));
        }
    };

    let lock_json: serde_json::Value = match serde_json::from_str(&lock_content) {
        Ok(v) => v,
        Err(e) => {
            return LoadResult::Error(s.fi_error_parse_failed.replace("{}", &e.to_string()));
        }
    };

    let inputs = parse_flake_lock(&lock_json);

    if inputs.is_empty() {
        return LoadResult::Error(s.fi_error_no_inputs.to_string());
    }

    LoadResult::Done {
        inputs,
        flake_path: flake_dir,
    }
}

fn parse_flake_lock(lock: &serde_json::Value) -> Vec<FlakeInput> {
    let nodes = match lock.get("nodes").and_then(|n| n.as_object()) {
        Some(n) => n,
        None => return Vec::new(),
    };

    // Find the root node to get direct input names
    let root_name = lock.get("root").and_then(|r| r.as_str()).unwrap_or("root");

    let root_inputs = nodes
        .get(root_name)
        .and_then(|n| n.get("inputs"))
        .and_then(|i| i.as_object());

    let direct_input_names: HashMap<String, String> = match root_inputs {
        Some(inputs) => inputs
            .iter()
            .filter_map(|(name, target)| {
                let target_name = if let Some(s) = target.as_str() {
                    s.to_string()
                } else if let Some(arr) = target.as_array() {
                    // Follows syntax: ["nixpkgs"]
                    arr.first()?.as_str()?.to_string()
                } else {
                    return None;
                };
                Some((name.clone(), target_name))
            })
            .collect(),
        None => HashMap::new(),
    };

    let now = chrono::Local::now().timestamp();

    let mut inputs: Vec<FlakeInput> = direct_input_names
        .iter()
        .filter_map(|(display_name, node_name)| {
            let node = nodes.get(node_name)?;
            let locked = node.get("locked")?;

            let input_type = locked
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let owner = locked
                .get("owner")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let repo = locked
                .get("repo")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let revision = locked
                .get("rev")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let rev_short = if revision.len() >= 7 {
                revision[..7].to_string()
            } else {
                revision.clone()
            };

            let nar_hash = locked
                .get("narHash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let last_modified = locked
                .get("lastModified")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let branch = node
                .get("original")
                .and_then(|o| o.get("ref"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Build display URL
            let url = match input_type.as_str() {
                "github" => {
                    if branch.is_empty() {
                        format!("github:{}/{}", owner, repo)
                    } else {
                        format!("github:{}/{}/{}", owner, repo, branch)
                    }
                }
                "git" => locked
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("git:unknown")
                    .to_string(),
                "path" => locked
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("path:unknown")
                    .to_string(),
                _ => format!("{}:{}", input_type, display_name),
            };

            // Calculate age
            let age_secs = (now - last_modified).max(0) as u64;
            let age_days = age_secs / 86400;
            let age_text = format_age(age_days);

            // Check follows
            let follows: Vec<String> = node
                .get("inputs")
                .and_then(|i| i.as_object())
                .map(|inputs| {
                    inputs
                        .iter()
                        .filter_map(|(k, v)| {
                            if let Some(s) = v.as_str() {
                                // Direct follow to another root-level input
                                if direct_input_names.values().any(|n| n == s) {
                                    Some(format!("{} → {}", k, s))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            let is_indirect = input_type == "indirect";

            Some(FlakeInput {
                name: display_name.clone(),
                input_type,
                url,
                owner,
                repo,
                branch,
                revision,
                rev_short,
                nar_hash,
                last_modified,
                age_text,
                age_days,
                follows,
                is_indirect,
            })
        })
        .collect();

    // Sort: most recently updated first
    inputs.sort_by(|a, b| a.name.cmp(&b.name));

    inputs
}

fn format_age(days: u64) -> String {
    if days == 0 {
        "today".to_string()
    } else if days == 1 {
        "1 day ago".to_string()
    } else if days < 7 {
        format!("{} days ago", days)
    } else if days < 30 {
        let weeks = days / 7;
        if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{} weeks ago", weeks)
        }
    } else if days < 365 {
        let months = days / 30;
        if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        }
    } else {
        let years = days / 365;
        if years == 1 {
            "1 year ago".to_string()
        } else {
            format!("{} years ago", years)
        }
    }
}

// ── Update process ──

fn run_selective_update(
    flake_dir: &str,
    inputs: &[(String, String)],
    tx: mpsc::Sender<UpdateStatus>,
    lang: Language,
) {
    use std::process::Command;
    let s = crate::i18n::get_strings(lang);

    // Read current lock before update for diffing
    let lock_path = format!("{}/flake.lock", flake_dir);
    let _old_lock = std::fs::read_to_string(&lock_path).ok();

    for (name, old_rev) in inputs {
        let _ = tx.send(UpdateStatus::Progress(
            s.fi_updating_input.replace("{}", name),
        ));

        let result = Command::new("nix")
            .args(["flake", "lock", "--update-input", name])
            .current_dir(flake_dir)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                // Read new lock to find new rev
                let new_rev = read_input_rev_from_lock(&lock_path, name)
                    .unwrap_or_else(|| "unknown".to_string());
                let new_rev_short = if new_rev.len() >= 7 {
                    new_rev[..7].to_string()
                } else {
                    new_rev.clone()
                };

                let changed = new_rev_short != *old_rev;
                let message = if changed {
                    s.fi_updated_input
                        .replacen("{}", old_rev, 1)
                        .replacen("{}", &new_rev_short, 1)
                } else {
                    s.fi_already_up_to_date.to_string()
                };

                let _ = tx.send(UpdateStatus::InputDone(UpdateResult {
                    input_name: name.clone(),
                    old_rev: old_rev.clone(),
                    new_rev: new_rev_short,
                    success: true,
                    message,
                }));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let msg = stderr
                    .lines()
                    .next()
                    .unwrap_or(s.fi_update_failed)
                    .to_string();
                let _ = tx.send(UpdateStatus::InputDone(UpdateResult {
                    input_name: name.clone(),
                    old_rev: old_rev.clone(),
                    new_rev: old_rev.clone(),
                    success: false,
                    message: msg,
                }));
            }
            Err(e) => {
                let _ = tx.send(UpdateStatus::InputDone(UpdateResult {
                    input_name: name.clone(),
                    old_rev: old_rev.clone(),
                    new_rev: old_rev.clone(),
                    success: false,
                    message: format!("Failed to run nix: {}", e),
                }));
            }
        }
    }

    let _ = tx.send(UpdateStatus::AllDone);
}

fn read_input_rev_from_lock(lock_path: &str, input_name: &str) -> Option<String> {
    let content = std::fs::read_to_string(lock_path).ok()?;
    let lock: serde_json::Value = serde_json::from_str(&content).ok()?;

    let nodes = lock.get("nodes")?.as_object()?;
    let root_name = lock.get("root").and_then(|r| r.as_str()).unwrap_or("root");
    let root_inputs = nodes.get(root_name)?.get("inputs")?.as_object()?;

    let node_name = root_inputs.get(input_name)?.as_str()?;
    let node = nodes.get(node_name)?;
    let rev = node.get("locked")?.get("rev")?.as_str()?;

    Some(rev.to_string())
}

// ── Age color helper ──

fn age_color(days: u64, theme: &Theme) -> ratatui::style::Color {
    if days <= 7 {
        theme.success
    } else if days <= 30 {
        theme.warning
    } else {
        theme.error
    }
}

// ── Rendering ──

pub fn render(
    frame: &mut Frame,
    state: &FlakeInputsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_flake_inputs))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    // Loading
    if state.loading {
        let lines = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(
                format!("  ⏳  {}...", s.fi_loading),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        frame.render_widget(Paragraph::new(lines).style(theme.block_style()), inner);
        return;
    }

    // Error
    if let Some(err) = &state.error_message {
        let lines = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(format!("  ✗ {}", err), Style::default().fg(theme.error)),
            Line::raw(""),
            Line::styled(
                format!("  {}", s.fi_no_flake_hint),
                Style::default().fg(theme.fg_dim),
            ),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .style(theme.block_style())
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    if !state.loaded || state.inputs.is_empty() {
        let lines = vec![
            Line::raw(""),
            Line::styled(
                format!("  {}", s.fi_empty),
                Style::default().fg(theme.fg_dim),
            ),
        ];
        frame.render_widget(Paragraph::new(lines).style(theme.block_style()), inner);
        return;
    }

    // Tab bar + content
    let chunks = Layout::vertical([
        Constraint::Length(1), // Flake path
        Constraint::Length(2), // Tab bar
        Constraint::Min(3),    // Content
    ])
    .split(inner);

    // Flake path line
    if let Some(ref path) = state.flake_path {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  📁 ", Style::default().fg(theme.accent)),
                Span::styled(path.clone(), Style::default().fg(theme.fg_dim)),
                Span::styled(
                    format!("  ({} inputs)", state.inputs.len()),
                    Style::default().fg(theme.fg_dim),
                ),
            ]))
            .style(theme.block_style()),
            chunks[0],
        );
    }

    // Tab bar
    let tabs = vec![
        s.fi_tab_overview.to_string(),
        s.fi_tab_update.to_string(),
        s.fi_tab_history.to_string(),
        s.fi_tab_details.to_string(),
    ];
    let tab_selected = state.sub_tab.index();
    let tab_titles: Vec<Line> = tabs.into_iter().map(Line::from).collect();
    let tabs_widget = Tabs::new(tab_titles)
        .select(tab_selected)
        .style(theme.text_dim())
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" │ ");
    let tabs_area = widgets::render_sub_tab_nav(frame, theme, chunks[1]);
    frame.render_widget(tabs_widget, tabs_area);

    // Content
    match state.sub_tab {
        FlakeSubTab::Overview => render_overview(frame, state, theme, lang, chunks[2]),
        FlakeSubTab::Update => render_update(frame, state, theme, lang, chunks[2]),
        FlakeSubTab::History => render_history(frame, state, theme, lang, chunks[2]),
        FlakeSubTab::Details => render_details(frame, state, theme, lang, chunks[2]),
    }

    // Popup overlay
    if state.popup != FlakePopup::None {
        render_popup(frame, state, theme, lang, area);
    }
}

fn render_overview(
    frame: &mut Frame,
    state: &FlakeInputsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let visible_height = area.height as usize;

    let mut scroll = state.scroll_offset;
    if state.selected >= scroll + visible_height {
        scroll = state.selected + 1 - visible_height;
    }
    if state.selected < scroll {
        scroll = state.selected;
    }

    let name_w = 20usize.min(area.width as usize / 4);
    let url_w = 30usize.min(area.width as usize / 3);
    let rev_w = 9;

    let items: Vec<ListItem> = state
        .inputs
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, input)| {
            let is_selected = i == state.selected;
            let style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };

            let name_display = if input.name.len() > name_w {
                let t = safe_truncate(&input.name, name_w.saturating_sub(1));
                format!("{}…", t)
            } else {
                format!("{:<width$}", input.name, width = name_w)
            };

            let url_display = if input.url.len() > url_w {
                let t = safe_truncate(&input.url, url_w.saturating_sub(1));
                format!("{}…", t)
            } else {
                format!("{:<width$}", input.url, width = url_w)
            };

            let age_c = age_color(input.age_days, theme);

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {}", name_display),
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.accent)
                    },
                ),
                Span::styled(
                    format!(" {} ", url_display),
                    if is_selected {
                        style
                    } else {
                        Style::default().fg(theme.fg_dim)
                    },
                ),
                Span::styled(
                    format!("{:<width$}", input.rev_short, width = rev_w),
                    if is_selected { style } else { theme.text() },
                ),
                Span::styled(format!(" {}", input.age_text), Style::default().fg(age_c)),
            ]))
        })
        .collect();

    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(""),
                Line::styled(
                    format!("  {}", s.fi_empty),
                    Style::default().fg(theme.fg_dim),
                ),
            ])
            .style(theme.block_style()),
            area,
        );
    } else {
        frame.render_widget(List::new(items).style(theme.block_style()), area);
    }
}

fn render_update(
    frame: &mut Frame,
    state: &FlakeInputsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    // Hint line
    let chunks = Layout::vertical([
        Constraint::Length(1), // Hint
        Constraint::Min(3),    // List
    ])
    .split(area);

    let checked_count = state.update_checked.iter().filter(|&&v| v).count();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("  {}", s.fi_update_hint),
                Style::default().fg(theme.fg_dim),
            ),
            Span::styled(
                format!("  ({}/{})", checked_count, state.inputs.len()),
                Style::default().fg(theme.accent),
            ),
        ]))
        .style(theme.block_style()),
        chunks[0],
    );

    let visible_height = chunks[1].height as usize;
    let mut scroll = state.update_scroll;
    if state.update_selected >= scroll + visible_height {
        scroll = state.update_selected + 1 - visible_height;
    }
    if state.update_selected < scroll {
        scroll = state.update_selected;
    }

    let items: Vec<ListItem> = state
        .inputs
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, input)| {
            let is_selected = i == state.update_selected;
            let is_checked = state.update_checked.get(i).copied().unwrap_or(false);
            let style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };

            let checkbox = if is_checked { "[✓]" } else { "[ ]" };
            let checkbox_style = if is_checked {
                Style::default().fg(theme.success)
            } else {
                Style::default().fg(theme.fg_dim)
            };

            let age_c = age_color(input.age_days, theme);

            ListItem::new(Line::from(vec![
                Span::styled(format!("  {} ", checkbox), checkbox_style),
                Span::styled(
                    format!("{:<20}", input.name),
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.accent)
                    },
                ),
                Span::styled(
                    format!(" {}  ", input.rev_short),
                    if is_selected { style } else { theme.text() },
                ),
                Span::styled(input.age_text.clone(), Style::default().fg(age_c)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items).style(theme.block_style()), chunks[1]);
}

fn render_history(
    frame: &mut Frame,
    state: &FlakeInputsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if state.history.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(""),
                Line::raw(""),
                Line::styled(
                    format!("  {}", s.fi_history_empty),
                    Style::default().fg(theme.fg_dim),
                ),
                Line::raw(""),
                Line::styled(
                    format!("  {}", s.fi_history_hint),
                    Style::default().fg(theme.fg_dim),
                ),
            ])
            .alignment(Alignment::Center)
            .style(theme.block_style()),
            area,
        );
        return;
    }

    let visible_height = area.height as usize;
    let mut scroll = state.history_scroll;
    if state.history_selected >= scroll + visible_height {
        scroll = state.history_selected + 1 - visible_height;
    }
    if state.history_selected < scroll {
        scroll = state.history_selected;
    }

    let items: Vec<ListItem> = state
        .history
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, result)| {
            let is_selected = i == state.history_selected;
            let style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };

            let status_icon = if result.success {
                if result.old_rev == result.new_rev {
                    "═"
                } else {
                    "✓"
                }
            } else {
                "✗"
            };
            let status_color = if result.success {
                if result.old_rev == result.new_rev {
                    theme.fg_dim
                } else {
                    theme.success
                }
            } else {
                theme.error
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("{:<20}", result.input_name),
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.accent)
                    },
                ),
                Span::styled(
                    format!(" {} → {} ", result.old_rev, result.new_rev),
                    if is_selected { style } else { theme.text() },
                ),
                Span::styled(result.message.clone(), Style::default().fg(theme.fg_dim)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items).style(theme.block_style()), area);
}

fn render_details(
    frame: &mut Frame,
    state: &FlakeInputsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if state.selected >= state.inputs.len() {
        return;
    }
    let input = &state.inputs[state.selected];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));

    // Input name as title
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            input.name.clone(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Fields
    let fields: Vec<(&str, String, ratatui::style::Color)> = vec![
        (s.fi_detail_type, input.input_type.clone(), theme.fg),
        (s.fi_detail_url, input.url.clone(), theme.accent),
        (
            s.fi_detail_branch,
            if input.branch.is_empty() {
                "(default)".to_string()
            } else {
                input.branch.clone()
            },
            theme.fg,
        ),
        (s.fi_detail_rev, input.revision.clone(), theme.fg),
        (s.fi_detail_narhash, input.nar_hash.clone(), theme.fg_dim),
        (
            s.fi_detail_age,
            input.age_text.clone(),
            age_color(input.age_days, theme),
        ),
    ];

    for (label, value, color) in &fields {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<14}", label),
                Style::default().fg(theme.fg_dim),
            ),
            Span::styled(value.clone(), Style::default().fg(*color)),
        ]));
    }

    // Last modified timestamp
    if input.last_modified > 0 {
        let dt = chrono::DateTime::from_timestamp(input.last_modified, 0);
        if let Some(dt) = dt {
            let local: chrono::DateTime<chrono::Local> = dt.into();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<14}", s.fi_detail_locked),
                    Style::default().fg(theme.fg_dim),
                ),
                Span::styled(
                    local.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Style::default().fg(theme.fg),
                ),
            ]));
        }
    }

    // Follows
    if !input.follows.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("  {}", s.fi_detail_follows),
            Style::default()
                .fg(theme.fg_dim)
                .add_modifier(Modifier::BOLD),
        ));
        for follow in &input.follows {
            lines.push(Line::styled(
                format!("    {}", follow),
                Style::default().fg(theme.accent_dim),
            ));
        }
    }

    // Navigation hint
    lines.push(Line::raw(""));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  [j/k] {}  [r] {}", s.navigate, s.fi_refresh),
        Style::default().fg(theme.fg_dim),
    ));

    frame.render_widget(Paragraph::new(lines).style(theme.block_style()), area);
}

fn render_popup(
    frame: &mut Frame,
    state: &FlakeInputsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    // Center popup
    let popup_w = 50u16.min(area.width.saturating_sub(4));
    let popup_h = 12u16.min(area.height.saturating_sub(4));
    let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    // Clear background
    frame.render_widget(ratatui::widgets::Clear, popup_area);

    match &state.popup {
        FlakePopup::ConfirmUpdate => {
            let selected_names: Vec<String> = state
                .inputs
                .iter()
                .enumerate()
                .filter(|(i, _)| state.update_checked.get(*i).copied().unwrap_or(false))
                .map(|(_, inp)| inp.name.clone())
                .collect();

            let mut lines = vec![
                Line::raw(""),
                Line::styled(
                    format!("  {}", s.fi_confirm_title),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::raw(""),
            ];

            for name in &selected_names {
                lines.push(Line::styled(format!("    • {}", name), theme.text()));
            }

            lines.push(Line::raw(""));
            lines.push(Line::styled(
                format!("  [Enter/y] {}  [Esc/n] {}", s.confirm, s.cancel),
                Style::default().fg(theme.fg_dim),
            ));

            let block = Block::default()
                .title(format!(" {} ", s.fi_tab_update))
                .title_style(theme.title())
                .borders(Borders::ALL)
                .border_style(theme.border_focused())
                .style(theme.block_style());

            frame.render_widget(Paragraph::new(lines).block(block), popup_area);
        }
        FlakePopup::Updating => {
            let mut lines = vec![
                Line::raw(""),
                Line::styled(
                    format!("  ⏳ {}...", s.fi_updating),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::raw(""),
            ];

            // Show last few log lines
            for log_line in state.update_log.iter().rev().take(5).rev() {
                lines.push(Line::styled(
                    format!("  {}", log_line),
                    Style::default().fg(theme.success),
                ));
            }

            let block = Block::default()
                .title(format!(" {} ", s.fi_tab_update))
                .title_style(theme.title())
                .borders(Borders::ALL)
                .border_style(theme.border_focused())
                .style(theme.block_style());

            frame.render_widget(Paragraph::new(lines).block(block), popup_area);
        }
        FlakePopup::None => {}
    }
}

/// Safely truncate a string to at most `max_bytes`
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
