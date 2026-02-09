//! Storage module — Nix Store Analysis & Cleanup
//!
//! Sub-tabs: Dashboard, Explorer, Clean, History.
//! Shows disk usage, store path analysis, cleanup tools, and history.

use crate::config::Language;
use crate::i18n;
use crate::nix::storage::{self, CleanAction, DiskUsage, HistoryEntry, StoreInfo, StorePath};
use crate::types::format_bytes;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Frame,
};
use std::sync::mpsc;
use crate::types::FlashMessage;

// ── Sub-tabs ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StoSubTab {
    #[default]
    Dashboard,
    Explorer,
    Clean,
    History,
}

impl StoSubTab {
    pub fn all() -> &'static [StoSubTab] {
        &[
            StoSubTab::Dashboard,
            StoSubTab::Explorer,
            StoSubTab::Clean,
            StoSubTab::History,
        ]
    }

    pub fn index(&self) -> usize {
        match self {
            StoSubTab::Dashboard => 0,
            StoSubTab::Explorer => 1,
            StoSubTab::Clean => 2,
            StoSubTab::History => 3,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            StoSubTab::Dashboard => s.sto_dashboard,
            StoSubTab::Explorer => s.sto_explorer,
            StoSubTab::Clean => s.sto_clean,
            StoSubTab::History => s.sto_history,
        }
    }
}

// ── Popup state ──

#[derive(Debug, Clone)]
pub enum StoPopupState {
    None,
    ConfirmAction {
        action: CleanAction,
    },
    ActionResult {
        title: String,
        message: String,
    },
}

// ── Explorer filter ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplorerFilter {
    #[default]
    All,
    Live,
    Dead,
}

impl ExplorerFilter {
    pub fn next(&self) -> Self {
        match self {
            ExplorerFilter::All => ExplorerFilter::Live,
            ExplorerFilter::Live => ExplorerFilter::Dead,
            ExplorerFilter::Dead => ExplorerFilter::All,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            ExplorerFilter::All => s.sto_filter_all,
            ExplorerFilter::Live => s.sto_filter_live,
            ExplorerFilter::Dead => s.sto_filter_dead,
        }
    }
}

// ── Module state ──

pub struct StorageState {
    pub active_sub_tab: StoSubTab,

    // Data
    pub info: StoreInfo,
    pub history: Vec<HistoryEntry>,
    pub load_error: Option<String>,
    pub loaded: bool,
    pub loading: bool,
    load_rx: Option<mpsc::Receiver<StoreInfo>>,

    // Explorer
    pub explorer_selected: usize,
    pub explorer_filter: ExplorerFilter,
    pub explorer_search: String,
    pub explorer_search_active: bool,

    // Clean
    pub clean_selected: usize,

    // History
    pub history_scroll: usize,

    // Popup & flash
    pub popup: StoPopupState,
    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

impl StorageState {
    pub fn new() -> Self {
        let history = storage::load_history();

        Self {
            active_sub_tab: StoSubTab::Dashboard,
            info: StoreInfo::default(),
            history,
            load_error: None,
            loaded: false,
            loading: false,
            load_rx: None,
            explorer_selected: 0,
            explorer_filter: ExplorerFilter::default(),
            explorer_search: String::new(),
            explorer_search_active: false,
            clean_selected: 0,
            history_scroll: 0,
            popup: StoPopupState::None,
            lang: Language::English,
            flash_message: None,
        }
    }

    /// Kick off background loading (non-blocking). Called from render.
    pub fn start_loading(&mut self) {
        if self.loaded || self.loading {
            return;
        }
        self.loading = true;
        let (tx, rx) = mpsc::channel();
        self.load_rx = Some(rx);
        std::thread::spawn(move || {
            let info = storage::load_store_info();
            let _ = tx.send(info);
        });
    }

