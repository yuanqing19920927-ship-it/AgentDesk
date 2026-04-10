//! Module 5.1 — Workflow persistence.
//!
//! Reads and writes `WorkflowDef` and `WorkflowRun` records under
//! `{project}/.agentdesk/workflows/`. Every write goes through the
//! tmp-file-and-rename pattern so a crash in the middle of a save
//! leaves the previous file intact — the engine reconciles against
//! the last successfully-written snapshot.
//!
//! Directory layout:
//!
//! ```text
//! .agentdesk/
//! └── workflows/
//!     ├── defs/
//!     │   └── {workflow_id}.json
//!     └── runs/
//!         └── {workflow_id}/
//!             └── {run_id}.json
//! ```
//!
//! Workflows and runs are keyed by the workflow id so we can list
//! every run of a specific workflow cheaply, which is the common
//! query from the orchestration UI.

use crate::models::{WorkflowDef, WorkflowRun};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Root directory `{project}/.agentdesk/workflows/`.
fn root_dir(project_root: &Path) -> PathBuf {
    project_root.join(".agentdesk").join("workflows")
}

fn defs_dir(project_root: &Path) -> PathBuf {
    root_dir(project_root).join("defs")
}

fn runs_dir_for(project_root: &Path, workflow_id: &str) -> PathBuf {
    root_dir(project_root)
        .join("runs")
        .join(sanitize_id(workflow_id))
}

/// Keep file names safe: workflow ids are generated internally, but
/// we still sanitise to defend against manual edits of the JSON
/// sneaking path separators into ids.
fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

fn def_path(project_root: &Path, workflow_id: &str) -> PathBuf {
    defs_dir(project_root).join(format!("{}.json", sanitize_id(workflow_id)))
}

fn run_path(project_root: &Path, workflow_id: &str, run_id: &str) -> PathBuf {
    runs_dir_for(project_root, workflow_id).join(format!("{}.json", sanitize_id(run_id)))
}

// ───────────── Definitions ─────────────

/// Load every workflow definition in the project.
///
/// Returns an empty vec if the directory doesn't exist yet — this is
/// the common case for a freshly-created project.
pub fn load_defs(project_root: &Path) -> Vec<WorkflowDef> {
    let dir = defs_dir(project_root);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<WorkflowDef> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| {
            let content = fs::read_to_string(e.path()).ok()?;
            serde_json::from_str::<WorkflowDef>(&content).ok()
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Load one workflow definition by id.
pub fn load_def(project_root: &Path, workflow_id: &str) -> Option<WorkflowDef> {
    let path = def_path(project_root, workflow_id);
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save a workflow definition atomically. Updates `updated_at`
/// before writing so the on-disk mtime matches the logical mtime.
pub fn save_def(project_root: &Path, def: &WorkflowDef) -> Result<(), String> {
    // Reject invalid graphs eagerly — otherwise we'd happily persist
    // a DAG that would blow up at run time.
    def.topo_order().map_err(|e| e.to_string())?;

    let dir = defs_dir(project_root);
    fs::create_dir_all(&dir).map_err(|e| format!("创建工作流目录失败: {}", e))?;

    let mut to_write = def.clone();
    to_write.updated_at = chrono::Utc::now();

    write_json_atomic(&def_path(project_root, &def.id), &to_write)
}

/// Delete a workflow definition + all of its recorded runs. Missing
/// files are treated as success.
pub fn delete_def(project_root: &Path, workflow_id: &str) -> Result<(), String> {
    let path = def_path(project_root, workflow_id);
    match fs::remove_file(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("删除工作流定义失败: {}", e)),
    }
    let runs = runs_dir_for(project_root, workflow_id);
    match fs::remove_dir_all(&runs) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("删除工作流运行记录失败: {}", e)),
    }
    Ok(())
}

// ───────────── Runs ─────────────

/// Load every run of a specific workflow, newest first.
pub fn load_runs(project_root: &Path, workflow_id: &str) -> Vec<WorkflowRun> {
    let dir = runs_dir_for(project_root, workflow_id);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<WorkflowRun> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| {
            let content = fs::read_to_string(e.path()).ok()?;
            serde_json::from_str::<WorkflowRun>(&content).ok()
        })
        .collect();
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    out
}

/// Load one run by id.
pub fn load_run(
    project_root: &Path,
    workflow_id: &str,
    run_id: &str,
) -> Option<WorkflowRun> {
    let path = run_path(project_root, workflow_id, run_id);
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save a run atomically. Called on every node state transition by
/// the engine, so this lives in the hot path — keep it allocation-
/// light and don't block on anything other than disk I/O.
pub fn save_run(project_root: &Path, run: &WorkflowRun) -> Result<(), String> {
    let dir = runs_dir_for(project_root, &run.workflow_id);
    fs::create_dir_all(&dir).map_err(|e| format!("创建运行记录目录失败: {}", e))?;
    write_json_atomic(
        &run_path(project_root, &run.workflow_id, &run.id),
        run,
    )
}

/// Enumerate every run across every workflow in the project. Used by
/// crash recovery on startup — we need to see all `Launching` and
/// `Running` states regardless of which workflow they belong to.
pub fn load_all_runs(project_root: &Path) -> Vec<WorkflowRun> {
    let root = root_dir(project_root).join("runs");
    let Ok(workflow_dirs) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut all = Vec::new();
    for entry in workflow_dirs.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(wid) = path.file_name().and_then(|n| n.to_str()) else { continue };
        all.extend(load_runs(project_root, wid));
    }
    all
}

