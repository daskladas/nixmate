//! System detection for NixOS and Home-Manager

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

/// Information about the detected system configuration
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub hostname: String,
    pub username: String,
    pub uses_flakes: bool,
    pub system_profile: PathBuf,
    pub home_manager: Option<HomeManagerInfo>,
}

/// Home-Manager installation info
#[derive(Debug, Clone)]
pub struct HomeManagerInfo {
    pub profile_path: PathBuf,
    #[allow(dead_code)] // Parsed during detection, reserved for HM module
    pub is_standalone: bool,
}

/// Detect system configuration
pub fn detect_system() -> Result<SystemInfo> {
    let hostname = get_hostname()?;
    let username = get_username()?;
    let uses_flakes = detect_flakes();
    let system_profile = PathBuf::from("/nix/var/nix/profiles/system");
    let home_manager = detect_home_manager(&username);

    Ok(SystemInfo {
        hostname,
        username,
        uses_flakes,
        system_profile,
        home_manager,
    })
}

fn get_hostname() -> Result<String> {
    if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
        let hostname = hostname.trim().to_string();
        if !hostname.is_empty() {
            return Ok(hostname);
        }
    }

    let output = std::process::Command::new("hostname")
        .output()
        .context("Failed to get hostname")?;

    let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if hostname.is_empty() {
        Ok("unknown".to_string())
    } else {
        Ok(hostname)
    }
}

fn get_username() -> Result<String> {
    env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .context("Could not determine username from USER or LOGNAME environment variable")
}

pub fn detect_flakes() -> bool {
    let home = env::var("HOME").unwrap_or_default();
    let flake_paths = [
        PathBuf::from("/etc/nixos/flake.nix"),
        PathBuf::from(&home).join(".config/nixos/flake.nix"),
        PathBuf::from(&home).join("nixos/flake.nix"),
        PathBuf::from(&home).join(".nixos/flake.nix"),
    ];
    flake_paths.iter().any(|p| p.exists())
}

/// Find the directory containing flake.nix (checks common locations)
pub fn find_flake_path() -> Option<String> {
    let home = env::var("HOME").unwrap_or_default();
    let candidates = [
        "/etc/nixos/flake.nix".to_string(),
        format!("{}/.config/nixos/flake.nix", home),
        format!("{}/nixos/flake.nix", home),
        format!("{}/.nixos/flake.nix", home),
    ];

    for candidate in &candidates {
        let path = Path::new(candidate);
        if path.exists() {
            return path.parent().map(|p| p.to_string_lossy().to_string());
        }
    }
    None
}

fn detect_home_manager(username: &str) -> Option<HomeManagerInfo> {
    let home = env::var("HOME").ok()?;
    let standalone_path = PathBuf::from(&home).join(".local/state/home-manager/profiles");

    if standalone_path.exists() && has_generation_links(&standalone_path) {
        return Some(HomeManagerInfo {
            profile_path: standalone_path,
            is_standalone: true,
        });
    }

    let module_path = PathBuf::from("/nix/var/nix/profiles/per-user")
        .join(username)
        .join("home-manager");

    if module_path.exists() || module_path.is_symlink() {
        let profile_dir = module_path.parent()?;
        return Some(HomeManagerInfo {
            profile_path: profile_dir.to_path_buf(),
            is_standalone: false,
        });
    }

    let alt_standalone = PathBuf::from(&home).join(".nix-profile");
    if alt_standalone.exists() {
        let alt_state = PathBuf::from(&home).join(".local/state/nix/profiles/home-manager");
        if alt_state.exists() {
            return Some(HomeManagerInfo {
                profile_path: alt_state.parent()?.to_path_buf(),
                is_standalone: true,
            });
        }
    }

    None
}

fn has_generation_links(path: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("home-manager-") && name_str.ends_with("-link") {
                return true;
            }
        }
    }
    false
}
