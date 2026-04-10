use crate::models::Project;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn custom_projects_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdesk")
        .join("custom_projects.json")
}

/// Load manually added project paths
pub fn load_custom_projects() -> Vec<String> {
    let path = custom_projects_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save manually added project paths
fn save_custom_projects(paths: &[String]) {
    let dir = dirs::home_dir().unwrap_or_default().join(".agentdesk");
    let _ = fs::create_dir_all(&dir);
    let path = custom_projects_path();
    if let Ok(json) = serde_json::to_string_pretty(paths) {
        let _ = fs::write(path, json);
    }
}

/// Add a project path to custom list
pub fn add_custom_project(dir: &str) -> Result<(), String> {
    let path = PathBuf::from(dir);
    if !path.is_dir() {
        return Err(format!("目录不存在: {}", dir));
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|e| format!("无法解析路径: {}", e))?;
    let canonical_str = canonical.to_string_lossy().to_string();

    let mut projects = load_custom_projects();
    let set: HashSet<&str> = projects.iter().map(|s| s.as_str()).collect();
    if set.contains(canonical_str.as_str()) {
        return Err("该项目已存在".to_string());
    }
    projects.push(canonical_str);
    save_custom_projects(&projects);
    Ok(())
}

/// Remove a project path from custom list
pub fn remove_custom_project(dir: &str) {
    let mut projects = load_custom_projects();
    projects.retain(|p| p != dir);
    save_custom_projects(&projects);
}

/// Convert custom project paths to Project structs
pub fn custom_projects_as_models() -> Vec<Project> {
    let paths = load_custom_projects();
    paths.iter().filter_map(|p| {
        let path = PathBuf::from(p);
        if !path.is_dir() { return None; }
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Some(Project {
            root: path,
            name,
            claude_dir_names: Vec::new(),
            codex_session_files: Vec::new(),
            agent_count: 0,
            last_active: None,
            session_count: 0,
        })
    }).collect()
}

// ── Project nicknames ──

fn nicknames_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdesk")
        .join("project_nicknames.json")
}

/// Load project nicknames: path -> nickname
pub fn load_nicknames() -> std::collections::HashMap<String, String> {
    let path = nicknames_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save a nickname for a project
pub fn set_nickname(project_path: &str, nickname: &str) {
    let mut map = load_nicknames();
    if nickname.trim().is_empty() {
        map.remove(project_path);
    } else {
        map.insert(project_path.to_string(), nickname.trim().to_string());
    }
    let dir = dirs::home_dir().unwrap_or_default().join(".agentdesk");
    let _ = fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(nicknames_path(), json);
    }
}

/// Open macOS folder picker dialog, returns selected path or None
pub fn pick_folder() -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"set chosenFolder to choose folder with prompt "选择项目目录"
return POSIX path of chosenFolder"#)
        .output()
        .ok()?;

    if !output.status.success() {
        return None; // User cancelled
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        // Remove trailing slash
        Some(path.trim_end_matches('/').to_string())
    }
}
