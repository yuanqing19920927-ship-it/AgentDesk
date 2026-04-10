//! Project memory indexer.
//!
//! Scans Claude Code JSONL session files under `~/.claude/projects/<dir>/`
//! and produces:
//!
//! * `index.json`      — structured memory index with scan cursors
//! * `memory.md`       — human-readable summary (read by Agents)
//! * `sessions/*.md`   — per-date archive of session summaries
//!
//! **Storage mode is decided per project:**
//!
//! * **Project-local** — `{project}/.agentdesk/` — only when the project
//!   is on the approved whitelist AND the `.agentdesk/` directory is not
//!   already tracked by git. Git guards are enforced before any write.
//! * **User-level fallback** — `~/.agentdesk/projects/{path_hash}/` — used
//!   whenever project-local writes are refused. No files are ever written
//!   into the project tree in this mode.
//!
//! Writes are crash-safe: derived artifacts (`memory.md`, `sessions/*.md`)
//! are written first, then `index.json` (which carries the cursor state)
//! is written last via atomic rename. A crash between the two steps means
//! the next scan re-processes some entries, but UUID-based dedup makes
//! that a no-op.

use crate::models::{Cursor, MemoryEntry, MemoryIndex, SessionRecord};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::services::approved_projects;

/// In-process lock keyed by canonical storage root. Ensures only one
/// indexing run per project happens at a time (avoids the full actor
/// infrastructure of the design doc while still preventing torn writes
/// when the UI triggers two scans back-to-back).
static INDEX_LOCKS: Mutex<Option<HashMap<PathBuf, ()>>> = Mutex::new(None);

fn lock_project(root: &Path) -> bool {
    let mut guard = INDEX_LOCKS.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    if map.contains_key(root) {
        return false;
    }
    map.insert(root.to_path_buf(), ());
    true
}

fn unlock_project(root: &Path) {
    let mut guard = INDEX_LOCKS.lock().unwrap();
    if let Some(map) = guard.as_mut() {
        map.remove(root);
    }
}

struct ProjectLockGuard {
    root: PathBuf,
}

impl Drop for ProjectLockGuard {
    fn drop(&mut self) {
        unlock_project(&self.root);
    }
}

/// Storage mode chosen for a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageMode {
    /// Writing to `{project}/.agentdesk/`.
    ProjectLocal,
    /// Writing to `~/.agentdesk/projects/{path_hash}/`.
    UserFallback,
}

/// Summary of a completed scan, returned to the UI.
#[derive(Clone, Debug)]
pub struct ScanReport {
    pub mode: StorageMode,
    pub storage_path: PathBuf,
    pub total_entries: usize,
    pub new_entries: usize,
    pub scanned_files: usize,
    pub skipped_reason: Option<String>,
}

// ──────────────────────── public entry points ────────────────────────

