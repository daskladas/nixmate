# Adding Error Patterns

This guide explains how to add new error patterns to the Error Translator module. When someone pastes a Nix error that nixmate doesn't recognize yet, you can teach it.

---

## Files you'll touch

| File | What you do there |
|------|-------------------|
| `src/modules/errors/patterns.rs` | Add the pattern (regex + English explanation + fix) |
| `src/modules/errors/patterns_i18n.rs` | Add the German translation |

That's it — two files, and the second one is optional if you don't speak German.

---

## How error patterns work

When a user pastes an error message, nixmate tries every pattern against it:

```
User pastes error → for each Pattern in PATTERNS → try regex match
                     ├── Match found? → show title + explanation + solution
                     └── No match?    → try next pattern (or AI fallback)
```

Each pattern has:

| Field | What it does |
|-------|-------------|
| `id` | Unique identifier like `"missing-header"` (used as key for translations) |
| `category` | `Build`, `Eval`, `Flake`, `Fetch`, or `NixOS` |
| `regex_str` | The regex that matches the error message |
| `title` | One-line summary (can use `$1`, `$2` for captured groups) |
| `explanation` | Why this error happens (2-3 sentences, educational) |
| `solution` | What to do — concrete code the user can copy-paste |
| `deep_dive` | Detailed technical explanation (shown when user wants to learn more) |
| `tip` | Optional one-liner with a useful hint |

---

## Real example: Adding a pattern

Let's say you saw this error that nixmate doesn't recognize:

```
error: collision between '/nix/store/...-python3-3.11.6/bin/python'
and '/nix/store/...-python3-3.12.1/bin/python'
```

### Step 1: Write the regex

Open `src/modules/errors/patterns.rs` and scroll to the bottom of the `PATTERNS` array.

```rust
Pattern {
    id: "collision-between",
    category: Category::NixOS,
    regex_str: r"(?i)collision between\s+['\x60\x22]?(/nix/store/[^'\"]+/([^/'\"]+))['\x60\x22]?\s+and\s+['\x60\x22]?(/nix/store/[^'\"]+)",
    title: "File collision: $2",
    explanation: "Two packages are trying to install the same file '$2'. \
        This usually means you have two versions of the same tool in your systemPackages.",
    solution: "\
# Find both packages in your configuration and remove one:
# Check your environment.systemPackages for duplicate entries.
#
# Common fix: use package priorities
# (higher number = lower priority)
environment.systemPackages = [
  (lib.hiPrio pkgs.python3)   # this one wins
  pkgs.python312
];",
    deep_dive: "\
WHY THIS HAPPENS:
When NixOS builds your system profile, it creates a single directory tree
by symlinking files from all packages in systemPackages. If two packages
provide the same file (like /bin/python), Nix doesn't know which one to use.

COMMON CAUSES:
1. Two Python versions (python3 + python312)
2. Conflicting CLI tools (e.g. ripgrep from two different sources)
3. Overlapping packages (like imagemagick and graphicsmagick)

HOW TO FIX:
Option A: Remove one of the conflicting packages
Option B: Use lib.hiPrio to set priority:
  (lib.hiPrio pkgs.python3)  # higher priority = this file wins
Option C: Use lib.lowPrio for the one you want to lose:
  (lib.lowPrio pkgs.python312)",
    tip: Some("Use `nix-store -q --referrers /nix/store/...-python3/` to see what depends on it"),
},
```

**Regex tips:**

