//! Module 5.1 — Workflow data model.
//!
//! A **workflow** (`WorkflowDef`) is a DAG of Agent launches the user
//! wires together in the orchestration canvas. Nodes describe *what*
//! to launch (agent type, permission mode, initial prompt); edges
//! describe *ordering* (node B starts only after node A exits
//! successfully). Per the design doc (§5) we deliberately avoid
//! file-change or message-based triggers — a child launches strictly
//! when its upstream node reaches exit code 0.
//!
//! A **run** (`WorkflowRun`) is one execution of a workflow. Each run
//! carries a per-node state machine plus a launch token that survives
//! app restarts so `workflow_engine` can reconcile against the
//! process table after a crash.
//!
//! Both types are serialized verbatim to JSON under
//! `{project}/.agentdesk/workflows/`. The on-disk layout is:
//!
//! ```text
//! .agentdesk/workflows/
//! ├── defs/
//! │   └── {workflow_id}.json       # one WorkflowDef per file
//! └── runs/
//!     └── {workflow_id}/
//!         └── {run_id}.json        # one WorkflowRun per file
//! ```

use crate::models::{AgentType, PermissionMode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A reusable DAG definition. Nodes and edges are stored as plain
/// vecs; the engine builds a petgraph representation on demand.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkflowDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One Agent launch in a workflow. Kept intentionally close to
/// `AgentTemplate` — a template is a headless workflow of size 1.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkflowNode {
    pub id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub permission_mode: PermissionMode,
    /// Initial prompt delivered to the REPL after launch. Subject to
    /// the same "user must confirm first use" rule as templates.
    #[serde(default)]
    pub initial_prompt: Option<String>,
    /// Upper bound for time-in-running before the engine marks the
    /// node as `Failed`. `None` = wait forever.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Optional hint rendered on the canvas (e.g. "runs tests").
    /// Not consumed by the engine.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Directed edge `from → to`. We don't model edge kinds (success vs
/// failure) in P3 — the spec says "exit 0 triggers downstream" and
/// everything else blocks the run.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
}

/// Per-node state machine. Mirrors the state diagram in §5 of the
/// design doc precisely — renaming or collapsing states would break
/// the crash-recovery contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    /// Upstream dependencies not yet satisfied.
    Pending,
    /// Upstream just went to `Completed`; engine is about to launch
    /// but has not yet written the launch token.
    TriggerRequested,
    /// Launch token written; osascript has been asked to open a
    /// terminal but we have not yet observed a matching PID. Crash
    /// recovery must scan the process table for this token before
    /// deciding whether to retry.
    Launching,
    /// Matching PID observed. This is the first state in which the
    /// node is "really" running.
    Running,
    Completed,
    Failed,
}

impl NodeState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, NodeState::Completed | NodeState::Failed)
    }

    pub fn label(&self) -> &'static str {
        match self {
            NodeState::Pending => "待运行",
            NodeState::TriggerRequested => "触发中",
            NodeState::Launching => "启动中",
            NodeState::Running => "运行中",
            NodeState::Completed => "完成",
            NodeState::Failed => "失败",
        }
    }
}

/// One execution of a workflow. A new `WorkflowRun` is created on
/// user "运行" action; it then becomes the authoritative log of every
/// state transition and is rewritten atomically on each change.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    /// Per-node runtime state keyed by `WorkflowNode::id`.
    pub nodes: HashMap<String, NodeRun>,
    /// Terminal state for the whole run (Completed iff every node is
    /// Completed; Failed iff any node ended Failed).
    #[serde(default)]
    pub status: RunStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Running,
    Completed,
    Failed,
}

/// Runtime state of a single node within a run.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NodeRun {
    pub state: NodeState,
    /// Observed PID after the node reaches `Running`. Cleared when
    /// the node leaves `Running` so stale PIDs can't mislead
    /// subsequent reconciliation passes.
    #[serde(default)]
    pub pid: Option<u32>,
    /// Opaque token passed via `AGENTDESK_LAUNCH_TOKEN` so crash
    /// recovery can match surviving processes back to nodes. Written
    /// the moment we enter `Launching` and kept for the lifetime of
    /// the run — never reused.
    #[serde(default)]
    pub launch_token: Option<String>,
    /// Terminal identifier (e.g. iTerm2 session id) captured at
    /// launch. Best-effort hint for crash recovery; may be absent.
    #[serde(default)]
    pub terminal_id: Option<String>,
    /// When we left `Pending`. Used for relative display + timeout.
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    /// Exit code captured on process reap (if any). Missing on
    /// terminal states means the engine inferred the transition from
    /// a process disappearance rather than an observed exit.
    #[serde(default)]
    pub exit_code: Option<i32>,
    /// Human-readable failure reason for UI display.
    #[serde(default)]
    pub failure_reason: Option<String>,
}

