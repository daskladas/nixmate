//! Rebuild Dashboard — Live `nixos-rebuild` monitoring
//!
//! Sub-tabs: Dashboard, Log, Changes, History
//! Tracks build phases, derivation counts, warnings, errors.
//! Post-rebuild diff: packages added/removed/updated, services restarted.
//! Supports Flakes, Channels, and Home-Manager configurations.

use crate::config::Language;
use crate::i18n;
use crate::nix::detect::{detect_flakes, find_flake_path};
use crate::types::FlashMessage;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs},
    Frame,
};
use std::sync::mpsc;
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use std::time::{Duration, Instant};

// ── Sub-tabs ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RebuildSubTab {
    #[default]
    Dashboard,
    Log,
    Changes,
    History,
}

impl RebuildSubTab {
    pub fn all() -> &'static [RebuildSubTab] {
        &[
            RebuildSubTab::Dashboard,
            RebuildSubTab::Log,
            RebuildSubTab::Changes,
            RebuildSubTab::History,
        ]
    }

    pub fn index(&self) -> usize {
        match self {
            RebuildSubTab::Dashboard => 0,
            RebuildSubTab::Log => 1,
            RebuildSubTab::Changes => 2,
            RebuildSubTab::History => 3,
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            RebuildSubTab::Dashboard => s.rb_dashboard,
            RebuildSubTab::Log => s.rb_log,
            RebuildSubTab::Changes => s.rb_changes,
            RebuildSubTab::History => s.rb_history,
        }
    }
}

// ── Rebuild mode ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RebuildMode {
    #[default]
    Switch,
    Boot,
    Test,
    Build,
    DryBuild,
}

impl RebuildMode {
    pub fn as_arg(&self) -> &'static str {
        match self {
            RebuildMode::Switch => "switch",
            RebuildMode::Boot => "boot",
            RebuildMode::Test => "test",
            RebuildMode::Build => "build",
            RebuildMode::DryBuild => "dry-build",
        }
    }

    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            RebuildMode::Switch => s.rb_mode_switch,
            RebuildMode::Boot => s.rb_mode_boot,
            RebuildMode::Test => s.rb_mode_test,
            RebuildMode::Build => s.rb_mode_build,
            RebuildMode::DryBuild => s.rb_mode_dry,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            RebuildMode::Switch => RebuildMode::Boot,
            RebuildMode::Boot => RebuildMode::Test,
            RebuildMode::Test => RebuildMode::Build,
            RebuildMode::Build => RebuildMode::DryBuild,
            RebuildMode::DryBuild => RebuildMode::Switch,
        }
    }
}

// ── Build phase ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildPhase {
    #[default]
    Idle,
    Preparing,
    Evaluating,
    Building,
    Fetching,
    Activating,
    Bootloader,
    Done,
    Failed,
}

impl BuildPhase {
    pub fn label(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            BuildPhase::Idle => s.rb_phase_idle,
            BuildPhase::Preparing => s.rb_phase_preparing,
            BuildPhase::Evaluating => s.rb_phase_evaluating,
            BuildPhase::Building => s.rb_phase_building,
            BuildPhase::Fetching => s.rb_phase_fetching,
            BuildPhase::Activating => s.rb_phase_activating,
            BuildPhase::Bootloader => s.rb_phase_bootloader,
            BuildPhase::Done => s.rb_phase_done,
            BuildPhase::Failed => s.rb_phase_failed,
        }
    }

    /// Index in the 5-phase dashboard (0-based). Returns None for non-pipeline phases.
    pub fn pipeline_index(&self) -> Option<usize> {
        match self {
            BuildPhase::Evaluating | BuildPhase::Preparing => Some(0),
            BuildPhase::Fetching => Some(1),
            BuildPhase::Building => Some(2),
            BuildPhase::Activating => Some(3),
            BuildPhase::Bootloader => Some(4),
            _ => None,
        }
    }

    /// The 5 pipeline phases for the dashboard boxes.
    pub fn pipeline_phases() -> [BuildPhase; 5] {
        [
            BuildPhase::Evaluating,
            BuildPhase::Fetching,
            BuildPhase::Building,
            BuildPhase::Activating,
            BuildPhase::Bootloader,
        ]
    }

    /// Longer educational explanation of what's happening in this phase.
    pub fn explanation(&self, lang: Language) -> &'static str {
        let s = i18n::get_strings(lang);
        match self {
            BuildPhase::Evaluating | BuildPhase::Preparing => s.rb_explain_eval,
            BuildPhase::Fetching => s.rb_explain_fetch,
            BuildPhase::Building => s.rb_explain_build,
            BuildPhase::Activating => s.rb_explain_activate,
            BuildPhase::Bootloader => s.rb_explain_bootloader,
            BuildPhase::Done => s.rb_explain_done,
            BuildPhase::Failed => s.rb_explain_failed,
            BuildPhase::Idle => "",
        }
    }
}

// ── Log line classification ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Normal,
    Info,
    Warning,
    Error,
    Phase,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub text: String,     // beautified display text
    pub raw: String,      // original unmodified output
    pub level: LogLevel,
}

// ── Diff types ──

#[derive(Debug, Clone)]
pub struct RebuildDiff {
    pub added: Vec<(String, String)>,     // (name, version)
    pub removed: Vec<(String, String)>,   // (name, version)
    pub updated: Vec<(String, String, String)>, // (name, old_ver, new_ver)
    pub kernel_changed: Option<(String, String)>, // (old, new)
    pub reboot_needed: bool,
    pub services_restarted: Vec<String>,
    pub nixos_version: Option<(String, String)>, // (old, new)
}

impl Default for RebuildDiff {
    fn default() -> Self {
        Self {
            added: Vec::new(),
            removed: Vec::new(),
            updated: Vec::new(),
            kernel_changed: None,
            reboot_needed: false,
            services_restarted: Vec::new(),
            nixos_version: None,
        }
    }
}

// ── History entry ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    #[serde(with = "rebuild_mode_serde")]
    pub mode: RebuildMode,
    #[serde(with = "duration_serde")]
    pub duration: Duration,
    pub success: bool,
    pub error_preview: Option<String>,
    pub command: String,
}

