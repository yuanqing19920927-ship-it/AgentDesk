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
            div { class: "sidebar-header", "AgentDesk" }
            for (i, project) in projects.iter().enumerate() {
                {
                    let is_selected = selected_idx == Some(i);
                    let cls = if is_selected { "project-item selected" } else { "project-item" };
                    rsx! {
                        div {
                            class: "{cls}",
                            onclick: move |_| on_select.call(i),
                            div { class: "project-name", "{project.name}" }
                            div { class: "project-path", "{project.root.display()}" }
                            div { class: "project-meta",
                                if project.agent_count > 0 {
                                    span { class: "agent-badge", "{project.agent_count} agent(s)" }
                                }
                                span { "{project.session_count} sessions" }
                            }
                        }
                    }
                }
            }
            if projects.is_empty() {
                div { class: "empty-state",
                    p { "No projects found" }
                    p { style: "font-size: 12px; margin-top: 8px;", "Projects appear after using Claude Code" }
                }
            }
        }
    }
}
