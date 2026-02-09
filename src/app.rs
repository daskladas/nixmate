//! Application state and event handling for nixmate

use crate::config::Config;
use crate::i18n;
use crate::modules::config_showcase::ConfigShowcaseState;
use crate::modules::errors::ErrorsState;
use crate::modules::flake_inputs::FlakeInputsState;
use crate::modules::generations::GenerationsState;
use crate::modules::health::HealthState;
use crate::modules::options::OptionsState;
use crate::modules::packages::PackagesState;
use crate::modules::rebuild::RebuildState;
use crate::modules::services::ServicesState;
use crate::modules::splash::{self, ImageCache, ImageProtocol, WelcomeState};
use crate::modules::storage::StorageState;
use crate::ui::{ModuleTab, Theme};
use crate::types::FlashMessage;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;

/// Main application state
pub struct App {
    pub should_quit: bool,
    pub active_tab: ModuleTab,
    pub config: Config,
    pub theme: Theme,
    pub settings_selected: usize,
    pub settings_editing: bool,
    pub settings_edit_buffer: String,
    pub popup: PopupState,
    pub flash_message: Option<FlashMessage>,

    // Module intro pages (dismissed per session)
    pub intros_dismissed: HashSet<usize>,

    // Terminal image protocol
    pub image_protocol: ImageProtocol,
    pub image_cache: Option<ImageCache>,
    /// Set by render functions: where the image should be displayed this frame
    pub image_area: Option<(u16, u16, u16, u16)>,
    /// Tracking for efficient Kitty protocol (only re-send on change)
    pub image_displayed: bool,
    pub last_image_area: Option<(u16, u16, u16, u16)>,

    // Module states
    pub welcome: WelcomeState,
    pub generations: GenerationsState,
    pub errors: ErrorsState,
    pub services: ServicesState,
    pub storage: StorageState,
    pub config_showcase: ConfigShowcaseState,
    pub options: OptionsState,
    pub packages: PackagesState,
    pub health: HealthState,
    pub rebuild: RebuildState,
    pub flake_inputs: FlakeInputsState,
}

#[derive(Debug, Clone)]
pub enum PopupState {
    None,
    Error { title: String, message: String },
    #[allow(dead_code)] // Reserved for async operations
    // Planned for future use
    Loading { message: String },
}

