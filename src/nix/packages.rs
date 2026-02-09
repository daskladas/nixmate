//! Package extraction from generations

use crate::types::Package;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Get all packages in a generation
pub fn get_packages(gen_path: &Path) -> Result<Vec<Package>> {
    if let Ok(packages) = get_packages_from_path_info(gen_path) {
        if !packages.is_empty() {
            return Ok(packages);
        }
    }

    if let Ok(packages) = get_packages_from_sw(gen_path) {
        if !packages.is_empty() {
            return Ok(packages);
        }
    }

    Ok(Vec::new())
}

fn get_packages_from_path_info(gen_path: &Path) -> Result<Vec<Package>> {
    let output = Command::new("nix")
        .args(["path-info", "-r", "-s", "--json"])
        .arg(gen_path)
        .output()
        .context("Failed to run nix path-info")?;

    if !output.status.success() {
        anyhow::bail!("nix path-info failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_path_info_json(&stdout)
}

fn parse_path_info_json(json_str: &str) -> Result<Vec<Package>> {
    let data: HashMap<String, serde_json::Value> =
        serde_json::from_str(json_str).context("Failed to parse nix path-info JSON")?;

    let mut packages: Vec<Package> = Vec::new();
    let mut seen_names: HashMap<String, usize> = HashMap::new();

    for (path, info) in data {
        if let Some((name, version)) = parse_store_path(&path) {
            if should_skip_package(&name) {
                continue;
            }

            let size = info.get("narSize").and_then(|v| v.as_u64()).unwrap_or(0);

            if let Some(&idx) = seen_names.get(&name) {
                if packages[idx].size < size {
                    packages[idx] = Package {
                        name: name.clone(),
                        version,
                        size,
                    };
                }
            } else {
                seen_names.insert(name.clone(), packages.len());
                packages.push(Package { name, version, size });
            }
        }
    }

    packages.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(packages)
}

fn parse_store_path(path: &str) -> Option<(String, String)> {
    let filename = path.rsplit('/').next()?;

    if filename.len() <= 33 {
        return None;
    }
    let name_version = &filename[33..];

    let mut split_pos = None;
    let chars: Vec<char> = name_version.chars().collect();

    for i in (1..chars.len()).rev() {
        if chars[i - 1] == '-' && chars[i].is_ascii_digit() {
            split_pos = Some(i - 1);
            break;
        }
    }

    if let Some(pos) = split_pos {
        let name = name_version[..pos].to_string();
        let version = name_version[pos + 1..].to_string();
        Some((name, version))
    } else {
        Some((name_version.to_string(), "".to_string()))
    }
}

fn should_skip_package(name: &str) -> bool {
    let skip_prefixes = [
        "bootstrap-",
        "hook-",
        "wrap-",
        "setup-",
        "stdenv-",
        "builder-",
        "source-",
        "raw-",
        "manifest",
        "env-manifest",
        "nix-support",
    ];

    let skip_suffixes = ["-info", "-man", "-doc", "-dev", "-debug", ".drv"];

    let skip_names = ["source", "builder", "hook", "wrapper", "nixos-system-"];

    for prefix in skip_prefixes {
        if name.starts_with(prefix) {
            return true;
        }
    }
    for suffix in skip_suffixes {
        if name.ends_with(suffix) {
            return true;
        }
    }
    for skip_name in skip_names {
        if name == skip_name || name.starts_with(skip_name) {
            return true;
        }
    }

    false
}

fn get_packages_from_sw(gen_path: &Path) -> Result<Vec<Package>> {
    let sw_path = gen_path.join("sw");
    if !sw_path.exists() {
        return Ok(Vec::new());
    }

    let manifest_path = sw_path.join("manifest.nix");
    if manifest_path.exists() {
        return parse_manifest(&manifest_path);
    }

    let bin_path = sw_path.join("bin");
    if bin_path.exists() {
        let mut packages = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&bin_path) {
            for entry in entries.flatten() {
                if let Ok(target) = std::fs::read_link(entry.path()) {
                    if let Some((name, version)) = parse_store_path(&target.to_string_lossy()) {
                        if !packages.iter().any(|p: &Package| p.name == name) {
                            packages.push(Package {
                                name,
                                version,
                                size: 0,
                            });
                        }
                    }
                }
            }
        }
        packages.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        return Ok(packages);
    }

    Ok(Vec::new())
}

fn parse_manifest(path: &Path) -> Result<Vec<Package>> {
    let content = std::fs::read_to_string(path).context("Failed to read manifest")?;

    let mut packages = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("name = \"") {
            let name = line
                .trim_start_matches("name = \"")
                .trim_end_matches("\";")
                .to_string();

            if !should_skip_package(&name) {
                if let Some((pkg_name, version)) =
                    parse_store_path(&format!("/nix/store/xxxxxxxx-{}", name))
                {
                    packages.push(Package {
                        name: pkg_name,
                        version,
                        size: 0,
                    });
                }
            }
        }
    }

    packages.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(packages)
}
