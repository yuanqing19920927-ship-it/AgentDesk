use dioxus::prelude::*;
use crate::models::Project;
use crate::services::project_manager;

#[component]
pub fn Sidebar(
    projects: Vec<Project>,
    selected_idx: Option<usize>,
    on_select: EventHandler<usize>,
    on_add_project: EventHandler<()>,
) -> Element {
    let nicknames = use_hook(|| project_manager::load_nicknames());
    let mut editing_idx = use_signal(|| None::<usize>);
    let mut edit_value = use_signal(String::new);

    // Separate home project from the rest
    let home_dir = dirs::home_dir().unwrap_or_default();
    let home_str = home_dir.to_string_lossy().to_string();

    let home_idx = projects.iter().position(|p| p.root.to_string_lossy() == home_str);
    let other_projects: Vec<(usize, &Project)> = projects.iter().enumerate()
        .filter(|(_, p)| p.root.to_string_lossy() != home_str)
        .collect();

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-title", "AgentDesk" }

            // ── Home project (pinned) ──
            if let Some(hi) = home_idx {
                {
                    let home_proj = &projects[hi];
                    let is_selected = selected_idx == Some(hi);
                    let nick = nicknames.get(&home_proj.root.to_string_lossy().to_string()).cloned();
                    let display = nick.unwrap_or_else(|| "主目录".to_string());
                    let cls = if is_selected { "home-item selected" } else { "home-item" };
                    let agent_count = home_proj.agent_count;
                    let session_count = home_proj.session_count;
                    rsx! {
                        div {
                            class: "{cls}",
                            onclick: move |_| on_select.call(hi),
                            div { class: "home-icon", "🏠" }
                            div { class: "home-info",
                                div { class: "home-name", "{display}" }
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
            }

            // ── Projects section ──
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
                for (i, project) in other_projects.iter() {
                    {
                        let idx = *i;
                        let is_selected = selected_idx == Some(idx);
                        let cls = if is_selected { "project-item selected" } else { "project-item" };
                        let is_editing = editing_idx() == Some(idx);
                        let is_custom = project.claude_dir_names.is_empty();
                        let agent_count = project.agent_count;
                        let session_count = project.session_count;
                        let root_str = project.root.to_string_lossy().to_string();
                        let nick = nicknames.get(&root_str).cloned();
                        let display_name = nick.clone().unwrap_or_else(|| project.name.clone());
                        let has_nick = nick.is_some();
                        let root_for_key = root_str.clone();
                        let root_for_blur = root_str.clone();

                        rsx! {
                            div {
                                class: "{cls}",
                                onclick: move |_| {
                                    if editing_idx() != Some(idx) {
                                        on_select.call(idx);
                                    }
                                },
                                ondoubleclick: move |_| {
                                    editing_idx.set(Some(idx));
                                    edit_value.set(display_name.clone());
                                },
                                if is_editing {
                                    input {
                                        class: "nickname-input",
                                        value: "{edit_value}",
                                        autofocus: true,
                                        oninput: move |e| edit_value.set(e.value()),
                                        onkeydown: move |e| {
                                            if e.key() == Key::Enter {
                                                let val = edit_value().clone();
                                                let path = root_for_key.clone();
                                                project_manager::set_nickname(&path, &val);
                                                editing_idx.set(None);
                                            } else if e.key() == Key::Escape {
                                                editing_idx.set(None);
                                            }
                                        },
                                        onfocusout: move |_| {
                                            let val = edit_value().clone();
                                            let path = root_for_blur.clone();
                                            project_manager::set_nickname(&path, &val);
                                            editing_idx.set(None);
                                        },
                                    }
                                } else {
                                    div { class: "project-name-row",
                                        span { class: "project-name", "{display_name}" }
                                        if is_custom {
                                            span { class: "custom-badge", "手动" }
                                        }
                                        if has_nick {
                                            span { class: "nick-badge", "备注" }
                                        }
                                    }
                                }
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
                if other_projects.is_empty() {
                    div { class: "empty-state",
                        p { style: "font-size: 13px;", "未发现项目" }
                        p { style: "font-size: 11px; margin-top: 6px;",
                            "使用 Claude Code 或点击 ＋ 添加"
                        }
                    }
                }
            }
        }
    }
}
