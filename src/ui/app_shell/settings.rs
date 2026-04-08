use dioxus::prelude::*;
use crate::services::{config, project_manager};

#[component]
pub fn SettingsPanel(
    on_close: EventHandler<()>,
    on_refresh: EventHandler<()>,
) -> Element {
    let mut scan_dirs = use_signal(|| config::load_config().scan_dirs);
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        div {
            // ── Header ──
            div { class: "page-header",
                div { class: "page-header-info",
                    h1 { "设置" }
                }
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
                        let dir_display = dir.clone();
                        let dir_for_remove = dir.clone();
                        let is_default = dir.contains(".claude/projects");
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "row-content",
                                    div { class: "row-label-bold", "{dir_display}" }
                                    if is_default {
                                        div { class: "row-sub", "默认 — Claude Code 会话目录" }
                                    }
                                }
                                if !is_default {
                                    button {
                                        class: "btn-remove",
                                        onclick: move |_| {
                                            config::remove_scan_dir(&dir_for_remove);
                                            scan_dirs.set(config::load_config().scan_dirs);
                                            on_refresh.call(());
                                        },
                                        "移除"
                                    }
                                }
                            }
                        }
                    })}
                    // Add button row
                    div { class: "grouped-row grouped-row-clickable",
                        onclick: move |_| {
                            spawn(async move {
                                let result = tokio::task::spawn_blocking(|| {
                                    project_manager::pick_folder()
                                }).await;
                                match result {
                                    Ok(Some(path)) => {
                                        match config::add_scan_dir(&path) {
                                            Ok(()) => {
                                                scan_dirs.set(config::load_config().scan_dirs);
                                                error_msg.set(None);
                                                on_refresh.call(());
                                            }
                                            Err(e) => error_msg.set(Some(e)),
                                        }
                                    }
                                    _ => {} // cancelled
                                }
                            });
                        },
                        div { style: "color: #007aff; font-size: 13px; font-weight: 500;", "＋ 添加扫描目录" }
                    }
                }

                if let Some(ref err) = error_msg() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-top: 8px;", "{err}" }
                }
            }

            // ── Info ──
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
