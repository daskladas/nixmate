# Changelog

Full development history of nixmate — every version, every feature.

---

### v0.1 — ✅ Foundations

The core architecture, module system, and first two real modules.

- [x] Full TUI framework: ratatui-based, keyboard-driven, modular architecture
- [x] Module system with sub-tabs (F1–F4), popup dialogs, flash messages, and status bar hints
- [x] **Generations module** — overview, packages, diff, and manage with undo safety net
  - [x] System + Home-Manager profile detection (Flakes and Channels)
  - [x] Side-by-side diff with added/removed/updated packages
  - [x] Delete, pin, restore with 5-second undo countdown
  - [x] Kernel update and security package highlighting
- [x] **Error Translator** — pattern matching engine with 50+ patterns
  - [x] Deep-dive explanations (not just "do this", but "here's why")
  - [x] Submit-your-own-pattern workflow
  - [x] Full German translations for all patterns
- [x] Global settings: theme switching, language switching, layout modes
- [x] Three themes: Gruvbox (warm), Nord (cool), Transparent (terminal-native)
- [x] English + German localization (UI, error patterns, all modules)
- [x] Nix flake packaging with `nix run` support

### v0.2 — ✅ Server Tools

Making nixmate useful for server admins, not just desktop users.

- [x] **Services & Ports** — unified server dashboard
  - [x] Systemd services, Docker containers, Podman containers in one view
  - [x] Automatic port-to-service/container mapping via PID + process name
  - [x] Start/stop/restart/enable/disable with sudo confirmation
  - [x] Live logs with color coding (journalctl + docker/podman logs)
  - [x] Filter by Active / Systemd / Containers / Failed + text search
- [x] **Storage** — Nix store analysis and cleanup
  - [x] Disk usage dashboard with progress bars and color coding
  - [x] Store breakdown: total/live/dead paths with sizes
  - [x] Top 10 largest store paths with visual bars
  - [x] Store explorer with live/dead filter and text search
  - [x] Garbage collect, store optimize, full clean with confirmation
  - [x] Persistent cleanup history tracking
  - [x] Actionable recommendations on dashboard
- [x] **Help / About tab** — module overview, contribute info
- [x] Error Translator input fix (no more accidental key capture on tab switch)
- [x] **Lazy loading** — Services and Storage load data on first tab visit, not at startup
- [x] **Timeout protection** — all Docker/Podman/Nix commands have timeouts (3–30s). No more freezes when Docker daemon is down or Nix store is huge.
- [x] README overhaul with detailed roadmap

### v0.3 — ✅ Pipe Integration & AI

Making the Error Translator truly powerful.

- [x] **Pipe support**: `nixos-rebuild switch 2>&1 | nixmate` — pipe build output directly into the error translator
- [x] **AI fallback**: when no pattern matches, send the error to Claude/OpenAI/Ollama for analysis
- [x] **Ollama local mode**: fully offline AI analysis via local models — with NixOS setup guide when Ollama isn't running
- [x] **API key management**: inline editing in the TUI — no more manual config file edits
- [x] **Universal quote matching**: all 50+ error patterns now match `'`, `` ` ``, and `"` interchangeably

### v0.4 — ✅ Config Showcase

Generate a beautiful dark-themed system poster — designed for r/unixporn, GitHub READMEs, and flex posts.

- [x] **System scanner** — detects hostname, NixOS version, kernel, channel, Flakes, Home Manager, hardware (CPU/GPU/RAM), desktop environment, shell, terminal, editor
- [x] **SVG export** — clean vector poster with JetBrains Mono, gradient topbar, info pills, 6 themed cards (Hardware, Services, Network, Packages, Storage, System), badges, and footer summary. Zero dependencies, always works.
- [x] **Background scanning** — system info gathered in background thread with timeouts on every command, never blocks the TUI
- [x] **Works everywhere** — Flakes, Channels, Home Manager, headless servers, desktops with any WM/DE

### v0.5 — ✅ Config Showcase: Diagram Mode

