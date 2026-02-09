//! Generation listing and parsing
//!
//! Handles listing generations for both System and Home-Manager profiles.
//! Uses TWO strategies:
//!   1. Filesystem-based (no permissions needed) — reads symlinks directly
//!   2. nix-env fallback (if filesystem parsing fails)

use crate::types::{Generation, ProfileType};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, TimeZone};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Source of generations (which profile)
#[derive(Debug, Clone)]
pub struct GenerationSource {
    pub profile_type: ProfileType,
    pub profile_path: PathBuf,
}

/// List all generations for a given profile.
/// Tries filesystem-based listing first (no root needed),
/// falls back to nix-env if that fails.
pub fn list_generations(source: &GenerationSource) -> Result<Vec<Generation>> {
    // Strategy 1: Parse from filesystem (no permissions needed)
    match list_generations_from_fs(source) {
        Ok(gens) if !gens.is_empty() => return Ok(gens),
        Ok(_) => {} // empty, try fallback
        Err(_) => {} // failed, try fallback
    }

    // Strategy 2: Use nix-env (may need permissions)
    list_generations_from_nix_env(source)
}

/// Parse generations by reading symlinks from the profile directory.
/// This does NOT require root and works for both system and HM profiles.
fn list_generations_from_fs(source: &GenerationSource) -> Result<Vec<Generation>> {
    let profile_dir = source
        .profile_path
        .parent()
        .unwrap_or(Path::new("/nix/var/nix/profiles"));

    let prefix = match source.profile_type {
        ProfileType::System => "system-",
        ProfileType::HomeManager => "home-manager-",
    };
    let suffix = "-link";

    // Read directory and find all generation symlinks
    let entries = std::fs::read_dir(profile_dir)
        .with_context(|| format!("Cannot read profile directory: {:?}", profile_dir))?;

    let current_id = get_current_generation_id(&source.profile_path).unwrap_or(0);

    let boot_entries = if source.profile_type == ProfileType::System {
        get_boot_entries().unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut generations = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Match pattern: system-142-link or home-manager-89-link
        if !name_str.starts_with(prefix) || !name_str.ends_with(suffix) {
            continue;
        }

        // Extract generation ID from filename
        let id_str = &name_str[prefix.len()..name_str.len() - suffix.len()];
        let id: u32 = match id_str.parse() {
            Ok(id) => id,
            Err(_) => continue,
        };

        let gen_path = profile_dir.join(&*name_str);

        // Get timestamp from symlink metadata (modification time)
        let timestamp = get_link_timestamp(&gen_path);

        let generation = parse_generation(
            id,
            timestamp,
            &gen_path,
            id == current_id,
            boot_entries.contains(&id),
            source.profile_type,
        )?;

        generations.push(generation);
    }

    // Sort by ID descending (newest first)
    generations.sort_by(|a, b| b.id.cmp(&a.id));

    Ok(generations)
}

/// Get timestamp from a symlink (uses lstat metadata)
fn get_link_timestamp(path: &Path) -> DateTime<Local> {
    // Try symlink metadata first, then regular metadata
    let metadata = std::fs::symlink_metadata(path)
        .or_else(|_| std::fs::metadata(path));

    match metadata {
        Ok(meta) => {
            if let Ok(modified) = meta.modified() {
                DateTime::<Local>::from(modified)
            } else {
                Local::now()
            }
        }
        Err(_) => Local::now(),
    }
}

/// Fallback: list generations using nix-env
fn list_generations_from_nix_env(source: &GenerationSource) -> Result<Vec<Generation>> {
    let profile_path = &source.profile_path;

    let output = Command::new("nix-env")
        .args(["--list-generations", "--profile"])
        .arg(profile_path)
        .output()
        .context("Failed to run nix-env --list-generations")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nix-env failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw_generations = parse_generation_list(&stdout)?;

    let current_id = get_current_generation_id(profile_path).unwrap_or(0);

    let boot_entries = if source.profile_type == ProfileType::System {
        get_boot_entries().unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut generations = Vec::new();
    for (id, timestamp) in raw_generations {
        let gen_path = get_generation_path(profile_path, id, source.profile_type);

        if !gen_path.exists() {
            continue;
        }

        let generation = parse_generation(
            id,
            timestamp,
            &gen_path,
            id == current_id,
            boot_entries.contains(&id),
            source.profile_type,
        )?;

        generations.push(generation);
    }

    generations.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(generations)
}

/// Parse nix-env --list-generations output
fn parse_generation_list(output: &str) -> Result<Vec<(u32, DateTime<Local>)>> {
    let mut result = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let id: u32 = parts[0]
            .parse()
            .with_context(|| format!("Invalid generation ID: {}", parts[0]))?;

        let datetime_str = format!("{} {}", parts[1], parts[2]);
        let timestamp = parse_datetime(&datetime_str)?;

        result.push((id, timestamp));
    }

    Ok(result)
}

fn parse_datetime(s: &str) -> Result<DateTime<Local>> {
    let naive = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .with_context(|| format!("Failed to parse datetime: {}", s))?;

    Ok(Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(Local::now))
}

/// Get the current generation ID by reading the profile symlink
fn get_current_generation_id(profile_path: &Path) -> Result<u32> {
    let target = std::fs::read_link(profile_path)
        .with_context(|| format!("Failed to read profile symlink: {:?}", profile_path))?;

    extract_generation_id(&target)
}

