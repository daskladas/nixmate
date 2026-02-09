# Contributing to nixmate

Contributions are welcome! Whether it's a bug fix, new error pattern, theme, or entire module — here's how to get started.

---

## Quick Wins (no Rust knowledge needed)

These contributions don't require understanding Rust:

- **Add error patterns** — teach nixmate to recognize new Nix errors. See [ADDING_PATTERNS.md](ADDING_PATTERNS.md). Just regex + English/German text.
- **Add themes** — pick colors, paste them in. See [ADDING_THEMES.md](ADDING_THEMES.md). Two files, no logic.
- **Add translations** — add German strings for any English-only text. See [TRANSLATIONS.md](TRANSLATIONS.md).
- **Improve docs** — fix typos, add examples, clarify confusing sections.
- **Report bugs** — open an issue with steps to reproduce.
- **Share screenshots** — made a cool Config Showcase poster? Share it!

---

## Development Setup

### Prerequisites

- NixOS (for testing — the modules call NixOS-specific commands)
- Rust toolchain (the flake provides one, or install via `rustup`)

### Clone and build

```bash
git clone https://github.com/daskladas/nixmate.git
cd nixmate

# Option A: Use the flake's dev shell (has Rust + tools)
nix develop

# Option B: Use your own Rust installation
# (needs Rust 1.70+)

# Build
cargo build

# Run
cargo run

# Run with release optimizations (faster, but slower to compile)
cargo build --release
./target/release/nixmate
```

### Project structure

Read [ARCHITECTURE.md](ARCHITECTURE.md) for a full overview. The short version:

```
src/
├── main.rs          # Entry point
├── app.rs           # State + key routing (the hub)
├── config.rs        # Config file handling
├── i18n.rs          # All UI strings (EN + DE)
├── modules/         # One directory per module
├── nix/             # NixOS command wrappers
└── ui/              # Rendering, themes, widgets
```

---

## Workflow

### 1. Fork and branch

```bash
# Fork on GitHub, then:
git clone https://github.com/YOUR_USERNAME/nixmate.git
cd nixmate
git checkout -b my-feature
```

### 2. Make your changes

- Keep commits focused — one feature or fix per commit.
- Test manually: `cargo run` and check that your changes work.
- Run `cargo build` to catch compile errors.

### 3. Commit

```bash
git add -A
git commit -m "Short description of what changed"
```

**Commit message style:**
- `fix: terminal not restored after panic` — bug fix
- `feat: add Solarized Light theme` — new feature
- `docs: improve PIPE_MODE examples` — documentation
- `refactor: simplify key routing in app.rs` — code cleanup
- `i18n: add German strings for Package Search` — translations

### 4. Push and PR

```bash
git push origin my-feature
```

Then open a Pull Request on GitHub. Describe:
- What you changed
- Why
- How to test it

---

## Code Style

nixmate doesn't use `rustfmt` or strict linting (yet). General guidelines:

- **Naming:** `snake_case` for variables/functions, `CamelCase` for types/enums.
- **Comments:** Explain *why*, not *what*. The code shows what — comments should say why it's done that way.
- **Error handling:** Use `anyhow::Result` for functions that can fail. Use `let _ =` for operations where failure is acceptable (like saving config).
- **i18n:** Every user-visible string goes through `i18n.rs`. No hardcoded English strings in module code (except debug/log messages).
- **Timeouts:** Every external command must have a timeout. Nix commands can hang — never let them block the UI.

---

## What makes a good contribution

**Error patterns** — The Error Translator is only as good as its pattern library. If you encounter a Nix error that nixmate doesn't recognize, adding a pattern helps every future user.

**Themes** — Popular terminal color schemes that aren't included yet. Check [catppuccin.com](https://catppuccin.com), [terminal.sexy](https://terminal.sexy), or your favorite editor's theme.

**Bug fixes** — Especially around edge cases: unusual system configs, missing commands, non-standard NixOS setups.

**Module improvements** — Better data, more options, faster loading. Look at the [roadmap](../../CHANGELOG.md) for ideas.

---

## What to avoid

- **Don't add external dependencies lightly.** Every dependency increases build time and attack surface. If something can be done with the standard library, prefer that.
- **Don't break existing keybindings.** Users build muscle memory. Changing keys is a big deal.
- **Don't add mouse-only features.** nixmate is keyboard-first. Mouse support is secondary.
- **Don't hardcode paths.** Always use detection functions or config values.

---

## Testing

There's no full test suite yet (TUI testing is hard). Manual testing checklist:

- [ ] `cargo build` succeeds with no errors
- [ ] No warnings except known ones
- [ ] Your feature works in Kitty and a non-image terminal (like Alacritty)
- [ ] Both languages display correctly (switch to German and check)
- [ ] Quitting with `q` leaves a clean terminal
- [ ] Piping still works: `echo "test" | cargo run`

---

## Getting help

- **Questions about the codebase:** Open an issue tagged "question"
- **Feature ideas:** Open an issue tagged "enhancement"
- **Stuck on something:** Open a draft PR and ask for help in the description

Don't be shy about asking questions. The code has comments and the docs explain the patterns, but some things are clearer with a quick chat.

---

## License

By contributing, you agree that your contributions are licensed under the MIT license (same as the project).
