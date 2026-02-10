//! Generations module (formerly nixhist)
//!
//! Integrated into nixmate as an inline module.
//! Has sub-tabs: Overview, Packages, Diff, Manage.
//! Uses nixmate's global theme, i18n, and config.

use crate::config::Language;
use crate::i18n;
use crate::nix::{self, CommandResult, GenerationSource};
use crate::types::FlashMessage;
use crate::types::{Generation, GenerationDiff, Package, ProfileType};
use crate::ui::theme::Theme;
use crate::ui::widgets;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};
use std::collections::HashSet;
use std::time::Instant;

// ── Sub-tabs ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GenSubTab {
    #[default]
    Overview,
    Packages,
    Diff,
    Manage,
}

impl GenSubTab {
    pub fn all() -> &'static [GenSubTab] {
        &[
            GenSubTab::Overview,
            GenSubTab::Packages,
            GenSubTab::Diff,
            GenSubTab::Manage,
        ]
    }

    pub fn index(&self) -> usize {
        match self {
            GenSubTab::Overview => 0,
            GenSubTab::Packages => 1,
            GenSubTab::Diff => 2,
            GenSubTab::Manage => 3,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            GenSubTab::Overview => s.gen_overview,
            GenSubTab::Packages => s.gen_packages,
            GenSubTab::Diff => s.gen_diff,
            GenSubTab::Manage => s.gen_manage,
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
pub enum GenPopupState {
    None,
    Confirm {
        title: String,
        message: String,
        command: String,
    },
    Error {
        title: String,
        message: String,
    },
    Undo {
        message: String,
        seconds_remaining: u8,
    },
}

#[derive(Debug, Clone)]
pub struct PendingUndo {
    #[allow(dead_code)] // Stored for future undo implementation
    pub action: UndoAction,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub enum UndoAction {
    Delete {
        _profile: ProfileType,
        _generation_ids: Vec<u32>,
    },
}

// ── Module state ──

pub struct GenerationsState {
    // Data
    #[allow(dead_code)] // Set during init, used by sub-views
    pub hostname: String,
    #[allow(dead_code)] // Set during init, used by sub-views
    pub username: String,
    #[allow(dead_code)] // Set during init, used by sub-views
    pub uses_flakes: bool,
    #[allow(dead_code)] // Set during init, used by sub-views
    pub has_home_manager: bool,

    pub system_generations: Vec<Generation>,
    pub system_source: GenerationSource,
    pub home_manager_generations: Vec<Generation>,
    pub home_manager_source: Option<GenerationSource>,
    pub dry_run: bool,

    // Diagnostic: any errors during init
    pub init_errors: Vec<String>,

    // Navigation
    pub active_sub_tab: GenSubTab,

    // Overview
    pub overview_focus: usize, // 0 = system, 1 = HM
    pub overview_system_selected: usize,
    pub overview_hm_selected: usize,

    // Packages
    pub packages_list: Vec<Package>,
    pub packages_gen_id: Option<u32>,
    pub packages_profile: ProfileType,
    pub packages_selected: usize,
    pub packages_filter: String,
    pub packages_filter_active: bool,
    #[allow(dead_code)] // Set during init, used by sub-views
    pub packages_loading: bool,

    // Diff
    pub diff_focus: usize,
    pub diff_from_cursor: usize,
    pub diff_to_cursor: usize,
    pub diff_from_gen: Option<u32>,
    pub diff_to_gen: Option<u32>,
    pub diff_scroll: usize,
    pub current_diff: Option<GenerationDiff>,

    // Manage
    pub manage_profile: ProfileType,
    pub manage_cursor: usize,
    pub manage_selected: HashSet<u32>,

    // Pinned
    pub pinned_system: HashSet<u32>,
    pub pinned_hm: HashSet<u32>,

    // Popup
    pub popup: GenPopupState,
    pub pending_undo: Option<PendingUndo>,

    // Flash
    pub lang: Language,
    pub flash_message: Option<FlashMessage>,
}

impl GenerationsState {
    /// Initialize the generations module.
    /// This ALWAYS succeeds – errors are stored for display, not propagated.
    pub fn new(dry_run: bool) -> Self {
        let mut init_errors = Vec::new();

        // Detect system
        let (hostname, username, uses_flakes, system_profile, hm_info) = match nix::detect_system()
        {
            Ok(info) => (
                info.hostname,
                info.username,
                info.uses_flakes,
                info.system_profile,
                info.home_manager,
            ),
            Err(e) => {
                init_errors.push(format!("System detection failed: {}", e));
                (
                    "unknown".into(),
                    "unknown".into(),
                    false,
                    std::path::PathBuf::from("/nix/var/nix/profiles/system"),
                    None,
                )
            }
        };

        // Load system generations
        let system_source = GenerationSource {
            profile_type: ProfileType::System,
            profile_path: system_profile,
        };

        let system_generations = match nix::list_generations(&system_source) {
            Ok(gens) => gens,
            Err(e) => {
                init_errors.push(format!("System generations: {}", e));
                Vec::new()
            }
        };

        // Load HM generations
        let has_home_manager = hm_info.is_some();
        let (home_manager_source, home_manager_generations) = if let Some(hm) = hm_info {
            let source = GenerationSource {
                profile_type: ProfileType::HomeManager,
                profile_path: hm.profile_path,
            };
            match nix::list_generations(&source) {
                Ok(gens) => (Some(source), gens),
                Err(e) => {
                    init_errors.push(format!("Home-Manager generations: {}", e));
                    (None, Vec::new())
                }
            }
        } else {
            (None, Vec::new())
        };

        Self {
            hostname,
            username,
            uses_flakes,
            has_home_manager,

            system_generations,
            system_source,
            home_manager_generations,
            home_manager_source,
            dry_run,

            init_errors,

            active_sub_tab: GenSubTab::Overview,

            overview_focus: 0,
            overview_system_selected: 0,
            overview_hm_selected: 0,

            packages_list: Vec::new(),
            packages_gen_id: None,
            packages_profile: ProfileType::System,
            packages_selected: 0,
            packages_filter: String::new(),
            packages_filter_active: false,
            packages_loading: false,

            diff_focus: 0,
            diff_from_cursor: 0,
            diff_to_cursor: 0,
            diff_from_gen: None,
            diff_to_gen: None,
            diff_scroll: 0,
            current_diff: None,

            manage_profile: ProfileType::System,
            manage_cursor: 0,
            manage_selected: HashSet::new(),

            pinned_system: HashSet::new(),
            pinned_hm: HashSet::new(),

            popup: GenPopupState::None,
            pending_undo: None,
            lang: Language::English,
            flash_message: None,
        }
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Clear expired flash
        if let Some(msg) = &self.flash_message {
            if msg.is_expired(3) {
                self.flash_message = None;
            }
        }

        // Handle popup states first
        match &self.popup {
            GenPopupState::Confirm { .. } => return self.handle_confirm_key(key),
            GenPopupState::Error { .. } => return self.handle_error_key(key),
            GenPopupState::Undo { .. } => return self.handle_undo_key(key),
            GenPopupState::None => {}
        }

        // Sub-tab switching with [ / ]
        match key.code {
            KeyCode::Char('[') => {
                self.active_sub_tab = self.active_sub_tab.prev();
                return Ok(());
            }
            KeyCode::Char(']') => {
                self.active_sub_tab = self.active_sub_tab.next();
                return Ok(());
            }
            _ => {}
        }

        match self.active_sub_tab {
            GenSubTab::Overview => self.handle_overview_key(key),
            GenSubTab::Packages => self.handle_packages_key(key),
            GenSubTab::Diff => self.handle_diff_key(key),
            GenSubTab::Manage => self.handle_manage_key(key),
        }
    }

    /// Update undo timer
    pub fn update_undo_timer(&mut self) -> Result<()> {
        if let Some(pending) = &self.pending_undo {
            let elapsed = pending.started_at.elapsed().as_secs() as u8;
            let remaining = 10u8.saturating_sub(elapsed);

            if remaining == 0 {
                self.pending_undo = None;
                self.popup = GenPopupState::None;
                let s = crate::i18n::get_strings(self.lang);
                self.show_flash(s.gen_action_confirmed, false);
            } else if let GenPopupState::Undo { message, .. } = &self.popup {
                self.popup = GenPopupState::Undo {
                    message: message.clone(),
                    seconds_remaining: remaining,
                };
            }
        }
        Ok(())
    }

    // ── Key handlers ──

    fn handle_overview_key(&mut self, key: KeyEvent) -> Result<()> {
        let has_hm = !self.home_manager_generations.is_empty();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.overview_focus == 0 {
                    let max = self.system_generations.len().saturating_sub(1);
                    if self.overview_system_selected < max {
                        self.overview_system_selected += 1;
                    }
                } else {
                    let max = self.home_manager_generations.len().saturating_sub(1);
                    if self.overview_hm_selected < max {
                        self.overview_hm_selected += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.overview_focus == 0 {
                    self.overview_system_selected = self.overview_system_selected.saturating_sub(1);
                } else {
                    self.overview_hm_selected = self.overview_hm_selected.saturating_sub(1);
                }
            }
            KeyCode::Char('g') => {
                if self.overview_focus == 0 {
                    self.overview_system_selected = 0;
                } else {
                    self.overview_hm_selected = 0;
                }
            }
            KeyCode::Char('G') => {
                if self.overview_focus == 0 {
                    self.overview_system_selected = self.system_generations.len().saturating_sub(1);
                } else {
                    self.overview_hm_selected =
                        self.home_manager_generations.len().saturating_sub(1);
                }
            }
            KeyCode::Tab => {
                if has_hm {
                    self.overview_focus = (self.overview_focus + 1) % 2;
                }
            }
            KeyCode::Enter => {
                let (gen, profile) = if self.overview_focus == 0 {
                    (
                        self.system_generations.get(self.overview_system_selected),
                        ProfileType::System,
                    )
                } else {
                    (
                        self.home_manager_generations.get(self.overview_hm_selected),
                        ProfileType::HomeManager,
                    )
                };

                if let Some(gen) = gen {
                    let gen_id = gen.id;
                    self.load_packages(gen_id, profile)?;
                    self.active_sub_tab = GenSubTab::Packages;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_packages_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.packages_filter_active {
            match key.code {
                KeyCode::Esc => {
                    self.packages_filter.clear();
                    self.packages_filter_active = false;
                    self.packages_selected = 0;
                }
                KeyCode::Enter => {
                    self.packages_filter_active = false;
                }
                KeyCode::Backspace => {
                    self.packages_filter.pop();
                    self.packages_selected = 0;
                }
                KeyCode::Char(c) => {
                    self.packages_filter.push(c);
                    self.packages_selected = 0;
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('/') => {
                self.packages_filter_active = true;
                self.packages_filter.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let count = self.filtered_packages_count();
                if count > 0 && self.packages_selected < count - 1 {
                    self.packages_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.packages_selected = self.packages_selected.saturating_sub(1);
            }
            KeyCode::Char('g') => self.packages_selected = 0,
            KeyCode::Char('G') => {
                let count = self.filtered_packages_count();
                if count > 0 {
                    self.packages_selected = count - 1;
                }
            }
            KeyCode::Esc => {
                self.active_sub_tab = GenSubTab::Overview;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_diff_key(&mut self, key: KeyEvent) -> Result<()> {
        let gen_count = self.system_generations.len();
        if gen_count == 0 {
            return Ok(());
        }

        match key.code {
            KeyCode::Tab => {
                self.diff_focus = (self.diff_focus + 1) % 2;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let max = gen_count.saturating_sub(1);
                if self.diff_focus == 0 {
                    if self.diff_from_cursor < max {
                        self.diff_from_cursor += 1;
                    }
                } else if self.diff_to_cursor < max {
                    self.diff_to_cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.diff_focus == 0 {
                    self.diff_from_cursor = self.diff_from_cursor.saturating_sub(1);
                } else {
                    self.diff_to_cursor = self.diff_to_cursor.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                if let Some(gen) = self.system_generations.get(if self.diff_focus == 0 {
                    self.diff_from_cursor
                } else {
                    self.diff_to_cursor
                }) {
                    let id = gen.id;
                    if self.diff_focus == 0 {
                        self.diff_from_gen = Some(id);
                    } else {
                        self.diff_to_gen = Some(id);
                    }

                    if self.diff_from_gen.is_some() && self.diff_to_gen.is_some() {
                        self.calculate_diff()?;
                    }
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.diff_from_gen = None;
                self.diff_to_gen = None;
                self.current_diff = None;
                self.diff_scroll = 0;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_manage_key(&mut self, key: KeyEvent) -> Result<()> {
        let generations = self.get_manage_generations();
        let gen_count = generations.len();
        if gen_count == 0 {
            return Ok(());
        }

        match key.code {
            KeyCode::Tab => {
                if !self.home_manager_generations.is_empty() {
                    self.manage_profile = match self.manage_profile {
                        ProfileType::System => ProfileType::HomeManager,
                        ProfileType::HomeManager => ProfileType::System,
                    };
                    self.manage_cursor = 0;
                    self.manage_selected.clear();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.manage_cursor < gen_count.saturating_sub(1) {
                    self.manage_cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.manage_cursor = self.manage_cursor.saturating_sub(1);
            }
            KeyCode::Char(' ') => {
                if let Some(gen) = generations.get(self.manage_cursor) {
                    if !gen.is_current {
                        let id = gen.id;
                        if self.manage_selected.contains(&id) {
                            self.manage_selected.remove(&id);
                        } else {
                            self.manage_selected.insert(id);
                        }
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                for gen in &generations {
                    if !gen.is_current && !gen.is_pinned {
                        self.manage_selected.insert(gen.id);
                    }
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.manage_selected.clear();
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if let Some(gen) = generations.get(self.manage_cursor) {
                    let id = gen.id;
                    self.toggle_pin(id);
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.prompt_restore()?;
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.prompt_delete()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.execute_pending_action()?;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.popup = GenPopupState::None;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_error_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('o') | KeyCode::Enter | KeyCode::Esc => {
                self.popup = GenPopupState::None;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_undo_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') | KeyCode::Esc => {
                self.pending_undo = None;
                self.popup = GenPopupState::None;
                let s = crate::i18n::get_strings(self.lang);
                self.show_flash(s.gen_undo_closed, false);
            }
            _ => {}
        }
        Ok(())
    }

    // ── Helpers ──

    fn get_manage_generations(&self) -> Vec<Generation> {
        if self.manage_profile == ProfileType::System {
            self.system_generations.clone()
        } else {
            self.home_manager_generations.clone()
        }
    }

    fn load_packages(&mut self, gen_id: u32, profile: ProfileType) -> Result<()> {
        let source = if profile == ProfileType::System {
            &self.system_source
        } else {
            match &self.home_manager_source {
                Some(s) => s,
                None => &self.system_source,
            }
        };

        let prefix = if profile == ProfileType::System {
            "system"
        } else {
            "home-manager"
        };

        let gen_path = source
            .profile_path
            .parent()
            .unwrap_or(&source.profile_path)
            .join(format!("{}-{}-link", prefix, gen_id));

        self.packages_list = nix::get_packages(&gen_path).unwrap_or_default();
        if self.packages_list.is_empty() {
            let msg = if !gen_path.exists() {
                format!("Generation path not found: {}", gen_path.display())
            } else {
                format!("No packages found in {}", gen_path.display())
            };
            self.packages_list = vec![crate::types::Package {
                name: msg,
                version: String::new(),
                size: 0,
            }];
        }
        self.packages_gen_id = Some(gen_id);
        self.packages_profile = profile;
        self.packages_selected = 0;
        self.packages_filter.clear();
        self.packages_filter_active = false;

        Ok(())
    }

    fn filtered_packages(&self) -> Vec<&Package> {
        if self.packages_filter.is_empty() {
            self.packages_list.iter().collect()
        } else {
            let filter_lower = self.packages_filter.to_lowercase();
            self.packages_list
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&filter_lower))
                .collect()
        }
    }

    fn filtered_packages_count(&self) -> usize {
        self.filtered_packages().len()
    }

    fn calculate_diff(&mut self) -> Result<()> {
        let (from_id, to_id) = match (self.diff_from_gen, self.diff_to_gen) {
            (Some(from), Some(to)) => (from, to),
            _ => return Ok(()),
        };

        let parent = self
            .system_source
            .profile_path
            .parent()
            .unwrap_or(&self.system_source.profile_path);

        let from_path = parent.join(format!("system-{}-link", from_id));
        let to_path = parent.join(format!("system-{}-link", to_id));

        let from_packages = nix::get_packages(&from_path).unwrap_or_default();
        let to_packages = nix::get_packages(&to_path).unwrap_or_default();

        self.current_diff = Some(GenerationDiff::calculate(&from_packages, &to_packages));
        self.diff_scroll = 0;

        Ok(())
    }

    fn toggle_pin(&mut self, gen_id: u32) {
        let pinned = if self.manage_profile == ProfileType::System {
            &mut self.pinned_system
        } else {
            &mut self.pinned_hm
        };

        if pinned.contains(&gen_id) {
            pinned.remove(&gen_id);
        } else {
            pinned.insert(gen_id);
        }

        let gens = if self.manage_profile == ProfileType::System {
            &mut self.system_generations
        } else {
            &mut self.home_manager_generations
        };

        if let Some(gen) = gens.iter_mut().find(|g| g.id == gen_id) {
            gen.is_pinned = pinned.contains(&gen_id);
        }

        let s = crate::i18n::get_strings(self.lang);
        self.show_flash(s.gen_pin_updated, false);
    }

    fn prompt_restore(&mut self) -> Result<()> {
        let generations = self.get_manage_generations();
        let gen = match generations.get(self.manage_cursor) {
            Some(g) if !g.is_current => g,
            _ => {
                let s = crate::i18n::get_strings(self.lang);
                self.show_flash(s.gen_cannot_restore_current, true);
                return Ok(());
            }
        };

        let source = if self.manage_profile == ProfileType::System {
            &self.system_source
        } else {
            match &self.home_manager_source {
                Some(s) => s,
                None => &self.system_source,
            }
        };

        let command = nix::commands::get_restore_command_preview(
            &source.profile_path,
            gen.id,
            self.manage_profile,
        );

        let s = crate::i18n::get_strings(self.lang);
        self.popup = GenPopupState::Confirm {
            title: s.gen_confirm_restore.into(),
            message: s
                .gen_restore_msg
                .replacen("{}", self.manage_profile.as_str(), 1)
                .replacen("{}", &gen.id.to_string(), 1)
                .replacen("{}", &gen.formatted_date(), 1)
                .replacen("{}", gen.nixos_version.as_deref().unwrap_or("?"), 1),
            command,
        };

        Ok(())
    }

    fn prompt_delete(&mut self) -> Result<()> {
        let generations = self.get_manage_generations();

        let ids: Vec<u32> = if self.manage_selected.is_empty() {
            match generations.get(self.manage_cursor) {
                Some(g) if g.is_current => {
                    let s = crate::i18n::get_strings(self.lang);
                    self.show_flash(s.gen_cannot_delete_current, true);
                    return Ok(());
                }
                Some(g) if g.is_pinned => {
                    let s = crate::i18n::get_strings(self.lang);
                    self.show_flash(s.gen_cannot_delete_pinned, true);
                    return Ok(());
                }
                Some(g) => vec![g.id],
                _ => return Ok(()),
            }
        } else {
            self.manage_selected.iter().copied().collect()
        };

        if ids.is_empty() {
            return Ok(());
        }

        let source = if self.manage_profile == ProfileType::System {
            &self.system_source
        } else {
            match &self.home_manager_source {
                Some(s) => s,
                None => &self.system_source,
            }
        };

        let command = nix::commands::get_delete_command_preview(
            &source.profile_path,
            &ids,
            self.manage_profile,
        );

        let s = crate::i18n::get_strings(self.lang);
        self.popup = GenPopupState::Confirm {
            title: s.gen_confirm_delete.into(),
            message: s
                .gen_delete_msg
                .replacen("{}", &ids.len().to_string(), 1)
                .replacen("{}", &format!("{:?}", ids), 1),
            command,
        };

        Ok(())
    }

    fn execute_pending_action(&mut self) -> Result<()> {
        let title = match &self.popup {
            GenPopupState::Confirm { title, .. } => title.clone(),
            _ => return Ok(()),
        };

        let s = crate::i18n::get_strings(self.lang);
        let result = if title == s.gen_confirm_restore {
            self.execute_restore()
        } else if title == s.gen_confirm_delete {
            self.execute_delete()
        } else {
            return Ok(());
        };

        match result {
            Ok(cmd_result) if cmd_result.success => {
                self.popup = GenPopupState::None;
                self.show_flash(&cmd_result.message, false);
                let _ = self.refresh_generations();
            }
            Ok(cmd_result) => {
                self.popup = GenPopupState::Error {
                    title: s.gen_command_failed.into(),
                    message: cmd_result.message,
                };
            }
            Err(e) => {
                self.popup = GenPopupState::Error {
                    title: s.error.into(),
                    message: e.to_string(),
                };
            }
        }

        Ok(())
    }

    fn execute_restore(&self) -> Result<CommandResult> {
        let generations = self.get_manage_generations();
        let gen = generations
            .get(self.manage_cursor)
            .ok_or_else(|| anyhow::anyhow!("No generation selected"))?;

        let source = if self.manage_profile == ProfileType::System {
            &self.system_source
        } else {
            self.home_manager_source
                .as_ref()
                .unwrap_or(&self.system_source)
        };

        nix::restore_generation(
            &source.profile_path,
            gen.id,
            self.manage_profile,
            self.dry_run,
        )
    }

    fn execute_delete(&mut self) -> Result<CommandResult> {
        let generations = self.get_manage_generations();
        let ids: Vec<u32> = if self.manage_selected.is_empty() {
            generations
                .get(self.manage_cursor)
                .map(|g| vec![g.id])
                .unwrap_or_default()
        } else {
            self.manage_selected.iter().copied().collect()
        };

        let source = if self.manage_profile == ProfileType::System {
            &self.system_source
        } else {
            self.home_manager_source
                .as_ref()
                .unwrap_or(&self.system_source)
        };

        let result = nix::delete_generations(
            &source.profile_path,
            &ids,
            self.manage_profile,
            self.dry_run,
        )?;

        if result.success && !self.dry_run {
            self.pending_undo = Some(PendingUndo {
                action: UndoAction::Delete {
                    _profile: self.manage_profile,
                    _generation_ids: ids.clone(),
                },
                started_at: Instant::now(),
            });
            self.popup = GenPopupState::Undo {
                message: {
                    let s = crate::i18n::get_strings(self.lang);
                    s.gen_deleted_count.replace("{}", &ids.len().to_string())
                },
                seconds_remaining: 10,
            };
        }

        self.manage_selected.clear();
        Ok(result)
    }

    fn refresh_generations(&mut self) -> Result<()> {
        self.system_generations = nix::list_generations(&self.system_source).unwrap_or_default();
        for gen in &mut self.system_generations {
            gen.is_pinned = self.pinned_system.contains(&gen.id);
        }

        if let Some(source) = &self.home_manager_source {
            if let Ok(mut gens) = nix::list_generations(source) {
                for gen in &mut gens {
                    gen.is_pinned = self.pinned_hm.contains(&gen.id);
                }
                self.home_manager_generations = gens;
            }
        }

        Ok(())
    }

    fn show_flash(&mut self, message: &str, is_error: bool) {
        self.flash_message = Some(FlashMessage::new(message.into(), is_error));
    }
}

// ══════════════════════════════════════════════════════════════
//  RENDERING
// ══════════════════════════════════════════════════════════════

/// Render the generations module
pub fn render(
    frame: &mut Frame,
    state: &GenerationsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    // Full background
    frame.render_widget(Block::default().style(theme.block_style()), area);

    // Layout: sub-tab bar + content
    let chunks = Layout::vertical([
        Constraint::Length(2), // Sub-tab bar
        Constraint::Min(5),    // Content
    ])
    .split(area);

    render_sub_tab_bar(frame, state, theme, lang, chunks[0]);

    match state.active_sub_tab {
        GenSubTab::Overview => render_overview(frame, state, theme, chunks[1]),
        GenSubTab::Packages => render_packages(frame, state, theme, chunks[1]),
        GenSubTab::Diff => render_diff(frame, state, theme, chunks[1]),
        GenSubTab::Manage => render_manage(frame, state, theme, chunks[1]),
    }

    // Module popups (on top of everything)
    render_gen_popups(frame, state, theme, area);

    // Flash message
    if let Some(msg) = &state.flash_message {
        widgets::render_flash_message(frame, &msg.text, msg.is_error, theme, area);
    }
}

fn render_sub_tab_bar(
    frame: &mut Frame,
    state: &GenerationsState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    // Background for sub-tab area
    frame.render_widget(Block::default().style(theme.block_style()), area);

    let tab_titles: Vec<Line> = GenSubTab::all()
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

    // Render tabs with padding
    let tabs_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height.min(1),
    };
    frame.render_widget(tabs, tabs_area);
}

// ── Overview ──

fn render_overview(frame: &mut Frame, state: &GenerationsState, theme: &Theme, area: Rect) {
    let s = crate::i18n::get_strings(state.lang);
    let has_hm = !state.home_manager_generations.is_empty();
    let use_side_by_side = has_hm && area.width >= 100;

    // Show init errors if any
    if !state.init_errors.is_empty() && state.system_generations.is_empty() {
        let block = Block::default()
            .style(theme.block_style())
            .title(" Generations ")
            .title_style(theme.title())
            .borders(Borders::ALL)
            .border_style(theme.border_focused());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines = vec![
            Line::raw(""),
            Line::styled("⚠ Could not load generations:", theme.warning()),
            Line::raw(""),
        ];
        for err in &state.init_errors {
            lines.push(Line::styled(format!("  • {}", err), theme.error()));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Make sure you're running on NixOS with nix-env in PATH.",
            theme.text_dim(),
        ));

        frame.render_widget(
            Paragraph::new(lines)
                .style(theme.text())
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    if use_side_by_side {
        let panels = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        render_gen_list(
            frame,
            s.gen_system_label,
            &state.system_generations,
            state.overview_system_selected,
            state.overview_focus == 0,
            theme,
            panels[0],
        );

        render_gen_list(
            frame,
            s.gen_hm_label,
            &state.home_manager_generations,
            state.overview_hm_selected,
            state.overview_focus == 1,
            theme,
            panels[1],
        );
    } else if has_hm {
        // Stacked: show active panel only, with Tab hint
        let (title, gens, selected) = if state.overview_focus == 0 {
            (
                s.gen_system_label,
                &state.system_generations,
                state.overview_system_selected,
            )
        } else {
            (
                s.gen_hm_label,
                &state.home_manager_generations,
                state.overview_hm_selected,
            )
        };
        render_gen_list(frame, title, gens, selected, true, theme, area);
    } else {
        // System only
        render_gen_list(
            frame,
            s.gen_system_label,
            &state.system_generations,
            state.overview_system_selected,
            true,
            theme,
            area,
        );
    }
}

fn render_gen_list(
    frame: &mut Frame,
    title: &str,
    generations: &[Generation],
    selected: usize,
    is_focused: bool,
    theme: &Theme,
    area: Rect,
) {
    let border_style = if is_focused {
        theme.border_focused()
    } else {
        theme.border()
    };

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ({}) ", title, generations.len()))
        .title_style(if is_focused {
            theme.title()
        } else {
            theme.text_dim()
        })
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if generations.is_empty() {
        let msg = Paragraph::new(title)
            .style(theme.text_dim())
            .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
        return;
    }

    // Reserve 2 lines at bottom for details
    let list_height = inner.height.saturating_sub(2) as usize;
    if list_height == 0 {
        return;
    }

    // Calculate scroll offset to keep selected visible
    let scroll_offset = if selected >= list_height {
        selected - list_height + 1
    } else {
        0
    };

    let list_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: list_height as u16,
    };

    let items: Vec<ListItem> = generations
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(list_height)
        .map(|(i, gen)| {
            let marker = if gen.is_current {
                "● "
            } else if gen.is_pinned {
                "★ "
            } else if gen.in_bootloader {
                "⚡"
            } else {
                "  "
            };

            let version_str = gen.nixos_version.as_deref().unwrap_or("-");
            let line_text = format!(
                "{}#{:<4} {}  {}  {} pkgs  {}",
                marker,
                gen.id,
                gen.formatted_date(),
                version_str,
                gen.package_count,
                gen.formatted_size(),
            );

            let style = if i == selected {
                theme.selected()
            } else {
                theme.text()
            };

            ListItem::new(Line::styled(line_text, style))
        })
        .collect();

    let list = List::new(items).style(theme.block_style());
    frame.render_widget(list, list_area);

    // Detail line at bottom
    if let Some(gen) = generations.get(selected) {
        let detail_area = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        let kernel = gen.kernel_version.as_deref().unwrap_or("");
        let store = if gen.store_path.len() > 50 {
            &gen.store_path[gen.store_path.len() - 50..]
        } else {
            &gen.store_path
        };

        let detail_text = if kernel.is_empty() {
            format!("  {}", store)
        } else {
            format!("  Kernel: {} │ {}", kernel, store)
        };

        frame.render_widget(
            Paragraph::new(detail_text).style(theme.text_dim()),
            detail_area,
        );
    }
}

// ── Packages ──

fn render_packages(frame: &mut Frame, state: &GenerationsState, theme: &Theme, area: Rect) {
    let s = crate::i18n::get_strings(state.lang);
    let title = match state.packages_gen_id {
        Some(id) => format!(" Packages · Generation #{} ", id),
        None => " Packages ".to_string(),
    };

    let block = Block::default()
        .style(theme.block_style())
        .title(title)
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if state.packages_gen_id.is_none() {
        let hint = Paragraph::new(s.gen_select_hint)
            .style(theme.text_dim())
            .alignment(Alignment::Center);
        let centered = Rect {
            x: inner.x,
            y: inner.y + inner.height / 2,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(hint, centered);
        return;
    }

    // Filter bar
    let filter_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let cursor = if state.packages_filter_active {
        "█"
    } else {
        ""
    };
    let filter_text = format!(" Filter: {}{}", state.packages_filter, cursor);
    let filter_style = if state.packages_filter_active {
        theme.title()
    } else {
        theme.text_dim()
    };
    frame.render_widget(Paragraph::new(filter_text).style(filter_style), filter_area);

    // Package table area
    let table_area = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(4),
    };

    if table_area.height == 0 {
        return;
    }

    let filtered = state.filtered_packages();

    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new(s.gen_no_match_filter)
                .style(theme.text_dim())
                .alignment(Alignment::Center),
            table_area,
        );
        return;
    }

    // Table header
    let header = Row::new(vec![
        Cell::from(" NAME").style(theme.title()),
        Cell::from("VERSION").style(theme.title()),
        Cell::from("SIZE").style(theme.title()),
    ])
    .style(theme.block_style());

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let style = if i == state.packages_selected {
                theme.selected()
            } else {
                theme.text()
            };

            Row::new(vec![
                Cell::from(format!(" {}", pkg.name)),
                Cell::from(pkg.version.clone()),
                Cell::from(pkg.formatted_size()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Percentage(50),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .style(theme.block_style());

    frame.render_widget(table, table_area);

    // Count at bottom
    let count_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    let pos = (state.packages_selected + 1).min(filtered.len());
    frame.render_widget(
        Paragraph::new(format!(" {} / {} packages", pos, filtered.len()))
            .style(theme.text_dim())
            .alignment(Alignment::Right),
        count_area,
    );
}

// ── Diff ──

fn render_diff(frame: &mut Frame, state: &GenerationsState, theme: &Theme, area: Rect) {
    let s = crate::i18n::get_strings(state.lang);
    let block = Block::default()
        .style(theme.block_style())
        .title(" Compare Generations ")
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if state.system_generations.is_empty() {
        frame.render_widget(
            Paragraph::new(s.gen_no_comparison)
                .style(theme.text_dim())
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    // Split: top for selection lists, bottom for results
    let selector_height = (inner.height / 3).clamp(5, 12);
    let chunks =
        Layout::vertical([Constraint::Length(selector_height), Constraint::Min(3)]).split(inner);

    // Two selection lists side by side
    let lists = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    render_diff_selector(
        frame,
        s.gen_from,
        &state.system_generations,
        state.diff_from_cursor,
        state.diff_from_gen,
        state.diff_focus == 0,
        theme,
        lists[0],
    );

    render_diff_selector(
        frame,
        s.gen_to,
        &state.system_generations,
        state.diff_to_cursor,
        state.diff_to_gen,
        state.diff_focus == 1,
        theme,
        lists[1],
    );

    // Results
    let results_block = Block::default()
        .style(theme.block_style())
        .title(" Results ")
        .title_style(theme.text_dim())
        .borders(Borders::ALL)
        .border_style(theme.border());

    let results_inner = results_block.inner(chunks[1]);
    frame.render_widget(results_block, chunks[1]);

    if results_inner.height == 0 {
        return;
    }

    if state.diff_from_gen.is_none() || state.diff_to_gen.is_none() {
        frame.render_widget(
            Paragraph::new(s.gen_diff_hint)
                .style(theme.text_dim())
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false }),
            results_inner,
        );
    } else if let Some(diff) = &state.current_diff {
        render_diff_results(frame, diff, state.diff_scroll, theme, results_inner);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_diff_selector(
    frame: &mut Frame,
    title: &str,
    generations: &[Generation],
    cursor: usize,
    selected_id: Option<u32>,
    is_focused: bool,
    theme: &Theme,
    area: Rect,
) {
    let full_title = match selected_id {
        Some(id) => format!(" {} (#{}) ", title, id),
        None => format!(" {} ", title),
    };

    let block = Block::default()
        .style(theme.block_style())
        .title(full_title)
        .title_style(if is_focused {
            theme.title()
        } else {
            theme.text_dim()
        })
        .borders(Borders::ALL)
        .border_style(if is_focused {
            theme.border_focused()
        } else {
            theme.border()
        });

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let items: Vec<ListItem> = generations
        .iter()
        .enumerate()
        .map(|(i, gen)| {
            let check = if Some(gen.id) == selected_id {
                "● "
            } else {
                "  "
            };

            let text = format!("{}#{:<4} {}", check, gen.id, gen.formatted_date());

            let style = if i == cursor && is_focused {
                theme.selected()
            } else if Some(gen.id) == selected_id {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.text()
            };

            ListItem::new(Line::styled(text, style))
        })
        .collect();

    frame.render_widget(List::new(items).style(theme.block_style()), inner);
}

fn render_diff_results(
    frame: &mut Frame,
    diff: &GenerationDiff,
    scroll: usize,
    theme: &Theme,
    area: Rect,
) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::styled(
        format!(
            " +{} added  -{} removed  ~{} updated",
            diff.added.len(),
            diff.removed.len(),
            diff.updated.len()
        ),
        theme.title(),
    ));
    lines.push(Line::raw(""));

    if !diff.added.is_empty() {
        lines.push(Line::styled(
            format!("  Added ({})", diff.added.len()),
            theme.diff_added(),
        ));
        for pkg in &diff.added {
            lines.push(Line::from(vec![
                Span::styled("   + ", theme.diff_added()),
                Span::styled(&pkg.name, theme.text()),
                Span::raw(" "),
                Span::styled(&pkg.version, theme.text_dim()),
            ]));
        }
        lines.push(Line::raw(""));
    }

    if !diff.removed.is_empty() {
        lines.push(Line::styled(
            format!("  Removed ({})", diff.removed.len()),
            theme.diff_removed(),
        ));
        for pkg in &diff.removed {
            lines.push(Line::from(vec![
                Span::styled("   - ", theme.diff_removed()),
                Span::styled(&pkg.name, theme.text()),
                Span::raw(" "),
                Span::styled(&pkg.version, theme.text_dim()),
            ]));
        }
        lines.push(Line::raw(""));
    }

    if !diff.updated.is_empty() {
        lines.push(Line::styled(
            format!("  Updated ({})", diff.updated.len()),
            theme.diff_updated(),
        ));
        for upd in &diff.updated {
            let mut spans = vec![
                Span::styled("   ~ ", theme.diff_updated()),
                Span::styled(&upd.name, theme.text()),
                Span::raw(" "),
                Span::styled(&upd.old_version, theme.text_dim()),
                Span::raw(" → "),
                Span::styled(&upd.new_version, theme.text()),
            ];
            if upd.is_kernel {
                spans.push(Span::styled(" [kernel]", theme.warning()));
            } else if upd.is_security {
                spans.push(Span::styled(" [security]", theme.warning()));
            }
            lines.push(Line::from(spans));
        }
    }

    if diff.added.is_empty() && diff.removed.is_empty() && diff.updated.is_empty() {
        lines.push(Line::styled("  No differences found", theme.text_dim()));
    }

    // Apply scroll
    let visible: Vec<Line> = lines
        .into_iter()
        .skip(scroll)
        .take(area.height as usize)
        .collect();

    frame.render_widget(
        Paragraph::new(visible)
            .style(theme.text())
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ── Manage ──

fn render_manage(frame: &mut Frame, state: &GenerationsState, theme: &Theme, area: Rect) {
    let s = crate::i18n::get_strings(state.lang);
    let block = Block::default()
        .style(theme.block_style())
        .title(" Manage Generations ")
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Profile selector (line 0)
    let profile_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let profile_label = format!(
        " Profile: [{}]  (Tab to switch)",
        if state.manage_profile == ProfileType::System {
            s.gen_system_label
        } else {
            s.gen_hm_label
        }
    );
    frame.render_widget(
        Paragraph::new(profile_label).style(theme.text()),
        profile_area,
    );

    // Generation table
    let table_area = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(5),
    };

    if table_area.height == 0 {
        return;
    }

    let generations = if state.manage_profile == ProfileType::System {
        &state.system_generations
    } else {
        &state.home_manager_generations
    };

    if generations.is_empty() {
        frame.render_widget(
            Paragraph::new(s.gen_no_found)
                .style(theme.text_dim())
                .alignment(Alignment::Center),
            table_area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from("  ").style(theme.title()),
        Cell::from(" GEN").style(theme.title()),
        Cell::from("DATE").style(theme.title()),
        Cell::from("SIZE").style(theme.title()),
        Cell::from("STATUS").style(theme.title()),
    ])
    .style(theme.block_style());

    let rows: Vec<Row> = generations
        .iter()
        .enumerate()
        .map(|(i, gen)| {
            let sel_marker = if state.manage_selected.contains(&gen.id) {
                " ■"
            } else {
                " □"
            };

            let status = if gen.is_current {
                "● current"
            } else if gen.is_pinned {
                "★ pinned"
            } else if gen.in_bootloader {
                "⚡ boot"
            } else {
                ""
            };

            let style = if i == state.manage_cursor {
                theme.selected()
            } else {
                theme.text()
            };

            Row::new(vec![
                Cell::from(sel_marker),
                Cell::from(format!(" #{}", gen.id)),
                Cell::from(gen.formatted_date()),
                Cell::from(gen.formatted_size()),
                Cell::from(status),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(8),
        Constraint::Length(16),
        Constraint::Length(12),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .style(theme.block_style());

    frame.render_widget(table, table_area);

    // Actions bar at bottom
    let actions_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 1,
    };

    let sel_count = state.manage_selected.len();
    let actions = if sel_count > 0 {
        format!(
            " {} selected · [R] Restore  [D] Delete  [P] Pin  [C] Clear",
            sel_count
        )
    } else {
        " [Space] Select  [A] Select All  [R] Restore  [D] Delete  [P] Pin".to_string()
    };

    frame.render_widget(
        Paragraph::new(actions).style(theme.text_dim()),
        actions_area,
    );
}

// ── Popups ──

fn render_gen_popups(frame: &mut Frame, state: &GenerationsState, theme: &Theme, area: Rect) {
    let s = crate::i18n::get_strings(state.lang);
    match &state.popup {
        GenPopupState::None => {}
        GenPopupState::Confirm {
            title,
            message,
            command,
        } => {
            let content = vec![
                Line::raw(""),
                Line::styled(message.as_str(), theme.text()),
                Line::raw(""),
                Line::styled(s.gen_command_label, theme.text_dim()),
                Line::styled(command.as_str(), Style::default().fg(theme.fg_dim)),
                Line::raw(""),
            ];
            widgets::render_popup(
                frame,
                title,
                content,
                &[(s.yes, 'y'), (s.cancel, 'n')],
                theme,
                area,
            );
        }
        GenPopupState::Error { title, message } => {
            widgets::render_error_popup(frame, title, message, theme, area);
        }
        GenPopupState::Undo {
            message,
            seconds_remaining,
        } => {
            let bar_width = 30;
            let filled = (*seconds_remaining as usize * bar_width / 10).min(bar_width);
            let empty = bar_width - filled;
            let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

            let content = vec![
                Line::raw(""),
                Line::styled(message.as_str(), theme.text()),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(bar, theme.warning()),
                    Span::raw(format!("  {}s", seconds_remaining)),
                ]),
                Line::raw(""),
            ];
            widgets::render_popup(
                frame,
                s.gen_undo_available,
                content,
                &[(s.gen_dismiss, 'u')],
                theme,
                area,
            );
        }
    }
}
