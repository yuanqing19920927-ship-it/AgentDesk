use dioxus::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use chrono::Local;
use crate::models::{Agent, AuditSnapshot, ProjectCost, ProjectHealth, Project, SessionSummary};
use crate::services::{agent_detector, agent_names, audit_recorder, cost_tracker, health_monitor, log_streamer};
use crate::services::log_streamer::{StreamItem, StreamKind};
use super::memory_view::MemoryView;

fn scan_docs(root: &std::path::Path) -> Vec<PathBuf> {
    let mut docs = Vec::new();
    scan_docs_r(root, root, &mut docs, 0);
    docs.sort_by(|a, b| {
        let ad = a.strip_prefix(root).map(|p| p.components().count()).unwrap_or(99);
        let bd = b.strip_prefix(root).map(|p| p.components().count()).unwrap_or(99);
        ad.cmp(&bd).then_with(|| a.cmp(b))
    });
    docs
}
fn scan_docs_r(root: &std::path::Path, dir: &std::path::Path, docs: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            docs.push(path);
        } else if path.is_dir() {
            let n = entry.file_name().to_string_lossy().to_string();
            if n.starts_with('.') || matches!(n.as_str(), "node_modules"|"target"|"build"|"dist"|"vendor"|"Pods") { continue; }
            scan_docs_r(root, &path, docs, depth + 1);
        }
    }
}
fn open_file(p: &std::path::Path) { let _ = Command::new("open").arg(p).spawn(); }
fn read_project_summary(root: &std::path::Path) -> Option<String> {
    for name in &["README.md","readme.md","Readme.md"] {
        let p = root.join(name);
        if p.exists() {
            if let Ok(c) = std::fs::read_to_string(&p) {
                let s: String = c.lines()
                    .filter(|l| !l.starts_with('#') && !l.trim().is_empty() && !l.starts_with("![") && !l.starts_with("[!["))
                    .take(5).collect::<Vec<_>>().join("\n");
                if !s.is_empty() { return Some(s); }
            }
        }
    }
    None
}

