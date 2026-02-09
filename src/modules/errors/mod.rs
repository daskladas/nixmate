//! Error Translator module (formerly nixplain)
//!
//! Integrated into nixmate as an inline module.
//! Has sub-tabs: Analyze, Submit Pattern.
//! Uses nixmate's global theme, i18n, and config.

pub mod ai;
pub mod matcher;
pub mod patterns;
pub mod patterns_i18n;

use crate::config::Language;
use crate::i18n;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use matcher::MatchResult;
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
pub enum ErrSubTab {
    #[default]
    Analyze,
    Submit,
}

impl ErrSubTab {
    pub fn all() -> &'static [ErrSubTab] {
        &[ErrSubTab::Analyze, ErrSubTab::Submit]
    }

    pub fn index(&self) -> usize {
        match self {
            ErrSubTab::Analyze => 0,
            ErrSubTab::Submit => 1,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            ErrSubTab::Analyze => s.err_analyze,
            ErrSubTab::Submit => s.err_submit,
        }
    }
}

// ── Submit form ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubmitField {
    #[default]
    PatternName,
    ErrorMessage,
    Solution,
    Notes,
}

impl SubmitField {
    pub fn all() -> &'static [SubmitField] {
        &[
            SubmitField::PatternName,
            SubmitField::ErrorMessage,
            SubmitField::Solution,
            SubmitField::Notes,
        ]
    }

    pub fn next(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|f| f == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    pub fn prev(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|f| f == self).unwrap_or(0);
        all[(idx + all.len() - 1) % all.len()]
    }
}

#[derive(Debug, Clone, Default)]
pub struct SubmitForm {
    pub pattern_name: String,
    pub error_message: String,
    pub solution: String,
    pub notes: String,
    pub active_field: SubmitField,
}

impl SubmitForm {
    pub fn get_field_mut(&mut self, field: SubmitField) -> &mut String {
        match field {
            SubmitField::PatternName => &mut self.pattern_name,
            SubmitField::ErrorMessage => &mut self.error_message,
            SubmitField::Solution => &mut self.solution,
            SubmitField::Notes => &mut self.notes,
        }
    }

    pub fn get_field(&self, field: SubmitField) -> &str {
        match field {
            SubmitField::PatternName => &self.pattern_name,
            SubmitField::ErrorMessage => &self.error_message,
            SubmitField::Solution => &self.solution,
            SubmitField::Notes => &self.notes,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.pattern_name.trim().is_empty()
            && !self.error_message.trim().is_empty()
            && !self.solution.trim().is_empty()
    }
}

// ── Module state ──

pub struct ErrorsState {
    // Navigation
    pub active_sub_tab: ErrSubTab,

    // Analyze
    pub input_buffer: String,
    pub input_mode: bool,
    pub result: Option<MatchResult>,
    pub scroll_offset: usize,

    // Pipe mode
    #[allow(dead_code)] // Set during init, reserved for future pipe-specific UI
    pub piped: bool,

    // AI fallback
    pub ai_loading: bool,
    pub ai_result: Option<String>,
    pub ai_provider_name: String,
    pub ai_scroll: usize,
    pub ai_requested: bool,
    ai_rx: Option<mpsc::Receiver<Result<String, String>>>,

    // Submit
    pub submit_form: SubmitForm,

    // Flash
    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

impl ErrorsState {
    /// Initialize the errors module. Always succeeds.
    pub fn new() -> Self {
        Self {
            active_sub_tab: ErrSubTab::Analyze,
            input_buffer: String::new(),
            input_mode: false,
            result: None,
            scroll_offset: 0,
            piped: false,
            ai_loading: false,
            ai_result: None,
            ai_provider_name: String::new(),
            ai_scroll: 0,
            ai_requested: false,
            ai_rx: None,
            submit_form: SubmitForm::default(),
            lang: Language::English,
            flash_message: None,
        }
    }

    /// Initialize with piped input and auto-analyze.
    pub fn new_with_input(input: String, lang: Language) -> Self {
        let s = i18n::get_strings(lang);
        let mut state = Self {
            active_sub_tab: ErrSubTab::Analyze,
            input_buffer: input,
            input_mode: false,
            result: None,
            scroll_offset: 0,
            piped: true,
            ai_loading: false,
            ai_result: None,
            ai_provider_name: String::new(),
            ai_scroll: 0,
            ai_requested: false,
            ai_rx: None,
            submit_form: SubmitForm::default(),
            lang,
            flash_message: Some(FlashMessage::new(s.err_piped_hint.to_string(), false)),
        };
        state.analyze_input(lang);
        state
    }

