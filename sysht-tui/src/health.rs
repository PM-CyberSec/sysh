use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Structured health snapshot parsed from sysh raw output.
/// We keep it simple: raw text per section + parsed JSON for overview.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Timestamp of snapshot
    pub generated_at: String,
    /// Raw sections: title -> lines
    pub sections: HashMap<String, Vec<String>>,
    /// Ordered list of section titles as they appear
    pub order: Vec<String>,
    /// Full raw output lines (stripped ANSI)
    pub raw_lines: Vec<String>,
    /// Parsed JSON overview if available
    pub json_overview: Option<serde_json::Value>,
    /// Path to temp raw file
    pub raw_path: String,
}

impl Snapshot {
    pub fn section_text(&self, title: &str) -> String {
        self.sections.get(title).map(|v| v.join("\n")).unwrap_or_default()
    }
    pub fn total_sections(&self) -> usize { self.order.len() }
}

/// Run sysh bash script and capture output.
/// `sysh_path` is path to sysh bash script.
/// `top_n` controls Top-N tables.
/// `do_virt` enables thin-provisioned virt scan.
pub fn collect(sysh_path: &Path, top_n: usize, timeout_secs: u64) -> Result<Snapshot> {
    collect_with_virt(sysh_path, top_n, timeout_secs, false)
}
pub fn collect_with_virt(sysh_path: &Path, top_n: usize, timeout_secs: u64, do_virt: bool) -> Result<Snapshot> {
    // First, try to produce raw report with no-file, no-color, timeout.
    // Use bash with timeout wrapper if available.
    let raw_path = format!("/tmp/sysht-raw-{}.txt", std::process::id());
    let json_path = format!("/tmp/sysht-json-{}.json", std::process::id());

    // Build command: timeout 60 sysh --no-file --no-color -n TOP_N
    // We also separately get json via sysh --json-only
    let sysh_str = sysh_path.to_string_lossy().to_string();
    let top_s = top_n.to_string();

    // Capture raw via command
    let mut args = vec!["--no-file".to_string(), "--no-color".to_string(), "-n".to_string(), top_s.clone()];
    if do_virt { args.push("--virt".to_string()); }
    let output = if which("timeout").is_some() {
        let mut cmd_args = vec![timeout_secs.to_string(), "bash".to_string(), sysh_str.clone()];
        cmd_args.extend(args.clone());
        Command::new("timeout").args(cmd_args).output()
    } else {
        let mut cmd_args = vec![sysh_str.clone()];
        cmd_args.extend(args.clone());
        Command::new("bash").args(cmd_args).output()
    }?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let mut raw_lines: Vec<String> = raw.lines().map(|s| s.to_string()).collect();
    if raw_lines.is_empty() && !output.stderr.is_empty() {
        raw_lines = String::from_utf8_lossy(&output.stderr).lines().map(|s| s.to_string()).collect();
    }
    // also try to get json
    let mut json_args = vec!["--json-only".to_string(), "-n".to_string(), top_s];
    if do_virt { json_args.push("--virt".to_string()); }
    let mut json_cmd = vec![sysh_str];
    json_cmd.extend(json_args);
    let json_out = Command::new("bash").args(json_cmd).output();
    let json_val = if let Ok(o) = json_out {
        let s = String::from_utf8_lossy(&o.stdout).to_string();
        serde_json::from_str(&s).ok()
    } else { None };

    // Write raw to file for debugging / external view
    let _ = std::fs::write(&raw_path, raw_lines.join("\n"));
    if let Some(ref j) = json_val {
        let _ = std::fs::write(&json_path, serde_json::to_string_pretty(j).unwrap_or_default());
    }

    // Parse sections by delimiter "━━━ TITLE ━"
    let mut sections: HashMap<String, Vec<String>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in &raw_lines {
        if line.contains("━━━") {
            // Extract title between ━━━ and ━
            // Example: "━━━ DISK — Filesystems ━━━━━━━━━━━━━━━━━..."
            let title = line.replace('━', " ").trim().to_string();
            // Clean: remove leading "━━━" and trailing "━"
            let t = line.trim().trim_start_matches('━').trim_start_matches('─').trim_start_matches('━').to_string();
            // Alternative heuristic: split by whitespace, take words between
            // Simpler: use the segment between "━━━" markers
            let parsed = parse_section_title(line);
            let key = if parsed.is_empty() { line.clone() } else { parsed };
            current = Some(key.clone());
            if !order.contains(&key) {
                order.push(key.clone());
            }
            sections.entry(key).or_default().push(line.clone());
        } else if let Some(ref k) = current {
            sections.entry(k.clone()).or_default().push(line.clone());
        } else {
            // Before first section -> treat as header
            let k = "HEADER".to_string();
            if !order.contains(&k) { order.push(k.clone()); }
            sections.entry(k.clone()).or_default().push(line.clone());
            current = Some(k);
        }
    }

    Ok(Snapshot {
        generated_at: chrono_now(),
        sections,
        order,
        raw_lines,
        json_overview: json_val,
        raw_path,
    })
}

fn parse_section_title(line: &str) -> String {
    // line like "━━━ SYSTEM HEADER ━━━━━━━━━━━━━━━━━━━"
    // Extract middle
    let trimmed = line.trim();
    // Find first and second "━━━"
    let without = trimmed.trim_matches(|c| c == '━' || c == '─' || c == ' ' );
    // fallback simple
    let lower = without.to_string();
    // Try to strip leading/trailing dashes
    // If contains "HEADER", etc, return that word chunk.
    // We'll just remove box chars and trim.
    let cleaned: String = trimmed.chars().filter(|c| *c != '━' && *c != '─').collect();
    cleaned.trim().to_string()
}

fn chrono_now() -> String {
    // Use date command for portability without chrono crate
    Command::new("date").arg("-Is").output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn which(cmd: &str) -> Option<String> {
    Command::new("which").arg(cmd).output().ok().and_then(|o| {
        if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None }
    })
}
