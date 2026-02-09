//! Options Explorer module
//!
//! Search, browse, and discover all 20,000+ NixOS options.
//! Three sub-tabs:
//!   F1 Search  — fuzzy search with detail view + current values
//!   F2 Browse  — tree navigation through the option hierarchy
//!   F3 Related — sibling options for the selected option
//!
//! Data source: options.json from NixOS manual (pre-built or generated).
//! Current values loaded on-demand via nixos-option.

use crate::config::Language;
use crate::i18n;
use crate::ui::theme::Theme;
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
use crate::types::FlashMessage;
use std::time::Instant;

// ── Sub-tabs ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptSubTab {
    #[default]
    Search,
    Browse,
    Related,
}

// ── NixOS option data ──

#[derive(Debug, Clone)]
pub struct NixOption {
    pub path: String,
    pub type_str: String,
    pub description: String,
    pub default_str: Option<String>,
    pub example_str: Option<String>,
    pub declared_in: Vec<String>,
    pub read_only: bool,
}

// ── Tree node for Browse tab ──

#[derive(Debug, Clone)]
pub struct TreeRow {
    pub display_name: String,
    pub full_path: String,
    pub depth: usize,
    pub is_leaf: bool,
    pub is_expanded: bool,
    pub child_count: usize,
    pub option_idx: Option<usize>,
}

// ── Load status from background thread ──

#[derive(Debug)]
pub enum LoadStatus {
    Phase(String),
    Done(Vec<NixOption>),
    Error(String),
}

// ── Current value result ──

#[derive(Debug)]
pub struct CurrentValue {
    pub path: String,
    pub value: Option<String>,
    pub error: Option<String>,
}

// ── Module state ──

pub struct OptionsState {
    pub sub_tab: OptSubTab,

    // Data
    pub options: Vec<NixOption>,
    pub loaded: bool,
    pub loading: bool,
    pub loading_phase: String,
    pub loading_start: Option<Instant>,
    pub error_message: Option<String>,
    load_rx: Option<mpsc::Receiver<LoadStatus>>,

    // Search tab
    pub search_active: bool,
    pub search_query: String,
    pub search_results: Vec<usize>, // indices into options vec
    pub search_selected: usize,
    pub search_scroll: usize,

    // Detail view (shared between tabs)
    pub detail_open: bool,
    pub detail_option_idx: Option<usize>,
    pub detail_scroll: usize,
    pub current_value: Option<String>,
    pub current_value_loading: bool,
    current_value_rx: Option<mpsc::Receiver<CurrentValue>>,
    current_value_path: String,

    // Browse tab
    pub tree_rows: Vec<TreeRow>,
    pub tree_selected: usize,
    pub tree_scroll: usize,
    /// Map from path prefix -> expanded state
    tree_expanded: HashMap<String, bool>,
    tree_built: bool,

