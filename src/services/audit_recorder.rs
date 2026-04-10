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

// ──────────────────────── diff export ────────────────────────

/// Generate a unified diff text describing how to reach the snapshot's
/// recorded state from the *current* working tree. Users then save
/// this as a `.patch` they can apply elsewhere or hand to a reviewer.
///
/// Strategy:
/// 1. If the snapshot has a recorded HEAD SHA and the repo still has
///    it, run `git diff <sha>` — this is a real patch showing the
///    delta between that commit and the working tree.
/// 2. Otherwise fall back to an informational "snapshot listing"
///    header so the file is still useful for audit purposes.
///
/// The untracked files from the snapshot are appended as a note at the
/// end so reviewers know they existed at snapshot time.
pub fn export_diff_text(project_root: &Path, snap: &AuditSnapshot) -> Result<String, String> {
    let mut out = String::new();

    // Header — human-readable summary of the snapshot we're diffing from.
    out.push_str(&format!("# AgentDesk audit diff\n"));
    out.push_str(&format!("# project : {}\n", project_root.display()));
    out.push_str(&format!("# snapshot: {}\n", snap.id));
    out.push_str(&format!("# taken  : {}\n", snap.timestamp.to_rfc3339()));
    if let Some(b) = &snap.branch {
        out.push_str(&format!("# branch : {}\n", b));
    }
    if let Some(s) = &snap.head_sha {
        out.push_str(&format!("# head   : {}\n", s));
    }
    if let Some(label) = &snap.label {
        out.push_str(&format!("# label  : {}\n", label));
    }
    out.push_str("#\n");
    out.push_str("# Apply with:  git apply <this-file>\n");
    out.push_str("#\n\n");

    if let Some(sha) = &snap.head_sha {
        // Verify the SHA still exists in the repo before diffing —
        // otherwise `git diff` would error out and leave the user
        // without an export.
        let exists = Command::new("git")
            .arg("-C")
            .arg(project_root)
            .args(["cat-file", "-e", sha])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if exists {
            let diff_out = Command::new("git")
                .arg("-C")
                .arg(project_root)
                .args(["diff", sha])
                .output()
                .map_err(|e| format!("执行 git diff 失败: {}", e))?;
            if !diff_out.status.success() {
                let err = String::from_utf8_lossy(&diff_out.stderr).to_string();
                return Err(format!("git diff 失败: {}", err));
            }
            out.push_str(&String::from_utf8_lossy(&diff_out.stdout));
        } else {
            out.push_str(&format!(
                "# warning: commit {} no longer exists in this repo;\n",
                sha
            ));
            out.push_str("# emitting snapshot file listing only.\n\n");
            append_listing(&mut out, snap);
        }
    } else {
        append_listing(&mut out, snap);
    }

    // Always append the untracked list as a non-patch note — `git diff`
    // never includes untracked files so the reviewer would otherwise
    // miss them.
    if !snap.untracked.is_empty() {
        out.push_str("\n# untracked at snapshot time:\n");
        for p in &snap.untracked {
            out.push_str(&format!("#   {}\n", p));
        }
    }

    Ok(out)
}

fn append_listing(out: &mut String, snap: &AuditSnapshot) {
    let buckets: [(&str, &Vec<String>); 5] = [
        ("modified", &snap.modified),
        ("added", &snap.added),
        ("deleted", &snap.deleted),
        ("renamed", &snap.renamed),
        ("untracked", &snap.untracked),
    ];
    for (label, list) in buckets {
        if list.is_empty() {
            continue;
        }
        out.push_str(&format!("# {}:\n", label));
        for p in list {
            out.push_str(&format!("#   {}\n", p));
        }
    }
}

/// Write a diff export to disk. The caller picks the path via
/// `pick_diff_save_path`.
pub fn write_diff_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {}", e))?;
    }
    fs::write(path, content).map_err(|e| format!("写入 diff 文件失败: {}", e))
}

/// Prompt the user for an output `.patch` location. Cancel → `Ok(None)`.
/// Uses the same `choose file name` pattern as `bundle_io`.
pub fn pick_diff_save_path(default_name: &str) -> Result<Option<PathBuf>, String> {
    let safe_default = default_name.replace('"', "");
    let script = format!(
        r#"set target to choose file name with prompt "导出 Git diff 补丁" default name "{}"
return POSIX path of target"#,
        safe_default
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("调用 osascript 失败: {}", e))?;
    if !output.status.success() {
        return Ok(None);
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        Ok(None)
    } else {
        let mut pb = PathBuf::from(path);
        if pb.extension().is_none() {
            pb.set_extension("patch");
        }
        Ok(Some(pb))
    }
}

/// Pop a blocking macOS confirmation dialog. Used before destructive
/// operations like rollback. Returns true if the user clicked OK.
pub fn confirm_dialog(message: &str) -> bool {
    let safe = message.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"display dialog "{}" buttons {{"取消", "确认回滚"}} default button "取消" with icon caution"#,
        safe
    );
    let output = match Command::new("osascript").arg("-e").arg(&script).output() {
        Ok(o) => o,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }
    // When OK is clicked, osascript returns `button returned:确认回滚`.
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("确认回滚")
}

// ──────────────────────── rollback ────────────────────────

/// Roll the project back to the recorded HEAD of a snapshot. Current
/// uncommitted work (tracked *and* untracked) is stashed with a
/// recognizable message so the user can recover it with
/// `git stash list` / `git stash pop`.
///
/// Returns a human-readable summary (stash ref + new HEAD) on success.
pub fn rollback_to_snapshot(
    project_root: &Path,
    snap: &AuditSnapshot,
) -> Result<String, String> {
    if !project_root.join(".git").exists() {
        return Err("该项目不是 git 仓库，无法回滚".to_string());
    }
    let sha = snap
        .head_sha
        .as_ref()
        .ok_or_else(|| "快照没有记录 HEAD SHA，无法回滚".to_string())?;

    // Verify the target commit still exists.
    let exists = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["cat-file", "-e", sha])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !exists {
        return Err(format!("提交 {} 已不存在于本地仓库", sha));
    }

    // Stash current work (tracked + untracked) so nothing is lost.
    let stash_msg = format!("agentdesk-rollback-{}", snap.id);
    let stash_out = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["stash", "push", "-u", "-m", &stash_msg])
        .output()
        .map_err(|e| format!("执行 git stash 失败: {}", e))?;
    // Note: `git stash push` returns success with "No local changes to save"
    // when the tree is clean — we treat that as fine and continue.
    let stash_stdout = String::from_utf8_lossy(&stash_out.stdout).to_string();
    let stashed = stash_out.status.success()
        && !stash_stdout.contains("No local changes to save");
    if !stash_out.status.success() {
        let err = String::from_utf8_lossy(&stash_out.stderr).to_string();
        return Err(format!("git stash 失败: {}", err));
    }

    // Hard reset to the snapshot SHA.
    let reset_out = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["reset", "--hard", sha])
        .output()
        .map_err(|e| format!("执行 git reset 失败: {}", e))?;
    if !reset_out.status.success() {
        let err = String::from_utf8_lossy(&reset_out.stderr).to_string();
        return Err(format!("git reset 失败: {}", err));
    }

    let short = sha.chars().take(7).collect::<String>();
    if stashed {
        Ok(format!(
            "已回滚到 {} · 原有改动已暂存到 stash: {}",
            short, stash_msg
        ))
    } else {
        Ok(format!("已回滚到 {} · 工作区此前已是干净状态", short))
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
