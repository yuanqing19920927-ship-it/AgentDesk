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
            div {
                style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;",
                div {
                    h1 { class: "section-title", "{project.name}" }
                    p { style: "color: #6c7086; font-size: 13px;", "{project.root.display()}" }
                }
                button { class: "btn btn-primary", onclick: move |_| on_new_agent.call(()), "+ New Agent" }
            }

            h2 { style: "font-size: 16px; font-weight: 600; margin-bottom: 12px;",
                "Running Agents ({agents.len()})"
            }
            if agents.is_empty() {
                div { class: "card",
                    p { style: "color: #6c7086; text-align: center;", "No agents running in this project" }
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
                h2 { style: "font-size: 16px; font-weight: 600; margin-bottom: 12px;", "Project Info" }
                div { class: "card",
                    div { style: "display: flex; gap: 24px;",
                        div {
                            div { style: "font-size: 24px; font-weight: 700; color: #89b4fa;", "{project.session_count}" }
                            div { style: "font-size: 12px; color: #6c7086;", "Sessions" }
                        }
                        div {
                            div { style: "font-size: 24px; font-weight: 700; color: #a6e3a1;", "{project.agent_count}" }
                            div { style: "font-size: 12px; color: #6c7086;", "Active Agents" }
                        }
                        if let Some(ref last_str) = last_active_str {
                            div {
                                div { style: "font-size: 24px; font-weight: 700; color: #f9e2af;", "{last_str}" }
                                div { style: "font-size: 12px; color: #6c7086;", "Last Active" }
                            }
                        }
                    }
                }
            }

            // Recent sessions
            div { style: "margin-top: 24px;",
                h2 { style: "font-size: 16px; font-weight: 600; margin-bottom: 12px;",
                    "Recent Sessions ({sessions.len()})"
                }
                if sessions.is_empty() {
                    div { class: "card",
                        p { style: "color: #6c7086; text-align: center;", "No sessions found" }
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
                                    div { class: "session-meta", "{msg_count} messages" }
                                    if has_branch {
                                        div { class: "session-meta", "branch: {branch_str}" }
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
