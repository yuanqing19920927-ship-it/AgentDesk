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
            div { class: "sidebar-header", "项目" }
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
                                    span { class: "agent-badge", "{project.agent_count} 个 Agent" }
                                }
                                span { "{project.session_count} 次会话" }
                            }
                        }
                    }
                }
            }
            if projects.is_empty() {
                div { class: "empty-state",
                    p { "未发现项目" }
                    p { style: "font-size: 11px; margin-top: 6px;", "使用 Claude Code 后项目会自动出现" }
                }
            }
        }
    }
}
