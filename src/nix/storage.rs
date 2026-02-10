//! Storage analysis backend for nixmate
//!
//! Provides disk usage analysis, Nix store inspection,
//! garbage collection, store optimization, and cleanup history.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;
use std::time::Duration;

// ════════════════════════════════════════════════════════════════════
// DATA TYPES
// ════════════════════════════════════════════════════════════════════

/// A single store path with metadata
#[derive(Debug, Clone)]
pub struct StorePath {
    #[allow(dead_code)] // Parsed from nix-store output
    pub path: String,
    pub name: String,
    pub size: u64,
    pub is_dead: bool,
}

/// Filesystem usage information
#[derive(Debug, Clone)]
pub struct DiskUsage {
    #[allow(dead_code)] // Parsed from df output
    pub mount_point: String,
    pub filesystem: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percent: f64,
}

/// Overall store information
#[derive(Debug, Clone, Default)]
pub struct StoreInfo {
    pub disk_store: Option<DiskUsage>,
    pub disk_root: Option<DiskUsage>,
    pub paths: Vec<StorePath>,
    pub total_paths: usize,
    pub live_paths: usize,
    pub dead_paths: usize,
    pub total_size: u64,
    pub live_size: u64,
    pub dead_size: u64,
    pub has_sizes: bool,
}

/// Result of a garbage collection run
#[derive(Debug, Clone)]
pub struct GcResult {
    pub paths_removed: usize,
    pub bytes_freed: u64,
    #[allow(dead_code)] // Stored for log display
    pub output: String,
}

/// Result of store optimization
#[derive(Debug, Clone)]
pub struct OptimiseResult {
    pub bytes_saved: u64,
    #[allow(dead_code)] // Stored for log display
    pub output: String,
}

/// A recorded cleanup action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub action: String,
    pub freed_bytes: u64,
    pub paths_removed: usize,
}

/// Available cleanup actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanAction {
    /// Remove dead store paths (no sudo)
    GarbageCollect,
    /// Hardlink deduplication (no sudo)
    Optimise,
    /// Full GC including old generations (sudo)
    FullClean,
}

