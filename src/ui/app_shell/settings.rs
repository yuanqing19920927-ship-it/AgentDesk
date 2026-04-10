use dioxus::prelude::*;
use chrono::Local;
use crate::models::{NotificationEventType, NotificationLevel};
use crate::services::{config, notifier, project_manager};
use crate::ui::icons;

#[component]
pub fn SettingsPanel(
    on_close: EventHandler<()>,
    on_refresh: EventHandler<()>,
) -> Element {
    let mut scan_dirs = use_signal(|| config::load_config().scan_dirs);
    let mut groups = use_signal(|| config::load_config().groups);
    let mut new_group_name = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);
    let mut group_error = use_signal(|| None::<String>);
    let mut notification_rules = use_signal(notifier::load_rules);
    let mut notification_history = use_signal(notifier::load_history);
    let mut notification_error = use_signal(|| None::<String>);

    rsx! {
        div {
            // ── Hero ──
            div { class: "page-hero",
                div { class: "icon-tile icon-tile-lg tile-graphite", dangerous_inner_html: icons::GEAR }
                div { class: "hero-title", "设置" }
                div { class: "hero-desc", "管理扫描目录、项目分组、通知规则与勿扰时段。" }
            }

            div { class: "hero-toolbar",
                button { class: "btn btn-ghost", onclick: move |_| on_close.call(()), "返回" }
            }

            // ── Scan directories ──
            div { class: "section",
                div { class: "section-label", "扫描目录" }
                p { style: "font-size: 12px; color: #86868b; margin-bottom: 10px;",
                    "AgentDesk 会扫描以下目录中的 Agent 会话数据来自动发现项目"
                }
                div { class: "grouped-card",
                    {scan_dirs().iter().map(|dir| {
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
                                    button { class: "btn-remove", onclick: move |_| {
                                        config::remove_scan_dir(&dr);
                                        scan_dirs.set(config::load_config().scan_dirs);
                                        on_refresh.call(());
                                    }, "移除" }
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
                if let Some(ref e) = error_msg() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-top: 6px;", "{e}" }
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
                                    let pd = p.clone(); let pr = p.clone();
                                    let name = std::path::Path::new(p).file_name()
                                        .map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                    rsx! {
                                        div { class: "grouped-row",
                                            div { class: "row-content",
                                                div { class: "row-label-bold", "{name}" }
                                                div { class: "row-sub", "{pd}" }
                                            }
                                            button { class: "btn-remove", onclick: move |_| {
                                                project_manager::remove_custom_project(&pr);
                                                on_refresh.call(());
                                            }, "移除" }
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

            // ── Groups with reorder ──
            div { class: "section",
                div { class: "section-label", "项目分组" }
                p { style: "font-size: 12px; color: #86868b; margin-bottom: 10px;",
                    "创建分组后，可通过右键菜单将项目分配到对应分组。使用箭头调整顺序。"
                }
                div { class: "grouped-card",
                    {groups().iter().enumerate().map(|(gi, g)| {
                        let gn = g.name.clone();
                        let gr = g.name.clone();
                        let total = groups().len();
                        let is_first = gi == 0;
                        let is_last = gi == total - 1;
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "row-label-bold", style: "flex: 1;", "{gn}" }
                                div { class: "group-actions",
                                    if !is_first {
                                        button { class: "btn-reorder",
                                            onclick: move |_| {
                                                let mut cfg = config::load_config();
                                                if gi > 0 { cfg.groups.swap(gi, gi - 1); }
                                                config::save_config(&cfg);
                                                groups.set(cfg.groups);
                                            },
                                            "↑"
                                        }
                                    }
                                    if !is_last {
                                        button { class: "btn-reorder",
                                            onclick: move |_| {
                                                let mut cfg = config::load_config();
                                                if gi + 1 < cfg.groups.len() { cfg.groups.swap(gi, gi + 1); }
                                                config::save_config(&cfg);
                                                groups.set(cfg.groups);
                                            },
                                            "↓"
                                        }
                                    }
                                    button { class: "btn-remove", onclick: move |_| {
                                        config::remove_group(&gr);
                                        groups.set(config::load_config().groups);
                                    }, "删除" }
                                }
                            }
                        }
                    })}
                    // Add group
                    div { class: "grouped-row", style: "gap: 8px;",
                        input {
                            class: "group-input",
                            placeholder: "输入分组名称后回车",
                            value: "{new_group_name}",
                            oninput: move |e| { new_group_name.set(e.value()); group_error.set(None); },
                            onkeydown: move |e| {
                                if e.key() == Key::Enter {
                                    let name = new_group_name().clone();
                                    if !name.trim().is_empty() {
                                        match config::add_group(&name) {
                                            Ok(()) => {
                                                groups.set(config::load_config().groups);
                                                new_group_name.set(String::new());
                                                group_error.set(None);
                                            }
                                            Err(e) => group_error.set(Some(e)),
                                        }
                                    }
                                }
                            },
                        }
                        button { class: "btn btn-primary btn-sm",
                            onclick: move |_| {
                                let name = new_group_name().clone();
                                if !name.trim().is_empty() {
                                    match config::add_group(&name) {
                                        Ok(()) => {
                                            groups.set(config::load_config().groups);
                                            new_group_name.set(String::new());
                                            group_error.set(None);
                                        }
                                        Err(e) => group_error.set(Some(e)),
                                    }
                                }
                            },
                            "添加"
                        }
                    }
                }
                if let Some(ref e) = group_error() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-top: 6px;", "{e}" }
                }
            }

            // ── Notifications ──
            {
                let rules = notification_rules();
                let levels = [
                    NotificationLevel::All,
                    NotificationLevel::ErrorsOnly,
                    NotificationLevel::Mute,
                ];
                let current_level_idx = levels.iter().position(|l| *l == rules.global_level).unwrap_or(0);
                let quiet = rules.quiet_hours;
                let quiet_start_h = quiet.start_min / 60;
                let quiet_start_m = quiet.start_min % 60;
                let quiet_end_h = quiet.end_min / 60;
                let quiet_end_m = quiet.end_min % 60;
                let history = notification_history();
                let unread = history.iter().filter(|e| !e.read).count();
                let total = history.len();

                rsx! {
                    div { class: "section",
                        div { class: "section-label", "通知" }
                        div { class: "grouped-card",
                            div { class: "grouped-row",
                                div { class: "row-content",
                                    div { class: "row-label-bold", "全局通知级别" }
                                    div { class: "row-sub", "控制 Agent 完成、退出等事件的默认行为" }
                                }
                                select {
                                    class: "form-select",
                                    onchange: move |e| {
                                        if let Ok(i) = e.value().parse::<usize>() {
                                            let levels = [
                                                NotificationLevel::All,
                                                NotificationLevel::ErrorsOnly,
                                                NotificationLevel::Mute,
                                            ];
                                            if let Some(l) = levels.get(i) {
                                                let mut r = notification_rules();
                                                r.global_level = *l;
                                                match notifier::save_rules(&r) {
                                                    Ok(()) => {
                                                        notification_rules.set(r);
                                                        notification_error.set(None);
                                                    }
                                                    Err(err) => notification_error.set(Some(err)),
                                                }
                                            }
                                        }
                                    },
                                    for (i, l) in levels.iter().enumerate() {
                                        option { value: "{i}", selected: current_level_idx == i, "{l.label()}" }
                                    }
                                }
                            }
                            {NotificationEventType::all().iter().map(|t| {
                                let t = *t;
                                let enabled = rules.event_enabled(t);
                                rsx! {
                                    div { class: "grouped-row",
                                        div { class: "row-content",
                                            div { class: "row-label", "{t.label()}" }
                                        }
                                        label { style: "display: flex; align-items: center; gap: 6px; cursor: pointer;",
                                            input {
                                                r#type: "checkbox",
                                                checked: enabled,
                                                onchange: move |e| {
                                                    let mut r = notification_rules();
                                                    r.set_event_enabled(t, e.checked());
                                                    match notifier::save_rules(&r) {
                                                        Ok(()) => {
                                                            notification_rules.set(r);
                                                            notification_error.set(None);
                                                        }
                                                        Err(err) => notification_error.set(Some(err)),
                                                    }
                                                },
                                            }
                                            span { style: "font-size: 12px; color: #86868b;", if enabled { "启用" } else { "禁用" } }
                                        }
                                    }
                                }
                            })}
                            div { class: "grouped-row",
                                div { class: "row-content",
                                    div { class: "row-label-bold", "勿扰时段" }
                                    div { class: "row-sub",
                                        "{quiet_start_h:02}:{quiet_start_m:02} — {quiet_end_h:02}:{quiet_end_m:02}（时段内仅错误可通过）"
                                    }
                                }
                                label { style: "display: flex; align-items: center; gap: 6px; cursor: pointer;",
                                    input {
                                        r#type: "checkbox",
                                        checked: quiet.enabled,
                                        onchange: move |e| {
                                            let mut r = notification_rules();
                                            r.quiet_hours.enabled = e.checked();
                                            if r.quiet_hours.start_min == r.quiet_hours.end_min {
                                                r.quiet_hours.start_min = 22 * 60;
                                                r.quiet_hours.end_min = 8 * 60;
                                            }
                                            match notifier::save_rules(&r) {
                                                Ok(()) => {
                                                    notification_rules.set(r);
                                                    notification_error.set(None);
                                                }
                                                Err(err) => notification_error.set(Some(err)),
                                            }
                                        },
                                    }
                                    span { style: "font-size: 12px; color: #86868b;", if quiet.enabled { "启用" } else { "禁用" } }
                                }
                            }
                        }

                        if let Some(err) = notification_error() {
                            div { style: "color: #ff3b30; font-size: 12px; margin-top: 6px;", "{err}" }
                        }

                        div { class: "section-label", style: "margin-top: 14px;",
                            "通知历史 ({total})"
                            if unread > 0 { span { style: "margin-left: 8px; color: #ff3b30;", "· {unread} 条未读" } }
                        }
                        div { class: "grouped-card",
                            if history.is_empty() {
                                div { class: "grouped-row",
                                    div { class: "row-label", style: "color: #86868b;", "暂无通知" }
                                }
                            } else {
                                {history.iter().rev().take(20).map(|ev| {
                                    let ts = ev.timestamp.with_timezone(&Local).format("%m-%d %H:%M").to_string();
                                    let type_label = ev.event_type.label();
                                    let title = ev.title.clone();
                                    let message = ev.message.clone();
                                    let suppressed = ev.suppressed;
                                    rsx! {
                                        div { class: "grouped-row",
                                            div { class: "row-content",
                                                div { style: "display: flex; gap: 8px; align-items: center;",
                                                    span { class: "row-label-bold", "{title}" }
                                                    span { class: "nick-badge", "{type_label}" }
                                                    if suppressed { span { class: "custom-badge", "已过滤" } }
                                                }
                                                div { class: "row-sub", "{ts} · {message}" }
                                            }
                                        }
                                    }
                                })}
                            }
                        }
                        if !history.is_empty() {
                            div { style: "display: flex; gap: 8px; margin-top: 8px;",
                                button {
                                    class: "btn-ghost",
                                    style: "font-size: 12px; padding: 4px 10px;",
                                    onclick: move |_| {
                                        notifier::mark_all_read();
                                        notification_history.set(notifier::load_history());
                                    },
                                    "全部标记已读"
                                }
                                button {
                                    class: "btn-remove",
                                    style: "font-size: 12px;",
                                    onclick: move |_| {
                                        let _ = notifier::clear_history();
                                        notification_history.set(notifier::load_history());
                                    },
                                    "清空历史"
                                }
                            }
                        }
                    }
                }
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
