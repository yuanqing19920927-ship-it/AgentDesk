use dioxus::prelude::*;
use crate::models::Project;
use crate::services::{config, project_manager};
use crate::ui::icons;

const ICON_COLORS: &[&str] = &["", "orange", "purple", "pink", "teal"];
fn icon_color(idx: usize) -> &'static str { ICON_COLORS[idx % ICON_COLORS.len()] }
fn initial_char(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string())
}

#[component]
pub fn Sidebar(
    projects: Vec<Project>,
    selected_idx: Option<usize>,
    on_select: EventHandler<usize>,
    on_settings: EventHandler<()>,
    on_templates: EventHandler<()>,
) -> Element {
    let mut nicknames = use_signal(|| project_manager::load_nicknames());
    let mut editing_idx = use_signal(|| None::<usize>);
    let mut edit_value = use_signal(String::new);
    let mut ctx_menu = use_signal(|| None::<(usize, f64, f64)>);

    let home_dir = dirs::home_dir().unwrap_or_default();
    let home_str = home_dir.to_string_lossy().to_string();
    let home_idx = projects.iter().position(|p| p.root.to_string_lossy() == home_str);

    let cfg = config::load_config();
    let group_defs = cfg.groups.clone();
    let project_groups = cfg.project_groups.clone();

    let other_projects: Vec<(usize, &Project)> = projects.iter().enumerate()
        .filter(|(_, p)| p.root.to_string_lossy() != home_str).collect();

    // Build groups
    let mut grouped: std::collections::BTreeMap<String, Vec<(usize, &Project)>> = std::collections::BTreeMap::new();
    let mut ungrouped: Vec<(usize, &Project)> = Vec::new();
    for (i, p) in &other_projects {
        let rs = p.root.to_string_lossy().to_string();
        if let Some(g) = project_groups.get(&rs) {
            grouped.entry(g.clone()).or_default().push((*i, p));
        } else {
            ungrouped.push((*i, p));
        }
    }

    // Collect all items to render in order: groups first, then ungrouped
    struct SidebarSection<'a> {
        label: String,
        items: Vec<(usize, &'a Project)>,
    }
    let mut sections: Vec<SidebarSection> = Vec::new();
    for gdef in &group_defs {
        sections.push(SidebarSection {
            label: gdef.name.clone(),
            items: grouped.get(&gdef.name).cloned().unwrap_or_default(),
        });
    }
    let ungrouped_label = if group_defs.is_empty() { "项目".to_string() } else { "未分组".to_string() };
    if !ungrouped.is_empty() || sections.is_empty() {
        sections.push(SidebarSection { label: ungrouped_label, items: ungrouped });
    }

    rsx! {
        div { class: "sidebar",
            onclick: move |_| ctx_menu.set(None),

            // Home pinned
            if let Some(hi) = home_idx {
                {
                    let hp = &projects[hi];
                    let is_sel = selected_idx == Some(hi);
                    let cls = if is_sel { "home-item selected" } else { "home-item" };
                    let nick = nicknames().get(&hp.root.to_string_lossy().to_string()).cloned();
                    let display = nick.unwrap_or_else(|| "主目录".to_string());
                    let ac = hp.agent_count; let sc = hp.session_count;
                    rsx! {
                        div { class: "{cls}",
                            onclick: move |_| on_select.call(hi),
                            oncontextmenu: move |e| { e.prevent_default(); let c = e.page_coordinates(); ctx_menu.set(Some((hi, c.x, c.y))); },
                            div { class: "icon-tile icon-tile-md tile-blue project-tile-3d", dangerous_inner_html: icons::HOME }
                            div { class: "home-info",
                                div { class: "home-name", "{display}" }
                                div { class: "project-meta",
                                    if ac > 0 { span { class: "agent-badge", "{ac}" } }
                                    if sc > 0 { span { "{sc} 次会话" } }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "project-list",
                for section in sections.iter() {
                    {
                        let label = section.label.clone();
                        let items = section.items.clone();
                        rsx! {
                            div { class: "sidebar-section-label", "{label}" }
                            if items.is_empty() {
                                div { style: "padding: 4px 20px; font-size: 11px; color: #aeaeb2;", "暂无项目" }
                            }
                            for (idx, project) in items.iter() {
                                {
                                    let idx = *idx;
                                    let is_sel = selected_idx == Some(idx);
                                    let cls = if is_sel { "project-item selected" } else { "project-item" };
                                    let is_editing = editing_idx() == Some(idx);
                                    let is_custom = project.claude_dir_names.is_empty();
                                    let ac = project.agent_count;
                                    let sc = project.session_count;
                                    let rs = project.root.to_string_lossy().to_string();
                                    let nick = nicknames().get(&rs).cloned();
                                    let dn = nick.clone().unwrap_or_else(|| project.name.clone());
                                    let has_nick = nick.is_some();
                                    let rk = rs.clone(); let rb = rs.clone();
                                    let ic = initial_char(&dn);
                                    let cc = icon_color(idx);
                                    let icon_cls = if cc.is_empty() { "project-icon-box project-tile-3d".to_string() } else { format!("project-icon-box project-tile-3d {}", cc) };

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
                                                            if e.key() == Key::Enter { project_manager::set_nickname(&rk, &edit_value()); nicknames.set(project_manager::load_nicknames()); editing_idx.set(None); }
                                                            else if e.key() == Key::Escape { editing_idx.set(None); }
                                                        },
                                                        onfocusout: move |_| { project_manager::set_nickname(&rb, &edit_value()); nicknames.set(project_manager::load_nicknames()); editing_idx.set(None); },
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
                        }
                    }
                }
            }

            div { class: "sidebar-footer",
                div { class: "sidebar-footer-btn", onclick: move |_| on_templates.call(()),
                    div { class: "icon-tile icon-tile-sm tile-indigo", dangerous_inner_html: icons::DOC_STACK }
                    span { "模板" }
                }
                div { class: "sidebar-footer-btn", onclick: move |_| on_settings.call(()),
                    div { class: "icon-tile icon-tile-sm tile-graphite", dangerous_inner_html: icons::GEAR }
                    span { "设置" }
                }
            }
        }

        // ── Context menu ──
        if let Some((mi, mx, my)) = ctx_menu() {
            {
                let mp = projects.get(mi);
                let mr = mp.map(|p| p.root.to_string_lossy().to_string()).unwrap_or_default();
                let mn = mp.map(|p| nicknames().get(&p.root.to_string_lossy().to_string()).cloned().unwrap_or_else(|| p.name.clone())).unwrap_or_default();
                let r1 = mr.clone(); let r2 = mr.clone(); let r3 = mr.clone();

                let cfg_groups = config::load_config().groups;
                let current_group = config::get_project_group(&mr);

                rsx! {
                    div { class: "ctx-backdrop", onclick: move |_| ctx_menu.set(None) }
                    div { class: "ctx-menu", style: "left: {mx}px; top: {my}px;",
                        div { class: "ctx-menu-item",
                            onclick: move |_| { ctx_menu.set(None); editing_idx.set(Some(mi)); edit_value.set(mn.clone()); },
                            "✏️  修改备注名"
                        }
                        if !cfg_groups.is_empty() {
                            div { class: "ctx-menu-separator" }
                            div { class: "ctx-menu-header", "移动到分组" }
                            {cfg_groups.iter().map(|g| {
                                let gn = g.name.clone();
                                let is_current = current_group.as_ref() == Some(&gn);
                                let rr = r1.clone();
                                rsx! {
                                    div { class: if is_current { "ctx-menu-item ctx-menu-active" } else { "ctx-menu-item" },
                                        onclick: move |_| { ctx_menu.set(None); config::set_project_group(&rr, &gn); },
                                        if is_current { "✓ {gn}" } else { "　{gn}" }
                                    }
                                }
                            })}
                            if current_group.is_some() {
                                {
                                    let rr = r1.clone();
                                    rsx! {
                                        div { class: "ctx-menu-item",
                                            onclick: move |_| { ctx_menu.set(None); config::set_project_group(&rr, ""); },
                                            "　取消分组"
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "ctx-menu-separator" }
                        div { class: "ctx-menu-item",
                            onclick: move |_| { ctx_menu.set(None); let _ = std::process::Command::new("open").arg(&r2).spawn(); },
                            "📂  在 Finder 中打开"
                        }
                        div { class: "ctx-menu-item",
                            onclick: move |_| { ctx_menu.set(None); let _ = std::process::Command::new("bash").args(["-c", &format!("echo -n '{}' | pbcopy", r3)]).spawn(); },
                            "📋  复制路径"
                        }
                    }
                }
            }
        }
    }
}