mod rebuild_mode_serde {
    use super::RebuildMode;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(mode: &RebuildMode, s: S) -> Result<S::Ok, S::Error> {
        mode.as_arg().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<RebuildMode, D::Error> {
        let val = String::deserialize(d)?;
        Ok(match val.as_str() {
            "boot" => RebuildMode::Boot,
            "test" => RebuildMode::Test,
            "build" => RebuildMode::Build,
            "dry-build" => RebuildMode::DryBuild,
            _ => RebuildMode::Switch,
        })
    }
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.as_secs().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

// ── Messages from background thread ──

#[derive(Debug)]
pub enum RebuildMsg {
    OutputLine(String),
    Phase(BuildPhase),
    Stats(BuildStats),
    PreSnapshot(Vec<(String, String)>, Option<String>, Option<String>), // packages, kernel, nixos_ver
    PostSnapshot(Vec<(String, String)>, Option<String>, Option<String>),
    ServiceRestart(String),
    Finished(bool, Option<String>), // (success, error_message)
    CommandInfo(String),
}

#[derive(Debug, Clone, Default)]
pub struct BuildStats {
    pub derivations_built: u32,
    pub derivations_total: Option<u32>,
    pub fetched: u32,
    pub warnings: u32,
    pub errors: u32,
}

// ── Popup state ──

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebuildPopup {
    None,
    ConfirmRebuild,
}

// ── Module state ──

pub struct RebuildState {
    pub sub_tab: RebuildSubTab,
    pub mode: RebuildMode,
    pub phase: BuildPhase,
    pub popup: RebuildPopup,

    // Build tracking
    pub stats: BuildStats,
    pub start_time: Option<Instant>,
    pub log_lines: Vec<LogLine>,
    pub log_scroll: usize,
    pub log_auto_scroll: bool,
    pub log_search_active: bool,
    pub log_search_query: String,

    // Current build line (shown in dashboard)
    pub current_activity: String,

    // Last phase that had an explanation (for "linger" display on fast phases)
    pub last_explanation_phase: BuildPhase,

    // Phase timing: (start_time, end_time) per pipeline phase index
    pub phase_times: [Option<(Instant, Option<Instant>)>; 5],
    pub phase_skipped: [bool; 5],
    pub failed_phase_idx: Option<usize>, // which pipeline phase the build failed in

    // Pre/post snapshot for diff
    pre_packages: Vec<(String, String)>,
    pre_kernel: Option<String>,
    pre_nixos_ver: Option<String>,

    // Diff result
    pub diff: Option<RebuildDiff>,
    pub changes_scroll: usize,

    // History
    pub history: Vec<HistoryEntry>,
    pub history_selected: usize,

    // Config detection
    pub detected_command: Option<String>,
    pub uses_flakes: Option<bool>,
    pub flake_path: Option<String>,
    pub detected: bool,
    pub detecting: bool,

    // Flash message
    pub lang: Language,
    pub flash_message: Option<FlashMessage>,

    // Password for sudo
    pub password_buffer: String,

    // Show --show-trace flag
    pub show_trace: bool,

    // Child process PID for cancellation
    child_pid: Arc<AtomicU32>,

    // mpsc channels
    build_rx: Option<mpsc::Receiver<RebuildMsg>>,
    _detect_rx: Option<mpsc::Receiver<(bool, Option<String>)>>,
}

impl RebuildState {
    pub fn new() -> Self {
        let history = load_history().unwrap_or_default();
        Self {
            sub_tab: RebuildSubTab::Dashboard,
            mode: RebuildMode::Switch,
            phase: BuildPhase::Idle,
            popup: RebuildPopup::None,
            stats: BuildStats::default(),
            start_time: None,
            log_lines: Vec::new(),
            log_scroll: 0,
            log_auto_scroll: true,
            log_search_active: false,
            log_search_query: String::new(),
            current_activity: String::new(),
            last_explanation_phase: BuildPhase::Idle,
            phase_times: [None; 5],
            phase_skipped: [false; 5],
            failed_phase_idx: None,
            pre_packages: Vec::new(),
            pre_kernel: None,
            pre_nixos_ver: None,
            diff: None,
            changes_scroll: 0,
            history,
            history_selected: 0,
            detected_command: None,
            uses_flakes: None,
            flake_path: None,
            detected: false,
            detecting: false,
            lang: Language::English,
            flash_message: None,
            password_buffer: String::new(),
            show_trace: false,
            child_pid: Arc::new(AtomicU32::new(0)),
            build_rx: None,
            _detect_rx: None,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.phase,
            BuildPhase::Preparing
                | BuildPhase::Evaluating
                | BuildPhase::Building
                | BuildPhase::Fetching
                | BuildPhase::Activating
                | BuildPhase::Bootloader
        )
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    pub fn elapsed_str(&self) -> String {
        let secs = self.elapsed().as_secs();
        let m = secs / 60;
        let s = secs % 60;
        format!("{:02}:{:02}", m, s)
    }

    /// Estimated build time based on the average of the last 5 successful builds.
    pub fn estimated_time(&self) -> Option<Duration> {
        let successes: Vec<&HistoryEntry> = self
            .history
            .iter()
            .rev()
            .filter(|h| h.success)
            .take(5)
            .collect();
        if successes.is_empty() {
            return None;
        }
        let total: u64 = successes.iter().map(|h| h.duration.as_secs()).sum();
        Some(Duration::from_secs(total / successes.len() as u64))
    }

    /// Get elapsed time string for a pipeline phase index (0-4).
    pub fn phase_elapsed_str(&self, idx: usize) -> String {
        match self.phase_times.get(idx).copied().flatten() {
            Some((start, Some(end))) => {
                let secs = end.duration_since(start).as_secs();
                format!("{}s", secs)
            }
            Some((start, None)) => {
                let secs = start.elapsed().as_secs();
                format!("{}s", secs)
            }
            None => String::new(),
        }
    }

    /// Get the rebuild command for the current mode (dynamically computed)
    pub fn current_command(&self) -> String {
        let uses_flakes = self.uses_flakes.unwrap_or(false);
        let (program, args) = build_rebuild_command(
            self.mode.as_arg(),
            uses_flakes,
            self.flake_path.as_deref(),
        );
        let mut cmd = format!("{} {}", program, args.join(" "));
        if self.show_trace {
            cmd.push_str(" --show-trace");
        }
        cmd
    }

    /// Cancel a running build by killing the child process.
    pub fn cancel_build(&mut self) {
        let pid = self.child_pid.load(Ordering::SeqCst);
        if pid != 0 && self.is_running() {
            // Track which phase was cancelled
            self.failed_phase_idx = self.phase.pipeline_index();
            // Close timing for the current phase
            if let Some(idx) = self.phase.pipeline_index() {
                if let Some(ref mut entry) = self.phase_times[idx] {
                    if entry.1.is_none() {
                        entry.1 = Some(Instant::now());
                    }
                }
            }
            // Send SIGTERM to the process group
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
            self.phase = BuildPhase::Failed;
            let s = crate::i18n::get_strings(self.lang);
            self.log_lines.push(LogLine {
                text: format!("⏹ {}", s.rb_build_cancelled),
                raw: s.rb_build_cancelled.to_string(),
                level: LogLevel::Warning,
            });
            self.child_pid.store(0, Ordering::SeqCst);
            // Mark unvisited phases as skipped
            for i in 0..5 {
                if self.phase_times[i].is_none() {
                    self.phase_skipped[i] = true;
                }
            }
        }
    }

    /// Detect system config (flakes vs channels, flake path)
    pub fn ensure_detected(&mut self) {
        if self.detected || self.detecting {
            return;
        }
        self.detecting = true;

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let uses_flakes = detect_flakes();
            let flake_path = if uses_flakes {
                find_flake_path()
            } else {
                None
            };
            let _ = tx.send((uses_flakes, flake_path));
        });

        self._detect_rx = Some(rx);
    }

    /// Poll detection result
    pub fn poll_detect(&mut self) {
        if let Some(rx) = &self._detect_rx {
            if let Ok((uses_flakes, flake_path)) = rx.try_recv() {
                self.uses_flakes = Some(uses_flakes);
                self.flake_path = flake_path;
                self.detected = true;
                self.detecting = false;
                self._detect_rx = None;
            }
        }
    }

    /// Start rebuild in background
    pub fn start_rebuild(&mut self, password: Option<String>) {
        if self.is_running() {
            return;
        }

        let uses_flakes = self.uses_flakes.unwrap_or(false);
        let flake_path = self.flake_path.clone();
        let mode = self.mode;

        // Reset state
        self.phase = BuildPhase::Preparing;
        self.stats = BuildStats::default();
        self.start_time = Some(Instant::now());
        self.log_lines.clear();
        self.log_scroll = 0;
        self.log_auto_scroll = true;
        self.log_search_active = false;
        self.log_search_query.clear();
        self.current_activity.clear();
        self.last_explanation_phase = BuildPhase::Idle;
        self.diff = None;
        self.changes_scroll = 0;
        self.phase_times = [None; 5];
        self.phase_skipped = [false; 5];
        self.failed_phase_idx = None;
        self.sub_tab = RebuildSubTab::Dashboard;

        let (tx, rx) = mpsc::channel();
        self.build_rx = Some(rx);
        self.child_pid.store(0, Ordering::SeqCst);

        let (prog, args) = build_rebuild_command(mode.as_arg(), uses_flakes, flake_path.as_deref());
        let mut command = format!("{} {}", prog, args.join(" "));
        let show_trace = self.show_trace;
        if show_trace {
            command.push_str(" --show-trace");
        }
        self.detected_command = Some(command.clone());
        let _ = tx.send(RebuildMsg::CommandInfo(command));

        let auth_msg = crate::i18n::get_strings(self.lang).rb_authenticating.to_string();
        let pid_ref = Arc::clone(&self.child_pid);
        std::thread::spawn(move || {
            run_rebuild(tx, mode, uses_flakes, flake_path.as_deref(), password, show_trace, pid_ref, auth_msg);
        });
    }

    /// Poll build progress messages
    pub fn poll_build(&mut self) {
        let rx = match &self.build_rx {
            Some(rx) => rx,
            None => return,
        };

        // Drain all available messages (non-blocking)
        let mut finished = false;
        for _ in 0..100 {
            match rx.try_recv() {
                Ok(msg) => match msg {
                    RebuildMsg::OutputLine(line) => {
                        let level = classify_line(&line);
                        let display_text = beautify_store_path(&line);
                        self.current_activity = display_text.clone();
                        self.log_lines.push(LogLine { text: display_text, raw: line, level });
                        // Cap log lines to prevent unbounded memory growth
                        if self.log_lines.len() > 50_000 {
                            self.log_lines.drain(..10_000);
                            if self.log_scroll > 10_000 {
                                self.log_scroll -= 10_000;
                            } else {
                                self.log_scroll = 0;
                            }
                        }
                    }
                    RebuildMsg::Phase(phase) => {
                        // Close timing for old phase
                        if let Some(old_idx) = self.phase.pipeline_index() {
                            if let Some(ref mut entry) = self.phase_times[old_idx] {
                                if entry.1.is_none() {
                                    entry.1 = Some(Instant::now());
                                }
                            }
                        }
                        // Track last phase for lingering explanation display
                        if self.phase != BuildPhase::Idle && self.phase != BuildPhase::Preparing {
                            self.last_explanation_phase = self.phase;
                        }
                        self.phase = phase;
                        // Start timing for new phase
                        if let Some(new_idx) = phase.pipeline_index() {
                            if self.phase_times[new_idx].is_none() {
                                self.phase_times[new_idx] = Some((Instant::now(), None));
                            }
                        }
                        let level = LogLevel::Phase;
                        let text = format!("── {} ──", phase_label(phase, self.lang));
                        self.log_lines.push(LogLine { text: text.clone(), raw: text, level });
                    }
                    RebuildMsg::Stats(stats) => {
                        self.stats = stats;
                    }
                    RebuildMsg::PreSnapshot(pkgs, kernel, ver) => {
                        self.pre_packages = pkgs;
                        self.pre_kernel = kernel;
                        self.pre_nixos_ver = ver;
                    }
                    RebuildMsg::PostSnapshot(pkgs, kernel, ver) => {
                        // Calculate diff
                        let diff = calculate_diff(
                            &self.pre_packages,
                            &pkgs,
                            &self.pre_kernel,
                            &kernel,
                            &self.pre_nixos_ver,
                            &ver,
                        );
                        self.diff = Some(diff);
                    }
                    RebuildMsg::ServiceRestart(svc) => {
                        if let Some(ref mut diff) = self.diff {
                            diff.services_restarted.push(svc);
                        }
                    }
                    RebuildMsg::CommandInfo(cmd) => {
                        self.detected_command = Some(cmd.clone());
                        let level = LogLevel::Info;
                        let text = format!("$ {}", cmd);
                        self.log_lines.push(LogLine {
                            text: text.clone(),
                            raw: text,
                            level,
                        });
                    }
                    RebuildMsg::Finished(success, err_msg) => {
                        // Close timing for the final active phase
                        if let Some(old_idx) = self.phase.pipeline_index() {
                            if let Some(ref mut entry) = self.phase_times[old_idx] {
                                if entry.1.is_none() {
                                    entry.1 = Some(Instant::now());
                                }
                            }
                            // Track which phase failed
                            if !success {
                                self.failed_phase_idx = Some(old_idx);
                            }
                        }

                        self.phase = if success {
                            BuildPhase::Done
                        } else {
                            BuildPhase::Failed
                        };

                        // Mark phases that were never entered as skipped
                        for i in 0..5 {
                            if self.phase_times[i].is_none() {
                                self.phase_skipped[i] = true;
                            }
                        }

                        // Record in history
                        let duration = self.elapsed();
                        let error_preview = if !success {
                            err_msg.clone().or_else(|| {
                                self.log_lines
                                    .iter()
                                    .rev()
                                    .find(|l| l.level == LogLevel::Error)
                                    .map(|l| {
                                        if l.raw.chars().count() > 80 {
                                            let truncated: String = l.raw.chars().take(80).collect();
                                            format!("{}...", truncated)
                                        } else {
                                            l.raw.clone()
                                        }
                                    })
                            })
                        } else {
                            None
                        };

                        let entry = HistoryEntry {
                            timestamp: chrono::Local::now()
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string(),
                            mode: self.mode,
                            duration,
                            success,
                            error_preview,
                            command: self.detected_command.clone().unwrap_or_default(),
                        };
                        self.history.push(entry);

                        // Persist to disk
                        let _ = save_history(&self.history);

                        // Terminal bell to notify user
                        print!("\x07");
                        let _ = std::io::Write::flush(&mut std::io::stdout());

                        finished = true;
                    }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Background thread terminated unexpectedly
                    if self.is_running() {
                        self.failed_phase_idx = self.phase.pipeline_index();
                        if let Some(idx) = self.phase.pipeline_index() {
                            if let Some(ref mut entry) = self.phase_times[idx] {
                                if entry.1.is_none() {
                                    entry.1 = Some(Instant::now());
                                }
                            }
                        }
                        self.phase = BuildPhase::Failed;
                        for i in 0..5 {
                            if self.phase_times[i].is_none() {
                                self.phase_skipped[i] = true;
                            }
                        }
                        self.log_lines.push(LogLine {
                            text: format!("✗ {}", crate::i18n::get_strings(self.lang).rb_terminated),
                            raw: crate::i18n::get_strings(self.lang).rb_terminated.to_string(),
                            level: LogLevel::Error,
                        });
                    }
                    finished = true;
                    break;
                }
            }
        }

        if finished {
            self.build_rx = None;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        // Popup handling — password input
        if self.popup == RebuildPopup::ConfirmRebuild {
            match key.code {
                KeyCode::Enter => {
                    let password = if self.password_buffer.is_empty() {
                        None // NOPASSWD users
                    } else {
                        Some(self.password_buffer.clone())
                    };
                    self.password_buffer.clear();
                    self.popup = RebuildPopup::None;
                    self.start_rebuild(password);
                    return Ok(true);
                }
                KeyCode::Esc => {
                    self.password_buffer.clear();
                    self.popup = RebuildPopup::None;
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    self.password_buffer.pop();
                    return Ok(true);
                }
                KeyCode::Char(c) => {
                    self.password_buffer.push(c);
                    return Ok(true);
                }
                _ => return Ok(true),
            }
        }

        // Log search mode
        if self.log_search_active {
            match key.code {
                KeyCode::Esc => {
                    self.log_search_active = false;
                    self.log_search_query.clear();
                    return Ok(true);
                }
                KeyCode::Enter => {
                    self.log_search_active = false;
                    // Keep query for highlighting
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    self.log_search_query.pop();
                    return Ok(true);
                }
                KeyCode::Char(c) => {
                    self.log_search_query.push(c);
                    return Ok(true);
                }
                _ => return Ok(true),
            }
        }

        // Sub-tab switching
        match key.code {
            KeyCode::F(1) => {
                self.sub_tab = RebuildSubTab::Dashboard;
                return Ok(true);
            }
            KeyCode::F(2) => {
                self.sub_tab = RebuildSubTab::Log;
                return Ok(true);
            }
            KeyCode::F(3) => {
                self.sub_tab = RebuildSubTab::Changes;
                return Ok(true);
            }
            KeyCode::F(4) => {
                self.sub_tab = RebuildSubTab::History;
                return Ok(true);
            }
            // Cancel running build from any tab
            KeyCode::Char('c') if self.is_running() => {
                self.cancel_build();
                return Ok(true);
            }
            _ => {}
        }

        match self.sub_tab {
            RebuildSubTab::Dashboard => self.handle_dashboard_key(key),
            RebuildSubTab::Log => self.handle_log_key(key),
            RebuildSubTab::Changes => self.handle_changes_key(key),
            RebuildSubTab::History => self.handle_history_key(key),
        }
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            // Dismiss build results and return to idle
            KeyCode::Esc => {
                if matches!(self.phase, BuildPhase::Done | BuildPhase::Failed) {
                    self.phase = BuildPhase::Idle;
                }
                Ok(true)
            }
            KeyCode::Enter | KeyCode::Char('r') => {
                if !self.is_running() {
                    self.popup = RebuildPopup::ConfirmRebuild;
                }
                Ok(true)
            }
            KeyCode::Char('m') => {
                if !self.is_running() {
                    self.mode = self.mode.next();
                }
                Ok(true)
            }
            KeyCode::Char('t') => {
                if !self.is_running() {
                    self.show_trace = !self.show_trace;
                }
                Ok(true)
            }
            KeyCode::Char('j') | KeyCode::Down => {
                // Scroll live output
                if !self.log_lines.is_empty() {
                    self.log_auto_scroll = false;
                    self.log_scroll = (self.log_scroll + 1).min(self.log_lines.len().saturating_sub(1));
                }
                Ok(true)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.log_auto_scroll = false;
                self.log_scroll = self.log_scroll.saturating_sub(1);
                Ok(true)
            }
            KeyCode::Char('G') => {
                self.log_auto_scroll = true;
                self.log_scroll = self.log_lines.len().saturating_sub(1);
                Ok(true)
            }
            KeyCode::Char('g') => {
                self.log_auto_scroll = false;
                self.log_scroll = 0;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn handle_log_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.log_auto_scroll = false;
                self.log_scroll = (self.log_scroll + 1).min(self.log_lines.len().saturating_sub(1));
                Ok(true)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.log_auto_scroll = false;
                self.log_scroll = self.log_scroll.saturating_sub(1);
                Ok(true)
            }
            KeyCode::Char('G') => {
                self.log_auto_scroll = true;
                self.log_scroll = self.log_lines.len().saturating_sub(1);
                Ok(true)
            }
            KeyCode::Char('g') => {
                self.log_auto_scroll = false;
                self.log_scroll = 0;
                Ok(true)
            }
            KeyCode::Char('/') => {
                self.log_search_active = true;
                self.log_search_query.clear();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn handle_changes_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.changes_scroll += 1;
                Ok(true)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.changes_scroll = self.changes_scroll.saturating_sub(1);
                Ok(true)
            }
            KeyCode::Char('g') => {
                self.changes_scroll = 0;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn handle_history_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.history.is_empty() {
                    self.history_selected =
                        (self.history_selected + 1).min(self.history.len().saturating_sub(1));
                }
                Ok(true)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.history_selected = self.history_selected.saturating_sub(1);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

// ── Rendering ──

pub fn render(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_rebuild))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 6 || inner.width < 30 {
        return;
    }

    // Layout: sub-tabs on top, content below
    let layout = Layout::vertical([
        Constraint::Length(2), // sub-tab bar
        Constraint::Min(4),   // content
    ])
    .split(inner);

    render_sub_tabs(frame, state, theme, lang, layout[0]);

    match state.sub_tab {
        RebuildSubTab::Dashboard => render_dashboard(frame, state, theme, lang, layout[1]),
        RebuildSubTab::Log => render_log(frame, state, theme, lang, layout[1]),
        RebuildSubTab::Changes => render_changes(frame, state, theme, lang, layout[1]),
        RebuildSubTab::History => render_history(frame, state, theme, lang, layout[1]),
    }

    // Popup overlay
    if state.popup == RebuildPopup::ConfirmRebuild {
        render_confirm_popup(frame, state, theme, lang, area);
    }
}

fn render_sub_tabs(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let titles: Vec<Line> = RebuildSubTab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            Line::from(format!(" F{} {} ", i + 1, tab.label(lang)))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(state.sub_tab.index())
        .highlight_style(theme.tab_active())
        .style(theme.tab_inactive())
        .divider("│");

    frame.render_widget(tabs, area);
}

fn render_dashboard(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    if state.phase == BuildPhase::Idle && !state.is_running() {
        render_idle_dashboard(frame, state, theme, lang, area);
        return;
    }

    // Running/finished layout
    let layout = Layout::vertical([
        Constraint::Length(5), // phase boxes (compact: border+1 content line)
        Constraint::Length(5), // active phase explanation (enough for wrapped text)
        Constraint::Length(1), // stats row
        Constraint::Length(1), // separator
        Constraint::Min(4),   // live output
    ])
    .split(area);

    // Phase boxes
    render_phase_boxes(frame, state, theme, lang, layout[0]);

    // Active phase explanation
    render_phase_explanation(frame, state, theme, lang, layout[1]);

    // Stats row
    render_stats_row(frame, state, theme, lang, layout[2]);

    // Separator
    let sep_line = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(sep_line).style(Style::default().fg(theme.border)),
        layout[3],
    );

    // Live output
    render_live_output(frame, state, theme, lang, layout[4]);
}

fn render_phase_boxes(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let phases = BuildPhase::pipeline_phases();
    let phase_labels = ["①", "②", "③", "④", "⑤"];

    // Decide layout: horizontal if wide enough, otherwise vertical
    let wide_enough = area.width >= 70;

    if wide_enough {
        let constraints: Vec<Constraint> = (0..5).map(|_| Constraint::Percentage(20)).collect();
        let cols = Layout::horizontal(constraints).split(area);

        for (i, (phase, col)) in phases.iter().zip(cols.iter()).enumerate() {
            render_single_phase_box(frame, state, theme, lang, *col, *phase, phase_labels[i], i);
        }
    } else {
        // Narrow terminal: stack 2 rows of boxes
        let rows = Layout::vertical([Constraint::Length(3), Constraint::Length(3)])
            .split(area);
        let top = Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(rows[0]);
        let bot = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[1]);

        for (i, col) in top.iter().enumerate() {
            render_single_phase_box(frame, state, theme, lang, *col, phases[i], phase_labels[i], i);
        }
        for (i, col) in bot.iter().enumerate() {
            render_single_phase_box(frame, state, theme, lang, *col, phases[i + 3], phase_labels[i + 3], i + 3);
        }
    }
}

fn render_single_phase_box(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
    phase: BuildPhase,
    num_label: &str,
    idx: usize,
) {
    let current_idx = state.phase.pipeline_index();
    let is_skipped = state.phase_skipped[idx];

    // Determine phase visual state
    let has_timing = state.phase_times[idx].is_some();

    let (is_active, is_done, is_failed) = if state.is_running() {
        // Build in progress
        let active = current_idx == Some(idx);
        let done = current_idx.map_or(false, |ci| idx < ci);
        (active, done, false)
    } else if matches!(state.phase, BuildPhase::Done) {
        // Build succeeded: all non-skipped phases are done
        (false, !is_skipped, false)
    } else if matches!(state.phase, BuildPhase::Failed) {
        // Build failed: use failed_phase_idx to determine which phase failed
        let failed_here = state.failed_phase_idx == Some(idx);
        let completed = has_timing && !failed_here && !is_skipped;
        (false, completed, failed_here)
    } else {
        // Idle
        (false, false, false)
    };

    let (icon, color) = if is_failed {
        ("✗", theme.error)
    } else if is_active {
        ("◉", theme.accent)
    } else if is_done {
        ("✓", theme.success)
    } else if is_skipped {
        ("─", theme.fg_dim)
    } else {
        ("○", theme.fg_dim)
    };

    let border_color = if is_active {
        theme.accent
    } else if is_done {
        theme.success
    } else if is_failed {
        theme.error
    } else {
        theme.border
    };

    let title = format!("{} {}", num_label, phase.label(lang));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title)
        .title_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 1 || inner.width < 4 {
        return;
    }

