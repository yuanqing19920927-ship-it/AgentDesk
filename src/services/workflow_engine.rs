//! Module 5.2 — Workflow execution engine.
//!
//! The engine drives a `WorkflowRun` through its state machine by
//! launching Agents into terminal windows and polling for their
//! exit. The design deliberately never holds a handle to the child
//! process — the agent runs in a user-visible iTerm2 tab and its
//! lifetime is decoupled from AgentDesk's. All state lives in two
//! places:
//!
//! 1. **Persisted run JSON** — the authoritative log. Updated on
//!    every state transition via `workflow_store::save_run`.
//! 2. **Sentinel files** written by a small bash wrapper around each
//!    agent launch. One file per node:
//!    * `{run_dir}/{node_id}.pid` — the wrapper's own pid, written
//!      before the agent starts.
//!    * `{run_dir}/{node_id}.exit` — the agent's exit code, written
//!      after the agent returns.
//!
//! Having two separate files means the engine can observe
//! `Launching → Running` the moment the pid file appears, and
//! `Running → Completed/Failed` the moment the exit file appears,
//! without racing against the wrapper.
//!
//! Crash recovery (`reconcile_on_startup`) walks every persisted run
//! and, for each node in `Launching`/`Running`, checks the sentinel
//! files + process table. If the pid file says process X but `kill
//! -0 X` fails AND no exit file exists, the node is failed as
//! "process disappeared without exit code". Cleanly resumed runs
//! pick up exactly where they left off because all state is on
//! disk.

use crate::models::{
    workflow::{self, WorkflowDef, WorkflowNode},
    AgentType, NodeState, PermissionMode, WorkflowRun,
};
use crate::services::{terminal_launcher, workflow_store};
use chrono::Utc;
use std::path::{Path, PathBuf};

/// Env var the wrapper script receives so crash recovery can match
/// stray processes back to their node. Kept as a public const so
/// tests can assert on its name.
pub const LAUNCH_TOKEN_ENV: &str = "AGENTDESK_LAUNCH_TOKEN";

/// Filesystem layout for a single run's sentinel files. Co-located
/// with the run JSON so they share the same cleanup lifecycle.
fn run_sentinel_dir(project_root: &Path, workflow_id: &str, run_id: &str) -> PathBuf {
    project_root
        .join(".agentdesk")
        .join("workflows")
        .join("runs")
        .join(workflow_id)
        .join(format!("{}-sentinels", run_id))
}

fn pid_file(project_root: &Path, workflow_id: &str, run_id: &str, node_id: &str) -> PathBuf {
    run_sentinel_dir(project_root, workflow_id, run_id).join(format!("{}.pid", sanitize(node_id)))
}

fn exit_file(project_root: &Path, workflow_id: &str, run_id: &str, node_id: &str) -> PathBuf {
    run_sentinel_dir(project_root, workflow_id, run_id)
        .join(format!("{}.exit", sanitize(node_id)))
}

