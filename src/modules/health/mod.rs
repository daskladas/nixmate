//! Nix Doctor — System Health module
//!
//! Dashboard: health score + check list (read-only)
//! Fix: actionable items to heal your system
//!
//! Checks:
//! - Old generations (>30 days)
//! - Channel/flake freshness
//! - Nix store size
//! - Duplicate packages
//! - Root disk usage

use crate::config::Language;
use crate::i18n;
use crate::types::FlashMessage;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};
use std::sync::mpsc;

// ── Sub-tabs ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HealthSubTab {
    #[default]
    Dashboard,
    Fix,
}

impl HealthSubTab {
    pub fn all() -> &'static [HealthSubTab] {
        &[HealthSubTab::Dashboard, HealthSubTab::Fix]
    }

    pub fn index(&self) -> usize {
        match self {
            HealthSubTab::Dashboard => 0,
            HealthSubTab::Fix => 1,
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

// ── Health check severity ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Ok,
    Warning,
    Critical,
}

// ── Individual health check ──

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    #[allow(dead_code)] // Populated by checks, reserved for detail view
    pub description: String,
    pub severity: Severity,
    pub detail: String,
    pub fix_command: Option<String>,
    pub fix_description: Option<String>,
    /// Weight for score calculation (0-20)
    pub weight: u8,
    /// Whether this check has been fixed in current session
    pub fixed: bool,
}

// ── Module state ──

pub struct HealthState {
    pub sub_tab: HealthSubTab,
    pub checks: Vec<HealthCheck>,
    pub selected: usize,
    pub scanning: bool,
    pub scanned: bool,
    scan_rx: Option<mpsc::Receiver<Vec<HealthCheck>>>,

    // Fix action state
    pub fix_running: bool,
    pub fix_message: Option<FlashMessage>,
    fix_rx: Option<mpsc::Receiver<(usize, bool, String)>>,

    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

impl HealthState {
    pub fn new() -> Self {
        Self {
            sub_tab: HealthSubTab::Dashboard,
            checks: Vec::new(),
            selected: 0,
            scanning: false,
            scanned: false,
            scan_rx: None,
            fix_running: false,
            fix_message: None,
            fix_rx: None,
            lang: Language::English,
            flash_message: None,
        }
    }

    pub fn ensure_scanned(&mut self) {
        if self.scanned || self.scanning {
            return;
        }
        self.scanning = true;
        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        let lang = self.lang;

        std::thread::spawn(move || {
            let checks = run_health_checks(lang);
            let _ = tx.send(checks);
        });
    }

    pub fn rescan(&mut self) {
        self.scanned = false;
        self.scanning = false;
        self.scan_rx = None;
        self.checks.clear();
        self.ensure_scanned();
    }

    pub fn poll_scan(&mut self) {
        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(checks) => {
                    self.checks = checks;
                    self.scanning = false;
                    self.scanned = true;
                    self.scan_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scanning = false;
                    self.scanned = true;
                    self.scan_rx = None;
                }
            }
        }

        // Poll fix result
        if let Some(rx) = &self.fix_rx {
            match rx.try_recv() {
                Ok((idx, success, msg)) => {
                    self.fix_running = false;
                    self.fix_rx = None;
                    if success && idx < self.checks.len() {
                        self.checks[idx].fixed = true;
                        self.checks[idx].severity = Severity::Ok;
                    }
                    self.fix_message = Some(FlashMessage::new(msg, !success));
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.fix_running = false;
                    self.fix_rx = None;
                }
            }
        }

