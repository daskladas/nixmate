# Troubleshooting

Common problems and how to fix them.

---

## Terminal broken after crash

**Symptom:** Your terminal shows garbled text, doesn't respond to input normally, or looks weird after nixmate crashed or was killed.

**Fix:**

```bash
reset
```

That's it. The `reset` command restores your terminal to a sane state. If that doesn't work:

```bash
stty sane
tput rmcup
```

**Why this happens:** nixmate puts the terminal in "raw mode" and switches to the "alternate screen" (like vim does). If it crashes before restoring these, your terminal stays in that state. The v0.7.0 panic handler should prevent this, but `kill -9` can still cause it.

---

## Mascot image not showing

**Symptom:** The welcome screen or help tab shows blank space where the mascot should be.

**Cause:** Your terminal doesn't support the Kitty Graphics Protocol or iTerm2 Inline Images.

**Supported terminals:**
- ✅ Kitty
- ✅ WezTerm
- ✅ Ghostty
- ✅ iTerm2 (macOS)
- ❌ Alacritty (no image protocol)
- ❌ GNOME Terminal
- ❌ Konsole
- ❌ xterm
- ❌ tmux / screen (blocks image protocols)

**Fix:** This is purely cosmetic — nixmate works perfectly without the image. Switch to a supported terminal if you want the mascot, or just enjoy the text-only experience.

**tmux users:** tmux strips terminal escape sequences including image protocols. Run nixmate outside tmux to see images.

---

## RAM usage after closing

**Symptom:** After closing nixmate, `free -h` shows more used memory than before.

**Possible causes:**

1. **Kitty GPU textures:** The Kitty terminal caches images in GPU memory. nixmate sends a delete command on exit, but if the exit was unclean, textures may linger. Fix: close and reopen the Kitty window, or run `kitty @ remove-image --all` if using Kitty's remote control.

2. **Linux page cache:** Linux aggressively caches disk reads in RAM. This is normal and expected — the memory IS available, it's just being used as cache. Check `available` column in `free -h`, not `used`.

3. **Nix commands:** Some modules run `nix-env`, `nix search`, etc. in background threads. These Nix processes can use significant RAM. They should exit when nixmate exits, but check with `ps aux | grep nix`.

---

## Options Explorer says "Failed to load options"

**Symptom:** The Options Explorer shows an error instead of the 20,000+ options.

**Common causes:**

1. **Missing `nixos-option`:** The Options Explorer uses `nixos-option` to read current values. Install it:
   ```nix
   environment.systemPackages = [ pkgs.nixos-option ];
   ```

2. **No `NIX_PATH`:** If you use Flakes exclusively and don't have `NIX_PATH` set, the option loading may fail. The Options Explorer tries multiple paths — if all fail, it shows an error.

3. **Slow evaluation:** Loading 20k+ options can take 10-30 seconds on first visit. Wait for the loading indicator to finish.

---

## Package Search shows no results

**Symptom:** Searching in Package Search returns nothing or shows an error.

**Possible causes:**

1. **Wrong nixpkgs channel:** Go to Settings (`,`) → Nixpkgs Channel and check the value. `auto` should work for most setups. If not, set it explicitly to your channel (e.g. `nixos-unstable` or `nixos-24.11`).

2. **`nix search` not available:** If you're on an older NixOS version without the `nix` command (using `nix-env` only), Package Search may not work. Enable the Nix command:
   ```nix
   nix.settings.experimental-features = [ "nix-command" "flakes" ];
   ```

3. **First search is slow:** The first search triggers a `nix search` which can take 30-60 seconds while Nix evaluates nixpkgs. Subsequent searches are fast.

---

## Rebuild Dashboard: "Permission denied"

**Symptom:** Starting a rebuild shows a permission error.

**Fix:** nixmate runs `sudo nixos-rebuild` and prompts for your password in a popup. Make sure:

1. Your user is in the `wheel` group (can use sudo).
2. You type the correct password in the popup.
3. If you use NOPASSWD sudo, just press Enter without typing anything.

**NOPASSWD setup** (optional, in `configuration.nix`):

```nix
security.sudo.extraRules = [{
  users = [ "yourusername" ];
  commands = [{
    command = "/run/current-system/sw/bin/nixos-rebuild";
    options = [ "NOPASSWD" ];
  }];
}];
```

---

## Services & Ports: Docker/Podman not showing

**Symptom:** Docker or Podman containers don't appear in the Services tab.

**Check:**

```bash
# Is Docker running?
systemctl status docker

# Is Podman available?
which podman
```

nixmate auto-detects Docker and Podman. If the service isn't running or the command isn't installed, those containers won't appear. Systemd services always show up.

---

## Config file corrupted

**Symptom:** nixmate crashes on startup with a config parse error.

**Fix:** Delete the config and let nixmate recreate it:

```bash
rm ~/.config/nixmate/config.toml
```

Or fix it manually — it's just TOML:

```bash
cat ~/.config/nixmate/config.toml
# Look for syntax errors: missing quotes, bad values, etc.
```

---

## Flake Input Manager: "No flake.nix found"

**Symptom:** The Flake Input Manager says it can't find your flake.

**Where it looks:**
1. `/etc/nixos/flake.nix`
2. `~/.config/nixos/flake.nix`
3. `~/nixos/flake.nix`
4. `~/.nixos/flake.nix`

If your flake is elsewhere, the Flake Input Manager can't find it yet. This is a known limitation.

---

## Still stuck?

Open an issue: [github.com/daskladas/nixmate/issues](https://github.com/daskladas/nixmate/issues)

Include:
- What you tried to do
- What happened instead
- Your NixOS version (`nixos-version`)
- Your terminal emulator
- The error message (if any)