/// Run a full incremental scan for the given project and persist the
/// resulting index + derived files.
///
/// `claude_dir_names` is the list of directories under
/// `~/.claude/projects/` that map to this project (populated by
/// `project_scanner`).
pub fn scan_project(
    project_root: &Path,
    claude_dir_names: &[String],
) -> Result<ScanReport, String> {
    let storage_root = resolve_storage_root(project_root)?;

    if !lock_project(&storage_root.path) {
        return Err("项目扫描正在进行中，请稍候".to_string());
    }
    let _guard = ProjectLockGuard {
        root: storage_root.path.clone(),
    };

    fs::create_dir_all(&storage_root.path)
        .map_err(|e| format!("无法创建存储目录: {}", e))?;
    let sessions_dir = storage_root.path.join("sessions");
    fs::create_dir_all(&sessions_dir)
        .map_err(|e| format!("无法创建 sessions 目录: {}", e))?;

    let index_path = storage_root.path.join("index.json");
    let mut index = load_index(&index_path);

    // Collect every JSONL across all bound claude project dirs.
    let home = dirs::home_dir().unwrap_or_default();
    let mut jsonl_files: Vec<PathBuf> = Vec::new();
    for name in claude_dir_names {
        let dir = home.join(".claude").join("projects").join(name);
        if let Ok(rd) = fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|ext| ext == "jsonl") {
                    jsonl_files.push(p);
                }
            }
        }
    }

    let mut new_entries = 0usize;
    let mut entries_by_date: HashMap<String, Vec<MemoryEntry>> = HashMap::new();

    // Pre-load existing entries by date so rewriting session markdown
    // keeps historical records (we always regenerate the session md file
    // from the full set of entries for that date).
    for entry in &index.entries {
        if let Some(ts) = entry.timestamp {
            let date = ts.format("%Y-%m-%d").to_string();
            entries_by_date.entry(date).or_default().push(entry.clone());
        }
    }

    for jsonl in &jsonl_files {
        let fname = match jsonl.file_name().and_then(|n| n.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let meta = match fs::metadata(jsonl) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let (inode, size) = inode_and_size(&meta);

        // Decide where to start reading based on the stored cursor.
        let start_offset = match index.cursors.get(&fname) {
            Some(c) if c.inode == inode && c.file_size <= size => c.byte_offset,
            _ => 0,
        };

        let content = match fs::read_to_string(jsonl) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let bytes = content.as_bytes();
        if start_offset > bytes.len() as u64 {
            // File shrank somehow; restart from 0.
            index.cursors.insert(fname.clone(), Cursor::default());
        }
        let slice_start = std::cmp::min(start_offset as usize, bytes.len());
        let slice = &content[slice_start..];

        let mut last_uuid: Option<String> = index
            .cursors
            .get(&fname)
            .and_then(|c| c.last_uuid.clone());

        for line in slice.lines() {
            let record: SessionRecord = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Only index user messages — they carry the intent and are
            // dense enough to serve as summaries without LLM help.
            if record.record_type != "user" {
                continue;
            }

            let Some(uuid) = record.uuid.clone() else { continue };
            if index.has_uuid(&uuid) {
                last_uuid = Some(uuid);
                continue;
            }

            let text = extract_text(&record);
            let summary_text = filter_secrets(&truncate(&text, 500));
            if summary_text.trim().is_empty() {
                last_uuid = Some(uuid);
                continue;
            }

            let ts = record.timestamp.as_deref().and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });
            let date_key = ts
                .map(|t| t.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let id = index.next_entry_id();
            let session_id = record
                .session_id
                .clone()
                .unwrap_or_else(|| fname.trim_end_matches(".jsonl").to_string());

            let entry = MemoryEntry {
                id: id.clone(),
                uuid: uuid.clone(),
                session_id,
                timestamp: ts,
                branch: record.git_branch.clone(),
                topics: Vec::new(),
                summary: summary_text.clone(),
                keywords: extract_keywords(&summary_text),
                file_ref: format!("sessions/{}.md#{}", date_key, id),
            };

            index.entries.push(entry.clone());
            entries_by_date
                .entry(date_key)
                .or_default()
                .push(entry);
            new_entries += 1;
            last_uuid = Some(uuid);
        }

        // Update cursor: read all the way to end of file.
        index.cursors.insert(
            fname,
            Cursor {
                byte_offset: bytes.len() as u64,
                inode,
                file_size: size,
                last_uuid,
            },
        );
    }

    index.last_scan = Some(Utc::now());

    // ── Write order (crash-safe): derived files → index.json ──
    for (date, list) in &entries_by_date {
        let path = sessions_dir.join(format!("{}.md", date));
        write_atomic(&path, &render_sessions_md(date, list))?;
    }
    let memory_md = render_memory_md(&index);
    write_atomic(&storage_root.path.join("memory.md"), &memory_md)?;
    // Index last — its cursor state only advances after derived files land.
    write_index(&index_path, &index)?;

    // CLAUDE.md integration only in project-local mode.
    if matches!(storage_root.mode, StorageMode::ProjectLocal) {
        let _ = super::claudemd_writer::ensure_memory_section(project_root);
    }

    Ok(ScanReport {
        mode: storage_root.mode,
        storage_path: storage_root.path,
        total_entries: index.entries.len(),
        new_entries,
        scanned_files: jsonl_files.len(),
        skipped_reason: None,
    })
}

/// Check the current state of a project's memory without triggering a
/// scan. Returns the resolved storage mode + path even when `index.json`
/// has not yet been written — in that case `total_entries` is 0 and the
/// UI should still surface the correct mode (ProjectLocal vs UserFallback).
pub fn read_report(project_root: &Path) -> Option<ScanReport> {
    let storage = resolve_storage_root(project_root).ok()?;
    let index_path = storage.path.join("index.json");
    let (total, scanned) = if index_path.exists() {
        let index = load_index(&index_path);
        (index.entries.len(), index.cursors.len())
    } else {
        (0, 0)
    };
    Some(ScanReport {
        mode: storage.mode,
        storage_path: storage.path,
        total_entries: total,
        new_entries: 0,
        scanned_files: scanned,
        skipped_reason: None,
    })
}

