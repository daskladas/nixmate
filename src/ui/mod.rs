//! User Interface layer for nixmate
//!
//! Contains all UI-related code:
//! - Theme definitions and colors (global for all modules)
//! - Reusable widgets
//! - Main render loop with module routing
//! - Tab bar, logo, status bar

pub mod render;
pub mod theme;
pub mod widgets;

pub use render::render;
pub use render::ModuleTab;
pub use theme::Theme;
