//! System information gathering for Config Showcase poster.
//!
//! Collects NixOS-specific system details using shell commands.
//! ALL commands have timeouts — never blocks indefinitely.

use crate::nix::detect::detect_flakes;
use std::process::Command;
use std::time::{Duration, Instant};

/// Complete system information for the poster.
#[derive(Debug, Clone)]
pub struct PosterInfo {
    pub hostname: String,
    pub nixos_version: String,
    pub kernel: String,
    pub uptime: String,
    pub channel: String,
    pub is_flake: bool,
    pub has_home_manager: bool,
    pub package_count: usize,
    pub cpu: String,
    pub memory: String,
    pub gpu: String,
    pub desktop: String,
    pub shell: String,
    pub terminal: String,
    pub editor: String,
    pub services: Vec<String>,
    pub service_count: usize,
    pub container_count: usize,
    pub interfaces: Vec<(String, String)>,
    #[allow(dead_code)] // Populated by sysinfo collector
    pub store_size: String,
    pub store_paths: usize,
    pub disk_total: String,
    pub disk_used: String,
    pub disk_free: String,
    pub disk_fs: String,
    pub users: Vec<String>,
    pub bootloader: String,
    pub generation_count: usize,
}

/// Gather all system info. Blocking — run in background thread!
pub fn gather() -> PosterInfo {
    PosterInfo {
        hostname: get_hostname(),
        nixos_version: get_nixos_version(),
        kernel: get_kernel(),
        uptime: get_uptime(),
        channel: get_channel(),
        is_flake: detect_flakes(None),
        has_home_manager: detect_home_manager(),
        package_count: get_package_count(),
        cpu: get_cpu(),
        memory: get_memory(),
        gpu: get_gpu(),
        desktop: detect_desktop(),
        shell: detect_shell(),
        terminal: detect_terminal(),
        editor: detect_editor(),
        services: get_running_services(),
        service_count: get_service_count(),
        container_count: get_container_count(),
        interfaces: get_network_interfaces(),
        store_size: get_store_size(),
        store_paths: get_store_path_count(),
        disk_total: get_disk_info("size"),
        disk_used: get_disk_info("used"),
        disk_free: get_disk_info("avail"),
        disk_fs: get_disk_info("fs"),
        users: get_users(),
        bootloader: detect_bootloader(),
        generation_count: get_generation_count(),
    }
}

fn cmd(program: &str, args: &[&str], timeout_secs: u64) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let output = child.wait_with_output().ok()?;
                    return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
                }
                return None;
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

/// Like cmd() but also returns output on non-zero exit code
fn cmd_any(program: &str, args: &[&str], timeout_secs: u64) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().ok()?;
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
                return Some(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

fn get_hostname() -> String {
    cmd("hostname", &[], 3).unwrap_or_else(|| "nixos".into())
}

fn get_nixos_version() -> String {
    cmd("nixos-version", &[], 3)
        .map(|v| {
            let parts: Vec<&str> = v.split_whitespace().collect();
            let version = parts.first().unwrap_or(&"");
            let short = version.split('.').take(2).collect::<Vec<&str>>().join(".");
            if parts.len() > 1 {
                format!("{} {}", short, parts[1..].join(" "))
            } else {
                short
            }
        })
        .unwrap_or_else(|| "Unknown".into())
}

fn get_kernel() -> String {
    cmd("uname", &["-r"], 3).unwrap_or_else(|| "Unknown".into())
}

fn get_uptime() -> String {
    cmd("uptime", &["-p"], 3)
        .map(|s| s.trim_start_matches("up ").to_string())
        .unwrap_or_else(|| {
            std::fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|s| {
                    let secs: f64 = s.split_whitespace().next()?.parse().ok()?;
                    let hours = secs as u64 / 3600;
                    let mins = (secs as u64 % 3600) / 60;
                    if hours > 24 {
                        Some(format!("{} days, {} hours", hours / 24, hours % 24))
                    } else {
                        Some(format!("{} hours, {} min", hours, mins))
                    }
                })
                .unwrap_or_else(|| "unknown".into())
        })
}