- `(?i)` at the start makes the match case-insensitive.
- Use `['\x60\x22]?` instead of just `'` to match single quotes, backticks, and double quotes. Nix uses different quote styles depending on the version.
  - `'` = single quote
  - `\x60` = backtick (`` ` ``)
  - `\x22` = double quote (`"`)
- Capture groups `($1)`, `($2)` let you insert matched text into the title and explanation.
- `\s+` matches one or more spaces/newlines.
- Test your regex at [regex101.com](https://regex101.com) — select "Rust" flavor.

### Step 2: Add the German translation

Open `src/modules/errors/patterns_i18n.rs` and add an entry to the `TRANSLATIONS_DE` HashMap:

```rust
m.insert("collision-between", PatternTranslation {
    title: "Datei-Kollision: $2",
    explanation: "Zwei Pakete versuchen die gleiche Datei '$2' zu installieren. \
        Das bedeutet meistens, dass du zwei Versionen des gleichen Tools in deinen \
        systemPackages hast.",
    solution: "\
# Finde beide Pakete in deiner Konfiguration und entferne eines:
# Prüfe deine environment.systemPackages auf doppelte Einträge.
#
# Häufiger Fix: Paket-Prioritäten setzen
environment.systemPackages = [
  (lib.hiPrio pkgs.python3)   # dieses gewinnt
  pkgs.python312
];",
    deep_dive: "\
WARUM PASSIERT DAS:
Wenn NixOS dein System-Profil baut, erstellt es einen einzigen
Verzeichnisbaum durch Symlinks aus allen Paketen in systemPackages.
Wenn zwei Pakete die gleiche Datei liefern (z.B. /bin/python),
weiß Nix nicht welche es nehmen soll.

HÄUFIGE URSACHEN:
1. Zwei Python-Versionen (python3 + python312)
2. Konfligierende CLI-Tools (z.B. ripgrep aus zwei Quellen)
3. Überlappende Pakete (z.B. imagemagick und graphicsmagick)

WIE DU ES BEHEBST:
Option A: Entferne eines der kollidierenden Pakete
Option B: Setze lib.hiPrio für Priorität:
  (lib.hiPrio pkgs.python3)  # höhere Priorität = diese Datei gewinnt
Option C: Setze lib.lowPrio für das unterlegene Paket:
  (lib.lowPrio pkgs.python312)",
    tip: Some("Nutze `nix-store -q --referrers /nix/store/...-python3/` um Abhängigkeiten zu sehen"),
});
```

**Important:** The `id` in the HashMap key must match the `id` in `patterns.rs` exactly.

---

## Style guide for explanations

**Do:**
- Explain **why** the error happens, not just what to do.
- Provide **copy-pasteable** code in the solution.
- Use the deep_dive for the "I want to understand this properly" audience.
- Keep the explanation short (2-3 sentences). Save the details for deep_dive.
- Include common variants of the error in your regex.

**Don't:**
- Don't just say "add X to your config" without explaining why.
- Don't assume the user knows Nix internals.
- Don't write a solution that only works for Flakes OR Channels — try to cover both.

---

## Testing your pattern

1. Build and run:
   ```bash
   cargo build && cargo run
   ```

2. Switch to the Error Translator (press `2`).

3. Press `i` to enter input mode, paste your error message, press `Esc`.

4. Press `Enter` to analyze. Your pattern should match.

5. If it doesn't match, check:
   - Did you add the pattern inside the `PATTERNS` array (between `&[` and `]`)?
   - Is the regex valid? (Test at regex101.com)
   - Is the regex too strict? Try making parts optional with `?`.

---

## The `$1`, `$2` placeholder system

Capture groups in the regex become `$1`, `$2`, etc. in the title, explanation, and solution:

```rust
// Regex: r"undefined variable ['`\"](\w+)['`\"]"
// If the error is: undefined variable 'pkgs'
// Then $1 = "pkgs"

title: "Undefined variable: $1",
// Becomes: "Undefined variable: pkgs"
```

This works in all text fields: title, explanation, solution, deep_dive, and tip.

---

## Universal quote matching

Nix error messages use different quote styles depending on the version and context. Always use this pattern for quoted strings:

```
['\x60\x22]   matches ' or ` or "
```

Example:
```rust
// BAD:  r"undefined variable '(\w+)'"
// GOOD: r"undefined variable ['\x60\x22](\w+)['\x60\x22]"
```

This ensures your pattern matches regardless of which Nix version the user runs.

---

## Checklist

- [ ] Pattern added to `PATTERNS` array in `patterns.rs`
- [ ] Regex tested at regex101.com (Rust flavor)
- [ ] Explanation is educational (explains why, not just what)
- [ ] Solution has copy-pasteable code
- [ ] Deep dive provides real understanding
- [ ] Universal quote matching (`['\x60\x22]`) used where needed
- [ ] German translation added to `patterns_i18n.rs` (same `id`)
- [ ] Tested manually — paste a real error message and verify the match