    // Status line: icon + timer/status
    let timing = state.phase_elapsed_str(idx);
    let status_text = if is_active {
        if timing.is_empty() { "active".to_string() } else { timing }
    } else if is_done {
        if timing.is_empty() { "done".to_string() } else { timing }
    } else if is_failed {
        if timing.is_empty() { "failed".to_string() } else { format!("{} ✗", timing) }
    } else if is_skipped {
        "skipped".to_string()
    } else {
        "waiting".to_string()
    };

    let status_line = Line::from(vec![
        Span::styled(format!(" {} ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(status_text, Style::default().fg(color)),
    ]);

    let status_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(status_line), status_area);

    // Current activity line (if enough space and this is the active phase)
    if inner.height >= 2 && is_active && !state.current_activity.is_empty() {
        let act_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        let max_chars = inner.width as usize - 1;
        let truncated: String = state.current_activity.chars().take(max_chars).collect();
        frame.render_widget(
            Paragraph::new(Line::styled(
                format!(" {}", truncated),
                Style::default().fg(theme.fg_dim),
            )),
            act_area,
        );
    }
}

/// Renders the educational explanation for the currently active phase below the boxes.
/// If the current phase has no explanation (fast transition), shows the last known phase.
fn render_phase_explanation(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    // Determine which phase to show explanation for
    let display_phase = if !state.phase.explanation(lang).is_empty() {
        state.phase
    } else if !state.last_explanation_phase.explanation(lang).is_empty() {
        state.last_explanation_phase
    } else {
        return;
    };

    let phase_name = display_phase.label(lang);
    let explain = display_phase.explanation(lang);
    if explain.is_empty() {
        return;
    }

    let elapsed = state.elapsed_str();

    let lines = vec![
        Line::from(vec![
            Span::styled("  ℹ ", Style::default().fg(theme.accent)),
            Span::styled(
                phase_name,
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ⏱ {}", elapsed),
                Style::default().fg(theme.fg_dim),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(explain, Style::default().fg(theme.fg_dim)),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: true }),
        area,
    );
}

fn render_idle_dashboard(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  ⚡ ", Style::default().fg(theme.accent)),
        Span::styled(
            s.rb_idle_title,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // System detection info
    if state.detecting {
        lines.push(Line::from(vec![
            Span::styled("  ⏳ ", Style::default().fg(theme.warning)),
            Span::styled(s.rb_detecting, Style::default().fg(theme.fg_dim)),
        ]));
    } else if state.detected {
        let config_type = if state.uses_flakes.unwrap_or(false) {
            s.rb_config_flakes
        } else {
            s.rb_config_channels
        };

        lines.push(Line::from(vec![
            Span::styled("  📋 ", Style::default()),
            Span::styled(
                s.rb_config_detected,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {}", config_type), Style::default().fg(theme.accent)),
        ]));

        if let Some(ref path) = state.flake_path {
            lines.push(Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    format!("{}: {}", s.rb_flake_path, path),
                    Style::default().fg(theme.fg_dim),
                ),
            ]));
        }

        lines.push(Line::raw(""));

        // Show current command (dynamic based on mode)
        if state.detected {
            let cmd = state.current_command();
            lines.push(Line::from(vec![
                Span::styled("  $ ", Style::default().fg(theme.success)),
                Span::styled(cmd, Style::default().fg(theme.fg)),
            ]));
        }
    }

    lines.push(Line::raw(""));

    // Current mode + show-trace on separate lines but compact
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            s.rb_current_mode,
            Style::default().fg(theme.fg),
        ),
        Span::styled(
            format!(" [{}]", state.mode.label(lang)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} [m]", s.rb_cycle_mode),
            Style::default().fg(theme.fg_dim),
        ),
        Span::styled("    --show-trace: ", Style::default().fg(theme.fg_dim)),
        if state.show_trace {
            Span::styled("ON", Style::default().fg(theme.success).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("off", Style::default().fg(theme.fg_dim))
        },
        Span::styled(" [t]", Style::default().fg(theme.fg_dim)),
    ]));

    lines.push(Line::raw(""));

    // Hint
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(s.rb_idle_hint, Style::default().fg(theme.fg_dim)),
    ]));

    lines.push(Line::raw(""));

    // Time estimation from history
    if let Some(est) = state.estimated_time() {
        let est_str = format_duration(est);
        lines.push(Line::from(vec![
            Span::styled("  ⏱ ", Style::default().fg(theme.fg_dim)),
            Span::styled(
                s.rb_estimated_time,
                Style::default().fg(theme.fg_dim),
            ),
            Span::styled(
                format!(" ~{}", est_str),
                Style::default().fg(theme.accent),
            ),
        ]));
        lines.push(Line::raw(""));
    }

    // Last build result if available
    if let Some(last) = state.history.last() {
        let status_style = if last.success {
            Style::default().fg(theme.success)
        } else {
            Style::default().fg(theme.error)
        };
        let status_icon = if last.success { "✓" } else { "✗" };
        let duration_str = format_duration(last.duration);

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", s.rb_last_build),
                Style::default().fg(theme.fg_dim),
            ),
            Span::styled(
                format!("{} ", status_icon),
                status_style,
            ),
            Span::styled(
                format!("{} ({}) — {}", last.mode.as_arg(), duration_str, last.timestamp),
                Style::default().fg(theme.fg_dim),
            ),
        ]));

        if let Some(ref err) = last.error_preview {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(err.as_str(), Style::default().fg(theme.error)),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

// (Phase banner replaced by render_phase_boxes above)

// (Progress bar replaced by phase boxes)

fn render_stats_row(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let elapsed_str = format!("⏱ {}", state.elapsed_str());
    let built_str = format!(
        "{}:{}",
        s.rb_stat_built,
        state.stats.derivations_built
    );
    let fetched_str = format!(
        "{}:{}",
        s.rb_stat_fetched,
        state.stats.fetched
    );
    let warn_str = format!("⚠:{}", state.stats.warnings);
    let err_str = format!("✗:{}", state.stats.errors);

    let mut spans = vec![
        Span::styled(format!("  {}", elapsed_str), Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("  │  ", Style::default().fg(theme.border)),
        Span::styled(built_str, Style::default().fg(theme.fg)),
        Span::styled("  │  ", Style::default().fg(theme.border)),
        Span::styled(fetched_str, Style::default().fg(theme.fg)),
        Span::styled("  │  ", Style::default().fg(theme.border)),
        Span::styled(
            warn_str,
            if state.stats.warnings > 0 {
                Style::default().fg(theme.warning)
            } else {
                Style::default().fg(theme.fg_dim)
            },
        ),
        Span::styled("  │  ", Style::default().fg(theme.border)),
        Span::styled(
            err_str,
            if state.stats.errors > 0 {
                Style::default().fg(theme.error)
            } else {
                Style::default().fg(theme.fg_dim)
            },
        ),
    ];

    if state.is_running() {
        spans.push(Span::styled("  │  ", Style::default().fg(theme.border)));
        spans.push(Span::styled("[c] cancel", Style::default().fg(theme.fg_dim)));
    } else if matches!(state.phase, BuildPhase::Done | BuildPhase::Failed) {
        spans.push(Span::styled("  │  ", Style::default().fg(theme.border)));
        spans.push(Span::styled("[Esc] back  [r] rebuild", Style::default().fg(theme.fg_dim)));
    }

    let stats = Line::from(spans);

    frame.render_widget(Paragraph::new(stats), area);
}

fn render_live_output(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if area.height < 2 {
        return;
    }

    let visible_lines = area.height.saturating_sub(1) as usize;
    let total = state.log_lines.len();

    let scroll_pos = if state.log_auto_scroll {
        total.saturating_sub(visible_lines)
    } else {
        state.log_scroll.min(total.saturating_sub(visible_lines))
    };

    // Header
    let header = Line::from(vec![
        Span::styled(
            format!("  {} ", s.rb_live_output),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({}/{})", scroll_pos + visible_lines.min(total), total),
            Style::default().fg(theme.fg_dim),
        ),
        if state.log_auto_scroll {
            Span::styled(
                format!("  [{}]", s.rb_auto_scroll),
                Style::default().fg(theme.success),
            )
        } else {
            Span::styled(
                format!("  [G] {}", s.rb_resume_scroll),
                Style::default().fg(theme.fg_dim),
            )
        },
    ]);

    let header_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(header), header_area);

    let lines_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    let lines: Vec<ListItem> = state
        .log_lines
        .iter()
        .skip(scroll_pos)
        .take(visible_lines)
        .map(|line| {
            let style = match line.level {
                LogLevel::Normal => Style::default().fg(theme.fg),
                LogLevel::Info => Style::default().fg(theme.accent),
                LogLevel::Warning => Style::default().fg(theme.warning),
                LogLevel::Error => Style::default().fg(theme.error),
                LogLevel::Phase => Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            };

            let max_chars = area.width as usize - 2;
            let display = if line.text.chars().count() > max_chars {
                let truncated: String = line.text.chars().take(max_chars - 2).collect();
                format!(" {}", truncated)
            } else {
                format!(" {}", line.text)
            };

            ListItem::new(Line::styled(display, style))
        })
        .collect();

    let list = List::new(lines);
    frame.render_widget(list, lines_area);
}

fn render_log(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if state.log_lines.is_empty() {
        let empty_msg = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(
                s.rb_log_empty,
                Style::default().fg(theme.fg_dim),
            ),
            Line::styled(
                s.rb_log_empty_hint,
                Style::default().fg(theme.fg_dim),
            ),
        ];
        frame.render_widget(
            Paragraph::new(empty_msg).alignment(Alignment::Center),
            area,
        );
        return;
    }

    let visible_lines = area.height as usize;
    let total = state.log_lines.len();
    let scroll_pos = if state.log_auto_scroll {
        total.saturating_sub(visible_lines)
    } else {
        state.log_scroll.min(total.saturating_sub(visible_lines))
    };

    let search_query = if !state.log_search_query.is_empty() {
        Some(state.log_search_query.as_str())
    } else {
        None
    };

    let lines: Vec<ListItem> = state
        .log_lines
        .iter()
        .skip(scroll_pos)
        .take(visible_lines)
        .map(|line| {
            let style = match line.level {
                LogLevel::Normal => Style::default().fg(theme.fg),
                LogLevel::Info => Style::default().fg(theme.accent),
                LogLevel::Warning => Style::default().fg(theme.warning),
                LogLevel::Error => Style::default().fg(theme.error),
                LogLevel::Phase => Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            };

            // Log tab shows RAW output (full nix paths, unmodified)
            let raw = &line.raw;

            // Highlight search matches
            let highlighted = if let Some(query) = search_query {
                if !query.is_empty() && raw.to_lowercase().contains(&query.to_lowercase()) {
                    Style::default()
                        .fg(theme.selection_fg)
                        .bg(theme.selection_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    style
                }
            } else {
                style
            };

            let display = format!(" {}", raw);
            ListItem::new(Line::styled(display, highlighted))
        })
        .collect();

    let list = List::new(lines);
    frame.render_widget(list, area);

    // Search bar overlay at bottom if active
    if state.log_search_active {
        let search_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };
        frame.render_widget(Clear, search_area);
        let search_line = Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent)),
            Span::styled(
                &state.log_search_query,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled("_", Style::default().fg(theme.accent)),
        ]);
        frame.render_widget(Paragraph::new(search_line), search_area);
    }
}