fn get_channel() -> String {
    if detect_flakes(None) {
        if let Ok(content) = std::fs::read_to_string("/etc/nixos/flake.nix") {
            for tag in &[
                "nixos-unstable",
                "nixos-25.05",
                "nixos-25.11",
                "nixos-24.11",
                "nixos-24.05",
            ] {
                if content.contains(tag) {
                    return format!("{} (flake)", tag);
                }
            }
            return "flake".into();
        }
    }
    cmd("nix-channel", &["--list"], 5)
        .and_then(|output| {
            for line in output.lines() {
                if line.starts_with("nixos ") || line.starts_with("nixos\t") {
                    let url = line.split_whitespace().nth(1)?;
                    let name = url.rsplit('/').next()?;
                    return Some(name.to_string());
                }
            }
            output.lines().next().map(|l| l.to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn detect_home_manager() -> bool {
    // Check HM command
    if cmd("which", &["home-manager"], 3).is_some() {
        return true;
    }
    // Check for HM generations (standalone)
    if let Ok(entries) = std::fs::read_dir("/nix/var/nix/profiles/per-user") {
        for entry in entries.flatten() {
            let path = entry.path().join("home-manager");
            if path.exists() {
                return true;
            }
        }
    }
    // Check for HM as NixOS module (activation unit)
    if cmd_any("systemctl", &["list-units", "--no-legend", "--no-pager"], 5)
        .map(|s| s.contains("home-manager"))
        .unwrap_or(false)
    {
        return true;
    }
    // Check for HM profile link
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    std::path::Path::new(&format!("{}/.local/state/nix/profiles/home-manager", home)).exists()
        || std::path::Path::new(&format!("{}/.local/state/home-manager/gcroots", home)).exists()
}

fn get_package_count() -> usize {
    // Method 1: nix path-info (works better with flakes)
    if let Some(out) = cmd("nix", &["path-info", "-r", "/run/current-system"], 30) {
        let count = out.lines().count();
        if count > 0 {
            return count;
        }
    }
    // Method 2: nix-store --requisites (classic)
    if let Some(out) = cmd(
        "nix-store",
        &["-q", "--requisites", "/run/current-system"],
        30,
    ) {
        let count = out.lines().count();
        if count > 0 {
            return count;
        }
    }
    // Method 3: Count entries in system PATH
    if let Ok(entries) = std::fs::read_dir("/run/current-system/sw/bin") {
        let count = entries.count();
        if count > 0 {
            return count;
        }
    }
    0
}

// ── Hardware ──

fn get_cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            for line in content.lines() {
                if line.starts_with("model name") {
                    let name = line.split(':').nth(1)?.trim();
                    return Some(shorten_cpu(name));
                }
            }
            None
        })
        .unwrap_or_else(|| "Unknown".into())
}

fn shorten_cpu(name: &str) -> String {
    let s = name
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace("(tm)", "")
        .replace("CPU ", "")
        .replace("Processor", "")
        .replace("with Radeon Graphics", "")
        .replace("with Radeon Vega Graphics", "");
    let s = s.trim();

    // AMD Ryzen: keep "Ryzen X NNNN[U/X/H]" (up to 4 words)
    if s.contains("Ryzen") {
        if let Some(pos) = s.find("Ryzen") {
            let rest = &s[pos..];
            let words: Vec<&str> = rest.split_whitespace().collect();
            // "Ryzen 7 PRO 5850U" → take 4, "Ryzen 7 5800X" → take 3
            let take = if words.len() > 2 && words[2] == "PRO" {
                4
            } else {
                3
            };
            return words
                .iter()
                .take(take)
                .copied()
                .collect::<Vec<&str>>()
                .join(" ");
        }
    }
    // Intel: find iN-NNNNN pattern
    for prefix in &["i3-", "i5-", "i7-", "i9-"] {
        if let Some(pos) = s.find(prefix) {
            let rest = &s[pos..];
            return rest.split_whitespace().next().unwrap_or(rest).to_string();
        }
    }
    // Apple
    if s.contains("Apple") {
        return s.to_string();
    }

    // Fallback: truncate
    if s.len() > 25 {
        let t: String = s.chars().take(25).collect();
        format!("{t}…")
    } else {
        s.to_string()
    }
}

fn get_memory() -> String {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            let mut total_kb: u64 = 0;
            let mut avail_kb: u64 = 0;
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    total_kb = line.split_whitespace().nth(1)?.parse().ok()?;
                }
                if line.starts_with("MemAvailable:") {
                    avail_kb = line.split_whitespace().nth(1)?.parse().ok()?;
                }
            }
            if total_kb == 0 {
                return None;
            }
            let used_gb = (total_kb - avail_kb) as f64 / 1048576.0;
            let total_gb = total_kb as f64 / 1048576.0;
            Some(format!("{:.1}/{:.1} GB", used_gb, total_gb))
        })
        .unwrap_or_else(|| "Unknown".into())
}

