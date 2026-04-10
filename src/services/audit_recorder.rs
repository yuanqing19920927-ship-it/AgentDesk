//! Module 9 — audit snapshot recorder.
//!
//! Captures point-in-time git snapshots for a project and persists
//! them as JSON files the UI can browse as a timeline. Storage lives
//! under `~/.agentdesk/audits/<path_hash>/` — always user-level for
//! the MVP so we don't have to coordinate with the memory-indexer's
//! project-local git guard.
//!
//! No rollback is performed: snapshots are a read-only audit trail,
//! meant to answer "what changed between before I ran this agent and
//! now?" rather than to revert.

use crate::models::{AuditDiff, AuditSnapshot};
use chrono::Utc;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ──────────────────────── storage ────────────────────────

/// Storage root for a given project's audit snapshots. Mirrors the
/// FNV-1a `path_hash` scheme used by `memory_indexer::user_fallback`.
fn audit_dir(project_root: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let hash = path_hash(project_root);
    Some(home.join(".agentdesk").join("audits").join(hash))
}

fn path_hash(p: &Path) -> String {
    let bytes = p.to_string_lossy().as_bytes().to_vec();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{:016x}", hash)
}

// ──────────────────────── snapshot ────────────────────────

/// Capture a fresh snapshot for `project_root` and persist it.
///
/// Returns the snapshot on success, or an error string on filesystem
/// failures. A project that is not a git repo still produces a valid
/// (empty) snapshot, so the timeline has something to show.
pub fn take_snapshot(
    project_root: &Path,
    label: Option<String>,
) -> Result<AuditSnapshot, String> {
    let now = Utc::now();
    // ID format: `YYYYMMDD-HHMMSS` is both human-sortable and
    // filename-safe on every supported platform.
    let id = now.format("%Y%m%d-%H%M%S").to_string();

    let branch = run_git(project_root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let head_sha = run_git(project_root, &["rev-parse", "HEAD"]);
    let status_raw = run_git(project_root, &["status", "--porcelain=v1"]).unwrap_or_default();

    let mut modified = Vec::new();
    let mut added = Vec::new();
    let mut deleted = Vec::new();
    let mut renamed = Vec::new();
    let mut untracked = Vec::new();
    parse_porcelain(
        &status_raw,
        &mut modified,
        &mut added,
        &mut deleted,
        &mut renamed,
        &mut untracked,
    );

    let snap = AuditSnapshot {
        id: id.clone(),
        timestamp: now,
        label,
        branch,
        head_sha,
        modified,
        added,
        deleted,
        renamed,
        untracked,
    };

    let dir = audit_dir(project_root).ok_or_else(|| "无法定位 HOME 目录".to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建审计目录: {}", e))?;
    let file = dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&snap)
        .map_err(|e| format!("快照序列化失败: {}", e))?;
    fs::write(&file, json).map_err(|e| format!("写入快照失败: {}", e))?;

    Ok(snap)
}

/// Return all snapshots on disk for a project, newest first.
pub fn list_snapshots(project_root: &Path) -> Vec<AuditSnapshot> {
    let Some(dir) = audit_dir(project_root) else { return Vec::new() };
    let Ok(rd) = fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<AuditSnapshot> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| {
            let content = fs::read_to_string(e.path()).ok()?;
            serde_json::from_str::<AuditSnapshot>(&content).ok()
        })
        .collect();
    out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    out
}

/// Delete a single snapshot file by id.
pub fn delete_snapshot(project_root: &Path, id: &str) -> Result<(), String> {
    let dir = audit_dir(project_root).ok_or_else(|| "无法定位 HOME 目录".to_string())?;
    let file = dir.join(format!("{}.json", id));
    if !file.exists() {
        return Err("快照不存在".to_string());
    }
    fs::remove_file(&file).map_err(|e| format!("删除快照失败: {}", e))
}

/// Compute a diff between two snapshots. `files_added` is paths that
/// are dirty in `new` but were clean in `old`, and vice versa for
/// `files_removed`. `files_changed` is paths dirty in both but under
/// a different status kind.
pub fn diff_snapshots(old: &AuditSnapshot, new: &AuditSnapshot) -> AuditDiff {
    let old_map = flatten_with_kind(old);
    let new_map = flatten_with_kind(new);
    let old_paths: HashSet<&String> = old_map.iter().map(|(p, _)| p).collect();
    let new_paths: HashSet<&String> = new_map.iter().map(|(p, _)| p).collect();

    let mut files_added: Vec<String> = new_paths
        .difference(&old_paths)
        .map(|s| (*s).clone())
        .collect();
    let mut files_removed: Vec<String> = old_paths
        .difference(&new_paths)
        .map(|s| (*s).clone())
        .collect();
    let mut files_changed = Vec::new();
    for (path, kind) in &new_map {
        if let Some((_, old_kind)) = old_map.iter().find(|(p, _)| p == path) {
            if old_kind != kind {
                files_changed.push(path.clone());
            }
        }
    }
    files_added.sort();
    files_removed.sort();
    files_changed.sort();

    AuditDiff {
        files_added,
        files_removed,
        files_changed,
        old_sha: old.head_sha.clone(),
        new_sha: new.head_sha.clone(),
        head_changed: old.head_sha != new.head_sha,
    }
}

// ──────────────────────── helpers ────────────────────────

fn run_git(project_root: &Path, args: &[&str]) -> Option<String> {
    if !project_root.join(".git").exists() {
        return None;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn parse_porcelain(
    raw: &str,
    modified: &mut Vec<String>,
    added: &mut Vec<String>,
    deleted: &mut Vec<String>,
    renamed: &mut Vec<String>,
    untracked: &mut Vec<String>,
) {
    for line in raw.lines() {
        if line.len() < 3 {
            continue;
        }
        // Porcelain v1: XY <path>  (with `X` = index, `Y` = work tree).
        let x = line.chars().next().unwrap_or(' ');
        let y = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].trim().to_string();
        if x == '?' && y == '?' {
            untracked.push(path);
        } else if x == 'R' || y == 'R' {
            renamed.push(path);
        } else if x == 'A' || y == 'A' {
            added.push(path);
        } else if x == 'D' || y == 'D' {
            deleted.push(path);
        } else if x == 'M' || y == 'M' {
            modified.push(path);
        }
    }
}

fn flatten_with_kind(snap: &AuditSnapshot) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for p in &snap.modified {
        out.push((p.clone(), "modified"));
    }
    for p in &snap.added {
        out.push((p.clone(), "added"));
    }
    for p in &snap.deleted {
        out.push((p.clone(), "deleted"));
    }
    for p in &snap.renamed {
        out.push((p.clone(), "renamed"));
    }
    for p in &snap.untracked {
        out.push((p.clone(), "untracked"));
    }
    out
}
