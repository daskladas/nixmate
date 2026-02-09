//! Config Showcase module — generate system overview posters and config diagrams.
//!
//! Two modes:
//!   F1 — Overview: beautiful system poster (existing)
//!   F2 — Diagram:  architecture diagram of NixOS config structure
//!
//! Both export as SVG for GitHub READMEs, r/unixporn, or anywhere else.

pub mod diagram;
pub mod poster;

use crate::config::Language;
use crate::i18n;
use crate::nix::sysinfo::{self, PosterInfo};
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
pub enum CfgSubTab {
    #[default]
    Overview,
    Diagram,
}

impl CfgSubTab {
    pub fn all() -> &'static [CfgSubTab] {
        &[CfgSubTab::Overview, CfgSubTab::Diagram]
    }

    pub fn index(&self) -> usize {
        match self {
            CfgSubTab::Overview => 0,
            CfgSubTab::Diagram => 1,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            CfgSubTab::Overview => s.cfg_overview,
            CfgSubTab::Diagram => s.cfg_diagram,
        }
    }
}

// ── State ──

pub struct ConfigShowcaseState {
    pub active_sub_tab: CfgSubTab,

    // Overview state
    pub scanning: bool,
    pub scan_result: Option<PosterInfo>,
    pub export_path: Option<String>,
    pub export_error: Option<String>,
    scan_rx: Option<mpsc::Receiver<PosterInfo>>,

    // Diagram state
    pub diagram_scanning: bool,
    pub diagram_result: Option<diagram::DiagramInfo>,
    pub diagram_export_path: Option<String>,
    pub diagram_export_error: Option<String>,
    diagram_rx: Option<mpsc::Receiver<diagram::DiagramInfo>>,

    // Common
    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

impl ConfigShowcaseState {
    pub fn new() -> Self {
        Self {
            active_sub_tab: CfgSubTab::Overview,
            scanning: false,
            scan_result: None,
            export_path: None,
            export_error: None,
            scan_rx: None,
            diagram_scanning: false,
            diagram_result: None,
            diagram_export_path: None,
            diagram_export_error: None,
            diagram_rx: None,
            lang: Language::English,
            flash_message: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Sub-tab switching
        match key.code {
            KeyCode::F(1) => {
                self.active_sub_tab = CfgSubTab::Overview;
                return Ok(());
            }
            KeyCode::F(2) => {
                self.active_sub_tab = CfgSubTab::Diagram;
                return Ok(());
            }
            _ => {}
        }

        // Tab-specific keys
        match self.active_sub_tab {
            CfgSubTab::Overview => {
                if self.scanning {
                    return Ok(());
                }
                match key.code {
                    KeyCode::Enter | KeyCode::Char('g') => {
                        self.start_overview_scan();
                    }
                    _ => {}
                }
            }
            CfgSubTab::Diagram => {
                if self.diagram_scanning {
                    return Ok(());
                }
                match key.code {
                    KeyCode::Enter | KeyCode::Char('g') => {
                        self.start_diagram_scan();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn start_overview_scan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        self.export_path = None;
        self.export_error = None;

        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);

        std::thread::spawn(move || {
            let info = sysinfo::gather();
            let _ = tx.send(info);
        });
    }

    fn start_diagram_scan(&mut self) {
        if self.diagram_scanning {
            return;
        }
        self.diagram_scanning = true;
        self.diagram_export_path = None;
        self.diagram_export_error = None;

        let (tx, rx) = mpsc::channel();
        self.diagram_rx = Some(rx);

        std::thread::spawn(move || {
            let info = diagram::scan_config();
            let _ = tx.send(info);
        });
    }

    /// Poll for scan completion. Call from update_timers.
    pub fn poll_scan(&mut self) {
        // Poll overview scan
        if let Some(ref rx) = self.scan_rx {
            match rx.try_recv() {
                Ok(info) => {
                    self.scan_result = Some(info);
                    self.scanning = false;
                    self.scan_rx = None;
                    self.do_overview_export();
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scanning = false;
                    self.scan_rx = None;
                    self.flash_message =
                        Some(FlashMessage::new(crate::i18n::get_strings(self.lang).thread_crashed.into(), true));
                }
            }
        }

        // Poll diagram scan
        if let Some(ref rx) = self.diagram_rx {
            match rx.try_recv() {
                Ok(info) => {
                    self.diagram_result = Some(info);
                    self.diagram_scanning = false;
                    self.diagram_rx = None;
                    self.do_diagram_export();
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.diagram_scanning = false;
                    self.diagram_rx = None;
                    self.flash_message =
                        Some(FlashMessage::new(crate::i18n::get_strings(self.lang).thread_crashed.into(), true));
                }
            }
        }
    }

    fn do_overview_export(&mut self) {
        let Some(info) = &self.scan_result else {
            return;
        };
        match poster::save_svg(info) {
            Ok(path) => {
                self.export_path = Some(path.display().to_string());
                self.export_error = None;
            }
            Err(e) => {
                self.export_error = Some(e.to_string());
                self.export_path = None;
            }
        }
    }

    fn do_diagram_export(&mut self) {
        let Some(info) = &self.diagram_result else {
            return;
        };
        match diagram::save_diagram_svg(info) {
            Ok(path) => {
                self.diagram_export_path = Some(path.display().to_string());
                self.diagram_export_error = None;
            }
            Err(e) => {
                self.diagram_export_error = Some(e.to_string());
                self.diagram_export_path = None;
            }
        }
    }
}

// ═══════════════════════════════════════
//  Rendering
// ═══════════════════════════════════════

pub fn render(
    frame: &mut Frame,
    state: &ConfigShowcaseState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_config))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 8 || inner.width < 30 {
        return;
    }

    let layout = Layout::vertical([
        Constraint::Length(3), // Sub-tab bar
        Constraint::Min(4),   // Content
    ])
    .split(inner);

    // ── Sub-tab bar ──
    render_sub_tabs(frame, state, theme, lang, layout[0]);

    // ── Content ──
    match state.active_sub_tab {
        CfgSubTab::Overview => render_overview(frame, state, theme, lang, layout[1]),
        CfgSubTab::Diagram => render_diagram(frame, state, theme, lang, layout[1]),
    }

    // Flash
    if let Some(msg) = &state.flash_message {
        widgets::render_flash_message(frame, &msg.text, msg.is_error, theme, area);
    }
}

fn render_sub_tabs(
    frame: &mut Frame,
    state: &ConfigShowcaseState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let tab_titles: Vec<Line> = CfgSubTab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let style = if state.active_sub_tab == *tab {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg_dim)
            };
            Line::styled(format!(" F{} {} ", i + 1, tab.label(lang)), style)
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(state.active_sub_tab.index())
        .style(theme.text_dim())
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled(" │ ", Style::default().fg(theme.border)));

