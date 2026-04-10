use dioxus::prelude::*;
use chrono::Local;
use crate::models::{MemoryEntry, Project};
use crate::services::{approved_projects, memory_indexer};

/// Memory panel embedded inside the project dashboard. Lets the user
/// enable/disable project-local memory, trigger a manual scan, and
/// browse the most recent memory entries.
#[component]
pub fn MemoryView(project: Project) -> Element {
    let project_for_report = project.clone();
    let mut report = use_signal(move || memory_indexer::read_report(&project_for_report.root));
    let mut entries = use_signal(|| {
        let p = project.clone();
        memory_indexer::load_entries(&p.root)
    });
    let mut approved = use_signal({
        let p = project.clone();
        move || approved_projects::is_approved(&p.root)
    });
    let mut scanning = use_signal(|| false);
    let mut error_msg = use_signal(|| None::<String>);
    let mut last_scan_msg = use_signal(|| None::<String>);

    let project_clone = project.clone();

    let toggle_approval = {
        let project = project_clone.clone();
        move |_| {
            let p = project.root.clone();
            let currently = approved();
            let result = if currently {
                approved_projects::revoke(&p)
            } else {
                approved_projects::approve(&p)
            };
            match result {
                Ok(()) => {
                    error_msg.set(None);
                    report.set(memory_indexer::read_report(&project.root));
                    entries.set(memory_indexer::load_entries(&project.root));
                    approved.set(approved_projects::is_approved(&project.root));

                    // On enabling: kick off a scan immediately so the user sees
                    // memory populate right away instead of a stale "0 条" card.
                    if !currently && !scanning() {
                        scanning.set(true);
                        last_scan_msg.set(None);
                        let project_root = project.root.clone();
                        let project_root_refresh = project_root.clone();
                        let claude_dirs = project.claude_dir_names.clone();
                        spawn(async move {
                            let result = tokio::task::spawn_blocking(move || {
                                memory_indexer::scan_project(&project_root, &claude_dirs)
                            })
                            .await;
                            match result {
                                Ok(Ok(r)) => {
                                    let mode_label = match r.mode {
                                        memory_indexer::StorageMode::ProjectLocal => "项目内 .agentdesk/",
                                        memory_indexer::StorageMode::UserFallback => "用户级回退存储",
                                    };
                                    last_scan_msg.set(Some(format!(
                                        "扫描完成：{} 条新条目（共 {} 条），存储在 {}",
                                        r.new_entries, r.total_entries, mode_label
                                    )));
                                }
                                Ok(Err(e)) => error_msg.set(Some(e)),
                                Err(e) => error_msg.set(Some(format!("任务错误: {}", e))),
                            }
                            scanning.set(false);
                            report.set(memory_indexer::read_report(&project_root_refresh));
                            entries.set(memory_indexer::load_entries(&project_root_refresh));
                        });
                    }
                }
                Err(e) => error_msg.set(Some(e)),
            }
        }
    };

    let start_scan = {
        let project = project_clone.clone();
        move |_| {
            if scanning() {
                return;
            }
            scanning.set(true);
            error_msg.set(None);
            last_scan_msg.set(None);
            let project_root = project.root.clone();
            let project_root_refresh = project_root.clone();
            let claude_dirs = project.claude_dir_names.clone();
            spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    memory_indexer::scan_project(&project_root, &claude_dirs)
                })
                .await;
                match result {
                    Ok(Ok(r)) => {
                        let mode_label = match r.mode {
                            memory_indexer::StorageMode::ProjectLocal => "项目内 .agentdesk/",
                            memory_indexer::StorageMode::UserFallback => "用户级回退存储",
                        };
                        last_scan_msg.set(Some(format!(
                            "扫描完成：{} 条新条目（共 {} 条），存储在 {}",
                            r.new_entries, r.total_entries, mode_label
                        )));
                    }
                    Ok(Err(e)) => error_msg.set(Some(e)),
                    Err(e) => error_msg.set(Some(format!("任务错误: {}", e))),
                }
                scanning.set(false);
                report.set(memory_indexer::read_report(&project_root_refresh));
                entries.set(memory_indexer::load_entries(&project_root_refresh));
                approved.set(approved_projects::is_approved(&project_root_refresh));
            });
        }
    };

    let open_memory_md = {
        let project = project_clone.clone();
        move |_| {
            let storage = match memory_indexer::read_report(&project.root) {
                Some(r) => r.storage_path,
                None => return,
            };
            let path = storage.join("memory.md");
            if path.exists() {
                let _ = std::process::Command::new("open").arg(&path).spawn();
            }
        }
    };

    let current_report = report();
    let (mode_text, storage_text, total_text) = match &current_report {
        Some(r) => (
            match r.mode {
                memory_indexer::StorageMode::ProjectLocal => "项目本地".to_string(),
                memory_indexer::StorageMode::UserFallback => "用户级回退".to_string(),
            },
            r.storage_path.display().to_string(),
            format!("{} 条", r.total_entries),
        ),
        None => ("不可用".to_string(), "—".to_string(), "0 条".to_string()),
    };

    rsx! {
        div { class: "section",
            div { class: "section-label", "项目记忆" }
            div { class: "grouped-card", style: "margin-bottom: 10px;",
                div { class: "grouped-row",
                    div { class: "row-content",
                        div { class: "row-label-bold",
                            "模式：{mode_text}"
                            " · 条目：{total_text}"
                        }
                        div { class: "row-sub", "{storage_text}" }
                    }
                    div { style: "display: flex; gap: 6px;",
                        button {
                            class: if approved() { "btn-ghost" } else { "btn btn-primary" },
                            style: "font-size: 12px; padding: 4px 10px;",
                            onclick: toggle_approval,
                            if approved() { "禁用记忆" } else { "启用记忆" }
                        }
                        button {
                            class: "btn btn-primary",
                            style: "font-size: 12px; padding: 4px 10px;",
                            disabled: scanning(),
                            onclick: start_scan,
                            if scanning() { "扫描中..." } else { "立即扫描" }
                        }
                        button {
                            class: "btn-ghost",
                            style: "font-size: 12px; padding: 4px 10px;",
                            disabled: current_report.is_none(),
                            onclick: open_memory_md,
                            "打开 memory.md"
                        }
                    }
                }
                if !approved() {
                    div { class: "grouped-row",
                        div { class: "row-sub",
                            "未启用时记忆写入 ~/.agentdesk/projects/{{path_hash}}/，不会修改项目目录。启用后会在项目根目录创建 .agentdesk/，并自动加入 .gitignore。"
                        }
                    }
                }
                if let Some(msg) = last_scan_msg() {
                    div { class: "grouped-row",
                        div { class: "row-sub", style: "color: #30a46c;", "{msg}" }
                    }
                }
                if let Some(err) = error_msg() {
                    div { class: "grouped-row",
                        div { class: "row-sub", style: "color: #ff3b30;", "{err}" }
                    }
                }
            }

            if !entries().is_empty() {
                div { class: "section-label", style: "margin-top: 14px;", "最近记忆条目" }
                div { class: "grouped-card",
                    {entries().iter().take(8).map(render_entry)}
                }
            }
        }
    }
}

fn render_entry(entry: &MemoryEntry) -> Element {
    let date = entry
        .timestamp
        .map(|t| t.with_timezone(&Local).format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".to_string());
    let branch = entry.branch.clone().unwrap_or_default();
    let has_branch = entry.branch.is_some();
    let first_line = entry
        .summary
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let keywords = entry.keywords.join(", ");
    let has_keywords = !entry.keywords.is_empty();

    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { style: "display: flex; gap: 8px; align-items: center;",
                    span { class: "row-label-bold", "{date}" }
                    if has_branch { span { class: "nick-badge", "{branch}" } }
                }
                div { class: "row-sub", "{first_line}" }
                if has_keywords {
                    div { class: "row-sub", style: "color: #007aff;", "{keywords}" }
                }
            }
        }
    }
}
