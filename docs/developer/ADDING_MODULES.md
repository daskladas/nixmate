# Adding a Module to nixmate

This guide walks you through creating a brand new module from scratch. By the end you'll have a working module with sub-tabs, key handling, and bilingual UI — wired into the sidebar and ready to go.

No Rust experience required. If you can copy-paste and change variable names, you're good.

---

## Files you'll touch

| File | What you do there |
|------|-------------------|
| `src/modules/yourmodule/mod.rs` | Your module's state, key handling, and rendering |
| `src/modules/mod.rs` | Register the module (one line) |
| `src/app.rs` | Add state field, lazy loading, key routing, polling |
| `src/ui/render.rs` | Add render dispatch + intro page content |
| `src/i18n.rs` | Add all UI strings (English + German) |

Let's build a "Hello World" module step by step.

---

## Step 1: Create the module file

Create a new directory and file:

```
src/modules/hello/mod.rs
```

Paste this complete starter module:

```rust
//! Hello World module — a minimal example

use crate::config::Language;
use crate::i18n;
use crate::types::FlashMessage;
use crate::ui::theme::Theme;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::time::Instant;

// ── State ──

pub struct HelloState {
    pub counter: usize,
    pub flash_message: Option<FlashMessage>,
}

impl HelloState {
    pub fn new() -> Self {
        Self {
            counter: 0,
            flash_message: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                self.counter += 1;
                self.flash_message = Some(FlashMessage::new(
                    format!("Count: {}", self.counter),
                    false,
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

// ── Render ──

pub fn render(
    frame: &mut Frame,
    state: &HelloState,
    theme: &Theme,
    lang: Language,
    area: Rect,
) {
    let s = i18n::get_strings(lang);

    let lines = vec![
        Line::styled(
            format!("  {} 👋", s.hello_title),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            format!("  {} {}", s.hello_counter, state.counter),
            Style::default().fg(theme.fg),
        ),
        Line::raw(""),
        Line::styled(
            format!("  {}", s.hello_hint),
            Style::default().fg(theme.fg_dim),
        ),
    ];

    frame.render_widget(
        Paragraph::new(lines).style(theme.block_style()),
        area,
    );
}
```