fn render_changes(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let diff = match &state.diff {
        Some(d) => d,
        None => {
            // No diff available yet
            let msg = if state.is_running() {
                s.rb_changes_pending
            } else if state.phase == BuildPhase::Idle {
                s.rb_changes_no_build
            } else {
                s.rb_changes_empty
            };

            let content = vec![
                Line::raw(""),
                Line::raw(""),
                Line::styled(msg, Style::default().fg(theme.fg_dim)),
            ];
            frame.render_widget(
                Paragraph::new(content).alignment(Alignment::Center),
                area,
            );
            return;
        }
    };

    let mut lines: Vec<Line> = Vec::new();

    // Summary header
    let total_changes = diff.added.len() + diff.removed.len() + diff.updated.len();
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {} ", s.rb_changes_summary),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "+{} {} / -{} {} / ~{} {}",
                diff.added.len(),
                s.rb_changes_added,
                diff.removed.len(),
                s.rb_changes_removed,
                diff.updated.len(),
                s.rb_changes_updated,
            ),
            Style::default().fg(theme.fg),
        ),
    ]));
    lines.push(Line::raw(""));

    // Kernel change warning
    if let Some((ref old, ref new)) = diff.kernel_changed {
        lines.push(Line::from(vec![
            Span::styled("  ⚠ ", Style::default().fg(theme.warning)),
            Span::styled(
                s.rb_kernel_changed,
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} → {}", old, new),
                Style::default().fg(theme.fg),
            ),
        ]));
        if diff.reboot_needed {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(
                    s.rb_reboot_needed,
                    Style::default().fg(theme.error).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // NixOS version change
    if let Some((ref old, ref new)) = diff.nixos_version {
        lines.push(Line::from(vec![
            Span::styled("  🔄 ", Style::default()),
            Span::styled("NixOS: ", Style::default().fg(theme.fg)),
            Span::styled(old.as_str(), Style::default().fg(theme.diff_removed)),
            Span::styled(" → ", Style::default().fg(theme.fg_dim)),
            Span::styled(new.as_str(), Style::default().fg(theme.diff_added)),
        ]));
        lines.push(Line::raw(""));
    }

    // Services restarted
    if !diff.services_restarted.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  ⚙ ", Style::default()),
            Span::styled(
                format!("{} ({})", s.rb_services_restarted, diff.services_restarted.len()),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for svc in &diff.services_restarted {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(svc.as_str(), Style::default().fg(theme.fg)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Packages added
    if !diff.added.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  ✚ {} ({})", s.rb_changes_added, diff.added.len()),
                Style::default()
                    .fg(theme.diff_added)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for (name, ver) in &diff.added {
            lines.push(Line::from(vec![
                Span::styled("    + ", Style::default().fg(theme.diff_added)),
                Span::styled(name.as_str(), Style::default().fg(theme.fg)),
                Span::styled(format!(" {}", ver), Style::default().fg(theme.fg_dim)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Packages removed
    if !diff.removed.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  ✖ {} ({})", s.rb_changes_removed, diff.removed.len()),
                Style::default()
                    .fg(theme.diff_removed)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for (name, ver) in &diff.removed {
            lines.push(Line::from(vec![
                Span::styled("    - ", Style::default().fg(theme.diff_removed)),
                Span::styled(name.as_str(), Style::default().fg(theme.fg)),
                Span::styled(format!(" {}", ver), Style::default().fg(theme.fg_dim)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Packages updated
    if !diff.updated.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  ↑ {} ({})", s.rb_changes_updated, diff.updated.len()),
                Style::default()
                    .fg(theme.diff_updated)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for (name, old_v, new_v) in &diff.updated {
            lines.push(Line::from(vec![
                Span::styled("    ~ ", Style::default().fg(theme.diff_updated)),
                Span::styled(name.as_str(), Style::default().fg(theme.fg)),
                Span::styled(format!(" {} → {}", old_v, new_v), Style::default().fg(theme.fg_dim)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    if total_changes == 0 && diff.kernel_changed.is_none() && diff.services_restarted.is_empty() {
        lines.push(Line::styled(
            format!("  {}", s.rb_no_changes),
            Style::default().fg(theme.fg_dim),
        ));
    }

    // Apply scroll
    let visible = area.height as usize;
    let max_scroll = lines.len().saturating_sub(visible);
    let scroll = state.changes_scroll.min(max_scroll);

    let display_lines: Vec<Line> = lines.into_iter().skip(scroll).take(visible).collect();

    frame.render_widget(Paragraph::new(display_lines), area);
}

fn render_history(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    if state.history.is_empty() {
        let content = vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(s.rb_history_empty, Style::default().fg(theme.fg_dim)),
            Line::styled(
                s.rb_history_empty_hint,
                Style::default().fg(theme.fg_dim),
            ),
        ];
        frame.render_widget(
            Paragraph::new(content).alignment(Alignment::Center),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = state
        .history
        .iter()
        .enumerate()
        .rev() // newest first
        .map(|(i, entry)| {
            // Map visual position to data index: reversed, so visual row 0 = last data index
            let visual_idx = state.history.len().saturating_sub(1) - i;
            let is_selected = visual_idx == state.history_selected;
            let status_icon = if entry.success { "✓" } else { "✗" };
            let status_color = if entry.success {
                theme.success
            } else {
                theme.error
            };

            let duration_str = format_duration(entry.duration);

            let spans = vec![
                Span::styled(
                    if is_selected { " ▸ " } else { "   " },
                    Style::default().fg(theme.accent),
                ),
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("{} ", entry.timestamp),
                    Style::default().fg(theme.fg_dim),
                ),
                Span::styled(
                    format!("{} ", entry.mode.as_arg()),
                    Style::default().fg(theme.accent),
                ),
                Span::styled(
                    format!("({})", duration_str),
                    Style::default().fg(theme.fg_dim),
                ),
            ];

            let mut lines = vec![Line::from(spans)];

            // Show error preview for failed builds
            if !entry.success {
                if let Some(ref err) = entry.error_preview {
                    lines.push(Line::from(vec![
                        Span::styled("     ", Style::default()),
                        Span::styled(
                            err.as_str(),
                            Style::default().fg(theme.error),
                        ),
                    ]));
                }
            }

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, area);
}

fn render_confirm_popup(
    frame: &mut Frame,
    state: &RebuildState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let cmd = state.current_command();
    let mode_label = state.mode.label(lang);

    // Password mask
    let pw_display = if state.password_buffer.is_empty() {
        s.rb_password_hint.to_string()
    } else {
        "●".repeat(state.password_buffer.len())
    };

    let content = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                format!("  {}: ", s.rb_confirm_mode),
                Style::default().fg(theme.fg),
            ),
            Span::styled(
                mode_label,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                format!("  {}: ", s.rb_confirm_cmd),
                Style::default().fg(theme.fg),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("  $ {}", cmd),
                Style::default().fg(theme.success),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("  {}", s.rb_sudo_note),
                Style::default().fg(theme.fg_dim),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                format!("  {} ", s.rb_password_label),
                Style::default().fg(theme.fg),
            ),
            Span::styled(
                format!("[{}]", pw_display),
                if state.password_buffer.is_empty() {
                    Style::default().fg(theme.fg_dim)
                } else {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                s.rb_nopasswd_hint,
                Style::default().fg(theme.fg_dim),
            ),
        ]),
    ];

    // Use custom popup rendering for wider width
    let popup_width = 66.min(area.width.saturating_sub(4));
    let popup_height = (content.len() as u16 + 6).min(area.height.saturating_sub(4));
    let popup_area = widgets::centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.rb_confirm_title))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());
    frame.render_widget(block, popup_area);

    let inner = Rect {
        x: popup_area.x + 2,
        y: popup_area.y + 1,
        width: popup_area.width.saturating_sub(4),
        height: popup_area.height.saturating_sub(4),
    };

    let content_widget = Paragraph::new(content)
        .style(theme.text())
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(content_widget, inner);

    // Bottom buttons
    let button_area = Rect {
        x: popup_area.x + 2,
        y: popup_area.y + popup_area.height - 2,
        width: popup_area.width.saturating_sub(4),
        height: 1,
    };

    let buttons = Line::from(vec![
        Span::styled("[", theme.text_dim()),
        Span::styled("⏎", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("] ", theme.text_dim()),
        Span::styled(s.rb_password_submit, theme.text()),
        Span::raw("    "),
        Span::styled("[", theme.text_dim()),
        Span::styled("⎋", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("] ", theme.text_dim()),
        Span::styled(s.cancel, theme.text()),
    ]);

    frame.render_widget(
        Paragraph::new(buttons).alignment(Alignment::Center),
        button_area,
    );
}

// ── Background rebuild logic ──

fn run_rebuild(tx: mpsc::Sender<RebuildMsg>, mode: RebuildMode, uses_flakes: bool, flake_path: Option<&str>, password: Option<String>, show_trace: bool, child_pid: Arc<AtomicU32>, auth_msg: String) {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    // Phase 1: Take pre-rebuild snapshot
    let _ = tx.send(RebuildMsg::Phase(BuildPhase::Preparing));
    let pre_snapshot = take_package_snapshot();
    let _ = tx.send(RebuildMsg::PreSnapshot(
        pre_snapshot.0.clone(),
        pre_snapshot.1.clone(),
        pre_snapshot.2.clone(),
    ));

    // Phase 2: Build the command
    let _ = tx.send(RebuildMsg::Phase(BuildPhase::Evaluating));

    let cmd_str = build_rebuild_command(mode.as_arg(), uses_flakes, flake_path);

    // Build the command args
    let (program, base_args) = cmd_str;
    let has_sudo = program == "sudo";
    let mut args: Vec<String> = if has_sudo && password.is_some() {
        // Insert -S flag after "sudo" to read password from stdin
        let mut new_args = vec!["-S".to_string()];
        new_args.extend(base_args);
        new_args
    } else {
        base_args
    };

    if show_trace {
        args.push("--show-trace".into());
    }

    if password.is_some() {
        let _ = tx.send(RebuildMsg::OutputLine(auth_msg));
    }

    let mut child = match Command::new(&program)
        .args(&args)
        .stdin(if password.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(RebuildMsg::OutputLine(format!("Failed to start: {}", e)));
            let _ = tx.send(RebuildMsg::Finished(false, Some(e.to_string())));
            return;
        }
    };

    // Store child PID for cancellation
    child_pid.store(child.id(), Ordering::SeqCst);

    // Write password to sudo's stdin if provided
    if let Some(ref pw) = password {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = writeln!(stdin, "{}", pw);
            drop(stdin); // Close stdin so sudo proceeds
        }
    }
    // Password is dropped here (consumed by move into closure / dropped at end of scope)

    // Read stderr in a separate thread
    let stderr = child.stderr.take();
    let tx_stderr = tx.clone();
    let stderr_handle = std::thread::spawn(move || {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut stats = BuildStats::default();
            let mut current_phase = BuildPhase::Evaluating;

            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => continue,
                };

                // Phase detection
                let new_phase = detect_phase(&line, current_phase);
                if new_phase != current_phase {
                    current_phase = new_phase;
                    let _ = tx_stderr.send(RebuildMsg::Phase(new_phase));
                }

                // Stats tracking
                update_stats(&line, &mut stats);
                let _ = tx_stderr.send(RebuildMsg::Stats(stats.clone()));

                // Service restart detection
                if let Some(svc) = detect_service_restart(&line) {
                    let _ = tx_stderr.send(RebuildMsg::ServiceRestart(svc));
                }

                let _ = tx_stderr.send(RebuildMsg::OutputLine(line));
            }
        }
    });

    // Read stdout
    let stdout = child.stdout.take();
    let tx_stdout = tx.clone();
    let stdout_handle = std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx_stdout.send(RebuildMsg::OutputLine(line));
                }
            }
        }
    });

    // Wait for process to complete
    let status = child.wait();
    let _ = stderr_handle.join();
    let _ = stdout_handle.join();

    let (success, err_msg) = match status {
        Ok(s) => {
            if s.success() {
                (true, None)
            } else {
                (false, Some(format!("Exit code: {:?}", s.code())))
            }
        }
        Err(e) => (false, Some(e.to_string())),
    };

    // Phase 3: Post-rebuild snapshot (only if successful)
    if success {
        // Don't change phase here — the correct phases were already detected
        // from the build output. Just take the snapshot silently.
        std::thread::sleep(std::time::Duration::from_millis(500));

        let post_snapshot = take_package_snapshot();
        let _ = tx.send(RebuildMsg::PostSnapshot(
            post_snapshot.0,
            post_snapshot.1,
            post_snapshot.2,
        ));
    }

    let _ = tx.send(RebuildMsg::Finished(success, err_msg));
}

// ── System detection helpers ──

fn build_rebuild_command(mode: &str, uses_flakes: bool, flake_path: Option<&str>) -> (String, Vec<String>) {
    if uses_flakes {
        let path = flake_path.unwrap_or("/etc/nixos");
        (
            "sudo".into(),
            vec![
                "nixos-rebuild".into(),
                mode.into(),
                "--flake".into(),
                format!("{}#", path),
            ],
        )
    } else {
        (
            "sudo".into(),
            vec!["nixos-rebuild".into(), mode.into()],
        )
    }
}

// ── Line parsing ──

fn detect_phase(line: &str, current: BuildPhase) -> BuildPhase {
    let lower = line.to_lowercase();

    // Evaluation phase markers
    if lower.contains("evaluating") || lower.contains("trace:") {
        return BuildPhase::Evaluating;
    }

    // Building phase markers
    if lower.contains("building '") || lower.contains("these derivations will be built") || lower.contains("these paths will be fetched") {
        return BuildPhase::Building;
    }

    // Fetching from cache
    if lower.contains("copying path") || lower.contains("fetching ") || lower.contains("downloading ") {
        return BuildPhase::Fetching;
    }

    // Bootloader phase (must check BEFORE activation since boot keywords are distinct)
    if lower.contains("updating boot")
        || lower.contains("installing boot")
        || lower.contains("updating the boot")
        || lower.contains("grub")
        || lower.contains("systemd-boot")
        || lower.contains("bootctl")
        || lower.contains("updating efi")
    {
        return BuildPhase::Bootloader;
    }

    // Activation phase
    if lower.contains("activating the configuration")
        || lower.contains("setting up")
        || lower.contains("switching to")
        || lower.contains("updating systemd")
        || lower.contains("reloading systemd")
        || lower.contains("restarting")
        || lower.contains("stopping")
        || lower.contains("starting")
    {
        return BuildPhase::Activating;
    }

    current
}

fn update_stats(line: &str, stats: &mut BuildStats) {
    let lower = line.to_lowercase();

    // Count building derivations
    if lower.contains("building '") {
        stats.derivations_built += 1;
    }

    // Parse "these X derivations will be built" for total
    if lower.contains("derivations will be built") || lower.contains("derivation(s) will be built") {
        if let Some(num) = extract_number(line) {
            stats.derivations_total = Some(num);
        }
    }

    // Fetched paths
    if lower.contains("copying path") || lower.contains("fetching path") {
        stats.fetched += 1;
    }

    // Warnings
    if lower.contains("warning:") {
        stats.warnings += 1;
    }

    // Errors
    if lower.contains("error:") {
        stats.errors += 1;
    }
}

fn extract_number(line: &str) -> Option<u32> {
    for word in line.split_whitespace() {
        if let Ok(n) = word.parse::<u32>() {
            return Some(n);
        }
    }
    None
}

fn detect_service_restart(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if lower.contains("restarting") || lower.contains("starting") {
        // Try to extract service name
        // Common format: "restarting the following units: foo.service"
        // or: "starting the following units: bar.service"
        if let Some(idx) = lower.find("units:") {
            let rest = &line[idx + 6..];
            let services: Vec<&str> = rest.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !services.is_empty() {
                return Some(services.join(", "));
            }
        }
        // Format: "restarting foo.service..."
        if let Some(idx) = lower.find(".service") {
            let before = &line[..idx];
            if let Some(last_space) = before.rfind(' ') {
                let svc = before[last_space + 1..].trim();
                if !svc.is_empty() {
                    return Some(format!("{}.service", svc));
                }
            }
        }
    }
    None
}

fn classify_line(line: &str) -> LogLevel {
    let lower = line.to_lowercase();
    if lower.contains("error:") || lower.contains("error ") || lower.starts_with("error") {
        LogLevel::Error
    } else if lower.contains("warning:") {
        LogLevel::Warning
    } else if lower.contains("building '") || lower.contains("fetching ") || lower.contains("copying path") {
        LogLevel::Info
    } else {
        LogLevel::Normal
    }
}

fn phase_label(phase: BuildPhase, lang: crate::config::Language) -> &'static str {
    let s = crate::i18n::get_strings(lang);
    match phase {
        BuildPhase::Idle => s.rb_phase_idle,
        BuildPhase::Preparing => s.rb_phase_preparing,
        BuildPhase::Evaluating => s.rb_phase_evaluating,
        BuildPhase::Building => s.rb_phase_building,
        BuildPhase::Fetching => s.rb_phase_fetching,
        BuildPhase::Activating => s.rb_phase_activating,
        BuildPhase::Bootloader => s.rb_phase_bootloader,
        BuildPhase::Done => s.rb_phase_done,
        BuildPhase::Failed => s.rb_phase_failed,
    }
}

// ── Package snapshot for diff ──

fn take_package_snapshot() -> (Vec<(String, String)>, Option<String>, Option<String>) {
    let mut packages = Vec::new();
    let mut kernel = None;
    let mut nixos_ver = None;

    // Get current system profile path
    let system_path = std::path::Path::new("/run/current-system");

    // Try to get NixOS version
    let ver_path = system_path.join("nixos-version");
    if ver_path.exists() {
        if let Ok(v) = std::fs::read_to_string(&ver_path) {
            nixos_ver = Some(v.trim().to_string());
        }
    }

    // Try to get kernel version
    let kernel_modules = system_path.join("kernel-modules/lib/modules");
    if kernel_modules.exists() {
        if let Ok(entries) = std::fs::read_dir(&kernel_modules) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') {
                    kernel = Some(name);
                    break;
                }
            }
        }
    }

    // Get packages via sw/bin listing (fast method)
    let sw_path = system_path.join("sw/bin");
    if sw_path.exists() {
        // Use nix path-info for accurate package list — but with timeout
        let output = std::process::Command::new("nix")
            .args(["path-info", "-r", "--json"])
            .arg(system_path.to_string_lossy().as_ref())
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                packages = parse_path_info_for_snapshot(&stdout);
            }
        }
    }

    // Fallback: just list sw/bin contents for a rough package list
    if packages.is_empty() {
        // Use a simple heuristic
        if let Ok(output) = std::process::Command::new("ls")
            .arg("-1")
            .arg(system_path.join("sw/bin").to_string_lossy().as_ref())
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let name = line.trim().to_string();
                    if !name.is_empty() {
                        packages.push((name, String::new()));
                    }
                }
            }
        }
    }

    (packages, kernel, nixos_ver)
}

