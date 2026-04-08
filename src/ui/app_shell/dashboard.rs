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

    let last_active_str = project.last_active
        .map(|dt| dt.format("%m-%d %H:%M").to_string());

    rsx! {
        div {
            // Header
            div {
                style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px;",
                div {
                    h1 { class: "section-title", "{project.name}" }
                    p { style: "color: #86868b; font-size: 12px;", "{project.root.display()}" }
                }
                button { class: "btn btn-primary", onclick: move |_| on_new_agent.call(()), "+ 新建 Agent" }
            }

            // Running agents
            h2 { style: "font-size: 15px; font-weight: 600; margin-bottom: 10px; color: #3a3a3c;",
                "运行中的 Agent ({agents.len()})"
            }
            if agents.is_empty() {
                div { class: "card",
                    p { style: "color: #86868b; text-align: center; padding: 8px;", "当前项目没有运行中的 Agent" }
                }
            } else {
                {agents.iter().map(|agent| {
                    let cwd_str = agent.cwd.as_ref().map(|c| c.display().to_string()).unwrap_or_default();
                    let has_cwd = agent.cwd.is_some();
                    let label = agent.agent_type.label().to_string();
                    let pid = agent.pid;
                    rsx! {
                        div { class: "card agent-card",
                            div { class: "agent-info",
                                div {
                                    span { class: "status-dot" }
                                    span { class: "agent-type", "{label}" }
                                }
                                div { class: "agent-pid", "PID: {pid}" }
                                if has_cwd {
                                    div { class: "agent-cwd", "{cwd_str}" }
                                }
                            }
                        }
                    }
                })}
            }

            // Project stats
            div { style: "margin-top: 24px;",
                h2 { style: "font-size: 15px; font-weight: 600; margin-bottom: 10px; color: #3a3a3c;", "项目概览" }
                div { class: "card",
                    div { class: "stats-row",
                        div { class: "stat-item",
                            div { class: "stat-value blue", "{project.session_count}" }
                            div { class: "stat-label", "会话数" }
                        }
                        div { class: "stat-item",
                            div { class: "stat-value green", "{project.agent_count}" }
                            div { class: "stat-label", "活跃 Agent" }
                        }
                        if let Some(ref last_str) = last_active_str {
                            div { class: "stat-item",
                                div { class: "stat-value orange", "{last_str}" }
                                div { class: "stat-label", "最近活跃" }
                            }
                        }
                    }
                }
            }

            // Recent sessions
            div { style: "margin-top: 24px;",
                h2 { style: "font-size: 15px; font-weight: 600; margin-bottom: 10px; color: #3a3a3c;",
                    "近期会话 ({sessions.len()})"
                }
                if sessions.is_empty() {
                    div { class: "card",
                        p { style: "color: #86868b; text-align: center; padding: 8px;", "暂无会话记录" }
                    }
                } else {
                    {recent_sessions.iter().map(|session| {
                        let ts_str = session.started_at.map(|ts| ts.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
                        let has_ts = session.started_at.is_some();
                        let msg_count = session.message_count;
                        let branch_str = session.git_branch.clone().unwrap_or_default();
                        let has_branch = session.git_branch.is_some();
                        let preview_str = session.preview.clone().unwrap_or_default();
                        let has_preview = session.preview.is_some();
                        rsx! {
                            div { class: "card session-item",
                                div { style: "display: flex; justify-content: space-between;",
                                    if has_ts {
                                        div { class: "session-meta", "{ts_str}" }
                                    }
                                    div { class: "session-meta", "{msg_count} 条消息" }
                                    if has_branch {
                                        div { class: "session-meta", "分支: {branch_str}" }
                                    }
                                }
                                if has_preview {
                                    div { class: "session-preview", "{preview_str}" }
                                }
                            }
                        }
                    })}
                }
            }
        }
    }
}
