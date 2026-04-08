use dioxus::prelude::*;
use crate::models::Project;

#[component]
pub fn Sidebar(
    projects: Vec<Project>,
    selected_idx: Option<usize>,
    on_select: EventHandler<usize>,
) -> Element {
    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-title", "AgentDesk" }
            div { class: "sidebar-section-label", "项目" }
            div { class: "project-list",
                for (i, project) in projects.iter().enumerate() {
                    {
                        let is_selected = selected_idx == Some(i);
                        let cls = if is_selected { "project-item selected" } else { "project-item" };
                        let agent_count = project.agent_count;
                        let session_count = project.session_count;
                        rsx! {
                            div {
                                class: "{cls}",
                                onclick: move |_| on_select.call(i),
                                div { class: "project-name", "{project.name}" }
                                div { class: "project-path", "{project.root.display()}" }
                                div { class: "project-meta",
                                    if agent_count > 0 {
                                        span { class: "agent-badge", "{agent_count}" }
                                    }
                                    span { "{session_count} 次会话" }
                                }
                            }
                        }
                    }
                }
                if projects.is_empty() {
                    div { class: "empty-state",
                        p { style: "font-size: 13px;", "未发现项目" }
                        p { style: "font-size: 11px; margin-top: 6px;",
                            "使用 Claude Code 后项目会自动出现"
                        }
                    }
                }
            }
        }
    }
}
