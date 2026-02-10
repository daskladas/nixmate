//! Core data types shared across all modules
//!
//! Types used by the nix backend and the generations module.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;

/// A temporary UI message shown to the user (e.g. success/error notifications)
#[derive(Clone)]
pub struct FlashMessage {
    pub text: String,
    pub is_error: bool,
    pub created: Instant,
}

impl FlashMessage {
    pub fn new(text: String, is_error: bool) -> Self {
        Self {
            text,
            is_error,
            created: Instant::now(),
        }
    }

    pub fn is_expired(&self, seconds: u64) -> bool {
        self.created.elapsed().as_secs() >= seconds
    }
}

/// Represents a NixOS or Home-Manager generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub id: u32,
    pub date: DateTime<Local>,
    pub is_current: bool,
    pub nixos_version: Option<String>,
    pub kernel_version: Option<String>,
    pub package_count: usize,
    pub closure_size: u64,
    pub store_path: String,
    pub is_pinned: bool,
    pub in_bootloader: bool,
}

impl Generation {
    pub fn formatted_date(&self) -> String {
        self.date.format("%d.%m.%y %H:%M").to_string()
    }

    pub fn formatted_size(&self) -> String {
        format_bytes(self.closure_size)
    }
}

/// Represents a package in a generation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub size: u64,
}

impl Package {
    pub fn formatted_size(&self) -> String {
        format_bytes(self.size)
    }
}

/// Result of comparing two generations
#[derive(Debug, Clone, Default)]
pub struct GenerationDiff {
    pub added: Vec<Package>,
    pub removed: Vec<Package>,
    pub updated: Vec<PackageUpdate>,
}

impl GenerationDiff {
    pub fn calculate(old_packages: &[Package], new_packages: &[Package]) -> Self {
        let old_set: HashSet<&str> = old_packages.iter().map(|p| p.name.as_str()).collect();
        let new_set: HashSet<&str> = new_packages.iter().map(|p| p.name.as_str()).collect();

        let added: Vec<Package> = new_packages
            .iter()
            .filter(|p| !old_set.contains(p.name.as_str()))
            .cloned()
            .collect();

        let removed: Vec<Package> = old_packages
            .iter()
            .filter(|p| !new_set.contains(p.name.as_str()))
            .cloned()
            .collect();

        let mut updated = Vec::new();
        for new_pkg in new_packages {
            if let Some(old_pkg) = old_packages.iter().find(|p| p.name == new_pkg.name) {
                if old_pkg.version != new_pkg.version {
                    updated.push(PackageUpdate {
                        name: new_pkg.name.clone(),
                        old_version: old_pkg.version.clone(),
                        new_version: new_pkg.version.clone(),
                        is_kernel: new_pkg.name.starts_with("linux-"),
                        is_security: is_security_package(&new_pkg.name),
                    });
                }
            }
        }

        Self {
            added,
            removed,
            updated,
        }
    }
}

/// Represents a package version update
#[derive(Debug, Clone)]
pub struct PackageUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub is_kernel: bool,
    pub is_security: bool,
}

/// Profile type (System or Home-Manager)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileType {
    System,
    HomeManager,
}

impl ProfileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProfileType::System => "System",
            ProfileType::HomeManager => "Home-Manager",
        }
    }
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn is_security_package(name: &str) -> bool {
    let security_packages = [
        "openssl",
        "openssh",
        "gnupg",
        "gpg",
        "sudo",
        "polkit",
        "pam",
        "shadow",
        "nss",
        "ca-certificates",
        "curl",
        "wget",
    ];
    security_packages.iter().any(|s| name.contains(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_generation_diff_calculate() {
        let old = vec![
            Package {
                name: "firefox".into(),
                version: "120".into(),
                size: 0,
            },
            Package {
                name: "vim".into(),
                version: "9.0".into(),
                size: 0,
            },
            Package {
                name: "git".into(),
                version: "2.42".into(),
                size: 0,
            },
        ];
        let new = vec![
            Package {
                name: "firefox".into(),
                version: "121".into(),
                size: 0,
            },
            Package {
                name: "git".into(),
                version: "2.42".into(),
                size: 0,
            },
            Package {
                name: "ripgrep".into(),
                version: "14".into(),
                size: 0,
            },
        ];
        let diff = GenerationDiff::calculate(&old, &new);
        assert_eq!(diff.added.len(), 1); // ripgrep
        assert_eq!(diff.removed.len(), 1); // vim
        assert_eq!(diff.updated.len(), 1); // firefox
        assert_eq!(diff.updated[0].name, "firefox");
    }
    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }
    #[test]
    fn test_flash_message_expiry() {
        let msg = FlashMessage::new("test".into(), false);
        assert!(!msg.is_expired(3));
        // Can't easily test expiry without sleep, just verify creation works
        assert_eq!(msg.text, "test");
        assert!(!msg.is_error);
    }
}
