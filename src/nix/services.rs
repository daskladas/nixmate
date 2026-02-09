//! Services & Ports backend
//!
//! Gathers a unified view of what's running on a NixOS server:
//! - systemd services (systemctl)
//! - Docker containers (docker ps)
//! - Podman containers (podman ps)
//! - Listening ports (ss) with mapping to services/containers
//!
//! No sudo needed for read operations.
//! Sudo only for service management actions (start/stop/restart/enable/disable).

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

// ═══════════════════════════════════════
//  DATA TYPES
// ═══════════════════════════════════════

/// A unified "running thing" — either a systemd service or a container
#[derive(Debug, Clone)]
pub struct ServiceEntry {
    pub kind: EntryKind,
    pub name: String,
    pub display_name: String,
    pub status: RunState,
    pub enabled: EnableState,
    pub description: String,
    pub pid: Option<u32>,
    pub memory: Option<String>,
    pub uptime: Option<String>,
    /// Ports this entry is listening on (filled in after port scan)
    pub ports: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Systemd,
    Docker,
    Podman,
}

impl EntryKind {
    pub fn label(&self) -> &'static str {
        match self {
            EntryKind::Systemd => "systemd",
            EntryKind::Docker => "docker",
            EntryKind::Podman => "podman",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            EntryKind::Systemd => "⚙",
            EntryKind::Docker => "🐳",
            EntryKind::Podman => "⬡",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Running,
    Stopped,
    Failed,
    Restarting,
    Paused,
    Created,
    Exited,
    Unknown,
}

impl RunState {
    pub fn symbol(&self) -> &'static str {
        match self {
            RunState::Running => "●",
            RunState::Stopped | RunState::Exited => "○",
            RunState::Failed => "✗",
            RunState::Restarting => "↻",
            RunState::Paused => "⏸",
            RunState::Created => "◌",
            RunState::Unknown => "?",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, RunState::Running | RunState::Restarting)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnableState {
    Enabled,
    Disabled,
    Static,
    Masked,
    NotApplicable,
    Unknown,
}

impl EnableState {
    pub fn as_str(&self) -> &'static str {
        match self {
            EnableState::Enabled => "enabled",
            EnableState::Disabled => "disabled",
            EnableState::Static => "static",
            EnableState::Masked => "masked",
            EnableState::NotApplicable => "n/a",
            EnableState::Unknown => "?",
        }
    }
}

/// A listening port with info about what owns it
#[derive(Debug, Clone)]
pub struct PortEntry {
    pub protocol: String,
    pub port: u16,
    pub address: String,
    pub process_name: String,
    pub pid: Option<u32>,
    /// Resolved: which service/container this port belongs to
    pub owner: String,
    pub owner_kind: EntryKind,
}

/// Available management actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}

impl ServiceAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceAction::Start => "start",
            ServiceAction::Stop => "stop",
            ServiceAction::Restart => "restart",
            ServiceAction::Enable => "enable",
            ServiceAction::Disable => "disable",
        }
    }

    pub fn needs_sudo(&self, kind: EntryKind) -> bool {
        // Containers don't need sudo. Systemd always does.
        kind == EntryKind::Systemd
    }

    /// Whether this action is valid for a given entry kind
    pub fn valid_for(&self, kind: EntryKind) -> bool {
        match self {
            ServiceAction::Start | ServiceAction::Stop | ServiceAction::Restart => true,
            ServiceAction::Enable | ServiceAction::Disable => kind == EntryKind::Systemd,
        }
    }
}

/// Summary stats for the overview dashboard
#[derive(Debug, Clone, Default)]
pub struct DashboardStats {
    pub services_running: usize,
    pub services_failed: usize,
    pub services_total: usize,
    pub containers_running: usize,
    pub containers_stopped: usize,
    pub containers_total: usize,
    pub ports_open: usize,
    pub has_docker: bool,
    pub has_podman: bool,
}

// ═══════════════════════════════════════
//  DATA LOADING
// ═══════════════════════════════════════