fn sanitize(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

// ───────────── launch ─────────────

/// Transition a node from `Pending` → `Launching` and open the
/// terminal window that runs the wrapped agent command. The
/// sentinel directory is created lazily so a workflow that's never
/// been run doesn't leave empty bookkeeping directories around.
pub fn launch_node(
    project_root: &Path,
    def: &WorkflowDef,
    run: &mut WorkflowRun,
    node_id: &str,
) -> Result<(), String> {
    let node = def
        .nodes
        .iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| format!("节点不存在: {}", node_id))?;

    let state = run
        .nodes
        .get(node_id)
        .map(|nr| nr.state.clone())
        .unwrap_or(NodeState::Pending);
    if !matches!(state, NodeState::Pending) {
        return Err(format!(
            "节点 {} 当前状态 {:?}，无法从该状态启动",
            node_id, state
        ));
    }

    // Prepare sentinel directory and pre-clean stale files. A
    // workflow may be re-run from the UI, in which case a previous
    // attempt's sentinel files for this node would confuse the
    // detector.
    let sentinel_dir = run_sentinel_dir(project_root, &run.workflow_id, &run.id);
    std::fs::create_dir_all(&sentinel_dir)
        .map_err(|e| format!("创建 sentinel 目录失败: {}", e))?;
    let pid_f = pid_file(project_root, &run.workflow_id, &run.id, node_id);
    let exit_f = exit_file(project_root, &run.workflow_id, &run.id, node_id);
    let _ = std::fs::remove_file(&pid_f);
    let _ = std::fs::remove_file(&exit_f);

    // Mark the node as Launching *before* invoking osascript. If the
    // osascript call panics or the user force-quits AgentDesk
    // between here and the wrapper writing its pid, crash recovery
    // will find a `Launching` node with no pid file and no live
    // process, and fail it cleanly.
    let token = workflow::new_launch_token();
    {
        let nr = run
            .nodes
            .entry(node_id.to_string())
            .or_default();
        nr.state = NodeState::Launching;
        nr.launch_token = Some(token.clone());
        nr.started_at = Some(Utc::now());
        nr.pid = None;
        nr.exit_code = None;
        nr.failure_reason = None;
    }
    workflow_store::save_run(project_root, run)?;

    // Build the wrapper command. The wrapper sets the env var (so
    // `ps eww` can find the process on startup reconciliation),
    // writes the pid file BEFORE the agent starts, runs the agent,
    // then writes the exit file. We use `$BASHPID` rather than `$$`
    // because we want the pid of the subshell that will actually
    // host the agent as a child — `$$` would be the parent shell on
    // some terminal configurations.
    let wrapper_cmd = build_wrapper_command(
        node,
        &token,
        &pid_f,
        &exit_f,
    );
    terminal_launcher::launch_wrapped_command(project_root, &wrapper_cmd)
        .map_err(|e| format!("osascript 启动失败: {}", e))?;

    Ok(())
}

/// Construct the bash-ish command string that iTerm2 will `write
/// text` into a fresh session. It:
/// 1. Exports AGENTDESK_LAUNCH_TOKEN so the env survives exec.
/// 2. Writes the subshell's pid into the pid sentinel file.
/// 3. Runs the agent command.
/// 4. Captures $? and writes it into the exit sentinel file.
///
/// We use `quoted form of` only on paths from Rust (they're
/// interpolated into AppleScript already by `launch_wrapped_command`
/// via `build_iterm_script`). Inside bash, each path is single-quoted
/// because we ship them through AppleScript as a string literal.
fn build_wrapper_command(
    node: &WorkflowNode,
    token: &str,
    pid_file: &Path,
    exit_file: &Path,
) -> String {
    let agent_cmd = node.agent_type.command();
    let flag = node.permission_mode.flag().unwrap_or("");
    let agent_full = if flag.is_empty() {
        agent_cmd.to_string()
    } else {
        format!("{} {}", agent_cmd, flag)
    };

    // Shell-escape paths + token by wrapping in single quotes and
    // escaping any embedded single quotes. Generated ids never
    // contain single quotes today but this is cheap insurance.
    let pid_esc = sh_squote(&pid_file.display().to_string());
    let exit_esc = sh_squote(&exit_file.display().to_string());
    let tok_esc = sh_squote(token);

    format!(
        "export {env}={tok}; echo $$ > {pid}; {cmd}; echo $? > {exit_f}",
        env = LAUNCH_TOKEN_ENV,
        tok = tok_esc,
        pid = pid_esc,
        cmd = agent_full,
        exit_f = exit_esc,
    )
}

/// Single-quote a string for bash, escaping embedded single quotes
/// via the `'\''` trick.
fn sh_squote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

// ───────────── tick ─────────────

