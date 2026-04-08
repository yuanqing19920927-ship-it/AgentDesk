use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    /// Directories to scan for agent session data (JSONL files)
    /// Default: ["~/.claude/projects"]
    pub scan_dirs: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_dir = dirs::home_dir()
            .map(|h| h.join(".claude").join("projects").to_string_lossy().to_string())
            .unwrap_or_else(|| "~/.claude/projects".to_string());
        Self {
            scan_dirs: vec![default_dir],
        }
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdesk")
        .join("config.json")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    fs::read_to_string(&path)
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

pub fn add_scan_dir(dir: &str) -> Result<(), String> {
    let path = PathBuf::from(dir);
    if !path.is_dir() {
        return Err(format!("目录不存在: {}", dir));
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|e| format!("无法解析路径: {}", e))?
        .to_string_lossy().to_string();

    let mut cfg = load_config();
    if cfg.scan_dirs.contains(&canonical) {
        return Err("该扫描目录已存在".to_string());
    }
    cfg.scan_dirs.push(canonical);
    save_config(&cfg);
    Ok(())
}

pub fn remove_scan_dir(dir: &str) {
    let mut cfg = load_config();
    cfg.scan_dirs.retain(|d| d != dir);
    if cfg.scan_dirs.is_empty() {
        // Always keep at least the default
        cfg = AppConfig::default();
    }
    save_config(&cfg);
}