        // Clear fix message after 4s
        if let Some(m) = &self.fix_message {
            if m.is_expired(4) {
                self.fix_message = None;
            }
        }
    }

    pub fn health_score(&self) -> u8 {
        if self.checks.is_empty() {
            return 100;
        }
        let total_weight: u16 = self.checks.iter().map(|c| c.weight as u16).sum();
        if total_weight == 0 {
            return 100;
        }
        let lost: u16 = self
            .checks
            .iter()
            .map(|c| match c.severity {
                Severity::Ok => 0,
                Severity::Warning => (c.weight as u16) / 2,
                Severity::Critical => c.weight as u16,
            })
            .sum();
        let score = 100u16.saturating_sub((lost * 100) / total_weight);
        score as u8
    }

    fn start_fix(&mut self) {
        if self.fix_running || self.selected >= self.checks.len() {
            return;
        }
        let check = &self.checks[self.selected];
        if check.severity == Severity::Ok || check.fix_command.is_none() {
            return;
        }

        let Some(cmd) = check.fix_command.clone() else {
            return;
        };
        let idx = self.selected;
        self.fix_running = true;

        let (tx, rx) = mpsc::channel();
        self.fix_rx = Some(rx);
        let lang = self.lang;

        std::thread::spawn(move || {
            let output = std::process::Command::new("sh").args(["-c", &cmd]).output();
            let s = crate::i18n::get_strings(lang);
            match output {
                Ok(o) if o.status.success() => {
                    let _ = tx.send((idx, true, s.health_fix_success.to_string()));
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    let msg = if stderr.is_empty() {
                        s.health_fix_failed.to_string()
                    } else {
                        s.health_fix_error_detail
                            .replace("{}", stderr.lines().next().unwrap_or(""))
                    };
                    let _ = tx.send((idx, false, msg));
                }
                Err(e) => {
                    let _ = tx.send((idx, false, format!("{}: {}", s.health_fix_failed, e)));
                }
            }
        });
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('[') => {
                self.sub_tab = self.sub_tab.prev();
                return Ok(true);
            }
            KeyCode::Char(']') => {
                self.sub_tab = self.sub_tab.next();
                return Ok(true);
            }
            KeyCode::Char('r') => {
                self.rescan();
                return Ok(true);
            }
            _ => {}
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.checks.is_empty() {
                    self.selected = (self.selected + 1).min(self.checks.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Enter => {
                if self.sub_tab == HealthSubTab::Fix && !self.fix_running {
                    self.start_fix();
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

// ── Health checks implementation ──

fn run_health_checks(lang: Language) -> Vec<HealthCheck> {
    let s = crate::i18n::get_strings(lang);
    let mut checks = Vec::new();

    let mut c = check_old_generations(lang);
    c.name = s.health_name_old_gens.to_string();
    checks.push(c);

    let mut c = check_store_size(lang);
    c.name = s.health_name_store_size.to_string();
    checks.push(c);

    let mut c = check_disk_usage(lang);
    c.name = s.health_name_disk_usage.to_string();
    checks.push(c);

    let mut c = check_channel_freshness(lang);
    c.name = s.health_name_freshness.to_string();
    checks.push(c);

    let mut c = check_duplicate_packages(lang);
    c.name = s.health_name_duplicates.to_string();
    checks.push(c);

    checks
}

fn check_old_generations(lang: Language) -> HealthCheck {
    use std::process::Command;
    let s = crate::i18n::get_strings(lang);

    let output = Command::new("sh")
        .args([
            "-c",
            "nixos-rebuild list-generations 2>/dev/null | head -50",
        ])
        .output();

    let mut old_count = 0u32;
    let mut total_count = 0u32;

    if let Ok(o) = output {
        let stdout = String::from_utf8_lossy(&o.stdout);
        let now = chrono_now_days();

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("Generation") {
                continue;
            }
            total_count += 1;

            // Try to parse date from generation line
            // Format varies: "  42   2024-01-15 10:30:00" or similar
            if let Some(days) = extract_generation_age_days(line, now) {
                if days > 30 {
                    old_count += 1;
                }
            }
        }
    }

    // Fallback: count profiles
    if total_count == 0 {
        let profile_path = std::path::Path::new("/nix/var/nix/profiles");
        if let Ok(entries) = std::fs::read_dir(profile_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("system-") && name.ends_with("-link") {
                    total_count += 1;
                    // Check age via mtime
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                if elapsed.as_secs() > 30 * 86400 {
                                    old_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let (severity, detail) = if old_count == 0 {
        (
            Severity::Ok,
            s.health_detail_gens_all_recent
                .replace("{}", &total_count.to_string()),
        )
    } else if old_count > 10 {
        (
            Severity::Critical,
            s.health_detail_gens_older
                .replace("{}", &old_count.to_string()),
        )
    } else {
        (
            Severity::Warning,
            s.health_detail_gens_older
                .replace("{}", &old_count.to_string()),
        )
    };

    let fix_cmd = if old_count > 0 {
        Some("sudo nix-collect-garbage --delete-older-than 30d".to_string())
    } else {
        None
    };

    HealthCheck {
        name: s.health_name_old_gens.to_string(),
        description: s.health_desc_old_gens.to_string(),
        severity,
        detail,
        fix_command: fix_cmd,
        fix_description: Some(s.health_fix_old_gens.to_string()),
        weight: 15,
        fixed: false,
    }
}

fn check_store_size(lang: Language) -> HealthCheck {
    let s = crate::i18n::get_strings(lang);
    let store_path = std::path::Path::new("/nix/store");
    let mut size_gb = 0.0f64;

    // Fast method: use df on /nix/store
    if let Ok(output) = std::process::Command::new("df")
        .args(["-B1", "/nix/store"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().nth(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                if let Ok(used) = parts[2].parse::<u64>() {
                    size_gb = used as f64 / 1_073_741_824.0;
                }
            }
        }
    }

    // Also count store paths
    let path_count = if store_path.exists() {
        std::fs::read_dir(store_path)
            .map(|entries| entries.count())
            .unwrap_or(0)
    } else {
        0
    };

    let size_str = format!("{:.1}", size_gb);
    let (severity, detail) = if size_gb < 20.0 {
        (
            Severity::Ok,
            s.health_detail_store_ok
                .replacen("{}", &size_str, 1)
                .replacen("{}", &path_count.to_string(), 1),
        )
    } else if size_gb < 50.0 {
        (
            Severity::Warning,
            s.health_detail_store_warn.replace("{}", &size_str),
        )
    } else {
        (
            Severity::Critical,
            s.health_detail_store_crit.replace("{}", &size_str),
        )
    };

    HealthCheck {
        name: s.health_name_store_size.to_string(),
        description: s.health_desc_store_size.to_string(),
        severity,
        detail,
        fix_command: Some("sudo nix-collect-garbage -d".to_string()),
        fix_description: Some(s.health_fix_store_size.to_string()),
        weight: 20,
        fixed: false,
    }
}

fn check_disk_usage(lang: Language) -> HealthCheck {
    let s = crate::i18n::get_strings(lang);
    let mut usage_pct = 0u8;

    if let Ok(output) = std::process::Command::new("df")
        .args(["--output=pcent", "/"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().nth(1) {
            if let Ok(pct) = line.trim().trim_end_matches('%').parse::<u8>() {
                usage_pct = pct;
            }
        }
    }

    let pct_str = usage_pct.to_string();
    let (severity, detail) = if usage_pct < 80 {
        (
            Severity::Ok,
            s.health_detail_disk_ok.replace("{}", &pct_str),
        )
    } else if usage_pct < 90 {
        (
            Severity::Warning,
            s.health_detail_disk_warn.replace("{}", &pct_str),
        )
    } else {
        (
            Severity::Critical,
            s.health_detail_disk_crit.replace("{}", &pct_str),
        )
    };

    HealthCheck {
        name: s.health_name_disk_usage.to_string(),
        description: s.health_desc_disk.to_string(),
        severity,
        detail,
        fix_command: Some("sudo nix-collect-garbage --delete-older-than 7d".to_string()),
        fix_description: Some(s.health_fix_disk.to_string()),
        weight: 25,
        fixed: false,
    }
}

fn check_channel_freshness(lang: Language) -> HealthCheck {
    let s = crate::i18n::get_strings(lang);
    // Check when the current system was last built
    let system_path = std::path::Path::new("/run/current-system");
    let mut days_old = 0u64;

    if let Ok(meta) = std::fs::symlink_metadata(system_path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                days_old = elapsed.as_secs() / 86400;
            }
        }
    }

    let days_str = days_old.to_string();
    let (severity, detail) = if days_old <= 14 {
        (
            Severity::Ok,
            s.health_detail_fresh_ok.replace("{}", &days_str),
        )
    } else if days_old <= 30 {
        (
            Severity::Warning,
            s.health_detail_fresh_warn.replace("{}", &days_str),
        )
    } else {
        (
            Severity::Critical,
            s.health_detail_fresh_crit.replace("{}", &days_str),
        )
    };

    // Detect if flakes or channels for fix command
    let uses_flakes = crate::nix::detect::detect_system(None)
        .map(|s| s.uses_flakes)
        .unwrap_or(false);

    let fix_cmd = if uses_flakes {
        "cd /etc/nixos && sudo nix flake update && sudo nixos-rebuild switch".to_string()
    } else {
        "sudo nix-channel --update && sudo nixos-rebuild switch".to_string()
    };

    HealthCheck {
        name: s.health_name_freshness.to_string(),
        description: s.health_desc_freshness.to_string(),
        severity,
        detail,
        fix_command: Some(fix_cmd),
        fix_description: Some(s.health_fix_freshness.to_string()),
        weight: 20,
        fixed: false,
    }
}

fn check_duplicate_packages(lang: Language) -> HealthCheck {
    use std::collections::HashMap;
    let s = crate::i18n::get_strings(lang);

    let system_path = std::path::Path::new("/run/current-system");
    let mut duplicates = 0u32;
    let mut total = 0u32;

    if system_path.exists() {
        if let Ok(pkgs) = crate::nix::packages::get_packages(system_path) {
            total = pkgs.len() as u32;
            let mut name_counts: HashMap<String, u32> = HashMap::new();
            for pkg in &pkgs {
                *name_counts.entry(pkg.name.clone()).or_insert(0) += 1;
            }
            duplicates = name_counts.values().filter(|&&v| v > 1).count() as u32;
        }
    }

    let (severity, detail) = if duplicates == 0 {
        (
            Severity::Ok,
            s.health_detail_dupes_ok.replace("{}", &total.to_string()),
        )
    } else if duplicates < 5 {
        (
            Severity::Warning,
            s.health_detail_dupes_warn
                .replace("{}", &duplicates.to_string()),
        )
    } else {
        (
            Severity::Critical,
            s.health_detail_dupes_crit
                .replace("{}", &duplicates.to_string()),
        )
    };

    HealthCheck {
        name: s.health_name_duplicates.to_string(),
        description: s.health_desc_duplicates.to_string(),
        severity,
        detail,
        fix_command: None, // Can't auto-fix this easily
        fix_description: Some(s.health_fix_duplicates.to_string()),
        weight: 20,
        fixed: false,
    }
}

// ── Time helpers ──

fn chrono_now_days() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86400)
        .unwrap_or(0)
}

fn extract_generation_age_days(line: &str, now_days: u64) -> Option<u64> {
    // Try to find a date pattern YYYY-MM-DD in the line
    for word in line.split_whitespace() {
        if word.len() == 10 && word.chars().nth(4) == Some('-') && word.chars().nth(7) == Some('-')
        {
            let parts: Vec<&str> = word.split('-').collect();
            if parts.len() == 3 {
                let y: u64 = parts[0].parse().ok()?;
                let m: u64 = parts[1].parse().ok()?;
                let d: u64 = parts[2].parse().ok()?;
                // Rough day count since epoch
                let gen_days = (y - 1970) * 365 + (y - 1969) / 4 + (m - 1) * 30 + d;
                return Some(now_days.saturating_sub(gen_days));
            }
        }
    }
    None
}

// ── Rendering ──

pub fn render(frame: &mut Frame, state: &HealthState, theme: &Theme, lang: Language, area: Rect) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_health))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 6 || inner.width < 30 {
        return;
    }

    // Sub-tab bar
    let chunks = Layout::vertical([
        Constraint::Length(2), // Tab bar
        Constraint::Min(4),    // Content
    ])
    .split(inner);

    // Render tab bar
    let tab_titles: Vec<Line> = vec![
        Line::from(format!(" {} ", s.health_dashboard)),
        Line::from(format!(" {} ", s.health_fix)),
    ];
    let tab_idx = match state.sub_tab {
        HealthSubTab::Dashboard => 0,
        HealthSubTab::Fix => 1,
    };
    let tabs = Tabs::new(tab_titles)
        .select(tab_idx)
        .style(theme.tab_inactive())
        .highlight_style(theme.tab_active())
        .divider(" ");
    let tabs_area = widgets::render_sub_tab_nav(frame, theme, chunks[0]);
    frame.render_widget(tabs, tabs_area);

    if state.scanning {
        let lines = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(
                format!("  ⏳ {}...", s.health_scanning),
                Style::default().fg(theme.accent),
            ),
        ];
        frame.render_widget(Paragraph::new(lines).style(theme.block_style()), chunks[1]);
        return;
    }

    match state.sub_tab {
        HealthSubTab::Dashboard => render_dashboard(frame, state, theme, lang, chunks[1]),
        HealthSubTab::Fix => render_fix(frame, state, theme, lang, chunks[1]),
    }
}

fn render_dashboard(
    frame: &mut Frame,
    state: &HealthState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let score = state.health_score();

    let chunks = Layout::vertical([
        Constraint::Length(5), // Score display
        Constraint::Min(3),    // Check list
    ])
    .split(area);

    // Score display
    let score_color = if score >= 90 {
        theme.success
    } else if score >= 60 {
        theme.warning
    } else {
        theme.error
    };

    let score_label = if score >= 90 {
        s.health_excellent
    } else if score >= 75 {
        s.health_good
    } else if score >= 60 {
        s.health_fair
    } else {
        s.health_poor
    };

    // Score bar visualization
    let bar_width = (area.width as usize).saturating_sub(6).min(40);
    let filled = (bar_width * score as usize) / 100;
    let bar_filled: String = "█".repeat(filled);
    let bar_empty: String = "░".repeat(bar_width - filled);

    let score_lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                format!("  {} ", s.health_score_label),
                Style::default().fg(theme.fg_dim),
            ),
            Span::styled(
                format!("{}%", score),
                Style::default()
                    .fg(score_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  — {}", score_label),
                Style::default().fg(score_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(bar_filled, Style::default().fg(score_color)),
            Span::styled(bar_empty, Style::default().fg(theme.border)),
        ]),
        Line::raw(""),
    ];

    frame.render_widget(
        Paragraph::new(score_lines).style(theme.block_style()),
        chunks[0],
    );

    // Check list
    render_check_list(frame, state, theme, chunks[1], false);
}