fn parse_path_info_for_snapshot(json_str: &str) -> Vec<(String, String)> {
    // Parse nix path-info JSON to extract package names and versions
    let mut packages = Vec::new();

    // The JSON is an object with store paths as keys
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(obj) = val.as_object() {
            for (path, _) in obj {
                if let Some((name, version)) = parse_store_path_name(path) {
                    if !should_skip_pkg(&name) {
                        packages.push((name, version));
                    }
                }
            }
        }
        // Sometimes it's an array
        if let Some(arr) = val.as_array() {
            for item in arr {
                let path = if let Some(p) = item.get("path").and_then(|v| v.as_str()) {
                    p.to_string()
                } else if let Some(s) = item.as_str() {
                    s.to_string()
                } else {
                    continue;
                };
                if let Some((name, version)) = parse_store_path_name(&path) {
                    if !should_skip_pkg(&name) {
                        packages.push((name, version));
                    }
                }
            }
        }
    }

    packages.sort_by(|a, b| a.0.cmp(&b.0));
    packages.dedup_by(|a, b| a.0 == b.0);
    packages
}

fn parse_store_path_name(path: &str) -> Option<(String, String)> {
    // Format: /nix/store/hash-name-version
    let basename = path.rsplit('/').next()?;
    // Skip the hash prefix (32 chars + dash)
    if basename.len() < 34 {
        return None;
    }
    let rest = &basename[33..]; // skip "hash-"
    // Split name and version — version usually starts with a digit
    let parts: Vec<&str> = rest.rsplitn(2, '-').collect();
    if parts.len() == 2 {
        let maybe_ver = parts[0];
        if maybe_ver.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            return Some((parts[1].to_string(), maybe_ver.to_string()));
        }
    }
    Some((rest.to_string(), String::new()))
}