/// Load the entries for display in the UI, newest first.
pub fn load_entries(project_root: &Path) -> Vec<MemoryEntry> {
    let Ok(storage) = resolve_storage_root(project_root) else { return Vec::new() };
    let index_path = storage.path.join("index.json");
    if !index_path.exists() {
        return Vec::new();
    }
    let mut index = load_index(&index_path);
    index.entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    index.entries
}

// ──────────────────────── storage resolution ────────────────────────

struct StorageRoot {
    path: PathBuf,
    mode: StorageMode,
}

fn resolve_storage_root(project_root: &Path) -> Result<StorageRoot, String> {
    // Try project-local if approved.
    if approved_projects::is_approved(project_root) {
        if let Some(root) = try_project_local(project_root)? {
            return Ok(StorageRoot {
                path: root,
                mode: StorageMode::ProjectLocal,
            });
        }
    }
    // Fallback to user-level storage.
    let hash = path_hash(project_root);
    let user_root = dirs::home_dir()
        .ok_or_else(|| "无法定位 HOME 目录".to_string())?
        .join(".agentdesk")
        .join("projects")
        .join(hash);
    Ok(StorageRoot {
        path: user_root,
        mode: StorageMode::UserFallback,
    })
}

/// Try the project-local mode. Returns `Ok(Some(path))` when writes are
/// allowed in-tree, `Ok(None)` when the git guard rejected it (so the
/// caller should fall back to user-level storage), or `Err` for fatal
/// filesystem errors.
fn try_project_local(project_root: &Path) -> Result<Option<PathBuf>, String> {
    // Design doc: refuse if `.agentdesk/` is already tracked by git.
    if is_git_tracked(project_root, ".agentdesk") {
        return Ok(None);
    }
    let agentdesk_dir = project_root.join(".agentdesk");
    ensure_gitignore(project_root).ok();
    Ok(Some(agentdesk_dir))
}

/// Returns true when `relative` is tracked by git in the given repo. If
/// `project_root` is not a git repo, returns false (safe to write).
fn is_git_tracked(project_root: &Path, relative: &str) -> bool {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["ls-files", "--error-unmatch", relative])
        .output();
    matches!(out, Ok(o) if o.status.success())
}

/// Ensure `.agentdesk/` is listed in the project's `.gitignore`. Creates
/// the file if missing. Appends only when the entry is not already
/// present to avoid duplicating lines on repeated scans.
fn ensure_gitignore(project_root: &Path) -> Result<(), String> {
    let path = project_root.join(".gitignore");
    let current = fs::read_to_string(&path).unwrap_or_default();
    if current
        .lines()
        .any(|l| l.trim() == ".agentdesk/" || l.trim() == ".agentdesk")
    {
        return Ok(());
    }
    let mut new_content = current;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(".agentdesk/\n");
    fs::write(&path, new_content).map_err(|e| format!("更新 .gitignore 失败: {}", e))
}

/// Stable filesystem-safe hash of a path for user-level fallback storage.
///
/// Uses FNV-1a 64-bit — deterministic across runs without adding a
/// crypto dependency. Collisions are cosmetic (two projects share a
/// storage dir), so the trade-off is acceptable for a local-only cache.
fn path_hash(p: &Path) -> String {
    let bytes = p.to_string_lossy().as_bytes().to_vec();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{:016x}", hash)
}

// ──────────────────────── io helpers ────────────────────────

fn load_index(path: &Path) -> MemoryIndex {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_index(path: &Path, index: &MemoryIndex) -> Result<(), String> {
    let json = serde_json::to_string_pretty(index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    write_atomic(path, &json)
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension(
        format!(
            "{}.tmp",
            path.extension().and_then(|e| e.to_str()).unwrap_or("tmp")
        ),
    );
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        f.write_all(content.as_bytes())
            .map_err(|e| format!("写入文件失败: {}", e))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| format!("重命名文件失败: {}", e))
}

#[cfg(unix)]
fn inode_and_size(meta: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (meta.ino(), meta.len())
}

#[cfg(not(unix))]
fn inode_and_size(meta: &fs::Metadata) -> (u64, u64) {
    (0, meta.len())
}

// ──────────────────────── text processing ────────────────────────

fn extract_text(record: &SessionRecord) -> String {
    let Some(message) = record.message.as_ref() else { return String::new() };
    if let Some(s) = message.as_str() {
        return s.to_string();
    }
    if let Some(content) = message.get("content") {
        if let Some(s) = content.as_str() {
            return s.to_string();
        }
        if let Some(arr) = content.as_array() {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(text.to_string());
                }
            }
            return parts.join("\n");
        }
    }
    String::new()
}