impl Default for NodeRun {
    fn default() -> Self {
        Self {
            state: NodeState::Pending,
            pid: None,
            launch_token: None,
            terminal_id: None,
            started_at: None,
            finished_at: None,
            exit_code: None,
            failure_reason: None,
        }
    }
}

impl WorkflowDef {
    /// Create a new, empty workflow with a freshly generated id.
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: new_workflow_id(),
            name,
            description: String::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Return node ids in a topological order, or `Err` if the graph
    /// contains a cycle or references an unknown node. The engine
    /// uses this both at save time (to reject invalid graphs) and at
    /// run time (to decide which node is ready next).
    pub fn topo_order(&self) -> Result<Vec<String>, WorkflowValidationError> {
        let ids: std::collections::HashSet<&str> =
            self.nodes.iter().map(|n| n.id.as_str()).collect();
        for edge in &self.edges {
            if !ids.contains(edge.from.as_str()) {
                return Err(WorkflowValidationError::UnknownNode(edge.from.clone()));
            }
            if !ids.contains(edge.to.as_str()) {
                return Err(WorkflowValidationError::UnknownNode(edge.to.clone()));
            }
        }

        // Kahn's algorithm — cheap for the <100-node graphs we expect
        // and avoids pulling in petgraph just for ordering.
        let mut indeg: HashMap<&str, usize> = HashMap::new();
        for n in &self.nodes {
            indeg.insert(n.id.as_str(), 0);
        }
        for e in &self.edges {
            *indeg.entry(e.to.as_str()).or_insert(0) += 1;
        }
        let mut queue: std::collections::VecDeque<&str> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| *k)
            .collect();
        let mut out: Vec<String> = Vec::with_capacity(self.nodes.len());
        while let Some(id) = queue.pop_front() {
            out.push(id.to_string());
            for e in self.edges.iter().filter(|e| e.from == id) {
                let d = indeg.get_mut(e.to.as_str()).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.push_back(e.to.as_str());
                }
            }
        }
        if out.len() != self.nodes.len() {
            return Err(WorkflowValidationError::Cycle);
        }
        Ok(out)
    }

    /// Return the ids of every node whose upstream edges all end in a
    /// node that is `Completed`. A node is "ready" iff it is
    /// currently in `Pending` and meets this criterion.
    pub fn ready_nodes<'a>(&'a self, run: &'a WorkflowRun) -> Vec<&'a str> {
        self.nodes
            .iter()
            .filter(|n| {
                let cur = run
                    .nodes
                    .get(&n.id)
                    .map(|nr| &nr.state)
                    .unwrap_or(&NodeState::Pending);
                if !matches!(cur, NodeState::Pending) {
                    return false;
                }
                self.edges
                    .iter()
                    .filter(|e| e.to == n.id)
                    .all(|e| {
                        matches!(
                            run.nodes.get(&e.from).map(|nr| &nr.state),
                            Some(NodeState::Completed)
                        )
                    })
            })
            .map(|n| n.id.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowValidationError {
    UnknownNode(String),
    Cycle,
}

impl std::fmt::Display for WorkflowValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownNode(id) => write!(f, "边引用了不存在的节点: {}", id),
            Self::Cycle => write!(f, "工作流存在环路"),
        }
    }
}

impl WorkflowRun {
    /// Create a fresh run with every node initialised to `Pending`.
    pub fn new(workflow: &WorkflowDef) -> Self {
        let mut nodes = HashMap::with_capacity(workflow.nodes.len());
        for n in &workflow.nodes {
            nodes.insert(n.id.clone(), NodeRun::default());
        }
        Self {
            id: new_run_id(),
            workflow_id: workflow.id.clone(),
            started_at: Utc::now(),
            finished_at: None,
            nodes,
            status: RunStatus::Running,
        }
    }

