# Architecture Overview

This guide explains how nixmate is built so you can understand and modify the codebase — even if you're not a Rust developer.

---

## Project structure

```
src/
├── main.rs              # Entry point, terminal setup, main event loop
├── app.rs               # App state, key routing, timer updates
├── config.rs            # Config struct, theme/language enums, TOML loading
├── i18n.rs              # All UI strings in English + German
├── types.rs             # Shared types
├── modules/
│   ├── mod.rs           # Module registry (one pub mod line per module)
│   ├── splash.rs        # Welcome screen + terminal image display
│   ├── generations.rs   # [1] Generations module
│   ├── errors/          # [2] Error Translator (patterns + AI)
│   ├── services/        # [3] Services & Ports
│   ├── storage/         # [4] Storage
│   ├── config_showcase/ # [5] Config Showcase (poster + diagram)
│   ├── options/         # [6] Options Explorer
│   ├── rebuild/         # [7] Rebuild Dashboard
│   ├── flake_inputs/    # [8] Flake Input Manager
│   ├── packages/        # [9] Package Search
│   └── health/          # [0] Nix Doctor
├── nix/                 # NixOS command wrappers
└── ui/
    ├── mod.rs           # Re-exports
    ├── render.rs        # Main render loop, sidebar, intro pages
    ├── theme.rs         # 13 color themes
    └── widgets.rs       # Reusable UI widgets
```

---

## How the app starts

```
main()
  ├── Check --help / --version
  ├── Check for piped input (stdin)
  ├── Load config from ~/.config/nixmate/config.toml
  ├── Create App struct (all module states initialized)
  ├── Setup terminal (raw mode, alternate screen)
  ├── Install panic handler (restores terminal on crash)
  └── Enter main_loop()
        ├── terminal.draw()     → calls ui::render() → draws everything
        ├── app.handle_image()  → display/clear terminal images
        ├── app.update_timers() → poll background threads, expire flash messages
        ├── event::poll()       → wait up to 100ms for keyboard input
        ├── app.handle_key()    → route the keypress
        └── loop until app.should_quit == true
```

The main loop runs ~10× per second (100ms poll timeout). Every iteration redraws the entire screen and polls all background threads. This is the standard ratatui approach — like a game loop.

---

## State management

All state lives in one `App` struct in `app.rs`:

```
App
├── should_quit: bool           # true → main loop exits
├── active_tab: ModuleTab       # which module is visible
├── config: Config              # theme, language, layout, AI settings
├── theme: Theme                # current color scheme
├── popup: PopupState           # global popup overlay (error/loading/none)
├── flash_message               # temporary 3-second status message
├── intros_dismissed            # which intros were dismissed this session
├── image_protocol / image_cache # terminal image rendering
│
├── welcome: WelcomeState
├── generations: GenerationsState
├── errors: ErrorsState
├── services: ServicesState
├── ... (one state struct per module)
└── flake_inputs: FlakeInputsState
```

There is no global event bus. Each module owns its state. `App` coordinates by calling methods directly.

---

## How keys are routed

```
app.handle_key(key)
  ├── Welcome screen active? → handle welcome keys, return
  ├── Popup showing? → handle popup keys (Esc/Enter), return
  ├── Settings editing text? → handle text input, return
  ├── Intro page showing? → dismiss on Enter, return
  ├── try_module_key(key)
  │     └── Ask the active module if it wants this key
  │         Module captures:  search active, popup open, form active → true
  │         Module ignores:   tab-switch keys (1-9), quit (q) → false
  └── Global keys (not consumed by module):
        'q' → quit, '1'-'0' → switch tab, ',' → settings, '?' → help
```

**Module key capture:** When a search box or popup is active, the module captures ALL keys so typing 'q' doesn't quit the app. Otherwise, navigation keys fall through to global handling.

---

## How rendering works

Every frame, `ui::render()` in `render.rs` runs:

