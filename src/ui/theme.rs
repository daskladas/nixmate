//! Theme definitions for nixmate
//!
//! Provides three built-in themes: Gruvbox, Nord, and Transparent.
//! One theme instance – applied globally to every module.

use crate::config::ThemeName;
use ratatui::style::{Color, Modifier, Style};

/// Complete theme with all required colors
#[derive(Debug, Clone)]
pub struct Theme {
    // Base colors
    pub bg: Color,
    pub fg: Color,
    pub fg_dim: Color,

    // Accent colors
    pub accent: Color,
    pub accent_dim: Color,

    // Status colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,

    // UI element colors
    pub border: Color,
    pub border_focused: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,

    // Diff colors (used by generations module)
    pub diff_added: Color,
    pub diff_removed: Color,
    pub diff_updated: Color,

    // Internal flag for transparent mode
    is_transparent: bool,
}

impl Theme {
    /// Create a theme from a theme name
    pub fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Gruvbox => Self::gruvbox(),
            ThemeName::Nord => Self::nord(),
            ThemeName::Catppuccin => Self::catppuccin(),
            ThemeName::Dracula => Self::dracula(),
            ThemeName::TokyoNight => Self::tokyo_night(),
            ThemeName::RosePine => Self::rose_pine(),
            ThemeName::Everforest => Self::everforest(),
            ThemeName::Kanagawa => Self::kanagawa(),
            ThemeName::SolarizedDark => Self::solarized_dark(),
            ThemeName::OneDark => Self::one_dark(),
            ThemeName::Monokai => Self::monokai(),
            ThemeName::Hacker => Self::hacker(),
            ThemeName::Transparent => Self::transparent(),
        }
    }

    /// Gruvbox dark theme (default)
    pub fn gruvbox() -> Self {
        Self {
            bg: Color::Rgb(40, 40, 40),
            fg: Color::Rgb(235, 219, 178),
            fg_dim: Color::Rgb(146, 131, 116),
            accent: Color::Rgb(254, 128, 25),
            accent_dim: Color::Rgb(214, 93, 14),
            success: Color::Rgb(184, 187, 38),
            warning: Color::Rgb(250, 189, 47),
            error: Color::Rgb(251, 73, 52),
            border: Color::Rgb(80, 73, 69),
            border_focused: Color::Rgb(168, 153, 132),
            selection_bg: Color::Rgb(80, 73, 69),
            selection_fg: Color::Rgb(235, 219, 178),
            diff_added: Color::Rgb(184, 187, 38),
            diff_removed: Color::Rgb(251, 73, 52),
            diff_updated: Color::Rgb(131, 165, 152),
            is_transparent: false,
        }
    }

    /// Nord theme
    pub fn nord() -> Self {
        Self {
            bg: Color::Rgb(46, 52, 64),
            fg: Color::Rgb(236, 239, 244),
            fg_dim: Color::Rgb(76, 86, 106),
            accent: Color::Rgb(136, 192, 208),
            accent_dim: Color::Rgb(94, 129, 172),
            success: Color::Rgb(163, 190, 140),
            warning: Color::Rgb(235, 203, 139),
            error: Color::Rgb(191, 97, 106),
            border: Color::Rgb(59, 66, 82),
            border_focused: Color::Rgb(136, 192, 208),
            selection_bg: Color::Rgb(76, 86, 106),
            selection_fg: Color::Rgb(236, 239, 244),
            diff_added: Color::Rgb(163, 190, 140),
            diff_removed: Color::Rgb(191, 97, 106),
            diff_updated: Color::Rgb(129, 161, 193),
            is_transparent: false,
        }
    }

    /// Catppuccin Mocha theme
    pub fn catppuccin() -> Self {
        Self {
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            fg_dim: Color::Rgb(108, 112, 134),
            accent: Color::Rgb(137, 180, 250),     // blue
            accent_dim: Color::Rgb(116, 199, 236), // sapphire
            success: Color::Rgb(166, 227, 161),    // green
            warning: Color::Rgb(249, 226, 175),    // yellow
            error: Color::Rgb(243, 139, 168),      // red
            border: Color::Rgb(69, 71, 90),        // surface1
            border_focused: Color::Rgb(137, 180, 250),
            selection_bg: Color::Rgb(69, 71, 90),
            selection_fg: Color::Rgb(205, 214, 244),
            diff_added: Color::Rgb(166, 227, 161),
            diff_removed: Color::Rgb(243, 139, 168),
            diff_updated: Color::Rgb(137, 180, 250),
            is_transparent: false,
        }
    }

    /// Dracula theme
    pub fn dracula() -> Self {
        Self {
            bg: Color::Rgb(40, 42, 54),
            fg: Color::Rgb(248, 248, 242),
            fg_dim: Color::Rgb(98, 114, 164),      // comment
            accent: Color::Rgb(189, 147, 249),     // purple
            accent_dim: Color::Rgb(139, 233, 253), // cyan
            success: Color::Rgb(80, 250, 123),     // green
            warning: Color::Rgb(241, 250, 140),    // yellow
            error: Color::Rgb(255, 85, 85),        // red
            border: Color::Rgb(68, 71, 90),        // current line
            border_focused: Color::Rgb(189, 147, 249),
            selection_bg: Color::Rgb(68, 71, 90),
            selection_fg: Color::Rgb(248, 248, 242),
            diff_added: Color::Rgb(80, 250, 123),
            diff_removed: Color::Rgb(255, 85, 85),
            diff_updated: Color::Rgb(139, 233, 253),
            is_transparent: false,
        }
    }

    /// Tokyo Night theme
    pub fn tokyo_night() -> Self {
        Self {
            bg: Color::Rgb(26, 27, 38),
            fg: Color::Rgb(192, 202, 245),
            fg_dim: Color::Rgb(86, 95, 137),
            accent: Color::Rgb(122, 162, 247),     // blue
            accent_dim: Color::Rgb(125, 207, 255), // cyan
            success: Color::Rgb(158, 206, 106),    // green
            warning: Color::Rgb(224, 175, 104),    // yellow
            error: Color::Rgb(247, 118, 142),      // red
            border: Color::Rgb(41, 46, 66),        // bg_highlight
            border_focused: Color::Rgb(122, 162, 247),
            selection_bg: Color::Rgb(41, 46, 66),
            selection_fg: Color::Rgb(192, 202, 245),
            diff_added: Color::Rgb(158, 206, 106),
            diff_removed: Color::Rgb(247, 118, 142),
            diff_updated: Color::Rgb(122, 162, 247),
            is_transparent: false,
        }
    }

    /// Rosé Pine theme
    pub fn rose_pine() -> Self {
        Self {
            bg: Color::Rgb(35, 33, 54),
            fg: Color::Rgb(224, 222, 244),
            fg_dim: Color::Rgb(110, 106, 134),
            accent: Color::Rgb(196, 167, 231),     // iris
            accent_dim: Color::Rgb(156, 207, 216), // foam
            success: Color::Rgb(156, 207, 216),    // foam
            warning: Color::Rgb(246, 193, 119),    // gold
            error: Color::Rgb(235, 111, 146),      // love
            border: Color::Rgb(57, 53, 82),        // highlight med
            border_focused: Color::Rgb(196, 167, 231),
            selection_bg: Color::Rgb(57, 53, 82),
            selection_fg: Color::Rgb(224, 222, 244),
            diff_added: Color::Rgb(156, 207, 216),
            diff_removed: Color::Rgb(235, 111, 146),
            diff_updated: Color::Rgb(196, 167, 231),
            is_transparent: false,
        }
    }

    /// Everforest dark theme
    pub fn everforest() -> Self {
        Self {
            bg: Color::Rgb(39, 46, 43),
            fg: Color::Rgb(211, 198, 170),
            fg_dim: Color::Rgb(135, 139, 124),
            accent: Color::Rgb(167, 192, 128),     // green
            accent_dim: Color::Rgb(131, 192, 159), // aqua
            success: Color::Rgb(167, 192, 128),    // green
            warning: Color::Rgb(219, 188, 127),    // yellow
            error: Color::Rgb(230, 126, 128),      // red
            border: Color::Rgb(58, 67, 62),        // bg3
            border_focused: Color::Rgb(167, 192, 128),
            selection_bg: Color::Rgb(58, 67, 62),
            selection_fg: Color::Rgb(211, 198, 170),
            diff_added: Color::Rgb(167, 192, 128),
            diff_removed: Color::Rgb(230, 126, 128),
            diff_updated: Color::Rgb(131, 192, 159),
            is_transparent: false,
        }
    }

    /// Kanagawa theme (wave)
    pub fn kanagawa() -> Self {
        Self {
            bg: Color::Rgb(31, 31, 40),
            fg: Color::Rgb(220, 215, 186),
            fg_dim: Color::Rgb(114, 113, 105),
            accent: Color::Rgb(126, 156, 216),     // crystal blue
            accent_dim: Color::Rgb(122, 168, 159), // spring green
            success: Color::Rgb(152, 187, 108),    // spring green
            warning: Color::Rgb(226, 194, 114),    // carp yellow
            error: Color::Rgb(195, 64, 67),        // autumn red
            border: Color::Rgb(54, 54, 70),        // sumiInk4
            border_focused: Color::Rgb(126, 156, 216),
            selection_bg: Color::Rgb(54, 54, 70),
            selection_fg: Color::Rgb(220, 215, 186),
            diff_added: Color::Rgb(152, 187, 108),
            diff_removed: Color::Rgb(195, 64, 67),
            diff_updated: Color::Rgb(126, 156, 216),
            is_transparent: false,
        }
    }

    /// Solarized Dark theme
    pub fn solarized_dark() -> Self {
        Self {
            bg: Color::Rgb(0, 43, 54),
            fg: Color::Rgb(131, 148, 150),
            fg_dim: Color::Rgb(88, 110, 117),
            accent: Color::Rgb(38, 139, 210),     // blue
            accent_dim: Color::Rgb(42, 161, 152), // cyan
            success: Color::Rgb(133, 153, 0),     // green
            warning: Color::Rgb(181, 137, 0),     // yellow
            error: Color::Rgb(220, 50, 47),       // red
            border: Color::Rgb(7, 54, 66),        // base02
            border_focused: Color::Rgb(38, 139, 210),
            selection_bg: Color::Rgb(7, 54, 66),
            selection_fg: Color::Rgb(147, 161, 161),
            diff_added: Color::Rgb(133, 153, 0),
            diff_removed: Color::Rgb(220, 50, 47),
            diff_updated: Color::Rgb(38, 139, 210),
            is_transparent: false,
        }
    }

    /// One Dark theme (Atom/VS Code)
    pub fn one_dark() -> Self {
        Self {
            bg: Color::Rgb(40, 44, 52),
            fg: Color::Rgb(171, 178, 191),
            fg_dim: Color::Rgb(92, 99, 112),
            accent: Color::Rgb(97, 175, 239),     // blue
            accent_dim: Color::Rgb(86, 182, 194), // cyan
            success: Color::Rgb(152, 195, 121),   // green
            warning: Color::Rgb(229, 192, 123),   // yellow
            error: Color::Rgb(224, 108, 117),     // red
            border: Color::Rgb(62, 68, 81),
            border_focused: Color::Rgb(97, 175, 239),
            selection_bg: Color::Rgb(62, 68, 81),
            selection_fg: Color::Rgb(171, 178, 191),
            diff_added: Color::Rgb(152, 195, 121),
            diff_removed: Color::Rgb(224, 108, 117),
            diff_updated: Color::Rgb(97, 175, 239),
            is_transparent: false,
        }
    }

    /// Monokai theme
    pub fn monokai() -> Self {
        Self {
            bg: Color::Rgb(39, 40, 34),
            fg: Color::Rgb(248, 248, 242),
            fg_dim: Color::Rgb(117, 113, 94),
            accent: Color::Rgb(102, 217, 239),     // cyan
            accent_dim: Color::Rgb(174, 129, 255), // purple
            success: Color::Rgb(166, 226, 46),     // green
            warning: Color::Rgb(230, 219, 116),    // yellow
            error: Color::Rgb(249, 38, 114),       // pink/red
            border: Color::Rgb(62, 61, 50),
            border_focused: Color::Rgb(102, 217, 239),
            selection_bg: Color::Rgb(62, 61, 50),
            selection_fg: Color::Rgb(248, 248, 242),
            diff_added: Color::Rgb(166, 226, 46),
            diff_removed: Color::Rgb(249, 38, 114),
            diff_updated: Color::Rgb(102, 217, 239),
            is_transparent: false,
        }
    }

    /// Hacker theme (black + green)
    pub fn hacker() -> Self {
        Self {
            bg: Color::Rgb(0, 0, 0),
            fg: Color::Rgb(0, 255, 0),
            fg_dim: Color::Rgb(0, 140, 0),
            accent: Color::Rgb(0, 255, 65),
            accent_dim: Color::Rgb(0, 200, 0),
            success: Color::Rgb(0, 255, 0),
            warning: Color::Rgb(80, 255, 0),
            error: Color::Rgb(255, 0, 0),
            border: Color::Rgb(0, 60, 0),
            border_focused: Color::Rgb(0, 255, 0),
            selection_bg: Color::Rgb(0, 50, 0),
            selection_fg: Color::Rgb(0, 255, 0),
            diff_added: Color::Rgb(0, 255, 0),
            diff_removed: Color::Rgb(255, 0, 0),
            diff_updated: Color::Rgb(0, 200, 255),
            is_transparent: false,
        }
    }

    /// Transparent theme (uses terminal colors)
    pub fn transparent() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::White,
            fg_dim: Color::Gray,
            accent: Color::Cyan,
            accent_dim: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            selection_bg: Color::Reset,
            selection_fg: Color::White,
            diff_added: Color::Green,
            diff_removed: Color::Red,
            diff_updated: Color::Blue,
            is_transparent: true,
        }
    }

    // === STYLE HELPERS ===

    pub fn text(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.fg)
        } else {
            Style::default().fg(self.fg).bg(self.bg)
        }
    }

    pub fn text_dim(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.fg_dim)
        } else {
            Style::default().fg(self.fg_dim).bg(self.bg)
        }
    }

    pub fn title(&self) -> Style {
        if self.is_transparent {
            Style::default()
                .fg(self.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.accent)
                .bg(self.bg)
                .add_modifier(Modifier::BOLD)
        }
    }

    pub fn selected(&self) -> Style {
        if self.is_transparent {
            Style::default()
                .fg(self.selection_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.selection_fg)
                .bg(self.selection_bg)
                .add_modifier(Modifier::BOLD)
        }
    }

    pub fn border(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.border)
        } else {
            Style::default().fg(self.border).bg(self.bg)
        }
    }

    pub fn border_focused(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.border_focused)
        } else {
            Style::default().fg(self.border_focused).bg(self.bg)
        }
    }

    pub fn tab_inactive(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.fg_dim)
        } else {
            Style::default().fg(self.fg_dim).bg(self.bg)
        }
    }

    pub fn tab_active(&self) -> Style {
        if self.is_transparent {
            Style::default()
                .fg(self.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.accent)
                .bg(self.bg)
                .add_modifier(Modifier::BOLD)
        }
    }

    pub fn success(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.success)
        } else {
            Style::default().fg(self.success).bg(self.bg)
        }
    }

    pub fn warning(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.warning)
        } else {
            Style::default().fg(self.warning).bg(self.bg)
        }
    }

    pub fn error(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.error)
        } else {
            Style::default().fg(self.error).bg(self.bg)
        }
    }

    pub fn diff_added(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.diff_added)
        } else {
            Style::default().fg(self.diff_added).bg(self.bg)
        }
    }

    pub fn diff_removed(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.diff_removed)
        } else {
            Style::default().fg(self.diff_removed).bg(self.bg)
        }
    }

    pub fn diff_updated(&self) -> Style {
        if self.is_transparent {
            Style::default().fg(self.diff_updated)
        } else {
            Style::default().fg(self.diff_updated).bg(self.bg)
        }
    }

    pub fn block_style(&self) -> Style {
        if self.is_transparent {
            Style::default()
        } else {
            Style::default().bg(self.bg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_from_name() {
        let gruvbox = Theme::from_name(ThemeName::Gruvbox);
        assert_eq!(gruvbox.bg, Color::Rgb(40, 40, 40));
        assert!(!gruvbox.is_transparent);

        let nord = Theme::from_name(ThemeName::Nord);
        assert_eq!(nord.bg, Color::Rgb(46, 52, 64));

        let transparent = Theme::from_name(ThemeName::Transparent);
        assert!(transparent.is_transparent);
    }
}