impl CleanAction {
    pub fn all() -> &'static [CleanAction] {
        &[
            CleanAction::GarbageCollect,
            CleanAction::Optimise,
            CleanAction::FullClean,
        ]
    }

    pub fn needs_sudo(&self) -> bool {
        matches!(self, CleanAction::FullClean)
    }

    pub fn icon(&self) -> &'static str {
        match self {
            CleanAction::GarbageCollect => "🗑",
            CleanAction::Optimise => "🔗",
            CleanAction::FullClean => "⚠",
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// LOADING
// ════════════════════════════════════════════════════════════════════

/// Load complete store information
pub fn load_store_info() -> StoreInfo {
    let mut info = StoreInfo {
        disk_store: parse_disk_usage("/nix/store"),
        disk_root: parse_disk_usage("/"),
        ..Default::default()
    };

    // If store and root are on the same filesystem, only show root
    if let (Some(store), Some(root)) = (&info.disk_store, &info.disk_root) {
        if store.filesystem == root.filesystem {
            info.disk_store = None;
        }
    }

    // Load store paths with sizes
    let dead_set = load_dead_set();

    // Try nix path-info first (gives sizes)
    let paths = load_paths_with_sizes(&dead_set);
    if !paths.is_empty() {
        info.has_sizes = true;
        info.paths = paths;
    } else {
        // Fallback: just path listing without sizes
        info.paths = load_paths_without_sizes(&dead_set);
        info.has_sizes = false;
    }

    // Sort by size descending
    info.paths.sort_by(|a, b| b.size.cmp(&a.size));

    // Compute stats
    info.total_paths = info.paths.len();
    for p in &info.paths {
        if p.is_dead {
            info.dead_paths += 1;
            info.dead_size += p.size;
        } else {
            info.live_paths += 1;
            info.live_size += p.size;
        }
    }
    info.total_size = info.live_size + info.dead_size;

    info
}

/// Parse disk usage from `df` for a given path
fn parse_disk_usage(path: &str) -> Option<DiskUsage> {
    let output = Command::new("df")
        .args(["-B1", "--output=source,target,size,used,avail,pcent", path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?; // Skip header
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() < 6 {
        return None;
    }

    let total: u64 = parts[2].parse().unwrap_or(0);
    let used: u64 = parts[3].parse().unwrap_or(0);
    let available: u64 = parts[4].parse().unwrap_or(0);
    let percent_str = parts[5].trim_end_matches('%');
    let percent: f64 = percent_str.parse().unwrap_or(0.0);

    Some(DiskUsage {
        filesystem: parts[0].to_string(),
        mount_point: parts[1].to_string(),
        total,
        used,
        available,
        percent,
    })
}

/// Load the set of dead (unreferenced) store paths (with timeout)
fn load_dead_set() -> HashSet<String> {
    let mut dead = HashSet::new();

    let output = output_with_timeout("nix-store", &["--gc", "--print-dead"], 15);

    if let Some(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("/nix/store/") {
                    dead.insert(trimmed.to_string());
                }
            }
        }
    }

    dead
}

/// Load all store paths with NAR sizes via `nix path-info --all -S` (with timeout)
fn load_paths_with_sizes(dead_set: &HashSet<String>) -> Vec<StorePath> {
    let output = output_with_timeout("nix", &["path-info", "--all", "-S"], 30);

    let out = match output {
        Some(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let mut paths = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Format: /nix/store/hash-name   SIZE
        // Split from the right to handle paths with spaces (unlikely but safe)
        if let Some(last_space) = trimmed.rfind(|c: char| c.is_whitespace()) {
            let path = trimmed[..last_space].trim();
            let size_str = trimmed[last_space..].trim();
            let size: u64 = size_str.parse().unwrap_or(0);

            if path.starts_with("/nix/store/") {
                paths.push(StorePath {
                    name: path_to_name(path),
                    is_dead: dead_set.contains(path),
                    path: path.to_string(),
                    size,
                });
            }
        }
    }

    paths
}

/// Fallback: load paths without sizes via `nix-store -q --all` (with timeout)
fn load_paths_without_sizes(dead_set: &HashSet<String>) -> Vec<StorePath> {
    let output = output_with_timeout("nix-store", &["-q", "--all"], 15);

    let out = match output {
        Some(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let mut paths = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("/nix/store/") {
            paths.push(StorePath {
                name: path_to_name(trimmed),
                is_dead: dead_set.contains(trimmed),
                path: trimmed.to_string(),
                size: 0,
            });
        }
    }

    paths
}

/// Extract a human-readable name from a store path
/// /nix/store/abc123...xyz-package-name-1.0 → package-name-1.0
fn path_to_name(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/nix/store/") {
        // Hash is 32 chars, then a dash
        if rest.len() > 33 && rest.as_bytes()[32] == b'-' {
            return rest[33..].to_string();
        }
    }
    path.to_string()
}

// ════════════════════════════════════════════════════════════════════
// ACTIONS
// ════════════════════════════════════════════════════════════════════

/// Run garbage collection (dead paths only, no sudo)
pub fn run_gc() -> Result<GcResult> {
    let output = Command::new("nix-collect-garbage")
        .output()
        .context("Failed to run nix-collect-garbage")?;

    let text = String::from_utf8_lossy(&output.stderr).to_string()
        + &String::from_utf8_lossy(&output.stdout);

    let (paths_removed, bytes_freed) = parse_gc_output(&text);

    Ok(GcResult {
        paths_removed,
        bytes_freed,
        output: text,
    })
}

/// Run full garbage collection including old generations (sudo)
pub fn run_gc_full() -> Result<GcResult> {
    let output = Command::new("sudo")
        .args(["nix-collect-garbage", "-d"])
        .output()
        .context("Failed to run sudo nix-collect-garbage -d")?;

    let text = String::from_utf8_lossy(&output.stderr).to_string()
        + &String::from_utf8_lossy(&output.stdout);

    let (paths_removed, bytes_freed) = parse_gc_output(&text);

    Ok(GcResult {
        paths_removed,
        bytes_freed,
        output: text,
    })
}

/// Parse GC output for "N store paths deleted, X MiB freed"
fn parse_gc_output(text: &str) -> (usize, u64) {
    let mut paths_removed = 0usize;
    let mut bytes_freed = 0u64;

    for line in text.lines() {
        let line = line.trim().to_lowercase();

        // Match: "123 store paths deleted, 456.78 MiB freed"
        if line.contains("store paths deleted") || line.contains("store path deleted") {
            // Extract number of paths
            if let Some(num_str) = line.split_whitespace().next() {
                paths_removed = num_str.parse().unwrap_or(0);
            }
        }

        if line.contains("freed") {
            // Extract freed amount: "X.Y MiB freed" or "X.Y GiB freed"
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if *part == "freed" && i >= 2 {
                    let amount: f64 = parts[i - 2].parse().unwrap_or(0.0);
                    let unit = parts[i - 1].to_lowercase();
                    bytes_freed = match unit.as_str() {
                        "kib" => (amount * 1024.0) as u64,
                        "mib" => (amount * 1024.0 * 1024.0) as u64,
                        "gib" => (amount * 1024.0 * 1024.0 * 1024.0) as u64,
                        "tib" => (amount * 1024.0 * 1024.0 * 1024.0 * 1024.0) as u64,
                        _ => 0,
                    };
                    break;
                }
            }
        }
    }

    (paths_removed, bytes_freed)
}

/// Run nix store optimise (hardlink dedup)
pub fn run_optimise() -> Result<OptimiseResult> {
    let output = Command::new("nix")
        .args(["store", "optimise"])
        .output()
        .context("Failed to run nix store optimise")?;

    let text = String::from_utf8_lossy(&output.stderr).to_string()
        + &String::from_utf8_lossy(&output.stdout);

    let bytes_saved = parse_optimise_output(&text);

    Ok(OptimiseResult {
        bytes_saved,
        output: text,
    })
}

/// Parse optimize output for bytes saved
fn parse_optimise_output(text: &str) -> u64 {
    // Format: "X.Y MiB freed by hard-linking N files"
    for line in text.lines() {
        let line = line.trim().to_lowercase();
        if line.contains("freed") && line.contains("hard-linking") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let amount: f64 = parts[0].parse().unwrap_or(0.0);
                let unit = parts[1].to_lowercase();
                return match unit.as_str() {
                    "kib" => (amount * 1024.0) as u64,
                    "mib" => (amount * 1024.0 * 1024.0) as u64,
                    "gib" => (amount * 1024.0 * 1024.0 * 1024.0) as u64,
                    "bytes" | "b" => amount as u64,
                    _ => 0,
                };
            }
        }
    }
    0
}

