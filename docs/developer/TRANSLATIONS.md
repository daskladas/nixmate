# Adding Translations (i18n)

This guide explains how nixmate handles translations and how to add new UI strings or a completely new language.

---

## Files you'll touch

| File | What you do there |
|------|-------------------|
| `src/i18n.rs` | Add string fields + English value + German value |
| `src/config.rs` | Add new language to enum (only if adding a whole new language) |

For adding a single new string, you only touch `i18n.rs`.

---

## How it works

All user-facing text lives in `src/i18n.rs`. There's one big struct and two instances:

```rust
pub struct Strings {
    pub app_title: &'static str,
    pub tab_generations: &'static str,
    pub gen_overview: &'static str,
    // ... hundreds more fields ...
}

static ENGLISH: Strings = Strings {
    app_title: "nixmate",
    tab_generations: "Generations",
    gen_overview: "Overview",
    // ...
};

static GERMAN: Strings = Strings {
    app_title: "nixmate",
    tab_generations: "Generationen",
    gen_overview: "Übersicht",
    // ...
};

pub fn get_strings(lang: Language) -> &'static Strings {
    match lang {
        Language::English => &ENGLISH,
        Language::German => &GERMAN,
    }
}
```

Code gets strings like this:

```rust
let s = i18n::get_strings(lang);
// Use s.tab_generations, s.gen_overview, etc.
```

---

## The 3-place rule

Every string appears **exactly 3 times** in `i18n.rs`:

1. As a **field** in the `Strings` struct
2. As a **value** in the `ENGLISH` instance
3. As a **value** in the `GERMAN` instance

If you add a field without both values, the code won't compile. Rust's type system enforces completeness — you can't forget a translation.

---

## How to add a new string

Let's say you're building a feature that needs the text "No results found" / "Keine Ergebnisse gefunden".

### Step 1: Add the field

Open `src/i18n.rs` and find the section for your module. Fields are grouped by module with comments:

```rust
pub struct Strings {
    // === App-level ===
    pub app_title: &'static str,
    // ...

    // === Generations ===
    pub gen_overview: &'static str,
    // ...

    // === Your module ===
    pub mymod_no_results: &'static str,    // ← add this
}
```

### Step 2: Add the English value

Find the `static ENGLISH: Strings = Strings { ... }` block and add your value in the same position:

```rust
    mymod_no_results: "No results found",
```

### Step 3: Add the German value

Find the `static GERMAN: Strings = Strings { ... }` block and add:

```rust
    mymod_no_results: "Keine Ergebnisse gefunden",
```

### Step 4: Use it in code

```rust
let s = i18n::get_strings(lang);
let text = s.mymod_no_results;  // → "No results found" or "Keine Ergebnisse gefunden"
```

That's it. Build with `cargo build` to verify everything compiles.

---

## Naming conventions

String field names follow a prefix system based on the module:

| Prefix | Module |
|--------|--------|
| `app_`, `welcome_` | App-level, welcome screen |
| `tab_`, `desc_` | Tab names and descriptions |
| `gen_` | Generations |
| `err_` | Error Translator |
| `svc_` | Services & Ports |
| `sto_` | Storage |
| `cfg_` | Config Showcase |
| `opt_` | Options Explorer |
| `rb_` | Rebuild Dashboard |
| `fi_` | Flake Input Manager |
| `pkg_` | Package Search |
| `health_` | Nix Doctor |
| `settings_` | Settings |
| `help_` | Help / About |

After the prefix, use a descriptive name:
- `gen_delete_confirm` — the deletion confirmation text in Generations
- `svc_restart_success` — "Service restarted" flash message in Services
- `rb_phase_evaluating` — the "Evaluating" phase name in Rebuild

---

## How to verify you didn't miss anything

If you added a field but forgot one of the values, `cargo build` will immediately tell you:

```
error[E0063]: missing field `mymod_no_results` in initializer of `Strings`
```

This is Rust's type system doing the work for you. You literally cannot forget a translation.

To double-check your additions, you can grep for your prefix:

```bash
# How many fields start with mymod_?
grep -c 'mymod_' src/i18n.rs
# Should be a multiple of 3 (1 field + 1 English + 1 German per string)
```

---

## How to add a new language

This is more involved but still straightforward. Let's say you want to add French.

### Step 1: Add the language variant

Open `src/config.rs` and add to the `Language` enum:

```rust
pub enum Language {
    English,
    German,
    French,     // ← add this
}
```

Then update these methods in the same file:

```rust
impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::German => "Deutsch",
            Language::French => "Français",     // ← add this
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Language::English => Language::German,
            Language::German => Language::French,    // ← change this
            Language::French => Language::English,   // ← add this
        }
    }
}
```

### Step 2: Create the translation instance

Open `src/i18n.rs` and create a new `static FRENCH`:

```rust
static FRENCH: Strings = Strings {
    app_title: "nixmate",
    tab_generations: "Générations",
    // ... every single field needs a French value ...
};
```

**Tip:** The easiest way is to copy the entire `ENGLISH` block, rename it to `FRENCH`, and then translate every value. The compiler will tell you if you missed any.

### Step 3: Wire it into get_strings

```rust
pub fn get_strings(lang: Language) -> &'static Strings {
    match lang {
        Language::English => &ENGLISH,
        Language::German => &GERMAN,
        Language::French => &FRENCH,     // ← add this
    }
}
```

### Step 4: Update the welcome screen

The welcome screen lets users pick a language with ←/→ keys. You may need to add the French option there too (in `src/modules/splash.rs`).

### Step 5: Build and test

```bash
cargo build
cargo run
```

Switch to French in Settings and check that all strings look correct across all modules.

---

## Common mistakes

**1. Forgetting a comma**

Each line in the Strings instance ends with a comma:

```rust
// WRONG — missing comma after the value
app_title: "nixmate"
tab_generations: "Generations",

// RIGHT
app_title: "nixmate",
tab_generations: "Generations",
```

**2. Wrong order**

The fields in `ENGLISH` and `GERMAN` must be in the **same order** as in the `Strings` struct. If they're out of order, Rust will give confusing type errors.

**3. Mixing up the blocks**

Make sure you're adding English strings to the `ENGLISH` block and German strings to the `GERMAN` block. It's easy to lose your place in a 1200-line file. Use your editor's search to jump to the right block.

**4. Using runtime strings**

All strings must be `&'static str` — that means string literals defined at compile time. You can't use `format!()` or `String` here:

```rust
// WRONG
app_title: format!("nixmate v{}", VERSION),

// RIGHT (use &'static str, do formatting in the render code)
app_title: "nixmate",
```

---

## Error Translator patterns

The Error Translator has its own separate translation system in `patterns_i18n.rs` because error patterns use `$1`/`$2` placeholders. See [ADDING_PATTERNS.md](ADDING_PATTERNS.md) for how to translate error patterns.