fn truncate(s: &str, max_chars: usize) -> String {
    let taken: String = s.chars().take(max_chars).collect();
    if taken.chars().count() < s.chars().count() {
        format!("{}...", taken)
    } else {
        taken
    }
}

/// Pull plausible keywords out of a summary. This is a deliberately
/// naive heuristic (pick words ≥ 4 chars, dedup, cap at 8) — serves as a
/// placeholder until an LLM summary step is added.
fn extract_keywords(text: &str) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for w in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let w = w.trim().to_lowercase();
        if w.len() < 4 || w.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if seen.insert(w.clone()) {
            out.push(w);
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

/// Replace common secret patterns in-place with `[REDACTED]` before a
/// summary hits the disk. This is a coarse net — it targets the shapes
/// most likely to appear in Agent transcripts (API keys, GitHub tokens,
/// `key=value` pairs with a secret-looking name). Not a substitute for
/// a proper DLP scanner.
fn filter_secrets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_inclusive(|c: char| c.is_whitespace()) {
        if looks_like_secret(word.trim_end()) {
            let trailing: String = word.chars().rev().take_while(|c| c.is_whitespace()).collect();
            out.push_str("[REDACTED]");
            out.push_str(&trailing.chars().rev().collect::<String>());
        } else {
            out.push_str(word);
        }
    }
    out
}

fn looks_like_secret(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '=');
    if w.len() < 16 {
        return false;
    }
    let prefixes = [
        "sk-", "ghp_", "gho_", "ghu_", "ghs_", "github_pat_", "xoxb-", "xoxp-", "AIza",
    ];
    if prefixes.iter().any(|p| w.starts_with(p)) {
        return true;
    }
    // `api_key=...` / `password=...` / `token=...`
    let lower = w.to_lowercase();
    for key in ["password=", "api_key=", "apikey=", "secret=", "token="] {
        if let Some(rest) = lower.strip_prefix(key) {
            if rest.len() >= 8 {
                return true;
            }
        }
    }
    false
}

// ──────────────────────── markdown rendering ────────────────────────

fn render_sessions_md(date: &str, entries: &[MemoryEntry]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# 会话摘要 — {}\n\n", date));
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    for e in &sorted {
        out.push_str(&format!("## {}\n", e.id));
        out.push_str(&format!("<a id=\"{}\"></a>\n\n", e.id));
        if let Some(ts) = e.timestamp {
            out.push_str(&format!("- 时间: {}\n", ts.to_rfc3339()));
        }
        out.push_str(&format!("- 会话: {}\n", e.session_id));
        if let Some(branch) = &e.branch {
            out.push_str(&format!("- 分支: {}\n", branch));
        }
        if !e.keywords.is_empty() {
            out.push_str(&format!("- 关键词: {}\n", e.keywords.join(", ")));
        }
        out.push_str("\n");
        out.push_str(&e.summary);
        out.push_str("\n\n---\n\n");
    }
    out
}

fn render_memory_md(index: &MemoryIndex) -> String {
    let mut out = String::new();
    out.push_str("# 项目长记忆\n\n");
    if let Some(ts) = index.last_scan {
        out.push_str(&format!("_最后扫描: {}_\n\n", ts.to_rfc3339()));
    }
    out.push_str(&format!("共 {} 条记忆条目\n\n", index.entries.len()));
    out.push_str("## 最近会话\n\n");

    let mut recent = index.entries.clone();
    recent.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    for e in recent.iter().take(20) {
        let date = e
            .timestamp
            .map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let branch = e
            .branch
            .as_deref()
            .map(|b| format!(" ({})", b))
            .unwrap_or_default();
        let first_line = e.summary.lines().next().unwrap_or("").trim();
        out.push_str(&format!("- **{}{}** [{}]: {}\n", date, branch, e.id, truncate(first_line, 120)));
    }
    out
}
