//! Main rendering module for nixmate
//!
//! Renders the complete UI:
//! - Vertical sidebar with categories (left)
//! - Active module content area (right)
//! - Global status bar (bottom)
//! - Popup overlays + flash messages

use crate::app::{App, PopupState};
use crate::config::Language;
use crate::i18n;
use crate::ui::widgets;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Tab definition with index for keybinding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleTab {
    Generations,
    Errors,
    Services,
    Storage,
    Config,
    Options,
    Rebuild,
    FlakeInputs,
    Packages,
    Health,
    Settings,
    HelpAbout,
}

impl ModuleTab {
    pub fn index(&self) -> usize {
        match self {
            ModuleTab::Generations => 0,
            ModuleTab::Errors => 1,
            ModuleTab::Services => 2,
            ModuleTab::Storage => 3,
            ModuleTab::Config => 4,
            ModuleTab::Options => 5,
            ModuleTab::Rebuild => 6,
            ModuleTab::FlakeInputs => 7,
            ModuleTab::Packages => 8,
            ModuleTab::Health => 9,
            ModuleTab::Settings => 10,
            ModuleTab::HelpAbout => 11,
        }
    }

    /// Get the localized label for this tab
    pub fn label(&self, app: &App) -> &'static str {
        let s = i18n::get_strings(app.config.language);
        match self {
            ModuleTab::Generations => s.tab_generations,
            ModuleTab::Errors => s.tab_errors,
            ModuleTab::Services => s.tab_services,
            ModuleTab::Storage => s.tab_storage,
            ModuleTab::Config => s.tab_config,
            ModuleTab::Options => s.tab_options,
            ModuleTab::Rebuild => s.tab_rebuild,
            ModuleTab::FlakeInputs => s.tab_flake_inputs,
            ModuleTab::Packages => s.tab_packages,
            ModuleTab::Health => s.tab_health,
            ModuleTab::Settings => s.tab_settings,
            ModuleTab::HelpAbout => s.tab_help,
        }
    }

    /// Keybind hint shown in sidebar
    pub fn key_hint(&self) -> &'static str {
        match self {
            ModuleTab::Generations => "1",
            ModuleTab::Errors => "2",
            ModuleTab::Services => "3",
            ModuleTab::Storage => "4",
            ModuleTab::Config => "5",
            ModuleTab::Options => "6",
            ModuleTab::Rebuild => "7",
            ModuleTab::FlakeInputs => "8",
            ModuleTab::Packages => "9",
            ModuleTab::Health => "0",
            ModuleTab::Settings => ",",
            ModuleTab::HelpAbout => "?",
        }
    }
}

/// Modules shown in the main sidebar area (numbered 1-9, 0)
const SIDEBAR_MODULES: &[ModuleTab] = &[
    ModuleTab::Generations,
    ModuleTab::Errors,
    ModuleTab::Services,
    ModuleTab::Storage,
    ModuleTab::Config,
    ModuleTab::Options,
    ModuleTab::Rebuild,
    ModuleTab::FlakeInputs,
    ModuleTab::Packages,
    ModuleTab::Health,
];

/// Bottom items (below separator)
const SIDEBAR_BOTTOM: &[ModuleTab] = &[ModuleTab::Settings, ModuleTab::HelpAbout];

const SIDEBAR_WIDTH: u16 = 24;

/// Main render function – entry point for all UI rendering
pub fn render(frame: &mut Frame, app: &mut App) {
    // Reset image area each frame
    app.image_area = None;

    // Welcome screen takes over the entire screen (first run only)
    if app.welcome.active {
        app.image_area = crate::modules::splash::render_welcome(
            frame,
            &app.welcome,
            &app.theme,
            app.image_protocol.is_supported(),
        );
        return;
    }

    let area = frame.area();
    let theme = &app.theme;

    // Fill entire background
    frame.render_widget(Block::default().style(theme.block_style()), area);

    // Main layout: sidebar | content, status bar at bottom
    let vertical = Layout::vertical([
        Constraint::Min(8),    // sidebar + content
        Constraint::Length(1), // status bar
    ])
    .split(area);

    let horizontal = Layout::horizontal([
        Constraint::Length(SIDEBAR_WIDTH),
        Constraint::Min(30), // content area
    ])
    .split(vertical[0]);

    render_sidebar(frame, app, horizontal[0]);
    render_module_content(frame, app, horizontal[1]);
    render_status_bar(frame, app, vertical[1]);

    // Popup overlays
    render_popups(frame, app, area);
}

/// Render the vertical sidebar
fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let sidebar_block = Block::default()
        .style(theme.block_style())
        .borders(Borders::RIGHT)
        .border_style(theme.border());
    let inner = sidebar_block.inner(area);
    frame.render_widget(sidebar_block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::from(vec![
        Span::styled(
            " nixmate",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.fg_dim),
        ),
    ]));
    lines.push(Line::raw(""));

    // Main modules (numbered 1-9)
    for &module in SIDEBAR_MODULES {
        render_sidebar_item(&mut lines, app, module, theme);
    }

    // Separator
    lines.push(Line::raw(""));
    let sep_width = inner.width.saturating_sub(2) as usize;
    lines.push(Line::styled(
        format!(" {}", "─".repeat(sep_width.min(20))),
        Style::default().fg(theme.border),
    ));

    // Bottom items (Settings, Help)
    for &module in SIDEBAR_BOTTOM {
        render_sidebar_item(&mut lines, app, module, theme);
    }

    frame.render_widget(Paragraph::new(lines).style(theme.block_style()), area);
}