fn should_skip_pkg(name: &str) -> bool {
    // Skip infrastructure packages that aren't meaningful for users
    let skip_prefixes = [
        "hook", "setup-hook", "source", "patch", "wrap",
        "move-", "make-", "compress-", "strip-",
        "audit-", "fixup-",
    ];
    let skip_names = [
        "stdenv", "builder", "raw", "env-manifest",
    ];
    skip_prefixes.iter().any(|p| name.starts_with(p))
        || skip_names.iter().any(|n| name == *n)
}

// ── Diff calculation ──

fn calculate_diff(
    pre_pkgs: &[(String, String)],
    post_pkgs: &[(String, String)],
    pre_kernel: &Option<String>,
    post_kernel: &Option<String>,
    pre_ver: &Option<String>,
    post_ver: &Option<String>,
) -> RebuildDiff {
    use std::collections::HashMap;

    let pre_map: HashMap<&str, &str> = pre_pkgs.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let post_map: HashMap<&str, &str> = post_pkgs.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut updated = Vec::new();

    for (name, ver) in post_pkgs {
        match pre_map.get(name.as_str()) {
            None => added.push((name.clone(), ver.clone())),
            Some(&old_ver) => {
                if !old_ver.is_empty() && !ver.is_empty() && old_ver != ver.as_str() {
                    updated.push((name.clone(), old_ver.to_string(), ver.clone()));
                }
            }
        }
    }

    for (name, ver) in pre_pkgs {
        if !post_map.contains_key(name.as_str()) {
            removed.push((name.clone(), ver.clone()));
        }
    }

    let kernel_changed = match (pre_kernel, post_kernel) {
        (Some(old), Some(new)) if old != new => Some((old.clone(), new.clone())),
        _ => None,
    };

    let reboot_needed = kernel_changed.is_some();

    let nixos_version = match (pre_ver, post_ver) {
        (Some(old), Some(new)) if old != new => Some((old.clone(), new.clone())),
        _ => None,
    };

    RebuildDiff {
        added,
        removed,
        updated,
        kernel_changed,
        reboot_needed,
        services_restarted: Vec::new(),
        nixos_version,
    }
}

