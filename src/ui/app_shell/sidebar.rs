use dioxus::prelude::*;
use crate::models::Project;
use crate::services::project_manager;

const ICON_COLORS: &[&str] = &["", "orange", "purple", "pink", "teal"];

fn icon_color(idx: usize) -> &'static str {
    ICON_COLORS[idx % ICON_COLORS.len()]
}

fn initial_char(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string())
}

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
    let mut ctx_menu = use_signal(|| None::<(usize, f64, f64)>);

    let home_dir = dirs::home_dir().unwrap_or_default();
    let home_str = home_dir.to_string_lossy().to_string();
    let home_idx = projects.iter().position(|p| p.root.to_string_lossy() == home_str);
    let other_projects: Vec<(usize, &Project)> = projects.iter().enumerate()
        .filter(|(_, p)| p.root.to_string_lossy() != home_str).collect();

    rsx! {
        div { class: "sidebar",
            onclick: move |_| ctx_menu.set(None),

            // ── Home pinned ──
            if let Some(hi) = home_idx {
                {
                    let hp = &projects[hi];
                    let is_sel = selected_idx == Some(hi);
                    let cls = if is_sel { "home-item selected" } else { "home-item" };
                    let nick = nicknames.get(&hp.root.to_string_lossy().to_string()).cloned();
                    let display = nick.unwrap_or_else(|| "主目录".to_string());
                    let ac = hp.agent_count;
                    let sc = hp.session_count;
                    let hr = hp.root.to_string_lossy().to_string();
                    rsx! {
                        div {
                            class: "{cls}",
                            onclick: move |_| on_select.call(hi),
                            oncontextmenu: move |e| {
                                e.prevent_default();
                                let c = e.page_coordinates();
                                ctx_menu.set(Some((hi, c.x, c.y)));
                            },
                            div { class: "home-icon-box", "🏠" }
                            div { class: "home-info",
                                if editing_idx() == Some(hi) {
                                    input {
                                        class: "nickname-input", value: "{edit_value}", autofocus: true,
                                        onclick: move |e| e.stop_propagation(),
                                        oninput: move |e| edit_value.set(e.value()),
                                        onkeydown: { let p = hr.clone(); move |e: KeyboardEvent| {
                                            if e.key() == Key::Enter { project_manager::set_nickname(&p, &edit_value()); editing_idx.set(None); }
                                            else if e.key() == Key::Escape { editing_idx.set(None); }
                                        }},
                                        onfocusout: { let p = hr.clone(); move |_| { project_manager::set_nickname(&p, &edit_value()); editing_idx.set(None); }},
                                    }
                                } else {
                                    div { class: "home-name", "{display}" }
                                }
                                div { class: "project-meta",
                                    if ac > 0 { span { class: "agent-badge", "{ac}" } }
                                    if sc > 0 { span { "{sc} 次会话" } }
                                }
                            }
                        }
                    }
                }
            }

            // ── Section header ──
            div { style: "display: flex; justify-content: space-between; align-items: center; padding: 0 20px;",
                div { class: "sidebar-section-label", "项目" }
                button { class: "sidebar-add-btn", title: "新增项目", onclick: move |_| on_add_project.call(()), "＋" }
            }

            // ── Project list ──
            div { class: "project-list",
                for (i, project) in other_projects.iter() {
                    {
                        let idx = *i;
                        let is_sel = selected_idx == Some(idx);
                        let cls = if is_sel { "project-item selected" } else { "project-item" };
                        let is_editing = editing_idx() == Some(idx);
                        let is_custom = project.claude_dir_names.is_empty();
                        let ac = project.agent_count;
                        let sc = project.session_count;
                        let rs = project.root.to_string_lossy().to_string();
                        let nick = nicknames.get(&rs).cloned();
                        let dn = nick.clone().unwrap_or_else(|| project.name.clone());
                        let has_nick = nick.is_some();
                        let rk = rs.clone();
                        let rb = rs.clone();
                        let ic = initial_char(&dn);
                        let color_cls = icon_color(idx);
                        let icon_cls = if color_cls.is_empty() { "project-icon-box".to_string() } else { format!("project-icon-box {}", color_cls) };

                        rsx! {
                            div {
                                class: "{cls}",
                                onclick: move |_| { if editing_idx() != Some(idx) { on_select.call(idx); } },
                                oncontextmenu: move |e| { e.prevent_default(); let c = e.page_coordinates(); ctx_menu.set(Some((idx, c.x, c.y))); },
                                div { class: "{icon_cls}", "{ic}" }
                                div { class: "project-item-info",
                                    if is_editing {
                                        input {
                                            class: "nickname-input", value: "{edit_value}", autofocus: true,
                                            onclick: move |e| e.stop_propagation(),
                                            oninput: move |e| edit_value.set(e.value()),
                                            onkeydown: move |e| {
                                                if e.key() == Key::Enter { project_manager::set_nickname(&rk, &edit_value()); editing_idx.set(None); }
                                                else if e.key() == Key::Escape { editing_idx.set(None); }
                                            },
                                            onfocusout: move |_| { project_manager::set_nickname(&rb, &edit_value()); editing_idx.set(None); },
                                        }
                                    } else {
                                        div { class: "project-name-row",
                                            span { class: "project-name", "{dn}" }
                                            if is_custom { span { class: "custom-badge", "手动" } }
                                            if has_nick { span { class: "nick-badge", "备注" } }
                                        }
                                    }
                                    div { class: "project-meta",
                                        if ac > 0 { span { class: "agent-badge", "{ac}" } }
                                        if sc > 0 { span { "{sc} 次会话" } }
                                    }
                                }
                            }
                        }
                    }
                }
                if other_projects.is_empty() {
                    div { class: "empty-state", style: "padding: 30px 16px;",
                        p { style: "font-size: 13px;", "未发现项目" }
                        p { style: "font-size: 11px; margin-top: 4px; color: #aeaeb2;", "使用 Claude Code 或点击 ＋ 添加" }
                    }
                }
            }
        }

        // ── Context menu ──
        if let Some((mi, mx, my)) = ctx_menu() {
            {
                let mp = projects.get(mi);
                let mr = mp.map(|p| p.root.to_string_lossy().to_string()).unwrap_or_default();
                let mn = mp.map(|p| nicknames.get(&p.root.to_string_lossy().to_string()).cloned().unwrap_or_else(|| p.name.clone())).unwrap_or_default();
                let is_cust = mp.map(|p| p.claude_dir_names.is_empty()).unwrap_or(false);
                let r1 = mr.clone(); let r2 = mr.clone(); let r3 = mr.clone();
                rsx! {
                    div { class: "ctx-backdrop", onclick: move |_| ctx_menu.set(None) }
                    div { class: "ctx-menu", style: "left: {mx}px; top: {my}px;",
                        div { class: "ctx-menu-item", onclick: move |_| { ctx_menu.set(None); editing_idx.set(Some(mi)); edit_value.set(mn.clone()); }, "✏️  修改备注名" }
                        div { class: "ctx-menu-item", onclick: move |_| { ctx_menu.set(None); let _ = std::process::Command::new("open").arg(&r1).spawn(); }, "📂  在 Finder 中打开" }
                        div { class: "ctx-menu-item", onclick: move |_| { ctx_menu.set(None); let _ = std::process::Command::new("bash").args(["-c", &format!("echo -n '{}' | pbcopy", r2)]).spawn(); }, "📋  复制路径" }
                        if is_cust {
                            div { class: "ctx-menu-item ctx-menu-danger", onclick: move |_| { ctx_menu.set(None); project_manager::remove_custom_project(&r3); }, "🗑  移除项目" }
                        }
                    }
                }
            }
        }
    }
}
