use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// On-disk whitelist of project roots that are allowed to receive writes
/// inside their own `.agentdesk/` subdirectory. A project MUST be present
/// here before memory indexing, audit records, or CLAUDE.md integration
/// may write files into the project tree. Missing entries → fall back to
/// user-level storage.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct ApprovedProjectsFile {
    /// Canonical absolute project root paths.
    #[serde(default)]
    approved: Vec<String>,
}

fn approved_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdesk")
        .join("approved_projects.json")
}

fn load() -> ApprovedProjectsFile {
    let path = approved_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(file: &ApprovedProjectsFile) -> Result<(), String> {
    let dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdesk");
    fs::create_dir_all(&dir).map_err(|e| format!("创建 .agentdesk 目录失败: {}", e))?;

    let json = serde_json::to_string_pretty(file)
        .map_err(|e| format!("序列化白名单失败: {}", e))?;

    let final_path = approved_path();
    let tmp_path = final_path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        f.write_all(json.as_bytes())
            .map_err(|e| format!("写入白名单失败: {}", e))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp_path, &final_path)
        .map_err(|e| format!("重命名白名单文件失败: {}", e))
}

/// True if the given project root is approved for in-tree writes.
///
/// **Fail-closed**: any error reading the whitelist is treated as "not
/// approved" rather than granting access.
pub fn is_approved(project_root: &Path) -> bool {
    let canonical = canonicalize(project_root);
    let Some(canonical) = canonical else { return false };
    let file = load();
    let set: HashSet<String> = file.approved.into_iter().collect();
    set.contains(&canonical.to_string_lossy().to_string())
}

/// Add a project to the whitelist. Idempotent.
pub fn approve(project_root: &Path) -> Result<(), String> {
    let canonical = canonicalize(project_root)
        .ok_or_else(|| "无法解析项目路径".to_string())?;
    let canonical_str = canonical.to_string_lossy().to_string();
    let mut file = load();
    if !file.approved.iter().any(|p| p == &canonical_str) {
        file.approved.push(canonical_str);
    }
    save(&file)
}

/// Remove a project from the whitelist. Does not delete any on-disk data.
pub fn revoke(project_root: &Path) -> Result<(), String> {
    let canonical = canonicalize(project_root)
        .ok_or_else(|| "无法解析项目路径".to_string())?;
    let canonical_str = canonical.to_string_lossy().to_string();
    let mut file = load();
    file.approved.retain(|p| p != &canonical_str);
    save(&file)
}

/// Return every approved project root, canonical form.
#[allow(dead_code)]
pub fn list() -> Vec<PathBuf> {
    load()
        .approved
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Resolve symlinks and canonicalize to an absolute path. Returns `None`
/// when the path does not exist.
fn canonicalize(p: &Path) -> Option<PathBuf> {
    fs::canonicalize(p).ok()
}