Expand Config Showcase with a full architecture diagram of your NixOS configuration.

- [x] **Config Diagram** — architecture visualization of your entire NixOS config structure as a professional SVG node graph
  - [x] Scan `/etc/nixos/` (or flake root) for all `.nix` files and `imports` chains
  - [x] Parse `flake.nix` → extract inputs (nixpkgs, home-manager, ...) and outputs
  - [x] Smart directory grouping — 126 files collapse into ~20 readable group cards
  - [x] Expanded file listings — each group card shows all contained files
  - [x] Color-coded nodes: flake root (cyan), flake inputs (blue), system/host configs (green), hardware (pink), home manager (purple), modules (orange)
  - [x] Structural arrows showing hierarchy: outputs → builds → imports → contains
  - [x] Smart labels on key arrows (outputs, builds, imports, uses)
  - [x] Variable-height cards — groups grow to fit content, dynamic SVG sizing
  - [x] Multi-row layout with automatic wrapping (4 nodes per row max)
  - [x] Color-sorted layers — same types grouped together within each row
  - [x] Works with any config size: 1 file or 200+ files
  - [x] SVG export with JetBrains Mono, glow effects, gradient topbar, legend
  - [x] Two sub-tabs: F1 = System Overview poster, F2 = Config Diagram

### v0.6 — ✅ Welcome Screen, Mascot & Sidebar

First-run experience, native terminal images, and full UI overhaul.

- [x] **One-time welcome screen** — friendly greeting with mascot image, feature overview, and language selector (←/→ to toggle English/Deutsch). Saves choice to config. Never shown again after Enter.
- [x] **Native terminal image rendering** — mascot via Kitty Graphics Protocol (Kitty, WezTerm, Ghostty) or iTerm2 Inline Images. Full-color PNG, graceful text-only fallback. Auto-detected.
- [x] **Mascot in Help tab** — always visible in the Help/About module for supported terminals
- [x] **Vertical sidebar navigation** — replaces horizontal tab bar. All modules listed top-to-bottom with key hints, content area on the right. Scales to any number of modules.
- [x] **New keybind system** — modules `1`–`9`, Settings `,`, Help `?`. Intuitive numbering matches sidebar position.
- [x] **4 new module placeholders** — Options Explorer, Rebuild, Flake Inputs, Package Search visible in sidebar and routable
- [x] **Config persistence** — `welcome_shown` flag, language selection saved on dismiss

### v0.7 — ✅ NixOS-Native Modules

The modules that make nixmate essential for every NixOS user.

- [x] **Package Search** — fast fuzzy-searchable nixpkgs browser with install status, version info, and "add to config" helper. Auto-detects flakes vs channels, configurable in Settings. Fun loading messages while nix evaluates. Replaces slow `nix search`.
- [x] **6 new themes** — Catppuccin, Dracula, Tokyo Night, Rosé Pine, Everforest, Kanagawa
- [x] **Nixpkgs channel autodetect** — automatically detects flakes vs channels and which branch/channel is in use. Configurable override in Settings.
- [x] **Nix Doctor** — system health dashboard with 0-100% score, 5 automated checks (old generations, store size, disk usage, channel freshness, duplicate packages), and one-click Fix tab.
- [x] **4 more themes** — Solarized Dark, One Dark, Monokai, Hacker (13 total)
- [x] **Options Explorer** — TUI version of search.nixos.org. Browse all 20,000+ NixOS options with fuzzy search, type info, defaults, current values. Tree browsing, related options, your current config values. Works over SSH.
  - [x] 3 sub-tabs: Search (fuzzy), Browse (tree), Related (siblings)
  - [x] Full detail view: type, default, example, current value, declared-in
  - [x] Background loading with progress indicator
  - [x] Current values loaded on-demand via nixos-option
  - [x] Fuzzy matching + substring matching + type color coding