#[component]
pub fn Dashboard(
    project: Project,
    agents: Vec<Agent>,
    sessions: Vec<SessionSummary>,
    on_new_agent: EventHandler<()>,
) -> Element {
    let la = project.last_active.map(|dt| dt.with_timezone(&Local).format("%m-%d %H:%M").to_string());
    let has_la = la.is_some();
    let la_display = la.unwrap_or_default();
    // Skip heavy scan for home directory to prevent freeze
    let is_home = dirs::home_dir().is_some_and(|h| h == project.root);
    let docs = if is_home { Vec::new() } else { scan_docs(&project.root) };
    let summary = if is_home { None } else { read_project_summary(&project.root) };
    let has_summary = summary.is_some();
    let summary_text = summary.clone().unwrap_or_default();
    let sc = sessions.len();
    let tm: usize = sessions.iter().map(|s| s.message_count).sum();

    let mut expanded_sid = use_signal(|| None::<String>);
    let mut expanded_stream = use_signal(Vec::<StreamItem>::new);
    let mut loading = use_signal(|| false);
    // Log viewer filter — which kinds of stream items to show.
    let mut show_text = use_signal(|| true);
    let mut show_thinking = use_signal(|| false);
    let mut show_tool_use = use_signal(|| true);
    let mut show_tool_result = use_signal(|| true);
    // Track which PID is pending kill confirmation
    let mut confirm_kill_pid = use_signal(|| None::<u32>);
    // Agent alias editing state
    let mut alias_map = use_signal(agent_names::load_all);
    let mut editing_alias_pid = use_signal(|| None::<u32>);
    let mut alias_edit_value = use_signal(String::new);
    let project_root_str = project.root.to_string_lossy().to_string();

    // Cost rollup — computed in a background task so the dashboard
    // doesn't block on parsing every JSONL in the project.
    let mut cost = use_signal(|| None::<ProjectCost>);
    {
        let project_root = project.root.clone();
        let claude_dirs = project.claude_dir_names.clone();
        use_hook(move || {
            spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    cost_tracker::project_cost(&project_root, &claude_dirs)
                })
                .await
                .ok();
                if let Some(c) = result {
                    cost.set(Some(c));
                }
            });
        });
    }

    // Audit snapshots — user-driven timeline. Loaded lazily on mount
    // and refreshed after each "记录快照" button click.
    let mut audit_snapshots = use_signal(Vec::<AuditSnapshot>::new);
    let mut audit_loading = use_signal(|| false);
    let mut audit_error = use_signal(|| None::<String>);
    {
        let project_root = project.root.clone();
        use_hook(move || {
            spawn(async move {
                let list = tokio::task::spawn_blocking(move || {
                    audit_recorder::list_snapshots(&project_root)
                })
                .await
                .unwrap_or_default();
                audit_snapshots.set(list);
            });
        });
    }

    // Health rollup — same lazy pattern. Depends on active_agents at
    // render time, so we snapshot it here (changing the count after
    // the initial compute isn't a big deal — the next project switch
    // will recompute).
    let mut health = use_signal(|| None::<ProjectHealth>);
    {
        let project_root = project.root.clone();
        let claude_dirs = project.claude_dir_names.clone();
        let active = project.agent_count;
        use_hook(move || {
            spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    health_monitor::compute(&project_root, &claude_dirs, active)
                })
                .await
                .ok();
                if let Some(h) = result {
                    health.set(Some(h));
                }
            });
        });
    }

    rsx! {
        div {
            // ── Header ──
            div { class: "page-header",
                div { class: "page-header-info",
                    h1 { "{project.name}" }
                    div { class: "path", "{project.root.display()}" }
                }
                div { class: "page-header-actions",
                    button { class: "btn btn-primary", onclick: move |_| on_new_agent.call(()), "＋ 新建 Agent" }
                }
            }

            // ── Health ──
            {
                let current = health();
                rsx! {
                    div { class: "section",
                        div { class: "section-label", "项目健康度" }
                        div { class: "grouped-card",
                            if let Some(h) = current {
                                {render_health_card(&h)}
                            } else {
                                div { class: "grouped-row",
                                    div { class: "row-sub", "评估中..." }
                                }
                            }
                        }
                    }
                }
            }

            // ── Overview ──
            div { class: "section",
                div { class: "section-label", "项目总览" }
                if has_summary {
                    div { class: "grouped-card", style: "margin-bottom: 10px;",
                        div { class: "grouped-row",
                            div { class: "summary-text", "{summary_text}" }
                        }
                    }
                }
                div { class: "stats-grid",
                    div { class: "stat-card",
                        div { class: "stat-value green", "{project.agent_count}" }
                        div { class: "stat-label", "运行中 Agent" }
                    }
                    div { class: "stat-card",
                        div { class: "stat-value blue", "{sc}" }
                        div { class: "stat-label", "会话总数" }
                    }
                    div { class: "stat-card",
                        div { class: "stat-value blue", "{tm}" }
                        div { class: "stat-label", "消息总数" }
                    }
                    if has_la {
                        div { class: "stat-card",
                            div { class: "stat-value orange", "{la_display}" }
                            div { class: "stat-label", "最近活跃" }
                        }
                    }
                }
            }

            // ── Agents ──
            div { class: "section",
                div { class: "section-label", "运行中的 Agent" }
                div { class: "grouped-card",
                    if agents.is_empty() {
                        div { class: "grouped-row",
                            div { class: "row-label", style: "color: #86868b;", "当前没有运行中的 Agent" }
                        }
                    } else {
                        {agents.iter().map(|agent| {
                            let cwd_str = agent.cwd.as_ref().map(|c| c.display().to_string()).unwrap_or_default();
                            let has_cwd = agent.cwd.is_some();
                            let label = agent.agent_type.label().to_string();
                            let pid = agent.pid;
                            let cpu = agent.cpu_percent;
                            let status_label = agent.status.label().to_string();
                            let is_busy = agent.status == crate::models::AgentStatus::Busy;
                            let dot_cls = if is_busy { "status-dot busy" } else { "status-dot idle" };
                            let tty = agent.tty.clone();
                            let has_tty = tty.is_some();
                            let is_sub = agent.is_subagent;
                            let parent_pid = agent.parent_pid.unwrap_or(0);
                            let row_cls = if is_sub { "grouped-row subagent-row" } else { "grouped-row" };
                            let alias_key = agent_names::agent_key(tty.as_deref(), pid);
                            let alias = alias_map().get(&project_root_str).and_then(|m| m.get(&alias_key)).cloned();
                            let has_alias = alias.is_some();
                            let alias_text = alias.clone().unwrap_or_default();
                            let is_editing_alias = editing_alias_pid() == Some(pid);
                            let pr_for_save = project.root.clone();
                            let pr_for_blur = project.root.clone();
                            let tty_for_save = tty.clone();
                            let tty_for_blur = tty.clone();
                            rsx! {
                                div { class: "{row_cls}",
                                    div { style: "display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0;",
                                        if is_sub {
                                            div { style: "width: 16px; flex-shrink: 0;" } // indent
                                        }
                                        div { class: "{dot_cls}" }
                                        div { class: "row-content",
                                            div { style: "display: flex; align-items: center; gap: 6px;",
                                                if is_editing_alias {
                                                    input {
                                                        class: "nickname-input",
                                                        value: "{alias_edit_value}",
                                                        autofocus: true,
                                                        placeholder: "输入备注名…",
                                                        onclick: move |e| e.stop_propagation(),
                                                        oninput: move |e| alias_edit_value.set(e.value()),
                                                        onkeydown: move |e| {
                                                            if e.key() == Key::Enter {
                                                                let _ = agent_names::set_alias(&pr_for_save, tty_for_save.as_deref(), pid, &alias_edit_value());
                                                                alias_map.set(agent_names::load_all());
                                                                editing_alias_pid.set(None);
                                                            } else if e.key() == Key::Escape {
                                                                editing_alias_pid.set(None);
                                                            }
                                                        },
                                                        onfocusout: move |_| {
                                                            let _ = agent_names::set_alias(&pr_for_blur, tty_for_blur.as_deref(), pid, &alias_edit_value());
                                                            alias_map.set(agent_names::load_all());
                                                            editing_alias_pid.set(None);
                                                        },
                                                    }
                                                } else if has_alias {
                                                    span { class: if is_sub { "row-label" } else { "row-label-bold" }, "{alias_text}" }
                                                    span { class: "nick-badge", "{label}" }
                                                } else {
                                                    span { class: if is_sub { "row-label" } else { "row-label-bold" },
                                                        if is_sub { "↳ 子 Agent" } else { "{label}" }
                                                    }
                                                }
                                                span { class: if is_busy { "status-tag busy" } else { "status-tag idle" },
                                                    "{status_label}"
                                                }
                                                if is_sub {
                                                    span { class: "sub-badge", "子进程 ← PID {parent_pid}" }
                                                }
                                            }
                                            div { class: "row-sub",
                                                "PID {pid} · CPU {cpu:.1}%"
                                                if has_cwd { " · {cwd_str}" }
                                            }
                                        }
                                    }
                                    div { class: "agent-actions",
                                        button {
                                            class: "btn-focus-terminal",
                                            title: "编辑备注名",
                                            onclick: move |_| {
                                                alias_edit_value.set(alias_text.clone());
                                                editing_alias_pid.set(Some(pid));
                                            },
                                            "✏️"
                                        }
                                        if has_tty {
                                            button {
                                                class: "btn-focus-terminal",
                                                onclick: move |_| {
                                                    if let Some(ref t) = tty {
                                                        let tc = t.clone();
                                                        spawn(async move { let _ = tokio::task::spawn_blocking(move || agent_detector::focus_agent_terminal(&tc)).await; });
                                                    }
                                                },
                                                "↗ 终端"
                                            }
                                        }
                                        if confirm_kill_pid() == Some(pid) {
                                            // Confirmation state
                                            button {
                                                class: "btn-kill confirm",
                                                onclick: move |_| {
                                                    let _ = std::process::Command::new("kill").arg(pid.to_string()).output();
                                                    confirm_kill_pid.set(None);
                                                },
                                                "确认终止"
                                            }
                                            button {
                                                class: "btn-kill-cancel",
                                                onclick: move |_| confirm_kill_pid.set(None),
                                                "取消"
                                            }
                                        } else {
                                            button {
                                                class: "btn-kill",
                                                onclick: move |_| confirm_kill_pid.set(Some(pid)),
                                                "终止"
                                            }
                                        }
                                    }
                                }
                            }
                        })}
                    }
                }
            }

            // ── Project memory ──
            // `key` is critical: it forces MemoryView to remount when
            // switching projects so its `use_signal` initial closures
            // (approved / report / entries) re-evaluate against the new
            // project root. Without it, a stale value from the first
            // visited project leaks onto every subsequently selected one.
            MemoryView {
                key: "{project.root.display()}",
                project: project.clone(),
            }

            // ── Cost & usage ──
            {
                let current = cost();
                rsx! {
                    div { class: "section",
                        div { class: "section-label", "费用与用量" }
                        div { class: "grouped-card",
                            if let Some(c) = current {
                                {render_cost_card(&c)}
                            } else {
                                div { class: "grouped-row",
                                    div { class: "row-sub", "统计中..." }
                                }
                            }
                        }
                    }
                }
            }

            // ── Audit timeline (Module 9) ──
            {
                let snapshots = audit_snapshots();
                let is_loading = audit_loading();
                let err = audit_error();
                let pr_click = project.root.clone();
                let pr_refresh = project.root.clone();
                rsx! {
                    div { class: "section",
                        div { class: "section-label", "变更审计" }
                        div { class: "grouped-card",
                            div { class: "grouped-row",
                                div { class: "row-content",
                                    div { class: "row-label-bold", "Git 快照时间线" }
                                    div { class: "row-sub", "记录当前 git 状态，便于日后对比 Agent 带来的变更" }
                                }
                                button {
                                    class: "btn btn-ghost",
                                    disabled: is_loading,
                                    onclick: move |_| {
                                        let root = pr_click.clone();
                                        let root_reload = pr_refresh.clone();
                                        audit_loading.set(true);
                                        audit_error.set(None);
                                        spawn(async move {
                                            let res = tokio::task::spawn_blocking(move || {
                                                audit_recorder::take_snapshot(&root, None)
                                            }).await.map_err(|e| e.to_string()).and_then(|r| r);
                                            match res {
                                                Ok(_) => {
                                                    let list = tokio::task::spawn_blocking(move || {
                                                        audit_recorder::list_snapshots(&root_reload)
                                                    }).await.unwrap_or_default();
                                                    audit_snapshots.set(list);
                                                }
                                                Err(e) => audit_error.set(Some(e)),
                                            }
                                            audit_loading.set(false);
                                        });
                                    },
                                    if is_loading { "记录中..." } else { "📸 记录快照" }
                                }
                            }
                            if let Some(e) = err {
                                div { class: "grouped-row",
                                    div { class: "row-sub", style: "color: #d93025;", "{e}" }
                                }
                            }
                            if snapshots.is_empty() {
                                div { class: "grouped-row",
                                    div { class: "row-label", style: "color: #86868b;", "暂无快照记录" }
                                }
                            } else {
                                {snapshots.iter().map(|snap| render_snapshot_row(snap, project.root.clone(), audit_snapshots))}
                            }
                        }
                    }
                }
            }

            // ── Docs ──
            div { class: "section",
                div { class: "section-label", "项目文档" }
                div { class: "grouped-card",
                    if docs.is_empty() {
                        div { class: "grouped-row",
                            div { class: "row-label", style: "color: #86868b;", "未发现 Markdown 文档" }
                        }
                    } else {
                        {docs.iter().map(|path| {
                            let dn = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            let rp = path.strip_prefix(&project.root).map(|p| p.display().to_string()).unwrap_or_else(|_| path.display().to_string());
                            let pc = path.clone();
                            rsx! {
                                div { class: "grouped-row grouped-row-clickable", onclick: move |_| open_file(&pc),
                                    div { style: "display: flex; align-items: center; gap: 8px;",
                                        div { class: "doc-icon", "📄" }
                                        div {
                                            div { class: "doc-name", "{dn}" }
                                            div { class: "doc-path", "{rp}" }
                                        }
                                    }
                                    div { class: "row-value", "›" }
                                }
                            }
                        })}
                    }
                }
            }

            // ── Session history ──
            div { class: "section",
                div { class: "section-label", "历史会话 ({sc})" }
                div { class: "grouped-card",
                    if sessions.is_empty() {
                        div { class: "grouped-row",
                            div { class: "row-label", style: "color: #86868b;", "暂无会话记录" }
                        }
                    } else {
                        {sessions.iter().map(|session| {
                            let sid = session.session_id.clone();
                            let is_exp = expanded_sid() == Some(sid.clone());
                            let ts = session.started_at.map(|t| t.with_timezone(&Local).format("%m-%d %H:%M").to_string()).unwrap_or_default();
                            let has_ts = session.started_at.is_some();
                            let mc = session.message_count;
                            let br = session.git_branch.clone().unwrap_or_default();
                            let has_br = session.git_branch.is_some();
                            let pv = session.preview.clone().unwrap_or_default();
                            let has_pv = session.preview.is_some();
                            let arrow = if is_exp { "▼" } else { "▶" };
                            let row_cls = if is_exp { "grouped-row session-expanded" } else { "grouped-row" };
                            let sid_click = sid.clone();
                            let cdirs = project.claude_dir_names.clone();

                            rsx! {
                                div {
                                    div { class: "{row_cls}",
                                        div {
                                            class: "session-header-row",
                                            onclick: move |_| {
                                                if expanded_sid() == Some(sid_click.clone()) {
                                                    expanded_sid.set(None); expanded_stream.set(Vec::new());
                                                } else {
                                                    let sl = sid_click.clone(); let d = cdirs.clone();
                                                    expanded_sid.set(Some(sl.clone())); loading.set(true);
                                                    spawn(async move {
                                                        let m = tokio::task::spawn_blocking(move || {
                                                            let h = dirs::home_dir().unwrap_or_default();
                                                            for dn in &d {
                                                                let cd = h.join(".claude").join("projects").join(dn);
                                                                let items = log_streamer::read_session_stream(&cd, &sl);
                                                                if !items.is_empty() { return items; }
                                                            }
                                                            Vec::new()
                                                        }).await.unwrap_or_default();
                                                        expanded_stream.set(m); loading.set(false);
                                                    });
                                                }
                                            },
                                            span { class: "session-arrow", "{arrow}" }
                                            if has_ts { span { class: "session-time", "{ts}" } }
                                            if has_br { span { class: "session-branch", "{br}" } }
                                            span { class: "session-msgs", "{mc} 条消息" }
                                        }
                                    }
                                    if !is_exp {
                                        if has_pv {
                                            div { style: "padding: 0 16px 8px;",
                                                div { class: "session-preview-text", "{pv}" }
                                            }
                                        }
                                    }
                                    if is_exp {
                                        div { class: "session-detail", style: "padding: 0 16px 12px;",
                                            // Filter toolbar
                                            div { class: "log-filter-bar",
                                                label { class: "log-filter-chip",
                                                    input { r#type: "checkbox", checked: show_text(), oninput: move |e| show_text.set(e.value() == "true") }
                                                    span { "消息" }
                                                }
                                                label { class: "log-filter-chip",
                                                    input { r#type: "checkbox", checked: show_tool_use(), oninput: move |e| show_tool_use.set(e.value() == "true") }
                                                    span { "工具调用" }
                                                }
                                                label { class: "log-filter-chip",
                                                    input { r#type: "checkbox", checked: show_tool_result(), oninput: move |e| show_tool_result.set(e.value() == "true") }
                                                    span { "工具结果" }
                                                }
                                                label { class: "log-filter-chip",
                                                    input { r#type: "checkbox", checked: show_thinking(), oninput: move |e| show_thinking.set(e.value() == "true") }
                                                    span { "思考" }
                                                }
                                                button {
                                                    class: "btn-ghost",
                                                    style: "font-size: 11px; padding: 2px 8px; margin-left: auto;",
                                                    onclick: move |_| {
                                                        let md = log_streamer::export_as_markdown(&expanded_stream());
                                                        if let Some(home) = dirs::home_dir() {
                                                            let p = home.join("Desktop").join(format!("agentdesk_session_{}.md",
                                                                chrono::Local::now().format("%Y%m%d_%H%M%S")));
                                                            if std::fs::write(&p, md).is_ok() {
                                                                let _ = std::process::Command::new("open").arg("-R").arg(&p).spawn();
                                                            }
                                                        }
                                                    },
                                                    "导出 Markdown"
                                                }
                                            }
                                            if loading() {
                                                p { style: "color: #86868b; padding: 12px 0; text-align: center;", "加载中..." }
                                            } else if expanded_stream().is_empty() {
                                                p { style: "color: #86868b; padding: 12px 0; text-align: center;", "无法加载会话内容" }
                                            } else {
                                                {
                                                    let st = show_text();
                                                    let stu = show_tool_use();
                                                    let str_ = show_tool_result();
                                                    let sth = show_thinking();
                                                    expanded_stream().into_iter()
                                                        .filter(move |item| match item.kind {
                                                            StreamKind::Text => st,
                                                            StreamKind::ToolUse => stu,
                                                            StreamKind::ToolResult => str_,
                                                            StreamKind::Thinking => sth,
                                                        })
                                                        .map(|item| render_stream_item(&item))
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        })}
                    }
                }
            }
        }
    }
}

fn truncate_msg(s: &str, max: usize) -> String {
    let t: String = s.chars().take(max).collect();
    if t.len() < s.len() { format!("{}...\n\n[内容过长，已截断]", t) } else { t }
}

/// Render a single stream item from the session log viewer.
fn render_stream_item(item: &StreamItem) -> Element {
    let is_u = item.role == "user";
    let (role_label, bubble_base) = if is_u {
        ("用户", "msg-bubble msg-user")
    } else {
        ("助手", "msg-bubble msg-assistant")
    };
    let kind_cls = match item.kind {
        StreamKind::Text => "msg-kind-text",
        StreamKind::Thinking => "msg-kind-thinking",
        StreamKind::ToolUse => "msg-kind-tool-use",
        StreamKind::ToolResult => "msg-kind-tool-result",
    };
    let bc = format!("{} {}", bubble_base, kind_cls);
    let td = item.timestamp.map(|t| t.with_timezone(&Local).format("%H:%M:%S").to_string()).unwrap_or_default();
    let has_mt = item.timestamp.is_some();
    let kind_label = item.kind.label();
    let tool = item.tool_name.clone().unwrap_or_default();
    let has_tool = item.tool_name.is_some();
    let cd = truncate_msg(&item.content, 2000);
    let is_code = matches!(item.kind, StreamKind::ToolUse | StreamKind::ToolResult);

    rsx! {
        div { class: "{bc}",
            div { class: "msg-header",
                span { class: "msg-role", "{role_label}" }
                span { class: "msg-kind-badge", "{kind_label}" }
                if has_tool { span { class: "msg-tool-name", "{tool}" } }
                if has_mt { span { class: "msg-time", "{td}" } }
            }
            if is_code {
                pre { class: "msg-content msg-code", "{cd}" }
            } else {
                div { class: "msg-content", "{cd}" }
            }
        }
    }
}

/// Render the "cost & usage" card body: total USD, token counts,
/// and per-model breakdown. Called from Dashboard when the async
/// cost computation has landed.
fn render_cost_card(c: &ProjectCost) -> Element {
    let total_cost = cost_tracker::format_usd(c.cost_usd);
    let total_tokens = cost_tracker::format_tokens(c.tokens.input + c.tokens.output + c.tokens.cache_write + c.tokens.cache_read);
    let input_t = cost_tracker::format_tokens(c.tokens.input);
    let output_t = cost_tracker::format_tokens(c.tokens.output);
    let cache_w = cost_tracker::format_tokens(c.tokens.cache_write);
    let cache_r = cost_tracker::format_tokens(c.tokens.cache_read);
    let session_count = c.session_count;
    let msg_count = c.message_count;
    let has_data = msg_count > 0;

    if !has_data {
        return rsx! {
            div { class: "grouped-row",
                div { class: "row-sub", "暂无使用记录" }
            }
        };
    }

    let models = c.models.clone();
    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { class: "row-label-bold", "累计费用 {total_cost}" }
                div { class: "row-sub",
                    "{session_count} 个会话 · {msg_count} 次助手调用 · {total_tokens} tokens"
                }
            }
        }
        div { class: "grouped-row",
            div { class: "row-content",
                div { class: "row-sub",
                    "输入 {input_t} · 输出 {output_t} · 缓存写入 {cache_w} · 缓存读取 {cache_r}"
                }
            }
        }
        {models.iter().map(|m| {
            let name = shorten_model_name(&m.model);
            let cost = cost_tracker::format_usd(m.cost_usd);
            let tokens = cost_tracker::format_tokens(m.tokens.input + m.tokens.output + m.tokens.cache_write + m.tokens.cache_read);
            let mc = m.message_count;
            rsx! {
                div { class: "grouped-row",
                    div { class: "row-content",
                        div { style: "display: flex; align-items: center; gap: 6px;",
                            span { class: "row-label", "{name}" }
                            span { class: "nick-badge", "{mc} 次" }
                        }
                        div { class: "row-sub", "{tokens} tokens" }
                    }
                    div { class: "row-value", "{cost}" }
                }
            }
        })}
    }
}

/// Render a single snapshot row in the audit timeline. Includes the
/// timestamp, branch + short SHA, dirty file counts, and a delete
/// button that re-fetches the list on success.
fn render_snapshot_row(
    snap: &AuditSnapshot,
    project_root: PathBuf,
    mut snapshots_signal: Signal<Vec<AuditSnapshot>>,
) -> Element {
    let id = snap.id.clone();
    let id_del = id.clone();
    let ts = snap.timestamp.with_timezone(&Local).format("%m-%d %H:%M").to_string();
    let branch = snap.branch.clone().unwrap_or_else(|| "—".to_string());
    let short_sha = snap.short_sha();
    let modified = snap.modified.len();
    let added = snap.added.len();
    let deleted = snap.deleted.len();
    let untracked = snap.untracked.len();
    let dirty = snap.dirty_count();
    let has_label = snap.label.is_some();
    let label_text = snap.label.clone().unwrap_or_default();

    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { style: "display: flex; align-items: center; gap: 6px;",
                    span { class: "row-label-bold", "{ts}" }
                    span { class: "nick-badge", "{branch}" }
                    span { class: "nick-badge", style: "font-family: ui-monospace, monospace;", "{short_sha}" }
                    if has_label {
                        span { class: "row-sub", "{label_text}" }
                    }
                }
                div { class: "row-sub",
                    if dirty == 0 {
                        "工作区干净"
                    } else {
                        "修改 {modified} · 新增 {added} · 删除 {deleted} · 未跟踪 {untracked}"
                    }
                }
            }
            button {
                class: "btn-kill-cancel",
                title: "删除快照",
                onclick: move |_| {
                    let root = project_root.clone();
                    let sid = id_del.clone();
                    spawn(async move {
                        let _ = tokio::task::spawn_blocking({
                            let root = root.clone();
                            let sid = sid.clone();
                            move || audit_recorder::delete_snapshot(&root, &sid)
                        }).await;
                        let list = tokio::task::spawn_blocking(move || {
                            audit_recorder::list_snapshots(&root)
                        }).await.unwrap_or_default();
                        snapshots_signal.set(list);
                    });
                },
                "删除"
            }
        }
    }
}

/// Render the "project health" card body: overall status dot, the
/// supporting metrics (git/sessions/memory/agents), and any hints
/// explaining why a non-green status was given.
fn render_health_card(h: &ProjectHealth) -> Element {
    let status_cls = h.overall.css_class();
    let status_label = h.overall.label();
    let dot_cls = format!("health-dot {}", status_cls);
    let chip_cls = format!("health-chip {}", status_cls);

    let commits_7d = h.commits_7d;
    let commits_30d = h.commits_30d;
    let last_commit = match h.last_commit_age_days {
        Some(0) => "今天".to_string(),
        Some(n) => format!("{} 天前", n),
        None => "—".to_string(),
    };
    let sessions_7d = h.sessions_7d;
    let memory_entries = h.memory_entries;
    let memory_status = if h.memory_enabled {
        format!("已启用 · {} 条记忆", memory_entries)
    } else {
        "未启用".to_string()
    };
    let active_agents = h.active_agents;
    let hints = h.hints.clone();
    let has_hints = !hints.is_empty();

    rsx! {
        div { class: "grouped-row",
            div { style: "display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0;",
                div { class: "{dot_cls}" }
                div { class: "row-content",
                    div { class: "row-label-bold", "{status_label}" }
                    div { class: "row-sub",
                        "近 7 天 {commits_7d} 次提交 · 近 30 天 {commits_30d} 次 · 最近提交 {last_commit}"
                    }
                }
                span { class: "{chip_cls}", "{status_label}" }
            }
        }
        div { class: "grouped-row",
            div { class: "row-content",
                div { class: "row-sub",
                    "会话（7d）{sessions_7d} · 记忆：{memory_status} · 运行 Agent {active_agents}"
                }
            }
        }
        if has_hints {
            {hints.into_iter().map(|hint| rsx! {
                div { class: "grouped-row",
                    div { class: "row-content",
                        div { class: "row-sub health-hint", "· {hint}" }
                    }
                }
            })}
        }
    }
}

/// Strip the "claude-" prefix and trailing numeric suffixes to produce
/// a compact model label like "opus-4-6" → "Opus 4.6".
fn shorten_model_name(model: &str) -> String {
    let m = model.strip_prefix("claude-").unwrap_or(model);
    let parts: Vec<&str> = m.split('-').collect();
    if parts.is_empty() {
        return model.to_string();
    }
    let family = match parts[0] {
        "opus" => "Opus",
        "sonnet" => "Sonnet",
        "haiku" => "Haiku",
        other => return other.to_string(),
    };
    // Re-join remaining version segments with dots, stripping any
    // date segments that look like "20250101".
    let version: Vec<String> = parts[1..]
        .iter()
        .filter(|p| !p.chars().all(|c| c.is_ascii_digit()) || p.len() < 5)
        .map(|s| s.to_string())
        .collect();
    if version.is_empty() {
        family.to_string()
    } else {
        format!("{} {}", family, version.join("."))
    }
}
