//! Module 8 补完 — in-app notification center.
//!
//! A floating panel anchored to the sidebar bell. Lists the notification
//! ring buffer in reverse-chronological order with three filter tabs
//! (全部 / 未读 / 错误), per-row mark-read and delete actions, plus a
//! "跳转到项目" link when the event is project-scoped.

use crate::models::{NotificationEvent, NotificationEventType};
use crate::services::notifier;
use chrono::{DateTime, Local, Utc};
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Unread,
    Errors,
}

impl Filter {
    fn label(&self) -> &'static str {
        match self {
            Filter::All => "全部",
            Filter::Unread => "未读",
            Filter::Errors => "错误",
        }
    }
}

fn event_row_class(e: &NotificationEvent) -> &'static str {
    if e.event_type.is_error() {
        if e.read {
            "notif-row notif-row-error notif-row-read"
        } else {
            "notif-row notif-row-error"
        }
    } else if e.read {
        "notif-row notif-row-read"
    } else {
        "notif-row"
    }
}

fn fmt_local(ts: DateTime<Utc>) -> String {
    ts.with_timezone(&Local).format("%m-%d %H:%M:%S").to_string()
}

#[component]
pub fn NotificationCenter(
    on_close: EventHandler<()>,
    on_jump_project: EventHandler<String>,
) -> Element {
    let mut events = use_signal(notifier::load_history);
    let mut filter = use_signal(|| Filter::All);

    // Order newest-first and apply filter.
    let visible: Vec<NotificationEvent> = {
        let mut list: Vec<NotificationEvent> = events().clone();
        list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        let f = filter();
        list.into_iter()
            .filter(|e| match f {
                Filter::All => true,
                Filter::Unread => !e.read,
                Filter::Errors => e.event_type.is_error(),
            })
            .collect()
    };

    let unread_total = events().iter().filter(|e| !e.read).count();
    let error_total = events()
        .iter()
        .filter(|e| e.event_type.is_error())
        .count();
    let total = events().len();

    rsx! {
        div { class: "notif-backdrop", onclick: move |_| on_close.call(()) }
        div {
            class: "notif-panel",
            onclick: move |e| e.stop_propagation(),
            div { class: "notif-header",
                div { class: "notif-title", "通知中心" }
                div { class: "notif-header-actions",
                    button {
                        class: "btn-ghost btn-xs",
                        onclick: move |_| {
                            notifier::mark_all_read();
                            events.set(notifier::load_history());
                        },
                        "全部已读"
                    }
                    button {
                        class: "btn-ghost btn-xs",
                        onclick: move |_| {
                            let _ = notifier::clear_history();
                            events.set(notifier::load_history());
                        },
                        "清空"
                    }
                    button {
                        class: "btn-ghost btn-xs",
                        onclick: move |_| on_close.call(()),
                        "关闭"
                    }
                }
            }

            div { class: "notif-tabs",
                {
                    [Filter::All, Filter::Unread, Filter::Errors].iter().map(|f| {
                        let f = *f;
                        let count = match f {
                            Filter::All => total,
                            Filter::Unread => unread_total,
                            Filter::Errors => error_total,
                        };
                        let is_active = filter() == f;
                        let cls = if is_active { "notif-tab notif-tab-active" } else { "notif-tab" };
                        rsx! {
                            div {
                                class: "{cls}",
                                onclick: move |_| filter.set(f),
                                "{f.label()} ({count})"
                            }
                        }
                    })
                }
            }

            div { class: "notif-body",
                if visible.is_empty() {
                    div { class: "notif-empty", "没有通知" }
                } else {
                    for ev in visible.iter() {
                        {
                            let ts = ev.timestamp;
                            let row_cls = event_row_class(ev);
                            let title = ev.title.clone();
                            let message = ev.message.clone();
                            let type_label = ev.event_type.label();
                            let time_str = fmt_local(ev.timestamp);
                            let is_read = ev.read;
                            let project_root = ev.project_root.clone();
                            let is_error = ev.event_type.is_error();
                            let badge_cls = if is_error { "notif-badge notif-badge-error" } else { "notif-badge" };
                            rsx! {
                                div { class: "{row_cls}",
                                    div { class: "notif-row-main",
                                        div { class: "notif-row-head",
                                            if !is_read { div { class: "notif-dot" } }
                                            span { class: "notif-row-title", "{title}" }
                                            span { class: "{badge_cls}", "{type_label}" }
                                            span { class: "notif-row-time", "{time_str}" }
                                        }
                                        div { class: "notif-row-msg", "{message}" }
                                        if project_root.is_some() {
                                            {
                                                let pr = project_root.clone().unwrap();
                                                let display = pr.clone();
                                                rsx! {
                                                    div { class: "notif-row-foot",
                                                        span { class: "notif-row-path", "{display}" }
                                                        button {
                                                            class: "btn-link",
                                                            onclick: move |_| {
                                                                on_jump_project.call(pr.clone());
                                                            },
                                                            "跳转到项目"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    div { class: "notif-row-actions",
                                        if !is_read {
                                            button {
                                                class: "btn-ghost btn-xs",
                                                onclick: move |_| {
                                                    notifier::mark_read(ts);
                                                    events.set(notifier::load_history());
                                                },
                                                "已读"
                                            }
                                        }
                                        button {
                                            class: "btn-ghost btn-xs",
                                            onclick: move |_| {
                                                notifier::delete_event(ts);
                                                events.set(notifier::load_history());
                                            },
                                            "删除"
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
}

// Silence unused-import warning if the enum label variants grow.
#[allow(dead_code)]
fn _all_event_types_referenced(t: NotificationEventType) -> &'static str {
    t.label()
}
