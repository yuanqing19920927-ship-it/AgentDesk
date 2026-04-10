//! Codex session discovery.
//!
//! Codex (the OpenAI desktop coding agent) writes its per-turn
//! rollout logs to `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`.
//! These files are structurally different from Claude Code's JSONL
//! sessions — see `cost_tracker::cost_for_codex_session_file` for the
//! shape — but they do contain the working directory inside the
//! `session_meta.payload.cwd` field, which lets us bind each file to
//! an existing AgentDesk project.
//!
//! This module is intentionally narrow: it only discovers files and
//! groups them by resolved project root. Cost parsing lives in
//! `cost_tracker`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Walk `~/.codex/sessions` and return a map of `project_root →
/// list of session file paths`. "Project root" is the cwd recorded in
/// the first `session_meta` line of each rollout, canonicalized and
/// resolved up to the enclosing git root (same convention used for
/// Claude Code projects).
///
/// Runs synchronously. The session tree is small (one file per turn)
/// and the caller invokes this from the same background thread that
/// scans Claude Code projects, so this keeps things simple.
pub fn scan_codex_sessions() -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut result: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    let Some(home) = dirs::home_dir() else { return result };
    let root = home.join(".codex").join("sessions");
    if !root.exists() {
        return result;
    }

    // Layout: sessions/<year>/<month>/<day>/rollout-*.jsonl
    for year in iter_dirs(&root) {
        for month in iter_dirs(&year) {
            for day in iter_dirs(&month) {
                let Ok(entries) = fs::read_dir(&day) else { continue };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }
                    let Some(cwd) = extract_cwd(&path) else { continue };
                    let project_root = resolve_project_root(&cwd);
                    result.entry(project_root).or_default().push(path);
                }
            }
        }
    }

    result
}

/// Read a rollout file and return the `cwd` recorded in its first
/// `session_meta` record, if any. We only need the first 64KB of the
/// file to find it — the meta block is always near the top.
fn extract_cwd(path: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines().take(50) {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if record.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
            continue;
        }
        let cwd = record
            .get("payload")
            .and_then(|p| p.get("cwd"))
            .and_then(|c| c.as_str())?;
        return Some(PathBuf::from(cwd));
    }
    None
}

/// Canonicalize a cwd and walk up to the enclosing git root, mirroring
/// `project_scanner::resolve_project_root`. Kept local so we don't
/// create a public dependency across services.
fn resolve_project_root(cwd: &Path) -> PathBuf {
    if !cwd.exists() {
        return cwd.to_path_buf();
    }
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
            fs::canonicalize(&root).unwrap_or_else(|_| PathBuf::from(root))
        }
        _ => fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf()),
    }
}

fn iter_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect()
}
