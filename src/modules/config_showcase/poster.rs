//! System poster generator — SVG and PNG export.
//!
//! Generates a dark-themed system overview infographic.
//! Designed for r/unixporn, GitHub READMEs, and flex posts.

#![allow(clippy::write_with_newline)]
use crate::nix::sysinfo::PosterInfo;
use std::fmt::Write;
use std::path::PathBuf;

// ── Colors ──
const BG: &str = "#0d1117";
const CARD_BG: &str = "#161b22";
const CARD_BORDER: &str = "#30363d";
const FG: &str = "#e6edf3";
const FG2: &str = "#8b949e";
const DIM: &str = "#484f58";
const BLUE: &str = "#58a6ff";
const GREEN: &str = "#3fb950";
const ORANGE: &str = "#d29922";
const PURPLE: &str = "#bc8cff";
const CYAN: &str = "#39d353";
const PINK: &str = "#f778ba";
const TEAL: &str = "#56d4dd";

// ── Layout ──
const W: f64 = 1200.0;
const H: f64 = 820.0;
const PAD: f64 = 50.0;

// Quick info pills (centered)
const PILL_W: f64 = 255.0;
const PILL_H: f64 = 48.0;
const PILL_GAP: f64 = 15.0;
// 4 pills: total = 4*255 + 3*15 = 1065 → start at (1200-1065)/2 = 67.5
const PILL_X: f64 = 67.5;

// Main cards (3-column grid, centered)
const CARD_W: f64 = 340.0;
const CARD_H: f64 = 200.0;
const CARD_GAP: f64 = 20.0;
const CARD_R: f64 = 12.0;
// 3 cards: total = 3*340 + 2*20 = 1060 → start at (1200-1060)/2 = 70
const COL1: f64 = 70.0;
const COL2: f64 = 70.0 + CARD_W + CARD_GAP;
const COL3: f64 = 70.0 + (CARD_W + CARD_GAP) * 2.0;

// Vertical positions
const PILL_Y: f64 = 185.0;
const ROW1: f64 = 255.0;
const ROW2: f64 = ROW1 + CARD_H + CARD_GAP;

/// Generate the complete SVG string.
pub fn generate_svg(info: &PosterInfo) -> String {
    let mut s = String::with_capacity(16384);

    // SVG header + embedded font import
    let _ = write!(s,
r#"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
<defs>
<style>
@import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600;700&amp;display=swap');
text {{ font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace; }}
</style>
</defs>
"#);

    background(&mut s);
    header(&mut s, info);
    badges(&mut s, info);
    pills(&mut s, info);
    card_hardware(&mut s, info);
    card_services(&mut s, info);
    card_network(&mut s, info);
    card_packages(&mut s, info);
    card_storage(&mut s, info);
    card_system(&mut s, info);
    footer(&mut s, info);

    s.push_str("</svg>");
    s
}

/// Save SVG file. Returns file path.
pub fn save_svg(info: &PosterInfo) -> std::io::Result<PathBuf> {
    let svg = generate_svg(info);
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nixmate-poster");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}-nixos.svg", info.hostname));
    std::fs::write(&path, &svg)?;
    Ok(path)
}


// ═══════════════════════════════════════
//  Background + Header
// ═══════════════════════════════════════

