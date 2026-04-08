use dioxus::prelude::*;
use crate::models::{Agent, Project, SessionSummary};

#[component]
pub fn Dashboard(
    project: Project,
    agents: Vec<Agent>,
    sessions: Vec<SessionSummary>,
    on_new_agent: EventHandler<()>,
) -> Element {
    let recent_sessions: Vec<&SessionSummary> = sessions.iter().take(10).collect();
    let last_active_str = project.last_active.map(|dt| dt.format("%m-%d %H:%M").to_string());
    let has_last_active = last_active_str.is_some();
    let last_active_display = last_active_str.unwrap_or_default();

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

            // ── Stats ──
            div { class: "section",
                div { class: "section-label", "概览" }
                div { class: "stats-grid",
                    div { class: "stat-card",
                        div { class: "stat-value green", "{project.agent_count}" }
                        div { class: "stat-label", "运行中 Agent" }
                    }
                    div { class: "stat-card",
                        div { class: "stat-value blue", "{project.session_count}" }
                        div { class: "stat-label", "会话总数" }
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

            // ── Recent sessions ──
            div { class: "section",
                div { class: "section-label", "近期会话 ({sessions.len()})" }
                if sessions.is_empty() {
                    div { class: "card",
                        p { style: "color: #86868b; text-align: center; padding: 6px; font-size: 12px;",
                            "暂无会话记录"
                        }
                    }
                } else {
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
    }
}
