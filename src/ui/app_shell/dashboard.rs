use dioxus::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use chrono::Local;
use crate::models::{Agent, Project, SessionMessage, SessionSummary};
use crate::services::{agent_detector, session_reader};

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
    let docs = scan_docs(&project.root);
    let summary = read_project_summary(&project.root);
    let has_summary = summary.is_some();
    let summary_text = summary.clone().unwrap_or_default();
    let sc = sessions.len();
    let tm: usize = sessions.iter().map(|s| s.message_count).sum();

    let mut expanded_sid = use_signal(|| None::<String>);
    let mut expanded_msgs = use_signal(Vec::<SessionMessage>::new);
    let mut loading = use_signal(|| false);

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
                            let tty = agent.tty.clone();
                            let has_tty = tty.is_some();
                            rsx! {
                                div { class: "grouped-row",
                                    div { style: "display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0;",
                                        div { class: "status-dot" }
                                        div { class: "row-content",
                                            div { class: "row-label-bold", "{label}" }
                                            div { class: "row-sub",
                                                "PID {pid}"
                                                if has_cwd { " · {cwd_str}" }
                                            }
                                        }
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
                                }
                            }
                        })}
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
                                                    expanded_sid.set(None); expanded_msgs.set(Vec::new());
                                                } else {
                                                    let sl = sid_click.clone(); let d = cdirs.clone();
                                                    expanded_sid.set(Some(sl.clone())); loading.set(true);
                                                    spawn(async move {
                                                        let m = tokio::task::spawn_blocking(move || {
                                                            let h = dirs::home_dir().unwrap_or_default();
                                                            for dn in &d {
                                                                let cd = h.join(".claude").join("projects").join(dn);
                                                                let ms = session_reader::read_session_messages(&cd, &sl);
                                                                if !ms.is_empty() { return ms; }
                                                            }
                                                            Vec::new()
                                                        }).await.unwrap_or_default();
                                                        expanded_msgs.set(m); loading.set(false);
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
                                            if loading() {
                                                p { style: "color: #86868b; padding: 12px 0; text-align: center;", "加载中..." }
                                            } else if expanded_msgs().is_empty() {
                                                p { style: "color: #86868b; padding: 12px 0; text-align: center;", "无法加载会话内容" }
                                            } else {
                                                {expanded_msgs().iter().map(|msg| {
                                                    let is_u = msg.role == "user";
                                                    let rl = if is_u { "用户" } else { "助手" };
                                                    let bc = if is_u { "msg-bubble msg-user" } else { "msg-bubble msg-assistant" };
                                                    let td = msg.timestamp.map(|t| t.with_timezone(&Local).format("%H:%M:%S").to_string()).unwrap_or_default();
                                                    let has_mt = msg.timestamp.is_some();
                                                    let cd = truncate_msg(&msg.content, 2000);
                                                    rsx! {
                                                        div { class: "{bc}",
                                                            div { class: "msg-header",
                                                                span { class: "msg-role", "{rl}" }
                                                                if has_mt { span { class: "msg-time", "{td}" } }
                                                            }
                                                            div { class: "msg-content", "{cd}" }
                                                        }
                                                    }
                                                })}
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
