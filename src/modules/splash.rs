//! Welcome Screen & Terminal Image Display for nixmate
//!
//! Shows a one-time welcome screen on first launch with language selection.
//! Displays the mascot via native terminal graphics protocols:
//! - Kitty Graphics Protocol (Kitty, WezTerm, Ghostty)
//! - iTerm2 Inline Images (iTerm2, WezTerm)
//! - Fallback: no image for unsupported terminals

use crate::config::Language;
use crate::i18n;
use crate::ui::Theme;
use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use std::io::Write;
use std::time::Instant;

/// Embedded icon PNG (compiled into the binary — 1024×1024 RGBA)
const ICON_BYTES: &[u8] = include_bytes!("../../assets/icon.png");

// ─── Terminal Image Protocol Detection ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Kitty,
    ITerm2,
    None,
}

impl ImageProtocol {
    pub fn detect() -> Self {
        if std::env::var("KITTY_WINDOW_ID").is_ok() {
            return Self::Kitty;
        }
        if let Ok(term) = std::env::var("TERM") {
            if term.contains("kitty") || term == "xterm-kitty" {
                return Self::Kitty;
            }
        }
        if std::env::var("GHOSTTY_RESOURCES_DIR").is_ok() {
            return Self::Kitty;
        }
        if let Ok(prog) = std::env::var("TERM_PROGRAM") {
            let lower = prog.to_lowercase();
            if lower.contains("wezterm") {
                return Self::Kitty;
            }
            if lower.contains("iterm") {
                return Self::ITerm2;
            }
        }
        Self::None
    }

    pub fn is_supported(&self) -> bool {
        *self != Self::None
    }

}

// ─── Image Cache ────────────────────────────────────────────────────

pub struct ImageCache {
    base64: String,
}

impl ImageCache {
    pub fn new() -> Option<Self> {
        let img = image::load_from_memory(ICON_BYTES).ok()?;
        let resized = img.resize(200, 200, image::imageops::FilterType::Lanczos3);
        let mut cursor = std::io::Cursor::new(Vec::new());
        resized
            .write_to(&mut cursor, image::ImageOutputFormat::Png)
            .ok()?;
        Some(Self {
            base64: base64_encode(&cursor.into_inner()),
        })
    }
}

// ─── Protocol Image Display ─────────────────────────────────────────

pub fn display_image(
    protocol: ImageProtocol,
    cache: &ImageCache,
    col: u16,
    row: u16,
    cols: u16,
    rows: u16,
) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();

    match protocol {
        ImageProtocol::Kitty => {
            // q=2 = quiet mode (NO response from terminal — prevents ghost key events!)
            write!(stdout, "\x1b_Ga=d,d=i,i=1,q=2;\x1b\\")?;
            write!(stdout, "\x1b[{};{}H", row + 1, col + 1)?;

            let b64 = cache.base64.as_bytes();
            let chunk_size = 4096;
            let mut offset = 0;
            let mut first = true;

            while offset < b64.len() {
                let end = (offset + chunk_size).min(b64.len());
                let chunk = std::str::from_utf8(&b64[offset..end]).unwrap_or("");
                let more = if end < b64.len() { 1 } else { 0 };

                if first {
                    write!(
                        stdout,
                        "\x1b_Gf=100,a=T,t=d,i=1,q=2,c={},r={},m={};{}\x1b\\",
                        cols, rows, more, chunk
                    )?;
                    first = false;
                } else {
                    write!(stdout, "\x1b_Gm={},q=2;{}\x1b\\", more, chunk)?;
                }
                offset = end;
            }
        }
        ImageProtocol::ITerm2 => {
            write!(stdout, "\x1b[{};{}H", row + 1, col + 1)?;
            write!(
                stdout,
                "\x1b]1337;File=inline=1;width={};height={};preserveAspectRatio=1:{}\x07",
                cols, rows, cache.base64
            )?;
        }
        ImageProtocol::None => {}
    }

    stdout.flush()
}

pub fn clear_image(protocol: ImageProtocol) -> std::io::Result<()> {
    match protocol {
        ImageProtocol::Kitty => {
            let mut stdout = std::io::stdout();
            // Delete ALL transmitted images (d=A), not just ID 1
            // This prevents GPU texture memory accumulation across sessions
            write!(stdout, "\x1b_Ga=d,d=A,q=2;\x1b\\")?;
            stdout.flush()
        }
        _ => Ok(()),
    }
}

// ─── Base64 ─────────────────────────────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        result.push(if chunk.len() > 1 { CHARS[((triple >> 6) & 0x3F) as usize] as char } else { '=' });
        result.push(if chunk.len() > 2 { CHARS[(triple & 0x3F) as usize] as char } else { '=' });
    }
    result
}

// ─── Welcome Screen State ───────────────────────────────────────────