fn get_gpu() -> String {
    cmd("lspci", &[], 3)
        .and_then(|output| {
            for line in output.lines() {
                if line.contains("VGA")
                    || line.contains("3D controller")
                    || line.contains("Display")
                {
                    // Everything after the last ": "
                    let name = line.rsplit(": ").next()?.trim();
                    return Some(shorten_gpu(name));
                }
            }
            None
        })
        .unwrap_or_else(|| "Integrated".into())
}

fn shorten_gpu(name: &str) -> String {
    // Check for [bracketed name] like "[GeForce RTX 3060]" or "[Radeon RX 6700 XT]"
    if let Some(start) = name.rfind('[') {
        if let Some(end) = name.rfind(']') {
            let inner = &name[start + 1..end];
            let short = inner
                .replace("GeForce ", "")
                .replace("Radeon ", "")
                .replace("Lite Hash Rate", "");
            let trimmed = short.trim();
            if !trimmed.is_empty() && trimmed.len() <= 22 {
                return trimmed.to_string();
            }
            // If still too long, take first meaningful part
            if let Some(slash) = trimmed.find('/') {
                let first = trimmed[..slash].trim();
                if !first.is_empty() {
                    return first.to_string();
                }
            }
            if trimmed.len() > 22 {
                return {
                    let t: String = trimmed.chars().take(22).collect();
                    format!("{t}…")
                };
            }
            return trimmed.to_string();
        }
    }

    // No brackets — clean up vendor names
    let short = name
        .replace("NVIDIA Corporation", "NVIDIA")
        .replace("Advanced Micro Devices, Inc.", "AMD")
        .replace("[AMD/ATI]", "")
        .replace("Intel Corporation", "Intel");
    let trimmed = short.trim();

    // For AMD integrated: "Picasso/Raven 2 [Radeon Vega Series / Radeon Vega Mobile Series]"
    // After bracket extraction fails (too long), try to find the marketing name
    if trimmed.contains("Vega") {
        if trimmed.contains("Mobile") {
            return "Radeon Vega (iGPU)".into();
        }
        return "Radeon Vega".into();
    }
    if trimmed.contains("Rembrandt") || trimmed.contains("Raphael") {
        return "RDNA2 (iGPU)".into();
    }
    if trimmed.contains("Phoenix") || trimmed.contains("Hawk Point") {
        return "RDNA3 (iGPU)".into();
    }

    if trimmed.len() > 22 {
        let t: String = trimmed.chars().take(22).collect();
        format!("{t}…")
    } else {
        trimmed.to_string()
    }
}

// ── Rice info ──

