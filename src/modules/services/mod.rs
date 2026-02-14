//! Services & Ports module — Server Dashboard
//!
//! Integrated into nixmate as an inline module.
//! Sub-tabs: Overview, Ports, Manage, Logs.
//! Shows systemd services, Docker/Podman containers, and open ports in one view.
//! Uses nixmate's global theme, i18n, and config.

use crate::config::Language;
use crate::i18n;
use crate::nix::services::{
    self, DashboardStats, EnableState, EntryKind, PortEntry, RunState, ServiceAction, ServiceEntry,
};
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
use std::sync::mpsc;

// ── Sub-tabs ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvcSubTab {
    #[default]
    Overview,
    Ports,
    Manage,
    Logs,
}

impl SvcSubTab {
    pub fn all() -> &'static [SvcSubTab] {
        &[
            SvcSubTab::Overview,
            SvcSubTab::Ports,
            SvcSubTab::Manage,
            SvcSubTab::Logs,
        ]
    }

    pub fn index(&self) -> usize {
        match self {
            SvcSubTab::Overview => 0,
            SvcSubTab::Ports => 1,
            SvcSubTab::Manage => 2,
            SvcSubTab::Logs => 3,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            SvcSubTab::Overview => s.svc_overview,
            SvcSubTab::Ports => s.svc_ports,
            SvcSubTab::Manage => s.svc_manage,
            SvcSubTab::Logs => s.svc_logs,
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

// ── Popup state ──

#[derive(Debug, Clone)]
pub enum SvcPopupState {
    None,
    ConfirmAction {
        entry_name: String,
        entry_display: String,
        entry_kind: EntryKind,
        action: ServiceAction,
    },
}

// ── Filter mode ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    All,
    Active, // Running + Restarting
    Systemd,
    Containers, // Docker + Podman
    Failed,
}

impl FilterKind {
    pub fn next(&self) -> Self {
        match self {
            FilterKind::All => FilterKind::Active,
            FilterKind::Active => FilterKind::Systemd,
            FilterKind::Systemd => FilterKind::Containers,
            FilterKind::Containers => FilterKind::Failed,
            FilterKind::Failed => FilterKind::All,
        }
    }

    pub fn label<'a>(&self, lang: Language) -> &'a str {
        let s = i18n::get_strings(lang);
        match self {
            FilterKind::All => s.svc_filter_all,
            FilterKind::Active => s.svc_filter_active,
            FilterKind::Systemd => s.svc_filter_systemd,
            FilterKind::Containers => s.svc_filter_containers,
            FilterKind::Failed => s.svc_filter_failed,
        }
    }
}

// ── Module state ──

/// Result type for background loading
type SvcLoadResult = Result<(Vec<ServiceEntry>, Vec<PortEntry>, DashboardStats)>;

pub struct ServicesState {
    // Data
    pub entries: Vec<ServiceEntry>,
    pub ports: Vec<PortEntry>,
    pub stats: DashboardStats,
    pub logs: Vec<String>,
    pub load_error: Option<String>,
    pub loaded: bool,
    pub loading: bool,
    load_rx: Option<mpsc::Receiver<SvcLoadResult>>,

    // Navigation
    pub active_sub_tab: SvcSubTab,

    // Overview
    pub overview_selected: usize,
    pub filter_kind: FilterKind,
    pub search_text: String,
    pub search_active: bool,

    // Ports
    pub ports_selected: usize,

    // Manage
    pub manage_action_idx: usize,

    // Logs
    pub logs_scroll: usize,

    // Popup
    pub popup: SvcPopupState,

    // Flash
    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

impl ServicesState {
    /// Initialize. Always succeeds (graceful degradation).
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            ports: Vec::new(),
            stats: DashboardStats::default(),
            logs: Vec::new(),
            load_error: None,
            loaded: false,
            loading: false,
            load_rx: None,
            active_sub_tab: SvcSubTab::Overview,
            overview_selected: 0,
            filter_kind: FilterKind::Active,
            search_text: String::new(),
            search_active: false,
            ports_selected: 0,
            manage_action_idx: 0,
            logs_scroll: 0,
            popup: SvcPopupState::None,
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
            let result = services::load_dashboard();
            let _ = tx.send(result);
        });
    }

    /// Poll for background load results. Called from update_timers (non-blocking).
    pub fn poll_load(&mut self) {
        if let Some(ref rx) = self.load_rx {
            match rx.try_recv() {
                Ok(Ok((e, p, s))) => {
                    self.entries = e;
                    self.ports = p;
                    self.stats = s;
                    self.load_error = None;
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
                Ok(Err(e)) => {
                    self.load_error = Some(e.to_string());
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still loading — do nothing
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.load_error = Some(
                        crate::i18n::get_strings(self.lang)
                            .thread_crashed
                            .to_string(),
                    );
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
            }
        }
    }

    /// Reload all data (blocking — only for user-triggered refresh)
    pub fn refresh(&mut self) {
        // Drop any pending background load
        self.load_rx = None;
        self.loading = false;

        match services::load_dashboard() {
            Ok((e, p, s)) => {
                self.entries = e;
                self.ports = p;
                self.stats = s;
                self.load_error = None;
            }
            Err(e) => {
                self.load_error = Some(e.to_string());
            }
        }
        self.loaded = true;
    }

    /// Filtered entry list based on current filter + search
    pub fn filtered_entries(&self) -> Vec<&ServiceEntry> {
        self.entries
            .iter()
            .filter(|e| match self.filter_kind {
                FilterKind::All => true,
                FilterKind::Active => e.status.is_active(),
                FilterKind::Systemd => e.kind == EntryKind::Systemd && e.status.is_active(),
                FilterKind::Containers => {
                    matches!(e.kind, EntryKind::Docker | EntryKind::Podman)
                }
                FilterKind::Failed => e.status == RunState::Failed,
            })
            .filter(|e| {
                if self.search_text.is_empty() {
                    return true;
                }
                let needle = self.search_text.to_lowercase();
                e.display_name.to_lowercase().contains(&needle)
                    || e.description.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Currently selected entry (if any)
    pub fn selected_entry(&self) -> Option<&ServiceEntry> {
        let filtered = self.filtered_entries();
        filtered.get(self.overview_selected).copied()
    }

    /// Load logs for the selected entry
    fn load_logs(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            match services::get_logs(&entry, 200) {
                Ok(lines) => {
                    self.logs = lines;
                    self.logs_scroll = if self.logs.len() > 10 {
                        self.logs.len().saturating_sub(10)
                    } else {
                        0
                    };
                }
                Err(e) => {
                    self.logs = vec![format!("Error: {}", e)];
                    self.logs_scroll = 0;
                }
            }
        } else {
            self.logs.clear();
            self.logs_scroll = 0;
        }
    }

    fn show_flash(&mut self, msg: &str, is_error: bool) {
        self.flash_message = Some(FlashMessage::new(msg.to_string(), is_error));
    }

    fn clamp_selection(&mut self) {
        let count = self.filtered_entries().len();
        if count == 0 {
            self.overview_selected = 0;
        } else if self.overview_selected >= count {
            self.overview_selected = count - 1;
        }
    }

    // ═══════════════════════════════════════
    //  KEY HANDLING
    // ═══════════════════════════════════════

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Clear expired flash
        if let Some(msg) = &self.flash_message {
            if msg.is_expired(3) {
                self.flash_message = None;
            }
        }

        // Handle popup first
        if let SvcPopupState::ConfirmAction {
            ref entry_name,
            ref entry_display,
            entry_kind,
            action,
        } = self.popup.clone()
        {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.popup = SvcPopupState::None;
                    // Build a temporary entry to pass to execute_action
                    let tmp = ServiceEntry {
                        kind: entry_kind,
                        name: entry_name.clone(),
                        display_name: entry_display.clone(),
                        status: RunState::Unknown,
                        enabled: EnableState::Unknown,
                        description: String::new(),
                        pid: None,
                        memory: None,
                        uptime: None,
                        ports: Vec::new(),
                    };
                    match services::execute_action(&tmp, action) {
                        Ok(msg) => {
                            self.show_flash(&msg, false);
                            self.refresh();
                        }
                        Err(e) => {
                            self.show_flash(&e.to_string(), true);
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.popup = SvcPopupState::None;
                }
                _ => {}
            }
            return Ok(());
        }

        // Sub-tab switching with [ / ]
        match key.code {
            KeyCode::Char('[') => {
                self.active_sub_tab = self.active_sub_tab.prev();
                if self.active_sub_tab == SvcSubTab::Logs {
                    self.load_logs();
                }
                return Ok(());
            }
            KeyCode::Char(']') => {
                self.active_sub_tab = self.active_sub_tab.next();
                if self.active_sub_tab == SvcSubTab::Logs {
                    self.load_logs();
                }
                return Ok(());
            }
            _ => {}
        }

        match self.active_sub_tab {
            SvcSubTab::Overview => self.handle_overview_key(key),
            SvcSubTab::Ports => self.handle_ports_key(key),
            SvcSubTab::Manage => self.handle_manage_key(key),
            SvcSubTab::Logs => self.handle_logs_key(key),
        }
    }

    fn handle_overview_key(&mut self, key: KeyEvent) -> Result<()> {
        // Search input mode
        if self.search_active {
            match key.code {
                KeyCode::Esc => {
                    self.search_active = false;
                    if self.search_text.is_empty() {
                        // no-op
                    }
                }
                KeyCode::Enter => {
                    self.search_active = false;
                }
                KeyCode::Backspace => {
                    self.search_text.pop();
                    self.overview_selected = 0;
                }
                KeyCode::Char(c) => {
                    self.search_text.push(c);
                    self.overview_selected = 0;
                }
                _ => {}
            }
            return Ok(());
        }

        let count = self.filtered_entries().len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if count > 0 && self.overview_selected < count - 1 {
                    self.overview_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.overview_selected = self.overview_selected.saturating_sub(1);
            }
            KeyCode::Char('/') => {
                self.search_active = true;
            }
            KeyCode::Char('f') => {
                self.filter_kind = self.filter_kind.next();
                self.overview_selected = 0;
            }
            KeyCode::Char('r') => {
                self.refresh();
                self.clamp_selection();
                let s = crate::i18n::get_strings(self.lang);
                self.show_flash(s.svc_refreshed, false);
            }
            KeyCode::Enter => {
                // Jump to Logs for selected
                self.active_sub_tab = SvcSubTab::Logs;
                self.load_logs();
            }
            KeyCode::Char('m') => {
                // Jump to Manage for selected
                self.active_sub_tab = SvcSubTab::Manage;
                self.manage_action_idx = 0;
            }
            KeyCode::Char('g') => {
                self.overview_selected = 0;
            }
            KeyCode::Char('G') => {
                if count > 0 {
                    self.overview_selected = count - 1;
                }
            }
            KeyCode::Esc => {
                // Clear search
                if !self.search_text.is_empty() {
                    self.search_text.clear();
                    self.overview_selected = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_ports_key(&mut self, key: KeyEvent) -> Result<()> {
        let count = self.ports.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if count > 0 && self.ports_selected < count - 1 {
                    self.ports_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ports_selected = self.ports_selected.saturating_sub(1);
            }
            KeyCode::Char('r') => {
                self.refresh();
                let s = crate::i18n::get_strings(self.lang);
                self.show_flash(s.svc_refreshed, false);
            }
            KeyCode::Char('g') => {
                self.ports_selected = 0;
            }
            KeyCode::Char('G') => {
                if count > 0 {
                    self.ports_selected = count - 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_manage_key(&mut self, key: KeyEvent) -> Result<()> {
        let entry = self.selected_entry().cloned();
        let actions = self.available_actions();
        let count = actions.len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if count > 0 && self.manage_action_idx < count - 1 {
                    self.manage_action_idx += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.manage_action_idx = self.manage_action_idx.saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(entry) = entry {
                    if let Some(&action) = actions.get(self.manage_action_idx) {
                        self.popup = SvcPopupState::ConfirmAction {
                            entry_name: entry.name.clone(),
                            entry_display: entry.display_name.clone(),
                            entry_kind: entry.kind,
                            action,
                        };
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_logs_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.logs_scroll = self.logs_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.logs_scroll = self.logs_scroll.saturating_sub(1);
            }
            KeyCode::Char('r') => {
                self.load_logs();
                let s = crate::i18n::get_strings(self.lang);
                self.show_flash(s.svc_logs_refreshed, false);
            }
            KeyCode::Char('g') => {
                self.logs_scroll = 0;
            }
            KeyCode::Char('G') => {
                if self.logs.len() > 5 {
                    self.logs_scroll = self.logs.len().saturating_sub(5);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Get valid actions for the currently selected entry
    fn available_actions(&self) -> Vec<ServiceAction> {
        let entry = self.selected_entry();
        let kind = entry.map(|e| e.kind).unwrap_or(EntryKind::Systemd);
        vec![
            ServiceAction::Start,
            ServiceAction::Stop,
            ServiceAction::Restart,
            ServiceAction::Enable,
            ServiceAction::Disable,
        ]
        .into_iter()
        .filter(|a| a.valid_for(kind))
        .collect()
    }
}

// ═══════════════════════════════════════════════════════════════
//  RENDERING
// ═══════════════════════════════════════════════════════════════

pub fn render(
    frame: &mut Frame,
    state: &mut ServicesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    // Kick off background loading on first render (non-blocking)
    let s = crate::i18n::get_strings(lang);
    state.start_loading();

    // Full background
    frame.render_widget(Block::default().style(theme.block_style()), area);

    // If still loading, show loading screen
    if state.loading && !state.loaded {
        let loading_text = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(
                format!("⏳  {} ...", s.svc_loading_title),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::styled(s.svc_scanning_desc, theme.text_dim()),
            Line::styled(s.svc_scanning_hint, theme.text_dim()),
        ];
        let loading = Paragraph::new(loading_text)
            .alignment(Alignment::Center)
            .style(theme.block_style());
        frame.render_widget(loading, area);
        return;
    }

    // Layout: sub-tab bar + content
    let chunks = Layout::vertical([
        Constraint::Length(2), // Sub-tab bar
        Constraint::Min(5),    // Content
    ])
    .split(area);

    render_sub_tab_bar(frame, state, theme, lang, chunks[0]);

    // Check for load error (graceful degradation)
    if let Some(ref err) = state.load_error {
        render_load_error(frame, err, theme, lang, chunks[1]);
    } else {
        match state.active_sub_tab {
            SvcSubTab::Overview => render_overview(frame, state, theme, lang, chunks[1]),
            SvcSubTab::Ports => render_ports(frame, state, theme, lang, chunks[1]),
            SvcSubTab::Manage => render_manage(frame, state, theme, lang, chunks[1]),
            SvcSubTab::Logs => render_logs(frame, state, theme, lang, chunks[1]),
        }
    }

    // Popup overlay
    render_popups(frame, state, theme, lang, area);

    // Flash message
    if let Some(msg) = &state.flash_message {
        widgets::render_flash_message(frame, &msg.text, msg.is_error, theme, area);
    }
}

fn render_sub_tab_bar(
    frame: &mut Frame,
    state: &ServicesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    frame.render_widget(Block::default().style(theme.block_style()), area);

    let tab_titles: Vec<Line> = SvcSubTab::all()
        .iter()
        .map(|tab| {
            let style = if state.active_sub_tab == *tab {
                theme.tab_active()
            } else {
                theme.tab_inactive()
            };
            Line::styled(format!(" {} ", tab.label(lang)), style)
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(state.active_sub_tab.index())
        .divider(" │ ")
        .style(theme.text());

    let tabs_area = widgets::render_sub_tab_nav(frame, theme, area);
    frame.render_widget(tabs, tabs_area);
}

fn render_load_error(frame: &mut Frame, err: &str, theme: &Theme, lang: Language, area: Rect) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_services))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::raw(""),
        Line::styled(format!("⚠ {}", s.svc_load_error), theme.warning()),
        Line::raw(""),
        Line::styled(err, theme.error()),
        Line::raw(""),
        Line::styled(s.svc_load_error_hint, theme.text_dim()),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.text())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        inner,
    );
}

// ── Overview (Dashboard) ──

fn render_overview(
    frame: &mut Frame,
    state: &ServicesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let st = &state.stats;

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.svc_overview))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Layout: dashboard stats → filter bar → list
    let layout = Layout::vertical([
        Constraint::Length(3), // Stats dashboard
        Constraint::Length(1), // Filter + search
        Constraint::Min(3),    // Entry list
    ])
    .split(inner);

    // ── Dashboard stats ──
    let mut stat_spans: Vec<Span> = vec![
        Span::styled("  ⚙ ", theme.text_dim()),
        Span::styled(
            format!("{}", st.services_running),
            Style::default().fg(theme.success),
        ),
        Span::styled(format!(" {} ", s.svc_running), theme.text_dim()),
    ];

    if st.services_failed > 0 {
        stat_spans.push(Span::styled(
            format!("  {} {} ", st.services_failed, s.svc_failed),
            Style::default().fg(theme.error),
        ));
    }

    stat_spans.push(Span::styled(
        format!("  {} {}", st.services_total, s.svc_total),
        theme.text_dim(),
    ));

    let stats_line1 = Line::from(stat_spans);

    // Container stats (only if Docker/Podman detected)
    let stats_line2 = if st.containers_total > 0 {
        let mut cspans: Vec<Span> = Vec::new();

        if st.has_docker {
            cspans.push(Span::styled("  🐳 ", theme.text_dim()));
        }
        if st.has_podman {
            cspans.push(Span::styled("  ⬡ ", theme.text_dim()));
        }

        cspans.push(Span::styled(
            format!("{}", st.containers_running),
            Style::default().fg(theme.success),
        ));
        cspans.push(Span::styled(
            format!(" {} ", s.svc_running),
            theme.text_dim(),
        ));

        if st.containers_stopped > 0 {
            cspans.push(Span::styled(
                format!("  {} stopped", st.containers_stopped),
                theme.text_dim(),
            ));
        }

        cspans.push(Span::styled(
            format!("  {} {}", st.containers_total, s.svc_total_containers),
            theme.text_dim(),
        ));

        Line::from(cspans)
    } else {
        Line::raw("")
    };

    // Port stats
    let stats_line3 = Line::from(vec![
        Span::styled("  🔌 ", theme.text_dim()),
        Span::styled(
            format!("{}", st.ports_open),
            Style::default().fg(theme.accent),
        ),
        Span::styled(format!(" {}", s.svc_ports_open), theme.text_dim()),
    ]);

    let stats_widget = Paragraph::new(vec![stats_line1, stats_line2, stats_line3]);
    frame.render_widget(stats_widget, layout[0]);

    // ── Filter + search bar ──
    let filtered = state.filtered_entries();
    let filter_label = state.filter_kind.label(lang);

    let filter_line = if state.search_active {
        Line::from(vec![
            Span::styled(format!("  [f] {} ", filter_label), theme.text_dim()),
            Span::styled("│ ", theme.text_dim()),
            Span::styled(
                format!("/{}█", state.search_text),
                Style::default().fg(theme.accent),
            ),
        ])
    } else {
        let mut spans = vec![Span::styled(
            format!("  [f] {} ", filter_label),
            Style::default().fg(theme.accent),
        )];
        if !state.search_text.is_empty() {
            spans.push(Span::styled("│ ", theme.text_dim()));
            spans.push(Span::styled(
                format!("/{} ", state.search_text),
                theme.text_dim(),
            ));
        }
        spans.push(Span::styled(
            format!("│ {} {}", filtered.len(), s.svc_shown),
            theme.text_dim(),
        ));
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(filter_line), layout[1]);

    // ── Entry list ──
    let list_area = layout[2];
    let visible_height = list_area.height as usize;

    if filtered.is_empty() {
        let msg = Paragraph::new(Line::styled(
            format!("  {}", s.svc_no_entries),
            theme.text_dim(),
        ));
        frame.render_widget(msg, list_area);
        return;
    }

    // Scroll to keep selection visible
    let scroll = if state.overview_selected >= visible_height {
        state.overview_selected - visible_height + 1
    } else {
        0
    };

    let name_width = (list_area.width as usize / 3).clamp(15, 35);

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, entry)| {
            let is_sel = i == state.overview_selected;

            let status_style = match entry.status {
                RunState::Running => Style::default().fg(theme.success),
                RunState::Failed => Style::default().fg(theme.error),
                RunState::Restarting => Style::default().fg(theme.warning),
                _ => theme.text_dim(),
            };

            let line_style = if is_sel {
                theme.selected()
            } else {
                theme.text()
            };

            let kind_icon = entry.kind.icon();
            let padded_name = format!("{:<width$}", entry.display_name, width = name_width);

            // Show ports inline if any
            let port_str = if entry.ports.is_empty() {
                String::new()
            } else {
                let port_list: Vec<String> = entry.ports.iter().map(|p| p.to_string()).collect();
                format!(" :{}", port_list.join(","))
            };

            let enabled_str = match entry.enabled {
                EnableState::Enabled => " ✓",
                EnableState::Disabled => " ✗",
                EnableState::NotApplicable => "",
                _ => "",
            };

            // Truncate description to fit
            let desc_width =
                list_area.width as usize - name_width - 12 - port_str.len() - enabled_str.len();
            let desc = truncate(&entry.description, desc_width);

            ListItem::new(Line::from(vec![
                Span::styled(
                    if is_sel { " ▸" } else { "  " },
                    Style::default().fg(theme.accent),
                ),
                Span::styled(format!("{} ", entry.status.symbol()), status_style),
                Span::styled(format!("{} ", kind_icon), theme.text_dim()),
                Span::styled(padded_name, line_style),
                Span::styled(enabled_str, theme.text_dim()),
                Span::styled(port_str, Style::default().fg(theme.accent)),
                Span::styled(format!("  {}", desc), theme.text_dim()),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), list_area);
}

// ── Ports ──

fn render_ports(
    frame: &mut Frame,
    state: &ServicesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.svc_ports))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.ports.is_empty() {
        let msg = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(s.svc_no_ports, theme.text_dim()),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
        return;
    }

    // Header + list
    let layout = Layout::vertical([
        Constraint::Length(2), // Header row
        Constraint::Min(3),    // Port list
    ])
    .split(inner);

    let header = Line::from(vec![Span::styled(
        format!(
            "  {:<7} {:<7} {:<20} {:<24} {}",
            s.svc_col_proto, s.svc_col_port, s.svc_col_address, s.svc_col_owner, s.svc_col_process,
        ),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )]);
    let sep = Line::styled(
        format!("  {}", "─".repeat(inner.width.saturating_sub(4) as usize)),
        theme.text_dim(),
    );
    frame.render_widget(Paragraph::new(vec![header, sep]), layout[0]);

    // Port list
    let visible = layout[1].height as usize;
    let scroll = if state.ports_selected >= visible {
        state.ports_selected - visible + 1
    } else {
        0
    };

    let items: Vec<ListItem> = state
        .ports
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, port)| {
            let is_sel = i == state.ports_selected;
            let style = if is_sel {
                theme.selected()
            } else {
                theme.text()
            };

            let proto_style = if port.protocol == "tcp" {
                Style::default().fg(theme.success)
            } else {
                Style::default().fg(theme.warning)
            };

            let owner_display = if port.owner.is_empty() {
                "-".to_string()
            } else {
                let icon = port.owner_kind.icon();
                format!("{} {}", icon, port.owner)
            };

            let pid_str = port.pid.map(|p| p.to_string()).unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(
                    if is_sel { " ▸" } else { "  " },
                    Style::default().fg(theme.accent),
                ),
                Span::styled(format!("{:<7}", port.protocol), proto_style),
                Span::styled(
                    format!("{:<7}", port.port),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<20}", port.address), style),
                Span::styled(format!("{:<24}", truncate(&owner_display, 23)), style),
                Span::styled(format!("{:<12}", port.process_name), theme.text_dim()),
                Span::styled(pid_str, theme.text_dim()),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), layout[1]);
}

// ── Manage ──

fn render_manage(
    frame: &mut Frame,
    state: &ServicesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let entry = state.selected_entry();

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.svc_manage))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::vertical([
        Constraint::Length(5), // Entry details
        Constraint::Length(1), // Separator
        Constraint::Min(5),    // Actions
    ])
    .split(inner);

    // ── Entry detail ──
    if let Some(entry) = entry {
        let status_style = match entry.status {
            RunState::Running => Style::default().fg(theme.success),
            RunState::Failed => Style::default().fg(theme.error),
            _ => theme.text_dim(),
        };

        let port_str = if entry.ports.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = entry.ports.iter().map(|p| p.to_string()).collect();
            format!("  Ports: {}", list.join(", "))
        };

        let mem_str = entry
            .memory
            .as_deref()
            .map(|m| format!("  Mem: {}", m))
            .unwrap_or_default();

        let detail = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!("  {} ", entry.kind.icon()), theme.text_dim()),
                Span::styled(
                    &entry.display_name,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  ({})", entry.kind.label()), theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", s.svc_status_label), theme.text_dim()),
                Span::styled(
                    format!("{} {}", entry.status.symbol(), status_str(&entry.status)),
                    status_style,
                ),
                Span::styled(
                    format!("    {}: {}", s.svc_enabled_label, entry.enabled.as_str()),
                    theme.text_dim(),
                ),
            ]),
            Line::from(vec![Span::styled(
                format!("  {} {}", s.svc_description_label, entry.description),
                theme.text_dim(),
            )]),
            Line::from(vec![Span::styled(
                format!("{}{}", port_str, mem_str),
                theme.text_dim(),
            )]),
        ]);
        frame.render_widget(detail, layout[0]);
    } else {
        let msg = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(format!("  {}", s.svc_select_first), theme.text_dim()),
        ]);
        frame.render_widget(msg, layout[0]);
    }

    // ── Separator ──
    let sep = Paragraph::new(Line::styled(
        format!("  ── {} ──", s.svc_actions),
        theme.text_dim(),
    ));
    frame.render_widget(sep, layout[1]);

    // ── Action list ──
    let actions = state.available_actions();

    let entry_kind = match entry {
        Some(e) => e.kind,
        None => {
            let msg = Paragraph::new(Line::styled(
                format!("  {}", s.svc_select_first),
                theme.text_dim(),
            ));
            frame.render_widget(msg, layout[2]);
            return;
        }
    };
    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let is_sel = i == state.manage_action_idx;
            let style = if is_sel {
                theme.selected()
            } else {
                theme.text()
            };

            let label = action_label(action, lang);
            let icon = action_icon(action);
            let sudo_hint = if action.needs_sudo(entry_kind) {
                " (sudo)"
            } else {
                ""
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    if is_sel { "  ▸ " } else { "    " },
                    Style::default().fg(theme.accent),
                ),
                Span::styled(format!("{} ", icon), Style::default().fg(theme.accent)),
                Span::styled(label, style),
                Span::styled(sudo_hint, theme.text_dim()),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), layout[2]);
}

