//! Module 5.3 — Workflow UI (list-based editor).
//!
//! Per-project panel embedded in `Dashboard`. The design doc
//! originally called for a drag-and-drop canvas, but the P3 plan
//! says "list-based editor first, visual canvas later" — this file
//! implements the list variant end to end.
//!
//! Responsibilities:
//! * list every `WorkflowDef` in `{project}/.agentdesk/workflows/`
//! * create / edit / delete definitions via an inline editor
//! * start a new run and show per-node state badges
//! * render recent run history for the selected workflow
//! * poll the engine every few seconds so state transitions surface
//!   without a manual refresh
//!
//! The polling loop lives in a `use_hook` background task owned by
//! this component — it reloads runs on every tick and calls
//! `workflow_engine::tick` for the active run. When the section
//! unmounts (project switch) the task is dropped.

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::models::{
    workflow, AgentType, NodeState, PermissionMode, RunStatus, WorkflowDef, WorkflowEdge,
    WorkflowNode, WorkflowRun,
};
use crate::services::{workflow_engine, workflow_store};

#[component]
pub fn WorkflowsSection(project_root: PathBuf) -> Element {
    let mut workflows = use_signal(Vec::<WorkflowDef>::new);
    let mut editing = use_signal(|| None::<WorkflowDef>);
    let mut selected_id = use_signal(|| None::<String>);
    let mut runs = use_signal(Vec::<WorkflowRun>::new);
    let mut error_msg = use_signal(|| None::<String>);

    // Initial load + crash reconciliation. We only reconcile once
    // per mount because subsequent mounts would touch the same
    // persisted state and potentially re-fail already-failed nodes.
    {
        let project_root = project_root.clone();
        use_hook(move || {
            let _ = workflow_engine::reconcile_on_startup(&project_root);
            let loaded = workflow_store::load_defs(&project_root);
            if let Some(first) = loaded.first() {
                selected_id.set(Some(first.id.clone()));
            }
            workflows.set(loaded);
        });
    }

    // Reload runs whenever the selected workflow changes.
    {
        let project_root = project_root.clone();
        use_effect(move || {
            let sel = selected_id();
            if let Some(id) = sel {
                runs.set(workflow_store::load_runs(&project_root, &id));
            } else {
                runs.set(Vec::new());
            }
        });
    }

    // Background ticker — polls the engine every 3 seconds while the
    // section is mounted. We only advance runs whose overall status
    // is still `Running`; terminal runs are quiesced.
    {
        let project_root = project_root.clone();
        use_hook(move || {
            spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let Some(id) = selected_id() else { continue };
                    let defs = workflow_store::load_defs(&project_root);
                    let Some(def) = defs.into_iter().find(|d| d.id == id) else { continue };
                    let project_root_inner = project_root.clone();
                    let def_inner = def.clone();
                    let tick_result = tokio::task::spawn_blocking(move || {
                        let mut active: Vec<WorkflowRun> = workflow_store::load_runs(&project_root_inner, &def_inner.id)
                            .into_iter()
                            .filter(|r| matches!(r.status, RunStatus::Running))
                            .collect();
                        let mut any_change = false;
                        for run in active.iter_mut() {
                            if let Ok(tr) = workflow_engine::tick(&project_root_inner, &def_inner, run) {
                                if tr.any_change() {
                                    any_change = true;
                                }
                            }
                        }
                        any_change
                    }).await.unwrap_or(false);
                    if tick_result {
                        let reloaded = workflow_store::load_runs(&project_root, &id);
                        runs.set(reloaded);
                    }
                }
            });
        });
    }

    let reload_defs = {
        let project_root = project_root.clone();
        move || {
            workflows.set(workflow_store::load_defs(&project_root));
        }
    };

    rsx! {
        div { class: "section",
            div { class: "section-label", "编排工作流" }
            div { class: "grouped-card",
                div { class: "grouped-row",
                    div { class: "row-content",
                        div { class: "row-label-bold", "工作流定义" }
                        div { class: "row-sub", "将多个 Agent 按 DAG 串联，上游 exit 0 触发下游" }
                    }
                    button {
                        class: "btn-focus-terminal",
                        onclick: move |_| {
                            editing.set(Some(WorkflowDef::new(String::new())));
                            error_msg.set(None);
                        },
                        "＋ 新建"
                    }
                }
                {
                    let list = workflows();
                    if list.is_empty() {
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "row-sub", style: "color: #86868b;",
                                    "暂无工作流。点击「新建」创建第一个 DAG。"
                                }
                            }
                        }
                    } else {
                        rsx! {
                            {list.iter().map(|wf| {
                                let id_for_click = wf.id.clone();
                                let id_for_edit = wf.id.clone();
                                let id_for_delete = wf.id.clone();
                                let id_for_run = wf.id.clone();
                                let wf_clone = wf.clone();
                                let is_selected = selected_id().as_deref() == Some(wf.id.as_str());
                                let name = wf.name.clone();
                                let node_n = wf.nodes.len();
                                let edge_n = wf.edges.len();
                                let mut reload = reload_defs.clone();
                                let project_root_for_run = project_root.clone();
                                let project_root_for_delete = project_root.clone();
                                let row_cls = if is_selected { "grouped-row wf-row-active" } else { "grouped-row" };
                                rsx! {
                                    div { class: "{row_cls}",
                                        onclick: move |_| selected_id.set(Some(id_for_click.clone())),
                                        div { class: "row-content",
                                            div { class: "row-label-bold", "{name}" }
                                            div { class: "row-sub", "{node_n} 节点 · {edge_n} 边" }
                                        }
                                        button {
                                            class: "btn-focus-terminal",
                                            onclick: move |e: Event<MouseData>| {
                                                e.stop_propagation();
                                                match workflow_engine::start_run(
                                                    &project_root_for_run,
                                                    &wf_clone,
                                                ) {
                                                    Ok(_) => {
                                                        selected_id.set(Some(id_for_run.clone()));
                                                        runs.set(workflow_store::load_runs(&project_root_for_run, &id_for_run));
                                                    }
                                                    Err(err) => error_msg.set(Some(err)),
                                                }
                                            },
                                            "▶ 运行"
                                        }
                                        button {
                                            class: "btn-focus-terminal",
                                            onclick: move |e: Event<MouseData>| {
                                                e.stop_propagation();
                                                let defs = workflows();
                                                if let Some(found) = defs.iter().find(|d| d.id == id_for_edit).cloned() {
                                                    editing.set(Some(found));
                                                }
                                            },
                                            "编辑"
                                        }
                                        button {
                                            class: "btn-remove",
                                            onclick: move |e: Event<MouseData>| {
                                                e.stop_propagation();
                                                match workflow_store::delete_def(&project_root_for_delete, &id_for_delete) {
                                                    Ok(()) => {
                                                        reload();
                                                        selected_id.set(None);
                                                    }
                                                    Err(err) => error_msg.set(Some(err)),
                                                }
                                            },
                                            "删除"
                                        }
                                    }
                                }
                            })}
                        }
                    }
                }
                if let Some(err) = error_msg() {
                    div { class: "grouped-row",
                        div { class: "row-sub", style: "color: #ff3b30;", "{err}" }
                    }
                }
            }
        }

        // Recent runs for the selected workflow.
        {
            let list = runs();
            let sel = selected_id();
            let sel_def = sel.as_ref().and_then(|id| workflows().into_iter().find(|d| &d.id == id));
            rsx! {
                if let Some(def) = sel_def {
                    div { class: "section",
                        div { class: "section-label", "{def.name} · 最近运行" }
                        div { class: "grouped-card",
                            if list.is_empty() {
                                div { class: "grouped-row",
                                    div { class: "row-sub", style: "color: #86868b;", "尚未运行过。点击「运行」启动一次。" }
                                }
                            } else {
                                {list.iter().take(5).map(|run| render_run_row(&def, run))}
                            }
                        }
                    }
                }
            }
        }

        // Editor dialog.
        if let Some(current) = editing() {
            WorkflowEditor {
                project_root: project_root.clone(),
                initial: current,
                on_save: {
                    let project_root = project_root.clone();
                    let mut reload = reload_defs.clone();
                    move |saved: WorkflowDef| {
                        match workflow_store::save_def(&project_root, &saved) {
                            Ok(()) => {
                                editing.set(None);
                                reload();
                                selected_id.set(Some(saved.id.clone()));
                            }
                            Err(e) => error_msg.set(Some(e)),
                        }
                    }
                },
                on_cancel: move |_| editing.set(None),
            }
        }
    }
}