fn detect_desktop() -> String {
    for var in &[
        "XDG_SESSION_DESKTOP",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
    ] {
        if let Ok(de) = std::env::var(var) {
            if !de.is_empty() {
                return capitalize(&de);
            }
        }
    }
    for (proc, name) in &[
        ("hyprland", "Hyprland"),
        ("sway", "Sway"),
        ("i3", "i3"),
        ("dwm", "dwm"),
        ("bspwm", "bspwm"),
        ("awesome", "awesome"),
        ("river", "river"),
        ("qtile", "Qtile"),
        ("niri", "niri"),
    ] {
        if cmd("pgrep", &["-x", proc], 2).is_some() {
            return name.to_string();
        }
    }
    "Headless".into()
}

fn detect_shell() -> String {
    std::env::var("SHELL")
        .map(|s| s.rsplit('/').next().unwrap_or(&s).to_string())
        .unwrap_or_else(|_| "bash".into())
}

fn detect_terminal() -> String {
    // Best source: TERM_PROGRAM
    if let Ok(t) = std::env::var("TERM_PROGRAM") {
        if !t.is_empty() {
            return t;
        }
    }
    // TERMINAL env
    if let Ok(t) = std::env::var("TERMINAL") {
        if !t.is_empty() {
            return t.rsplit('/').next().unwrap_or(&t).to_string();
        }
    }
    // Check running processes
    for (proc, name) in &[
        ("kitty", "kitty"),
        ("alacritty", "Alacritty"),
        ("wezterm-gui", "WezTerm"),
        ("foot", "foot"),
        ("konsole", "Konsole"),
        ("st", "st"),
        ("urxvt", "urxvt"),
        ("ghostty", "Ghostty"),
    ] {
        if cmd("pgrep", &["-x", proc], 2).is_some() {
            return name.to_string();
        }
    }
    // Fallback: TERM, but clean up
    std::env::var("TERM")
        .map(|t| {
            // "xterm-kitty" → "kitty", "xterm-256color" → "xterm"
            if let Some(rest) = t.strip_prefix("xterm-") {
                if rest != "256color" && rest != "color" {
                    return rest.to_string();
                }
            }
            t
        })
        .unwrap_or_else(|_| "unknown".into())
}

fn detect_editor() -> String {
    for var in &["EDITOR", "VISUAL"] {
        if let Ok(e) = std::env::var(var) {
            let name = e.rsplit('/').next().unwrap_or(&e).to_string();
            if !name.is_empty() {
                // Prettify common editor names
                return match name.as_str() {
                    "nvim" => "Neovim".into(),
                    "vim" => "Vim".into(),
                    "emacs" | "emacsclient" => "Emacs".into(),
                    "code" | "code-oss" => "VS Code".into(),
                    "hx" => "Helix".into(),
                    "kak" => "Kakoune".into(),
                    "micro" => "micro".into(),
                    other => other.to_string(),
                };
            }
        }
    }
    // Check which editors are installed
    for (bin, name) in &[
        ("nvim", "Neovim"),
        ("vim", "Vim"),
        ("emacs", "Emacs"),
        ("code", "VS Code"),
        ("hx", "Helix"),
    ] {
        if cmd("which", &[bin], 2).is_some() {
            return name.to_string();
        }
    }
    "nano".into()
}

// ── Services ──

fn get_running_services() -> Vec<String> {
    cmd(
        "systemctl",
        &[
            "list-units",
            "--type=service",
            "--state=running",
            "--no-legend",
            "--no-pager",
        ],
        5,
    )
    .map(|output| {
        output
            .lines()
            .filter_map(|line| {
                let name = line.split_whitespace().next()?;
                let clean = name.trim_end_matches(".service");
                // Skip internal/boring services
                let skip = [
                    "systemd-",
                    "dbus",
                    "nscd",
                    "polkit",
                    "accounts-daemon",
                    "nix-daemon",
                    "user@",
                    "rtkit",
                    "udisks",
                    "colord",
                    "ModemManager",
                    "NetworkManager-wait",
                    "power-profiles",
                    "audit",
                    "logind",
                    "upower",
                    "thermald",
                    "fwupd",
                    "getty@",
                    "serial-getty@",
                    "switcheroo",
                    "low-memory",
                    "fprintd",
                    "pcscd",
                    "greetd",
                ];
                if skip.iter().any(|s| clean.starts_with(s)) {
                    return None;
                }
                Some(clean.to_string())
            })
            .take(5)
            .collect()
    })
    .unwrap_or_default()
}

