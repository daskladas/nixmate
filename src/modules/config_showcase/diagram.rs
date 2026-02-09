//! Config Diagram — architecture diagram of NixOS configuration.
//!
//! Scans `/etc/nixos/` for all `.nix` files, parses imports and
//! flake inputs, builds a dependency graph, groups by directory,
//! and renders it as a professional SVG node-graph with arrows.

#![allow(clippy::write_with_newline)]
use std::collections::{HashMap, VecDeque};
use std::fmt::Write;
use std::path::{Path, PathBuf};

// ── Colors (same palette as poster) ──
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
const PINK: &str = "#f778ba";
const CYAN: &str = "#56d4dd";

// ── Layout constants ──
const PAD: f64 = 50.0;
const HEADER_H: f64 = 130.0;
const LEGEND_H: f64 = 80.0;
const FOOTER_H: f64 = 60.0;
const NODE_R: f64 = 12.0;

// ═══════════════════════════════════════
//  Data structures
// ═══════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    FlakeRoot,
    FlakeInput,
    SystemConfig,
    HardwareConfig,
    HomeManager,
    CustomModule,
}

impl NodeType {
    pub fn color(&self) -> &'static str {
        match self {
            NodeType::FlakeRoot => CYAN,
            NodeType::FlakeInput => BLUE,
            NodeType::SystemConfig => GREEN,
            NodeType::HardwareConfig => PINK,
            NodeType::HomeManager => PURPLE,
            NodeType::CustomModule => ORANGE,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            NodeType::FlakeRoot => "Flake Root",
            NodeType::FlakeInput => "Flake Input",
            NodeType::SystemConfig => "System Config",
            NodeType::HardwareConfig => "Hardware",
            NodeType::HomeManager => "Home Manager",
            NodeType::CustomModule => "Module",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagramNode {
    pub name: String,
    #[allow(dead_code)] // Used in SVG output
    pub full_path: String,
    pub node_type: NodeType,
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlakeInput {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct DiagramInfo {
    pub hostname: String,
    pub nixos_version: String,
    pub config_root: String,
    pub is_flake: bool,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<(usize, usize)>,
    pub flake_inputs: Vec<FlakeInput>,
    pub total_files: usize,
}

// ═══════════════════════════════════════
//  Scanning
// ═══════════════════════════════════════

pub fn scan_config() -> DiagramInfo {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .or_else(|_| {
            std::process::Command::new("hostname")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|_| "nixos".into());

    let nixos_version = std::process::Command::new("nixos-version")
        .output()
        .ok()
        .map(|o| {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            v.split_whitespace()
                .next()
                .unwrap_or("")
                .split('.')
                .take(2)
                .collect::<Vec<&str>>()
                .join(".")
        })
        .unwrap_or_else(|| "?".into());

    let config_root = find_config_root();
    let is_flake = Path::new(&config_root).join("flake.nix").exists();

    let mut nodes: Vec<DiagramNode> = Vec::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut path_to_idx: HashMap<String, usize> = HashMap::new();
    let mut flake_inputs: Vec<FlakeInput> = Vec::new();

    let nix_files = discover_nix_files(&config_root);
    let total_files = nix_files.len();

    if is_flake {
        let flake_path = Path::new(&config_root).join("flake.nix");
        if let Ok(content) = std::fs::read_to_string(&flake_path) {
            flake_inputs = parse_flake_inputs(&content);
        }
    }

    for file in &nix_files {
        let rel = file
            .strip_prefix(&config_root)
            .unwrap_or(file)
            .trim_start_matches('/');
        let name = if rel.is_empty() {
            Path::new(file)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else {
            rel.to_string()
        };
        let node_type = classify_file(&name, file);
        let idx = nodes.len();
        path_to_idx.insert(file.clone(), idx);
        nodes.push(DiagramNode {
            name,
            full_path: file.clone(),
            node_type,
            subtitle: None,
        });
    }

    let mut flake_root_idx: Option<usize> = None;
    for (file, idx) in &path_to_idx {
        if file.ends_with("flake.nix") {
            flake_root_idx = Some(*idx);
            break;
        }
    }

    for fi in &flake_inputs {
        let idx = nodes.len();
        let short_url = shorten_url(&fi.url);
        nodes.push(DiagramNode {
            name: fi.name.clone(),
            full_path: format!("input:{}", fi.name),
            node_type: NodeType::FlakeInput,
            subtitle: Some(short_url),
        });
        if let Some(root_idx) = flake_root_idx {
            edges.push((idx, root_idx));
        }
    }

    for file in &nix_files {
        let Some(&from_idx) = path_to_idx.get(file.as_str()) else {
            continue;
        };
        if let Ok(content) = std::fs::read_to_string(file) {
            let imports = parse_imports(&content, file, &config_root);
            for imp in imports {
                if let Some(&to_idx) = path_to_idx.get(imp.as_str()) {
                    if from_idx != to_idx {
                        edges.push((from_idx, to_idx));
                    }
                }
            }
            if file.ends_with("flake.nix") {
                let referenced = parse_flake_module_refs(&content, &config_root);
                for ref_path in referenced {
                    if let Some(&to_idx) = path_to_idx.get(ref_path.as_str()) {
                        if from_idx != to_idx && !edges.contains(&(from_idx, to_idx)) {
                            edges.push((from_idx, to_idx));
                        }
                    }
                }
            }
        }
    }

    if !is_flake {
        auto_link_standard_imports(&path_to_idx, &mut edges);
    }

    edges.sort();
    edges.dedup();

    DiagramInfo {
        hostname,
        nixos_version,
        config_root,
        is_flake,
        nodes,
        edges,
        flake_inputs,
        total_files,
    }
}

fn shorten_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("github:") {
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() >= 2 {
            return format!("{}/{}", parts[0], parts[1]);
        }
        return rest.to_string();
    }
    if url.len() > 40 {
        truncate(url, 39)
    } else {
        url.to_string()
    }
}

// ═══════════════════════════════════════
//  File system helpers
// ═══════════════════════════════════════

fn find_config_root() -> String {
    let candidates = ["/etc/nixos"];
    for c in &candidates {
        if Path::new(c).exists() {
            return c.to_string();
        }
    }
    "/etc/nixos".to_string()
}

fn discover_nix_files(root: &str) -> Vec<String> {
    let root_path = Path::new(root);
    if !root_path.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    walk_nix_files(root_path, &mut files);
    files.sort_by(|a, b| {
        let rank = |p: &str| -> u8 {
            if p.ends_with("flake.nix") {
                0
            } else if p.ends_with("configuration.nix") {
                1
            } else if p.ends_with("hardware-configuration.nix") || p.ends_with("hardware.nix") {
                2
            } else {
                3
            }
        };
        rank(a).cmp(&rank(b)).then(a.cmp(b))
    });
    files
}

fn walk_nix_files(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') || name == "result" || name == "node_modules" {
                continue;
            }
            walk_nix_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "nix") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "flake.lock" {
                continue;
            }
            out.push(path.to_string_lossy().to_string());
        }
    }
}

fn classify_file(rel_name: &str, _full_path: &str) -> NodeType {
    let lower = rel_name.to_lowercase();
    if lower == "flake.nix" {
        NodeType::FlakeRoot
    } else if lower.contains("hardware") {
        NodeType::HardwareConfig
    } else if lower == "configuration.nix" {
        NodeType::SystemConfig
    } else if lower.contains("home")
        || lower.contains("hm-")
        || lower.contains("home-manager")
        || lower.starts_with("users/")
    {
        NodeType::HomeManager
    } else if lower.starts_with("hosts/") || lower.starts_with("nixosconfigurations/") {
        NodeType::SystemConfig
    } else if !lower.contains('/') {
        // Root-level entry files (default.nix, disko-defaults.nix, etc.)
        // stand out from nested module files
        NodeType::SystemConfig
    } else {
        NodeType::CustomModule
    }
}

// ═══════════════════════════════════════
//  Import parsing
// ═══════════════════════════════════════

fn parse_imports(content: &str, file_path: &str, config_root: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let cleaned = remove_nix_comments(content);

    let Some(start) = find_imports_block(&cleaned) else {
        return imports;
    };

    let bracket_content = extract_bracket_content(&cleaned[start..]);
    let file_dir = Path::new(file_path)
        .parent()
        .unwrap_or(Path::new(config_root));

    for token in bracket_content.split_whitespace() {
        let token = token.trim_matches(|c: char| c == ',' || c == ';');
        if token.is_empty() || token.starts_with('#') {
            continue;
        }
        if token.starts_with("./") || token.starts_with("../") {
            let resolved = file_dir.join(token);
            if let Ok(canonical) = resolved.canonicalize() {
                imports.push(canonical.to_string_lossy().to_string());
            } else {
                let resolved_str = resolved.to_string_lossy().to_string();
                let normalized = normalize_path(&resolved_str);
                imports.push(normalized);
            }
        }
    }
    imports
}

fn remove_nix_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_block_comment = false;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            continue;
        }
        if c == '#' {
            while let Some(&nc) = chars.peek() {
                if nc == '\n' {
                    break;
                }
                chars.next();
            }
            result.push('\n');
            continue;
        }
        result.push(c);
    }
    result
}