/// Render a single sidebar item
fn render_sidebar_item<'a>(
    lines: &mut Vec<Line<'a>>,
    app: &App,
    module: ModuleTab,
    theme: &crate::ui::Theme,
) {
    let is_active = app.active_tab == module;
    let hint = module.key_hint();

    if is_active {
        lines.push(Line::from(vec![
            Span::styled(" ▸ ", Style::default().fg(theme.accent)),
            Span::styled(hint.to_string(), Style::default().fg(theme.accent)),
            Span::styled(
                format!(" {}", module.label(app)),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(hint.to_string(), Style::default().fg(theme.fg_dim)),
            Span::styled(
                format!(" {}", module.label(app)),
                Style::default().fg(theme.fg),
            ),
        ]));
    }
}
/// Render the active module's content
fn render_module_content(frame: &mut Frame, app: &mut App, area: Rect) {
    // Show module intro page on first visit
    if app.is_intro_showing() {
        render_module_intro(frame, app, area);
        return;
    }

    match app.active_tab {
        ModuleTab::Generations => {
            crate::modules::generations::render(
                frame,
                &app.generations,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Errors => {
            crate::modules::errors::render(
                frame,
                &app.errors,
                &app.theme,
                app.config.language,
                area,
                app.config.ai_available(),
            );
        }
        ModuleTab::Services => {
            crate::modules::services::render(
                frame,
                &mut app.services,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Storage => {
            crate::modules::storage::render(
                frame,
                &mut app.storage,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Config => {
            crate::modules::config_showcase::render(
                frame,
                &app.config_showcase,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Options => {
            crate::modules::options::render(
                frame,
                &app.options,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Rebuild => {
            crate::modules::rebuild::render(
                frame,
                &app.rebuild,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::FlakeInputs => {
            crate::modules::flake_inputs::render(
                frame,
                &app.flake_inputs,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Packages => {
            crate::modules::packages::render(
                frame,
                &app.packages,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Health => {
            crate::modules::health::render(
                frame,
                &app.health,
                &app.theme,
                app.config.language,
                area,
            );
        }
        ModuleTab::Settings => render_settings(frame, app, area),
        ModuleTab::HelpAbout => render_help_about(frame, app, area),
    }
}

/// Module intro content
struct IntroContent {
    emoji: &'static str,
    subtitle: &'static str,
    problem: &'static str,
    features: &'static [&'static str],
    tabs: &'static [&'static str],
}

fn get_intro_content(tab: ModuleTab, lang: Language) -> Option<IntroContent> {
    match (tab, lang) {
        // ── Generations ──
        (ModuleTab::Generations, Language::English) => Some(IntroContent {
            emoji: "🕰️",
            subtitle: "Browse, compare, and manage your NixOS generations",
            problem: "Comparing generations means juggling shell commands you never remember. Which packages changed? What kernel was running on that old generation? There's no single place to get a clear picture.",
            features: &[
                "All generations at a glance — size, date, kernel, package count",
                "Side-by-side diff of added, removed, and updated packages",
                "Delete, pin, and restore with 5-second undo safety net",
                "Kernel update and security package highlighting",
            ],
            tabs: &[
                "Overview  — all generations at a glance",
                "Packages  — searchable package list per generation",
                "Diff      — compare any two generations side-by-side",
                "Manage    — delete, pin, restore with undo",
            ],
        }),
        (ModuleTab::Generations, Language::German) => Some(IntroContent {
            emoji: "🕰️",
            subtitle: "NixOS-Generationen anzeigen, vergleichen und verwalten",
            problem: "Generationen vergleichen heißt Shell-Kommandos nachschlagen, die man nie auswendig kann. Welche Pakete haben sich geändert? Welcher Kernel lief auf der alten Generation?",
            features: &[
                "Alle Generationen auf einen Blick — Größe, Datum, Kernel, Paketanzahl",
                "Seite-an-Seite Diff von hinzugefügten, entfernten und aktualisierten Paketen",
                "Löschen, Pinnen und Wiederherstellen mit 5-Sekunden-Undo",
                "Kernel-Update und Sicherheitspaket-Hervorhebung",
            ],
            tabs: &[
                "Übersicht  — alle Generationen auf einen Blick",
                "Pakete     — durchsuchbare Paketliste pro Generation",
                "Diff       — zwei Generationen vergleichen",
                "Verwalten  — löschen, pinnen, wiederherstellen",
            ],
        }),
        // ── Error Translator ──
        (ModuleTab::Errors, Language::English) => Some(IntroContent {
            emoji: "🔍",
            subtitle: "Translate cryptic Nix errors into plain language",
            problem: "Cryptic Nix errors send you on a 30-minute search. You know something broke, but the error message tells you nothing useful about how to fix it.",
            features: &[
                "50+ built-in error patterns with human-readable explanations",
                "Deep-dive explanations of why errors happen, not just what to type",
                "AI fallback via Claude, OpenAI, or local Ollama for unknown errors",
                "Pipe support: nixos-rebuild switch 2>&1 | nixmate",
            ],
            tabs: &[
                "Translator  — paste an error, get a clear fix",
                "Submit      — submit new error patterns to help others",
            ],
        }),
        (ModuleTab::Errors, Language::German) => Some(IntroContent {
            emoji: "🔍",
            subtitle: "Kryptische Nix-Fehler verständlich übersetzen",
            problem: "Kryptische Nix-Fehlermeldungen schicken dich auf 30-minütige Recherche. Du weißt, dass etwas kaputt ist, aber die Fehlermeldung hilft dir nicht weiter.",
            features: &[
                "50+ eingebaute Fehlermuster mit verständlichen Erklärungen",
                "Tiefgehende Erklärungen warum Fehler auftreten, nicht nur was man tippen soll",
                "KI-Fallback über Claude, OpenAI oder lokales Ollama",
                "Pipe-Support: nixos-rebuild switch 2>&1 | nixmate",
            ],
            tabs: &[
                "Übersetzer  — Fehler einfügen, klare Lösung erhalten",
                "Einreichen  — neue Fehlermuster für andere einreichen",
            ],
        }),
        // ── Services & Ports ──
        (ModuleTab::Services, Language::English) => Some(IntroContent {
            emoji: "🖥️",
            subtitle: "Unified server dashboard for services and ports",
            problem: "Checking services means juggling systemctl, docker ps, podman ps, ss -tlnp, and journalctl — five different tools with five different interfaces.",
            features: &[
                "Systemd, Docker, and Podman containers in one unified view",
                "Automatic port-to-service mapping via PID + process name",
                "Start, stop, restart, enable, and disable with sudo confirmation",
                "Color-coded live logs from journalctl and container logs",
            ],
            tabs: &[
                "Overview  — all services and containers with status",
                "Ports     — every listening TCP/UDP port mapped to its service",
                "Manage    — start, stop, restart, enable, disable",
                "Logs      — live color-coded log viewer",
            ],
        }),
        (ModuleTab::Services, Language::German) => Some(IntroContent {
            emoji: "🖥️",
            subtitle: "Einheitliches Server-Dashboard für Dienste und Ports",
            problem: "Dienste prüfen heißt jonglieren mit systemctl, docker ps, podman ps, ss -tlnp und journalctl — fünf verschiedene Tools mit verschiedenen Interfaces.",
            features: &[
                "Systemd, Docker und Podman in einer einheitlichen Ansicht",
                "Automatische Port-zu-Dienst-Zuordnung via PID + Prozessname",
                "Start, Stop, Neustart, Aktivieren und Deaktivieren mit Sudo-Bestätigung",
                "Farbcodierte Live-Logs von journalctl und Container-Logs",
            ],
            tabs: &[
                "Übersicht  — alle Dienste und Container mit Status",
                "Ports      — alle TCP/UDP-Ports mit zugeordnetem Dienst",
                "Verwalten  — starten, stoppen, neustarten",
                "Logs       — farbcodierter Live-Log-Viewer",
            ],
        }),
        // ── Storage ──
        (ModuleTab::Storage, Language::English) => Some(IntroContent {
            emoji: "💾",
            subtitle: "Understand what's eating your disk and clean it up",
            problem: "Your /nix/store keeps growing and nix-collect-garbage is a blunt tool. You want to understand what's eating your disk before you clean it up.",
            features: &[
                "Disk usage dashboard with color-coded progress bars",
                "Nix store breakdown: live/dead paths with sizes and percentages",
                "Top 10 largest store paths with visual size bars",
                "Garbage collect, store optimize, and full clean with dry-run preview",
            ],
            tabs: &[
                "Dashboard  — disk usage overview and recommendations",
                "Explorer   — browse all store paths sorted by size",
                "Clean      — garbage collect, optimize, full clean",
                "History    — persistent log of all cleanup actions",
            ],
        }),
        (ModuleTab::Storage, Language::German) => Some(IntroContent {
            emoji: "💾",
            subtitle: "Verstehen was den Speicher frisst und aufräumen",
            problem: "Dein /nix/store wächst ständig und nix-collect-garbage ist ein stumpfes Werkzeug. Du willst verstehen, was den Speicher frisst, bevor du aufräumst.",
            features: &[
                "Speicherplatz-Dashboard mit farbcodierten Fortschrittsbalken",
                "Nix Store Aufschlüsselung: aktive/tote Pfade mit Größen",
                "Top 10 größte Store-Pfade mit visuellen Balken",
                "Garbage Collection, Store-Optimierung und Vollreinigung mit Vorschau",
            ],
            tabs: &[
                "Dashboard  — Speicherplatz-Übersicht und Empfehlungen",
                "Explorer   — Store-Pfade nach Größe durchsuchen",
                "Aufräumen  — GC, Optimierung, Vollreinigung",
                "Verlauf    — Protokoll aller Bereinigungen",
            ],
        }),
        // ── Config Showcase ──
        (ModuleTab::Config, Language::English) => Some(IntroContent {
            emoji: "🎨",
            subtitle: "Generate a beautiful poster of your NixOS setup",
            problem: "You've built an amazing NixOS configuration, but there's no easy way to show it off. Screenshots only capture part of the picture.",
            features: &[
                "Auto-detect your entire system: hardware, services, packages, network",
                "Beautiful dark-themed SVG poster with gradient and glow effects",
                "Config architecture diagram showing all .nix files and relationships",
                "Designed for r/unixporn, GitHub READMEs, and flex posts",
            ],
            tabs: &[
                "System Overview  — generate your system poster (SVG)",
                "Config Diagram   — architecture visualization of your config",
            ],
        }),
        (ModuleTab::Config, Language::German) => Some(IntroContent {
            emoji: "🎨",
            subtitle: "Ein schönes Poster deines NixOS-Setups generieren",
            problem: "Du hast eine tolle NixOS-Konfiguration gebaut, aber es gibt keinen einfachen Weg sie zu zeigen. Screenshots fangen nur einen Teil ein.",
            features: &[
                "Automatische Erkennung: Hardware, Dienste, Pakete, Netzwerk",
                "Schöner dunkler SVG-Poster mit Gradient- und Glow-Effekten",
                "Config-Architekturdiagramm mit allen .nix-Dateien und Beziehungen",
                "Designed für r/unixporn, GitHub READMEs und Flex-Posts",
            ],
            tabs: &[
                "Systemübersicht   — System-Poster generieren (SVG)",
                "Config-Diagramm   — Architektur-Visualisierung der Config",
            ],
        }),
        // ── Options Explorer ──
        (ModuleTab::Options, Language::English) => Some(IntroContent {
            emoji: "🔧",
            subtitle: "Search, browse, and discover all 20,000+ NixOS options",
            problem: "NixOS has 20,000+ options but finding the right ones means jumping between browser, wiki, and terminal. Over SSH? Even worse. And no tool shows your current values next to the defaults.",
            features: &[
                "Fuzzy search across all NixOS options — instant results",
                "Tree-based browsing: discover what's available without guessing names",
                "Your current values highlighted vs. defaults — no other tool does this",
                "Related sibling options: see everything you can configure for a service",
            ],
            tabs: &[
                "Search   — fuzzy search with detail view and current values",
                "Browse   — tree navigation through the option hierarchy",
                "Related  — sibling options for the selected option",
            ],
        }),
        (ModuleTab::Options, Language::German) => Some(IntroContent {
            emoji: "🔧",
            subtitle: "Alle 20.000+ NixOS-Optionen suchen, durchstöbern und entdecken",
            problem: "NixOS hat 20.000+ Optionen, aber die richtigen zu finden heißt zwischen Browser, Wiki und Terminal zu springen. Über SSH? Noch schlimmer. Und kein Tool zeigt deine aktuellen Werte neben den Standards.",
            features: &[
                "Fuzzy-Suche über alle NixOS-Optionen — sofortige Ergebnisse",
                "Baumbasiertes Durchstöbern: entdecken was möglich ist, ohne Namen zu raten",
                "Deine aktuellen Werte hervorgehoben vs. Standards — kein anderes Tool kann das",
                "Verwandte Optionen: alles sehen was du für einen Service konfigurieren kannst",
            ],
            tabs: &[
                "Suche        — Fuzzy-Suche mit Detailansicht und aktuellen Werten",
                "Durchsuchen  — Baumnavigation durch die Options-Hierarchie",
                "Verwandte    — Schwester-Optionen der ausgewählten Option",
            ],
        }),
        // ── Rebuild ──
        (ModuleTab::Rebuild, Language::English) => Some(IntroContent {
            emoji: "⚡",
            subtitle: "Live rebuild dashboard with progress tracking",
            problem: "nixos-rebuild is a black box — you run it and wait, with no idea what's happening, how long it will take, or what changed when it's done.",
            features: &[
                "5-phase progress dashboard with real-time status and per-phase timers",
                "Educational explanations: learn what NixOS does at each build step",
                "Intelligent log beautification: store paths become human-readable names",
                "Post-rebuild diff: packages added/removed, kernel changes, service restarts",
            ],
            tabs: &[
                "Dashboard  — 5-phase progress with live status",
                "Build Log  — full searchable build output",
                "Changes    — post-rebuild package and service diff",
                "History    — persistent log of all rebuilds",
            ],
        }),
        (ModuleTab::Rebuild, Language::German) => Some(IntroContent {
            emoji: "⚡",
            subtitle: "Live-Rebuild-Dashboard mit Fortschrittsverfolgung",
            problem: "nixos-rebuild ist eine Blackbox — du startest es und wartest, ohne zu wissen was passiert, wie lange es dauert, oder was sich geändert hat.",
            features: &[
                "5-Phasen-Fortschrittsdashboard mit Echtzeit-Status und Phasen-Timern",
                "Lehrreiche Erklärungen: lerne was NixOS bei jedem Build-Schritt macht",
                "Intelligente Log-Verschönerung: Store-Pfade werden zu lesbaren Namen",
                "Nachher-Diff: Pakete hinzugefügt/entfernt, Kernel-Änderungen, Dienst-Neustarts",
            ],
            tabs: &[
                "Dashboard   — 5-Phasen-Fortschritt mit Live-Status",
                "Build-Log   — vollständige durchsuchbare Build-Ausgabe",
                "Änderungen  — Paket- und Dienst-Diff nach Rebuild",
                "Verlauf     — Protokoll aller Rebuilds",
            ],
        }),
        // ── Flake Inputs ──
        (ModuleTab::FlakeInputs, Language::English) => Some(IntroContent {
            emoji: "📦",
            subtitle: "Manage your flake inputs individually — no more all-or-nothing updates",
            problem: "nix flake update is all-or-nothing. You can't update just nixpkgs without dragging home-manager and everything else along. Something breaks? Good luck figuring out which input caused it. And there's no place to see all your inputs at a glance with their ages and revisions.",
            features: &[
                "All inputs at a glance: name, URL, revision, age with color coding",
                "Selective per-input updates with checkboxes — update only what you want",
                "Confirmation popup before any update, live progress during update",
                "Full detail view: type, branch, revision, NAR hash, follows relationships",
            ],
            tabs: &[
                "Overview  — all inputs with revision, URL, and age",
                "Update    — select inputs with Space, update with Enter",
                "History   — diff of old → new revisions from last update",
                "Details   — full info for the selected input",
            ],
        }),
        (ModuleTab::FlakeInputs, Language::German) => Some(IntroContent {
            emoji: "📦",
            subtitle: "Flake-Inputs einzeln verwalten — Schluss mit Alles-oder-Nichts-Updates",
            problem: "nix flake update ist Alles-oder-Nichts. Du kannst nicht nur nixpkgs aktualisieren, ohne home-manager und alles andere mitzuziehen. Etwas geht kaputt? Viel Glück herauszufinden welcher Input schuld war. Und es gibt keinen Ort um alle Inputs mit Alter und Revisionen auf einen Blick zu sehen.",
            features: &[
                "Alle Inputs auf einen Blick: Name, URL, Revision, Alter mit Farbcodierung",
                "Selektive Einzel-Input-Updates mit Checkboxen — nur aktualisieren was du willst",
                "Bestätigungs-Popup vor jedem Update, Live-Fortschritt während des Updates",
                "Vollständige Detailansicht: Typ, Branch, Revision, NAR-Hash, Follows-Beziehungen",
            ],
            tabs: &[
                "Übersicht      — alle Inputs mit Revision, URL und Alter",
                "Aktualisieren  — Inputs mit Leertaste auswählen, mit Enter updaten",
                "Verlauf        — Diff von alter → neuer Revision",
                "Details        — vollständige Info zum ausgewählten Input",
            ],
        }),
        // ── Package Search ──
        (ModuleTab::Packages, Language::English) => Some(IntroContent {
            emoji: "📦",
            subtitle: "Search the entire nixpkgs catalog from your terminal",
            problem: "Searching nixpkgs means opening a browser and going to search.nixos.org, or waiting for the painfully slow nix search command. Over SSH, there's no convenient option at all.",
            features: &[
                "Fuzzy search across 100,000+ packages — results sorted by relevance",
                "Auto-detect whether you use Flakes or Channels (configurable in Settings)",
                "Installed packages marked with ✓ and sorted to the top",
                "Detail view with full description, version, and install command",
            ],
            tabs: &[
                "Press / to start searching — type your query and hit Enter",
            ],
        }),
        (ModuleTab::Packages, Language::German) => Some(IntroContent {
            emoji: "📦",
            subtitle: "Den gesamten nixpkgs-Katalog im Terminal durchsuchen",
            problem: "nixpkgs durchsuchen heißt Browser öffnen und zu search.nixos.org gehen, oder auf den langsamen nix search Befehl warten. Über SSH gibt es keine bequeme Option.",
            features: &[
                "Fuzzy-Suche über 100.000+ Pakete — Ergebnisse nach Relevanz sortiert",
                "Automatische Erkennung von Flakes vs. Channels (konfigurierbar)",
                "Installierte Pakete mit ✓ markiert und nach oben sortiert",
                "Detailansicht mit Beschreibung, Version und Installationsbefehl",
            ],
            tabs: &[
                "/ drücken zum Suchen — Suchbegriff eingeben und Enter drücken",
            ],
        }),
        // ── Nix Doctor / Health ──
        (ModuleTab::Health, Language::English) => Some(IntroContent {
            emoji: "🩺",
            subtitle: "System health dashboard — know if your NixOS needs attention",
            problem: "Is your NixOS healthy? Are old generations piling up? Is the store bloated? When did you last update? There's no quick way to check overall system health.",
            features: &[
                "Health score 0-100% with color coding (green/orange/red)",
                "5 automated checks: old generations, store size, disk, updates, duplicates",
                "One-click fixes: garbage collect, channel update, and more",
                "Auto-scan on entry, rescan anytime with r",
            ],
            tabs: &[
                "Dashboard  — health score and check overview",
                "Fix        — select an issue and apply the fix",
            ],
        }),
        (ModuleTab::Health, Language::German) => Some(IntroContent {
            emoji: "🩺",
            subtitle: "System-Health-Dashboard — wissen ob dein NixOS Aufmerksamkeit braucht",
            problem: "Ist dein NixOS gesund? Stapeln sich alte Generationen? Ist der Store aufgebläht? Wann war das letzte Update? Kein schneller Weg um den Systemzustand zu prüfen.",
            features: &[
                "Gesundheitsscore 0-100% mit Farbcodierung (grün/orange/rot)",
                "5 automatische Checks: alte Generationen, Store, Speicher, Updates, Duplikate",
                "Ein-Klick-Reparaturen: Garbage Collection, Channel-Update und mehr",
                "Auto-Scan bei Aufruf, erneuter Scan jederzeit mit r",
            ],
            tabs: &[
                "Dashboard   — Gesundheitsscore und Check-Übersicht",
                "Reparieren  — Problem auswählen und Fix anwenden",
            ],
        }),
        _ => None,
    }
}

/// Render a module intro/start page
fn render_module_intro(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let lang = app.config.language;
    let tab = app.active_tab;

    let intro = match get_intro_content(tab, lang) {
        Some(c) => c,
        None => return,
    };

    let title = tab.label(app);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} {} ", intro.emoji, title))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 6 || inner.width < 30 {
        return;
    }

    let wrap_width = (inner.width as usize).saturating_sub(6).max(20);
    let mut lines: Vec<Line> = Vec::new();

    // Subtitle
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!("  {}", intro.subtitle),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));

    // ── The Problem ──
    lines.push(Line::raw(""));
    let problem_label = match lang {
        Language::English => "── The Problem ──",
        Language::German => "── Das Problem ──",
    };
    lines.push(Line::styled(
        format!("  {}", problem_label),
        Style::default()
            .fg(theme.fg_dim)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));
    for wrapped in word_wrap_intro(intro.problem, wrap_width) {
        lines.push(Line::styled(format!("  {}", wrapped), theme.text()));
    }

    // ── Features ──
    lines.push(Line::raw(""));
    let features_label = match lang {
        Language::English => "── Features ──",
        Language::German => "── Funktionen ──",
    };
    lines.push(Line::styled(
        format!("  {}", features_label),
        Style::default()
            .fg(theme.fg_dim)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));
    for feature in intro.features {
        lines.push(Line::from(vec![
            Span::styled("  ✦ ", Style::default().fg(theme.accent)),
            Span::styled(feature.to_string(), theme.text()),
        ]));
    }

    // ── Tabs ──
    lines.push(Line::raw(""));
    let tabs_label = match lang {
        Language::English => "── Tabs ──",
        Language::German => "── Tabs ──",
    };
    lines.push(Line::styled(
        format!("  {}", tabs_label),
        Style::default()
            .fg(theme.fg_dim)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(""));
    for tab_desc in intro.tabs {
        lines.push(Line::styled(
            format!("  {}", tab_desc),
            Style::default().fg(theme.accent_dim),
        ));
    }

    // ── Press Enter to start ──
    lines.push(Line::raw(""));
    lines.push(Line::raw(""));
    let continue_text = match lang {
        Language::English => "─── Press Enter to start →",
        Language::German => "─── Enter drücken zum Starten →",
    };
    lines.push(Line::styled(
        format!("  {}", continue_text),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));

    let paragraph = Paragraph::new(lines)
        .style(theme.block_style())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Word wrap for intro text
fn word_wrap_intro(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut current_line = String::new();
    for word in words {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines
}

/// Render the Help / About tab (with mascot)
fn render_help_about(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let s = i18n::get_strings(app.config.language);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_help))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut content: Vec<Line> = Vec::new();

    // Mascot image via terminal protocol (same approach as welcome screen)
    let img_area =
        crate::modules::splash::help_image_area(inner, app.image_protocol.is_supported());
    if let Some((_col, _row, _cols, rows)) = img_area {
        for _ in 0..rows {
            content.push(Line::raw(""));
        }
        app.image_area = img_area;
    }

    // Title bar
    content.push(Line::raw(""));
    content.push(Line::from(vec![
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
    content.push(Line::styled(
        s.help_subtitle,
        Style::default().fg(theme.fg_dim),
    ));
    content.push(Line::raw(""));

    // Module table header
    content.push(Line::styled(
        format!("── {} ──", s.help_modules_title),
        Style::default().fg(theme.accent),
    ));
    content.push(Line::raw(""));

    // Module rows: clean aligned format  [key]  Name ··· Description
    let modules: Vec<(&str, &str, &str)> = vec![
        ("1", s.tab_generations, s.help_mod_gen),
        ("2", s.tab_errors, s.help_mod_err),
        ("3", s.tab_services, s.help_mod_svc),
        ("4", s.tab_storage, s.help_mod_gc),
        ("5", s.tab_config, s.help_mod_cfg),
        ("6", s.tab_options, s.help_mod_opt),
        ("7", s.tab_rebuild, s.help_mod_rebuild),
        ("8", s.tab_flake_inputs, s.help_mod_flake),
        ("9", s.tab_packages, s.help_mod_pkg),
        ("0", s.tab_health, s.help_mod_health),
        (",", s.tab_settings, s.help_mod_set),
    ];

    for (key, name, desc) in modules {
        // Pad name to 20 chars for alignment
        let padded_name = format!("{:<18}", name);
        content.push(Line::from(vec![
            Span::styled(format!("  [{}]  ", key), Style::default().fg(theme.accent)),
            Span::styled(
                padded_name,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc.to_string(), Style::default().fg(theme.fg_dim)),
        ]));
    }
    content.push(Line::raw(""));

    // Contribute section
    content.push(Line::styled(
        format!("── {} ──", s.help_contribute_title),
        Style::default().fg(theme.accent),
    ));
    content.push(Line::raw(""));
    content.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(s.help_contribute, Style::default().fg(theme.fg)),
    ]));
    content.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "github.com/daskladas/nixmate",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::UNDERLINED),
        ),
    ]));
    content.push(Line::raw(""));
    content.push(Line::styled(
        s.help_thanks,
        Style::default().fg(theme.success),
    ));

    frame.render_widget(Paragraph::new(content).alignment(Alignment::Center), inner);
}

/// Render the global Settings tab
fn render_settings(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let s = i18n::get_strings(app.config.language);

    let block = Block::default()
        .style(theme.block_style())
        .title(format!(" {} ", s.tab_settings))
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focused());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Global settings
    let global_settings: Vec<(&str, String)> = vec![
        (s.settings_theme, app.config.theme.as_str().to_string()),
        (
            s.settings_language,
            app.config.language.as_str().to_string(),
        ),
        (
            s.settings_layout,
            app.config.layout.as_str(app.config.language).to_string(),
        ),
        (
            s.settings_nixpkgs,
            if app.settings_editing && app.settings_selected == 3 {
                format!("{}_", app.settings_edit_buffer)
            } else {
                app.config.nixpkgs_channel.clone()
            },
        ),
    ];

    // Error Translator / AI settings
    let err_settings: Vec<(&str, String, bool)> = vec![
        (
            s.settings_ai_enabled,
            if app.config.ai_enabled {
                s.settings_enabled
            } else {
                s.settings_disabled
            }
            .to_string(),
            false,
        ),
        (
            s.settings_ai_provider,
            app.config.ai_provider.clone(),
            false,
        ),
        (
            s.settings_ai_key,
            if app.settings_editing && app.settings_selected == 6 {
                format!("{}_", app.settings_edit_buffer)
            } else if app.config.ai_api_key.is_some() {
                "••••••••".to_string()
            } else {
                s.settings_not_set.to_string()
            },
            app.settings_editing && app.settings_selected == 6,
        ),
        (
            s.settings_ollama_url,
            if app.settings_editing && app.settings_selected == 7 {
                format!("{}_", app.settings_edit_buffer)
            } else {
                app.config
                    .ollama_url
                    .clone()
                    .unwrap_or_else(|| s.settings_not_set.to_string())
            },
            app.settings_editing && app.settings_selected == 7,
        ),
        (
            s.settings_ollama_model,
            if app.settings_editing && app.settings_selected == 8 {
                format!("{}_", app.settings_edit_buffer)
            } else {
                app.config
                    .ollama_model
                    .clone()
                    .unwrap_or_else(|| s.settings_not_set.to_string())
            },
            app.settings_editing && app.settings_selected == 8,
        ),
        (
            s.settings_github_token,
            if app.settings_editing && app.settings_selected == 9 {
                format!("{}_", app.settings_edit_buffer)
            } else if app.config.has_github() {
                "••••••••".to_string()
            } else {
                s.settings_not_set.to_string()
            },
            app.settings_editing && app.settings_selected == 9,
        ),
    ];

    let mut items: Vec<ListItem> = Vec::new();

    // Global settings items
    for (i, (label, value)) in global_settings.iter().enumerate() {
        let style = if i == app.settings_selected {
            theme.selected()
        } else {
            theme.text()
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("  {:<24}", label), style),
            Span::styled(format!("[{}]", value), Style::default().fg(theme.accent)),
        ])));
    }

    // Section separator
    let separator_line = format!("  ── {} ──", s.settings_err_section);
    items.push(ListItem::new(Line::styled(
        separator_line,
        theme.text_dim(),
    )));

    // Error Translator / AI settings items
    for (i, (label, value, editing)) in err_settings.iter().enumerate() {
        let global_idx = i + 4; // offset by 4 global settings
        let style = if global_idx == app.settings_selected {
            theme.selected()
        } else {
            theme.text()
        };

        let value_style = if *editing {
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.accent)
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("  {:<24}", label), style),
            Span::styled(format!("[{}]", value), value_style),
        ])));
    }

    // Editing hint
    if app.settings_editing {
        items.push(ListItem::new(Line::raw("")));
        items.push(ListItem::new(Line::styled(
            format!("  💡 {}", s.settings_editing_hint),
            theme.text_dim(),
        )));
    }

    let list = List::new(items);
    frame.render_widget(list, inner);

    // Config path at bottom
    let config_path = crate::config::Config::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "Unknown".into());

    let path_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 1,
    };
    let path_widget = Paragraph::new(format!("{}: {}", s.settings_config_path, config_path))
        .style(theme.text_dim());
    frame.render_widget(path_widget, path_area);
}