    /// Perform analysis on the current input buffer
    fn analyze_input(&mut self, lang: Language) {
        if self.input_buffer.trim().is_empty() {
            return;
        }

        let lang_str = match lang {
            Language::English => "en",
            Language::German => "de",
        };

        self.result = matcher::analyze(&self.input_buffer)
            .map(|r| patterns_i18n::translate(&r, lang_str));
        self.input_mode = false;
        self.scroll_offset = 0;
    }

    pub fn show_flash(&mut self, msg: &str, is_error: bool) {
        self.flash_message = Some(FlashMessage::new(msg.to_string(), is_error));
    }

    /// Kick off AI analysis in a background thread (non-blocking).
    pub fn start_ai_analysis(
        &mut self,
        provider: &str,
        api_key: &str,
        ollama_url: &str,
        ollama_model: &str,
        lang: &str,
    ) {
        if self.ai_loading || self.input_buffer.trim().is_empty() {
            return;
        }

        self.ai_loading = true;
        self.ai_result = None;
        self.ai_scroll = 0;
        self.ai_provider_name = ai::provider_display_name(provider).to_string();

        let (tx, rx) = mpsc::channel();
        self.ai_rx = Some(rx);

        let provider = provider.to_string();
        let api_key = api_key.to_string();
        let ollama_url = ollama_url.to_string();
        let ollama_model = ollama_model.to_string();
        let error_text = self.input_buffer.clone();
        let lang = lang.to_string();

        std::thread::spawn(move || {
            let result = ai::analyze_with_ai(
                &provider, &api_key, &ollama_url, &ollama_model, &error_text, &lang,
            );
            let msg = match result {
                Ok(text) => Ok(text),
                Err(e) => Err(format!("{:#}", e)),
            };
            let _ = tx.send(msg);
        });
    }

    /// Poll for AI analysis results. Called from update_timers (non-blocking).
    pub fn poll_ai(&mut self) {
        if let Some(ref rx) = self.ai_rx {
            match rx.try_recv() {
                Ok(Ok(text)) => {
                    self.ai_result = Some(text);
                    self.ai_loading = false;
                    self.ai_scroll = 0;
                    self.ai_rx = None;
                }
                Ok(Err(err)) => {
                    self.show_flash(&err, true);
                    self.ai_loading = false;
                    self.ai_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still loading — do nothing
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.show_flash("AI analysis thread crashed", true);
                    self.ai_loading = false;
                    self.ai_rx = None;
                }
            }
        }
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: KeyEvent, lang: Language) -> Result<()> {
        // Clear expired flash
        if let Some(msg) = &self.flash_message {
            if msg.is_expired(3) {
                self.flash_message = None;
            }
        }

        // Sub-tab switching with F1-F2
        match key.code {
            KeyCode::F(1) => {
                self.active_sub_tab = ErrSubTab::Analyze;
                return Ok(());
            }
            KeyCode::F(2) => {
                self.active_sub_tab = ErrSubTab::Submit;
                return Ok(());
            }
            _ => {}
        }

        match self.active_sub_tab {
            ErrSubTab::Analyze => self.handle_analyze_key(key, lang),
            ErrSubTab::Submit => self.handle_submit_key(key, lang),
        }
    }