fn render_run_row(def: &WorkflowDef, run: &WorkflowRun) -> Element {
    let id_short: String = run.id.chars().rev().take(6).collect::<String>().chars().rev().collect();
    let started = run.started_at.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string();
    let status_label = match run.status {
        RunStatus::Running => "进行中",
        RunStatus::Completed => "已完成",
        RunStatus::Failed => "失败",
    };
    let status_cls = match run.status {
        RunStatus::Running => "status-tag busy",
        RunStatus::Completed => "status-tag idle",
        RunStatus::Failed => "status-tag busy",
    };
    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { style: "display: flex; align-items: center; gap: 6px;",
                    span { class: "row-label-bold", "#{id_short}" }
                    span { class: "{status_cls}", "{status_label}" }
                    span { class: "row-sub", "· {started}" }
                }
                div { class: "wf-node-badges",
                    {def.nodes.iter().map(|n| {
                        let nr = run.nodes.get(&n.id).cloned().unwrap_or_default();
                        let badge_cls = match nr.state {
                            NodeState::Pending => "wf-badge wf-badge-pending",
                            NodeState::TriggerRequested => "wf-badge wf-badge-pending",
                            NodeState::Launching => "wf-badge wf-badge-launching",
                            NodeState::Running => "wf-badge wf-badge-running",
                            NodeState::Completed => "wf-badge wf-badge-completed",
                            NodeState::Failed => "wf-badge wf-badge-failed",
                        };
                        let state_label = nr.state.label();
                        let node_name = n.name.clone();
                        let title = match &nr.failure_reason {
                            Some(r) => format!("{} · {}", state_label, r),
                            None => state_label.to_string(),
                        };
                        rsx! {
                            span { class: "{badge_cls}", title: "{title}", "{node_name}" }
                        }
                    })}
                }
            }
        }
    }
}

