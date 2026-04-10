use dioxus::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use chrono::Local;
use crate::models::{Agent, AgentTemplate, AgentType, AuditSnapshot, BudgetLevel, BudgetSettings, BudgetStatus, ComboPreset, PermissionMode, ProjectCost, ProjectHealth, Project, SessionSummary};
use crate::services::{agent_detector, agent_names, audit_recorder, budget_manager, cost_tracker, health_monitor, log_streamer, preset_manager, template_manager};
use crate::services::log_streamer::{StreamItem, StreamKind};
use super::instruction_dialog::{InstructionDialog, InstructionTarget};
use super::memory_view::MemoryView;
use super::templates::TemplateEditor;
// use super::workflows_section::WorkflowsSection; // 模块 5 暂缓

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
    // Module 10 补完 — full-text search and live-tail toggle.
    let mut log_search = use_signal(String::new);
    let mut log_live = use_signal(|| false);
    // Start a single background loop that re-reads the expanded
    // session's JSONL every 2 seconds while live mode is on. The loop
    // lives for the lifetime of the Dashboard scope (cleaned up on
    // project switch via the `key` prop).
    {
        let claude_dirs = project.claude_dir_names.clone();
        use_hook(move || {
            spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if !log_live() {
                        continue;
                    }
                    let Some(sid) = expanded_sid() else {
                        continue;
                    };
                    let d = claude_dirs.clone();
                    let items = tokio::task::spawn_blocking(move || {
                        let h = dirs::home_dir().unwrap_or_default();
                        for dn in &d {
                            let cd = h.join(".claude").join("projects").join(dn);
                            let items = log_streamer::read_session_stream(&cd, &sid);
                            if !items.is_empty() {
                                return items;
                            }
                        }
                        Vec::new()
                    })
                    .await
                    .unwrap_or_default();
                    if !items.is_empty() {
                        expanded_stream.set(items);
                    }
                }
            });
        });
    }
    // Track which PID is pending kill confirmation
    let mut confirm_kill_pid = use_signal(|| None::<u32>);
    // Agent alias editing state
    let mut alias_map = use_signal(agent_names::load_all);
    let mut editing_alias_pid = use_signal(|| None::<u32>);
    let mut alias_edit_value = use_signal(String::new);
    // Module 12.2: target of the open instruction dialog (None = closed).
    let mut instruction_target = use_signal(|| None::<InstructionTarget>);
    let project_root_str = project.root.to_string_lossy().to_string();

    // Module 7: combo preset launch. Presets are loaded on mount so the
    // "启动组合" picker can open instantly. The launch report is held in a
    // signal so a simple inline toast can summarise success/failure until
    // the user dismisses it.
    let mut combo_presets = use_signal(preset_manager::load_all);
    let mut combo_picker_open = use_signal(|| false);
    let mut combo_report = use_signal(|| None::<preset_manager::LaunchReport>);

    // Module 7: "save as template" draft. Populated when the user clicks
    // "另存为模板" from an expanded session log — holds a pre-filled
    // `AgentTemplate` that gets passed to `TemplateEditor` so the user
    // can tweak the name / permission before persisting.
    let mut save_as_template_draft = use_signal(|| None::<AgentTemplate>);
    let mut save_as_template_status = use_signal(|| None::<String>);

    // Module 6: budget settings + editor dialog state. Loaded once on
    // mount. The editor dialog is a simple two-field form (current
    // project limit + warn threshold). The computed status feeds the
    // progress bar in the cost section and the top-of-page alert
    // banner for over-budget projects.
    let mut budget_settings = use_signal(budget_manager::load);
    let mut budget_editor_open = use_signal(|| false);

    // Cost rollup — computed in a background task so the dashboard
    // doesn't block on parsing every JSONL in the project.
    let mut cost = use_signal(|| None::<ProjectCost>);
    {
        let project_root = project.root.clone();
        let claude_dirs = project.claude_dir_names.clone();
        let codex_files = project.codex_session_files.clone();
        use_hook(move || {
            spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    cost_tracker::project_cost(&project_root, &claude_dirs, &codex_files)
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
    // Module 9 补完 — transient status for rollback / diff-export toasts.
    let mut audit_status = use_signal(|| None::<String>);
    let mut audit_busy = use_signal(|| false);
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
                    button {
                        class: "btn btn-ghost",
                        onclick: move |_| {
                            // Re-read from disk each time so presets saved
                            // in the template panel appear without a
                            // dashboard refresh.
                            combo_presets.set(preset_manager::load_all());
                            combo_picker_open.set(true);
                        },
                        "▾ 启动组合"
                    }
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
                            let cwd_for_instr = agent.cwd.clone();
                            let has_cwd = agent.cwd.is_some();
                            let label = agent.agent_type.label().to_string();
                            let label_for_instr = label.clone();
                            let pid = agent.pid;
                            let cpu = agent.cpu_percent;
                            let status_label = agent.status.label().to_string();
                            let is_busy = agent.status == crate::models::AgentStatus::Busy;
                            let dot_cls = if is_busy { "status-dot busy" } else { "status-dot idle" };
                            let tty = agent.tty.clone();
                            let tty_for_instr = tty.clone();
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
                                        if has_cwd && has_tty {
                                            button {
                                                class: "btn-focus-terminal",
                                                title: "快速指令 (⌘K 搜索或点击打开)",
                                                onclick: move |_| {
                                                    if let Some(cwd) = cwd_for_instr.clone() {
                                                        instruction_target.set(Some(InstructionTarget {
                                                            pid,
                                                            tty: tty_for_instr.clone(),
                                                            cwd,
                                                            label: label_for_instr.clone(),
                                                        }));
                                                    }
                                                },
                                                "⌘ 指令"
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

            // ── Workflow orchestration (module 5) ──
            // 模块 5 暂缓：UI 暂不挂载，后端代码保留但未被引用。
            // 恢复方式：取消此处 + app_shell/mod.rs 的 mod 声明 +
            // services/mod.rs 和 models/mod.rs 的 mod 声明上的注释。
            // WorkflowsSection {
            //     key: "wf-{project.root.display()}",
            //     project_root: project.root.clone(),
            // }

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

            // ── Budget banner (module 6) ──
            // Shown only when the current project is at or over its
            // configured budget; nothing is rendered when the budget is
            // unset or still green.
            {
                let settings = budget_settings();
                let current_cost = cost();
                let used = current_cost.as_ref().map(|c| c.cost_usd).unwrap_or(0.0);
                let project_root_str_banner = project.root.to_string_lossy().to_string();
                let status = budget_manager::project_status(&settings, &project_root_str_banner, used);
                let show = matches!(status.level, BudgetLevel::Warn | BudgetLevel::Exceeded);
                if show {
                    let cls = format!("budget-banner {}", status.level.css_class());
                    let pct = status.percent.unwrap_or(0.0);
                    let limit_str = status
                        .limit_usd
                        .map(cost_tracker::format_usd)
                        .unwrap_or_else(|| "—".to_string());
                    let used_str = cost_tracker::format_usd(status.used_usd);
                    let label = status.level.label();
                    rsx! {
                        div { class: "{cls}",
                            span { "⚠ {label} · 已用 {used_str} / {limit_str}（{pct:.0}%）" }
                            button {
                                class: "btn-ghost",
                                style: "font-size: 11px; padding: 2px 8px;",
                                onclick: move |_| budget_editor_open.set(true),
                                "调整预算"
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }

            // ── Cost & usage ──
            {
                let current = cost();
                let settings = budget_settings();
                let used = current.as_ref().map(|c| c.cost_usd).unwrap_or(0.0);
                let project_root_str_cost = project.root.to_string_lossy().to_string();
                let status = budget_manager::project_status(&settings, &project_root_str_cost, used);
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
                            // Budget row — always rendered so the user
                            // has a "设置预算" button even before any
                            // cap is configured.
                            {render_budget_row(&status, budget_editor_open)}
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
                            if let Some(msg) = audit_status() {
                                div { class: "grouped-row",
                                    div { class: "row-sub", style: "color: #1d6f42;", "{msg}" }
                                }
                            }
                            if snapshots.is_empty() {
                                div { class: "grouped-row",
                                    div { class: "row-label", style: "color: #86868b;", "暂无快照记录" }
                                }
                            } else {
                                {snapshots.iter().map(|snap| render_snapshot_row(
                                    snap,
                                    project.root.clone(),
                                    audit_snapshots,
                                    audit_status,
                                    audit_busy,
                                ))}
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
                                                input {
                                                    class: "log-search-input",
                                                    r#type: "search",
                                                    placeholder: "搜索日志...",
                                                    value: "{log_search}",
                                                    oninput: move |e| log_search.set(e.value()),
                                                }
                                                label { class: "log-filter-chip log-live-chip",
                                                    title: "每 2 秒重读会话文件",
                                                    input { r#type: "checkbox", checked: log_live(), oninput: move |e| log_live.set(e.value() == "true") }
                                                    span { "实时" }
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
                                                button {
                                                    class: "btn-ghost",
                                                    style: "font-size: 11px; padding: 2px 8px;",
                                                    onclick: move |_| {
                                                        // Grab the first user text message from the
                                                        // expanded stream as the seed initial_prompt.
                                                        // Tool results also carry role="user" so we
                                                        // filter to Text kind to avoid dumping a
                                                        // tool-output blob into the prompt field.
                                                        let seed = expanded_stream()
                                                            .iter()
                                                            .find(|i| i.role == "user" && i.kind == StreamKind::Text)
                                                            .map(|i| i.content.trim().to_string())
                                                            .filter(|s| !s.is_empty());
                                                        let mut draft = AgentTemplate::new(
                                                            String::new(),
                                                            AgentType::ClaudeCode,
                                                            PermissionMode::Default,
                                                        );
                                                        draft.initial_prompt = seed;
                                                        save_as_template_draft.set(Some(draft));
                                                        save_as_template_status.set(None);
                                                    },
                                                    "另存为模板"
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
                                                    let q = log_search().trim().to_lowercase();
                                                    let has_q = !q.is_empty();
                                                    let items: Vec<StreamItem> = expanded_stream().into_iter()
                                                        .filter(|item| match item.kind {
                                                            StreamKind::Text => st,
                                                            StreamKind::ToolUse => stu,
                                                            StreamKind::ToolResult => str_,
                                                            StreamKind::Thinking => sth,
                                                        })
                                                        .filter(|item| {
                                                            if !has_q { return true; }
                                                            if item.content.to_lowercase().contains(&q) { return true; }
                                                            if let Some(tn) = &item.tool_name {
                                                                if tn.to_lowercase().contains(&q) { return true; }
                                                            }
                                                            false
                                                        })
                                                        .collect();
                                                    let count = items.len();
                                                    let empty = items.is_empty();
                                                    rsx! {
                                                        if has_q {
                                                            div { class: "log-search-summary", "匹配 {count} 条" }
                                                        }
                                                        if empty {
                                                            p { style: "color: #86868b; padding: 12px 0; text-align: center;",
                                                                if has_q { "没有匹配的日志" } else { "根据过滤条件无可显示项" }
                                                            }
                                                        }
                                                        {items.into_iter().map(|item| render_stream_item(&item))}
                                                    }
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
        if let Some(target) = instruction_target() {
            InstructionDialog {
                target,
                on_close: move |_| instruction_target.set(None),
            }
        }

        // ── Module 6: budget editor dialog ──
        if budget_editor_open() {
            BudgetEditor {
                initial: budget_settings(),
                project_root: project.root.to_string_lossy().to_string(),
                on_save: move |new_settings: BudgetSettings| {
                    budget_settings.set(new_settings);
                    budget_editor_open.set(false);
                },
                on_cancel: move |_| budget_editor_open.set(false),
            }
        }

        // ── Module 7: save-session-as-template dialog ──
        // Reuses the TemplateEditor component from templates.rs so the
        // form, validation, and styling stay in one place.
        if let Some(draft) = save_as_template_draft() {
            TemplateEditor {
                initial: draft,
                on_save: move |saved: AgentTemplate| {
                    match template_manager::save(&saved) {
                        Ok(()) => {
                            save_as_template_draft.set(None);
                            save_as_template_status.set(Some(format!("已保存模板「{}」", saved.name)));
                        }
                        Err(e) => save_as_template_status.set(Some(format!("保存失败：{}", e))),
                    }
                },
                on_cancel: move |_| save_as_template_draft.set(None),
            }
        }

        if let Some(msg) = save_as_template_status() {
            div {
                style: "position: fixed; left: 50%; bottom: 24px; transform: translateX(-50%); \
                        z-index: 9998; background: #1d1d1f; color: #fff; \
                        padding: 8px 14px; border-radius: 20px; font-size: 12px; \
                        box-shadow: 0 4px 16px rgba(0,0,0,0.25); cursor: pointer;",
                onclick: move |_| save_as_template_status.set(None),
                "{msg}"
            }
        }

        // ── Module 7: combo preset picker ──
        // Opened from the "▾ 启动组合" button. Lists every preset saved in
        // `~/.agentdesk/presets/` and launches the chosen one into the
        // current project root. Launch is synchronous and blocks the UI
        // thread only long enough to spawn terminal windows — the actual
        // agents run inside those windows, not here.
        if combo_picker_open() {
            {
                let presets_now = combo_presets();
                let project_root_for_launch = project.root.clone();
                rsx! {
                    div { class: "dialog-overlay",
                        onclick: move |_| combo_picker_open.set(false),
                        div { class: "dialog",
                            style: "max-width: 480px;",
                            onclick: move |e| e.stop_propagation(),
                            h2 { "启动组合" }
                            div { class: "row-sub", style: "color: #86868b; margin-bottom: 8px;",
                                "选择一个组合预设，一次性启动多个 Agent 窗口到当前项目。"
                            }
                            if presets_now.is_empty() {
                                div { style: "padding: 20px; text-align: center; color: #86868b;",
                                    "暂无组合预设 — 先在「模板与组合」里创建"
                                }
                            } else {
                                div { class: "grouped-card",
                                    {presets_now.iter().map(|p| {
                                        let preset_clone = p.clone();
                                        let project_root_inner = project_root_for_launch.clone();
                                        let name = p.name.clone();
                                        let item_count = p.items.len();
                                        let desc = p.description.clone();
                                        let has_desc = !desc.trim().is_empty();
                                        rsx! {
                                            div { class: "grouped-row",
                                                div { class: "row-content",
                                                    div { class: "row-label-bold", "{name}" }
                                                    div { class: "row-sub",
                                                        span { "{item_count} 个模板" }
                                                        if has_desc {
                                                            " · "
                                                            span { "{desc}" }
                                                        }
                                                    }
                                                }
                                                button {
                                                    class: "btn btn-primary",
                                                    style: "font-size: 12px; padding: 4px 12px;",
                                                    onclick: move |_| {
                                                        let root = project_root_inner.clone();
                                                        let preset = preset_clone.clone();
                                                        spawn(async move {
                                                            // Block off-thread so spawning
                                                            // terminal windows via osascript
                                                            // does not freeze the renderer.
                                                            let report = tokio::task::spawn_blocking(move || {
                                                                preset_manager::launch_preset(&root, &preset)
                                                            })
                                                            .await
                                                            .ok();
                                                            if let Some(r) = report {
                                                                combo_report.set(Some(r));
                                                            }
                                                            combo_picker_open.set(false);
                                                        });
                                                    },
                                                    "启动"
                                                }
                                            }
                                        }
                                    })}
                                }
                            }
                            div { class: "dialog-actions",
                                button {
                                    class: "btn-ghost",
                                    onclick: move |_| combo_picker_open.set(false),
                                    "关闭"
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Module 7: launch report toast ──
        // Non-modal summary shown after a combo launch. Clicking anywhere
        // on the card dismisses it. We keep the full list inline rather
        // than auto-hiding so the user can actually read which items
        // failed — half-successful launches are the interesting case.
        if let Some(report) = combo_report() {
            {
                let launched = report.launched.clone();
                let failed = report.failed.clone();
                let missing = report.missing_templates.clone();
                let total = report.total_attempted();
                let ok_count = launched.len();
                rsx! {
                    div {
                        style: "position: fixed; right: 20px; bottom: 20px; z-index: 9998; \
                                background: #fff; border: 0.5px solid #d1d1d6; border-radius: 10px; \
                                box-shadow: 0 8px 24px rgba(0,0,0,0.15); padding: 12px 14px; \
                                max-width: 360px; font-size: 12px; cursor: pointer;",
                        onclick: move |_| combo_report.set(None),
                        div { style: "font-weight: 600; margin-bottom: 6px;",
                            "组合启动完成：{ok_count}/{total}"
                        }
                        if !launched.is_empty() {
                            div { style: "color: #1c5b17;",
                                "✓ 已启动：{launched.join(\", \")}"
                            }
                        }
                        if !failed.is_empty() {
                            div { style: "color: #8a0f05; margin-top: 4px;",
                                {failed.iter().map(|(label, err)| rsx! {
                                    div { "✗ {label}：{err}" }
                                })}
                            }
                        }
                        if !missing.is_empty() {
                            div { style: "color: #8a0f05; margin-top: 4px;",
                                "⚠ 模板缺失：{missing.join(\", \")}"
                            }
                        }
                        div { style: "color: #86868b; margin-top: 6px; font-size: 11px;",
                            "点击关闭"
                        }
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
    let full_content = item.content.clone();

    rsx! {
        div { class: "{bc}",
            div { class: "msg-header",
                span { class: "msg-role", "{role_label}" }
                span { class: "msg-kind-badge", "{kind_label}" }
                if has_tool { span { class: "msg-tool-name", "{tool}" } }
                if has_mt { span { class: "msg-time", "{td}" } }
                button {
                    class: "msg-copy-btn",
                    title: "复制内容到剪贴板",
                    onclick: move |_| {
                        // Write through stdin to avoid any shell-escaping
                        // issues with quotes / backticks in tool outputs.
                        use std::io::Write as _;
                        if let Ok(mut child) = std::process::Command::new("pbcopy")
                            .stdin(std::process::Stdio::piped())
                            .spawn()
                        {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(full_content.as_bytes());
                            }
                            let _ = child.wait();
                        }
                    },
                    "复制"
                }
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
/// timestamp, branch + short SHA, dirty file counts, and four actions:
/// 导出 diff / 回滚到此处 / 删除。
fn render_snapshot_row(
    snap: &AuditSnapshot,
    project_root: PathBuf,
    mut snapshots_signal: Signal<Vec<AuditSnapshot>>,
    mut status_signal: Signal<Option<String>>,
    mut busy_signal: Signal<bool>,
) -> Element {
    let id = snap.id.clone();
    let id_del = id.clone();
    let id_export = id.clone();
    let root_export = project_root.clone();
    let root_rollback = project_root.clone();
    let root_delete = project_root.clone();
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
    let has_sha = snap.head_sha.is_some();
    let snap_for_export = snap.clone();
    let snap_for_rollback = snap.clone();
    let busy = busy_signal();

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
            div { style: "display: flex; gap: 6px;",
                button {
                    class: "btn-ghost btn-xs",
                    disabled: busy,
                    title: "导出 git diff 补丁",
                    onclick: move |_| {
                        let root = root_export.clone();
                        let snap = snap_for_export.clone();
                        let default_name = format!("agentdesk-diff-{}.patch", id_export);
                        busy_signal.set(true);
                        status_signal.set(None);
                        spawn(async move {
                            let result = tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>, String> {
                                let Some(path) = audit_recorder::pick_diff_save_path(&default_name)? else {
                                    return Ok(None);
                                };
                                let text = audit_recorder::export_diff_text(&root, &snap)?;
                                audit_recorder::write_diff_file(&path, &text)?;
                                Ok(Some(path))
                            }).await.map_err(|e| e.to_string()).and_then(|r| r);
                            match result {
                                Ok(Some(p)) => status_signal.set(Some(format!("已导出 diff: {}", p.display()))),
                                Ok(None) => {}
                                Err(e) => status_signal.set(Some(format!("导出失败: {}", e))),
                            }
                            busy_signal.set(false);
                        });
                    },
                    "导出 diff"
                }
                if has_sha {
                    button {
                        class: "btn-ghost btn-xs",
                        disabled: busy,
                        title: "stash 当前改动并 git reset --hard 到此快照",
                        style: "color: #c10b00;",
                        onclick: move |_| {
                            let root = root_rollback.clone();
                            let snap = snap_for_rollback.clone();
                            busy_signal.set(true);
                            status_signal.set(None);
                            spawn(async move {
                                let sha = snap.head_sha.clone().unwrap_or_default();
                                let short = sha.chars().take(7).collect::<String>();
                                let prompt = format!(
                                    "确认要回滚到快照 {} ({})?\\n\\n当前未提交改动会自动 stash 保留，可稍后 git stash pop 恢复。",
                                    snap.id, short
                                );
                                let confirmed = tokio::task::spawn_blocking(move || {
                                    audit_recorder::confirm_dialog(&prompt)
                                }).await.unwrap_or(false);
                                if !confirmed {
                                    busy_signal.set(false);
                                    return;
                                }
                                let root_refresh = root.clone();
                                let result = tokio::task::spawn_blocking(move || {
                                    audit_recorder::rollback_to_snapshot(&root, &snap)
                                }).await.map_err(|e| e.to_string()).and_then(|r| r);
                                match result {
                                    Ok(msg) => status_signal.set(Some(msg)),
                                    Err(e) => status_signal.set(Some(format!("回滚失败: {}", e))),
                                }
                                // Refresh list — post-rollback git status will differ.
                                let list = tokio::task::spawn_blocking(move || {
                                    audit_recorder::list_snapshots(&root_refresh)
                                }).await.unwrap_or_default();
                                snapshots_signal.set(list);
                                busy_signal.set(false);
                            });
                        },
                        "回滚"
                    }
                }
                button {
                    class: "btn-kill-cancel",
                    title: "删除快照",
                    disabled: busy,
                    onclick: move |_| {
                        let root = root_delete.clone();
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

/// Render the budget row at the bottom of the cost card: progress bar +
/// used/limit label + level chip + edit button. Always renders — when
/// no limit is set, the bar is grey and the chip says "未设置".
fn render_budget_row(status: &BudgetStatus, mut editor_open: Signal<bool>) -> Element {
    let level_cls = status.level.css_class();
    let level_label = status.level.label();
    let fill_cls = format!("budget-bar-fill {}", level_cls);
    let chip_cls = format!("budget-chip {}", level_cls);
    // Clamp the fill to 100% so the bar never overflows the track —
    // users can still see they're "over" via the chip/banner.
    let fill_pct = status
        .percent
        .map(|p| p.min(100.0))
        .unwrap_or(0.0);
    let fill_style = format!("width: {:.1}%;", fill_pct);
    let used_str = cost_tracker::format_usd(status.used_usd);
    let limit_str = status
        .limit_usd
        .map(cost_tracker::format_usd)
        .unwrap_or_else(|| "未设预算".to_string());
    let pct_str = status
        .percent
        .map(|p| format!("{:.0}%", p))
        .unwrap_or_else(|| "—".to_string());

    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { class: "budget-row-header",
                    span { class: "row-label-bold", "预算" }
                    span { class: "{chip_cls}", "{level_label}" }
                }
                div { class: "row-sub",
                    "已用 {used_str} · 上限 {limit_str} · {pct_str}"
                }
                div { class: "budget-bar-wrap", style: "margin-top: 6px;",
                    div { class: "{fill_cls}", style: "{fill_style}" }
                }
            }
            button {
                class: "btn-ghost",
                style: "font-size: 12px; padding: 4px 10px;",
                onclick: move |_| editor_open.set(true),
                "设置"
            }
        }
    }
}

/// Inline editor dialog for per-project budget and warn threshold. The
/// global budget is intentionally *not* editable here — this sits
/// inside a per-project dashboard and the global cap belongs in a
/// dedicated settings panel. Deferring that keeps scope tight.
#[component]
fn BudgetEditor(
    initial: BudgetSettings,
    project_root: String,
    on_save: EventHandler<BudgetSettings>,
    on_cancel: EventHandler<()>,
) -> Element {
    let seed_limit = initial
        .project_limit(&project_root)
        .map(|v| format!("{}", v))
        .unwrap_or_default();
    let seed_warn = format!("{}", initial.warn_at_percent);

    let mut limit_input = use_signal(|| seed_limit);
    let mut warn_input = use_signal(|| seed_warn);
    let mut err = use_signal(|| None::<String>);

    let project_root_for_save = project_root.clone();

    rsx! {
        div { class: "dialog-overlay",
            onclick: move |_| on_cancel.call(()),
            div { class: "dialog",
                style: "max-width: 460px;",
                onclick: move |e| e.stop_propagation(),
                h2 { "预算与告警" }
                div { class: "row-sub", style: "color: #86868b; margin-bottom: 10px;",
                    "项目累计花费达到阈值时会触发告警。留空清除限额。"
                }

                div { class: "form-group",
                    label { "当前项目预算（USD）" }
                    input {
                        class: "form-select",
                        style: "width: 100%; padding: 6px 8px;",
                        value: "{limit_input}",
                        placeholder: "例如 20",
                        oninput: move |e| limit_input.set(e.value()),
                    }
                }

                div { class: "form-group",
                    label { "告警阈值（% of 上限，1-100）" }
                    input {
                        class: "form-select",
                        style: "width: 100%; padding: 6px 8px;",
                        value: "{warn_input}",
                        oninput: move |e| warn_input.set(e.value()),
                    }
                }

                if let Some(msg) = err() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-bottom: 10px;", "{msg}" }
                }

                div { class: "dialog-actions",
                    button { class: "btn-ghost", onclick: move |_| on_cancel.call(()), "取消" }
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            let limit_raw = limit_input();
                            let trimmed = limit_raw.trim();
                            let new_limit: Option<f64> = if trimmed.is_empty() {
                                None
                            } else {
                                match trimmed.parse::<f64>() {
                                    Ok(v) if v > 0.0 => Some(v),
                                    Ok(_) => {
                                        err.set(Some("预算必须大于 0".into()));
                                        return;
                                    }
                                    Err(_) => {
                                        err.set(Some("预算格式无效，请输入数字".into()));
                                        return;
                                    }
                                }
                            };

                            let warn_raw = warn_input();
                            let warn_pct: f64 = match warn_raw.trim().parse::<f64>() {
                                Ok(v) => v,
                                Err(_) => {
                                    err.set(Some("告警阈值格式无效".into()));
                                    return;
                                }
                            };
                            if !(1.0..=100.0).contains(&warn_pct) {
                                err.set(Some("告警阈值必须在 1 到 100 之间".into()));
                                return;
                            }

                            // Persist both changes atomically (two
                            // saves are fine — each write is atomic,
                            // and worst case a crash between them
                            // leaves the project limit updated but
                            // the warn threshold unchanged).
                            match budget_manager::set_project_limit(&project_root_for_save, new_limit) {
                                Ok(_) => {}
                                Err(e) => { err.set(Some(e)); return; }
                            }
                            match budget_manager::set_warn_percent(warn_pct) {
                                Ok(settings) => on_save.call(settings),
                                Err(e) => err.set(Some(e)),
                            }
                        },
                        "保存"
                    }
                }
            }
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
