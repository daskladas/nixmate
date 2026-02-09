# Release Guide for nixmate v0.7.0

Everything you need for the first official release.

---

## Pre-Release Checklist

Run through this before tagging:

- [ ] `cargo build --release` succeeds with no warnings
- [ ] `cargo test` passes all tests
- [ ] `nix build` completes successfully
- [ ] `nix run .` launches correctly
- [ ] Version shows `0.7.0` in: welcome screen, help tab, sidebar, `--version` flag
- [ ] Both languages work (switch to German, check a few modules)
- [ ] Theme switching works (cycle through all 13)
- [ ] Pipe mode works: `echo "undefined variable" | cargo run`
- [ ] Clean exit: run nixmate, quit, check `free -h` (no major RAM lingering)
- [ ] `.gitignore` includes `/target` (so build artifacts aren't committed)
- [ ] No secrets/API keys in committed code
- [ ] CHANGELOG.md is up to date
- [ ] README screenshots are current

---

## Git Tag

```bash
# Make sure everything is committed
git add -A
git commit -m "Prepare v0.7.0 release"

# Create annotated tag
git tag -a v0.7.0 -m "v0.7.0 — NixOS-Native Modules

10 modules, 13 themes, EN/DE localization.
First official release."

# Push with tags
git push origin main --tags
```

---

## GitHub Release Notes (Draft)

Copy this to the GitHub "Create Release" page:

---

### nixmate v0.7.0 — First Release 🚀

**All your NixOS tools in one TUI.** 10 modules, 13 themes, English + German, works over SSH.

#### Highlights

**🔄 Generations** — Browse, diff, delete, pin, restore NixOS generations with undo safety net.

**🔍 Error Translator** — Paste any Nix error → human explanation + fix. 50+ patterns, AI fallback (Claude/OpenAI/Ollama), pipe support (`nixos-rebuild 2>&1 | nixmate`).

**📡 Services & Ports** — Systemd + Docker + Podman in one dashboard. Port mapping, start/stop/restart, live logs.

**💾 Storage** — Nix store analysis with live/dead paths, GC/optimize/clean, cleanup history.

**🖼️ Config Showcase** — Generate SVG system poster + config architecture diagram. Perfect for r/unixporn.

**📖 Options Explorer** — search.nixos.org in your terminal. 20,000+ options, fuzzy search, tree browsing, current values.

**🏗️ Rebuild Dashboard** — Live `nixos-rebuild` with 5-phase progress, educational explanations, post-build diff.

**❄️ Flake Input Manager** — Selective per-input updates instead of all-or-nothing `nix flake update`.

**📦 Package Search** — Fuzzy search over 100k+ packages with install status and flakes/channels auto-detect.

**🩺 Nix Doctor** — Health score 0-100%, 5 automated checks, one-click fixes.

#### Install

```bash
# Try instantly
nix run github:daskladas/nixmate

# Add to flake
inputs.nixmate.url = "github:daskladas/nixmate";

# Build from source
git clone https://github.com/daskladas/nixmate && cd nixmate
nix build   # or: cargo build --release
```

#### Requirements

- NixOS (any recent version)
- Terminal with 256-color support
- Optional: Kitty/WezTerm/iTerm2 for mascot image

---

## Release Binaries

Two options:

### Option A: Nix-only (recommended for first release)

Users install via `nix run` or add to their flake. No binaries to build or host. This is the NixOS way and your target audience already has Nix.

### Option B: Pre-built binaries via GitHub Actions (later)

If you want to provide binaries for non-NixOS users later, create `.github/workflows/release.yml`:

```yaml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: softprops/action-gh-release@v1
        with:
          files: target/release/nixmate
```

**For v0.7.0: Option A is fine.** Your users have Nix. Add binaries later if there's demand.

---

## Flake Compatibility

After tagging and pushing, verify:

```bash
# This should work for anyone:
nix run github:daskladas/nixmate

# With specific version:
nix run github:daskladas/nixmate/v0.7.0
```

The `flake.nix` already defines `packages.default` and `apps.default`, so this should work out of the box.

---

## README Badges (optional)

Add these under the title in README.md:

```markdown
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![NixOS](https://img.shields.io/badge/NixOS-supported-blue?logo=nixos)](https://nixos.org)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange?logo=rust)](https://www.rust-lang.org)
```

---

## Where to Announce

1. **r/NixOS** — main audience. Post with screenshots, explain what it does.
2. **NixOS Discourse** (discourse.nixos.org) — more technical crowd, good for feedback.
3. **r/unixporn** — post a Config Showcase screenshot with your theme. Great for visibility.
4. **r/rust** — "I built a TUI in ratatui" angle. Rust community loves these posts.
5. **Hacker News** — "Show HN: nixmate" if you want broader reach.

**Tip:** Post on r/NixOS first, gather feedback for a week, fix any issues, then go wider.

---

## nixpkgs Packaging

### Prerequisites for nixpkgs inclusion

- **License:** MIT ✅ (already set)
- **Builds reproducibly:** Must build with `nix build` ✅
- **Maintained:** You need to be listed as maintainer
- **Stable:** Should have at least one tagged release ✅ (v0.7.0)
- **Useful:** Solves a real problem ✅

### The Nix expression for nixpkgs

This goes in `pkgs/by-name/ni/nixmate/package.nix`:

```nix
{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:

rustPlatform.buildRustPackage rec {
  pname = "nixmate";
  version = "0.7.0";

  src = fetchFromGitHub {
    owner = "daskladas";
    repo = "nixmate";
    rev = "v${version}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    # ^ Replace with actual hash after tagging. Get it with:
    # nix-prefetch-url --unpack https://github.com/daskladas/nixmate/archive/v0.7.0.tar.gz
  };

  cargoHash = "sha256-BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=";
  # ^ Get this by running `nix build` and letting it fail — it tells you the right hash.

  meta = with lib; {
    description = "NixOS Multi-Tool — all your Nix utilities in one TUI";
    homepage = "https://github.com/daskladas/nixmate";
    license = licenses.mit;
    maintainers = with maintainers; [ /* your nixpkgs maintainer handle */ ];
    platforms = platforms.linux;
    mainProgram = "nixmate";
  };
}
```

### Getting the hashes

After you push the tag:

```bash
# Get the src hash:
nix-prefetch-url --unpack https://github.com/daskladas/nixmate/archive/v0.7.0.tar.gz

# Get the cargoHash: try to build with a dummy hash, Nix will tell you the correct one
```

### nixpkgs PR process

1. Fork nixpkgs on GitHub
2. Create a branch: `git checkout -b nixmate`
3. Add `pkgs/by-name/ni/nixmate/package.nix`
4. Commit: `nixmate: init at 0.7.0`
5. Push and open a PR

**PR description template:**

```markdown
## Description

Add nixmate, a NixOS multi-tool TUI.

- [Homepage](https://github.com/daskladas/nixmate)
- License: MIT
- Maintainer: @yourhandle

## Features

10 integrated NixOS utilities in one keyboard-driven TUI:
generations management, error translation, service dashboard,
store analysis, config visualization, options explorer,
rebuild dashboard, flake input manager, package search, and
system health checks.

## Testing

- `nix build .#nixmate` succeeds
- Tested on NixOS unstable (x86_64-linux)
```

### Realistic timeline

- **PR review:** 1-4 weeks (nixpkgs has thousands of open PRs)
- **Common rejection reasons:** Hash mismatches, missing meta fields, build failures on Hydra
- **Tip:** Keep the PR minimal. Just the package, nothing else. Don't bundle other changes.

### NUR as a stepping stone

If nixpkgs review takes too long, consider [NUR (Nix User Repository)](https://github.com/nix-community/NUR):

- Much faster: self-serve, no review required
- Users add your NUR repo as a flake input
- Good for testing packaging before the nixpkgs PR

```bash
# Users would install via:
nix run github:daskladas/nur-packages#nixmate
```

But honestly, since you already have a working `flake.nix`, users can already `nix run github:daskladas/nixmate`. The nixpkgs PR is mainly for discoverability and `nix-env -iA nixpkgs.nixmate` convenience.