// ────────────────── Editor ──────────────────

#[derive(Props, Clone, PartialEq)]
struct WorkflowEditorProps {
    #[allow(dead_code)]
    project_root: PathBuf,
    initial: WorkflowDef,
    on_save: EventHandler<WorkflowDef>,
    on_cancel: EventHandler<()>,
}

#[component]
fn WorkflowEditor(props: WorkflowEditorProps) -> Element {
    let mut name = use_signal(|| props.initial.name.clone());
    let mut description = use_signal(|| props.initial.description.clone());
    let mut nodes = use_signal(|| props.initial.nodes.clone());
    let mut edges = use_signal(|| props.initial.edges.clone());
    let mut local_error = use_signal(|| None::<String>);

    let original_initial = props.initial.clone();

    let save_click = {
        let on_save = props.on_save;
        move |_| {
            let trimmed = name().trim().to_string();
            if trimmed.is_empty() {
                local_error.set(Some("请填写工作流名称".into()));
                return;
            }
            if nodes().is_empty() {
                local_error.set(Some("至少需要一个节点".into()));
                return;
            }
            // Validate every node has a name.
            if nodes().iter().any(|n| n.name.trim().is_empty()) {
                local_error.set(Some("每个节点都需要名称".into()));
                return;
            }
            let built = WorkflowDef {
                id: original_initial.id.clone(),
                name: trimmed,
                description: description(),
                nodes: nodes(),
                edges: edges(),
                created_at: original_initial.created_at,
                updated_at: chrono::Utc::now(),
            };
            if let Err(e) = built.topo_order() {
                local_error.set(Some(e.to_string()));
                return;
            }
            on_save.call(built);
        }
    };

    rsx! {
        div { class: "dialog-overlay",
            onclick: move |_| props.on_cancel.call(()),
            div { class: "dialog wf-editor",
                onclick: move |e| e.stop_propagation(),
                h2 { "编辑工作流" }

                div { class: "form-group",
                    label { "名称" }
                    input {
                        class: "form-select",
                        style: "width: 100%; padding: 6px 8px;",
                        value: "{name()}",
                        oninput: move |e| name.set(e.value()),
                    }
                }
                div { class: "form-group",
                    label { "描述（可选）" }
                    input {
                        class: "form-select",
                        style: "width: 100%; padding: 6px 8px;",
                        value: "{description()}",
                        oninput: move |e| description.set(e.value()),
                    }
                }

                // ── Nodes ──
                div { class: "wf-editor-section-header",
                    span { "节点" }
                    button {
                        class: "btn-focus-terminal",
                        onclick: move |_| {
                            let mut cur = nodes();
                            cur.push(WorkflowNode {
                                id: workflow::new_node_id(),
                                name: format!("步骤 {}", cur.len() + 1),
                                agent_type: AgentType::ClaudeCode,
                                permission_mode: PermissionMode::Default,
                                initial_prompt: None,
                                timeout_secs: None,
                                tags: vec![],
                            });
                            nodes.set(cur);
                        },
                        "＋ 节点"
                    }
                }
                {nodes().iter().enumerate().map(|(idx, n)| {
                    let n_clone = n.clone();
                    let id_for_name = n.id.clone();
                    let id_for_agent = n.id.clone();
                    let id_for_perm = n.id.clone();
                    let id_for_prompt = n.id.clone();
                    let id_for_delete = n.id.clone();
                    rsx! {
                        div { key: "{n.id}", class: "wf-node-card",
                            div { style: "display: flex; gap: 6px; align-items: center;",
                                span { class: "row-sub", "节点 {idx + 1}" }
                                input {
                                    class: "form-select",
                                    style: "flex: 1;",
                                    placeholder: "节点名称",
                                    value: "{n_clone.name}",
                                    oninput: move |e| {
                                        let mut cur = nodes();
                                        if let Some(item) = cur.iter_mut().find(|x| x.id == id_for_name) {
                                            item.name = e.value();
                                        }
                                        nodes.set(cur);
                                    },
                                }
                                button {
                                    class: "btn-remove",
                                    onclick: move |_| {
                                        let cur: Vec<WorkflowNode> = nodes()
                                            .into_iter()
                                            .filter(|x| x.id != id_for_delete)
                                            .collect();
                                        nodes.set(cur);
                                        // Also prune dangling edges
                                        let keep_ids: std::collections::HashSet<String> =
                                            nodes().iter().map(|n| n.id.clone()).collect();
                                        let pruned: Vec<WorkflowEdge> = edges()
                                            .into_iter()
                                            .filter(|e| keep_ids.contains(&e.from) && keep_ids.contains(&e.to))
                                            .collect();
                                        edges.set(pruned);
                                    },
                                    "删除"
                                }
                            }
                            div { style: "display: flex; gap: 6px; margin-top: 4px;",
                                select {
                                    class: "form-select",
                                    onchange: move |e| {
                                        let mut cur = nodes();
                                        if let Some(item) = cur.iter_mut().find(|x| x.id == id_for_agent) {
                                            item.agent_type = match e.value().as_str() {
                                                "codex" => AgentType::Codex,
                                                _ => AgentType::ClaudeCode,
                                            };
                                        }
                                        nodes.set(cur);
                                    },
                                    option { value: "claude", selected: matches!(n_clone.agent_type, AgentType::ClaudeCode), "Claude Code" }
                                    option { value: "codex", selected: matches!(n_clone.agent_type, AgentType::Codex), "Codex" }
                                }
                                select {
                                    class: "form-select",
                                    onchange: move |e| {
                                        let mut cur = nodes();
                                        if let Some(item) = cur.iter_mut().find(|x| x.id == id_for_perm) {
                                            item.permission_mode = match e.value().as_str() {
                                                "skip" => PermissionMode::DangerouslySkipPermissions,
                                                "plan" => PermissionMode::Plan,
                                                _ => PermissionMode::Default,
                                            };
                                        }
                                        nodes.set(cur);
                                    },
                                    option { value: "default", selected: matches!(n_clone.permission_mode, PermissionMode::Default), "默认" }
                                    option { value: "skip", selected: matches!(n_clone.permission_mode, PermissionMode::DangerouslySkipPermissions), "跳过权限" }
                                    option { value: "plan", selected: matches!(n_clone.permission_mode, PermissionMode::Plan), "计划模式" }
                                }
                            }
                            textarea {
                                class: "form-select",
                                style: "width: 100%; min-height: 50px; margin-top: 4px; font-family: inherit;",
                                placeholder: "初始 prompt（可选）",
                                value: "{n_clone.initial_prompt.clone().unwrap_or_default()}",
                                oninput: move |e| {
                                    let mut cur = nodes();
                                    if let Some(item) = cur.iter_mut().find(|x| x.id == id_for_prompt) {
                                        let v = e.value();
                                        item.initial_prompt = if v.trim().is_empty() { None } else { Some(v) };
                                    }
                                    nodes.set(cur);
                                },
                            }
                        }
                    }
                })}

                // ── Edges ──
                div { class: "wf-editor-section-header",
                    span { "依赖边（上游 → 下游）" }
                    button {
                        class: "btn-focus-terminal",
                        onclick: move |_| {
                            let ns = nodes();
                            if ns.len() < 2 {
                                local_error.set(Some("至少需要两个节点才能添加边".into()));
                                return;
                            }
                            let mut cur = edges();
                            cur.push(WorkflowEdge {
                                from: ns[0].id.clone(),
                                to: ns[1].id.clone(),
                            });
                            edges.set(cur);
                        },
                        "＋ 边"
                    }
                }
                {edges().iter().enumerate().map(|(idx, e)| {
                    let e_from = e.from.clone();
                    let e_to = e.to.clone();
                    let ns = nodes();
                    rsx! {
                        div { key: "{idx}", class: "wf-edge-row",
                            select {
                                class: "form-select",
                                onchange: move |ev| {
                                    let mut cur = edges();
                                    if let Some(item) = cur.get_mut(idx) {
                                        item.from = ev.value();
                                    }
                                    edges.set(cur);
                                },
                                for n in &ns {
                                    option { value: "{n.id}", selected: n.id == e_from, "{n.name}" }
                                }
                            }
                            span { class: "row-sub", "→" }
                            select {
                                class: "form-select",
                                onchange: move |ev| {
                                    let mut cur = edges();
                                    if let Some(item) = cur.get_mut(idx) {
                                        item.to = ev.value();
                                    }
                                    edges.set(cur);
                                },
                                for n in &ns {
                                    option { value: "{n.id}", selected: n.id == e_to, "{n.name}" }
                                }
                            }
                            button {
                                class: "btn-remove",
                                onclick: move |_| {
                                    let cur: Vec<WorkflowEdge> = edges()
                                        .into_iter()
                                        .enumerate()
                                        .filter_map(|(i, e)| if i == idx { None } else { Some(e) })
                                        .collect();
                                    edges.set(cur);
                                },
                                "删除"
                            }
                        }
                    }
                })}

                if let Some(err) = local_error() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-top: 10px;", "{err}" }
                }

                div { class: "dialog-actions",
                    button { class: "btn-ghost", onclick: move |_| props.on_cancel.call(()), "取消" }
                    button { class: "btn btn-primary", onclick: save_click, "保存" }
                }
            }
        }
    }
}