```
ui::render(frame, app)
  ├── Welcome active? → render welcome screen, return
  ├── Draw sidebar (module list)
  ├── Intro showing? → render module intro
  └── Match active_tab → delegate to module::render()
  └── Draw popup overlay (if any)
```

Each module renders itself — it gets a `Rect` (drawing area) and draws whatever it wants inside.

---

## How background threading works

Modules run slow commands without freezing the UI using this pattern:

```
1. Create channel:     let (tx, rx) = mpsc::channel();
2. Spawn thread:       std::thread::spawn(move || { ... tx.send(result) });
3. Store receiver:     self.load_rx = Some(rx);
4. Each frame, poll:   if let Ok(msg) = rx.try_recv() { update state }
5. When done:          self.load_rx = None;
```

```
┌─────────────┐     mpsc::channel      ┌───────────────────┐
│  Main Thread │ ◄── rx (try_recv) ──── │  Background Thread │
│  (UI loop)   │                        │  (nix command)     │
│  poll_load() │     tx.send(data) ──►  │  runs nix-env -qa  │
└─────────────┘                         └───────────────────┘
```

`try_recv()` is non-blocking — it immediately returns `Ok(message)` or `Err(Empty)`.

**Lazy loading:** Most modules call `ensure_loaded()` only when you first visit them. This keeps startup instant.

---

## How i18n works

Every UI string lives in `src/i18n.rs`. One struct, two instances:

```rust
pub struct Strings {
    pub app_title: &'static str,
    pub tab_generations: &'static str,
    // ... hundreds of fields ...
}

static ENGLISH: Strings = Strings { app_title: "nixmate", ... };
static GERMAN: Strings = Strings { app_title: "nixmate", ... };

pub fn get_strings(lang: Language) -> &'static Strings { ... }
```

Every string appears exactly 3 times: struct field + English value + German value.

Code uses: `let s = i18n::get_strings(lang); s.tab_generations`

See [TRANSLATIONS.md](TRANSLATIONS.md) for adding strings.

---

## How config persistence works

Config is `~/.config/nixmate/config.toml`:

```toml
theme = "gruvbox"
language = "english"
layout = "auto"
welcome_shown = true
```

`Config::load()` reads and parses it (or creates defaults). `Config::save()` writes it back. Changes are saved immediately in the Settings tab.

---

## How terminal images work

```
1. Detect terminal: Kitty? iTerm2? Neither?
2. Pre-encode: load icon.png → resize → PNG → base64 (stored in ImageCache)
3. Display: cursor-position + escape sequence + flush stdout
4. Cleanup: delete-all-images command before exiting
```

The image is NOT rendered by ratatui — it's drawn directly via terminal escape sequences on top of blank space that ratatui left empty.

---

## Module pattern summary

Every module has the same shape:

```rust
// State struct
pub struct ModuleState {
    pub items: Vec<Something>,
    pub loaded: bool,
    pub selected: usize,
    load_rx: Option<mpsc::Receiver<...>>,
    pub flash_message: Option<FlashMessage>,
}

// Methods
impl ModuleState {
    pub fn new() -> Self { ... }
    pub fn ensure_loaded(&mut self) { ... }
    pub fn poll_load(&mut self) { ... }
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> { ... }
}

// Standalone render function
pub fn render(frame: &mut Frame, state: &ModuleState, ...) { ... }
```

See [ADDING_MODULES.md](ADDING_MODULES.md) for a full walkthrough.

---

## Tips for reading the code

- **Start with `app.rs`** — it's the hub connecting everything.
- **Each module is self-contained** — reading `packages/mod.rs` doesn't require understanding `rebuild/mod.rs`.
- **`i18n.rs` is just data** — don't read it top-to-bottom.
- **`render.rs` is the router** — it dispatches to module render functions.
- **`nix/` is the system interface** — how nixmate talks to NixOS commands.