pub struct WelcomeState {
    pub active: bool,
    pub created_at: Instant,
    /// Language selection (user can toggle on welcome screen)
    pub selected_lang: Language,
}

impl WelcomeState {
    pub fn new(show: bool, initial_lang: Language) -> Self {
        Self {
            active: show,
            created_at: Instant::now(),
            selected_lang: initial_lang,
        }
    }

    pub fn dismiss(&mut self) {
        self.active = false;
    }

    pub fn ready_for_input(&self) -> bool {
        self.created_at.elapsed().as_millis() >= 400
    }

    pub fn toggle_language(&mut self) {
        self.selected_lang = self.selected_lang.next();
    }
}

// ─── Welcome Screen Render ──────────────────────────────────────────

pub fn render_welcome(
    frame: &mut Frame,
    state: &WelcomeState,
    theme: &Theme,
    has_protocol: bool,
) -> Option<(u16, u16, u16, u16)> {
    let area = frame.area();
    // Use the language the user selected on the welcome screen
    let s = i18n::get_strings(state.selected_lang);

    frame.render_widget(Block::default().style(theme.block_style()), area);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Reserve space for mascot image
    let image_info = if has_protocol && area.height >= 24 && area.width >= 30 {
        let img_rows = ((area.height as f32 * 0.35) as u16).clamp(8, 16);
        let img_cols = (img_rows * 2).min(area.width.saturating_sub(4));
        for _ in 0..img_rows {
            lines.push(Line::raw(""));
        }
        lines.push(Line::raw(""));
        Some((img_cols, img_rows))
    } else {
        None
    };

    // Title + version
    lines.push(Line::from(vec![
        Span::styled(
            "nixmate",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.fg_dim),
        ),
    ]));
    lines.push(Line::raw(""));

    // Greeting
    lines.push(Line::styled(
        s.welcome_greeting,
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));

    // Body text
    for line in word_wrap(s.welcome_body, 58) {
        lines.push(Line::styled(line, Style::default().fg(theme.fg)));
    }
    lines.push(Line::raw(""));

    // Language selector
    let lang_en = if state.selected_lang == Language::English { "● English" } else { "○ English" };
    let lang_de = if state.selected_lang == Language::German { "● Deutsch" } else { "○ Deutsch" };
    lines.push(Line::from(vec![
        Span::styled(
            s.welcome_language,
            Style::default().fg(theme.fg_dim),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            lang_en.to_string(),
            if state.selected_lang == Language::English {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg_dim)
            },
        ),
        Span::styled("   ", Style::default()),
        Span::styled(
            lang_de.to_string(),
            if state.selected_lang == Language::German {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg_dim)
            },
        ),
        Span::styled(
            format!("   [{}]", s.welcome_lang_hint),
            Style::default().fg(theme.fg_dim),
        ),
    ]));
    lines.push(Line::raw(""));

    // Promise
    lines.push(Line::styled(
        s.welcome_once,
        Style::default().fg(theme.fg_dim),
    ));
    lines.push(Line::raw(""));

    // Continue
    lines.push(Line::from(vec![
        Span::styled("── ", Style::default().fg(theme.fg_dim)),
        Span::styled(
            s.welcome_continue,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ──", Style::default().fg(theme.fg_dim)),
    ]));

    // Vertical center
    let content_height = lines.len() as u16;
    let top_pad = area.height.saturating_sub(content_height) / 2;

    let mut padded: Vec<Line<'static>> = Vec::with_capacity(top_pad as usize + lines.len());
    for _ in 0..top_pad {
        padded.push(Line::raw(""));
    }
    padded.extend(lines);

    frame.render_widget(
        Paragraph::new(padded)
            .alignment(Alignment::Center)
            .style(theme.block_style()),
        area,
    );

    image_info.map(|(img_cols, img_rows)| {
        let img_row = area.y + top_pad;
        let img_col = area.x + (area.width.saturating_sub(img_cols)) / 2;
        (img_col, img_row, img_cols, img_rows)
    })
}

// ─── Help Tab: Image Area ───────────────────────────────────────────

pub fn help_image_area(inner: ratatui::layout::Rect, has_protocol: bool) -> Option<(u16, u16, u16, u16)> {
    if !has_protocol {
        return None;
    }
    let text_rows = 20;
    let available = inner.height.saturating_sub(text_rows);
    if available < 6 || inner.width < 20 {
        return None;
    }
    let img_rows = available.min(14);
    let img_cols = (img_rows * 2).min(inner.width.saturating_sub(4));
    let img_col = inner.x + (inner.width.saturating_sub(img_cols)) / 2;
    Some((img_col, inner.y, img_cols, img_rows))
}

// ─── Helpers ────────────────────────────────────────────────────────

fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() > max_width {
            lines.push(current);
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