fn background(s: &mut String) {
    let _ = write!(s,
r#"<rect width="{W}" height="{H}" rx="16" fill="{BG}"/>
<defs>
<pattern id="grid" width="30" height="30" patternUnits="userSpaceOnUse">
<circle cx="15" cy="15" r="0.5" fill="{DIM}" opacity="0.2"/>
</pattern>
<linearGradient id="topbar" x1="0" y1="0" x2="1" y2="0">
<stop offset="0%" stop-color="{BLUE}"/><stop offset="25%" stop-color="{PURPLE}"/>
<stop offset="50%" stop-color="{PINK}"/><stop offset="75%" stop-color="{ORANGE}"/>
<stop offset="100%" stop-color="{GREEN}"/>
</linearGradient>
</defs>
<rect width="{W}" height="{H}" rx="16" fill="url(#grid)"/>
<rect width="{W}" height="4" rx="2" fill="url(#topbar)"/>
"#);
}

fn header(s: &mut String, info: &PosterInfo) {
    // Small label
    let _ = write!(s,
        r#"<text x="{PAD}" y="40" font-size="11" fill="{DIM}" letter-spacing="3" font-weight="600">NIXMATE  ·  SYSTEM OVERVIEW</text>
"#);

    // Hostname (big)
    let _ = write!(s,
        r#"<text x="{PAD}" y="90" font-size="42" fill="{FG}" font-weight="700">{}</text>
"#, esc(&info.hostname));

    // Subtitle: version + kernel
    let _ = write!(s,
        r#"<text x="{PAD}" y="120" font-size="15" fill="{FG2}">NixOS {}  ·  Linux {}  ·  up {}</text>
"#, esc(&info.nixos_version), esc(&info.kernel), esc(&info.uptime));
}

// ═══════════════════════════════════════
//  Badges
// ═══════════════════════════════════════

fn badges(s: &mut String, info: &PosterInfo) {
    let mut x = PAD;
    let y = 138.0;

    badge(s, x, y, &info.channel, BLUE);
    x += badge_w(&info.channel) + 10.0;

    if info.is_flake {
        badge(s, x, y, "Flakes", CYAN);
        x += badge_w("Flakes") + 10.0;
    }

    if info.has_home_manager {
        badge(s, x, y, "Home Manager", PURPLE);
        x += badge_w("Home Manager") + 10.0;
    }

    if info.package_count > 0 {
        let pkg_label = format!("{} packages", fmt_num(info.package_count));
        badge(s, x, y, &pkg_label, GREEN);
    }
}

fn badge(s: &mut String, x: f64, y: f64, text: &str, color: &str) {
    let w = badge_w(text);
    let _ = write!(s,
r#"<rect x="{x}" y="{y}" rx="6" width="{w}" height="24" fill="{color}" opacity="0.12"/>
<rect x="{x}" y="{y}" rx="6" width="{w}" height="24" fill="none" stroke="{color}" stroke-width="1" opacity="0.35"/>
<text x="{tx}" y="{ty}" font-size="11" fill="{color}" text-anchor="middle" font-weight="600">{text}</text>
"#, tx = x + w / 2.0, ty = y + 16.0, text = esc(text));
}

fn badge_w(text: &str) -> f64 { text.len() as f64 * 7.0 + 22.0 }

// ═══════════════════════════════════════
//  Quick Info Pills
// ═══════════════════════════════════════

fn pills(s: &mut String, info: &PosterInfo) {
    let items: [(&str, &str, &str); 4] = [
        ("WM / DE", &info.desktop, PURPLE),
        ("Shell", &info.shell, GREEN),
        ("Terminal", &info.terminal, TEAL),
        ("Editor", &info.editor, ORANGE),
    ];

    for (i, (label, value, color)) in items.iter().enumerate() {
        let x = PILL_X + i as f64 * (PILL_W + PILL_GAP);
        pill(s, x, PILL_Y, label, value, color);
    }
}

fn pill(s: &mut String, x: f64, y: f64, label: &str, value: &str, color: &str) {
    let _ = write!(s,
r#"<rect x="{x}" y="{y}" rx="8" width="{PILL_W}" height="{PILL_H}" fill="{CARD_BG}" stroke="{CARD_BORDER}" stroke-width="1"/>
<rect x="{x}" y="{ay}" rx="2" width="3" height="{ah}" fill="{color}"/>
<text x="{lx}" y="{ly}" font-size="10" fill="{DIM}" font-weight="600" letter-spacing="1">{label}</text>
<text x="{lx}" y="{vy}" font-size="15" fill="{FG}" font-weight="600">{value}</text>
"#,
        ay = y + 8.0, ah = PILL_H - 16.0,
        lx = x + 16.0, ly = y + 18.0, vy = y + 36.0,
        label = esc(&label.to_uppercase()), value = esc(value));
}

// ═══════════════════════════════════════
//  Card primitives
// ═══════════════════════════════════════

fn card_bg(s: &mut String, x: f64, y: f64, accent: &str) {
    let _ = write!(s,
r#"<rect x="{x}" y="{y}" rx="{CARD_R}" width="{CARD_W}" height="{CARD_H}" fill="{CARD_BG}" stroke="{CARD_BORDER}" stroke-width="1"/>
<rect x="{x}" y="{ay}" rx="{CARD_R}" width="3" height="{ah}" fill="{accent}"/>
"#, ay = y + 10.0, ah = CARD_H - 20.0);
}

fn card_hdr(s: &mut String, x: f64, y: f64, title: &str, accent: &str) {
    let _ = write!(s,
r#"<text x="{tx}" y="{ty}" font-size="13" fill="{accent}" font-weight="700" letter-spacing="1">{title}</text>
<line x1="{lx}" y1="{ly}" x2="{lx2}" y2="{ly}" stroke="{CARD_BORDER}" stroke-width="1"/>
"#, tx = x + 18.0, ty = y + 28.0, title = esc(&title.to_uppercase()),
     lx = x + 14.0, lx2 = x + CARD_W - 14.0, ly = y + 38.0);
}

fn row(s: &mut String, x: f64, y: f64, idx: usize, text: &str, color: &str) {
    let truncated = if text.chars().count() > 38 {
        let t: String = text.chars().take(35).collect();
        format!("{t}...")
    } else {
        text.to_string()
    };
    let _ = write!(s,
        r#"<text x="{tx}" y="{ty}" font-size="12" fill="{color}">{text}</text>
"#, tx = x + 20.0, ty = y + 57.0 + idx as f64 * 20.0, text = esc(&truncated));
}

fn kv(s: &mut String, x: f64, y: f64, idx: usize, key: &str, val: &str) {
    // Truncate value to ~24 chars to stay within card bounds
    let truncated = if val.chars().count() > 24 {
        let t: String = val.chars().take(21).collect();
        format!("{t}...")
    } else {
        val.to_string()
    };
    let _ = write!(s,
r#"<text x="{kx}" y="{ty}" font-size="12" fill="{FG2}">{key}</text>
<text x="{vx}" y="{ty}" font-size="12" fill="{FG}">{val}</text>
"#, kx = x + 20.0, vx = x + 150.0, ty = y + 57.0 + idx as f64 * 20.0,
     key = esc(key), val = esc(&truncated));
}

fn card_footer_text(s: &mut String, x: f64, y: f64, text: &str) {
    let _ = write!(s,
        r#"<text x="{tx}" y="{ty}" font-size="10" fill="{DIM}">{text}</text>
"#, tx = x + 20.0, ty = y + CARD_H - 12.0, text = esc(text));
}

// ═══════════════════════════════════════
//  Row 1: Hardware, Services, Network
// ═══════════════════════════════════════

fn card_hardware(s: &mut String, info: &PosterInfo) {
    let (x, y) = (COL1, ROW1);
    card_bg(s, x, y, PINK);
    card_hdr(s, x, y, "Hardware", PINK);

    kv(s, x, y, 0, "CPU", &info.cpu);
    kv(s, x, y, 1, "Memory", &info.memory);
    kv(s, x, y, 2, "GPU", &info.gpu);
    kv(s, x, y, 3, "Kernel", &info.kernel);
}

fn card_services(s: &mut String, info: &PosterInfo) {
    let (x, y) = (COL2, ROW1);
    card_bg(s, x, y, GREEN);
    card_hdr(s, x, y, "Services", GREEN);

    if info.services.is_empty() {
        row(s, x, y, 0, "No services detected", DIM);
    } else {
        for (i, svc) in info.services.iter().take(5).enumerate() {
            row(s, x, y, i, &format!("▸ {}", svc), FG);
        }
        let rem = info.service_count.saturating_sub(5);
        if rem > 0 {
            row(s, x, y, 5, &format!("  +{} more", rem), DIM);
        }
    }

    card_footer_text(s, x, y, &format!(
        "{} services · {} containers", info.service_count, info.container_count));
}

fn card_network(s: &mut String, info: &PosterInfo) {
    let (x, y) = (COL3, ROW1);
    card_bg(s, x, y, TEAL);
    card_hdr(s, x, y, "Network", TEAL);

    if info.interfaces.is_empty() {
        row(s, x, y, 0, "No active interfaces", DIM);
    } else {
        for (i, (name, ip)) in info.interfaces.iter().enumerate() {
            kv(s, x, y, i, name, ip);
        }
    }
}

// ═══════════════════════════════════════
//  Row 2: Packages, Storage, System
// ═══════════════════════════════════════

fn card_packages(s: &mut String, info: &PosterInfo) {
    let (x, y) = (COL1, ROW2);
    card_bg(s, x, y, BLUE);
    card_hdr(s, x, y, "Packages", BLUE);

    // Big centered number
    let _ = write!(s,
r#"<text x="{cx}" y="{cy}" font-size="52" fill="{BLUE}" font-weight="700" text-anchor="middle">{count}</text>
<text x="{cx}" y="{cy2}" font-size="12" fill="{FG2}" text-anchor="middle">in system closure</text>
"#, cx = x + CARD_W / 2.0, cy = y + 105.0, cy2 = y + 125.0,
     count = fmt_num(info.package_count));

    card_footer_text(s, x, y, &format!(
        "{} store paths · {}/{}", fmt_num(info.store_paths), info.disk_used, info.disk_total));
}

fn card_storage(s: &mut String, info: &PosterInfo) {
    let (x, y) = (COL2, ROW2);
    card_bg(s, x, y, ORANGE);
    card_hdr(s, x, y, "Storage", ORANGE);

    kv(s, x, y, 0, "Filesystem", &info.disk_fs);
    kv(s, x, y, 1, "Total", &info.disk_total);
    kv(s, x, y, 2, "Used", &info.disk_used);
    kv(s, x, y, 3, "Free", &info.disk_free);
    kv(s, x, y, 4, "Nix Store", &format!("{} paths", fmt_num(info.store_paths)));
}

fn card_system(s: &mut String, info: &PosterInfo) {
    let (x, y) = (COL3, ROW2);
    card_bg(s, x, y, CYAN);
    card_hdr(s, x, y, "System", CYAN);

    kv(s, x, y, 0, "Bootloader", &info.bootloader);
    kv(s, x, y, 1, "Generations", &info.generation_count.to_string());
    let users_display = if info.users.len() > 3 {
        let first3: Vec<&str> = info.users.iter().take(3).map(|s| s.as_str()).collect();
        format!("{} +{}", first3.join(", "), info.users.len() - 3)
    } else {
        info.users.join(", ")
    };
    kv(s, x, y, 2, "Users", &users_display);
    if info.has_home_manager {
        kv(s, x, y, 3, "Home-Manager", "active");
    }
    if info.is_flake {
        kv(s, x, y, if info.has_home_manager { 4 } else { 3 }, "Config", "Flakes");
    }
}

// ═══════════════════════════════════════
//  Footer
// ═══════════════════════════════════════

fn footer(s: &mut String, info: &PosterInfo) {
    let fy = H - 80.0;

    // Divider
    let _ = write!(s,
r#"<line x1="{PAD}" y1="{fy}" x2="{x2}" y2="{fy}" stroke="{CARD_BORDER}" stroke-width="1"/>
"#, x2 = W - PAD);

    // Summary — only include non-zero items
    let mut parts = Vec::new();
    if info.package_count > 0 {
        parts.push(format!("{} packages", fmt_num(info.package_count)));
    }
    parts.push(format!("{} services", info.service_count));
    parts.push(format!("{}/{} disk", info.disk_used, info.disk_total));
    parts.push(format!("{} generations", info.generation_count));
    parts.push(info.channel.clone());
    let summary = parts.join("  ·  ");
    let _ = write!(s,
r#"<text x="{cx}" y="{sy}" font-size="12" fill="{FG2}" text-anchor="middle">{summary}</text>
"#, cx = W / 2.0, sy = fy + 25.0, summary = esc(&summary));

    // Branding
    let _ = write!(s,
r#"<text x="{cx}" y="{by}" font-size="10" fill="{DIM}" text-anchor="middle">generated with nixmate  ·  github.com/daskladas/nixmate</text>
"#, cx = W / 2.0, by = fy + 50.0);
}

// ═══════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
     .replace('>', "&gt;").replace('"', "&quot;")
}

fn fmt_num(n: usize) -> String {
    if n >= 1000 {
        let t = n / 1000;
        let r = (n % 1000) / 100;
        if r > 0 { format!("{}.{}K", t, r) } else { format!("{}K", t) }
    } else {
        n.to_string()
    }
}
