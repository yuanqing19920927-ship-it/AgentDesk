use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    /// Directories to scan for agent session data
    pub scan_dirs: Vec<String>,
    /// Custom groups: group_name -> list of project root paths
    #[serde(default)]
    pub groups: Vec<GroupDef>,
    /// Project -> group assignment: project_root_path -> group_name
    #[serde(default)]
    pub project_groups: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GroupDef {
    pub name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_dir = dirs::home_dir()
            .map(|h| h.join(".claude").join("projects").to_string_lossy().to_string())
            .unwrap_or_else(|| "~/.claude/projects".to_string());
        Self {
            scan_dirs: vec![default_dir],
            groups: vec![],
            project_groups: HashMap::new(),
        }
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".agentdesk").join("config.json")
}

pub fn load_config() -> AppConfig {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(cfg: &AppConfig) {
    let dir = dirs::home_dir().unwrap_or_default().join(".agentdesk");
    let _ = fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = fs::write(config_path(), json);
    }
}

// ── Scan dirs ──

pub fn add_scan_dir(dir: &str) -> Result<(), String> {
    let canonical = fs::canonicalize(dir)
        .map_err(|e| format!("无法解析路径: {}", e))?
        .to_string_lossy().to_string();
    let mut cfg = load_config();
    if cfg.scan_dirs.contains(&canonical) { return Err("该扫描目录已存在".to_string()); }
    cfg.scan_dirs.push(canonical);
    save_config(&cfg);
    Ok(())
}

pub fn remove_scan_dir(dir: &str) {
    let mut cfg = load_config();
    cfg.scan_dirs.retain(|d| d != dir);
    if cfg.scan_dirs.is_empty() { cfg = AppConfig::default(); }
    save_config(&cfg);
}

// ── Groups ──

pub fn add_group(name: &str) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("分组名不能为空".to_string()); }
    let mut cfg = load_config();
    if cfg.groups.iter().any(|g| g.name == name) { return Err("分组已存在".to_string()); }
    cfg.groups.push(GroupDef { name });
    save_config(&cfg);
    Ok(())
}

pub fn remove_group(name: &str) {
    let mut cfg = load_config();
    cfg.groups.retain(|g| g.name != name);
    cfg.project_groups.retain(|_, v| v != name);
    save_config(&cfg);
}

pub fn set_project_group(project_path: &str, group_name: &str) {
    let mut cfg = load_config();
    if group_name.is_empty() {
        cfg.project_groups.remove(project_path);
    } else {
        cfg.project_groups.insert(project_path.to_string(), group_name.to_string());
    }
    save_config(&cfg);
}

pub fn get_project_group(project_path: &str) -> Option<String> {
    let cfg = load_config();
    cfg.project_groups.get(project_path).cloned()
}