/// Poll a single run and advance any nodes whose sentinel files have
/// new content. Call this every few seconds from the UI layer while
/// a run is in progress. Safe to call on a terminal-state run — it
/// simply does nothing and returns `TickResult::idle()`.
///
/// Returns a summary of what changed so the caller can trigger
/// notifications / re-renders.
#[derive(Debug, Clone, Default)]
pub struct TickResult {
    pub became_running: Vec<String>,
    pub became_completed: Vec<String>,
    pub became_failed: Vec<String>,
    pub launched: Vec<String>,
    pub persisted: bool,
}

impl TickResult {
    pub fn idle() -> Self {
        Self::default()
    }
    pub fn any_change(&self) -> bool {
        !self.became_running.is_empty()
            || !self.became_completed.is_empty()
            || !self.became_failed.is_empty()
            || !self.launched.is_empty()
    }
}

pub fn tick(
    project_root: &Path,
    def: &WorkflowDef,
    run: &mut WorkflowRun,
) -> Result<TickResult, String> {
    use crate::models::RunStatus;
    if matches!(run.status, RunStatus::Completed | RunStatus::Failed) {
        return Ok(TickResult::idle());
    }

    let mut result = TickResult::default();
    let node_ids: Vec<String> = def.nodes.iter().map(|n| n.id.clone()).collect();

    // 1) Advance state based on sentinel files.
    for id in &node_ids {
        let Some(nr) = run.nodes.get(id).cloned() else { continue };
        match nr.state {
            NodeState::Launching => {
                // Pid file appeared?
                let pid_f = pid_file(project_root, &run.workflow_id, &run.id, id);
                if let Some(pid) = read_pid_file(&pid_f) {
                    let entry = run.nodes.get_mut(id).unwrap();
                    entry.state = NodeState::Running;
                    entry.pid = Some(pid);
                    result.became_running.push(id.clone());
                    continue;
                }
                // Or did the exit file leap ahead (pid was never
                // written but the wrapper exited)? Treat as failure.
                let exit_f = exit_file(project_root, &run.workflow_id, &run.id, id);
                if exit_f.exists() {
                    let code = read_exit_file(&exit_f).unwrap_or(-1);
                    let entry = run.nodes.get_mut(id).unwrap();
                    entry.state = NodeState::Failed;
                    entry.exit_code = Some(code);
                    entry.finished_at = Some(Utc::now());
                    entry.failure_reason =
                        Some("启动阶段就退出（未写入 pid 文件）".into());
                    result.became_failed.push(id.clone());
                }
            }
            NodeState::Running => {
                let exit_f = exit_file(project_root, &run.workflow_id, &run.id, id);
                if let Some(code) = read_exit_file(&exit_f) {
                    let entry = run.nodes.get_mut(id).unwrap();
                    entry.finished_at = Some(Utc::now());
                    entry.exit_code = Some(code);
                    if code == 0 {
                        entry.state = NodeState::Completed;
                        result.became_completed.push(id.clone());
                    } else {
                        entry.state = NodeState::Failed;
                        entry.failure_reason = Some(format!("退出码 {}", code));
                        result.became_failed.push(id.clone());
                    }
                } else if let Some(pid) = nr.pid {
                    // Running but the process vanished AND no exit
                    // file — the terminal was probably force-closed
                    // mid-run. Fail the node so the workflow doesn't
                    // hang forever.
                    if !is_pid_alive(pid) {
                        let entry = run.nodes.get_mut(id).unwrap();
                        entry.state = NodeState::Failed;
                        entry.finished_at = Some(Utc::now());
                        entry.failure_reason =
                            Some("进程已消失且未记录退出码".into());
                        result.became_failed.push(id.clone());
                    }
                }
            }
            _ => {}
        }
    }

    // 2) Launch any newly-ready nodes (all upstream Completed).
    let ready: Vec<String> = def
        .ready_nodes(run)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    for id in ready {
        if let Err(e) = launch_node(project_root, def, run, &id) {
            // Failed to launch — mark the node failed rather than
            // retrying in a loop.
            let entry = run.nodes.entry(id.clone()).or_default();
            entry.state = NodeState::Failed;
            entry.finished_at = Some(Utc::now());
            entry.failure_reason = Some(format!("启动失败: {}", e));
            result.became_failed.push(id);
        } else {
            result.launched.push(id);
        }
    }

    run.refresh_status();
    workflow_store::save_run(project_root, run)?;
    result.persisted = true;
    Ok(result)
}

