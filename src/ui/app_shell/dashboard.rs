use dioxus::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use crate::models::{Agent, Project, SessionSummary};

/// Recursively scan project for .md files, skipping hidden dirs and common noise
fn scan_docs(root: &std::path::Path) -> Vec<PathBuf> {
    let mut docs = Vec::new();
    scan_docs_recursive(root, root, &mut docs, 0);
    docs.sort_by(|a, b| {
        // Root-level docs first, then by path
        let a_depth = a.strip_prefix(root).map(|p| p.components().count()).unwrap_or(99);
        let b_depth = b.strip_prefix(root).map(|p| p.components().count()).unwrap_or(99);
        a_depth.cmp(&b_depth).then_with(|| a.cmp(b))
    });
    docs
}

fn scan_docs_recursive(root: &std::path::Path, dir: &std::path::Path, docs: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 { return; } // limit recursion depth
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
            // Skip hidden dirs, build artifacts, dependencies
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "build"
                || name == "dist"
                || name == "vendor"
                || name == ".build"
                || name == "Pods"
            {
                continue;
            }
            scan_docs_recursive(root, &path, docs, depth + 1);
        }
    }
}

fn open_file(path: &std::path::Path) {
    let _ = Command::new("open").arg(path).spawn();
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

    let session_count = sessions.len();
    let recent_sessions: Vec<&SessionSummary> = sessions.iter().take(5).collect();

    // Compute total messages across all sessions
    let total_messages: usize = sessions.iter().map(|s| s.message_count).sum();

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

            // ── Stats overview ──
            div { class: "section",
                div { class: "section-label", "项目总览" }
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
                // Inline recent session previews
                if !recent_sessions.is_empty() {
                    div { style: "margin-top: 12px;",
                        {recent_sessions.iter().map(|session| {
                            let ts_str = session.started_at
                                .map(|ts| ts.format("%m-%d %H:%M").to_string())
                                .unwrap_or_default();
                            let has_ts = session.started_at.is_some();
                            let msg_count = session.message_count;
                            let branch_str = session.git_branch.clone().unwrap_or_default();
                            let has_branch = session.git_branch.is_some();
                            let preview_str = session.preview.clone().unwrap_or_default();
                            let has_preview = session.preview.is_some();
                            rsx! {
                                div { class: "card session-card",
                                    div { class: "session-row",
                                        if has_ts {
                                            span { class: "session-time", "{ts_str}" }
                                        }
                                        if has_branch {
                                            span { class: "session-branch", "{branch_str}" }
                                        }
                                        span { class: "session-msgs", "{msg_count} 条消息" }
                                    }
                                    if has_preview {
                                        div { class: "session-preview-text", "{preview_str}" }
                                    }
                                }
                            }
                        })}
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
        }
    }
}