/// Render status bar with context-sensitive keybindings
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let s = i18n::get_strings(app.config.language);

    let hints = match app.active_tab {
        ModuleTab::Generations => {
            let gen_state = &app.generations;
            match gen_state.active_sub_tab {
                crate::modules::generations::GenSubTab::Overview => {
                    format!(
                        "[j/k] {}  [Tab] Panel  [Enter] Pkgs  [/] Sub-Tab  {}",
                        s.navigate, s.status_quit
                    )
                }
                crate::modules::generations::GenSubTab::Packages => {
                    format!(
                        "[j/k] {}  [/] Filter  [Esc] Back  {}",
                        s.navigate, s.status_quit
                    )
                }
                crate::modules::generations::GenSubTab::Diff => {
                    format!(
                        "[Tab] List  [j/k] {}  [Enter] {}  [c] Clear  {}",
                        s.navigate, s.select, s.status_quit
                    )
                }
                crate::modules::generations::GenSubTab::Manage => {
                    format!(
                        "[Space] {}  [R] Restore  [D] Delete  [P] Pin  {}",
                        s.select, s.status_quit
                    )
                }
            }
        }
        ModuleTab::Settings => {
            if app.settings_editing {
                format!("{}  {}", s.settings_editing_hint, s.status_quit)
            } else {
                format!(
                    "{}  {}  {}",
                    s.status_navigate, s.status_change, s.status_quit
                )
            }
        }
        ModuleTab::Errors => {
            let err_state = &app.errors;
            match err_state.active_sub_tab {
                crate::modules::errors::ErrSubTab::Analyze => {
                    if err_state.input_mode {
                        format!(
                            "[Enter] {}  [Esc] {}  [/] Sub-Tab  {}",
                            s.confirm, s.back, s.status_quit
                        )
                    } else if err_state.ai_loading {
                        format!(
                            "🔄 {}  [Esc] {}  {}",
                            s.err_ai_analyzing, s.cancel, s.status_quit
                        )
                    } else if err_state.ai_result.is_some() {
                        format!(
                            "[j/k] Scroll  [n] {}  {}",
                            s.err_new_analysis, s.status_quit
                        )
                    } else if err_state.result.is_some() {
                        format!(
                            "[j/k] Scroll  [n] {}  [s] Submit  [/] Sub-Tab  {}",
                            s.err_new_analysis, s.status_quit
                        )
                    } else if !err_state.input_buffer.is_empty() && app.config.ai_available() {
                        format!(
                            "[a] 🤖 {}  [n] {}  [s] {}  {}",
                            s.err_ai_ask, s.err_new_analysis, s.err_submit_pattern, s.status_quit
                        )
                    } else {
                        format!(
                            "[i] {}  [s] {}  [/] Sub-Tab  {}",
                            s.err_start_input, s.err_submit_pattern, s.status_quit
                        )
                    }
                }
                crate::modules::errors::ErrSubTab::Submit => {
                    format!(
                        "[Tab] Next  [Enter] Submit  [Esc] {}  {}",
                        s.back, s.status_quit
                    )
                }
            }
        }
        ModuleTab::Services => {
            let svc_state = &app.services;
            if svc_state.loading && !svc_state.loaded {
                format!("Loading services...  {}", s.status_quit)
            } else {
                match svc_state.active_sub_tab {
                    crate::modules::services::SvcSubTab::Overview => {
                        if svc_state.search_active {
                            format!("[Enter] {}  [Esc] {}  {}", s.confirm, s.back, s.status_quit)
                        } else {
                            format!(
                            "[j/k] {}  [/] Search  [f] Filter  [r] Refresh  [Enter] Logs  [m] Manage  [/] Sub-Tab  {}",
                            s.navigate, s.status_quit
                        )
                        }
                    }
                    crate::modules::services::SvcSubTab::Ports => {
                        format!(
                            "[j/k] {}  [r] Refresh  [/] Sub-Tab  {}",
                            s.navigate, s.status_quit
                        )
                    }
                    crate::modules::services::SvcSubTab::Manage => {
                        format!(
                            "[j/k] {}  [Enter] Execute  [/] Sub-Tab  {}",
                            s.navigate, s.status_quit
                        )
                    }
                    crate::modules::services::SvcSubTab::Logs => {
                        format!(
                            "[j/k] Scroll  [r] Refresh  [g/G] Top/End  [/] Sub-Tab  {}",
                            s.status_quit
                        )
                    }
                }
            }
        }
        ModuleTab::Storage => {
            let sto_state = &app.storage;
            if sto_state.loading && !sto_state.loaded {
                format!("Loading store data...  {}", s.status_quit)
            } else {
                match sto_state.active_sub_tab {
                    crate::modules::storage::StoSubTab::Dashboard => {
                        format!("[r] Refresh  [/] Sub-Tab  {}", s.status_quit)
                    }
                    crate::modules::storage::StoSubTab::Explorer => {
                        if sto_state.explorer_search_active {
                            format!("[Enter] {}  [Esc] {}  {}", s.confirm, s.back, s.status_quit)
                        } else {
                            format!(
                                "[j/k] {}  [/] Search  [f] Filter  [r] Refresh  [/] Sub-Tab  {}",
                                s.navigate, s.status_quit
                            )
                        }
                    }
                    crate::modules::storage::StoSubTab::Clean => {
                        format!(
                            "[j/k] {}  [Enter] Execute  [/] Sub-Tab  {}",
                            s.navigate, s.status_quit
                        )
                    }
                    crate::modules::storage::StoSubTab::History => {
                        format!("[j/k] Scroll  [r] Refresh  [/] Sub-Tab  {}", s.status_quit)
                    }
                }
            }
        }
        ModuleTab::Config => {
            let is_scanning = match app.config_showcase.active_sub_tab {
                crate::modules::config_showcase::CfgSubTab::Overview => {
                    app.config_showcase.scanning
                }
                crate::modules::config_showcase::CfgSubTab::Diagram => {
                    app.config_showcase.diagram_scanning
                }
            };
            let generate_label = match app.config_showcase.active_sub_tab {
                crate::modules::config_showcase::CfgSubTab::Overview => s.cfg_generate,
                crate::modules::config_showcase::CfgSubTab::Diagram => s.cfg_diag_generate,
            };
            let scanning_label = match app.config_showcase.active_sub_tab {
                crate::modules::config_showcase::CfgSubTab::Overview => s.cfg_scanning,
                crate::modules::config_showcase::CfgSubTab::Diagram => s.cfg_diag_scanning,
            };
            if is_scanning {
                format!("⏳ {}  [/] Sub-Tab  {}", scanning_label, s.status_quit)
            } else {
                format!(
                    "[Enter/g] {}  [/] Sub-Tab  {}",
                    generate_label, s.status_quit
                )
            }
        }
        ModuleTab::Packages => {
            let pkg = &app.packages;
            if pkg.search_active {
                format!("[Enter] {}  [Esc] {}  {}", s.confirm, s.back, s.status_quit)
            } else if pkg.detail_open {
                format!("[Esc/Enter] {}  {}", s.back, s.status_quit)
            } else if !pkg.results.is_empty() {
                format!(
                    "[j/k] {}  [/] Search  [Enter] Details  [n] New  {}",
                    s.navigate, s.status_quit
                )
            } else {
                format!("[/] Search  [n] New  {}", s.status_quit)
            }
        }
        ModuleTab::Health => {
            if app.health.scanning {
                format!("⏳ Scanning...  [/] Sub-Tab  {}", s.status_quit)
            } else if app.health.sub_tab == crate::modules::health::HealthSubTab::Fix {
                format!(
                    "[j/k] {}  [Enter] Fix  [r] Rescan  [/] Sub-Tab  {}",
                    s.navigate, s.status_quit
                )
            } else {
                format!(
                    "[j/k] {}  [r] Rescan  [/] Sub-Tab  {}",
                    s.navigate, s.status_quit
                )
            }
        }
        ModuleTab::Rebuild => {
            let rb = &app.rebuild;
            if rb.is_running() {
                match rb.sub_tab {
                    crate::modules::rebuild::RebuildSubTab::Dashboard
                    | crate::modules::rebuild::RebuildSubTab::Log => {
                        format!("[j/k] Scroll  [G] Live  [/] Sub-Tab  {}", s.status_quit)
                    }
                    _ => {
                        format!("[j/k] Scroll  [/] Sub-Tab  {}", s.status_quit)
                    }
                }
            } else if rb.log_search_active {
                format!("[Enter] {}  [Esc] {}  {}", s.confirm, s.back, s.status_quit)
            } else {
                match rb.sub_tab {
                    crate::modules::rebuild::RebuildSubTab::Dashboard => {
                        format!(
                            "[Enter/r] Rebuild  [m] Mode  [/] Sub-Tab  {}",
                            s.status_quit
                        )
                    }
                    crate::modules::rebuild::RebuildSubTab::Log => {
                        format!(
                            "[j/k] Scroll  [/] Search  [g/G] Top/End  [/] Sub-Tab  {}",
                            s.status_quit
                        )
                    }
                    crate::modules::rebuild::RebuildSubTab::Changes => {
                        format!("[j/k] Scroll  [/] Sub-Tab  {}", s.status_quit)
                    }
                    crate::modules::rebuild::RebuildSubTab::History => {
                        format!("[j/k] {}  [/] Sub-Tab  {}", s.navigate, s.status_quit)
                    }
                }
            }
        }
        ModuleTab::Options => {
            let opt = &app.options;
            if opt.search_active {
                format!("[Enter] {}  [Esc] {}  {}", s.confirm, s.back, s.status_quit)
            } else if opt.detail_open {
                format!(
                    "[j/k] Scroll  [r] Related  [Esc] {}  {}",
                    s.back, s.status_quit
                )
            } else {
                format!(
                    "[j/k] {}  [/] Search  [Enter] Details  [/] Sub-Tab  {}",
                    s.navigate, s.status_quit
                )
            }
        }
        ModuleTab::FlakeInputs => {
            let fi = &app.flake_inputs;
            match fi.sub_tab {
                crate::modules::flake_inputs::FlakeSubTab::Update => {
                    format!(
                        "[j/k] {}  [Space] Select  [u] Update  [/] Sub-Tab  {}",
                        s.navigate, s.status_quit
                    )
                }
                _ => {
                    format!(
                        "[j/k] {}  [Enter] Details  [/] Sub-Tab  {}",
                        s.navigate, s.status_quit
                    )
                }
            }
        }
        _ => {
            format!("{}  {}", s.status_switch_tab, s.status_quit)
        }
    };

    widgets::render_status_bar(frame, &hints, "", theme, area);
}

/// Render popup overlays
fn render_popups(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    match &app.popup {
        PopupState::None => {}
        PopupState::Error { title, message } => {
            widgets::render_error_popup(frame, title, message, theme, area);
        }
        PopupState::Loading { message } => {
            widgets::render_loading(frame, message, theme, area);
        }
    }

    // Flash message
    if let Some(msg) = &app.flash_message {
        widgets::render_flash_message(frame, &msg.text, msg.is_error, &app.theme, area);
    }
}