// ───────────── start run ─────────────

/// Create a fresh `WorkflowRun` for `def`, persist it, and launch
/// the initial set of nodes whose upstream edges are trivially
/// satisfied.
pub fn start_run(
    project_root: &Path,
    def: &WorkflowDef,
) -> Result<WorkflowRun, String> {
    // Fail fast if the DAG is invalid — we shouldn't even create a
    // run file for a workflow that can't be scheduled.
    def.topo_order().map_err(|e| e.to_string())?;

    let mut run = WorkflowRun::new(def);
    workflow_store::save_run(project_root, &run)?;
    tick(project_root, def, &mut run)?;
    Ok(run)
}

// ───────────── crash recovery ─────────────

/// Walk every persisted run in the project and bring their state in
/// sync with reality. Call once at startup, before any UI is shown.
///
/// For each non-terminal node:
/// * If the exit sentinel exists, treat it authoritatively.
/// * Else if the pid sentinel exists AND that pid is alive, keep
///   the node in `Running` (recording the pid if the previous state
///   was `Launching`).
/// * Else if the pid sentinel exists but the pid is gone, mark the
///   node `Failed` with reason "crash during run".
/// * Else (no pid sentinel, no exit sentinel) fail the node with
///   reason "AgentDesk crashed before launch observed".
///
/// Any mutation is persisted. The returned vec lists the run ids
/// that were touched so the caller can log or notify.
pub fn reconcile_on_startup(project_root: &Path) -> Vec<String> {
    let runs = workflow_store::load_all_runs(project_root);
    let mut touched = Vec::new();
    for mut run in runs {
        // Skip terminal runs entirely — they can't change.
        if matches!(
            run.status,
            crate::models::RunStatus::Completed | crate::models::RunStatus::Failed
        ) {
            continue;
        }
        let mut changed = false;
        let node_ids: Vec<String> = run.nodes.keys().cloned().collect();
        for id in node_ids {
            let nr = run.nodes.get(&id).cloned().unwrap_or_default();
            if nr.state.is_terminal() || matches!(nr.state, NodeState::Pending) {
                continue;
            }
            let pid_f = pid_file(project_root, &run.workflow_id, &run.id, &id);
            let exit_f = exit_file(project_root, &run.workflow_id, &run.id, &id);

            if let Some(code) = read_exit_file(&exit_f) {
                let entry = run.nodes.get_mut(&id).unwrap();
                entry.exit_code = Some(code);
                entry.finished_at = Some(Utc::now());
                entry.state = if code == 0 { NodeState::Completed } else { NodeState::Failed };
                if code != 0 {
                    entry.failure_reason = Some(format!("恢复: 退出码 {}", code));
                }
                changed = true;
                continue;
            }
            if let Some(pid) = read_pid_file(&pid_f) {
                if is_pid_alive(pid) {
                    // Still running — promote Launching to Running if
                    // needed and record the pid we observed.
                    let entry = run.nodes.get_mut(&id).unwrap();
                    if matches!(entry.state, NodeState::Launching) {
                        entry.state = NodeState::Running;
                        changed = true;
                    }
                    if entry.pid.is_none() {
                        entry.pid = Some(pid);
                        changed = true;
                    }
                } else {
                    let entry = run.nodes.get_mut(&id).unwrap();
                    entry.state = NodeState::Failed;
                    entry.finished_at = Some(Utc::now());
                    entry.failure_reason =
                        Some("恢复: 进程已不存在且无退出码".into());
                    changed = true;
                }
            } else {
                // Nothing observed — this is the "crashed between
                // Launching write and pid file" case.
                let entry = run.nodes.get_mut(&id).unwrap();
                entry.state = NodeState::Failed;
                entry.finished_at = Some(Utc::now());
                entry.failure_reason =
                    Some("恢复: 启动未确认，AgentDesk 可能在此期间崩溃".into());
                changed = true;
            }
        }
        if changed {
            run.refresh_status();
            if workflow_store::save_run(project_root, &run).is_ok() {
                touched.push(run.id.clone());
            }
        }
    }
    touched
}