    /// Poll for background load results. Called from update_timers (non-blocking).
    pub fn poll_load(&mut self) {
        if let Some(ref rx) = self.load_rx {
            match rx.try_recv() {
                Ok(info) => {
                    self.info = info;
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still loading
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.load_error = Some(crate::i18n::get_strings(self.lang).thread_crashed.to_string());
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
            }
        }
    }

    pub fn refresh(&mut self) {
        self.load_rx = None;
        self.loading = false;

        self.info = storage::load_store_info();
        self.history = storage::load_history();
        self.loaded = true;
        self.explorer_selected = 0;
    }

    fn filtered_paths(&self) -> Vec<&StorePath> {
        self.info
            .paths
            .iter()
            .filter(|p| match self.explorer_filter {
                ExplorerFilter::All => true,
                ExplorerFilter::Live => !p.is_dead,
                ExplorerFilter::Dead => p.is_dead,
            })
            .filter(|p| {
                if self.explorer_search.is_empty() {
                    true
                } else {
                    let q = self.explorer_search.to_lowercase();
                    p.name.to_lowercase().contains(&q)
                }
            })
            .collect()
    }

    fn show_flash(&mut self, msg: &str, is_error: bool) {
        self.flash_message = Some(FlashMessage::new(msg.to_string(), is_error));
    }

    fn execute_action(&mut self, action: CleanAction) {
        let s = crate::i18n::get_strings(self.lang);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

        match action {
            CleanAction::GarbageCollect => {
                match storage::run_gc() {
                    Ok(result) => {
                        let msg = format!(
                            "GC complete: {} paths removed, {} freed",
                            result.paths_removed,
                            format_bytes(result.bytes_freed)
                        );
                        let _ = storage::save_history_entry(HistoryEntry {
                            timestamp: now,
                            action: s.stor_gc_action.to_string(),
                            freed_bytes: result.bytes_freed,
                            paths_removed: result.paths_removed,
                        });
                        self.popup = StoPopupState::ActionResult {
                            title: s.stor_gc_title.to_string(),
                            message: msg,
                        };
                    }
                    Err(e) => {
                        self.show_flash(&format!("{}: {}", s.error, e), true);
                    }
                }
            }
            CleanAction::Optimise => {
                match storage::run_optimise() {
                    Ok(result) => {
                        let msg = s.stor_optimize_result.replace("{}", &format_bytes(result.bytes_saved));
                        let _ = storage::save_history_entry(HistoryEntry {
                            timestamp: now,
                            action: s.stor_optimize_action.to_string(),
                            freed_bytes: result.bytes_saved,
                            paths_removed: 0,
                        });
                        self.popup = StoPopupState::ActionResult {
                            title: s.stor_optimize_title.to_string(),
                            message: msg,
                        };
                    }
                    Err(e) => {
                        self.show_flash(&format!("{}: {}", s.error, e), true);
                    }
                }
            }
            CleanAction::FullClean => {
                match storage::run_gc_full() {
                    Ok(result) => {
                        let msg = s.stor_fullclean_result
                                .replacen("{}", &result.paths_removed.to_string(), 1)
                                .replacen("{}", &format_bytes(result.bytes_freed), 1);
                        let _ = storage::save_history_entry(HistoryEntry {
                            timestamp: now,
                            action: s.stor_fullclean_action.to_string(),
                            freed_bytes: result.bytes_freed,
                            paths_removed: result.paths_removed,
                        });
                        self.popup = StoPopupState::ActionResult {
                            title: s.stor_fullclean_title.to_string(),
                            message: msg,
                        };
                    }
                    Err(e) => {
                        self.show_flash(&format!("{}: {}", s.error, e), true);
                    }
                }
            }
        }

        // Refresh data after action
        self.info = storage::load_store_info();
        self.history = storage::load_history();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Flash expiry
        if let Some(msg) = &self.flash_message {
            if msg.is_expired(3) {
                self.flash_message = None;
            }
        }

        // Popup handling
        match &self.popup {
            StoPopupState::ConfirmAction { action } => {
                let action = *action;
                match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        self.popup = StoPopupState::None;
                        self.execute_action(action);
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        self.popup = StoPopupState::None;
                    }
                    _ => {}
                }
                return Ok(());
            }
            StoPopupState::ActionResult { .. } => {
                match key.code {
                    KeyCode::Enter | KeyCode::Esc | KeyCode::Char('o') => {
                        self.popup = StoPopupState::None;
                    }
                    _ => {}
                }
                return Ok(());
            }
            StoPopupState::None => {}
        }

        // Sub-tab switching
        match key.code {
            KeyCode::F(1) => { self.active_sub_tab = StoSubTab::Dashboard; return Ok(()); }
            KeyCode::F(2) => { self.active_sub_tab = StoSubTab::Explorer; return Ok(()); }
            KeyCode::F(3) => { self.active_sub_tab = StoSubTab::Clean; return Ok(()); }
            KeyCode::F(4) => { self.active_sub_tab = StoSubTab::History; return Ok(()); }
            _ => {}
        }