- [x] **Module intro pages** — each module shows a welcome page on first visit with problem statement, features, and tab overview. Press Enter to start. Bilingual (EN/DE).
- [x] **Rebuild Dashboard** — live `nixos-rebuild switch` monitoring with progress, derivation tracking, error catching, post-rebuild diff (packages added/removed/updated, services restarted).
  - [x] 4 sub-tabs: Dashboard, Build Log, Changes, History
  - [x] 5-phase dashboard boxes (Evaluation → Fetching → Building → Activating → Bootloader) with per-phase timers
  - [x] Educational phase explanations — learn what NixOS does at each step
  - [x] Intelligent log beautification — store paths → human-readable names (🔨 Building firefox 134.0)
  - [x] Color-coded live output with auto-scroll and manual scroll override
  - [x] Rebuild mode selector: switch, boot, test, build, dry-build
  - [x] Auto-detects Flakes vs Channels configuration
  - [x] Sudo confirmation popup before every rebuild
  - [x] Full searchable build log with syntax highlighting
  - [x] Post-rebuild diff: packages added/removed/updated, kernel changes, services restarted, NixOS version changes
  - [x] Reboot warning when kernel is updated
  - [x] Persistent JSON history (~/.config/nixmate/rebuild_history.json) with timestamps, mode, duration, and error preview
  - [x] Time estimation based on average of last 5 successful builds
  - [x] Bootloader phase detection (GRUB, systemd-boot)
  - [x] Build cancellation (`c` key) — sends SIGTERM to child process group
  - [x] `--show-trace` toggle (`t` key) — enables verbose Nix evaluation traces for debugging
  - [x] NOPASSWD sudo support — just press Enter without typing a password
  - [x] Terminal bell notification when build completes
  - [x] Log tab shows raw output, Dashboard shows beautified — both always available
  - [x] 15+ intelligent log patterns: derivation counts, fetch summaries, systemd actions, bootloader updates
  - [x] UTF-8 safe text truncation (no panics on multi-byte characters like emojis)
  - [x] Log buffer cap (50,000 lines) to prevent unbounded memory growth
  - [x] Phase explanation "linger" — fast phases still show their explanation text
- [x] **Flake Input Manager** — selective input updates, revision diff, version comparison, per-input updates. Replaces all-or-nothing `nix flake update`.
  - [x] 4 sub-tabs: Overview, Update (selective), History (diffs), Details
  - [x] Selective per-input updates with checkbox UI and confirmation popup
  - [x] Age color coding (green/yellow/red)
  - [x] Full detail view with type, URL, branch, revision, NAR hash, follows
  - [x] Live update progress with per-input status

### Roadmap

Sorted by impact — highest value first. Every feature solves a real NixOS problem that currently has no good solution.

#### 🔥 High Priority — Biggest Pain Points

- [ ] **Options Explorer → Live Config Editor** — Select any NixOS option, type a new value, nixmate writes it to the correct `.nix` file. Diff-preview before saving. For lists/attrsets: structured editor. *Currently, users must manually find which file to edit and guess the syntax. This alone would make nixmate essential.*

- [ ] **Error Translator → Auto-Fix** — Pattern recognizes error, nixmate shows a diff preview ("I'd change line 43 like this"), press Enter and it's written. Not just "here's what to do" but "press Enter and it's done." *Turns error diagnosis into one-click repair.*

- [ ] **Home Manager Dashboard** — HM generations, config diffs, separate HM rebuild, HM-specific option browser (HM options ≠ NixOS options). Detect standalone vs NixOS-module setups. *80%+ of NixOS users run HM. Zero TUI tooling exists for it.*

- [ ] **Package Search → Install from TUI** — Find a package, press Enter, nixmate adds it to `environment.systemPackages` with diff-preview. Also: remove packages the same way. *Bridges the gap between "I found the package" and "it's in my config".*

- [ ] **Channel / Release Upgrade Assistant** — Guided NixOS version upgrade (24.05 → 24.11): scan config for deprecated options, check package availability in target version, show breaking changes from release notes, dry-build before committing. *Currently: change the channel URL and pray. This would eliminate the #1 fear of NixOS upgrades.*

