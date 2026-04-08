use dioxus::prelude::*;
use crate::services::{config, project_manager};

#[component]
pub fn SettingsPanel(
    on_close: EventHandler<()>,
    on_refresh: EventHandler<()>,
) -> Element {
    let mut scan_dirs = use_signal(|| config::load_config().scan_dirs);
    let mut groups = use_signal(|| config::load_config().groups);
    let mut new_group_name = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        div {
            div { class: "page-header",
                div { class: "page-header-info", h1 { "设置" } }
                div { class: "page-header-actions",
                    button { class: "btn btn-ghost", onclick: move |_| on_close.call(()), "返回" }
                }
            }

            // ── Scan directories ──
            div { class: "section",
                div { class: "section-label", "扫描目录" }
                p { style: "font-size: 12px; color: #86868b; margin-bottom: 10px;",
                    "AgentDesk 会扫描以下目录中的 Agent 会话数据来自动发现项目"
                }
                div { class: "grouped-card",
                    {scan_dirs().iter().enumerate().map(|(i, dir)| {
                        let d = dir.clone();
                        let dr = dir.clone();
                        let is_default = dir.contains(".claude/projects");
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "row-content",
                                    div { class: "row-label-bold", "{d}" }
                                    if is_default { div { class: "row-sub", "默认 — Claude Code 会话目录" } }
                                }
                                if !is_default {
                                    button { class: "btn-remove",
                                        onclick: move |_| {
                                            config::remove_scan_dir(&dr);
                                            scan_dirs.set(config::load_config().scan_dirs);
                                            on_refresh.call(());
                                        },
                                        "移除"
                                    }
                                }
                            }
                        }
                    })}
                    div { class: "grouped-row grouped-row-clickable",
                        onclick: move |_| {
                            spawn(async move {
                                let r = tokio::task::spawn_blocking(|| project_manager::pick_folder()).await;
                                if let Ok(Some(path)) = r {
                                    match config::add_scan_dir(&path) {
                                        Ok(()) => { scan_dirs.set(config::load_config().scan_dirs); on_refresh.call(()); }
                                        Err(e) => error_msg.set(Some(e)),
                                    }
                                }
                            });
                        },
                        div { style: "color: #007aff; font-size: 13px; font-weight: 500;", "＋ 添加扫描目录" }
                    }
                }
            }

            // ── Manual projects ──
            div { class: "section",
                div { class: "section-label", "手动添加的项目" }
                {
                    let custom = project_manager::load_custom_projects();
                    rsx! {
                        div { class: "grouped-card",
                            if custom.is_empty() {
                                div { class: "grouped-row",
                                    div { class: "row-label", style: "color: #86868b;", "暂无手动添加的项目" }
                                }
                            } else {
                                {custom.iter().map(|p| {
                                    let pd = p.clone();
                                    let pr = p.clone();
                                    let name = std::path::Path::new(p).file_name()
                                        .map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                    rsx! {
                                        div { class: "grouped-row",
                                            div { class: "row-content",
                                                div { class: "row-label-bold", "{name}" }
                                                div { class: "row-sub", "{pd}" }
                                            }
                                            button { class: "btn-remove",
                                                onclick: move |_| {
                                                    project_manager::remove_custom_project(&pr);
                                                    on_refresh.call(());
                                                },
                                                "移除"
                                            }
                                        }
                                    }
                                })}
                            }
                            div { class: "grouped-row grouped-row-clickable",
                                onclick: move |_| {
                                    spawn(async move {
                                        let r = tokio::task::spawn_blocking(|| project_manager::pick_folder()).await;
                                        if let Ok(Some(path)) = r {
                                            match project_manager::add_custom_project(&path) {
                                                Ok(()) => on_refresh.call(()),
                                                Err(e) => error_msg.set(Some(e)),
                                            }
                                        }
                                    });
                                },
                                div { style: "color: #007aff; font-size: 13px; font-weight: 500;", "＋ 添加项目" }
                            }
                        }
                    }
                }
            }

            // ── Groups ──
            div { class: "section",
                div { class: "section-label", "项目分组" }
                p { style: "font-size: 12px; color: #86868b; margin-bottom: 10px;",
                    "创建分组后，可通过右键菜单将项目分配到对应分组"
                }
                div { class: "grouped-card",
                    {groups().iter().map(|g| {
                        let gn = g.name.clone();
                        let gr = g.name.clone();
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "row-label-bold", "{gn}" }
                                button { class: "btn-remove",
                                    onclick: move |_| {
                                        config::remove_group(&gr);
                                        groups.set(config::load_config().groups);
                                    },
                                    "删除"
                                }
                            }
                        }
                    })}
                    // Add group inline
                    div { class: "grouped-row",
                        input {
                            class: "group-input",
                            placeholder: "输入分组名称",
                            value: "{new_group_name}",
                            oninput: move |e| new_group_name.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter {
                                    let name = new_group_name().clone();
                                    if !name.trim().is_empty() {
                                        let _ = config::add_group(&name);
                                        groups.set(config::load_config().groups);
                                        new_group_name.set(String::new());
                                    }
                                }
                            },
                        }
                        button { class: "btn btn-primary btn-sm",
                            onclick: move |_| {
                                let name = new_group_name().clone();
                                if !name.trim().is_empty() {
                                    let _ = config::add_group(&name);
                                    groups.set(config::load_config().groups);
                                    new_group_name.set(String::new());
                                }
                            },
                            "添加"
                        }
                    }
                }
            }

            if let Some(ref err) = error_msg() {
                div { style: "color: #ff3b30; font-size: 12px; margin-top: 8px;", "{err}" }
            }

            // ── About ──
            div { class: "section",
                div { class: "section-label", "关于" }
                div { class: "grouped-card",
                    div { class: "grouped-row",
                        div { class: "row-label", "版本" }
                        div { class: "row-value", "0.1.0" }
                    }
                    div { class: "grouped-row",
                        div { class: "row-label", "配置目录" }
                        div { class: "row-value", "~/.agentdesk/" }
                    }
                }
            }
        }
    }
}