impl App {
    pub fn new(config: Config, piped_input: Option<String>) -> Result<Self> {
        let theme = Theme::from_name(config.theme);

        // If piped input is provided, auto-analyze in Error Translator (skip welcome)
        let show_welcome = !config.welcome_shown && piped_input.is_none();
        let initial_lang = config.language;

        // Detect terminal image protocol + prepare image cache
        // Create image cache for terminal image display (welcome screen + help page)
        let image_protocol = ImageProtocol::detect();
        let image_cache = if image_protocol.is_supported() {
            ImageCache::new()
        } else {
            None
        };

        let mut generations = GenerationsState::new(false);
        let mut services = ServicesState::new();
        let mut storage = StorageState::new();

        let (errors, active_tab, intros_dismissed) = if let Some(input) = piped_input {
            let errors = ErrorsState::new_with_input(input, config.language);
            let mut dismissed = HashSet::new();
            dismissed.insert(ModuleTab::Errors.index()); // Skip intro for piped input
            (errors, ModuleTab::Errors, dismissed)
        } else {
            (ErrorsState::new(), ModuleTab::Generations, HashSet::new())
        };

        // Sync language to all modules
        let lang = config.language;
        generations.lang = lang;
        services.lang = lang;
        storage.lang = lang;
        let mut config_showcase = ConfigShowcaseState::new();
        config_showcase.lang = lang;
        let mut options = OptionsState::new();
        options.lang = lang;
        let mut packages = PackagesState::new();
        packages.lang = lang;
        let mut health = HealthState::new();
        health.lang = lang;
        let mut rebuild = RebuildState::new();
        rebuild.lang = lang;
        let mut flake_inputs = FlakeInputsState::new();
        flake_inputs.lang = lang;

        Ok(Self {
            should_quit: false,
            active_tab,
            config,
            theme,
            settings_selected: 0,
            settings_editing: false,
            settings_edit_buffer: String::new(),
            popup: PopupState::None,
            flash_message: None,
            intros_dismissed,
            image_protocol,
            image_cache,
            image_area: None,
            image_displayed: false,
            last_image_area: None,
            welcome: WelcomeState::new(show_welcome, initial_lang),
            generations,
            errors,
            services,
            storage,
            config_showcase,
            options,
            packages,
            health,
            rebuild,
            flake_inputs,
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Welcome screen
        if self.welcome.active {
            if !self.welcome.ready_for_input() {
                return Ok(());
            }
            match key.code {
                KeyCode::Char('q') => {
                    self.config.welcome_shown = true;
                    self.config.language = self.welcome.selected_lang;
                    let _ = self.config.save();
                    self.should_quit = true;
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                    // Toggle language
                    self.welcome.toggle_language();
                }
                KeyCode::Enter => {
                    // Confirm: save language + dismiss
                    self.config.language = self.welcome.selected_lang;
                    self.sync_lang_to_modules();
                    self.config.welcome_shown = true;
                    let _ = self.config.save();
                    self.welcome.dismiss();
                    // Clear the welcome screen image (will be re-displayed on Help page)
                    let _ = splash::clear_image(self.image_protocol);
                    self.image_displayed = false;
                }
                _ => {} // Ignore other keys (don't accidentally dismiss)
            }
            return Ok(());
        }

        // Clear expired flash
        if let Some(msg) = &self.flash_message {
            if msg.is_expired(3) {
                self.flash_message = None;
            }
        }

        // App-level popup handling
        match &self.popup {
            PopupState::Error { .. } => {
                match key.code {
                    KeyCode::Char('o') | KeyCode::Enter | KeyCode::Esc => {
                        self.popup = PopupState::None;
                    }
                    _ => {}
                }
                return Ok(());
            }
            PopupState::Loading { .. } => return Ok(()),
            PopupState::None => {}
        }

        // Settings text editing mode captures ALL keys
        if self.settings_editing {
            self.handle_settings_edit_key(key)?;
            return Ok(());
        }

        // Module intro page handling
        if self.is_intro_showing() {
            match key.code {
                KeyCode::Enter | KeyCode::F(1..=4) => {
                    self.intros_dismissed.insert(self.active_tab.index());
                    return Ok(());
                }
                // Global nav keys fall through
                KeyCode::Char('1'..='9') | KeyCode::Char('0')
                | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => {}
                // All other keys are absorbed by intro
                _ => return Ok(()),
            }
        }

        // Try to let active module consume the key
        let consumed = self.try_module_key(key)?;
        if consumed {
            return Ok(());
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return Ok(());
            }
            KeyCode::Char('1') => self.active_tab = ModuleTab::Generations,
            KeyCode::Char('2') => self.active_tab = ModuleTab::Errors,
            KeyCode::Char('3') => self.active_tab = ModuleTab::Services,
            KeyCode::Char('4') => self.active_tab = ModuleTab::Storage,
            KeyCode::Char('5') => self.active_tab = ModuleTab::Config,
            KeyCode::Char('6') => self.active_tab = ModuleTab::Options,
            KeyCode::Char('7') => self.active_tab = ModuleTab::Rebuild,
            KeyCode::Char('8') => self.active_tab = ModuleTab::FlakeInputs,
            KeyCode::Char('9') => self.active_tab = ModuleTab::Packages,
            KeyCode::Char('0') => self.active_tab = ModuleTab::Health,
            KeyCode::Char(',') => self.active_tab = ModuleTab::Settings,
            KeyCode::Char('?') => self.active_tab = ModuleTab::HelpAbout,
            _ => {}
        }

        if self.active_tab == ModuleTab::Settings {
            self.handle_settings_key(key)?;
        }

        // Lazy-load installed packages when entering Packages tab
        if self.active_tab == ModuleTab::Packages {
            self.packages.ensure_source_detected(&self.config.nixpkgs_channel);
            self.packages.ensure_installed_loaded();
        }

        // Lazy-load options when entering Options tab
        if self.active_tab == ModuleTab::Options {
            self.options.ensure_loaded();
        }

        // Lazy-load flake inputs when entering FlakeInputs tab
        if self.active_tab == ModuleTab::FlakeInputs {
            self.flake_inputs.ensure_loaded();
        }

        // Lazy-load health checks when entering Health tab
        if self.active_tab == ModuleTab::Health {
            self.health.ensure_scanned();
        }

        // Lazy-load config detection when entering Rebuild tab
        if self.active_tab == ModuleTab::Rebuild {
            self.rebuild.ensure_detected();
        }

        Ok(())
    }

    fn try_module_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.active_tab {
            ModuleTab::Generations => {
                let gen = &self.generations;

                // Module captures ALL keys when popup or filter active
                let has_popup = !matches!(gen.popup, crate::modules::generations::GenPopupState::None);
                let filter_active = gen.packages_filter_active;

                if has_popup || filter_active {
                    self.generations.handle_key(key)?;
                    return Ok(true);
                }

                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=4)) {
                    self.generations.handle_key(key)?;
                    return Ok(true);
                }

                // Tab-switch keys and quit stay global
                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.generations.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::Errors => {
                let err = &self.errors;

                // Module captures ALL keys when in input mode or AI loading
                if err.input_mode || err.ai_loading {
                    let lang = self.config.language;
                    self.errors.handle_key(key, lang)?;
                    return Ok(true);
                }

                // Submit tab always captures (it's a form)
                if err.active_sub_tab == crate::modules::errors::ErrSubTab::Submit {
                    let lang = self.config.language;
                    self.errors.handle_key(key, lang)?;
                    return Ok(true);
                }

                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=2)) {
                    let lang = self.config.language;
                    self.errors.handle_key(key, lang)?;
                    return Ok(true);
                }