    // Related tab
    pub related_options: Vec<usize>,
    pub related_selected: usize,
    pub related_scroll: usize,
    pub related_for_path: String,

    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

impl OptionsState {
    pub fn new() -> Self {
        Self {
            sub_tab: OptSubTab::Search,
            options: Vec::new(),
            loaded: false,
            loading: false,
            loading_phase: String::new(),
            loading_start: None,
            error_message: None,
            load_rx: None,
            search_active: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_scroll: 0,
            detail_open: false,
            detail_option_idx: None,
            detail_scroll: 0,
            current_value: None,
            current_value_loading: false,
            current_value_rx: None,
            current_value_path: String::new(),
            tree_rows: Vec::new(),
            tree_selected: 0,
            tree_scroll: 0,
            tree_expanded: HashMap::new(),
            tree_built: false,
            related_options: Vec::new(),
            related_selected: 0,
            related_scroll: 0,
            related_for_path: String::new(),
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
        self.loading_phase = String::new();
        self.loading_start = Some(Instant::now());
        self.error_message = None;

        let (tx, rx) = mpsc::channel();
        self.load_rx = Some(rx);
        let lang = self.lang;

        std::thread::spawn(move || {
            load_options_background(tx, lang);
        });
    }

    /// Poll background loader
    pub fn poll_load(&mut self) {
        if let Some(rx) = &self.load_rx {
            loop {
                match rx.try_recv() {
                    Ok(LoadStatus::Phase(msg)) => {
                        self.loading_phase = msg;
                    }
                    Ok(LoadStatus::Done(options)) => {
                        let count = options.len();
                        self.options = options;
                        self.loaded = true;
                        self.loading = false;
                        self.load_rx = None;
                        self.loading_phase = format!("{} options loaded", count);
                        return;
                    }
                    Ok(LoadStatus::Error(msg)) => {
                        self.error_message = Some(msg);
                        self.loading = false;
                        self.loaded = true;
                        self.load_rx = None;
                        return;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.loading = false;
                        self.loaded = true;
                        self.load_rx = None;
                        if self.options.is_empty() {
                            self.error_message =
                                Some(crate::i18n::get_strings(self.lang).opt_load_failed.to_string());
                        }
                        return;
                    }
                }
            }
        }

        // Poll current value
        if let Some(rx) = &self.current_value_rx {
            match rx.try_recv() {
                Ok(cv) => {
                    if cv.path == self.current_value_path {
                        self.current_value = cv.value.or(cv.error);
                    }
                    self.current_value_loading = false;
                    self.current_value_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.current_value_loading = false;
                    self.current_value_rx = None;
                }
            }
        }
    }

    /// Run fuzzy search over loaded options
    fn run_search(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            self.search_results.clear();
            return;
        }

        let mut scored: Vec<(usize, i32)> = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| {
                let path_lower = opt.path.to_lowercase();
                let desc_lower = opt.description.to_lowercase();

                // Exact substring match in path (highest priority)
                if path_lower.contains(&query) {
                    let score = if path_lower == query {
                        1000
                    } else if path_lower.starts_with(&query) {
                        900
                    } else {
                        // Bonus for shorter paths (more specific matches)
                        800 - (opt.path.len() as i32).min(400)
                    };
                    return Some((i, score));
                }

                // Match in description
                if desc_lower.contains(&query) {
                    return Some((i, 200 - (opt.path.len() as i32).min(100)));
                }

                // Fuzzy: all query chars appear in order in path
                if fuzzy_match(&query, &path_lower) {
                    return Some((i, 100 - (opt.path.len() as i32).min(50)));
                }

                None
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(500);

        self.search_results = scored.into_iter().map(|(i, _)| i).collect();
        self.search_selected = 0;
        self.search_scroll = 0;
    }

    /// Build tree rows for Browse tab
    fn ensure_tree_built(&mut self) {
        if self.tree_built || self.options.is_empty() {
            return;
        }
        self.tree_built = true;
        self.rebuild_tree_rows();
    }

    fn rebuild_tree_rows(&mut self) {
        // Build a nested map: segment -> children
        // Top level prefixes are initially shown collapsed
        let mut top_level: Vec<String> = Vec::new();
        let mut prefix_counts: HashMap<String, usize> = HashMap::new();

        for opt in &self.options {
            if let Some(first_dot) = opt.path.find('.') {
                let prefix = &opt.path[..first_dot];
                *prefix_counts.entry(prefix.to_string()).or_insert(0) += 1;
                if !top_level.contains(&prefix.to_string()) {
                    top_level.push(prefix.to_string());
                }
            }
        }

        top_level.sort();

        self.tree_rows.clear();
        for prefix in &top_level {
            let count = prefix_counts.get(prefix).copied().unwrap_or(0);
            let expanded = self.tree_expanded.get(prefix).copied().unwrap_or(false);

            self.tree_rows.push(TreeRow {
                display_name: prefix.clone(),
                full_path: prefix.clone(),
                depth: 0,
                is_leaf: false,
                is_expanded: expanded,
                child_count: count,
                option_idx: None,
            });

            if expanded {
                self.add_children_for_prefix(prefix, 1);
            }
        }
    }

    fn add_children_for_prefix(&mut self, prefix: &str, depth: usize) {
        if depth > 6 {
            return; // Safety limit
        }

        // Find all unique next segments after this prefix
        let prefix_dot = format!("{}.", prefix);
        let mut next_segments: Vec<String> = Vec::new();
        let mut segment_counts: HashMap<String, usize> = HashMap::new();
        let mut direct_options: Vec<usize> = Vec::new();

        for (i, opt) in self.options.iter().enumerate() {
            if !opt.path.starts_with(&prefix_dot) {
                continue;
            }
            let rest = &opt.path[prefix_dot.len()..];
            if let Some(dot_pos) = rest.find('.') {
                let seg = &rest[..dot_pos];
                let full = format!("{}.{}", prefix, seg);
                *segment_counts.entry(full.clone()).or_insert(0) += 1;
                if !next_segments.contains(&full) {
                    next_segments.push(full);
                }
            } else {
                // This is a leaf option directly under this prefix
                direct_options.push(i);
            }
        }

        next_segments.sort();

        // Add leaf options first (direct children)
        for opt_idx in direct_options {
            let opt = &self.options[opt_idx];
            let leaf_name = opt.path[prefix_dot.len()..].to_string();
            self.tree_rows.push(TreeRow {
                display_name: leaf_name,
                full_path: opt.path.clone(),
                depth,
                is_leaf: true,
                is_expanded: false,
                child_count: 0,
                option_idx: Some(opt_idx),
            });
        }

        // Then add sub-groups
        for seg_path in &next_segments {
            let seg_name = seg_path.rsplit('.').next().unwrap_or(seg_path).to_string();
            let count = segment_counts.get(seg_path).copied().unwrap_or(0);
            let expanded = self.tree_expanded.get(seg_path).copied().unwrap_or(false);

            self.tree_rows.push(TreeRow {
                display_name: seg_name,
                full_path: seg_path.clone(),
                depth,
                is_leaf: false,
                is_expanded: expanded,
                child_count: count,
                option_idx: None,
            });

            if expanded {
                self.add_children_for_prefix(seg_path, depth + 1);
            }
        }
    }

    /// Toggle expand/collapse for a tree node
    fn toggle_tree_node(&mut self) {
        if self.tree_selected >= self.tree_rows.len() {
            return;
        }
        let row = &self.tree_rows[self.tree_selected];
        if row.is_leaf {
            // Open detail for leaf
            if let Some(idx) = row.option_idx {
                self.open_detail(idx);
            }
            return;
        }

        let path = row.full_path.clone();
        let was_expanded = row.is_expanded;
        self.tree_expanded.insert(path, !was_expanded);
        self.rebuild_tree_rows();

        // Keep selection in bounds
        if self.tree_selected >= self.tree_rows.len() {
            self.tree_selected = self.tree_rows.len().saturating_sub(1);
        }
    }

    /// Open detail view for an option
    fn open_detail(&mut self, option_idx: usize) {
        self.detail_open = true;
        self.detail_option_idx = Some(option_idx);
        self.detail_scroll = 0;
        self.current_value = None;
        self.current_value_loading = false;

        // Start loading current value
        if option_idx < self.options.len() {
            let path = self.options[option_idx].path.clone();
            self.current_value_path = path.clone();
            self.current_value_loading = true;

            let (tx, rx) = mpsc::channel();
            self.current_value_rx = Some(rx);

            let lang = self.lang;
            std::thread::spawn(move || {
                let result = load_current_value(&path, lang);
                let _ = tx.send(result);
            });
        }
    }

    /// Build related options for the Related tab
    fn build_related(&mut self, option_idx: usize) {
        if option_idx >= self.options.len() {
            return;
        }

        let path = &self.options[option_idx].path;
        self.related_for_path = path.clone();

        // Find parent path
        let parent = if let Some(dot_pos) = path.rfind('.') {
            &path[..dot_pos]
        } else {
            return;
        };

        let parent_dot = format!("{}.", parent);

        // Find all siblings (same parent, one level deep)
        self.related_options = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, opt)| {
                if !opt.path.starts_with(&parent_dot) {
                    return false;
                }
                let rest = &opt.path[parent_dot.len()..];
                // Only direct children (no further dots)
                !rest.contains('.')
            })
            .map(|(i, _)| i)
            .collect();

