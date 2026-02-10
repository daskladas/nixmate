# Keybindings

Complete keyboard reference for nixmate. Print this or keep it open in a second terminal while learning.

---

## Global (work everywhere)

| Key | Action |
|-----|--------|
| `1` – `9`, `0` | Switch to module 1–10 |
| `[` / `]` | Previous / next sub-tab |
| `,` | Open Settings |
| `?` | Open Help / About |
| `q` | Quit nixmate |

---

## Navigation (most modules)

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `Enter` | Select / confirm / expand |
| `Esc` | Back / close / cancel |
| `Tab` | Switch panel (where applicable) |

---

## [1] Generations

Sub-tabs: Overview · Packages · Diff · Manage

| Key | Action |
|-----|--------|
| `Enter` | Select generation for detail view |
| `/` | Search/filter packages (in Packages tab) |
| `Tab` | Switch focus between panels |
| `Space` | Toggle selection (in Manage tab) |
| `a` / `A` | Select all (in Manage tab) |
| `c` / `C` | Compare selected generations (in Manage tab) |
| `d` | Delete selected (in Manage tab) |

---

## [2] Error Translator

Sub-tabs: Analyze · Submit

| Key | Action |
|-----|--------|
| `i` | Enter input mode (start typing/pasting) |
| `Esc` | Exit input mode |
| `Enter` | Analyze the pasted error |
| `Tab` | Switch between explanation sections |
| `a` | Request AI analysis (if enabled in Settings) |

---

## [3] Services & Ports

Sub-tabs: Overview · Ports · Manage · Logs

| Key | Action |
|-----|--------|
| `/` | Search/filter services |
| `Enter` | View service details / logs |
| `s` | Start service |
| `S` | Stop service |
| `r` | Restart service |
| `e` | Enable service |
| `d` | Disable service |

---

## [4] Storage

Sub-tabs: Dashboard · Explorer · Clean · History

| Key | Action |
|-----|--------|
| `/` | Search store paths (in Explorer) |
| `Enter` | Run selected cleanup action |

---

## [5] Config Showcase

Sub-tabs: System Overview · Config Diagram

| Key | Action |
|-----|--------|
| `Enter` | Generate / export SVG |
| `r` | Refresh system scan |

---

## [6] Options Explorer

Sub-tabs: Search · Browse · Related

| Key | Action |
|-----|--------|
| `/` or `i` | Start typing a search query |
| `Enter` | Open detail view for selected option |
| `Esc` | Close detail view / exit search |
| `r` | Show related options for current selection |

---

## [7] Rebuild Dashboard

Sub-tabs: Dashboard · Log · Changes · History

| Key | Action |
|-----|--------|
| `Enter` / `r` | Start rebuild (shows sudo prompt) |
| `m` | Cycle rebuild mode (switch/boot/test/build/dry-build) |
| `t` | Toggle `--show-trace` |
| `c` | Cancel running build |
| `/` | Search in build log (Log tab) |

---

## [8] Flake Input Manager

Sub-tabs: Overview · Update · History · Details

| Key | Action |
|-----|--------|
| `Space` | Toggle input selection (Update tab) |
| `Enter` | Confirm update / view details |
| `u` | Update selected inputs |

---

## [9] Package Search

| Key | Action |
|-----|--------|
| `/` or `i` | Start search |
| `Enter` | View package details |
| `Esc` | Close search / detail view |

---

## [0] Nix Doctor

Sub-tabs: Dashboard · Fix

| Key | Action |
|-----|--------|
| `Enter` | Run selected fix |
| `r` | Re-scan |

---

## Settings (`,`)

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate settings |
| `Enter` / `→` | Change value / enter edit mode |
| `Esc` | Cancel text editing |

---

## Tips

- **Module intros:** The first time you visit each module in a session, you'll see an intro page. Press `Enter` to dismiss it.
- **Flash messages:** Status messages (like "Settings saved") disappear after 3 seconds automatically.
- **Pipe mode:** When you pipe into nixmate (`... | nixmate`), it opens directly in the Error Translator with your output pre-loaded.