/// Extract generation ID from a path like "system-142-link"
fn extract_generation_id(path: &Path) -> Result<u32> {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid generation path")?;

    // Pattern: name-ID-link (e.g., "system-142-link" or "home-manager-89-link")
    let parts: Vec<&str> = filename.rsplitn(3, '-').collect();
    if parts.len() >= 2 && parts[0] == "link" {
        parts[1]
            .parse()
            .with_context(|| format!("Invalid generation ID in path: {}", filename))
    } else {
        anyhow::bail!("Could not extract generation ID from: {}", filename)
    }
}

fn get_generation_path(profile_path: &Path, id: u32, profile_type: ProfileType) -> PathBuf {
    let parent = profile_path.parent().unwrap_or(Path::new("/"));
    let name = match profile_type {
        ProfileType::System => format!("system-{}-link", id),
        ProfileType::HomeManager => format!("home-manager-{}-link", id),
    };
    parent.join(name)
}

/// Parse a single generation's metadata from the filesystem
fn parse_generation(
    id: u32,
    timestamp: DateTime<Local>,
    gen_path: &Path,
    is_current: bool,
    in_bootloader: bool,
    profile_type: ProfileType,
) -> Result<Generation> {
    let nixos_version = get_version(gen_path, profile_type);
    let kernel_version = if profile_type == ProfileType::System {
        get_kernel_version(gen_path)
    } else {
        None
    };
    let package_count = get_package_count(gen_path);
    let closure_size = get_closure_size(gen_path).unwrap_or(0);
    let store_path = std::fs::read_link(gen_path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(Generation {
        id,
        date: timestamp,
        is_current,
        nixos_version,
        kernel_version,
        package_count,
        closure_size,
        store_path,
        is_pinned: false,
        in_bootloader,
    })
}

fn get_version(gen_path: &Path, profile_type: ProfileType) -> Option<String> {
    let version_file = match profile_type {
        ProfileType::System => gen_path.join("nixos-version"),
        ProfileType::HomeManager => gen_path.join("hm-version"),
    };

    if version_file.exists() {
        std::fs::read_to_string(&version_file)
            .ok()
            .map(|s| s.trim().to_string())
    } else {
        std::fs::read_link(gen_path).ok().and_then(|p| {
            let s = p.to_string_lossy();
            if let Some(idx) = s.find("-nixos-system-") {
                let rest = &s[idx + 14..];
                rest.split('-').nth(1).map(|v| v.to_string())
            } else {
                None
            }
        })
    }
}

fn get_kernel_version(gen_path: &Path) -> Option<String> {
    let kernel_dir = gen_path.join("kernel");

    if kernel_dir.exists() {
        std::fs::read_link(&kernel_dir).ok().and_then(|p| {
            let s = p.to_string_lossy();
            for part in s.split('/') {
                if part.starts_with("linux-") && part.len() > 6 {
                    return Some(part[6..].split('-').next()?.to_string());
                }
            }
            None
        })
    } else {
        let modules_dir = gen_path.join("kernel-modules/lib/modules");
        if modules_dir.exists() {
            std::fs::read_dir(&modules_dir).ok().and_then(|mut entries| {
                entries
                    .next()?
                    .ok()
                    .map(|e| e.file_name().to_string_lossy().to_string())
            })
        } else {
            None
        }
    }
}

fn get_package_count(gen_path: &Path) -> usize {
    // Try sw/bin first (system generations)
    let sw_path = gen_path.join("sw/bin");
    if sw_path.exists() {
        return std::fs::read_dir(&sw_path)
            .map(|entries| entries.count())
            .unwrap_or(0);
    }

    // Try home-manager manifest
    let manifest = gen_path.join("home-files/.nix-profile/manifest.nix");
    if manifest.exists() {
        return std::fs::read_to_string(&manifest)
            .map(|s| s.matches("name = ").count())
            .unwrap_or(0);
    }

    0
}

fn get_closure_size(gen_path: &Path) -> Result<u64> {
    // Try nix path-info -S (may fail without permissions, that's OK)
    let output = Command::new("nix")
        .args(["path-info", "-S"])
        .arg(gen_path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(size) = parts[1].parse::<u64>() {
                        return Ok(size);
                    }
                }
            }
            Ok(0)
        }
        _ => Ok(0), // Graceful: no size info available
    }
}

fn get_boot_entries() -> Result<Vec<u32>> {
    let mut entries = Vec::new();

    // Check systemd-boot entries
    let loader_entries = Path::new("/boot/loader/entries");
    if loader_entries.exists() {
        if let Ok(dir) = std::fs::read_dir(loader_entries) {
            for entry in dir.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("nixos-generation-") && name_str.ends_with(".conf") {
                    let id_str = &name_str[17..name_str.len() - 5];
                    if let Ok(id) = id_str.parse() {
                        entries.push(id);
                    }
                }
            }
        }
    }

    // Check GRUB entries
    let grub_cfg = Path::new("/boot/grub/grub.cfg");
    if grub_cfg.exists() && entries.is_empty() {
        if let Ok(content) = std::fs::read_to_string(grub_cfg) {
            for line in content.lines() {
                if line.contains("NixOS") && line.contains("Generation") {
                    if let Some(start) = line.find("Generation ") {
                        let rest = &line[start + 11..];
                        let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if let Ok(id) = num.parse() {
                            entries.push(id);
                        }
                    }
                }
            }
        }
    }

    Ok(entries)
}
