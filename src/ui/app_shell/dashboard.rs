use dioxus::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use crate::models::{Agent, Project, SessionMessage, SessionSummary};
use crate::services::session_reader;

/// Recursively scan project for .md files
fn scan_docs(root: &std::path::Path) -> Vec<PathBuf> {
    let mut docs = Vec::new();
    scan_docs_recursive(root, root, &mut docs, 0);
    docs.sort_by(|a, b| {
        let a_depth = a.strip_prefix(root).map(|p| p.components().count()).unwrap_or(99);
        let b_depth = b.strip_prefix(root).map(|p| p.components().count()).unwrap_or(99);
        a_depth.cmp(&b_depth).then_with(|| a.cmp(b))
    });
    docs
}

fn scan_docs_recursive(root: &std::path::Path, dir: &std::path::Path, docs: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 { return; }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            docs.push(path);
        } else if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || matches!(name.as_str(), "node_modules" | "target" | "build" | "dist" | "vendor" | "Pods") {
                continue;
            }
            scan_docs_recursive(root, &path, docs, depth + 1);
        }
    }
}

fn open_file(path: &std::path::Path) {
    let _ = Command::new("open").arg(path).spawn();
}

/// Read README.md or first .md file as project summary
fn read_project_summary(root: &std::path::Path) -> Option<String> {
    let candidates = ["README.md", "readme.md", "Readme.md", "README.MD"];
    for name in &candidates {
        let path = root.join(name);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                // Take first meaningful paragraph (skip title lines)
                let summary: String = content.lines()
                    .filter(|l| !l.starts_with('#') && !l.trim().is_empty() && !l.starts_with("![") && !l.starts_with("[!["))
                    .take(5)
                    .collect::<Vec<_>>()
                    .join("\n");
                if !summary.is_empty() {
                    return Some(summary);
                }
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
    let last_active_str = project.last_active.map(|dt| dt.format("%m-%d %H:%M").to_string());
    let has_last_active = last_active_str.is_some();
    let last_active_display = last_active_str.unwrap_or_default();

    let doc_files = use_hook({
        let root = project.root.clone();
        move || scan_docs(&root)
    });

    let project_summary = use_hook({
        let root = project.root.clone();
        move || read_project_summary(&root)
    });
    let has_summary = project_summary.is_some();
    let summary_text = project_summary.clone().unwrap_or_default();

    let session_count = sessions.len();
    let total_messages: usize = sessions.iter().map(|s| s.message_count).sum();

    // Track which session is expanded
    let mut expanded_session = use_signal(|| None::<String>);
    let mut expanded_messages = use_signal(Vec::<SessionMessage>::new);
    let mut loading_detail = use_signal(|| false);

    rsx! {
        div {
            // ── Header ──
            div { class: "page-header",
                div { class: "page-header-info",
                    h1 { "{project.name}" }
                    div { class: "path", "{project.root.display()}" }
                }
                button { class: "btn btn-primary", onclick: move |_| on_new_agent.call(()),
                    "＋ 新建 Agent"
                }
            }

            // ── Project Overview with Summary ──
            div { class: "section",
                div { class: "section-label", "项目总览" }

                // Summary from README
                if has_summary {
                    div { class: "card summary-card",
                        div { class: "summary-text", "{summary_text}" }
                    }
                }

                div { class: "stats-grid",
                    div { class: "stat-card",
                        div { class: "stat-value green", "{project.agent_count}" }
                        div { class: "stat-label", "运行中 Agent" }
                    }
                    div { class: "stat-card",
                        div { class: "stat-value blue", "{session_count}" }
                        div { class: "stat-label", "会话总数" }
                    }
                    div { class: "stat-card",
                        div { class: "stat-value blue", "{total_messages}" }
                        div { class: "stat-label", "消息总数" }
                    }
                    if has_last_active {
                        div { class: "stat-card",
                            div { class: "stat-value orange", "{last_active_display}" }
                            div { class: "stat-label", "最近活跃" }
                        }
                    }
                }
            }

            // ── Running agents ──
            div { class: "section",
                div { class: "section-label", "运行中的 Agent ({agents.len()})" }
                if agents.is_empty() {
                    div { class: "card",
                        p { style: "color: #86868b; text-align: center; padding: 6px; font-size: 12px;",
                            "当前没有运行中的 Agent"
                        }
                    }
                } else {
                    {agents.iter().map(|agent| {
                        let cwd_str = agent.cwd.as_ref().map(|c| c.display().to_string()).unwrap_or_default();
                        let has_cwd = agent.cwd.is_some();
                        let label = agent.agent_type.label().to_string();
                        let pid = agent.pid;
                        rsx! {
                            div { class: "card agent-card",
                                div { class: "agent-status-dot" }
                                div { class: "agent-card-body",
                                    div { class: "agent-card-title", "{label}" }
                                    div { class: "agent-card-sub",
                                        "PID {pid}"
                                        if has_cwd {
                                            " · {cwd_str}"
                                        }
                                    }
                                }
                            }
                        }
                    })}
                }
            }

            // ── Project docs ──
            div { class: "section",
                div { class: "section-label", "项目文档 ({doc_files.len()})" }
                if doc_files.is_empty() {
                    div { class: "card",
                        p { style: "color: #86868b; text-align: center; padding: 6px; font-size: 12px;",
                            "未发现 Markdown 文档"
                        }
                    }
                } else {
                    {doc_files.iter().map(|path| {
                        let display_name = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let rel_path = path.strip_prefix(&project.root)
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|_| path.display().to_string());
                        let path_clone = path.clone();
                        rsx! {
                            div {
                                class: "card doc-item",
                                onclick: move |_| open_file(&path_clone),
                                div { class: "doc-icon", "📄" }
                                div { class: "doc-info",
                                    div { class: "doc-name", "{display_name}" }
                                    div { class: "doc-path", "{rel_path}" }
                                }
                            }
                        }
                    })}
                }
            }

            // ── Session history (at bottom, expandable) ──
            div { class: "section",
                div { class: "section-label", "历史会话 ({session_count})" }
                if sessions.is_empty() {
                    div { class: "card",
                        p { style: "color: #86868b; text-align: center; padding: 6px; font-size: 12px;",
                            "暂无会话记录"
                        }
                    }
                } else {
                    {sessions.iter().map(|session| {
                        let sid = session.session_id.clone();
                        let is_expanded = expanded_session() == Some(sid.clone());
                        let ts_str = session.started_at
                            .map(|ts| ts.format("%m-%d %H:%M").to_string())
                            .unwrap_or_default();
                        let has_ts = session.started_at.is_some();
                        let msg_count = session.message_count;
                        let branch_str = session.git_branch.clone().unwrap_or_default();
                        let has_branch = session.git_branch.is_some();
                        let preview_str = session.preview.clone().unwrap_or_default();
                        let has_preview = session.preview.is_some();
                        let arrow = if is_expanded { "▼" } else { "▶" };

                        let card_cls = if is_expanded { "card session-card session-expanded" } else { "card session-card session-clickable" };

                        // For expanding: load messages
                        let sid_for_click = sid.clone();
                        let claude_dirs = project.claude_dir_names.clone();

                        rsx! {
                            div { class: "{card_cls}",
                                // Clickable header
                                div {
                                    class: "session-header-row",
                                    onclick: move |_| {
                                        if expanded_session() == Some(sid_for_click.clone()) {
                                            // Collapse
                                            expanded_session.set(None);
                                            expanded_messages.set(Vec::new());
                                        } else {
                                            // Expand: load messages
                                            let sid_load = sid_for_click.clone();
                                            let dirs = claude_dirs.clone();
                                            expanded_session.set(Some(sid_load.clone()));
                                            loading_detail.set(true);
                                            spawn(async move {
                                                let msgs = tokio::task::spawn_blocking(move || {
                                                    let home = dirs::home_dir().unwrap_or_default();
                                                    for dir_name in &dirs {
                                                        let claude_dir = home.join(".claude").join("projects").join(dir_name);
                                                        let msgs = session_reader::read_session_messages(&claude_dir, &sid_load);
                                                        if !msgs.is_empty() {
                                                            return msgs;
                                                        }
                                                    }
                                                    Vec::new()
                                                }).await.unwrap_or_default();
                                                expanded_messages.set(msgs);
                                                loading_detail.set(false);
                                            });
                                        }
                                    },
                                    span { class: "session-arrow", "{arrow}" }
                                    div { class: "session-row", style: "flex: 1;",
                                        if has_ts {
                                            span { class: "session-time", "{ts_str}" }
                                        }
                                        if has_branch {
                                            span { class: "session-branch", "{branch_str}" }
                                        }
                                        span { class: "session-msgs", "{msg_count} 条消息" }
                                    }
                                }
                                if !is_expanded {
                                    if has_preview {
                                        div { class: "session-preview-text", style: "padding-left: 22px;", "{preview_str}" }
                                    }
                                }
                                // Expanded detail
                                if is_expanded {
                                    div { class: "session-detail",
                                        if loading_detail() {
                                            p { style: "color: #86868b; padding: 12px; text-align: center;", "加载中..." }
                                        } else if expanded_messages().is_empty() {
                                            p { style: "color: #86868b; padding: 12px; text-align: center;", "无法加载会话内容" }
                                        } else {
                                            {expanded_messages().iter().map(|msg| {
                                                let is_user = msg.role == "user";
                                                let role_label = if is_user { "用户" } else { "助手" };
                                                let bubble_cls = if is_user { "msg-bubble msg-user" } else { "msg-bubble msg-assistant" };
                                                let ts_display = msg.timestamp
                                                    .map(|t| t.format("%H:%M:%S").to_string())
                                                    .unwrap_or_default();
                                                let has_msg_ts = msg.timestamp.is_some();
                                                let content_display = truncate_msg(&msg.content, 2000);
                                                rsx! {
                                                    div { class: "{bubble_cls}",
                                                        div { class: "msg-header",
                                                            span { class: "msg-role", "{role_label}" }
                                                            if has_msg_ts {
                                                                span { class: "msg-time", "{ts_display}" }
                                                            }
                                                        }
                                                        div { class: "msg-content", "{content_display}" }
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

fn truncate_msg(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...\n\n[内容过长，已截断]", truncated)
    } else {
        truncated
    }
}