fn find_imports_block(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let target = b"imports";
    let len = bytes.len();
    let mut i = 0;
    while i + target.len() < len {
        if &bytes[i..i + target.len()] == target {
            let before_ok =
                i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
            if before_ok {
                let mut j = i + target.len();
                while j < len && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < len && bytes[j] == b'=' {
                    j += 1;
                    while j < len && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < len && bytes[j] == b'[' {
                        return Some(j + 1);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn extract_bracket_content(content: &str) -> String {
    let mut depth = 1;
    let mut result = String::new();
    for c in content.chars() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        result.push(c);
    }
    result
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(component),
        }
    }
    format!("/{}", parts.join("/"))
}

// ═══════════════════════════════════════
//  Flake input parsing
// ═══════════════════════════════════════

fn parse_flake_inputs(content: &str) -> Vec<FlakeInput> {
    let mut inputs = Vec::new();
    let cleaned = remove_nix_comments(content);

    if let Some(block) = extract_inputs_block(&cleaned) {
        let mut current_name: Option<String> = None;
        let mut brace_depth: u32 = 0;

        for line in block.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            for c in trimmed.chars() {
                match c {
                    '{' => brace_depth += 1,
                    '}' => brace_depth = brace_depth.saturating_sub(1),
                    _ => {}
                }
            }
            if brace_depth <= 1 {
                if let Some((name, url)) = parse_input_line(trimmed) {
                    inputs.push(FlakeInput { name, url });
                    current_name = None;
                } else if let Some(name) = extract_input_name(trimmed) {
                    current_name = Some(name);
                }
            } else if let Some(ref name) = current_name {
                if let Some(url) = extract_url_from_line(trimmed) {
                    inputs.push(FlakeInput {
                        name: name.clone(),
                        url,
                    });
                    current_name = None;
                }
            }
        }
    }

    for line in cleaned.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("inputs.") && trimmed.contains(".url") && trimmed.contains('=') {
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            if parts.len() == 2 {
                let name_part = parts[0].trim();
                let name = name_part
                    .strip_prefix("inputs.")
                    .unwrap_or(name_part)
                    .strip_suffix(".url")
                    .unwrap_or(name_part)
                    .to_string();
                let url = parts[1]
                    .trim()
                    .trim_matches(|c: char| c == '"' || c == ';' || c == ' ')
                    .to_string();
                if !name.is_empty()
                    && !url.is_empty()
                    && !inputs.iter().any(|i| i.name == name)
                {
                    inputs.push(FlakeInput { name, url });
                }
            }
        }
    }
    inputs
}

fn extract_inputs_block(content: &str) -> Option<String> {
    let mut search = content;
    loop {
        let idx = search.find("inputs")?;
        let after = &search[idx + 6..];
        let before_ok = idx == 0 || {
            let before_char = search.as_bytes()[idx - 1];
            !before_char.is_ascii_alphanumeric() && before_char != b'_' && before_char != b'.'
        };
        if !before_ok {
            search = &search[idx + 6..];
            continue;
        }
        let trimmed = after.trim_start();
        if let Some(stripped) = trimmed.strip_prefix('=') {
            let after_eq = stripped.trim_start();
            if after_eq.starts_with('{') {
                let mut depth = 0;
                let mut end = 0;
                for (i, c) in after_eq.char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if end > 0 {
                    return Some(after_eq[1..end].to_string());
                }
            }
        }
        search = &search[idx + 6..];
    }
}

fn parse_input_line(line: &str) -> Option<(String, String)> {
    if line.contains(".url") && line.contains('=') && line.contains('"') {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            let name = parts[0]
                .trim()
                .strip_suffix(".url")
                .unwrap_or(parts[0].trim())
                .trim()
                .to_string();
            let url = parts[1]
                .trim()
                .trim_matches(|c: char| c == '"' || c == ';' || c == ' ')
                .to_string();
            if !name.is_empty() && !url.is_empty() {
                return Some((name, url));
            }
        }
    }
    None
}