        match self.active_sub_tab {
            StoSubTab::Dashboard => self.handle_dashboard_key(key),
            StoSubTab::Explorer => self.handle_explorer_key(key),
            StoSubTab::Clean => self.handle_clean_key(key),
            StoSubTab::History => self.handle_history_key(key),
        }
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char('r') = key.code { self.refresh() }
        Ok(())
    }

    fn handle_explorer_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.explorer_search_active {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.explorer_search_active = false;
                }
                KeyCode::Backspace => {
                    self.explorer_search.pop();
                    self.explorer_selected = 0;
                }
                KeyCode::Char(c) => {
                    self.explorer_search.push(c);
                    self.explorer_selected = 0;
                }
                _ => {}
            }
            return Ok(());
        }

        let count = self.filtered_paths().len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if count > 0 && self.explorer_selected < count - 1 {
                    self.explorer_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.explorer_selected = self.explorer_selected.saturating_sub(1);
            }
            KeyCode::Char('f') => {
                self.explorer_filter = self.explorer_filter.next();
                self.explorer_selected = 0;
            }
            KeyCode::Char('/') => {
                self.explorer_search_active = true;
            }
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('g') => self.explorer_selected = 0,
            KeyCode::Char('G') => {
                if count > 0 {
                    self.explorer_selected = count - 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_clean_key(&mut self, key: KeyEvent) -> Result<()> {
        let action_count = CleanAction::all().len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.clean_selected < action_count - 1 {
                    self.clean_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.clean_selected = self.clean_selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                let action = CleanAction::all()[self.clean_selected];
                self.popup = StoPopupState::ConfirmAction { action };
            }
            KeyCode::Char('r') => self.refresh(),
            _ => {}
        }
        Ok(())
    }

    fn handle_history_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.history_scroll = self.history_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.history_scroll = self.history_scroll.saturating_sub(1);
            }
            KeyCode::Char('g') => self.history_scroll = 0,
            KeyCode::Char('G') => {
                if !self.history.is_empty() {
                    self.history_scroll = self.history.len().saturating_sub(1);
                }
            }
            KeyCode::Char('r') => {
                self.history = storage::load_history();
            }
            _ => {}
        }
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════
// RENDERING
// ════════════════════════════════════════════════════════════════════

pub fn render(
    frame: &mut Frame,
    state: &mut StorageState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    // Kick off background loading on first render (non-blocking)
    let s = crate::i18n::get_strings(lang);
    state.start_loading();
    if state.loading && !state.loaded {
        frame.render_widget(Block::default().style(theme.block_style()), area);
        let loading_text = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled("⏳  Loading Nix Store ...", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Line::raw(""),
            Line::styled(
                s.stor_scanning_desc,
                theme.text_dim(),
            ),
            Line::styled(
                s.stor_scanning_hint,
                theme.text_dim(),
            ),
        ];
        let loading = Paragraph::new(loading_text)
            .alignment(Alignment::Center)
            .style(theme.block_style());
        frame.render_widget(loading, area);
        return;
    }

    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(8),
    ])
    .split(area);

    render_sub_tabs(frame, state, theme, lang, layout[0]);

    match state.active_sub_tab {
        StoSubTab::Dashboard => render_dashboard(frame, state, theme, lang, layout[1]),
        StoSubTab::Explorer => render_explorer(frame, state, theme, lang, layout[1]),
        StoSubTab::Clean => render_clean(frame, state, theme, lang, layout[1]),
        StoSubTab::History => render_history(frame, state, theme, lang, layout[1]),
    }

    // Popups
    match &state.popup {
        StoPopupState::ConfirmAction { action } => {
            render_confirm_popup(frame, *action, theme, lang, area);
        }
        StoPopupState::ActionResult { title, message } => {
            let content = vec![
                Line::raw(""),
                Line::styled(message.as_str(), theme.text()),
                Line::raw(""),
            ];
            widgets::render_popup(frame, title, content, &[("OK", 'o')], theme, area);
        }
        StoPopupState::None => {}
    }

    if let Some(msg) = &state.flash_message {
        widgets::render_flash_message(frame, &msg.text, msg.is_error, theme, area);
    }
}