/// Load all server data: services, containers, ports — then cross-reference
pub fn load_dashboard() -> Result<(Vec<ServiceEntry>, Vec<PortEntry>, DashboardStats)> {
    // 1. Gather systemd services
    let mut entries = list_systemd_services().unwrap_or_default();

    // 2. Gather containers
    let has_docker = tool_available("docker");
    let has_podman = tool_available("podman");

    if has_docker {
        entries.extend(list_docker_containers().unwrap_or_default());
    }
    if has_podman {
        entries.extend(list_podman_containers().unwrap_or_default());
    }

    // 3. Gather open ports
    let mut ports = list_ports().unwrap_or_default();

    // 4. Cross-reference: map ports → entries, and entries → ports
    cross_reference(&mut entries, &mut ports);

    // 5. Sort: failed first, then running, then rest
    entries.sort_by(|a, b| {
        fn rank(r: &RunState) -> u8 {
            match r {
                RunState::Failed => 0,
                RunState::Running => 1,
                RunState::Restarting => 2,
                RunState::Paused => 3,
                RunState::Created => 4,
                RunState::Stopped | RunState::Exited => 5,
                RunState::Unknown => 6,
            }
        }
        rank(&a.status)
            .cmp(&rank(&b.status))
            .then(a.kind.label().cmp(b.kind.label()))
            .then(a.display_name.cmp(&b.display_name))
    });

    // 6. Compute stats
    let stats = DashboardStats {
        services_running: entries
            .iter()
            .filter(|e| e.kind == EntryKind::Systemd && e.status.is_active())
            .count(),
        services_failed: entries
            .iter()
            .filter(|e| e.kind == EntryKind::Systemd && e.status == RunState::Failed)
            .count(),
        services_total: entries
            .iter()
            .filter(|e| e.kind == EntryKind::Systemd)
            .count(),
        containers_running: entries
            .iter()
            .filter(|e| matches!(e.kind, EntryKind::Docker | EntryKind::Podman) && e.status.is_active())
            .count(),
        containers_stopped: entries
            .iter()
            .filter(|e| {
                matches!(e.kind, EntryKind::Docker | EntryKind::Podman) && !e.status.is_active()
            })
            .count(),
        containers_total: entries
            .iter()
            .filter(|e| matches!(e.kind, EntryKind::Docker | EntryKind::Podman))
            .count(),
        ports_open: ports.len(),
        has_docker,
        has_podman,
    };

    Ok((entries, ports, stats))
}

/// Cross-reference ports with services/containers by PID matching
fn cross_reference(entries: &mut [ServiceEntry], ports: &mut [PortEntry]) {
    // Build PID → index map
    let mut pid_map: HashMap<u32, usize> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        if let Some(pid) = entry.pid {
            pid_map.insert(pid, i);
        }
    }

    for port in ports.iter_mut() {
        // Try PID match first
        let matched_idx = if let Some(pid) = port.pid {
            pid_map.get(&pid).copied()
        } else {
            None
        };

        // Fallback: process name match
        let matched_idx = matched_idx.or_else(|| {
            let proc_lower = port.process_name.to_lowercase();
            if proc_lower == "-" || proc_lower.is_empty() {
                return None;
            }
            entries.iter().position(|e| {
                e.display_name.to_lowercase() == proc_lower
                    || e.display_name.to_lowercase().contains(&proc_lower)
                    || proc_lower.contains(&e.display_name.to_lowercase())
            })
        });

        if let Some(idx) = matched_idx {
            port.owner = entries[idx].display_name.clone();
            port.owner_kind = entries[idx].kind;
            // Also register port on the entry (if not already from docker port map)
            if !entries[idx].ports.contains(&port.port) {
                entries[idx].ports.push(port.port);
            }
        }
    }
}

// ── systemd ──

