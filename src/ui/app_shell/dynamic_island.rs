use dioxus::prelude::*;
use crate::models::{Agent, AgentStatus};
use crate::services::agent_detector;

#[component]
pub fn DynamicIsland(agents: Vec<Agent>) -> Element {
    let busy_agents: Vec<&Agent> = agents.iter().filter(|a| a.status == AgentStatus::Busy && !a.is_subagent).collect();
    let idle_agents: Vec<&Agent> = agents.iter().filter(|a| a.status == AgentStatus::Idle && !a.is_subagent).collect();
    let total = busy_agents.len() + idle_agents.len();

    if total == 0 {
        return rsx! {
            div { class: "island island-empty",
                div { class: "island-content",
                    span { class: "island-icon", "💤" }
                    span { class: "island-text", "暂无活跃 Agent" }
                }
            }
        };
    }

    // Compact mode: show summary pill
    let busy_count = busy_agents.len();
    let idle_count = idle_agents.len();

    rsx! {
        div { class: "island",
            div { class: "island-content",
                // Busy agents
                if busy_count > 0 {
                    div { class: "island-group",
                        div { class: "island-dot busy" }
                        span { class: "island-count", "{busy_count}" }
                        span { class: "island-label", "工作中" }
                    }
                }
                // Idle agents
                if idle_count > 0 {
                    div { class: "island-group",
                        div { class: "island-dot idle" }
                        span { class: "island-count", "{idle_count}" }
                        span { class: "island-label", "空闲" }
                    }
                }

                // Separator
                div { class: "island-sep" }

                // Agent details (compact pills)
                {busy_agents.iter().map(|a| {
                    let label = a.agent_type.label().to_string();
                    let cpu = a.cpu_percent;
                    let proj = a.cwd.as_ref()
                        .and_then(|c| c.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let has_tty = a.tty.is_some();
                    let tty = a.tty.clone();
                    rsx! {
                        div { class: "island-agent busy",
                            span { class: "island-agent-name", "{proj}" }
                            span { class: "island-agent-cpu", "{cpu:.0}%" }
                            if has_tty {
                                button {
                                    class: "island-jump",
                                    onclick: move |_| {
                                        if let Some(ref t) = tty {
                                            let tc = t.clone();
                                            spawn(async move { let _ = tokio::task::spawn_blocking(move || agent_detector::focus_agent_terminal(&tc)).await; });
                                        }
                                    },
                                    "↗"
                                }
                            }
                        }
                    }
                })}
                {idle_agents.iter().map(|a| {
                    let proj = a.cwd.as_ref()
                        .and_then(|c| c.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let has_tty = a.tty.is_some();
                    let tty = a.tty.clone();
                    rsx! {
                        div { class: "island-agent idle",
                            span { class: "island-agent-name", "{proj}" }
                            if has_tty {
                                button {
                                    class: "island-jump",
                                    onclick: move |_| {
                                        if let Some(ref t) = tty {
                                            let tc = t.clone();
                                            spawn(async move { let _ = tokio::task::spawn_blocking(move || agent_detector::focus_agent_terminal(&tc)).await; });
                                        }
                                    },
                                    "↗"
                                }
                            }
                        }
                    }
                })}
            }
        }
    }
}