// ════════════════════════════════════════════════════════════════════
// HISTORY
// ════════════════════════════════════════════════════════════════════

fn history_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|p| p.join("nixmate").join("storage-history.json"))
}

/// Load cleanup history from disk
pub fn load_history() -> Vec<HistoryEntry> {
    let path = match history_path() {
        Some(p) => p,
        None => return Vec::new(),
    };

    if !path.exists() {
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    serde_json::from_str(&content).unwrap_or_default()
}

/// Save a new history entry
pub fn save_history_entry(entry: HistoryEntry) -> Result<()> {
    let path = history_path().context("No data directory")?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut entries = load_history();
    entries.insert(0, entry);

    // Keep last 100 entries
    entries.truncate(100);

    let json = serde_json::to_string_pretty(&entries)?;
    std::fs::write(&path, json)?;

    Ok(())
}

/// Get a summary of history for the dashboard
pub fn history_summary(entries: &[HistoryEntry]) -> (Option<String>, u64) {
    let last_cleanup = entries.first().map(|e| e.timestamp.clone());
    let total_freed: u64 = entries.iter().map(|e| e.freed_bytes).sum();
    (last_cleanup, total_freed)
}

// ════════════════════════════════════════════════════════════════════
// HELPERS
// ════════════════════════════════════════════════════════════════════

/// Run a command with a timeout. Returns None on timeout or error.
fn output_with_timeout(
    cmd: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Option<std::process::Output> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    let timeout = Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().ok();
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