fn render_fix(frame: &mut Frame, state: &HealthState, theme: &Theme, lang: Language, area: Rect) {
    let s = i18n::get_strings(lang);

    let has_fixable = state
        .checks
        .iter()
        .any(|c| c.severity != Severity::Ok && c.fix_command.is_some());

    let chunks = Layout::vertical([
        Constraint::Length(2), // Fix header
        Constraint::Min(3),    // Check list with fix info
        Constraint::Length(2), // Fix message
    ])
    .split(area);

    // Header
    let header_text = if has_fixable {
        format!("  {} [Enter] {}", s.health_fix_hint, s.health_fix_action)
    } else {
        format!("  ✓ {}", s.health_all_ok)
    };

    frame.render_widget(
        Paragraph::new(Line::styled(header_text, Style::default().fg(theme.fg_dim)))
            .style(theme.block_style()),
        chunks[0],
    );

    // Check list with fix details
    render_check_list(frame, state, theme, chunks[1], true);

    // Fix message
    if let Some(msg) = &state.fix_message {
        let color = if msg.is_error {
            theme.error
        } else {
            theme.success
        };
        frame.render_widget(
            Paragraph::new(Line::styled(
                format!("  {}", msg.text),
                Style::default().fg(color),
            ))
            .style(theme.block_style()),
            chunks[2],
        );
    } else if state.fix_running {
        frame.render_widget(
            Paragraph::new(Line::styled(
                format!("  ⏳ {}", s.health_applying_fix),
                Style::default().fg(theme.accent),
            ))
            .style(theme.block_style()),
            chunks[2],
        );
    }
}