fn list_systemd_services() -> Result<Vec<ServiceEntry>> {
    let output = Command::new("systemctl")
        .args([
            "list-units",
            "--type=service",
            "--all",
            "--no-pager",
            "--no-legend",
            "--plain",
        ])
        .output()
        .context("Failed to run systemctl list-units")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let enable_states = fetch_enable_states();
    let mut services = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let tokens: Vec<&str> = line
            .splitn(5, char::is_whitespace)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if tokens.len() < 4 {
            continue;
        }

        let unit_name = tokens[0];
        if !unit_name.ends_with(".service") {
            continue;
        }

        let display = unit_name.trim_end_matches(".service").to_string();
        let active = tokens[2];
        let sub = tokens[3];
        let desc = if tokens.len() > 4 {
            tokens[4].to_string()
        } else {
            String::new()
        };

        let status = match (active, sub) {
            (_, "running") => RunState::Running,
            ("failed", _) | (_, "failed") => RunState::Failed,
            (_, "activating" | "auto-restart" | "start") => RunState::Restarting,
            ("inactive", _) | (_, "dead" | "exited") => RunState::Stopped,
            _ => RunState::Unknown,
        };

        let enabled = enable_states
            .get(unit_name)
            .copied()
            .unwrap_or(EnableState::Unknown);

        services.push(ServiceEntry {
            kind: EntryKind::Systemd,
            name: unit_name.to_string(),
            display_name: display,
            status,
            enabled,
            description: desc,
            pid: None,
            memory: None,
            uptime: None,
            ports: Vec::new(),
        });
    }

    fill_systemd_pids(&mut services);
    Ok(services)
}

fn fetch_enable_states() -> HashMap<String, EnableState> {
    let mut map = HashMap::new();
    let Ok(output) = Command::new("systemctl")
        .args([
            "list-unit-files",
            "--type=service",
            "--no-pager",
            "--no-legend",
            "--plain",
        ])
        .output()
    else {
        return map;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let state = match parts[1] {
                "enabled" => EnableState::Enabled,
                "disabled" => EnableState::Disabled,
                "static" => EnableState::Static,
                "masked" => EnableState::Masked,
                _ => EnableState::Unknown,
            };
            map.insert(parts[0].to_string(), state);
        }
    }
    map
}