    /// Recompute the run's overall `status` + `finished_at` based on
    /// per-node state. Idempotent — safe to call after every
    /// transition.
    pub fn refresh_status(&mut self) {
        let any_failed = self
            .nodes
            .values()
            .any(|n| matches!(n.state, NodeState::Failed));
        let all_done = self
            .nodes
            .values()
            .all(|n| matches!(n.state, NodeState::Completed | NodeState::Failed));
        if any_failed && all_done {
            self.status = RunStatus::Failed;
            self.finished_at.get_or_insert_with(Utc::now);
        } else if all_done {
            self.status = RunStatus::Completed;
            self.finished_at.get_or_insert_with(Utc::now);
        } else {
            self.status = RunStatus::Running;
            self.finished_at = None;
        }
    }
}

fn new_workflow_id() -> String {
    format!("wf_{}", local_unique_suffix())
}

fn new_run_id() -> String {
    format!("run_{}", local_unique_suffix())
}

pub fn new_node_id() -> String {
    format!("n_{}", local_unique_suffix())
}

/// Generate a launch token that survives process restarts. Used by
/// the engine when entering `Launching` state. Kept here so both the
/// model and the engine share one definition.
pub fn new_launch_token() -> String {
    format!("tok_{}", local_unique_suffix())
}

fn local_unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}_{:x}", ms, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_workflow() -> WorkflowDef {
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
            agent_type: AgentType::ClaudeCode,
            permission_mode: PermissionMode::Default,
            initial_prompt: None,
            timeout_secs: None,
            tags: vec![],
        });
        w.edges.push(WorkflowEdge { from: "a".into(), to: "b".into() });
        w
    }

    #[test]
    fn topo_order_is_valid() {
        let w = simple_workflow();
        let order = w.topo_order().unwrap();
        assert_eq!(order.len(), 2);
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        assert!(pos_a < pos_b);
    }

    #[test]
    fn topo_detects_cycle() {
        let mut w = simple_workflow();
        w.edges.push(WorkflowEdge { from: "b".into(), to: "a".into() });
        assert!(matches!(w.topo_order(), Err(WorkflowValidationError::Cycle)));
    }

    #[test]
    fn topo_rejects_unknown_edge() {
        let mut w = simple_workflow();
        w.edges.push(WorkflowEdge { from: "a".into(), to: "c".into() });
        assert!(matches!(
            w.topo_order(),
            Err(WorkflowValidationError::UnknownNode(_))
        ));
    }

    #[test]
    fn ready_nodes_initially_returns_sources() {
        let w = simple_workflow();
        let run = WorkflowRun::new(&w);
        let ready = w.ready_nodes(&run);
        assert_eq!(ready, vec!["a"]);
    }

    #[test]
    fn ready_nodes_advances_when_upstream_completes() {
        let w = simple_workflow();
        let mut run = WorkflowRun::new(&w);
        run.nodes.get_mut("a").unwrap().state = NodeState::Completed;
        let ready = w.ready_nodes(&run);
        assert_eq!(ready, vec!["b"]);
    }

    #[test]
    fn refresh_status_marks_completed() {
        let w = simple_workflow();
        let mut run = WorkflowRun::new(&w);
        for nr in run.nodes.values_mut() {
            nr.state = NodeState::Completed;
        }
        run.refresh_status();
        assert_eq!(run.status, RunStatus::Completed);
        assert!(run.finished_at.is_some());
    }

    #[test]
    fn refresh_status_marks_failed_when_any_failed() {
        let w = simple_workflow();
        let mut run = WorkflowRun::new(&w);
        run.nodes.get_mut("a").unwrap().state = NodeState::Completed;
        run.nodes.get_mut("b").unwrap().state = NodeState::Failed;
        run.refresh_status();
        assert_eq!(run.status, RunStatus::Failed);
    }

    #[test]
    fn serde_roundtrip_def() {
        let w = simple_workflow();
        let json = serde_json::to_string(&w).unwrap();
        let back: WorkflowDef = serde_json::from_str(&json).unwrap();
        assert_eq!(w, back);
    }

    #[test]
    fn serde_roundtrip_run() {
        let w = simple_workflow();
        let run = WorkflowRun::new(&w);
        let json = serde_json::to_string(&run).unwrap();
        let back: WorkflowRun = serde_json::from_str(&json).unwrap();
        assert_eq!(run, back);
    }
}