                // Tab-switch keys, quit, and Tab (sidebar) stay global
                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        let lang = self.config.language;
                        self.errors.handle_key(key, lang)?;

                        // Check if AI analysis was requested
                        if self.errors.ai_requested {
                            self.errors.ai_requested = false;
                            self.handle_ai_request();
                        }

                        Ok(true)
                    }
                }
            }
            ModuleTab::Services => {
                let svc = &self.services;

                // Module captures ALL keys when search active or popup open
                let has_popup = !matches!(svc.popup, crate::modules::services::SvcPopupState::None);
                let search_active = svc.search_active;

                if has_popup || search_active {
                    self.services.handle_key(key)?;
                    return Ok(true);
                }

                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=4)) {
                    self.services.handle_key(key)?;
                    return Ok(true);
                }

                // Tab-switch keys, quit, and Tab (sidebar) stay global
                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.services.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::Storage => {
                let sto = &self.storage;

                // Module captures ALL keys when popup open or search active
                let has_popup = !matches!(sto.popup, crate::modules::storage::StoPopupState::None);
                let search_active = sto.explorer_search_active;

                if has_popup || search_active {
                    self.storage.handle_key(key)?;
                    return Ok(true);
                }

                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=4)) {
                    self.storage.handle_key(key)?;
                    return Ok(true);
                }

                // Tab-switch keys, quit, and Tab (sidebar) stay global
                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.storage.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::Config => {
                // F-keys always go to module for sub-tab switching
                if matches!(key.code, KeyCode::F(1..=2)) {
                    self.config_showcase.handle_key(key)?;
                    return Ok(true);
                }

                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.config_showcase.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::Packages => {
                let pkg = &self.packages;

                // Module captures ALL keys when search active or detail open
                if pkg.search_active || pkg.detail_open {
                    self.packages.handle_key(key)?;
                    return Ok(true);
                }

                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.packages.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::Health => {
                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=2)) {
                    self.health.handle_key(key)?;
                    return Ok(true);
                }

                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.health.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::Rebuild => {
                let rb = &self.rebuild;

                // Module captures ALL keys when popup open or search active
                let has_popup = rb.popup != crate::modules::rebuild::RebuildPopup::None;
                let search_active = rb.log_search_active;

                if has_popup || search_active {
                    self.rebuild.handle_key(key)?;
                    return Ok(true);
                }

                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=4)) {
                    self.rebuild.handle_key(key)?;
                    return Ok(true);
                }

                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.rebuild.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::Options => {
                let opt = &self.options;

                // Module captures ALL keys when search active or detail open
                if opt.search_active || opt.detail_open {
                    self.options.handle_key(key)?;
                    return Ok(true);
                }

                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=3)) {
                    self.options.handle_key(key)?;
                    return Ok(true);
                }

                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.options.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            ModuleTab::FlakeInputs => {
                let fi = &self.flake_inputs;

                // Module captures ALL keys when popup is showing
                if fi.popup != crate::modules::flake_inputs::FlakePopup::None {
                    self.flake_inputs.handle_key(key)?;
                    return Ok(true);
                }

                // F-keys always go to module
                if matches!(key.code, KeyCode::F(1..=4)) {
                    self.flake_inputs.handle_key(key)?;
                    return Ok(true);
                }

                match key.code {
                    KeyCode::Char('1'..='9') | KeyCode::Char('0') | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
                    _ => {
                        self.flake_inputs.handle_key(key)?;
                        Ok(true)
                    }
                }
            }
            _ => Ok(false),
        }
    }

    pub fn update_timers(&mut self) -> Result<()> {
        self.generations.update_undo_timer()?;

        // Poll background loaders (non-blocking)
        self.services.poll_load();
        self.storage.poll_load();
        self.errors.poll_ai();
        self.config_showcase.poll_scan();
        self.packages.poll_search();
        self.health.poll_scan();
        self.options.poll_load();
        self.flake_inputs.poll_load();
        self.rebuild.poll_detect();
        self.rebuild.poll_build();

        // Expire flash messages across all modules
        expire_flash(&mut self.generations.flash_message);
        expire_flash(&mut self.errors.flash_message);
        expire_flash(&mut self.services.flash_message);
        expire_flash(&mut self.storage.flash_message);
        expire_flash(&mut self.config_showcase.flash_message);
        expire_flash(&mut self.packages.flash_message);
        expire_flash(&mut self.health.flash_message);
        expire_flash(&mut self.options.flash_message);
        expire_flash(&mut self.flake_inputs.flash_message);
        expire_flash(&mut self.rebuild.flash_message);

        Ok(())
    }

    /// Display or clear terminal images based on current image_area.
    /// Called after each terminal.draw() in the main loop.
    pub fn handle_image(&mut self) -> Result<()> {
        if !self.image_protocol.is_supported() {
            return Ok(());
        }
        let cache = match &self.image_cache {
            Some(c) => c,
            None => return Ok(()),
        };

        match self.image_area {
            Some((col, row, cols, rows)) => {
                // Kitty: only re-send if position changed (image persists on its layer)
                // iTerm2: re-send every frame (ratatui overwrites cell content)
                let need_send = match self.image_protocol {
                    ImageProtocol::Kitty => {
                        self.last_image_area != Some((col, row, cols, rows))
                    }
                    ImageProtocol::ITerm2 => true,
                    ImageProtocol::None => false,
                };

                if need_send {
                    let _ = splash::display_image(
                        self.image_protocol,
                        cache,
                        col, row, cols, rows,
                    );
                    self.last_image_area = Some((col, row, cols, rows));
                    self.image_displayed = true;
                }
            }
            None => {
                if self.image_displayed {
                    let _ = splash::clear_image(self.image_protocol);
                    self.image_displayed = false;
                    self.last_image_area = None;
                }
            }
        }
        Ok(())
    }

    /// Clean up images before exiting (prevents ghost images in terminal)
    /// Called BEFORE LeaveAlternateScreen so the terminal can process the
    /// delete commands while still in the alternate screen buffer.
    pub fn cleanup_images(&mut self) {
        // Send delete command
        let _ = splash::clear_image(self.image_protocol);
        // Mark as not displayed so no further display attempts
        self.image_displayed = false;
        self.image_area = None;
        self.last_image_area = None;
    }

    /// Check if the module intro page is showing for the current tab
    pub fn is_intro_showing(&self) -> bool {
        matches!(
            self.active_tab,
            ModuleTab::Generations
                | ModuleTab::Errors
                | ModuleTab::Services
                | ModuleTab::Storage
                | ModuleTab::Config
                | ModuleTab::Options
                | ModuleTab::Rebuild
                | ModuleTab::FlakeInputs
                | ModuleTab::Packages
                | ModuleTab::Health
        ) && !self.intros_dismissed.contains(&self.active_tab.index())
    }

    fn handle_settings_key(&mut self, key: KeyEvent) -> Result<()> {
        let settings_count = 10; // 3 global + 1 pkg search + 6 error translator/AI
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.settings_selected < settings_count - 1 {
                    self.settings_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.settings_selected = self.settings_selected.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                match self.settings_selected {
                    0 => {
                        self.config.theme = self.config.theme.next();
                        self.theme = Theme::from_name(self.config.theme);
                    }
                    1 => {
                        self.config.language = self.config.language.next();
                        self.sync_lang_to_modules();
                    }
                    2 => {
                        self.config.layout = self.config.layout.next();
                    }
                    // Nixpkgs channel (text editable)
                    3 => {
                        self.settings_editing = true;
                        self.settings_edit_buffer = self.config.nixpkgs_channel.clone();
                        return Ok(());
                    }
                    // Error Translator / AI settings
                    4 => {
                        self.config.ai_enabled = !self.config.ai_enabled;
                    }
                    5 => {
                        self.config.ai_provider = match self.config.ai_provider.as_str() {
                            "claude" => "openai".to_string(),
                            "openai" => "ollama".to_string(),
                            _ => "claude".to_string(),
                        };
                    }
                    // Text-editable fields: enter edit mode
                    6 => {
                        // AI API Key
                        self.settings_editing = true;
                        self.settings_edit_buffer = String::new(); // Start fresh (don't reveal old key)
                        return Ok(());
                    }
                    7 => {
                        // Ollama URL
                        self.settings_editing = true;
                        self.settings_edit_buffer = self.config.ollama_url
                            .clone().unwrap_or_else(|| "http://localhost:11434".to_string());
                        return Ok(());
                    }
                    8 => {
                        // Ollama Model
                        self.settings_editing = true;
                        self.settings_edit_buffer = self.config.ollama_model
                            .clone().unwrap_or_else(|| "llama3".to_string());
                        return Ok(());
                    }
                    9 => {
                        // GitHub Token
                        self.settings_editing = true;
                        self.settings_edit_buffer = String::new();
                        return Ok(());
                    }
                    _ => {}
                }
                let s = i18n::get_strings(self.config.language);
                if let Err(e) = self.config.save() {
                    self.popup = PopupState::Error {
                        title: crate::i18n::get_strings(self.config.language).save_failed.into(),
                        message: e.to_string(),
                    };
                } else {
                    self.flash_message = Some(FlashMessage::new(s.settings_saved.into(), false));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events while editing a settings text field.
    fn handle_settings_edit_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // Cancel editing
                self.settings_editing = false;
                self.settings_edit_buffer.clear();
            }
            KeyCode::Enter => {
                // Save the value
                let value = self.settings_edit_buffer.clone();
                match self.settings_selected {
                    3 => {
                        self.config.nixpkgs_channel = if value.is_empty() {
                            "auto".to_string()
                        } else {
                            value
                        };
                        // Reset source detection so it picks up the new channel
                        self.packages.reset_source();
                    }
                    6 => {
                        self.config.ai_api_key = if value.is_empty() {
                            None
                        } else {
                            Some(value)
                        };
                    }
                    7 => {
                        self.config.ollama_url = if value.is_empty() {
                            None
                        } else {
                            Some(value)
                        };
                    }
                    8 => {
                        self.config.ollama_model = if value.is_empty() {
                            None
                        } else {
                            Some(value)
                        };
                    }
                    9 => {
                        self.config.github_token = if value.is_empty() {
                            None
                        } else {
                            Some(value)
                        };
                    }
                    _ => {}
                }
                self.settings_editing = false;
                self.settings_edit_buffer.clear();

                let s = i18n::get_strings(self.config.language);
                if let Err(e) = self.config.save() {
                    self.popup = PopupState::Error {
                        title: crate::i18n::get_strings(self.config.language).save_failed.into(),
                        message: e.to_string(),
                    };
                } else {
                    self.flash_message = Some(FlashMessage::new(s.settings_saved.into(), false));
                }
            }
            KeyCode::Backspace => {
                self.settings_edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.settings_edit_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle AI analysis request from the Error Translator.
    fn handle_ai_request(&mut self) {
        let s = i18n::get_strings(self.config.language);

        if !self.config.ai_enabled {
            self.errors.show_flash(s.err_ai_disabled, true);
            return;
        }

        if self.config.ai_provider != "ollama"
            && self.config.ai_api_key.as_ref().map_or(true, |k| k.is_empty())
        {
            self.errors.show_flash(s.err_ai_no_key, true);
            return;
        }

        let lang_str = match self.config.language {
            crate::config::Language::English => "en",
            crate::config::Language::German => "de",
        };

        self.errors.start_ai_analysis(
            &self.config.ai_provider,
            self.config.ai_api_key.as_deref().unwrap_or(""),
            self.config.ollama_url.as_deref().unwrap_or("http://localhost:11434"),
            self.config.ollama_model.as_deref().unwrap_or("llama3"),
            lang_str,
        );
    }
}

/// Expire a flash message after 3 seconds
fn expire_flash(msg: &mut Option<FlashMessage>) {
    if let Some(m) = msg {
        if m.is_expired(3) {
            *msg = None;
        }
    }
}

impl App {
    /// Sync the current language setting to all module states
    fn sync_lang_to_modules(&mut self) {
        let lang = self.config.language;
        self.generations.lang = lang;
        self.errors.lang = lang;
        self.services.lang = lang;
        self.storage.lang = lang;
        self.config_showcase.lang = lang;
        self.packages.lang = lang;
        self.health.lang = lang;
        self.options.lang = lang;
        self.flake_inputs.lang = lang;
        self.rebuild.lang = lang;
    }
}
