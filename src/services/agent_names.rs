//! Per-agent alias storage.
//!
//! Stored at `~/.agentdesk/agent_names.json` as
//! `{ project_root: { agent_key: alias } }`.
//!
//! `agent_key` prefers `tty:{tty}` because TTY is more stable across
//! re-execs of the same agent (e.g. exiting `claude` and running it
//! again in the same iTerm tab), and falls back to `pid:{pid}` when
//! the agent has no tty attached.
//!
//! Written to user-level config and therefore **not** subject to the
//! project write whitelist — per the design doc.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

type AliasMap = HashMap<String, HashMap<String, String>>;

fn storage_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdesk")
        .join("agent_names.json")
}

/// Compute the stable identity key for an agent within a project.
pub fn agent_key(tty: Option<&str>, pid: u32) -> String {
    match tty {
        Some(t) if !t.is_empty() => format!("tty:{}", t),
        _ => format!("pid:{}", pid),
    }
}

pub fn load_all() -> AliasMap {
    fs::read_to_string(storage_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_all(map: &AliasMap) -> Result<(), String> {
    let dir = dirs::home_dir().unwrap_or_default().join(".agentdesk");
    fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
    let json = serde_json::to_string_pretty(map)
        .map_err(|e| format!("序列化失败: {}", e))?;
    write_atomic(&storage_path(), &json)
}

/// Fetch an alias for a specific agent inside a project.
#[allow(dead_code)]
pub fn get_alias(project_root: &Path, tty: Option<&str>, pid: u32) -> Option<String> {
    let map = load_all();
    let pr = project_root.to_string_lossy().to_string();
    let key = agent_key(tty, pid);
    map.get(&pr).and_then(|m| m.get(&key)).cloned()
}

/// Set or clear an alias. Empty string clears the entry.
pub fn set_alias(project_root: &Path, tty: Option<&str>, pid: u32, alias: &str) -> Result<(), String> {
    let mut map = load_all();
    let pr = project_root.to_string_lossy().to_string();
    let key = agent_key(tty, pid);
    let trimmed = alias.trim();

    let bucket = map.entry(pr.clone()).or_default();
    if trimmed.is_empty() {
        bucket.remove(&key);
        if bucket.is_empty() {
            map.remove(&pr);
        }
    } else {
        bucket.insert(key, trimmed.to_string());
    }
    save_all(&map)
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("创建临时文件失败: {}", e))?;
        f.write_all(content.as_bytes()).map_err(|e| format!("写入失败: {}", e))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| format!("重命名失败: {}", e))
}