fn render_check_list(
    frame: &mut Frame,
    state: &HealthState,
    theme: &Theme,
    area: Rect,
    show_fix_info: bool,
) {
    if state.checks.is_empty() {
        return;
    }

    let items: Vec<ListItem> = state
        .checks
        .iter()
        .enumerate()
        .map(|(i, check)| {
            let is_selected = i == state.selected;

            let icon = match check.severity {
                Severity::Ok => "✓",
                Severity::Warning => "⚠",
                Severity::Critical => "✗",
            };
            let icon_color = match check.severity {
                Severity::Ok => theme.success,
                Severity::Warning => theme.warning,
                Severity::Critical => theme.error,
            };

            let name_style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };

            let mut spans = vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(icon_color)),
                Span::styled(
                    format!("{:<22}", check.name),
                    if is_selected {
                        name_style.add_modifier(Modifier::BOLD)
                    } else {
                        name_style
                    },
                ),
                Span::styled(
                    check.detail.clone(),
                    if is_selected {
                        name_style
                    } else {
                        Style::default().fg(theme.fg_dim)
                    },
                ),
            ];

            if show_fix_info && check.severity != Severity::Ok {
                if let Some(fix_desc) = &check.fix_description {
                    spans.push(Span::styled(
                        format!("  → {}", fix_desc),
                        Style::default().fg(theme.accent_dim),
                    ));
                }
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).style(theme.block_style());
    frame.render_widget(list, area);
}
