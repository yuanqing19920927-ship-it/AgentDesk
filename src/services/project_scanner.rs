use crate::models::{Project, SessionRecord};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn load_project_map() -> HashMap<String, String> {
    let path = dirs::home_dir()
        .map(|h| h.join(".agentdesk").join("project_map.json"))
        .unwrap_or_default();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_project_map(map: &HashMap<String, String>) {
    if let Some(home) = dirs::home_dir() {
        let dir = home.join(".agentdesk");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("project_map.json");
        if let Ok(json) = serde_json::to_string_pretty(map) {
            let _ = fs::write(path, json);
        }
    }
}

/// Scan all configured directories to discover projects
pub fn scan_projects() -> Vec<Project> {
    let cfg = crate::services::config::load_config();

    let mut project_map = load_project_map();
    let mut projects: HashMap<PathBuf, Project> = HashMap::new();

    for scan_dir_str in &cfg.scan_dirs {
        let scan_dir = PathBuf::from(scan_dir_str);
        if !scan_dir.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&scan_dir) else { continue };

        for entry in entries.flatten() {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let dir_path = entry.path();

            if !dir_path.is_dir() {
                continue;
            }

            let jsonl_files: Vec<PathBuf> = fs::read_dir(&dir_path)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
                .map(|e| e.path())
                .collect();

            if jsonl_files.is_empty() {
                continue;
            }

            let project_root = if let Some(bound) = project_map.get(&dir_name) {
                let bound_path = PathBuf::from(bound);
                if bound_path.exists() {
                    let current_cwd = extract_cwd_from_sessions(&jsonl_files);
                    if let Some(ref cwd) = current_cwd {
                        let resolved = resolve_project_root(cwd);
                        if resolved != bound_path {
                            continue;
                        }
                    }
                    bound_path
                } else {
                    continue;
                }
            } else {
                let cwd = match extract_cwd_from_sessions(&jsonl_files) {
                    Some(c) => c,
                    None => continue,
                };
                let root = resolve_project_root(&cwd);
                if !root.exists() {
                    continue;
                }
                project_map.insert(dir_name.clone(), root.to_string_lossy().to_string());
                root
            };

            let last_active = get_last_modified(&jsonl_files);

            let project = projects.entry(project_root.clone()).or_insert_with(|| {
                let name = project_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                Project {
                    root: project_root.clone(),
                    name,
                    claude_dir_names: Vec::new(),
                    codex_session_files: Vec::new(),
                    agent_count: 0,
                    last_active: None,
                    session_count: 0,
                }
            });
            if !project.claude_dir_names.contains(&dir_name) {
                project.claude_dir_names.push(dir_name.clone());
            }
            project.session_count += jsonl_files.len();
            if let Some(ts) = last_active {
                if project.last_active.is_none() || project.last_active.unwrap() < ts {
                    project.last_active = Some(ts);
                }
            }
        }
    }

    save_project_map(&project_map);

    // Second pass: merge Codex rollout sessions into the same
    // project map. For each codex cwd we either (a) attach to an
    // existing project when it matches an already-discovered root or
    // (b) create a codex-only project entry.
    let codex_by_root = crate::services::codex_scanner::scan_codex_sessions();
    for (root, files) in codex_by_root {
        let project = projects.entry(root.clone()).or_insert_with(|| {
            let name = root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            Project {
                root: root.clone(),
                name,
                claude_dir_names: Vec::new(),
                codex_session_files: Vec::new(),
                agent_count: 0,
                last_active: None,
                session_count: 0,
            }
        });
        // Track the most recent mtime across codex files too so
        // codex-only projects get a meaningful "last active".
        let codex_last = files
            .iter()
            .filter_map(|f| fs::metadata(f).ok())
            .filter_map(|m| m.modified().ok())
            .max()
            .map(chrono::DateTime::<chrono::Utc>::from);
        if let Some(ts) = codex_last {
            if project.last_active.is_none_or(|cur| cur < ts) {
                project.last_active = Some(ts);
            }
        }
        project.session_count += files.len();
        project.codex_session_files.extend(files);
    }

    let mut result: Vec<Project> = projects.into_values().collect();
    result.sort_by(|a, b| b.last_active.cmp(&a.last_active));
    result
}

fn extract_cwd_from_sessions(jsonl_files: &[PathBuf]) -> Option<String> {
    for file in jsonl_files {
        if let Ok(content) = fs::read_to_string(file) {
            for line in content.lines() {
                if let Ok(record) = serde_json::from_str::<SessionRecord>(line) {
                    if record.record_type == "user" {
                        if let Some(cwd) = record.cwd {
                            return Some(cwd);
                        }
                    }
                }
            }
        }
    }
    None
}

fn resolve_project_root(cwd: &str) -> PathBuf {
    let path = PathBuf::from(cwd);
    if !path.exists() {
        return path;
    }
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&path)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
            fs::canonicalize(&root).unwrap_or_else(|_| PathBuf::from(root))
        }
        _ => fs::canonicalize(&path).unwrap_or(path),
    }
}

fn get_last_modified(files: &[PathBuf]) -> Option<chrono::DateTime<chrono::Utc>> {
    files
        .iter()
        .filter_map(|f| fs::metadata(f).ok())
        .filter_map(|m| m.modified().ok())
        .max()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t))
}