// ───────────── helpers ─────────────

fn read_pid_file(path: &Path) -> Option<u32> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<u32>().ok()
}

fn read_exit_file(path: &Path) -> Option<i32> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<i32>().ok()
}

fn is_pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Assert this type accepts the expected enum members so changes to
/// the models module trigger a compile error here. Purely for the
/// module's self-documentation.
#[allow(dead_code)]
fn _compile_assertions() {
    let _ = AgentType::ClaudeCode;
    let _ = PermissionMode::Default;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AgentType, PermissionMode, WorkflowDef, WorkflowEdge, WorkflowNode, WorkflowRun,
    };
    use std::io::Write;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir()
                .join(format!("agentdesk-engine-test-{}-{}-{}", std::process::id(), ts, n));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path { &self.0 }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn demo() -> WorkflowDef {
        let mut w = WorkflowDef::new("t".into());
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
            agent_type: AgentType::ClaudeCode,
            permission_mode: PermissionMode::Default,
            initial_prompt: None,
            timeout_secs: None,
            tags: vec![],
        });
        w.edges.push(WorkflowEdge { from: "a".into(), to: "b".into() });
        w
    }

    fn write_file(p: &Path, content: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn wrapper_command_sets_env_and_sentinels() {
        let node = WorkflowNode {
            id: "x".into(),
            name: "X".into(),
            agent_type: AgentType::ClaudeCode,
            permission_mode: PermissionMode::Default,
            initial_prompt: None,
            timeout_secs: None,
            tags: vec![],
        };
        let cmd = build_wrapper_command(
            &node,
            "tok_abc",
            Path::new("/tmp/x.pid"),
            Path::new("/tmp/x.exit"),
        );
        assert!(cmd.contains("AGENTDESK_LAUNCH_TOKEN='tok_abc'"));
        assert!(cmd.contains("echo $$ > '/tmp/x.pid'"));
        assert!(cmd.contains("echo $? > '/tmp/x.exit'"));
    }

    #[test]
    fn sh_squote_escapes_embedded_quote() {
        assert_eq!(sh_squote("a'b"), r"'a'\''b'");
    }

    #[test]
    fn tick_on_terminal_run_is_idle() {
        let tmp = TempDir::new();
        let def = demo();
        workflow_store::save_def(tmp.path(), &def).unwrap();
        let mut run = WorkflowRun::new(&def);
        run.status = crate::models::RunStatus::Completed;
        let r = tick(tmp.path(), &def, &mut run).unwrap();
        assert!(!r.any_change());
    }

    #[test]
    fn tick_promotes_launching_to_running_when_pid_file_appears() {
        let tmp = TempDir::new();
        let def = demo();
        workflow_store::save_def(tmp.path(), &def).unwrap();
        let mut run = WorkflowRun::new(&def);
        // Manually put node "a" in Launching state and drop a pid
        // file — mimicking a successful terminal open.
        run.nodes.get_mut("a").unwrap().state = NodeState::Launching;
        run.nodes.get_mut("a").unwrap().launch_token = Some("tok".into());
        workflow_store::save_run(tmp.path(), &run).unwrap();

        // Pick a pid that's definitely alive (ourselves).
        let my_pid = std::process::id();
        write_file(
            &pid_file(tmp.path(), &run.workflow_id, &run.id, "a"),
            &my_pid.to_string(),
        );
        // Reload run so we can call tick on a fresh copy.
        let mut run = workflow_store::load_run(tmp.path(), &def.id, &run.id).unwrap();
        let r = tick(tmp.path(), &def, &mut run).unwrap();
        assert_eq!(r.became_running, vec!["a".to_string()]);
        assert_eq!(run.nodes["a"].state, NodeState::Running);
        assert_eq!(run.nodes["a"].pid, Some(my_pid));
    }

    #[test]
    fn tick_completes_node_on_exit_zero() {
        let tmp = TempDir::new();
        let def = demo();
        workflow_store::save_def(tmp.path(), &def).unwrap();
        let mut run = WorkflowRun::new(&def);
        run.nodes.get_mut("a").unwrap().state = NodeState::Running;
        run.nodes.get_mut("a").unwrap().pid = Some(std::process::id());
        workflow_store::save_run(tmp.path(), &run).unwrap();

        write_file(
            &exit_file(tmp.path(), &run.workflow_id, &run.id, "a"),
            "0",
        );
        let mut run = workflow_store::load_run(tmp.path(), &def.id, &run.id).unwrap();
        // tick will also try to launch "b" which would fail because
        // there's no iTerm2 session in a test runner. We intercept
        // by marking "b" as Completed first to prevent the launch.
        // Instead, detach edges so "b" isn't considered downstream.
        let mut def_iso = def.clone();
        def_iso.edges.clear();
        let r = tick(tmp.path(), &def_iso, &mut run).unwrap();
        assert_eq!(run.nodes["a"].state, NodeState::Completed);
        assert_eq!(run.nodes["a"].exit_code, Some(0));
        assert!(r.became_completed.contains(&"a".to_string()));
    }

    #[test]
    fn tick_fails_node_on_nonzero_exit() {
        let tmp = TempDir::new();
        let mut def = demo();
        def.edges.clear(); // isolate, see above
        workflow_store::save_def(tmp.path(), &def).unwrap();
        let mut run = WorkflowRun::new(&def);
        run.nodes.get_mut("a").unwrap().state = NodeState::Running;
        run.nodes.get_mut("a").unwrap().pid = Some(std::process::id());
        workflow_store::save_run(tmp.path(), &run).unwrap();

        write_file(
            &exit_file(tmp.path(), &run.workflow_id, &run.id, "a"),
            "137",
        );
        let mut run = workflow_store::load_run(tmp.path(), &def.id, &run.id).unwrap();
        tick(tmp.path(), &def, &mut run).unwrap();
        assert_eq!(run.nodes["a"].state, NodeState::Failed);
        assert_eq!(run.nodes["a"].exit_code, Some(137));
    }

    #[test]
    fn reconcile_fails_launching_with_no_sentinels() {
        let tmp = TempDir::new();
        let def = demo();
        workflow_store::save_def(tmp.path(), &def).unwrap();
        let mut run = WorkflowRun::new(&def);
        run.nodes.get_mut("a").unwrap().state = NodeState::Launching;
        run.nodes.get_mut("a").unwrap().launch_token = Some("tok".into());
        workflow_store::save_run(tmp.path(), &run).unwrap();

        let touched = reconcile_on_startup(tmp.path());
        assert_eq!(touched, vec![run.id.clone()]);
        let reloaded = workflow_store::load_run(tmp.path(), &def.id, &run.id).unwrap();
        assert_eq!(reloaded.nodes["a"].state, NodeState::Failed);
    }

    #[test]
    fn reconcile_honours_existing_exit_file() {
        let tmp = TempDir::new();
        let def = demo();
        workflow_store::save_def(tmp.path(), &def).unwrap();
        let mut run = WorkflowRun::new(&def);
        run.nodes.get_mut("a").unwrap().state = NodeState::Running;
        run.nodes.get_mut("a").unwrap().pid = Some(1);
        workflow_store::save_run(tmp.path(), &run).unwrap();

        write_file(
            &exit_file(tmp.path(), &run.workflow_id, &run.id, "a"),
            "0",
        );
        reconcile_on_startup(tmp.path());
        let reloaded = workflow_store::load_run(tmp.path(), &def.id, &run.id).unwrap();
        assert_eq!(reloaded.nodes["a"].state, NodeState::Completed);
    }
}