fn fill_systemd_pids(services: &mut [ServiceEntry]) {
    let running: Vec<String> = services
        .iter()
        .filter(|s| s.status == RunState::Running)
        .map(|s| s.name.clone())
        .collect();

    if running.is_empty() {
        return;
    }

    for chunk in running.chunks(50) {
        let mut args: Vec<&str> =
            vec!["show", "--property=Id,MainPID,MemoryCurrent,ActiveEnterTimestamp"];
        for name in chunk {
            args.push(name);
        }
        args.push("--no-pager");

        let Ok(output) = Command::new("systemctl").args(&args).output() else {
            continue;
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut cur_id = String::new();
        let mut cur_pid: Option<u32> = None;
        let mut cur_mem: Option<String> = None;
        let mut cur_up: Option<String> = None;

        for line in stdout.lines() {
            if line.is_empty() {
                if !cur_id.is_empty() {
                    if let Some(svc) = services.iter_mut().find(|s| s.name == cur_id) {
                        svc.pid = cur_pid;
                        svc.memory = cur_mem.take();
                        svc.uptime = cur_up.take();
                    }
                }
                cur_id.clear();
                cur_pid = None;
                cur_mem = None;
                cur_up = None;
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                match key {
                    "Id" => cur_id = val.to_string(),
                    "MainPID" => {
                        if let Ok(pid) = val.parse::<u32>() {
                            if pid > 0 {
                                cur_pid = Some(pid);
                            }
                        }
                    }
                    "MemoryCurrent" => {
                        if let Ok(bytes) = val.parse::<u64>() {
                            if bytes < u64::MAX {
                                cur_mem = Some(crate::types::format_bytes(bytes));
                            }
                        }
                    }
                    "ActiveEnterTimestamp" => {
                        if !val.is_empty() && val != "n/a" {
                            cur_up = Some(val.trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        // Flush last
        if !cur_id.is_empty() {
            if let Some(svc) = services.iter_mut().find(|s| s.name == cur_id) {
                svc.pid = cur_pid;
                svc.memory = cur_mem;
                svc.uptime = cur_up;
            }
        }
    }
}

// ── Docker ──

fn list_docker_containers() -> Result<Vec<ServiceEntry>> {
    let output = match output_with_timeout(
        "docker",
        &["ps", "-a", "--no-trunc", "--format",
          "{{.ID}}\t{{.Names}}\t{{.State}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}"],
        5,
    ) {
        Some(o) => o,
        None => return Ok(Vec::new()),
    };

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 5 {
            continue;
        }

        let name = parts[1].to_string();
        let state = parts[2];
        let status_detail = parts[3];
        let image = parts[4];
        let port_map = if parts.len() > 5 { parts[5] } else { "" };

        let status = match state {
            "running" => RunState::Running,
            "exited" => RunState::Exited,
            "paused" => RunState::Paused,
            "restarting" => RunState::Restarting,
            "created" => RunState::Created,
            "dead" => RunState::Failed,
            _ => RunState::Unknown,
        };

        let pid = if status == RunState::Running {
            get_container_pid("docker", &name)
        } else {
            None
        };

        containers.push(ServiceEntry {
            kind: EntryKind::Docker,
            name: format!("docker:{}", name),
            display_name: name,
            status,
            enabled: EnableState::NotApplicable,
            description: image.to_string(),
            pid,
            memory: None,
            uptime: if status_detail.is_empty() {
                None
            } else {
                Some(status_detail.to_string())
            },
            ports: parse_container_ports(port_map),
        });
    }

    Ok(containers)
}

// ── Podman ──

fn list_podman_containers() -> Result<Vec<ServiceEntry>> {
    let output = match output_with_timeout(
        "podman",
        &["ps", "-a", "--no-trunc", "--format",
          "{{.ID}}\t{{.Names}}\t{{.State}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}"],
        5,
    ) {
        Some(o) => o,
        None => return Ok(Vec::new()),
    };

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 5 {
            continue;
        }

        let name = parts[1].to_string();
        let state = parts[2];
        let status_detail = parts[3];
        let image = parts[4];
        let port_map = if parts.len() > 5 { parts[5] } else { "" };

        let status = match state.to_lowercase().as_str() {
            "running" => RunState::Running,
            "exited" => RunState::Exited,
            "paused" => RunState::Paused,
            "created" => RunState::Created,
            "stopped" => RunState::Stopped,
            _ => RunState::Unknown,
        };

        let pid = if status == RunState::Running {
            get_container_pid("podman", &name)
        } else {
            None
        };

        containers.push(ServiceEntry {
            kind: EntryKind::Podman,
            name: format!("podman:{}", name),
            display_name: name,
            status,
            enabled: EnableState::NotApplicable,
            description: image.to_string(),
            pid,
            memory: None,
            uptime: if status_detail.is_empty() {
                None
            } else {
                Some(status_detail.to_string())
            },
            ports: parse_container_ports(port_map),
        });
    }

    Ok(containers)
}

fn get_container_pid(runtime: &str, name: &str) -> Option<u32> {
    let stdout = run_with_timeout(runtime, &["inspect", "--format", "{{.State.Pid}}", name], 3)?;
    let pid: u32 = stdout.trim().parse().ok()?;
    if pid > 0 { Some(pid) } else { None }
}

/// Parse "0.0.0.0:8080->80/tcp, ..." into host port numbers
fn parse_container_ports(port_str: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    for mapping in port_str.split(", ") {
        if let Some(arrow) = mapping.find("->") {
            let host_part = &mapping[..arrow];
            if let Some(colon) = host_part.rfind(':') {
                if let Ok(port) = host_part[colon + 1..].parse::<u16>() {
                    ports.push(port);
                }
            }
        }
    }
    ports
}

// ── Ports ──

fn list_ports() -> Result<Vec<PortEntry>> {
    let mut ports = Vec::new();

    for (args, proto) in &[(["-tlnp"], "tcp"), (["-ulnp"], "udp")] {
        if let Ok(output) = Command::new("ss").args(args.as_slice()).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                if let Some(entry) = parse_ss_line(line, proto) {
                    ports.push(entry);
                }
            }
        }
    }

    ports.sort_by_key(|p| (p.port, p.protocol.clone()));
    ports.dedup_by(|a, b| a.port == b.port && a.protocol == b.protocol && a.address == b.address);
    Ok(ports)
}

fn parse_ss_line(line: &str, proto: &str) -> Option<PortEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }

    let local_addr = parts[3];
    let (address, port_str) = local_addr.rsplit_once(':')?;
    let port: u16 = port_str.parse().ok()?;

    let address = match address {
        "*" | "0.0.0.0" | "[::]" | "" => "*".to_string(),
        a => a.to_string(),
    };

    let process_col = parts.get(5).unwrap_or(&"");
    let (process_name, pid) = parse_ss_process(process_col);

    Some(PortEntry {
        protocol: proto.to_string(),
        port,
        address,
        process_name,
        pid,
        owner: String::new(),
        owner_kind: EntryKind::Systemd,
    })
}

fn parse_ss_process(s: &str) -> (String, Option<u32>) {
    if s.is_empty() || s == "-" {
        return ("-".to_string(), None);
    }

    let process = s
        .find("((\"")
        .and_then(|start| {
            let rest = &s[start + 3..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
        .unwrap_or_else(|| "-".to_string());

    let pid = s.find("pid=").and_then(|start| {
        let rest = &s[start + 4..];
        let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        num.parse().ok()
    });

    (process, pid)
}

// ── Logs ──

/// Get logs for any entry (dispatches based on kind)
pub fn get_logs(entry: &ServiceEntry, count: u32) -> Result<Vec<String>> {
    let count_str = count.to_string();
    match entry.kind {
        EntryKind::Systemd => {
            let output = Command::new("journalctl")
                .args([
                    "-u", &entry.name, "--no-pager", "-n", &count_str,
                    "--output=short-iso",
                ])
                .output()
                .context("Failed to run journalctl")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(stdout
                .lines()
                .filter(|l| !l.starts_with("-- "))
                .map(|l| l.to_string())
                .collect())
        }
        EntryKind::Docker | EntryKind::Podman => {
            let runtime = if entry.kind == EntryKind::Docker {
                "docker"
            } else {
                "podman"
            };
            let output = match output_with_timeout(
                runtime,
                &["logs", "--tail", &count_str, "--timestamps", &entry.display_name],
                5,
            ) {
                Some(o) => o,
                None => return Ok(vec!["(timeout fetching logs)".to_string()]),
            };

            let mut lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|l| l.to_string())
                .collect();
            // Container logs can also be on stderr
            lines.extend(
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .map(|l| l.to_string()),
            );
            Ok(lines)
        }
    }
}

// ── Management ──

/// Execute an action on a service/container
pub fn execute_action(entry: &ServiceEntry, action: ServiceAction) -> Result<String> {
    let cmd = action.as_str();
    match entry.kind {
        EntryKind::Systemd => {
            let output = Command::new("sudo")
                .args(["systemctl", cmd, &entry.name])
                .output()
                .context(format!("sudo systemctl {} {}", cmd, entry.name))?;

            if output.status.success() {
                Ok(format!("systemctl {} {} ✓", cmd, entry.display_name))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow::anyhow!("{}", stderr.trim()))
            }
        }
        EntryKind::Docker | EntryKind::Podman => {
            if matches!(action, ServiceAction::Enable | ServiceAction::Disable) {
                return Err(anyhow::anyhow!("Enable/Disable not applicable for containers"));
            }
            let runtime = if entry.kind == EntryKind::Docker {
                "docker"
            } else {
                "podman"
            };
            let output = match output_with_timeout(
                runtime,
                &[cmd, &entry.display_name],
                10,
            ) {
                Some(o) => o,
                None => return Err(anyhow::anyhow!("Timeout: {} {} {}", runtime, cmd, entry.display_name)),
            };

            if output.status.success() {
                Ok(format!("{} {} {} ✓", runtime, cmd, entry.display_name))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow::anyhow!("{}", stderr.trim()))
            }
        }
    }
}

// ── Helpers ──

fn tool_available(name: &str) -> bool {
    // Check if binary exists
    let exists = Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !exists {
        return false;
    }

    // For docker/podman: verify the daemon is actually responsive
    // docker ps / podman ps hang indefinitely if daemon is down
    match name {
        "docker" | "podman" => {
            run_with_timeout(name, &["info", "--format", "{{.ID}}"], 3).is_some()
        }
        _ => true,
    }
}

/// Run a command with a timeout. Returns stdout on success, None on timeout/error.
fn run_with_timeout(cmd: &str, args: &[&str], timeout_secs: u64) -> Option<String> {
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
            Ok(Some(status)) => {
                if status.success() {
                    let output = child.wait_with_output().ok()?;
                    return Some(String::from_utf8_lossy(&output.stdout).to_string());
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

/// Like Command::output() but with a timeout. Returns None if timeout or error.
fn output_with_timeout(cmd: &str, args: &[&str], timeout_secs: u64) -> Option<std::process::Output> {
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