fn extract_input_name(line: &str) -> Option<String> {
    if line.contains('=') {
        let name = line.split('=').next()?.trim().to_string();
        if !name.is_empty()
            && !name.contains('.')
            && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Some(name);
        }
    }
    None
}

fn extract_url_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("url") && trimmed.contains('=') && trimmed.contains('"') {
        let after_eq = trimmed.split('=').nth(1)?;
        let url = after_eq
            .trim()
            .trim_matches(|c: char| c == '"' || c == ';' || c == ' ');
        if !url.is_empty() {
            return Some(url.to_string());
        }
    }
    None
}

fn parse_flake_module_refs(content: &str, config_root: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let root = Path::new(config_root);
    for line in content.lines() {
        let trimmed = line.trim();
        let mut pos = 0;
        let bytes = trimmed.as_bytes();
        while pos + 2 < bytes.len() {
            if bytes[pos] == b'.' && bytes[pos + 1] == b'/' {
                let start = pos;
                let mut end = pos + 2;
                while end < bytes.len()
                    && (bytes[end].is_ascii_alphanumeric()
                        || bytes[end] == b'/'
                        || bytes[end] == b'-'
                        || bytes[end] == b'_'
                        || bytes[end] == b'.')
                {
                    end += 1;
                }
                let token = &trimmed[start..end];
                if token.ends_with(".nix") {
                    let resolved = root.join(token);
                    if let Ok(canonical) = resolved.canonicalize() {
                        let path_str = canonical.to_string_lossy().to_string();
                        if !refs.contains(&path_str) {
                            refs.push(path_str);
                        }
                    }
                }
                pos = end;
            } else {
                pos += 1;
            }
        }
    }
    refs
}

fn auto_link_standard_imports(
    path_to_idx: &HashMap<String, usize>,
    edges: &mut Vec<(usize, usize)>,
) {
    let mut config_idx = None;
    let mut hw_idx = None;
    for (path, &idx) in path_to_idx {
        if path.ends_with("configuration.nix") {
            config_idx = Some(idx);
        }
        if path.ends_with("hardware-configuration.nix") {
            hw_idx = Some(idx);
        }
    }
    if let (Some(ci), Some(hi)) = (config_idx, hw_idx) {
        if !edges.iter().any(|(from, to)| *from == ci && *to == hi) {
            edges.push((ci, hi));
        }
    }
}

// ═══════════════════════════════════════
//  Directory Tree + Grouping
// ═══════════════════════════════════════

/// A rendered card — either a single file or an expanded directory group.
struct GNode {
    name: String,          // full relative path key, e.g. "modules/core"
    display_name: String,  // short display name, e.g. "core"
    node_type: NodeType,
    children: Vec<String>, // child file names (short) for expanded groups
    is_group: bool,
    subtitle: Option<String>,
    /// Original node indices for edge remapping
    members: Vec<usize>,
}

impl GNode {
    /// Visual height of this card in the SVG.
    fn height(&self) -> f64 {
        if !self.is_group || self.children.is_empty() {
            return 60.0; // standard single-file node
        }
        let header = 44.0;
        let line_h = 18.0;
        let max_lines = 24;
        let n = self.children.len().min(max_lines);
        let extra = if self.children.len() > max_lines { 18.0 } else { 0.0 };
        header + n as f64 * line_h + extra + 14.0
    }

    /// Width: groups are wider to fit content.
    fn width(&self) -> f64 {
        if self.is_group { 300.0 } else { 260.0 }
    }
}

struct GroupedDiagram {
    groups: Vec<GNode>,
    edges: Vec<(usize, usize)>,
}

/// Determine the group key for a node.
fn group_key(name: &str, node_type: &NodeType) -> Option<String> {
    if matches!(node_type, NodeType::FlakeInput | NodeType::FlakeRoot) {
        return None;
    }
    let parts: Vec<&str> = name.split('/').filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 | 1 => None,
        2 => Some(parts[0].to_string()),
        _ => Some(format!("{}/{}", parts[0], parts[1])),
    }
}

fn classify_group(group_name: &str) -> NodeType {
    let lower = group_name.to_lowercase();
    if lower.contains("hardware") || lower.contains("peripheral") {
        NodeType::HardwareConfig
    } else if lower.starts_with("users") || lower.contains("home") {
        NodeType::HomeManager
    } else if lower.starts_with("hosts") || lower.starts_with("nixosconfig") {
        NodeType::SystemConfig
    } else {
        NodeType::CustomModule
    }
}

/// Sort priority for NodeType (for color-grouping within layers).
fn type_sort_order(nt: &NodeType) -> u8 {
    match nt {
        NodeType::FlakeRoot => 0,
        NodeType::FlakeInput => 1,
        NodeType::SystemConfig => 2,
        NodeType::HardwareConfig => 3,
        NodeType::HomeManager => 4,
        NodeType::CustomModule => 5,
    }
}