    fn handle_analyze_key(&mut self, key: KeyEvent, lang: Language) -> Result<()> {
        if self.input_mode {
            match key.code {
                KeyCode::Esc => {
                    // Always allow leaving input mode
                    self.input_mode = false;
                }
                KeyCode::Enter => {
                    self.analyze_input(lang);
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                _ => {}
            }
        } else if self.ai_loading {
            // AI is running — only allow Esc to cancel
            match key.code {
                KeyCode::Esc => {
                    self.ai_loading = false;
                    self.ai_rx = None;
                }
                _ => {}
            }
        } else if self.ai_result.is_some() {
            // Viewing AI result
            match key.code {
                KeyCode::Char('i') | KeyCode::Char('n') | KeyCode::Enter => {
                    self.input_mode = true;
                    self.input_buffer.clear();
                    self.result = None;
                    self.ai_result = None;
                    self.ai_scroll = 0;
                    self.scroll_offset = 0;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.ai_scroll = self.ai_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.ai_scroll = self.ai_scroll.saturating_sub(1);
                }
                _ => {}
            }
        } else if self.result.is_some() {
            // Viewing pattern result
            match key.code {
                KeyCode::Char('i') | KeyCode::Char('n') | KeyCode::Enter => {
                    // New analysis
                    self.input_mode = true;
                    self.input_buffer.clear();
                    self.result = None;
                    self.scroll_offset = 0;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
                KeyCode::Char('s') => {
                    // Switch to submit tab
                    self.submit_form = SubmitForm::default();
                    self.submit_form.error_message = self.input_buffer.clone();
                    self.active_sub_tab = ErrSubTab::Submit;
                }
                _ => {}
            }
        } else {
            // Idle or NOT FOUND state
            match key.code {
                KeyCode::Char('i') | KeyCode::Enter => {
                    self.input_mode = true;
                }
                KeyCode::Char('n') => {
                    self.input_mode = true;
                    self.input_buffer.clear();
                    self.result = None;
                    self.ai_result = None;
                }
                KeyCode::Char('a') => {
                    // Request AI analysis (handled by app.rs which has config access)
                    if !self.input_buffer.is_empty() && !self.ai_loading {
                        self.ai_requested = true;
                    }
                }
                KeyCode::Char('s') => {
                    self.submit_form = SubmitForm::default();
                    self.submit_form.error_message = self.input_buffer.clone();
                    self.active_sub_tab = ErrSubTab::Submit;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_submit_key(&mut self, key: KeyEvent, lang: Language) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.active_sub_tab = ErrSubTab::Analyze;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.submit_form.active_field = self.submit_form.active_field.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.submit_form.active_field = self.submit_form.active_field.prev();
            }
            KeyCode::Backspace => {
                let field = self.submit_form.get_field_mut(self.submit_form.active_field);
                field.pop();
            }
            KeyCode::Char(c) => {
                let field = self.submit_form.get_field_mut(self.submit_form.active_field);
                field.push(c);
            }
            KeyCode::Enter => {
                if self.submit_form.active_field == SubmitField::Notes {
                    self.submit_form.notes.push('\n');
                } else if self.submit_form.is_valid() {
                    self.do_submit(lang);
                } else {
                    let s = i18n::get_strings(lang);
                    self.show_flash(s.err_fill_fields, true);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn do_submit(&mut self, lang: Language) {
        // Save pattern template locally
        if let Some(path) = dirs::data_dir() {
            let pattern_dir = path.join("nixmate").join("submitted-patterns");
            let _ = std::fs::create_dir_all(&pattern_dir);
            let filename = format!(
                "{}.md",
                self.submit_form
                    .pattern_name
                    .to_lowercase()
                    .replace(' ', "-")
            );

            let template = format!(
                "## New Pattern: {}\n\n### Error Message\n```\n{}\n```\n\n### Solution\n```nix\n{}\n```\n\n### Notes\n{}\n",
                self.submit_form.pattern_name,
                self.submit_form.error_message,
                self.submit_form.solution,
                self.submit_form.notes,
            );

            let _ = std::fs::write(pattern_dir.join(&filename), &template);
        }

        let s = i18n::get_strings(lang);
        self.show_flash(s.err_submit_saved, false);
        self.active_sub_tab = ErrSubTab::Analyze;
    }
}

// ════════════════════════════════════════════════════════════════════
// RENDERING
// ════════════════════════════════════════════════════════════════════

/// Main render function for the errors module
pub fn render(
    frame: &mut Frame,
    state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
    ai_available: bool,
) {
    let _s = i18n::get_strings(lang);

    // Layout: sub-tab bar + content
    let layout = Layout::vertical([
        Constraint::Length(2), // Sub-tab bar
        Constraint::Min(8),   // Content
    ])
    .split(area);

    // Sub-tab bar
    render_sub_tabs(frame, state, theme, lang, layout[0]);

    // Content based on active sub-tab
    match state.active_sub_tab {
        ErrSubTab::Analyze => render_analyze(frame, state, theme, lang, layout[1], ai_available),
        ErrSubTab::Submit => render_submit(frame, state, theme, lang, layout[1]),
    }

    // Flash message
    if let Some(msg) = &state.flash_message {
        widgets::render_flash_message(frame, &msg.text, msg.is_error, theme, area);
    }
}

fn render_sub_tabs(
    frame: &mut Frame,
    state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let tab_titles: Vec<Line> = ErrSubTab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let is_active = state.active_sub_tab == *tab;
            let style = if is_active {
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

// ── Analyze ──

fn render_analyze(
    frame: &mut Frame,
    state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
    ai_available: bool,
) {
    if state.input_mode {
        render_input(frame, state, theme, lang, area);
    } else if state.ai_loading {
        render_ai_loading(frame, state, theme, lang, area);
    } else if state.ai_result.is_some() {
        render_ai_result(frame, state, theme, lang, area);
    } else if let Some(result) = &state.result {
        render_result_found(frame, state, result, theme, lang, area);
    } else if !state.input_buffer.is_empty() {
        render_result_not_found(frame, state, theme, lang, area, ai_available);
    } else {
        render_idle(frame, theme, lang, area);
    }
}

fn render_idle(
    frame: &mut Frame,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.err_analyze))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = vec![
        Line::raw(""),
        Line::raw(""),
        Line::styled(
            "🔍",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(s.err_idle_title, theme.text()),
        Line::raw(""),
        Line::styled(s.err_idle_hint, theme.text_dim()),
        Line::raw(""),
        Line::styled(
            format!("[i] / [Enter] → {}", s.err_start_input),
            Style::default().fg(theme.accent),
        ),
    ];

    frame.render_widget(
        Paragraph::new(content)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_input(
    frame: &mut Frame,
    state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.err_analyze))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    // Hint line
    let hint_area = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(s.err_paste_hint).style(theme.text_dim()),
        hint_area,
    );

    // Input area
    let input_area = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(3),
    };

    let input_block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.err_input))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let input_inner = input_block.inner(input_area);
    frame.render_widget(input_block, input_area);

    let input_text = Paragraph::new(state.input_buffer.as_str())
        .style(theme.text())
        .wrap(Wrap { trim: false });
    frame.render_widget(input_text, input_inner);

    // Cursor
    let inner_width = input_inner.width as usize;
    if inner_width > 0 {
        let text_len = state.input_buffer.chars().count();
        let cursor_x = input_inner.x + (text_len % inner_width) as u16;
        let cursor_y = input_inner.y + (text_len / inner_width) as u16;
        let max_y = input_inner.y + input_inner.height.saturating_sub(1);
        frame.set_cursor_position(ratatui::layout::Position::new(
            cursor_x.min(input_inner.x + input_inner.width.saturating_sub(1)),
            cursor_y.min(max_y),
        ));
    }
}

fn render_result_found(
    frame: &mut Frame,
    state: &ErrorsState,
    result: &MatchResult,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let chunks = Layout::vertical([
        Constraint::Length(3), // Status header
        Constraint::Length(4), // Problem
        Constraint::Length(6), // Solution
        Constraint::Min(6),   // Deep dive (scrollable)
    ])
    .split(area);

    // 1. Status header
    let status_title = format!(
        " ✅ {} · {} {}: {}",
        s.err_found,
        result.category.emoji(),
        result.category.name(),
        result.title
    );
    let title = Paragraph::new(status_title)
        .style(theme.success())
        .block(
            Block::default()
                .style(theme.block_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.success)),
        );
    frame.render_widget(title, chunks[0]);

    // 2. Problem
    let explanation = Paragraph::new(result.explanation.as_str())
        .block(
            Block::default()
                .style(theme.block_style())
                .borders(Borders::ALL)
                .border_style(theme.border())
                .title(format!(" 📋 {} ", s.err_problem))
                .title_style(theme.text_dim()),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(explanation, chunks[1]);

    // 3. Solution + tip
    let mut solution_text = result.solution.clone();
    if let Some(tip) = &result.tip {
        solution_text.push_str(&format!("\n💡 {}", tip));
    }
    let solution = Paragraph::new(solution_text)
        .block(
            Block::default()
                .style(theme.block_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.success))
                .title(format!(" ✅ {} ", s.err_solution))
                .title_style(theme.success()),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(solution, chunks[2]);

    // 4. Deep dive (scrollable)
    let deep_lines: Vec<&str> = result.deep_dive.lines().collect();
    let visible_height = chunks[3].height.saturating_sub(2) as usize;
    let max_scroll = deep_lines.len().saturating_sub(visible_height);
    let scroll = state.scroll_offset.min(max_scroll);

    let visible_text: String = deep_lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    let scroll_indicator = if deep_lines.len() > visible_height {
        format!(" [{}/{}]", scroll + 1, max_scroll + 1)
    } else {
        String::new()
    };

    let deep_title = format!(" 📚 {} (j/k){} ", s.err_understanding, scroll_indicator);

    let deep_dive = Paragraph::new(visible_text)
        .block(
            Block::default()
                .style(theme.block_style())
                .borders(Borders::ALL)
                .border_style(theme.border_focused())
                .title(deep_title)
                .title_style(Style::default().fg(theme.accent)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(deep_dive, chunks[3]);
}

fn render_result_not_found(
    frame: &mut Frame,
    _state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
    ai_available: bool,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.err_analyze))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut content = vec![
        Line::raw(""),
        Line::raw(""),
        Line::styled(
            format!(" ❌ {} ", s.err_not_found),
            theme.error(),
        ),
        Line::raw(""),
        Line::styled(s.err_no_match_msg, theme.text()),
        Line::raw(""),
        Line::raw(""),
    ];

    // AI option (only if configured)
    if ai_available {
        content.push(Line::from(vec![
            Span::styled("  [a] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(format!("🤖 {}", s.err_ai_ask), theme.text()),
        ]));
    }

    content.push(Line::from(vec![
        Span::styled("  [n] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(s.err_new_analysis, theme.text()),
    ]));
    content.push(Line::from(vec![
        Span::styled("  [s] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(s.err_submit_pattern, theme.text()),
    ]));

    let paragraph = Paragraph::new(content)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

// ── AI ──

fn render_ai_loading(
    frame: &mut Frame,
    state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" 🤖 {} ", s.err_ai_result))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = vec![
        Line::raw(""),
        Line::raw(""),
        Line::raw(""),
        Line::styled(
            format!("🔄 {}", s.err_ai_analyzing),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            format!("{} {}", s.err_ai_via, state.ai_provider_name),
            theme.text_dim(),
        ),
        Line::raw(""),
        Line::styled(
            format!("[Esc] {}", s.cancel),
            theme.text_dim(),
        ),
    ];

    frame.render_widget(
        Paragraph::new(content)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_ai_result(
    frame: &mut Frame,
    state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);
    let ai_text = state.ai_result.as_deref().unwrap_or("");

    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(6),   // AI response (scrollable)
    ])
    .split(area);

    // 1. Header
    let header_text = format!(
        " 🤖 {} ({} {}) ",
        s.err_ai_result, s.err_ai_via, state.ai_provider_name
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .style(theme.block_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent)),
        );
    frame.render_widget(header, chunks[0]);

    // 2. AI response (scrollable)
    let lines: Vec<&str> = ai_text.lines().collect();
    let visible_height = chunks[1].height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(visible_height);
    let scroll = state.ai_scroll.min(max_scroll);

    let visible_text: String = lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    let scroll_indicator = if lines.len() > visible_height {
        format!(" [{}/{}]", scroll + 1, max_scroll + 1)
    } else {
        String::new()
    };

    let response = Paragraph::new(visible_text)
        .block(
            Block::default()
                .style(theme.block_style())
                .borders(Borders::ALL)
                .border_style(theme.border_focused())
                .title(format!(" (j/k){} ", scroll_indicator))
                .title_style(theme.text_dim()),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(response, chunks[1]);
}

// ── Submit ──

fn render_submit(
    frame: &mut Frame,
    state: &ErrorsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.err_submit_title))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 12 || inner.width < 20 {
        return;
    }

    let field_labels: &[(SubmitField, &str)] = &[
        (SubmitField::PatternName, s.err_field_name),
        (SubmitField::ErrorMessage, s.err_field_error),
        (SubmitField::Solution, s.err_field_solution),
        (SubmitField::Notes, s.err_field_notes),
    ];

    // Divide inner area into fields
    let chunks = Layout::vertical([
        Constraint::Length(3), // Pattern name
        Constraint::Length(4), // Error message
        Constraint::Length(4), // Solution
        Constraint::Min(3),   // Notes
    ])
    .split(Rect {
        x: inner.x + 1,
        y: inner.y + 1,
        width: inner.width.saturating_sub(2),
        height: inner.height.saturating_sub(2),
    });

    for (i, (field, label)) in field_labels.iter().enumerate() {
        if i >= chunks.len() {
            break;
        }
        let is_active = state.submit_form.active_field == *field;
        let value = state.submit_form.get_field(*field);

        let border_style = if is_active {
            theme.border_focused()
        } else {
            theme.border()
        };
        let title_style = if is_active {
            theme.title()
        } else {
            theme.text_dim()
        };

        let required = if *field != SubmitField::Notes {
            " *"
        } else {
            ""
        };

        let input_block = Block::default()
            .style(theme.block_style())
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" {}{} ", label, required))
            .title_style(title_style);

        let input_inner = input_block.inner(chunks[i]);
        frame.render_widget(input_block, chunks[i]);

        let input = Paragraph::new(value)
            .style(theme.text())
            .wrap(Wrap { trim: false });
        frame.render_widget(input, input_inner);

        // Cursor for active field
        if is_active {
            let iw = input_inner.width as usize;
            if iw > 0 {
                let text_len = value.chars().count();
                let cx = input_inner.x + (text_len % iw) as u16;
                let cy = input_inner.y + (text_len / iw) as u16;
                let max_y = input_inner.y + input_inner.height.saturating_sub(1);
                frame.set_cursor_position(ratatui::layout::Position::new(
                    cx.min(input_inner.x + input_inner.width.saturating_sub(1)),
                    cy.min(max_y),
                ));
            }
        }
    }
}