#### ⚡ Medium Priority — Strong Value

- [ ] **Rebuild → Build Queue + Remote Builds** — Queue multiple rebuilds (first `build` to test, then `switch` to apply). Remote machine rebuilds via `nixos-rebuild --target-host user@server`. *Power users manage multiple machines — currently requires separate terminal sessions.*

- [ ] **Rebuild → Build Comparison** — "This build took 3 minutes, last one 45 seconds — why?" Show which derivations were rebuilt vs cached, what changed between builds. *Helps optimize build times — currently invisible.*

- [ ] **Storage → Dependency Graph** — "Why is this 2GB store path here?" Show the full chain: which package → derivation → flake input keeps it alive. Interactive tree explorer with size per branch. *Currently requires `nix why-depends` wizardry that most users don't know.*

- [ ] **Config Linter** — Static analysis of your NixOS config: unused imports, duplicate definitions, deprecated options, missing types, options set but never evaluated. Like `statix` + `nixfmt` but integrated with explanations and auto-fix suggestions. *Catches mistakes before rebuild.*

- [ ] **Nix Doctor → Extended Checks** — Nix daemon status, experimental features enabled but unused, orphaned profiles, broken symlinks in `/etc/nixos/`, missing `nix.conf` optimizations (substituters, `max-jobs`, `cores`), whether Flakes is enabled but not used (or vice versa). *Takes the health score from useful to comprehensive.*

- [ ] **Nix Evaluation Profiler** — "Why does `nixos-rebuild` spend 45 seconds just evaluating?" Show which imports are slow, which IFD calls block, which flake inputs trigger re-evaluation. Uses `nix eval --trace-function-calls`. *Zero existing tooling for this. Build times are a top NixOS complaint.*

#### 🧩 Nice to Have — Completing the Picture

- [ ] **Dev Environment Manager** — List all `flake.nix` files on your system, show defined `devShell`s, enter `nix develop` from the TUI, view packages and environment variables per shell. *"What dev environments do I have across my projects?" — currently: `find / -name flake.nix`.*

- [ ] **Secrets Dashboard** — Unified view for agenix / sops-nix: which secrets exist, which hosts they're encrypted for, last rotation date, dependent services. Re-key from the TUI. *Currently: juggling multiple CLI tools, manually tracking what's where.*

- [ ] **Generations → Rollback + Boot Selection** — One-key `switch --rollback` from the TUI with live progress. Also: set a specific generation for next boot without making it default. *Currently you can browse generations but switching still needs the terminal.*

- [ ] **Error Translator → Community Pattern Sync** — Share patterns via GitHub repo. Submit a pattern → after review it's automatically distributed to all users. Crowdsourced Nix error database. *Scales the Error Translator beyond what one person can maintain.*

- [ ] **Storage → Dedup Finder** — Find packages present in multiple versions (e.g. 3 OpenSSL versions). Show why each version exists and how to consolidate. *Store bloat is a common complaint — this makes it actionable.*

- [ ] **Flake Inputs → Vulnerability Check** — Per-input age + known CVE count. "Your nixpkgs input is 47 days old, 12 security fixes since then." Link to relevant advisories. *Turns flake hygiene from guesswork into data.*

- [ ] **Config Showcase → Export Formats** — Beyond SVG: PNG (directly shareable), Markdown badge for GitHub README, terminal output (`fastfetch`-style flex). *More formats = more sharing = more visibility for the user's config.*

- [ ] **Garbage Collection Scheduler** — Define GC rules: "keep last 5 generations, delete everything older than 30 days, keep store under 50GB." Dry-run preview before execution. *Currently: manual `nix-collect-garbage` and hoping nothing important gets deleted.*

- [ ] **Nix Build Log Viewer** — `nix log` but actually usable: searchable, syntax-highlighted, errors auto-highlighted and jumpable. *When a build fails, finding the actual error in 500 lines of log output is painful.*

---

<p align="center"><sub>See <a href="README.md">README.md</a> for install instructions and usage.</sub></p>