fn get_service_count() -> usize {
    cmd(
        "systemctl",
        &[
            "list-units",
            "--type=service",
            "--state=running",
            "--no-legend",
            "--no-pager",
        ],
        5,
    )
    .map(|s| s.lines().count())
    .unwrap_or(0)
}

fn get_container_count() -> usize {
    let docker = cmd("docker", &["ps", "-q"], 3)
        .map(|s| if s.is_empty() { 0 } else { s.lines().count() })
        .unwrap_or(0);
    let podman = cmd("podman", &["ps", "-q"], 3)
        .map(|s| if s.is_empty() { 0 } else { s.lines().count() })
        .unwrap_or(0);
    docker + podman
}

// ── Network ──

fn get_network_interfaces() -> Vec<(String, String)> {
    cmd("ip", &["-brief", "addr", "show"], 3)
        .map(|output| {
            output
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 3 {
                        return None;
                    }
                    let name = parts[0];
                    let state = parts[1];
                    if state != "UP" || name == "lo" {
                        return None;
                    }

                    // Skip Docker/container virtual interfaces
                    if name.starts_with("veth")
                        || name.starts_with("br-")
                        || name.starts_with("docker")
                        || name.starts_with("cni")
                        || name.starts_with("flannel")
                        || name.starts_with("vxlan")
                    {
                        return None;
                    }

                    // Find first IPv4 address (skip IPv6)
                    let ip = parts[2..]
                        .iter()
                        .find(|addr| {
                            // IPv4 addresses contain dots, IPv6 contain colons
                            addr.contains('.') && !addr.starts_with("169.254")
                        })
                        .or_else(|| parts.get(2))
                        .unwrap_or(&"");
                    let ip_clean = ip.split('/').next().unwrap_or(ip);

                    // Shorten interface names for display
                    let display_name = if name.len() > 12 {
                        format!("{}…", &name[..12])
                    } else {
                        name.to_string()
                    };

                    Some((display_name, ip_clean.to_string()))
                })
                .take(3)
                .collect()
        })
        .unwrap_or_default()
}

// ── Storage ──

fn get_store_size() -> String {
    // Fast method: use df to calculate Nix store usage
    // This avoids the slow `du -sh /nix/store` which can take minutes
    //
    // Alternative: nix store info (Nix 2.4+)
    if let Some(out) = cmd("nix", &["store", "info", "--json"], 10) {
        // Parse JSON-ish output for store size
        // Not all Nix versions support this, so fallback
        if let Some(pos) = out.find("\"storeDir\"") {
            let _ = pos; // just confirm it's valid output
        }
    }

    // Method: count store paths and estimate, or use disk usage
    // Best fast method: df shows total used on /nix partition
    if let Some(output) = cmd("df", &["-h", "/nix/store"], 3) {
        if let Some(line) = output.lines().nth(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(used) = parts.get(2) {
                return used.to_string();
            }
        }
    }

    // Slowest fallback: du with shorter timeout
    cmd("du", &["-sh", "/nix/store"], 30)
        .and_then(|s| s.split_whitespace().next().map(|v| v.to_string()))
        .unwrap_or_else(|| "N/A".into())
}

fn get_store_path_count() -> usize {
    std::fs::read_dir("/nix/store")
        .map(|e| e.count())
        .unwrap_or(0)
}