    frame.render_widget(tabs, area);
}

fn render_overview(
    frame: &mut Frame,
    state: &ConfigShowcaseState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if area.height < 6 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(4), // Description
        Constraint::Length(3), // Generate button
        Constraint::Length(1), // Spacer
        Constraint::Min(4),   // Status / result
    ])
    .split(area);

    // Description
    let desc = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(
            s.cfg_description,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(s.cfg_subtitle, theme.text_dim()),
    ])
    .alignment(Alignment::Center);
    frame.render_widget(desc, chunks[0]);

    // Generate button
    let btn_line = if state.scanning {
        Line::styled(
            format!("  ⏳ {}", s.cfg_scanning),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Line::styled(
            format!("  → [{}]", s.cfg_generate),
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        )
    };
    let btn_p =
        Paragraph::new(vec![Line::raw(""), btn_line]).alignment(Alignment::Center);
    frame.render_widget(btn_p, chunks[1]);

    // Status area
    let mut status_lines: Vec<Line> = Vec::new();

    if let Some(ref path) = state.export_path {
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  ✅ {}", s.cfg_success),
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ));
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  {}", path),
            Style::default().fg(theme.accent),
        ));
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  💡 {}", s.cfg_tip),
            theme.text_dim(),
        ));
    } else if let Some(ref err) = state.export_error {
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  ❌ {}", s.cfg_error),
            theme.error(),
        ));
        for line in err.lines() {
            status_lines.push(Line::styled(format!("  {}", line), theme.text_dim()));
        }
    } else if !state.scanning {
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  ℹ {}", s.cfg_hint),
            theme.text_dim(),
        ));
    }

    let status = Paragraph::new(status_lines).wrap(Wrap { trim: false });
    frame.render_widget(status, chunks[3]);
}

fn render_diagram(
    frame: &mut Frame,
    state: &ConfigShowcaseState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if area.height < 6 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(4), // Description
        Constraint::Length(3), // Generate button
        Constraint::Length(1), // Spacer
        Constraint::Min(4),   // Status / result
    ])
    .split(area);

    // Description
    let desc = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(
            s.cfg_diag_description,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(s.cfg_diag_subtitle, theme.text_dim()),
    ])
    .alignment(Alignment::Center);
    frame.render_widget(desc, chunks[0]);

    // Generate button
    let btn_line = if state.diagram_scanning {
        Line::styled(
            format!("  ⏳ {}", s.cfg_diag_scanning),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Line::styled(
            format!("  → [{}]", s.cfg_diag_generate),
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        )
    };
    let btn_p =
        Paragraph::new(vec![Line::raw(""), btn_line]).alignment(Alignment::Center);
    frame.render_widget(btn_p, chunks[1]);

    // Status area
    let mut status_lines: Vec<Line> = Vec::new();

    if let Some(ref path) = state.diagram_export_path {
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  ✅ {}", s.cfg_diag_success),
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ));
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  {}", path),
            Style::default().fg(theme.accent),
        ));
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  💡 {}", s.cfg_diag_tip),
            theme.text_dim(),
        ));

        // Show quick stats if diagram data available
        if let Some(ref info) = state.diagram_result {
            status_lines.push(Line::raw(""));
            let file_count = info.nodes.iter()
                .filter(|n| n.node_type != diagram::NodeType::FlakeInput)
                .count();
            let input_count = info.flake_inputs.len();
            let edge_count = info.edges.len();
            let mut stats = format!(
                "  📊 {} files · {} connections",
                file_count, edge_count
            );
            if input_count > 0 {
                stats.push_str(&format!(" · {} flake inputs", input_count));
            }
            status_lines.push(Line::styled(stats, theme.text_dim()));
        }
    } else if let Some(ref err) = state.diagram_export_error {
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  ❌ {}", s.cfg_error),
            theme.error(),
        ));
        for line in err.lines() {
            status_lines.push(Line::styled(format!("  {}", line), theme.text_dim()));
        }
    } else if !state.diagram_scanning {
        status_lines.push(Line::raw(""));
        status_lines.push(Line::styled(
            format!("  ℹ {}", s.cfg_diag_hint),
            theme.text_dim(),
        ));
    }

    let status = Paragraph::new(status_lines).wrap(Wrap { trim: false });
    frame.render_widget(status, chunks[3]);
}