/// Collapse raw file nodes into directory groups with expanded children.
fn collapse_to_groups(info: &DiagramInfo) -> GroupedDiagram {
    let file_nodes: usize = info.nodes.iter()
        .filter(|n| n.node_type != NodeType::FlakeInput)
        .count();

    // Very small configs: don't group at all
    if file_nodes <= 8 {
        let groups: Vec<GNode> = info.nodes.iter().enumerate().map(|(i, n)| {
            GNode {
                name: n.name.clone(),
                display_name: n.name.clone(),
                node_type: n.node_type.clone(),
                children: Vec::new(),
                is_group: false,
                subtitle: n.subtitle.clone(),
                members: vec![i],
            }
        }).collect();
        return GroupedDiagram {
            groups,
            edges: info.edges.clone(),
        };
    }

    // Build group map
    let mut group_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut individual: Vec<usize> = Vec::new();

    for (i, node) in info.nodes.iter().enumerate() {
        if let Some(key) = group_key(&node.name, &node.node_type) {
            group_map.entry(key).or_default().push(i);
        } else {
            individual.push(i);
        }
    }

    let mut groups: Vec<GNode> = Vec::new();
    let mut old_to_new: HashMap<usize, usize> = HashMap::new();

    // Individual nodes first (flake.nix, default.nix at root, etc.)
    for &orig_idx in &individual {
        let node = &info.nodes[orig_idx];
        let new_idx = groups.len();
        old_to_new.insert(orig_idx, new_idx);
        groups.push(GNode {
            name: node.name.clone(),
            display_name: node.name.clone(),
            node_type: node.node_type.clone(),
            children: Vec::new(),
            is_group: false,
            subtitle: node.subtitle.clone(),
            members: vec![orig_idx],
        });
    }

    // Group nodes: sorted for deterministic order
    let mut group_keys: Vec<String> = group_map.keys().cloned().collect();
    group_keys.sort();

    for key in &group_keys {
        let members = &group_map[key];
        let new_idx = groups.len();
        for &m in members {
            old_to_new.insert(m, new_idx);
        }
        let node_type = classify_group(key);
        let display = if key.contains('/') {
            key.split('/').next_back().unwrap_or(key).to_string()
        } else {
            key.clone()
        };

        // Build children list (short names relative to group key)
        let mut children: Vec<String> = Vec::new();
        for &m in members {
            let name = &info.nodes[m].name;
            let child_name = name.strip_prefix(key)
                .unwrap_or(name)
                .trim_start_matches('/');
            if !child_name.is_empty() {
                children.push(child_name.to_string());
            } else {
                children.push(name.clone());
            }
        }
        children.sort();

        groups.push(GNode {
            name: key.clone(),
            display_name: display,
            node_type,
            children,
            is_group: true,
            subtitle: None,
            members: members.clone(),
        });
    }

    // Remap edges and keep ALL cross-group connections
    let mut new_edges: Vec<(usize, usize)> = Vec::new();
    for &(from, to) in &info.edges {
        if let (Some(&nf), Some(&nt)) = (old_to_new.get(&from), old_to_new.get(&to)) {
            if nf != nt {
                new_edges.push((nf, nt));
            }
        }
    }

    // ── Generate structural edges from directory hierarchy ──
    // Strategy: show STRUCTURE (parent→child), not every cross-reference.
    // This keeps the diagram clean and readable.

    let mut name_to_gidx: HashMap<String, usize> = HashMap::new();
    for (i, g) in groups.iter().enumerate() {
        name_to_gidx.insert(g.name.clone(), i);
    }

    let flake_idx = groups.iter().position(|g| g.node_type == NodeType::FlakeRoot);

    // 1) flake.nix → nixosConfigurations (the primary output)
    if let Some(fi) = flake_idx {
        if let Some(&nc) = name_to_gidx.get("nixosConfigurations") {
            new_edges.push((fi, nc));
        }
        // flake.nix → root-level individual files (default.nix, etc.)
        for (i, g) in groups.iter().enumerate() {
            if i == fi { continue; }
            if g.node_type == NodeType::FlakeInput { continue; }
            if !g.is_group && !g.name.contains('/') {
                new_edges.push((fi, i));
            }
        }
    }

    // 2) nixosConfigurations → hosts parent (if exists)
    if let Some(&nc) = name_to_gidx.get("nixosConfigurations") {
        if let Some(&h) = name_to_gidx.get("hosts") {
            new_edges.push((nc, h));
        } else {
            // No "hosts" parent group — link directly to host children
            for (i, g) in groups.iter().enumerate() {
                if g.name.starts_with("hosts/") {
                    new_edges.push((nc, i));
                }
            }
        }
    }

    // 3) Parent → direct child containment (the tree structure)
    //    e.g., hosts → hosts/common, hosts → hosts/Desktop
    //    e.g., modules → modules/core (if "modules" group exists)
    for i in 0..groups.len() {
        for j in 0..groups.len() {
            if i == j { continue; }
            let parent = &groups[i].name;
            let child = &groups[j].name;
            if child.starts_with(parent)
                && child.len() > parent.len()
                && child.as_bytes().get(parent.len()) == Some(&b'/')
                && !child[parent.len() + 1..].contains('/')
            {
                new_edges.push((i, j));
            }
        }
    }

    // 4) One representative "hosts/common → modules" link
    //    (shows that host configs import modules, without N×M explosion)
    if let Some(&common) = name_to_gidx.get("hosts/common") {
        if let Some(&mods) = name_to_gidx.get("modules") {
            new_edges.push((common, mods));
        }
        // Also link to profiles/users/scripts if they exist
        for top in &["profiles", "users", "scripts"] {
            if let Some(&ti) = name_to_gidx.get(*top) {
                new_edges.push((common, ti));
            }
        }
    }

    new_edges.sort();
    new_edges.dedup();

    GroupedDiagram { groups, edges: new_edges }
}

// ═══════════════════════════════════════
//  SVG Generation (Expanded Layout)
// ═══════════════════════════════════════

