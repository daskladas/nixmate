//! Command execution for restore and delete operations

use crate::types::ProfileType;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

/// Result of a command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
    #[allow(dead_code)] // Stored for logging
    pub command: String,
}

/// Restore (switch to) a specific generation
pub fn restore_generation(
    profile_path: &Path,
    generation_id: u32,
    profile_type: ProfileType,
    dry_run: bool,
) -> Result<CommandResult> {
    let (program, args) = build_restore_command(profile_path, generation_id, profile_type);
    let command = format!("{} {}", program, args.join(" "));

    if dry_run {
        return Ok(CommandResult {
            success: true,
            message: format!(
                "Dry run: Would execute restore to generation {}",
                generation_id
            ),
            command,
        });
    }

    execute_sudo_command(
        &program,
        &args,
        &format!("restore generation {}", generation_id),
    )
}

/// Delete one or more generations
pub fn delete_generations(
    profile_path: &Path,
    generation_ids: &[u32],
    profile_type: ProfileType,
    dry_run: bool,
) -> Result<CommandResult> {
    if generation_ids.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: "No generations specified for deletion".to_string(),
            command: String::new(),
        });
    }

    let (program, args) = build_delete_command(profile_path, generation_ids, profile_type);
    let command = format!("{} {}", program, args.join(" "));

    if dry_run {
        return Ok(CommandResult {
            success: true,
            message: format!(
                "Dry run: Would delete {} generation(s)",
                generation_ids.len()
            ),
            command,
        });
    }

    execute_sudo_command(
        &program,
        &args,
        &format!("delete {} generation(s)", generation_ids.len()),
    )
}

fn build_restore_command(
    profile_path: &Path,
    generation_id: u32,
    profile_type: ProfileType,
) -> (String, Vec<String>) {
    match profile_type {
        ProfileType::System => {
            let gen_path = profile_path
                .parent()
                .unwrap_or(Path::new("/nix/var/nix/profiles"))
                .join(format!("system-{}-link", generation_id));
            let switch_bin = format!("{}/bin/switch-to-configuration", gen_path.display());
            ("sudo".into(), vec![switch_bin, "switch".into()])
        }
        ProfileType::HomeManager => {
            let home = std::env::var("HOME").unwrap_or_default();
            let gen_path = format!(
                "{}/.local/state/home-manager/profiles/home-manager-{}-link",
                home, generation_id
            );
            if Path::new(&gen_path).exists() {
                let activate = format!("{}/activate", gen_path);
                (activate, vec![])
            } else {
                (
                    "nix-env".into(),
                    vec![
                        "--switch-generation".into(),
                        generation_id.to_string(),
                        "--profile".into(),
                        profile_path.display().to_string(),
                    ],
                )
            }
        }
    }
}

fn build_delete_command(
    profile_path: &Path,
    generation_ids: &[u32],
    profile_type: ProfileType,
) -> (String, Vec<String>) {
    let ids_str: Vec<String> = generation_ids.iter().map(|id| id.to_string()).collect();

    match profile_type {
        ProfileType::System => {
            let mut args = vec!["nix-env".into(), "--delete-generations".into()];
            args.extend(ids_str);
            args.push("--profile".into());
            args.push(profile_path.display().to_string());
            ("sudo".into(), args)
        }
        ProfileType::HomeManager => {
            if command_exists("home-manager") {
                let mut args = vec!["remove-generations".into()];
                args.extend(ids_str);
                ("home-manager".into(), args)
            } else {
                let mut args = vec!["--delete-generations".into()];
                args.extend(ids_str);
                args.push("--profile".into());
                args.push(profile_path.display().to_string());
                ("nix-env".into(), args)
            }
        }
    }
}

fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn execute_sudo_command(
    program: &str,
    args: &[String],
    description: &str,
) -> Result<CommandResult> {
    let display_cmd = format!("{} {}", program, args.join(" "));

    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute: {}", display_cmd))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(CommandResult {
            success: true,
            message: format!("Successfully {}", description),
            command: display_cmd,
        })
    } else {
        let error_msg = if !stderr.is_empty() {
            stderr.trim().to_string()
        } else if !stdout.is_empty() {
            stdout.trim().to_string()
        } else {
            format!("Command failed with exit code: {:?}", output.status.code())
        };

        Ok(CommandResult {
            success: false,
            message: format!("Failed to {}: {}", description, error_msg),
            command: display_cmd,
        })
    }
}

/// Get the command that would be executed for restore (for display in confirmation)
pub fn get_restore_command_preview(
    profile_path: &Path,
    generation_id: u32,
    profile_type: ProfileType,
) -> String {
    let (program, args) = build_restore_command(profile_path, generation_id, profile_type);
    format!("{} {}", program, args.join(" "))
}

/// Get the command that would be executed for delete (for display in confirmation)
pub fn get_delete_command_preview(
    profile_path: &Path,
    generation_ids: &[u32],
    profile_type: ProfileType,
) -> String {
    let (program, args) = build_delete_command(profile_path, generation_ids, profile_type);
    format!("{} {}", program, args.join(" "))
}