fn get_disk_info(field: &str) -> String {
    cmd("df", &["-h", "/nix/store"], 3)
        .and_then(|output| {
            let line = output.lines().nth(1)?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            match field {
                "fs" => parts.first().map(|s| {
                    // "/dev/nvme0n1p2" → "nvme0n1p2"
                    s.trim_start_matches("/dev/").to_string()
                }),
                "size" => parts.get(1).map(|s| s.to_string()),
                "used" => parts.get(2).map(|s| s.to_string()),
                "avail" => parts.get(3).map(|s| s.to_string()),
                _ => None,
            }
        })
        .unwrap_or_else(|| "?".into())
}

// ── System ──

fn get_users() -> Vec<String> {
    std::fs::read_to_string("/etc/passwd")
        .map(|content| {
            content
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() < 7 {
                        return None;
                    }
                    let name = parts[0];
                    let uid: u32 = parts[2].parse().ok()?;
                    let shell = parts[6];

                    // Only real users: UID 1000-59999, with real shell, not system accounts
                    if !(1000..60000).contains(&uid) {
                        return None;
                    }

                    // Skip nix build users and other system-like accounts
                    if name.starts_with("nixbld")
                        || name.starts_with("systemd-")
                        || name.starts_with("polkitd")
                        || name == "nobody"
                        || name == "messagebus"
                        || name == "avahi"
                        || name == "nm-openconnect"
                        || name == "colord"
                        || name == "flatpak"
                    {
                        return None;
                    }

                    // Skip users with nologin/false shell
                    if shell.ends_with("nologin") || shell.ends_with("/false") {
                        return None;
                    }

                    Some(name.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn detect_bootloader() -> String {
    // Priority 1: NixOS authoritative source.
    // The active system's switch-to-configuration script references the actual
    // configured bootloader — immune to leftover files from previous setups.
    if let Ok(content) = std::fs::read_to_string("/run/current-system/bin/switch-to-configuration")
    {
        let lower = content.to_lowercase();
        if lower.contains("systemd-boot") || lower.contains("bootctl") {
            return "systemd-boot".into();
        }
        if lower.contains("grub") {
            return "GRUB".into();
        }
    }

    // Priority 2: bootctl status (works without root on most systems)
    if let Some(out) = cmd_any("bootctl", &["status"], 3) {
        if out.contains("systemd-boot") {
            return "systemd-boot".into();
        }
        if out.contains("grub") {
            return "GRUB".into();
        }
    }

    // Priority 3: Filesystem heuristics (last resort — may give false positives
    // if bootloader was changed without cleaning up old files)
    if std::path::Path::new("/boot/loader/loader.conf").exists()
        || std::path::Path::new("/boot/EFI/systemd/systemd-bootx64.efi").exists()
        || std::path::Path::new("/boot/efi/EFI/systemd/systemd-bootx64.efi").exists()
    {
        return "systemd-boot".into();
    }
    if std::path::Path::new("/boot/grub/grub.cfg").exists()
        || std::path::Path::new("/boot/EFI/grub").exists()
    {
        return "GRUB".into();
    }

    "unknown".into()
}

fn get_generation_count() -> usize {
    let mut count = 0;

    // System profiles
    if let Ok(entries) = std::fs::read_dir("/nix/var/nix/profiles/") {
        count += entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("system-") && n.ends_with("-link"))
                    .unwrap_or(false)
            })
            .count();
    }

    // Also check system profile symlink for number
    if count <= 1 {
        if let Ok(target) = std::fs::read_link("/nix/var/nix/profiles/system") {
            if let Some(name) = target.file_name().and_then(|n| n.to_str()) {
                // "system-142-link" → 142
                if let Some(num_str) = name
                    .strip_prefix("system-")
                    .and_then(|s| s.strip_suffix("-link"))
                {
                    if let Ok(n) = num_str.parse::<usize>() {
                        if n > count {
                            count = n;
                        }
                    }
                }
            }
        }
    }

    count
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