// ───────────── Atomic write helper ─────────────

fn write_json_atomic<T: serde::Serialize>(
    final_path: &Path,
    value: &T,
) -> Result<(), String> {
    let parent = final_path
        .parent()
        .ok_or_else(|| "路径缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;

    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("序列化失败: {}", e))?;

    let tmp_path = final_path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("写入失败: {}", e))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp_path, final_path)
        .map_err(|e| format!("原子替换失败: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentType, NodeState, PermissionMode, WorkflowDef, WorkflowEdge, WorkflowNode, WorkflowRun};

    /// Minimal temp-dir helper. We don't want a full tempfile
    /// dependency just for tests — each test creates a uniquely-named
    /// directory under the system temp root and removes it on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let pid = std::process::id();
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir()
                .join(format!("agentdesk-wf-test-{}-{}-{}", pid, ts, n));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path { &self.0 }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn tempdir() -> TempDir { TempDir::new() }

    fn demo_def() -> WorkflowDef {
        let mut w = WorkflowDef::new("demo".into());
        w.nodes.push(WorkflowNode {
            id: "a".into(),
            name: "A".into(),
            agent_type: AgentType::ClaudeCode,
            permission_mode: PermissionMode::Default,
            initial_prompt: None,
            timeout_secs: None,
            tags: vec![],
        });
        w.nodes.push(WorkflowNode {
            id: "b".into(),
            name: "B".into(),
            agent_type: AgentType::Codex,
            permission_mode: PermissionMode::Default,
            initial_prompt: None,
            timeout_secs: None,
            tags: vec![],
        });
        w.edges.push(WorkflowEdge { from: "a".into(), to: "b".into() });
        w
    }

    #[test]
    fn save_and_load_def_roundtrip() {
        let tmp = tempdir();
        let def = demo_def();
        save_def(tmp.path(), &def).unwrap();
        let loaded = load_def(tmp.path(), &def.id).unwrap();
        assert_eq!(loaded.id, def.id);
        assert_eq!(loaded.nodes.len(), 2);
        assert_eq!(loaded.edges.len(), 1);
    }

    #[test]
    fn save_def_rejects_cyclic_graph() {
        let tmp = tempdir();
        let mut def = demo_def();
        def.edges.push(WorkflowEdge { from: "b".into(), to: "a".into() });
        assert!(save_def(tmp.path(), &def).is_err());
    }

    #[test]
    fn list_defs_sorts_by_name() {
        let tmp = tempdir();
        let mut d1 = demo_def();
        d1.name = "zebra".into();
        let mut d2 = demo_def();
        d2.name = "alpha".into();
        d2.id = "wf_other".into();
        save_def(tmp.path(), &d1).unwrap();
        save_def(tmp.path(), &d2).unwrap();
        let list = load_defs(tmp.path());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha");
    }

    #[test]
    fn delete_def_removes_runs_too() {
        let tmp = tempdir();
        let def = demo_def();
        save_def(tmp.path(), &def).unwrap();
        let run = WorkflowRun::new(&def);
        save_run(tmp.path(), &run).unwrap();
        assert_eq!(load_runs(tmp.path(), &def.id).len(), 1);

        delete_def(tmp.path(), &def.id).unwrap();
        assert!(load_def(tmp.path(), &def.id).is_none());
        assert_eq!(load_runs(tmp.path(), &def.id).len(), 0);
    }

    #[test]
    fn save_run_is_reloadable() {
        let tmp = tempdir();
        let def = demo_def();
        save_def(tmp.path(), &def).unwrap();
        let mut run = WorkflowRun::new(&def);
        run.nodes.get_mut("a").unwrap().state = NodeState::Running;
        save_run(tmp.path(), &run).unwrap();
        let back = load_run(tmp.path(), &def.id, &run.id).unwrap();
        assert_eq!(back.nodes["a"].state, NodeState::Running);
    }

    #[test]
    fn load_all_runs_spans_workflows() {
        let tmp = tempdir();
        let d1 = demo_def();
        let mut d2 = demo_def();
        d2.id = "wf_other".into();
        d2.name = "other".into();
        save_def(tmp.path(), &d1).unwrap();
        save_def(tmp.path(), &d2).unwrap();
        save_run(tmp.path(), &WorkflowRun::new(&d1)).unwrap();
        save_run(tmp.path(), &WorkflowRun::new(&d2)).unwrap();
        let all = load_all_runs(tmp.path());
        assert_eq!(all.len(), 2);
    }
}
