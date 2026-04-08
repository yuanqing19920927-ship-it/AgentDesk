use dioxus::prelude::*;
use crate::models::Project;

#[component]
pub fn Sidebar(
    projects: Vec<Project>,
    selected_idx: Option<usize>,
    on_select: EventHandler<usize>,
    on_add_project: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-title", "AgentDesk" }
            div { style: "display: flex; justify-content: space-between; align-items: center; padding: 0 16px;",
                div { class: "sidebar-section-label", style: "padding: 12px 0 6px;", "项目" }
                button {
                    class: "sidebar-add-btn",
                    title: "新增项目",
                    onclick: move |_| on_add_project.call(()),
                    "＋"
                }
            }
            div { class: "project-list",
                for (i, project) in projects.iter().enumerate() {
                    {
                        let is_selected = selected_idx == Some(i);
                        let cls = if is_selected { "project-item selected" } else { "project-item" };
                        let agent_count = project.agent_count;
                        let session_count = project.session_count;
                        let is_custom = project.claude_dir_names.is_empty();
                        rsx! {
                            div {
                                class: "{cls}",
                                onclick: move |_| on_select.call(i),
                                div { class: "project-name",
                                    "{project.name}"
                                    if is_custom {
                                        span { class: "custom-badge", "手动" }
                                    }
                                }
                                div { class: "project-path", "{project.root.display()}" }
                                div { class: "project-meta",
                                    if agent_count > 0 {
                                        span { class: "agent-badge", "{agent_count}" }
                                    }
                                    if session_count > 0 {
                                        span { "{session_count} 次会话" }
                                    }
                                }
                            }
                        }
                    }
                }
                if projects.is_empty() {
                    div { class: "empty-state",
                        p { style: "font-size: 13px;", "未发现项目" }
                        p { style: "font-size: 11px; margin-top: 6px;",
                            "使用 Claude Code 或点击 ＋ 添加项目"
                        }
                    }
                }
            }
        }
    }
}