**What's happening here:**
- `HelloState` holds the module's data (just a counter for this example).
- `handle_key()` listens for Enter and increments the counter.
- `render()` draws the UI — a title, the counter, and a hint.
- `i18n::get_strings(lang)` gets the translated strings (we'll add those in Step 5).

---

## Step 2: Register the module

Open `src/modules/mod.rs` and add one line:

```rust
pub mod hello;     // ← add this
```

This tells Rust "there's a module called `hello` in the `modules/hello/` directory."

---

## Step 3: Wire into App state

Open `src/app.rs`. You need to do 4 things:

### 3a: Import it

At the top, add:

```rust
use crate::modules::hello::HelloState;
```

### 3b: Add to the App struct

Find the `pub struct App` definition and add a field:

```rust
pub struct App {
    // ... existing fields ...
    pub hello: HelloState,       // ← add this
}
```

### 3c: Initialize in App::new()

Find the `Ok(Self { ... })` block in `App::new()` and add:

```rust
Ok(Self {
    // ... existing fields ...
    hello: HelloState::new(),    // ← add this
})
```

### 3d: Route keys

Find the `try_module_key()` function. Add a match arm for your tab:

```rust
ModuleTab::Hello => {
    match key.code {
        // Let tab-switch and quit keys through to global handling
        KeyCode::Char('1'..='9') | KeyCode::Char('0')
        | KeyCode::Char(',') | KeyCode::Char('?') | KeyCode::Char('q') => Ok(false),
        // Everything else goes to the module
        _ => {
            self.hello.handle_key(key)?;
            Ok(true)
        }
    }
}
```

### 3e: Poll flash messages (optional)

In `update_timers()`, add flash message expiry:

```rust
if let Some(ref flash) = self.hello.flash_message {
    if flash.is_expired(3) {
        self.hello.flash_message = None;
    }
}
```

---

## Step 4: Add to the sidebar and render

Open `src/ui/render.rs`.

### 4a: Add to the ModuleTab enum

Find `pub enum ModuleTab` and add your variant:

```rust
pub enum ModuleTab {
    Generations,
    Errors,
    // ... existing tabs ...
    Hello,          // ← add this (before Settings)
    Settings,
    HelpAbout,
}
```

Then update the `all()`, `index()`, `label()`, `description()`, and `keybind()` methods to include `Hello`. Follow the pattern of the existing entries — they're all straightforward match arms.

### 4b: Add render dispatch

Find the big `match app.active_tab` block in the `render()` function and add:

```rust
ModuleTab::Hello => {
    crate::modules::hello::render(frame, &app.hello, &app.theme, app.config.language, content_area);
}
```

### 4c: Add intro page content

Find the intro page rendering section and add content for your module. The intro page shows when you first visit the module in a session:

```rust
ModuleTab::Hello => {
    // Title + body for the intro page
    (s.hello_intro_title, s.hello_intro_body)
}
```

---

## Step 5: Add i18n strings

Open `src/i18n.rs`. You need to add strings in 3 places:

### 5a: Add fields to the Strings struct

```rust
pub struct Strings {
    // ... existing fields ...

    // === Hello module ===
    pub hello_title: &'static str,
    pub hello_counter: &'static str,
    pub hello_hint: &'static str,
    pub hello_intro_title: &'static str,
    pub hello_intro_body: &'static str,
}
```

### 5b: Add English values

Find the `static ENGLISH: Strings = Strings { ... }` block and add:

```rust
    hello_title: "Hello World",
    hello_counter: "Counter:",
    hello_hint: "Press Enter to increment",
    hello_intro_title: "Hello World",
    hello_intro_body: "A minimal example module. Press Enter to count.",
```

### 5c: Add German values

Find the `static GERMAN: Strings = Strings { ... }` block and add:

```rust
    hello_title: "Hallo Welt",
    hello_counter: "Zähler:",
    hello_hint: "Enter drücken zum Hochzählen",
    hello_intro_title: "Hallo Welt",
    hello_intro_body: "Ein minimales Beispielmodul. Enter drücken zum Zählen.",
```

---

## Step 6: Assign a keybind

In `app.rs`, find the global key handling section and add a key that switches to your module:

```rust
// Somewhere in handle_key() after the module keys:
KeyCode::Char('h') => self.active_tab = ModuleTab::Hello,
```

> **Note:** For a real module you'd use a number key. Since all 10 number slots (1-9, 0) are taken, you'd need to think about where to put it. For testing, any unused letter works.

---

## Step 7: Build and test

```bash
cargo build
cargo run
```

Press your assigned key to switch to the Hello module. You should see the intro page on first visit (press Enter to dismiss), then the counter UI.

---

## Adding background loading

If your module needs to run a slow command, add lazy loading:

```rust
use std::sync::mpsc;

pub struct HelloState {
    pub data: Vec<String>,
    pub loaded: bool,
    pub loading: bool,
    load_rx: Option<mpsc::Receiver<Vec<String>>>,
    // ... other fields ...
}

impl HelloState {
    pub fn ensure_loaded(&mut self) {
        if self.loaded || self.loading {
            return;
        }
        self.loading = true;

        let (tx, rx) = mpsc::channel();
        self.load_rx = Some(rx);

        std::thread::spawn(move || {
            // Do slow work here (run a command, parse files, etc.)
            let result = vec!["item1".to_string(), "item2".to_string()];
            let _ = tx.send(result);
        });
    }

    pub fn poll_load(&mut self) {
        if let Some(rx) = &self.load_rx {
            match rx.try_recv() {
                Ok(data) => {
                    self.data = data;
                    self.loaded = true;
                    self.loading = false;
                    self.load_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {} // still loading
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.loaded = true;
                    self.load_rx = None;
                }
            }
        }
    }
}
```

Then in `app.rs`:
- Call `self.hello.ensure_loaded()` when the tab is activated.
- Call `self.hello.poll_load()` in `update_timers()`.

---

## Adding sub-tabs

If your module needs multiple views (like F1/F2/F3):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelloSubTab {
    #[default]
    Main,
    Details,
}

pub struct HelloState {
    pub sub_tab: HelloSubTab,
    // ...
}

impl HelloState {
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::F(1) => self.sub_tab = HelloSubTab::Main,
            KeyCode::F(2) => self.sub_tab = HelloSubTab::Details,
            _ => {
                match self.sub_tab {
                    HelloSubTab::Main => { /* handle main tab keys */ }
                    HelloSubTab::Details => { /* handle details tab keys */ }
                }
            }
        }
        Ok(())
    }
}
```

---

## Checklist

Before you're done, make sure:

- [ ] Module file created at `src/modules/yourmodule/mod.rs`
- [ ] Registered in `src/modules/mod.rs`
- [ ] State added to `App` struct in `app.rs`
- [ ] Initialized in `App::new()`
- [ ] Key routing added in `try_module_key()`
- [ ] Flash message expiry in `update_timers()` (if using flash messages)
- [ ] Tab variant added to `ModuleTab` enum in `render.rs`
- [ ] Render dispatch added in the main `render()` function
- [ ] Intro page content added
- [ ] All strings added to `i18n.rs` (struct + English + German)
- [ ] Builds with `cargo build`
- [ ] Tested manually — tab switching, keys, intro page all work