// ── Logs ──

fn render_logs(
    frame: &mut Frame,
    state: &ServicesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let entry = state.selected_entry();

    let entry_label = entry
        .map(|e| format!("{} {} ", e.kind.icon(), e.display_name))
        .unwrap_or_else(|| s.svc_no_selection.to_string());

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} {} ", s.svc_logs_for, entry_label))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.logs.is_empty() {
        let msg = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(s.svc_no_logs, theme.text_dim()),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
        return;
    }

    let visible = inner.height as usize;
    let max_scroll = state.logs.len().saturating_sub(visible);
    let scroll = state.logs_scroll.min(max_scroll);

    let log_lines: Vec<Line> = state
        .logs
        .iter()
        .skip(scroll)
        .take(visible)
        .map(|line| {
            let style =
                if line.contains("error") || line.contains("ERROR") || line.contains("Failed") {
                    Style::default().fg(theme.error)
                } else if line.contains("warning") || line.contains("WARN") {
                    Style::default().fg(theme.warning)
                } else {
                    theme.text()
                };
            Line::styled(line.as_str(), style)
        })
        .collect();

    frame.render_widget(Paragraph::new(log_lines), inner);
}

// ── Popups ──

fn render_popups(
    frame: &mut Frame,
    state: &ServicesState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    match &state.popup {
        SvcPopupState::None => {}
        SvcPopupState::ConfirmAction {
            entry_display,
            entry_kind,
            action,
            ..
        } => {
            let label = action_label(action, lang);
            let sudo_note = if action.needs_sudo(*entry_kind) {
                format!("\n{}", s.svc_sudo_note)
            } else {
                String::new()
            };

            let content = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled(format!("{} ", entry_kind.icon()), theme.text_dim()),
                    Span::styled(
                        entry_display.as_str(),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::raw(""),
                Line::styled(format!("→ {}", label), theme.text()),
                Line::raw(""),
                Line::styled(s.svc_confirm_action, theme.text()),
                Line::styled(sudo_note, theme.text_dim()),
            ];
            widgets::render_popup(
                frame,
                s.svc_action_title,
                content,
                &[(s.yes, 'y'), (s.no, 'n')],
                theme,
                area,
            );
        }
    }
}

// ═══════════════════════════════════════
//  HELPERS
// ═══════════════════════════════════════

fn truncate(s: &str, max: usize) -> String {
    if max < 2 {
        return String::new();
    }
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn status_str(state: &RunState) -> &'static str {
    match state {
        RunState::Running => "running",
        RunState::Stopped => "stopped",
        RunState::Failed => "failed",
        RunState::Restarting => "restarting",
        RunState::Paused => "paused",
        RunState::Created => "created",
        RunState::Exited => "exited",
        RunState::Unknown => "unknown",
    }
}

fn action_label(action: &ServiceAction, lang: Language) -> &'static str {
    let s = i18n::get_strings(lang);
    match action {
        ServiceAction::Start => s.svc_act_start,
        ServiceAction::Stop => s.svc_act_stop,
        ServiceAction::Restart => s.svc_act_restart,
        ServiceAction::Enable => s.svc_act_enable,
        ServiceAction::Disable => s.svc_act_disable,
    }
}

fn action_icon(action: &ServiceAction) -> &'static str {
    match action {
        ServiceAction::Start => "▶",
        ServiceAction::Stop => "■",
        ServiceAction::Restart => "↻",
        ServiceAction::Enable => "✓",
        ServiceAction::Disable => "✗",
    }
}