        self.related_selected = 0;
        self.related_scroll = 0;

        // Try to select the current option in the related list
        if let Some(pos) = self.related_options.iter().position(|&i| i == option_idx) {
            self.related_selected = pos;
        }

        self.sub_tab = OptSubTab::Related;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Detail overlay captures all keys
        if self.detail_open {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.detail_open = false;
                    self.detail_option_idx = None;
                    self.current_value = None;
                    self.current_value_loading = false;
                }
                KeyCode::Char('r') => {
                    // Switch to related tab for this option
                    if let Some(idx) = self.detail_option_idx {
                        self.detail_open = false;
                        self.build_related(idx);
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
                _ => {}
            }
            return Ok(true);
        }

        // Sub-tab switching
        match key.code {
            KeyCode::F(1) => {
                self.sub_tab = OptSubTab::Search;
                return Ok(true);
            }
            KeyCode::F(2) => {
                self.sub_tab = OptSubTab::Browse;
                self.ensure_tree_built();
                return Ok(true);
            }
            KeyCode::F(3) => {
                self.sub_tab = OptSubTab::Related;
                return Ok(true);
            }
            _ => {}
        }

        match self.sub_tab {
            OptSubTab::Search => self.handle_search_key(key),
            OptSubTab::Browse => self.handle_browse_key(key),
            OptSubTab::Related => self.handle_related_key(key),
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.search_active {
            match key.code {
                KeyCode::Enter => {
                    self.search_active = false;
                    self.run_search();
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
                if !self.search_results.is_empty() {
                    self.search_selected =
                        (self.search_selected + 1).min(self.search_results.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.search_selected > 0 {
                    self.search_selected -= 1;
                }
            }
            KeyCode::Char('g') => {
                self.search_selected = 0;
            }
            KeyCode::Char('G') => {
                if !self.search_results.is_empty() {
                    self.search_selected = self.search_results.len() - 1;
                }
            }
            KeyCode::Enter => {
                if !self.search_results.is_empty() {
                    let opt_idx = self.search_results[self.search_selected];
                    self.open_detail(opt_idx);
                }
            }
            KeyCode::Char('r') => {
                if !self.search_results.is_empty() {
                    let opt_idx = self.search_results[self.search_selected];
                    self.build_related(opt_idx);
                }
            }
            KeyCode::Char('n') => {
                self.search_query.clear();
                self.search_results.clear();
                self.search_active = true;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn handle_browse_key(&mut self, key: KeyEvent) -> Result<bool> {
        self.ensure_tree_built();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.tree_rows.is_empty() {
                    self.tree_selected =
                        (self.tree_selected + 1).min(self.tree_rows.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.tree_selected > 0 {
                    self.tree_selected -= 1;
                }
            }
            KeyCode::Char('g') => {
                self.tree_selected = 0;
            }
            KeyCode::Char('G') => {
                if !self.tree_rows.is_empty() {
                    self.tree_selected = self.tree_rows.len() - 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                self.toggle_tree_node();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                // Collapse current node or go to parent
                if self.tree_selected < self.tree_rows.len() {
                    let row = &self.tree_rows[self.tree_selected];
                    if !row.is_leaf && row.is_expanded {
                        let path = row.full_path.clone();
                        self.tree_expanded.insert(path, false);
                        self.rebuild_tree_rows();
                    } else if row.depth > 0 {
                        // Navigate to parent
                        let current_depth = row.depth;
                        let mut target = self.tree_selected;
                        while target > 0 {
                            target -= 1;
                            if self.tree_rows[target].depth < current_depth
                                && !self.tree_rows[target].is_leaf
                            {
                                self.tree_selected = target;
                                break;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                // Related for selected leaf option
                if self.tree_selected < self.tree_rows.len() {
                    let row = &self.tree_rows[self.tree_selected];
                    if row.is_leaf {
                        if let Some(idx) = row.option_idx {
                            self.build_related(idx);
                        }
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn handle_related_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.related_options.is_empty() {
                    self.related_selected =
                        (self.related_selected + 1).min(self.related_options.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.related_selected > 0 {
                    self.related_selected -= 1;
                }
            }
            KeyCode::Char('g') => {
                self.related_selected = 0;
            }
            KeyCode::Char('G') => {
                if !self.related_options.is_empty() {
                    self.related_selected = self.related_options.len() - 1;
                }
            }
            KeyCode::Enter => {
                if !self.related_options.is_empty() {
                    let opt_idx = self.related_options[self.related_selected];
                    self.open_detail(opt_idx);
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

// ── Fuzzy matching ──

fn fuzzy_match(query: &str, target: &str) -> bool {
    let mut query_chars = query.chars().peekable();
    for tc in target.chars() {
        if let Some(&qc) = query_chars.peek() {
            if qc == tc {
                query_chars.next();
            }
        }
    }
    query_chars.peek().is_none()
}

/// Safely truncate a string to at most `max_bytes` without splitting UTF-8 chars
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

// ── Background loading ──

fn load_options_background(tx: mpsc::Sender<LoadStatus>, lang: Language) {
    let s = crate::i18n::get_strings(lang);
    use std::process::Command;

    // Phase 1: Try pre-built options.json (fast path)
    let _ = tx.send(LoadStatus::Phase(s.opt_phase_prebuilt.to_string()));

    // Try standard NixOS documentation path
    let doc_path = "/run/current-system/sw/share/doc/nixos/options.json";
    if let Some(options) = try_load_options_json(doc_path) {
        let _ = tx.send(LoadStatus::Done(options));
        return;
    }

    // Phase 2: Try building options.json
    let _ = tx.send(LoadStatus::Phase(
        s.opt_building_db.to_string(),
    ));

    // Try nix-build for channels
    let result = Command::new("nix-build")
        .args([
            "<nixpkgs/nixos/release.nix>",
            "-A",
            "options",
            "--no-out-link",
        ])
        .output();

    if let Ok(output) = result {
        if output.status.success() {
            let store_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let json_path = format!("{}/share/doc/nixos/options.json", store_path);
            if let Some(options) = try_load_options_json(&json_path) {
                let _ = tx.send(LoadStatus::Done(options));
                return;
            }
        }
    }

    // Phase 3: Try flakes-based build
    let _ = tx.send(LoadStatus::Phase(
        s.opt_trying_flakes.to_string(),
    ));

    let home = std::env::var("HOME").unwrap_or_default();
    let flake_dirs = [
        "/etc/nixos",
        &format!("{}/.config/nixos", home),
        &format!("{}/nixos", home),
        &format!("{}/.nixos", home),
    ];

    for flake_dir in &flake_dirs {
        let flake_nix = format!("{}/flake.nix", flake_dir);
        if !std::path::Path::new(&flake_nix).exists() {
            continue;
        }

        let result = Command::new("nix")
            .args([
                "build",
                &format!("{}#nixosConfigurations.{}.config.system.build.manual.optionsJSON",
                    flake_dir,
                    get_hostname()),
                "--no-link",
                "--print-out-paths",
            ])
            .output();

        if let Ok(output) = result {
            if output.status.success() {
                let store_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                // Try both possible locations
                for suffix in &["/share/doc/nixos/options.json", ""] {
                    let json_path = format!("{}{}", store_path, suffix);
                    if let Some(options) = try_load_options_json(&json_path) {
                        let _ = tx.send(LoadStatus::Done(options));
                        return;
                    }
                }
            }
        }
    }

    // Phase 4: Last resort — try nixos-option -r (slow but universal)
    let _ = tx.send(LoadStatus::Phase(
        s.opt_phase_fallback.to_string(),
    ));

    if let Some(options) = try_nixos_option_fallback() {
        if !options.is_empty() {
            let _ = tx.send(LoadStatus::Done(options));
            return;
        }
    }

    let _ = tx.send(LoadStatus::Error(
        s.opt_load_error.to_string(),
    ));
}

fn try_load_options_json(path: &str) -> Option<Vec<NixOption>> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_options_json(&content)
}

fn parse_options_json(content: &str) -> Option<Vec<NixOption>> {
    let data: serde_json::Value = serde_json::from_str(content).ok()?;
    let obj = data.as_object()?;

    let mut options: Vec<NixOption> = obj
        .iter()
        .filter_map(|(path, info)| {
            // Skip internal/hidden options
            if path.starts_with("_") || path.contains("._") {
                return None;
            }

            let type_str = info
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let description = info
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let default_str = info.get("default").map(|v| {
                if v.is_string() {
                    v.as_str().unwrap_or("").to_string()
                } else if v.is_null() {
                    "null".to_string()
                } else {
                    // Format the JSON value compactly
                    format_nix_value(v)
                }
            });

            let example_str = info.get("example").and_then(|v| {
                if v.is_null() {
                    None
                } else if v.is_string() {
                    Some(v.as_str().unwrap_or("").to_string())
                } else {
                    Some(format_nix_value(v))
                }
            });

            let declared_in = info
                .get("declarations")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let read_only = info
                .get("readOnly")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            Some(NixOption {
                path: path.clone(),
                type_str,
                description,
                default_str,
                example_str,
                declared_in,
                read_only,
            })
        })
        .collect();

    options.sort_by(|a, b| a.path.cmp(&b.path));
    Some(options)
}

fn format_nix_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Array(arr) => {
            if arr.len() <= 3 {
                let items: Vec<String> = arr.iter().map(|v| format_nix_value(v)).collect();
                format!("[ {} ]", items.join(" "))
            } else {
                format!("[ ... ] ({} items)", arr.len())
            }
        }
        serde_json::Value::Object(obj) => {
            if obj.len() <= 2 {
                let items: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| format!("{} = {}", k, format_nix_value(v)))
                    .collect();
                format!("{{ {} }}", items.join("; "))
            } else {
                format!("{{ ... }} ({} attrs)", obj.len())
            }
        }
        serde_json::Value::Null => "null".to_string(),
    }
}

fn get_hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn try_nixos_option_fallback() -> Option<Vec<NixOption>> {
    use std::process::Command;

    // Get list of all option paths
    let output = Command::new("nixos-option")
        .args(["-r"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let options: Vec<NixOption> = stdout
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with(' '))
        .map(|path| NixOption {
            path: path.trim().to_string(),
            type_str: String::new(),
            description: String::new(),
            default_str: None,
            example_str: None,
            declared_in: Vec::new(),
            read_only: false,
        })
        .collect();

    Some(options)
}

fn load_current_value(path: &str, lang: crate::config::Language) -> CurrentValue {
    use std::process::Command;
    let s = crate::i18n::get_strings(lang);

    let output = Command::new("nixos-option")
        .arg(path)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // Parse "Value:\n  <value>" from output
            let value = parse_nixos_option_value(&stdout);
            CurrentValue {
                path: path.to_string(),
                value,
                error: None,
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            CurrentValue {
                path: path.to_string(),
                value: None,
                error: Some(
                    stderr
                        .lines()
                        .next()
                        .unwrap_or(s.opt_read_failed)
                        .to_string(),
                ),
            }
        }
        Err(e) => CurrentValue {
            path: path.to_string(),
            value: None,
            error: Some(s.opt_not_found.replace("{}", &e.to_string())),
        },
    }
}

fn parse_nixos_option_value(output: &str) -> Option<String> {
    let mut in_value = false;
    let mut value_lines: Vec<String> = Vec::new();

    for line in output.lines() {
        if line.starts_with("Value:") {
            in_value = true;
            // Check if value is on same line
            let rest = line.trim_start_matches("Value:").trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
            continue;
        }

        if in_value {
            // Stop at next section header
            if !line.starts_with(' ') && !line.is_empty() && line.contains(':') {
                break;
            }
            value_lines.push(line.trim().to_string());
        }
    }

    if value_lines.is_empty() {
        None
    } else {
        Some(value_lines.join("\n").trim().to_string())
    }
}

// ── Type color coding helper ──

fn type_color(type_str: &str, theme: &Theme) -> ratatui::style::Color {
    let t = type_str.to_lowercase();
    if t.contains("bool") {
        theme.success
    } else if t.contains("string") || t.contains("str") || t.contains("path") {
        theme.accent
    } else if t.contains("int") || t.contains("float") || t.contains("number") {
        theme.warning
    } else if t.contains("list") || t.contains("listof") {
        theme.error
    } else if t.contains("attr") || t.contains("submodule") {
        theme.accent_dim
    } else if t.contains("enum") || t.contains("one of") {
        theme.warning
    } else {
        theme.fg_dim
    }
}

// ── Rendering ──

pub fn render(
    frame: &mut Frame,
    state: &OptionsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_options))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    // Loading state
    if state.loading {
        render_loading(frame, state, theme, lang, inner);
        return;
    }

    // Error state
    if let Some(err) = &state.error_message {
        render_error(frame, err, theme, inner);
        return;
    }

    // Not loaded yet
    if !state.loaded || state.options.is_empty() {
        render_empty(frame, theme, lang, inner);
        return;
    }

    // Sub-tab bar + content
    let chunks = Layout::vertical([
        Constraint::Length(2), // Tab bar
        Constraint::Min(4),   // Content
    ])
    .split(inner);

    render_tab_bar(frame, state, theme, lang, chunks[0]);

    // Detail overlay or tab content
    if state.detail_open {
        render_detail(frame, state, theme, lang, chunks[1]);
    } else {
        match state.sub_tab {
            OptSubTab::Search => render_search(frame, state, theme, lang, chunks[1]),
            OptSubTab::Browse => render_browse(frame, state, theme, lang, chunks[1]),
            OptSubTab::Related => render_related(frame, state, theme, lang, chunks[1]),
        }
    }
}

fn render_tab_bar(
    frame: &mut Frame,
    state: &OptionsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let tabs = vec![
        s.opt_tab_search.to_string(),
        s.opt_tab_browse.to_string(),
        s.opt_tab_related.to_string(),
    ];

    let selected = match state.sub_tab {
        OptSubTab::Search => 0,
        OptSubTab::Browse => 1,
        OptSubTab::Related => 2,
    };

    let tab_titles: Vec<Line> = tabs.into_iter().map(Line::from).collect();

    let tabs_widget = Tabs::new(tab_titles)
        .select(selected)
        .style(theme.text_dim())
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" │ ");

    frame.render_widget(tabs_widget, area);

    // Option count on the right
    let count_text = format!("{} options ", state.options.len());
    if area.width > count_text.len() as u16 + 2 {
        let count_area = Rect {
            x: area.x + area.width - count_text.len() as u16 - 1,
            y: area.y,
            width: count_text.len() as u16 + 1,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::styled(
                count_text,
                Style::default().fg(theme.fg_dim),
            )),
            count_area,
        );
    }
}

fn render_loading(
    frame: &mut Frame,
    state: &OptionsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let elapsed = state.loading_start.map(|s| s.elapsed().as_secs()).unwrap_or(0);

    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_idx = (elapsed as usize * 3) % spinner_frames.len();
    let spinner = spinner_frames[spinner_idx];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));
    lines.push(Line::raw(""));

    // Spinner + title + elapsed
    lines.push(Line::styled(
        format!("  {}  {} ({}s)", spinner, s.opt_loading, elapsed),
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

    // Progress bar (visual feedback)
    let bar_width = (area.width as usize).saturating_sub(8).min(40);
    if bar_width > 4 {
        let fill = ((elapsed as usize * 2) % bar_width).min(bar_width);
        let filled: String = "█".repeat(fill);
        let empty: String = "░".repeat(bar_width - fill);
        lines.push(Line::styled(
            format!("  [{}{}]", filled, empty),
            Style::default().fg(theme.accent_dim),
        ));
        lines.push(Line::raw(""));
    }

    // Hint
    lines.push(Line::styled(
        format!("  {}", s.opt_loading_hint),
        Style::default().fg(theme.fg_dim),
    ));

    frame.render_widget(Paragraph::new(lines).style(theme.block_style()), area);
}

fn render_error(frame: &mut Frame, error: &str, theme: &Theme, area: Rect) {
    let lines = vec![
        Line::raw(""),
        Line::raw(""),
        Line::styled(
            "  ✗ Error loading options",
            Style::default()
                .fg(theme.error)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(format!("  {}", error), theme.text()),
    ];
    frame.render_widget(Paragraph::new(lines).style(theme.block_style()).wrap(Wrap { trim: false }), area);
}

fn render_empty(frame: &mut Frame, theme: &Theme, lang: Language, area: Rect) {
    let s = i18n::get_strings(lang);
    let lines = vec![
        Line::raw(""),
        Line::raw(""),
        Line::styled(
            format!("  {}", s.opt_empty),
            Style::default().fg(theme.fg_dim),
        ),
    ];
    frame.render_widget(Paragraph::new(lines).style(theme.block_style()), area);
}

fn render_search(
    frame: &mut Frame,
    state: &OptionsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let chunks = Layout::vertical([
        Constraint::Length(2), // Search bar
        Constraint::Min(3),   // Results
    ])
    .split(area);

    // Search bar
    let cursor_char = if state.search_active { "│" } else { "" };
    let query_display = if state.search_query.is_empty() && !state.search_active {
        s.opt_search_hint.to_string()
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
            format!("  {} ", s.opt_search_label),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(query_display, search_style),
    ]);
    frame.render_widget(Paragraph::new(line).style(theme.block_style()), chunks[0]);

    // Result count
    if !state.search_results.is_empty() {
        let count_text = format!("{} {} ", state.search_results.len(), s.opt_results);
        if chunks[0].width > count_text.len() as u16 + 2 {
            let count_area = Rect {
                x: chunks[0].x + chunks[0].width - count_text.len() as u16 - 1,
                y: chunks[0].y,
                width: count_text.len() as u16 + 1,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(Line::styled(
                    count_text,
                    Style::default().fg(theme.fg_dim),
                )),
                count_area,
            );
        }
    }

    // Results list
    if state.search_results.is_empty() {
        let msg = if state.search_query.is_empty() {
            s.opt_search_empty.to_string()
        } else {
            s.opt_no_results.to_string()
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(""),
                Line::raw(""),
                Line::styled(msg, Style::default().fg(theme.fg_dim)),
            ])
            .alignment(Alignment::Center)
            .style(theme.block_style()),
            chunks[1],
        );
        return;
    }

    render_option_list(
        frame,
        state,
        theme,
        &state.search_results,
        state.search_selected,
        state.search_scroll,
        chunks[1],
    );
}

fn render_browse(
    frame: &mut Frame,
    state: &OptionsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if state.tree_rows.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(""),
                Line::styled(
                    format!("  {}", s.opt_empty),
                    Style::default().fg(theme.fg_dim),
                ),
            ])
            .style(theme.block_style()),
            area,
        );
        return;
    }

    // Hint line at top
    let chunks = Layout::vertical([
        Constraint::Length(1), // Hint
        Constraint::Min(3),   // Tree
    ])
    .split(area);

    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("  {}", s.opt_browse_hint),
            Style::default().fg(theme.fg_dim),
        ))
        .style(theme.block_style()),
        chunks[0],
    );

    let visible_height = chunks[1].height as usize;
    let mut scroll = state.tree_scroll;
    if state.tree_selected >= scroll + visible_height {
        scroll = state.tree_selected + 1 - visible_height;
    }
    if state.tree_selected < scroll {
        scroll = state.tree_selected;
    }

    let items: Vec<ListItem> = state
        .tree_rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, row)| {
            let is_selected = i == state.tree_selected;
            let indent = "  ".repeat(row.depth + 1);

            let (icon, name_style) = if row.is_leaf {
                let tc = type_color(
                    &state.options.get(row.option_idx.unwrap_or(0))
                        .map(|o| o.type_str.as_str())
                        .unwrap_or(""),
                    theme,
                );
                ("• ", Style::default().fg(tc))
            } else if row.is_expanded {
                ("▼ ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
            } else {
                ("▶ ", Style::default().fg(theme.accent))
            };

            let count_str = if !row.is_leaf {
                format!(" ({})", row.child_count)
            } else {
                String::new()
            };

            let style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };

            let name_final = if is_selected {
                style.add_modifier(Modifier::BOLD)
            } else {
                name_style
            };

            ListItem::new(Line::from(vec![
                Span::styled(indent, style),
                Span::styled(icon, if is_selected { style } else { name_style }),
                Span::styled(row.display_name.clone(), name_final),
                Span::styled(count_str, Style::default().fg(theme.fg_dim)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items).style(theme.block_style()), chunks[1]);
}

fn render_related(
    frame: &mut Frame,
    state: &OptionsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if state.related_options.is_empty() {
        let msg = if state.related_for_path.is_empty() {
            s.opt_related_empty
        } else {
            s.opt_no_results
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

    // Header showing parent path
    let chunks = Layout::vertical([
        Constraint::Length(2), // Header
        Constraint::Min(3),   // List
    ])
    .split(area);

    let parent_path = if let Some(dot) = state.related_for_path.rfind('.') {
        &state.related_for_path[..dot]
    } else {
        &state.related_for_path
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("  {} ", s.opt_related_label),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{}.*", parent_path), theme.text()),
            Span::styled(
                format!("  ({} {})", state.related_options.len(), s.opt_results),
                Style::default().fg(theme.fg_dim),
            ),
        ]))
        .style(theme.block_style()),
        chunks[0],
    );

    render_option_list(
        frame,
        state,
        theme,
        &state.related_options,
        state.related_selected,
        state.related_scroll,
        chunks[1],
    );
}

/// Shared list renderer for search results and related options
fn render_option_list(
    frame: &mut Frame,
    state: &OptionsState,
    theme: &Theme,
    indices: &[usize],
    selected: usize,
    scroll_offset: usize,
    area: Rect,
) {
    let visible_height = area.height as usize;
    let mut scroll = scroll_offset;
    if selected >= scroll + visible_height {
        scroll = selected + 1 - visible_height;
    }
    if selected < scroll {
        scroll = selected;
    }

    let path_width = (area.width as usize * 2 / 5).max(20).min(60);
    let type_width = 14usize.min(area.width as usize / 5);

    let items: Vec<ListItem> = indices
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .filter(|(_, &opt_idx)| opt_idx < state.options.len())
        .map(|(i, &opt_idx)| {
            let opt = &state.options[opt_idx];
            let is_selected = i == selected;

            // Truncate path for display (option paths are ASCII but be safe)
            let path_display = if opt.path.len() > path_width {
                let start = opt.path.len() - path_width + 1;
                let safe_start = (start..).find(|&i| opt.path.is_char_boundary(i)).unwrap_or(opt.path.len());
                format!("…{}", &opt.path[safe_start..])
            } else {
                format!("{:<width$}", opt.path, width = path_width)
            };

            let type_display = if opt.type_str.len() > type_width {
                let trunc = safe_truncate(&opt.type_str, type_width.saturating_sub(1));
                format!("{}…", trunc)
            } else {
                format!("{:<width$}", opt.type_str, width = type_width)
            };

            let desc_width = (area.width as usize).saturating_sub(path_width + type_width + 6);
            let desc: String = if opt.description.len() > desc_width {
                if desc_width > 1 {
                    let trunc = safe_truncate(&opt.description, desc_width.saturating_sub(1));
                    format!("{}…", trunc)
                } else {
                    String::new()
                }
            } else {
                opt.description.clone()
            };

            let style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };

            let tc = type_color(&opt.type_str, theme);

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {}", path_display),
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.accent)
                    },
                ),
                Span::styled(format!(" {} ", type_display), Style::default().fg(tc)),
                Span::styled(
                    desc,
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
    state: &OptionsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let opt_idx = match state.detail_option_idx {
        Some(idx) if idx < state.options.len() => idx,
        _ => return,
    };
    let opt = &state.options[opt_idx];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));

    // Option path (title)
    lines.push(Line::from(vec![
        Span::styled(
            "  ",
            Style::default(),
        ),
        Span::styled(
            opt.path.clone(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Type
    let tc = type_color(&opt.type_str, theme);
    lines.push(Line::from(vec![
        Span::styled(format!("  {} ", s.opt_detail_type), Style::default().fg(theme.fg_dim)),
        Span::styled(opt.type_str.clone(), Style::default().fg(tc).add_modifier(Modifier::BOLD)),
    ]));

    // Default
    if let Some(ref def) = opt.default_str {
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", s.opt_detail_default), Style::default().fg(theme.fg_dim)),
            Span::styled(truncate_value(def, area.width as usize - 20), theme.text()),
        ]));
    }

    // Example
    if let Some(ref ex) = opt.example_str {
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", s.opt_detail_example), Style::default().fg(theme.fg_dim)),
            Span::styled(truncate_value(ex, area.width as usize - 20), Style::default().fg(theme.warning)),
        ]));
    }