fn render_sub_tabs(
    frame: &mut Frame,
    state: &StorageState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let tab_titles: Vec<Line> = StoSubTab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let style = if state.active_sub_tab == *tab {
                theme.tab_active()
            } else {
                theme.tab_inactive()
            };
            Line::styled(format!("[F{}] {}", i + 1, tab.label(lang)), style)
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(state.active_sub_tab.index())
        .divider(" │ ")
        .style(theme.text());

    let tabs_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(4),
        height: 1,
    };
    frame.render_widget(tabs, tabs_area);
}

// ── Dashboard ──

fn render_dashboard(
    frame: &mut Frame,
    state: &StorageState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let info = &state.info;

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.sto_dashboard))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    let bar_width = (inner.width as usize).saturating_sub(6).min(40);

    // ── Disk Usage Section ──
    lines.push(Line::styled(
        format!("  ── {} ──", s.sto_disk_title),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    // Show /nix/store disk if separate from root
    if let Some(disk) = &info.disk_store {
        lines.push(make_disk_line("/nix/store", disk, bar_width, theme));
        lines.push(make_bar_line(disk.percent, bar_width, theme));
        lines.push(Line::raw(""));
    }

    // Show root disk
    if let Some(disk) = &info.disk_root {
        let label = if info.disk_store.is_some() { "/" } else { "/ (incl. /nix/store)" };
        lines.push(make_disk_line(label, disk, bar_width, theme));
        lines.push(make_bar_line(disk.percent, bar_width, theme));
        lines.push(Line::raw(""));
    }

    // ── Store Breakdown Section ──
    lines.push(Line::styled(
        format!("  ── {} ──", s.sto_breakdown_title),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    if info.has_sizes {
        // Total
        lines.push(Line::from(vec![
            Span::styled("    Total    ", theme.text()),
            Span::styled(
                format!("{:>6} {}   ", format_number(info.total_paths), s.sto_paths),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format_bytes(info.total_size), Style::default().fg(theme.fg)),
        ]));

        // Live
        let live_pct = if info.total_size > 0 {
            (info.live_size as f64 / info.total_size as f64) * 100.0
        } else {
            0.0
        };
        lines.push(Line::from(vec![
            Span::styled("    Live     ", Style::default().fg(theme.success)),
            Span::styled(
                format!("{:>6} {}   ", format_number(info.live_paths), s.sto_paths),
                Style::default().fg(theme.success),
            ),
            Span::styled(
                format!("{}  ", format_bytes(info.live_size)),
                Style::default().fg(theme.success),
            ),
            Span::styled(format!("{:.0}%", live_pct), Style::default().fg(theme.success)),
        ]));

        // Dead
        let dead_pct = if info.total_size > 0 {
            (info.dead_size as f64 / info.total_size as f64) * 100.0
        } else {
            0.0
        };
        if info.dead_paths > 0 {
            lines.push(Line::from(vec![
                Span::styled("    Dead     ", Style::default().fg(theme.error)),
                Span::styled(
                    format!("{:>6} {}   ", format_number(info.dead_paths), s.sto_paths),
                    Style::default().fg(theme.error),
                ),
                Span::styled(
                    format!("{}  ", format_bytes(info.dead_size)),
                    Style::default().fg(theme.error),
                ),
                Span::styled(
                    format!("{:.0}%  ← {}", dead_pct, s.sto_reclaimable),
                    Style::default().fg(theme.error),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("    Dead     ", Style::default().fg(theme.success)),
                Span::styled(
                    format!("     0 {}   ", s.sto_paths),
                    Style::default().fg(theme.success),
                ),
                Span::styled("0 B  ✓", Style::default().fg(theme.success)),
            ]));
        }
    } else {
        // No size info
        lines.push(Line::from(vec![
            Span::styled("    Total    ", theme.text()),
            Span::styled(
                format!("{} {}", format_number(info.total_paths), s.sto_paths),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    Live     ", Style::default().fg(theme.success)),
            Span::styled(
                format!("{} {}", format_number(info.live_paths), s.sto_paths),
                Style::default().fg(theme.success),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    Dead     ", Style::default().fg(theme.error)),
            Span::styled(
                format!("{} {}", format_number(info.dead_paths), s.sto_paths),
                Style::default().fg(theme.error),
            ),
        ]));
        lines.push(Line::styled(
            format!("    ({})", s.sto_no_sizes_hint),
            theme.text_dim(),
        ));
    }

    lines.push(Line::raw(""));

    // ── Top Paths Section ──
    if info.has_sizes && !info.paths.is_empty() {
        lines.push(Line::styled(
            format!("  ── {} ──", s.sto_top_paths_title),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::raw(""));

        let max_size = info.paths.first().map(|p| p.size).unwrap_or(1).max(1);
        let name_width = (inner.width as usize).saturating_sub(30).clamp(10, 40);
        let mini_bar_width: usize = 12;

        for (i, path) in info.paths.iter().take(10).enumerate() {
            let fill = ((path.size as f64 / max_size as f64) * mini_bar_width as f64) as usize;
            let empty = mini_bar_width.saturating_sub(fill);
            let bar = format!("{}{}", "█".repeat(fill), "░".repeat(empty));

            let name = if path.name.len() > name_width {
                format!("{}…", &path.name[..name_width - 1])
            } else {
                format!("{:<width$}", path.name, width = name_width)
            };

            let dead_marker = if path.is_dead { " ✗" } else { "" };
            let dead_color = if path.is_dead { theme.error } else { theme.fg_dim };

            lines.push(Line::from(vec![
                Span::styled(format!("  {:>2}  ", i + 1), Style::default().fg(theme.fg_dim)),
                Span::styled(name, Style::default().fg(theme.fg)),
                Span::styled(format!("  {:>8}", format_bytes(path.size)), Style::default().fg(theme.accent)),
                Span::styled(format!("  {}", bar), Style::default().fg(dead_color)),
                Span::styled(dead_marker.to_string(), Style::default().fg(theme.error)),
            ]));
        }

        if info.paths.len() > 10 {
            lines.push(Line::styled(
                format!("    … {} → [F2]", s.sto_more_in_explorer),
                theme.text_dim(),
            ));
        }
    }

    lines.push(Line::raw(""));

    // ── Recommendations Section ──
    let mut recs: Vec<Line> = Vec::new();

    if info.dead_size > 0 {
        recs.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(theme.warning)),
            Span::styled(
                format!("{} {} → [F3]", format_bytes(info.dead_size), s.sto_rec_gc),
                Style::default().fg(theme.warning),
            ),
        ]));
    }

    if info.total_paths > 500 {
        recs.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(theme.accent)),
            Span::styled(s.sto_rec_optimise, Style::default().fg(theme.accent)),
        ]));
    }

    let (last_cleanup, _) = storage::history_summary(&state.history);
    if last_cleanup.is_none() {
        recs.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(theme.fg_dim)),
            Span::styled(s.sto_rec_never_cleaned, Style::default().fg(theme.fg_dim)),
        ]));
    }

    // Always show tip about generations
    recs.push(Line::from(vec![
        Span::styled("  ℹ ", Style::default().fg(theme.fg_dim)),
        Span::styled(s.sto_tip_generations, theme.text_dim()),
    ]));

    if !recs.is_empty() {
        lines.push(Line::styled(
            format!("  ── {} ──", s.sto_recommendations),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::raw(""));
        lines.extend(recs);
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner,
    );
}

