# Keybindings

Complete keyboard reference for nixmate. Print this or keep it open in a second terminal while learning.

---

## Global (work everywhere)

| Key | Action |
|-----|--------|
| `1` – `9`, `0` | Switch to module 1–10 |
| `,` | Open Settings |
| `?` | Open Help / About |
| `q` | Quit nixmate |
| `F1` – `F4` | Switch sub-tabs within a module |

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

| Key | Action |
|-----|--------|
| `F1` | Overview — list all generations |
| `F2` | Packages — show packages in selected generation |
| `F3` | Diff — compare two generations side by side |
| `F4` | Manage — delete, pin, restore generations |
| `Enter` | Select generation for detail view |
| `/` | Search/filter packages (in Packages tab) |
| `Tab` | Switch focus between panels |
| `Space` | Toggle selection (in Manage tab) |
| `a` / `A` | Select all (in Manage tab) |
| `c` / `C` | Compare selected generations (in Manage tab) |
| `d` | Delete selected (in Manage tab) |

---

## [2] Error Translator

| Key | Action |
|-----|--------|
| `F1` | Analyze — paste and analyze errors |
| `F2` | Submit — submit a new pattern |
| `i` | Enter input mode (start typing/pasting) |
| `Esc` | Exit input mode |
| `Enter` | Analyze the pasted error |
| `Tab` | Switch between explanation sections |
| `a` | Request AI analysis (if enabled in Settings) |

---

## [3] Services & Ports

| Key | Action |
|-----|--------|
| `F1` | All services |
| `F2` | Active only |
| `F3` | Failed only |
| `F4` | Ports |
| `/` | Search/filter services |
| `Enter` | View service details / logs |
| `s` | Start service |
| `S` | Stop service |
| `r` | Restart service |
| `e` | Enable service |
| `d` | Disable service |

---

## [4] Storage

| Key | Action |
|-----|--------|
| `F1` | Dashboard — disk usage overview |
| `F2` | Explorer — browse store paths |
| `F3` | Cleanup — GC, optimize, full clean |
| `F4` | History — past cleanup operations |
| `/` | Search store paths (in Explorer) |
| `Enter` | Run selected cleanup action |

---

## [5] Config Showcase

| Key | Action |
|-----|--------|
| `F1` | System Poster — generate system info SVG |
| `F2` | Config Diagram — architecture diagram |
| `Enter` | Generate / export SVG |
| `r` | Refresh system scan |

---

## [6] Options Explorer

| Key | Action |
|-----|--------|
| `F1` | Search — fuzzy search all options |
| `F2` | Browse — tree navigation |
| `F3` | Related — sibling options |
| `/` or `i` | Start typing a search query |
| `Enter` | Open detail view for selected option |
| `Esc` | Close detail view / exit search |
| `r` | Show related options for current selection |

---

## [7] Rebuild Dashboard

| Key | Action |
|-----|--------|
| `F1` | Dashboard — build progress |
| `F2` | Log — raw build output |
| `F3` | Changes — post-build diff |
| `F4` | History — past builds |
| `Enter` / `r` | Start rebuild (shows sudo prompt) |
| `m` | Cycle rebuild mode (switch/boot/test/build/dry-build) |
| `t` | Toggle `--show-trace` |
| `c` | Cancel running build |
| `/` | Search in build log (Log tab) |

---

## [8] Flake Input Manager

| Key | Action |
|-----|--------|
| `F1` | Overview — all inputs with age |
| `F2` | Update — selective per-input update |
| `F3` | History — update diffs |
| `F4` | Details — full input info |
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

| Key | Action |
|-----|--------|
| `F1` | Health — score and check results |
| `F2` | Fix — one-click fix suggestions |
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