struct LayoutNode {
    idx: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct LayerInfo {
    y: f64,
    label: String,
}

/// Generate the full config diagram SVG.
pub fn generate_diagram_svg(info: &DiagramInfo) -> String {
    if info.nodes.is_empty() {
        return generate_empty_diagram(&info.hostname);
    }

    let gd = collapse_to_groups(info);

    // Assign depths via BFS
    let num = gd.groups.len();
    let mut depth: Vec<Option<usize>> = vec![None; num];
    let root_idx = find_grouped_root(&gd.groups);
    depth[root_idx] = Some(0);

    let mut queue = VecDeque::new();
    queue.push_back(root_idx);
    while let Some(current) = queue.pop_front() {
        let cd = depth[current].unwrap_or(0);
        for &(from, to) in &gd.edges {
            if from == current && depth[to].is_none() {
                depth[to] = Some(cd + 1);
                queue.push_back(to);
            }
        }
    }

    // Separate flake inputs
    let flake_input_indices: Vec<usize> = (0..num)
        .filter(|&i| gd.groups[i].node_type == NodeType::FlakeInput)
        .collect();
    let has_inputs = !flake_input_indices.is_empty();
    for &fi in &flake_input_indices {
        depth[fi] = None;
    }

    // Smart orphan assignment
    #[allow(clippy::needless_range_loop)]
    for i in 0..num {
        if depth[i].is_none() && gd.groups[i].node_type != NodeType::FlakeInput {
            depth[i] = Some(smart_orphan_depth(&gd.groups[i].name, info.is_flake));
        }
    }

    let max_depth = depth.iter().filter_map(|d| *d).max().unwrap_or(0);

    // Sort nodes within each layer by type (color-grouping) then name
    let mut layer_lists: Vec<(String, Vec<usize>)> = Vec::new();
    let mut used_labels: Vec<String> = Vec::new();

    if has_inputs {
        let mut inputs = flake_input_indices.clone();
        inputs.sort_by(|a, b| gd.groups[*a].name.cmp(&gd.groups[*b].name));
        layer_lists.push(("INPUTS".into(), inputs));
    }

    for d in 0..=max_depth {
        let mut layer_nodes: Vec<usize> = (0..num)
            .filter(|&i| depth[i] == Some(d) && gd.groups[i].node_type != NodeType::FlakeInput)
            .collect();
        if layer_nodes.is_empty() { continue; }

        // Sort by type then name
        layer_nodes.sort_by(|a, b| {
            type_sort_order(&gd.groups[*a].node_type)
                .cmp(&type_sort_order(&gd.groups[*b].node_type))
                .then(gd.groups[*a].name.cmp(&gd.groups[*b].name))
        });

        let label = layer_label(d, info.is_flake, &layer_nodes, &gd.groups, &mut used_labels);
        layer_lists.push((label, layer_nodes));
    }

    // Calculate layout with variable heights and determine SVG size
    let max_per_row: usize = 4;
    let gap_x: f64 = 24.0;
    let row_gap: f64 = 28.0;
    let layer_gap: f64 = 40.0;
    let pad: f64 = 50.0;

    // First pass: determine required width
    let mut max_row_w: f64 = 0.0;
    for (_label, indices) in &layer_lists {
        let chunks: Vec<&[usize]> = indices.chunks(max_per_row).collect();
        for chunk in &chunks {
            let row_w: f64 = chunk.iter().map(|&i| gd.groups[i].width()).sum::<f64>()
                + (chunk.len() as f64 - 1.0).max(0.0) * gap_x;
            if row_w > max_row_w { max_row_w = row_w; }
        }
    }
    let svg_w = (max_row_w + 2.0 * pad + 60.0).max(1200.0);

    // Second pass: place nodes
    let mut positioned: Vec<LayoutNode> = Vec::new();
    let mut layers_info: Vec<LayerInfo> = Vec::new();
    let mut current_y = HEADER_H + 30.0;

    for (label, indices) in &layer_lists {
        let _layer_start_y = current_y;
        layers_info.push(LayerInfo {
            y: current_y + 30.0,
            label: label.clone(),
        });

        let chunks: Vec<Vec<usize>> = indices.chunks(max_per_row)
            .map(|c| c.to_vec())
            .collect();

        for chunk in &chunks {
            // Calculate row width and max height in this row
            let row_w: f64 = chunk.iter().map(|&i| gd.groups[i].width()).sum::<f64>()
                + (chunk.len() as f64 - 1.0).max(0.0) * gap_x;
            let start_x = ((svg_w - row_w) / 2.0).max(pad);
            let row_max_h: f64 = chunk.iter().map(|&i| gd.groups[i].height()).fold(0.0_f64, f64::max);

            let mut cx = start_x;
            for &idx in chunk {
                let w = gd.groups[idx].width();
                let h = gd.groups[idx].height();
                positioned.push(LayoutNode { idx, x: cx, y: current_y, w, h });
                cx += w + gap_x;
            }
            current_y += row_max_h + row_gap;
        }
        current_y += layer_gap - row_gap; // extra gap between layers
    }

    let svg_h = current_y + LEGEND_H + FOOTER_H;

    // ── Build SVG ──
    let mut svg = String::with_capacity(131072);

    let _ = write!(svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{svg_w}" height="{svg_h}" viewBox="0 0 {svg_w} {svg_h}">
<defs>
<style>
@import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600;700&amp;display=swap');
text {{ font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace; }}
</style>
"#);

    // Arrow markers
    // Glow filters
    for (id, color) in &[
        ("glow-cyan", CYAN), ("glow-blue", BLUE), ("glow-green", GREEN),
        ("glow-pink", PINK), ("glow-purple", PURPLE), ("glow-orange", ORANGE),
    ] {
        let _ = write!(svg,
            r#"<filter id="{id}" x="-20%" y="-20%" width="140%" height="140%">
<feDropShadow dx="0" dy="0" stdDeviation="5" flood-color="{color}" flood-opacity="0.15"/>
</filter>
"#);
    }

    // Gradient + grid
    let _ = write!(svg,
        r#"<linearGradient id="topbar" x1="0" y1="0" x2="1" y2="0">
<stop offset="0%" stop-color="{BLUE}"/><stop offset="25%" stop-color="{PURPLE}"/>
<stop offset="50%" stop-color="{PINK}"/><stop offset="75%" stop-color="{ORANGE}"/>
<stop offset="100%" stop-color="{GREEN}"/>
</linearGradient>
<pattern id="grid" width="30" height="30" patternUnits="userSpaceOnUse">
<circle cx="15" cy="15" r="0.5" fill="{DIM}" opacity="0.15"/>
</pattern>
</defs>
<rect width="{svg_w}" height="{svg_h}" rx="16" fill="{BG}"/>
<rect width="{svg_w}" height="{svg_h}" rx="16" fill="url(#grid)"/>
<rect width="{svg_w}" height="4" rx="2" fill="url(#topbar)"/>
"#);

    render_header(&mut svg, info);
    render_layer_labels(&mut svg, &layers_info);
    render_arrows(&mut svg, &gd, &positioned, svg_w);

    for ln in &positioned {
        render_node(&mut svg, ln, &gd.groups[ln.idx]);
    }

    render_legend(&mut svg, svg_h, svg_w, &gd);
    render_footer(&mut svg, svg_h, svg_w, info);

    svg.push_str("</svg>");
    svg
}

fn smart_orphan_depth(name: &str, is_flake: bool) -> usize {
    let lower = name.to_lowercase();
    let base = if is_flake { 1 } else { 0 };
    if lower.starts_with("nixosconfig") { return base + 1; }
    if lower.starts_with("hosts") { return base + 1; }
    if lower.starts_with("modules/") { return base + 2; }
    if lower.starts_with("modules") && !lower.contains('/') { return base + 2; }
    if lower.starts_with("profiles") || lower.starts_with("users") || lower.starts_with("scripts") { return base + 2; }
    let parts: Vec<&str> = name.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 1 { return base; }
    base + 2
}

fn layer_label(depth: usize, is_flake: bool, indices: &[usize], groups: &[GNode], used: &mut Vec<String>) -> String {
    if depth == 0 {
        let l: String = if is_flake { "ROOT".into() } else { "ENTRY".into() };
        used.push(l.clone());
        return l;
    }

    // Check content of this layer to pick the best label
    let has_hosts = indices.iter().any(|&i| {
        let n = groups[i].name.to_lowercase();
        n.starts_with("hosts") || n.starts_with("nixosconfig")
    });
    let has_modules = indices.iter().any(|&i| groups[i].name.to_lowercase().starts_with("modules"));
    let has_profiles = indices.iter().any(|&i| {
        let n = groups[i].name.to_lowercase();
        n.starts_with("profiles") || n.starts_with("users") || n.starts_with("scripts")
    });

    let candidate = if has_hosts && !used.contains(&"HOSTS".to_string()) {
        "HOSTS"
    } else if has_modules && !used.contains(&"MODULES".to_string()) {
        "MODULES"
    } else if has_profiles && !used.contains(&"EXTRAS".to_string()) {
        "EXTRAS"
    } else if has_hosts {
        "HOST CONFIG"
    } else if has_modules {
        "SUB-MODULES"
    } else if depth == 1 {
        "CONFIG"
    } else {
        "TREE"
    };

    let label = candidate.to_string();
    used.push(label.clone());
    label
}

fn find_grouped_root(groups: &[GNode]) -> usize {
    for (i, g) in groups.iter().enumerate() {
        if g.node_type == NodeType::FlakeRoot { return i; }
    }
    for (i, g) in groups.iter().enumerate() {
        if g.node_type == NodeType::SystemConfig { return i; }
    }
    for (i, g) in groups.iter().enumerate() {
        if g.node_type != NodeType::FlakeInput { return i; }
    }
    0
}

// ═══════════════════════════════════════
//  SVG Rendering
// ═══════════════════════════════════════

fn render_header(s: &mut String, info: &DiagramInfo) {
    let flake_badge = if info.is_flake {
        let bx = PAD + hostname_approx_width(&info.hostname) + 16.0;
        format!(
            r#"<rect x="{bx}" y="68" rx="6" width="60" height="22" fill="{CYAN}" opacity="0.15" stroke="{CYAN}" stroke-width="0.5"/>
<text x="{tx}" y="83" font-size="10" fill="{CYAN}" font-weight="600">FLAKE</text>"#,
            tx = bx + 8.0,
        )
    } else {
        String::new()
    };

    let _ = write!(s,
        r#"<text x="{PAD}" y="40" font-size="11" fill="{DIM}" letter-spacing="3" font-weight="600">NIXMATE  ·  CONFIG DIAGRAM</text>
<text x="{PAD}" y="85" font-size="36" fill="{FG}" font-weight="700">{hostname}</text>
{flake_badge}
<text x="{PAD}" y="112" font-size="14" fill="{FG2}">NixOS {ver}  ·  {root}  ·  {nf} files{fi}</text>
"#,
        hostname = esc(&info.hostname),
        ver = esc(&info.nixos_version),
        root = esc(&info.config_root),
        nf = info.total_files,
        fi = if info.is_flake {
            format!("  ·  {} flake inputs", info.flake_inputs.len())
        } else { String::new() }
    );
}

fn hostname_approx_width(hostname: &str) -> f64 {
    hostname.len() as f64 * 21.6
}

fn render_layer_labels(s: &mut String, layers: &[LayerInfo]) {
    for layer in layers {
        let _ = write!(s,
            r#"<text x="18" y="{y}" font-size="9" fill="{DIM}" letter-spacing="2" font-weight="600" transform="rotate(-90, 18, {y})">{label}</text>
"#,
            y = layer.y, label = esc(&layer.label),
        );
    }
}

fn render_node(s: &mut String, ln: &LayoutNode, gnode: &GNode) {
    let x = ln.x;
    let y = ln.y;
    let w = ln.w;
    let h = ln.h;
    let color = gnode.node_type.color();

    let glow_id = match &gnode.node_type {
        NodeType::FlakeRoot => "glow-cyan",
        NodeType::FlakeInput => "glow-blue",
        NodeType::SystemConfig => "glow-green",
        NodeType::HardwareConfig => "glow-pink",
        NodeType::HomeManager => "glow-purple",
        NodeType::CustomModule => "glow-orange",
    };

    // Card background
    let _ = write!(s,
        r#"<rect x="{x}" y="{y}" rx="{NODE_R}" width="{w}" height="{h}" fill="{CARD_BG}" stroke="{color}" stroke-width="1.5" filter="url(#{glow_id})"/>
"#);

    // Left accent bar
    let bar_h = (h - 20.0).max(10.0);
    let _ = write!(s,
        r#"<rect x="{ax}" y="{ay}" rx="3" width="4" height="{bar_h}" fill="{color}"/>
"#, ax = x + 3.0, ay = y + 10.0);

    // Clean colored indicator dot (no emoji!)
    let dot_cx = x + 22.0;
    let dot_cy = y + 22.0;
    let _ = write!(s,
        r#"<circle cx="{dot_cx}" cy="{dot_cy}" r="7" fill="{color}" opacity="0.2"/>
<circle cx="{dot_cx}" cy="{dot_cy}" r="4" fill="{color}"/>
"#);

    // Name (bold, white)
    let display_name = if gnode.display_name.chars().count() > 26 {
        truncate(&gnode.display_name, 25)
    } else {
        gnode.display_name.clone()
    };
    let _ = write!(s,
        r#"<text x="{nx}" y="{ny}" font-size="13" fill="{FG}" font-weight="700">{name}</text>
"#, nx = x + 38.0, ny = y + 26.0, name = esc(&display_name));

    if gnode.is_group && !gnode.children.is_empty() {
        // Group: show file count badge + child listing
        let count = gnode.members.len();
        let badge_x = x + w - 52.0;
        let badge_y = y + 6.0;
        let _ = write!(s,
            r#"<rect x="{badge_x}" y="{badge_y}" rx="8" width="44" height="18" fill="{color}" opacity="0.18"/>
<text x="{btx}" y="{bty}" font-size="10" fill="{color}" font-weight="600" text-anchor="middle">{count} files</text>
"#, btx = badge_x + 22.0, bty = badge_y + 13.0);

        // Group path as subtitle
        let _ = write!(s,
            r#"<text x="{sx}" y="{sy}" font-size="10" fill="{color}" opacity="0.6">{path}</text>
"#, sx = x + 42.0, sy = y + 41.0, path = esc(&gnode.name));

        // Separator line
        let sep_y = y + 46.0;
        let _ = write!(s,
            r#"<line x1="{lx1}" y1="{sep_y}" x2="{lx2}" y2="{sep_y}" stroke="{color}" stroke-width="0.5" opacity="0.2"/>
"#, lx1 = x + 12.0, lx2 = x + w - 12.0);

        // Child file listing
        let max_show = 24;
        for (i, child) in gnode.children.iter().take(max_show).enumerate() {
            let cy = y + 60.0 + i as f64 * 18.0;
            let child_display = if child.chars().count() > 32 {
                truncate(child, 31)
            } else {
                child.clone()
            };
            // Dim bullet
            let _ = write!(s,
                r#"<text x="{bx}" y="{cy}" font-size="8" fill="{color}" opacity="0.4">●</text>
<text x="{tx}" y="{cy}" font-size="10" fill="{FG2}">{child}</text>
"#, bx = x + 16.0, tx = x + 28.0, child = esc(&child_display));
        }
        if gnode.children.len() > max_show {
            let more = gnode.children.len() - max_show;
            let cy = y + 60.0 + max_show as f64 * 18.0;
            let _ = write!(s,
                r#"<text x="{tx}" y="{cy}" font-size="9" fill="{DIM}">+{more} more…</text>
"#, tx = x + 28.0);
        }
    } else {
        // Single file node: type label + optional subtitle
        let type_label = gnode.node_type.label();
        let _ = write!(s,
            r#"<text x="{tx}" y="{ty}" font-size="10" fill="{color}" opacity="0.7">{label}</text>
"#, tx = x + 42.0, ty = y + 42.0, label = esc(type_label));

        if let Some(ref sub) = gnode.subtitle {
            let display_sub = if sub.chars().count() > 28 {
                truncate(sub, 27)
            } else { sub.clone() };
            let _ = write!(s,
                r#"<text x="{sx}" y="{sy}" font-size="8" fill="{FG2}" opacity="0.5">{sub}</text>
"#, sx = x + 42.0, sy = y + 53.0, sub = esc(&display_sub));
        }
    }
}

fn render_arrows(s: &mut String, gd: &GroupedDiagram, layout: &[LayoutNode], svg_w: f64) {
    let mut pos_map: HashMap<usize, (f64, f64, f64, f64, f64)> = HashMap::new();
    for ln in layout {
        let cx = ln.x + ln.w / 2.0;
        pos_map.insert(ln.idx, (cx, ln.x, ln.x + ln.w, ln.y, ln.y + ln.h));
    }

    // Count arrivals per target for spread
    let mut edge_count_per_target: HashMap<usize, usize> = HashMap::new();
    for &(_, to) in &gd.edges { *edge_count_per_target.entry(to).or_insert(0) += 1; }
    let mut edge_arrival_idx: HashMap<usize, usize> = HashMap::new();
    let mut labeled_count = 0;

    for &(from_idx, to_idx) in &gd.edges {
        if from_idx == to_idx { continue; }
        let Some(&(from_cx, _fl, _fr, _ftop, fbot)) = pos_map.get(&from_idx) else { continue; };
        let Some(&(_to_cx, tl, tr, ttop, _tbot)) = pos_map.get(&to_idx) else { continue; };

        let source = &gd.groups[from_idx];
        let color = source.node_type.color();

        // Spread arrival points
        let arrival = edge_arrival_idx.entry(to_idx).or_insert(0);
        let total = *edge_count_per_target.get(&to_idx).unwrap_or(&1);
        let to_x = if total > 1 {
            let sw = tr - tl - 40.0;
            (tl + 20.0) + (*arrival as f64 / (total - 1).max(1) as f64) * sw
        } else {
            (tl + tr) / 2.0
        };
        *arrival += 1;

        let sy = fbot + 3.0;
        let ey = ttop - 3.0;

        if sy < ey {
            let dy = ey - sy;
            let cy1 = sy + dy * 0.3;
            let cy2 = sy + dy * 0.7;
            let _ = write!(s,
                r#"<path d="M {from_cx},{sy} C {from_cx},{cy1} {to_x},{cy2} {to_x},{ey}" stroke="{color}" stroke-width="1.8" fill="none" opacity="0.45"/>
"#);
            // Label only key structural arrows (max 6)
            let target = &gd.groups[to_idx];
            let label = arrow_label(source, target);
            let is_key = source.node_type == NodeType::FlakeRoot
                || source.node_type == NodeType::FlakeInput
                || label == "builds"
                || label == "imports";
            if is_key && labeled_count < 6 {
                let mx = (from_cx + to_x) / 2.0;
                let my = (sy + ey) / 2.0;
                let off = if (from_cx - to_x).abs() < 20.0 { 16.0 } else { 0.0 };
                let _ = write!(s,
                    r#"<text x="{mx}" y="{my}" font-size="8" fill="{color}" opacity="0.35" text-anchor="middle">{label}</text>
"#, mx = mx + off, my = my - 5.0);
                labeled_count += 1;
            }
        } else {
            // Same-level: dashed bezier routed around side
            let side = if from_cx < svg_w / 2.0 {
                (from_cx.min(to_x) - 45.0).max(12.0)
            } else {
                (from_cx.max(to_x) + 45.0).min(svg_w - 12.0)
            };
            let _ = write!(s,
                r#"<path d="M {from_cx},{fbot} C {side},{fby} {side},{tty} {to_x},{ttop}" stroke="{color}" stroke-width="1.2" fill="none" opacity="0.25" stroke-dasharray="5,3"/>
"#,
                fby = fbot + 20.0, tty = ttop - 20.0,
            );
        }
    }
}

/// Smart label for arrows based on source→target relationship.
fn arrow_label(source: &GNode, target: &GNode) -> &'static str {
    let sn = &source.name;
    let tn = &target.name;

    // flake.nix → anything = "outputs"
    if source.node_type == NodeType::FlakeRoot {
        return "outputs";
    }
    // flake input → flake = "input"
    if source.node_type == NodeType::FlakeInput {
        return "input";
    }
    // nixosConfigurations → hosts = "builds"
    if sn == "nixosConfigurations" || sn.starts_with("nixosConfig") {
        return "builds";
    }
    // hosts → modules = "imports"
    if sn.starts_with("hosts") && tn.starts_with("modules") {
        return "imports";
    }
    // hosts → profiles = "uses"
    if sn.starts_with("hosts") && (tn == "profiles" || tn == "users" || tn == "scripts") {
        return "uses";
    }
    // parent → child directory = "contains"
    if tn.starts_with(sn) {
        return "contains";
    }
    "imports"
}

fn render_legend(s: &mut String, total_h: f64, svg_w: f64, gd: &GroupedDiagram) {
    let ly = total_h - LEGEND_H - FOOTER_H;
    let _ = write!(s,
        r#"<line x1="{PAD}" y1="{ly}" x2="{x2}" y2="{ly}" stroke="{CARD_BORDER}" stroke-width="1"/>
"#, x2 = svg_w - PAD);

    let mut used_types: Vec<&NodeType> = Vec::new();
    let all_types = [
        NodeType::FlakeRoot, NodeType::FlakeInput, NodeType::SystemConfig,
        NodeType::HardwareConfig, NodeType::HomeManager, NodeType::CustomModule,
    ];
    for nt in &all_types {
        if gd.groups.iter().any(|g| &g.node_type == nt) {
            used_types.push(nt);
        }
    }

    let item_w = 155.0;
    let total_w = used_types.len() as f64 * item_w;
    let start_x = (svg_w - total_w) / 2.0;
    let dot_y = ly + 26.0;

    for (i, nt) in used_types.iter().enumerate() {
        let x = start_x + i as f64 * item_w;
        let _ = write!(s,
            r#"<circle cx="{cx}" cy="{dot_y}" r="5" fill="{c}"/>
<text x="{tx}" y="{ty}" font-size="11" fill="{FG2}">{label}</text>
"#, cx = x + 6.0, tx = x + 18.0, ty = dot_y + 4.0, c = nt.color(), label = esc(nt.label()));
    }

    let arrow_y = dot_y + 24.0;
    let _ = write!(s,
        r#"<line x1="{ax1}" y1="{arrow_y}" x2="{ax2}" y2="{arrow_y}" stroke="{FG2}" stroke-width="1.5" opacity="0.5"/>
<text x="{atx}" y="{aty}" font-size="10" fill="{DIM}">= imports / depends on</text>
"#,
        ax1 = svg_w / 2.0 - 80.0, ax2 = svg_w / 2.0 - 40.0,
        atx = svg_w / 2.0 - 30.0, aty = arrow_y + 4.0,
    );
}

fn render_footer(s: &mut String, total_h: f64, svg_w: f64, info: &DiagramInfo) {
    let fy = total_h - FOOTER_H + 10.0;
    let fc = info.nodes.iter().filter(|n| n.node_type != NodeType::FlakeInput).count();
    let ic = info.flake_inputs.len();
    let ec = info.edges.len();

    let mut parts = vec![format!("{} files", fc)];
    if ic > 0 { parts.push(format!("{} flake inputs", ic)); }
    parts.push(format!("{} connections", ec));
    let summary = parts.join("  ·  ");

    let _ = write!(s,
        r#"<text x="{cx}" y="{fy}" font-size="12" fill="{FG2}" text-anchor="middle">{summary}</text>
<text x="{cx}" y="{by}" font-size="10" fill="{DIM}" text-anchor="middle">generated with nixmate  ·  github.com/daskladas/nixmate</text>
"#, cx = svg_w / 2.0, summary = esc(&summary), by = fy + 22.0);
}

fn generate_empty_diagram(hostname: &str) -> String {
    let w = 1200.0;
    let h = 400.0;
    let mut s = String::with_capacity(2048);
    let _ = write!(s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<defs>
<style>@import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600;700&amp;display=swap');
text {{ font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace; }}</style>
<linearGradient id="topbar" x1="0" y1="0" x2="1" y2="0">
<stop offset="0%" stop-color="{BLUE}"/><stop offset="25%" stop-color="{PURPLE}"/>
<stop offset="50%" stop-color="{PINK}"/><stop offset="75%" stop-color="{ORANGE}"/>
<stop offset="100%" stop-color="{GREEN}"/>
</linearGradient>
<pattern id="grid" width="30" height="30" patternUnits="userSpaceOnUse">
<circle cx="15" cy="15" r="0.5" fill="{DIM}" opacity="0.15"/>
</pattern>
</defs>
<rect width="{w}" height="{h}" rx="16" fill="{BG}"/>
<rect width="{w}" height="{h}" rx="16" fill="url(#grid)"/>
<rect width="{w}" height="4" rx="2" fill="url(#topbar)"/>
<text x="{PAD}" y="40" font-size="11" fill="{DIM}" letter-spacing="3" font-weight="600">NIXMATE  ·  CONFIG DIAGRAM</text>
<text x="{PAD}" y="85" font-size="36" fill="{FG}" font-weight="700">{hostname}</text>
<text x="{cx}" y="220" font-size="16" fill="{FG2}" text-anchor="middle">No NixOS configuration found at /etc/nixos/</text>
<text x="{cx}" y="250" font-size="13" fill="{DIM}" text-anchor="middle">Make sure your NixOS config files are accessible</text>
<text x="{cx}" y="{by}" font-size="10" fill="{DIM}" text-anchor="middle">generated with nixmate  ·  github.com/daskladas/nixmate</text>
</svg>"#,
        cx = w / 2.0, hostname = esc(hostname), by = h - 20.0
    );
    s
}

/// Save diagram SVG to file.
pub fn save_diagram_svg(info: &DiagramInfo) -> std::io::Result<PathBuf> {
    let svg = generate_diagram_svg(info);
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nixmate-poster");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}-config-diagram.svg", info.hostname));
    std::fs::write(&path, &svg)?;
    Ok(path)
}

// ═══════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════

fn truncate(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
