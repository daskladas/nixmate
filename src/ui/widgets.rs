//! Reusable UI widgets for nixmate
//!
//! Common UI components shared across all modules:
//! - Popup dialogs
//! - Status bar
//! - Flash messages
//! - Loading indicators
//! - Layout helpers

use crate::ui::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Render a centered popup dialog
pub fn render_popup(
    frame: &mut Frame,
    title: &str,
    content: Vec<Line>,
    buttons: &[(&str, char)],
    theme: &Theme,
    area: Rect,
) {
    let popup_width = 56.min(area.width.saturating_sub(4));
    let popup_height = (content.len() as u16 + 8).min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", title))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());
    frame.render_widget(block, popup_area);

    let inner = Rect {
        x: popup_area.x + 2,
        y: popup_area.y + 2,
        width: popup_area.width.saturating_sub(4),
        height: popup_area.height.saturating_sub(5),
    };

    let content_widget = Paragraph::new(content)
        .style(theme.text())
        .wrap(Wrap { trim: false });
    frame.render_widget(content_widget, inner);

    if !buttons.is_empty() {
        let button_area = Rect {
            x: popup_area.x + 2,
            y: popup_area.y + popup_area.height - 3,
            width: popup_area.width.saturating_sub(4),
            height: 1,
        };

        let button_spans: Vec<Span> = buttons
            .iter()
            .enumerate()
            .flat_map(|(i, (label, key))| {
                let mut spans = vec![
                    Span::styled("[", theme.text_dim()),
                    Span::styled(
                        key.to_string(),
                        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("] ", theme.text_dim()),
                    Span::styled(*label, theme.text()),
                ];
                if i < buttons.len() - 1 {
                    spans.push(Span::raw("    "));
                }
                spans
            })
            .collect();

        let buttons_widget = Paragraph::new(Line::from(button_spans))
            .alignment(Alignment::Center);
        frame.render_widget(buttons_widget, button_area);
    }
}

/// Render an error popup
pub fn render_error_popup(
    frame: &mut Frame,
    title: &str,
    message: &str,
    theme: &Theme,
    area: Rect,
) {
    let content = vec![
        Line::raw(""),
        Line::styled(message, theme.error()),
        Line::raw(""),
    ];

    render_popup(frame, title, content, &[("OK", 'o')], theme, area);
}

/// Render a loading indicator
pub fn render_loading(
    frame: &mut Frame,
    message: &str,
    theme: &Theme,
    area: Rect,
) {
    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let frame_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 100) as usize
        % spinner_frames.len();

    let content = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(spinner_frames[frame_idx], Style::default().fg(theme.accent)),
            Span::raw(" "),
            Span::styled(message, theme.text()),
        ]),
        Line::raw(""),
    ];

    let popup_width = 40.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_width, 5, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .style(theme.block_style())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());
    frame.render_widget(block, popup_area);

    let inner = Rect {
        x: popup_area.x + 2,
        y: popup_area.y + 1,
        width: popup_area.width.saturating_sub(4),
        height: 3,
    };

    let loading = Paragraph::new(content).alignment(Alignment::Center);
    frame.render_widget(loading, inner);
}

/// Render a flash message at the bottom of the screen
pub fn render_flash_message(
    frame: &mut Frame,
    message: &str,
    is_error: bool,
    theme: &Theme,
    area: Rect,
) {
    let style = if is_error { theme.error() } else { theme.success() };
    let prefix = if is_error { "✗ " } else { "✓ " };

    let flash_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };

    let flash = Paragraph::new(Line::from(vec![
        Span::styled(prefix, style),
        Span::styled(message, style),
    ]));
    frame.render_widget(flash, flash_area);
}

/// Render status bar at bottom
pub fn render_status_bar(
    frame: &mut Frame,
    left_content: &str,
    right_content: &str,
    theme: &Theme,
    area: Rect,
) {
    let status_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };

    frame.render_widget(Clear, status_area);

    let left_widget = Paragraph::new(left_content).style(theme.text_dim());

    let right_len = right_content.len() as u16;
    let right_area = Rect {
        x: status_area.x + status_area.width.saturating_sub(right_len + 1),
        y: status_area.y,
        width: right_len + 1,
        height: 1,
    };
    let right_widget = Paragraph::new(right_content).style(theme.text_dim());

    frame.render_widget(left_widget, status_area);
    frame.render_widget(right_widget, right_area);
}

/// Helper: Create a centered rect of given size
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}