fn make_disk_line<'a>(label: &str, disk: &DiskUsage, _bar_width: usize, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("    {:<22}", label),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "{} / {}   ({} {})",
                format_bytes(disk.used),
                format_bytes(disk.total),
                format_bytes(disk.available),
                "free"
            ),
            Style::default().fg(theme.fg_dim),
        ),
    ])
}

fn make_bar_line<'a>(percent: f64, bar_width: usize, theme: &Theme) -> Line<'a> {
    let filled = ((percent / 100.0) * bar_width as f64) as usize;
    let empty = bar_width.saturating_sub(filled);
    let color = if percent < 60.0 {
        theme.success
    } else if percent < 80.0 {
        theme.warning
    } else {
        theme.error
    };

    Line::from(vec![
        Span::raw("    "),
        Span::styled("█".repeat(filled), Style::default().fg(color)),
        Span::styled("░".repeat(empty), Style::default().fg(theme.border)),
        Span::styled(format!("  {:.0}%", percent), Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

// ── Explorer ──

fn render_explorer(
    frame: &mut Frame,
    state: &StorageState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let info = &state.info;

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.sto_explorer))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 {
        return;
    }

    let paths = state.filtered_paths();

    // Filter bar (2 lines)
    let filter_line = if state.explorer_search_active {
        Line::from(vec![
            Span::styled(
                format!("  [f] {}  │  /", state.explorer_filter.label(lang)),
                theme.text_dim(),
            ),
            Span::styled(&state.explorer_search, Style::default().fg(theme.accent)),
            Span::styled("█", Style::default().fg(theme.accent)),
            Span::styled(format!("  │  {} {}", paths.len(), s.sto_shown), theme.text_dim()),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("  [f] {}  │  ", state.explorer_filter.label(lang)),
                theme.text_dim(),
            ),
            if state.explorer_search.is_empty() {
                Span::styled(format!("[/] {}  │  ", s.sto_search), theme.text_dim())
            } else {
                Span::styled(
                    format!("/{} │  ", state.explorer_search),
                    Style::default().fg(theme.accent),
                )
            },
            Span::styled(format!("{} {}", paths.len(), s.sto_shown), theme.text_dim()),
        ])
    };
    frame.render_widget(Paragraph::new(filter_line), Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    });

    // Header
    let header_y = inner.y + 1;
    let header = Line::from(vec![
        Span::styled(format!("  {:>4}  ", "#"), theme.text_dim()),
        Span::styled(format!("{:<30}  ", s.sto_col_name), theme.text_dim()),
        Span::styled(format!("{:>10}  ", s.sto_col_size), theme.text_dim()),
        Span::styled(format!("{:<6}", s.sto_col_status), theme.text_dim()),
    ]);
    frame.render_widget(Paragraph::new(header), Rect {
        x: inner.x,
        y: header_y,
        width: inner.width,
        height: 1,
    });

    // Path list
    let list_area = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };

    if paths.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(
                format!("  {}", s.sto_no_paths),
                theme.text_dim(),
            )),
            list_area,
        );
        return;
    }

    let visible = list_area.height as usize;
    let selected = state.explorer_selected.min(paths.len().saturating_sub(1));
    let scroll = if selected >= visible {
        selected - visible + 1
    } else {
        0
    };

    let name_width = (inner.width as usize).saturating_sub(28).clamp(10, 35);
    let mut lines: Vec<Line> = Vec::new();

    for (i, path) in paths.iter().enumerate().skip(scroll).take(visible) {
        let is_selected = i == selected;
        let marker = if is_selected { "▸ " } else { "  " };

        let name = if path.name.len() > name_width {
            format!("{}…", &path.name[..name_width - 1])
        } else {
            format!("{:<width$}", path.name, width = name_width)
        };

        let size_str = if info.has_sizes {
            format!("{:>10}", format_bytes(path.size))
        } else {
            format!("{:>10}", "-")
        };

        let (status_str, status_color) = if path.is_dead {
            ("dead", theme.error)
        } else {
            ("live", theme.success)
        };

        let row_style = if is_selected {
            theme.selected()
        } else {
            theme.text()
        };

        lines.push(Line::from(vec![
            Span::styled(marker, if is_selected { Style::default().fg(theme.accent) } else { theme.text() }),
            Span::styled(format!("{:>4}  ", i + 1), Style::default().fg(theme.fg_dim)),
            Span::styled(format!("{}  ", name), row_style),
            Span::styled(format!("{}  ", size_str), Style::default().fg(theme.accent)),
            Span::styled(status_str, Style::default().fg(status_color)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), list_area);
}

// ── Clean ──

fn render_clean(
    frame: &mut Frame,
    state: &StorageState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let info = &state.info;

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.sto_clean))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::styled(
        format!("  ── {} ──", s.sto_actions_title),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    let actions = CleanAction::all();
    for (i, action) in actions.iter().enumerate() {
        let is_selected = i == state.clean_selected;
        let marker = if is_selected { "▸ " } else { "  " };
        let bullet = if is_selected { "●" } else { "○" };

        let (title, desc, detail) = match action {
            CleanAction::GarbageCollect => (
                s.sto_gc_title,
                s.sto_gc_desc,
                if info.has_sizes && info.dead_size > 0 {
                    format!(
                        "      {} {} {}, ~{}",
                        s.sto_estimated,
                        info.dead_paths,
                        s.sto_paths,
                        format_bytes(info.dead_size)
                    )
                } else if info.dead_paths > 0 {
                    format!("      {} {} {}", s.sto_estimated, info.dead_paths, s.sto_paths)
                } else {
                    format!("      {}", s.sto_nothing_to_clean)
                },
            ),
            CleanAction::Optimise => (
                s.sto_optimise_title,
                s.sto_optimise_desc,
                String::new(),
            ),
            CleanAction::FullClean => (
                s.sto_full_title,
                s.sto_full_desc,
                format!("      {}", s.sto_full_warn),
            ),
        };

        let title_style = if is_selected {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            theme.text()
        };

        let sudo_hint = if action.needs_sudo() { " (sudo)" } else { "" };

        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(theme.accent)),
            Span::styled(format!("{} ", bullet), title_style),
            Span::styled(action.icon().to_string(), title_style),
            Span::styled(format!(" {}", title), title_style),
            Span::styled(sudo_hint, Style::default().fg(theme.warning)),
        ]));
        lines.push(Line::styled(
            format!("      {}", desc),
            theme.text_dim(),
        ));
        if !detail.is_empty() {
            let detail_color = if matches!(action, CleanAction::FullClean) {
                Style::default().fg(theme.warning)
            } else {
                Style::default().fg(theme.fg_dim)
            };
            lines.push(Line::styled(detail, detail_color));
        }
        lines.push(Line::raw(""));
    }

    // Hint
    lines.push(Line::styled(
        format!("  {}", s.sto_press_enter),
        theme.text_dim(),
    ));

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner,
    );
}

