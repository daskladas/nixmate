# Adding a Theme to nixmate

This guide walks you through creating your own color theme for nixmate. No Rust experience required — you're basically just picking colors.

---

## How themes work

Every visual element in nixmate gets its color from a **Theme struct**. This struct has ~18 color fields like `bg` (background), `fg` (text), `accent` (highlights), `success` (green things), `error` (red things), and so on.

When you create a new theme, you're defining what color each of these fields should be. That's it. nixmate handles the rest.

**Files you'll touch:**

| File | What you do there |
|------|-------------------|
| `src/ui/theme.rs` | Add your color function |
| `src/config.rs` | Register the theme name |

That's only 2 files. Let's go.

---

## Step 1: Pick your colors

You need RGB values for ~18 colors. The easiest approach: find a color scheme you like (from a terminal theme, VS Code theme, or a site like [catppuccin.com](https://catppuccin.com)) and map its colors to nixmate's fields.

Here's what each field controls:

```
bg              → main background (the darkest color)
fg              → normal text
fg_dim          → secondary text, hints, less important info
accent          → highlights, active items, links, the main "pop" color
accent_dim      → softer accent, used for less prominent highlights
success         → green things: healthy status, added packages, checkmarks
warning         → yellow/orange things: warnings, outdated items
error           → red things: errors, failed services, removed packages
border          → box borders (inactive)
border_focused  → box borders (active/selected)
selection_bg    → background of selected/highlighted rows
selection_fg    → text color of selected/highlighted rows
diff_added      → green in diffs (added packages)
diff_removed    → red in diffs (removed packages)
diff_updated    → blue in diffs (changed packages)
current_marker  → the marker for the current generation
pinned_marker   → the marker for pinned generations
boot_marker     → the marker for the boot generation
is_transparent  → true = use terminal's own background, false = use bg color
```

> **Tip:** Most themes use the same color for `success`/`diff_added`, `error`/`diff_removed`, and `accent`/`diff_updated`. That's totally fine.

---

## Step 2: Add your theme function

Open `src/ui/theme.rs` and scroll down to where the other themes are defined. You'll see functions like `pub fn gruvbox()`, `pub fn nord()`, etc.

Add yours right after the last theme:

```rust
pub fn mytheme() -> Self {
    Self {
        bg:             Color::Rgb(30, 30, 46),     // base
        fg:             Color::Rgb(205, 214, 244),  // text
        fg_dim:         Color::Rgb(108, 112, 134),  // overlay0
        accent:         Color::Rgb(137, 180, 250),  // blue
        accent_dim:     Color::Rgb(116, 199, 236),  // sapphire
        success:        Color::Rgb(166, 227, 161),  // green
        warning:        Color::Rgb(249, 226, 175),  // yellow
        error:          Color::Rgb(243, 139, 168),  // red
        border:         Color::Rgb(69, 71, 90),     // surface0
        border_focused: Color::Rgb(137, 180, 250),  // blue
        selection_bg:   Color::Rgb(69, 71, 90),     // surface0
        selection_fg:   Color::Rgb(205, 214, 244),  // text
        diff_added:     Color::Rgb(166, 227, 161),  // green
        diff_removed:   Color::Rgb(243, 139, 168),  // red
        diff_updated:   Color::Rgb(137, 180, 250),  // blue
        current_marker: Color::Rgb(166, 227, 161),  // green
        pinned_marker:  Color::Rgb(249, 226, 175),  // yellow
        boot_marker:    Color::Rgb(137, 180, 250),  // blue
        is_transparent: false,
    }
}
```

> **The `Color::Rgb(r, g, b)` format:** Each value is 0–255. You can convert hex colors like `#89b4fa` to RGB: `0x89` = 137, `0xb4` = 180, `0xfa` = 250 → `Color::Rgb(137, 180, 250)`.
>
> Google "hex to rgb converter" if you need one.

---

## Step 3: Register the theme name

Open `src/config.rs` and find the `ThemeName` enum. It looks like this:

```rust
pub enum ThemeName {
    Gruvbox,
    Nord,
    Transparent,
    Catppuccin,
    // ... more themes ...
}
```

**Add your theme to the enum:**

```rust
    Hacker,
    MyTheme,    // ← add this line at the end (before the closing brace)
}
```

Now you need to wire it up in 3 small places in the same file. Search for each of these and add your theme:

### 3a: `as_str()` — the name used in config.toml

Find the `as_str` function and add your line:

```rust
ThemeName::Hacker => "hacker",
ThemeName::MyTheme => "mytheme",    // ← add this
```

### 3b: `from_str()` — parsing the config file

Find where theme names are parsed from strings and add:

```rust
"hacker" => ThemeName::Hacker,
"mytheme" => ThemeName::MyTheme,    // ← add this
```

### 3c: `next()` — cycling through themes with the keybind

Find the `next()` function (it controls what happens when you press the theme key in Settings) and insert your theme into the chain:

```rust
ThemeName::Hacker => ThemeName::MyTheme,      // ← was: Hacker → Gruvbox
ThemeName::MyTheme => ThemeName::Gruvbox,      // ← add this: MyTheme wraps back
```

### 3d: Connect to your color function

Open `src/ui/theme.rs` and find the `from_name()` function:

```rust
pub fn from_name(name: ThemeName) -> Self {
    match name {
        ThemeName::Gruvbox => Self::gruvbox(),
        // ...
        ThemeName::Hacker => Self::hacker(),
        ThemeName::MyTheme => Self::mytheme(),    // ← add this
    }
}
```

---

## Step 4: Build and test

```bash
cargo build
cargo run
```

Press `,` to open Settings, then press Enter on the theme row to cycle through themes. Yours should appear in the rotation.

If it looks wrong, just tweak the RGB values in `theme.rs` and rebuild. The feedback loop is fast.

---

## Tips

- **Start by copying an existing theme** that's close to what you want, then adjust the colors. Much faster than starting from scratch.
- **Test with multiple modules** — check at least Generations (tables), Services (status colors), Storage (progress bars), and Help (the about page). These cover most of the color fields.
- **Transparent mode:** If you set `is_transparent: true`, nixmate will use your terminal's background color instead of `bg`. Great for terminals with custom backgrounds or blur effects.
- **Naming:** Use lowercase, no spaces for the config name (e.g. `"mytheme"`). The enum name can be CamelCase (e.g. `MyTheme`).

---

## Share your theme!

Created something nice? Open a PR — community themes are very welcome, with full credit of course. Include a screenshot if you can!