    // Current value
    lines.push(Line::raw(""));
    if state.current_value_loading {
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", s.opt_detail_current), Style::default().fg(theme.fg_dim)),
            Span::styled(s.opt_current_loading, Style::default().fg(theme.fg_dim)),
        ]));
    } else if let Some(ref val) = state.current_value {
        let val_color = if opt.default_str.as_deref() == Some(val.as_str()) {
            theme.fg_dim // Same as default, dim
        } else {
            theme.success // Different from default, highlight
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", s.opt_detail_current), Style::default().fg(theme.fg_dim)),
            Span::styled(truncate_value(val, area.width as usize - 20), Style::default().fg(val_color).add_modifier(Modifier::BOLD)),
        ]));
    }

    // Read-only marker
    if opt.read_only {
        lines.push(Line::styled(
            format!("  ⚠ {}", s.opt_read_only),
            Style::default().fg(theme.warning),
        ));
    }

    // Description
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  {}", s.opt_detail_desc),
        Style::default()
            .fg(theme.fg_dim)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    // Word-wrap description
    let wrap_width = (area.width as usize).saturating_sub(6).max(10);
    for wrapped_line in word_wrap(&opt.description, wrap_width) {
        lines.push(Line::styled(format!("    {}", wrapped_line), theme.text()));
    }

    // Declared in
    if !opt.declared_in.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("  {}", s.opt_detail_declared),
            Style::default().fg(theme.fg_dim),
        ));
        for decl in &opt.declared_in {
            // Shorten nixpkgs paths for readability
            let short = decl
                .replace("<nixpkgs/", "<")
                .replace("/nixos/modules/", ".../");
            lines.push(Line::styled(
                format!("    {}", short),
                Style::default().fg(theme.fg_dim),
            ));
        }
    }

    // Keybind hints
    lines.push(Line::raw(""));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  [Esc] {}  [r] {}  [j/k] {}", s.back, s.opt_related_label, s.navigate),
        Style::default().fg(theme.fg_dim),
    ));

    // Apply scroll
    let scroll = state.detail_scroll.min(lines.len().saturating_sub(1));
    let visible_lines: Vec<Line> = lines.into_iter().skip(scroll).collect();

    frame.render_widget(
        Paragraph::new(visible_lines).style(theme.block_style()),
        area,
    );
}

fn truncate_value(s: &str, max_width: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max_width && max_width > 3 {
        format!("{}...", &first_line[..max_width - 3])
    } else if s.lines().count() > 1 {
        format!("{} ...", first_line)
    } else {
        first_line.to_string()
    }
}

fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();
        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