// ── History ──

fn render_history(
    frame: &mut Frame,
    state: &StorageState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.sto_history))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.history.is_empty() {
        let content = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(s.sto_no_history, theme.text_dim()),
            Line::raw(""),
            Line::styled(s.sto_no_history_hint, theme.text_dim()),
        ];
        frame.render_widget(
            Paragraph::new(content).alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Summary
    let (last_cleanup, total_freed) = storage::history_summary(&state.history);
    lines.push(Line::styled(
        format!("  ── {} ──", s.sto_summary),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    if let Some(last) = &last_cleanup {
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", s.sto_last_cleanup), theme.text_dim()),
            Span::styled(last.as_str(), Style::default().fg(theme.fg)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled(format!("  {} ", s.sto_total_freed), theme.text_dim()),
        Span::styled(
            format_bytes(total_freed),
            Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  ({} {})", state.history.len(), s.sto_cleanups),
            theme.text_dim(),
        ),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  ── {} ──", s.sto_history_log),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    let visible = inner.height as usize;
    let scroll = state.history_scroll.min(state.history.len().saturating_sub(1));

    for entry in state.history.iter().skip(scroll).take(visible.saturating_sub(8)) {
        let freed_str = if entry.freed_bytes > 0 {
            format!("  {} {}", s.sto_freed, format_bytes(entry.freed_bytes))
        } else {
            String::new()
        };
        let paths_str = if entry.paths_removed > 0 {
            format!("  ({} {})", entry.paths_removed, s.sto_paths_removed)
        } else {
            String::new()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {}  ", entry.timestamp), Style::default().fg(theme.fg_dim)),
            Span::styled(&entry.action, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(freed_str, Style::default().fg(theme.success)),
            Span::styled(paths_str, theme.text_dim()),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner,
    );
}

// ── Confirm Popup ──

fn render_confirm_popup(
    frame: &mut Frame,
    action: CleanAction,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let (title, desc) = match action {
        CleanAction::GarbageCollect => (s.sto_gc_title, s.sto_gc_desc),
        CleanAction::Optimise => (s.sto_optimise_title, s.sto_optimise_desc),
        CleanAction::FullClean => (s.sto_full_title, s.sto_full_desc),
    };

    let mut content = vec![
        Line::raw(""),
        Line::styled(
            format!("{} {}", action.icon(), title),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Line::styled(desc, theme.text_dim()),
        Line::raw(""),
    ];

    if action.needs_sudo() {
        content.push(Line::styled(
            s.sto_sudo_note,
            Style::default().fg(theme.warning),
        ));
        content.push(Line::raw(""));
    }

    if matches!(action, CleanAction::FullClean) {
        content.push(Line::styled(
            s.sto_full_warn,
            Style::default().fg(theme.error),
        ));
        content.push(Line::raw(""));
    }

    content.push(Line::styled(s.sto_confirm_question, theme.text()));

    widgets::render_popup(
        frame,
        s.sto_confirm_title,
        content,
        &[(s.yes, 'y'), (s.no, 'n')],
        theme,
        area,
    );
}