// ── Helpers ──

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let m = secs / 60;
    let s = secs % 60;
    if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// Beautify Nix store paths and common output into human-readable messages.
/// This is the "intelligent log" feature — the core UX improvement over raw terminal output.
fn beautify_store_path(line: &str) -> String {
    let lower = line.to_lowercase();

    // Pattern: building '/nix/store/hash-name-version.drv'
    if lower.contains("building '") {
        if let Some(start) = line.find("/nix/store/") {
            if let Some(end) = line[start..].find('\'') {
                let store_path = &line[start..start + end];
                if let Some((name, version)) = parse_store_path_name(store_path) {
                    let clean_name = name.trim_end_matches(".drv");
                    if version.is_empty() || version.ends_with(".drv") {
                        let clean_ver = version.trim_end_matches(".drv");
                        if clean_ver.is_empty() {
                            return format!("🔨 Building {}", clean_name);
                        }
                        return format!("🔨 Building {} {}", clean_name, clean_ver);
                    }
                    return format!("🔨 Building {} {}", clean_name, version);
                }
            }
        }
    }

    // Pattern: copying path '/nix/store/hash-name-version' ...
    if lower.contains("copying path") || lower.contains("fetching path") {
        if let Some(start) = line.find("/nix/store/") {
            let rest = &line[start..];
            let end = rest.find('\'').or_else(|| rest.find(' ')).unwrap_or(rest.len());
            let store_path = &rest[..end];
            if let Some((name, version)) = parse_store_path_name(store_path) {
                if version.is_empty() {
                    return format!("📦 Fetching {}", name);
                }
                return format!("📦 Fetching {} {}", name, version);
            }
        }
    }

    // Pattern: "these N derivations will be built:"
    if lower.contains("derivations will be built") || lower.contains("derivation(s) will be built") {
        if let Some(n) = extract_number(line) {
            return format!("📋 {} derivations to build", n);
        }
    }

    // Pattern: "these N paths will be fetched (M MiB download, ..."
    if lower.contains("paths will be fetched") {
        if let Some(n) = extract_number(line) {
            // Try to extract size info
            if let Some(size_start) = line.find('(') {
                if let Some(size_end) = line.find(')') {
                    let size_info = &line[size_start + 1..size_end];
                    return format!("📋 {} paths to fetch ({})", n, size_info);
                }
            }
            return format!("📋 {} paths to fetch from cache", n);
        }
    }

    // Pattern: "evaluating derivation ..."
    if lower.starts_with("evaluating") {
        return format!("⚙ {}", line);
    }

    // Pattern: "activating the configuration..."
    if lower.contains("activating the configuration") {
        return "⚡ Activating new system configuration".to_string();
    }

    // Pattern: "setting up /etc..."
    if lower.contains("setting up /etc") {
        return "📁 Updating /etc configuration files".to_string();
    }

    // Pattern: "restarting the following units: ..."
    if lower.contains("restarting the following units:") {
        let units = line.split("units:").nth(1).unwrap_or("").trim();
        return format!("🔄 Restarting: {}", units);
    }

    // Pattern: "starting the following units: ..."
    if lower.contains("starting the following units:") {
        let units = line.split("units:").nth(1).unwrap_or("").trim();
        return format!("▶ Starting: {}", units);
    }

    // Pattern: "stopping the following units: ..."
    if lower.contains("stopping the following units:") {
        let units = line.split("units:").nth(1).unwrap_or("").trim();
        return format!("⏹ Stopping: {}", units);
    }

    // Pattern: "reloading the following units: ..."
    if lower.contains("reloading the following units:") {
        let units = line.split("units:").nth(1).unwrap_or("").trim();
        return format!("🔃 Reloading: {}", units);
    }

    // Pattern: "updating GRUB" / "updating systemd-boot"
    if lower.contains("updating grub") || lower.contains("installing grub") {
        return "🥾 Updating GRUB bootloader".to_string();
    }
    if lower.contains("updating systemd-boot") || lower.contains("installing systemd-boot") {
        return "🥾 Updating systemd-boot".to_string();
    }

    // Pattern: warning: ...
    if lower.starts_with("warning:") {
        return format!("⚠ {}", line);
    }

    // Pattern: error: ...
    if lower.starts_with("error:") {
        return format!("✗ {}", line);
    }

    // Pattern: trace: ... (Nix evaluation trace)
    if lower.starts_with("trace:") {
        // Shorten long traces
        if line.chars().count() > 100 {
            let truncated: String = line.chars().take(97).collect();
            return format!("… {}", truncated);
        }
    }

    line.to_string()
}

// ── Persistent history ──

fn history_path() -> std::path::PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("nixmate");
    config_dir.join("rebuild_history.json")
}

fn load_history() -> Option<Vec<HistoryEntry>> {
    let path = history_path();
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_history(history: &[HistoryEntry]) -> Result<(), Box<dyn std::error::Error>> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Keep last 100 entries
    let to_save: Vec<&HistoryEntry> = history.iter().rev().take(100).collect::<Vec<_>>().into_iter().rev().collect();
    let json = serde_json::to_string_pretty(&to_save)?;
    std::fs::write(&path, json)?;
    Ok(())
}
