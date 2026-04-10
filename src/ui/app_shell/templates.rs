use dioxus::prelude::*;
use crate::models::{AgentTemplate, AgentType, PermissionMode};
use crate::services::template_manager;
use crate::ui::icons;

/// Template management panel. Lets the user create, edit and delete agent
/// templates stored under `~/.agentdesk/templates/`.
#[component]
pub fn TemplatesPanel(on_close: EventHandler<()>) -> Element {
    let mut templates = use_signal(template_manager::load_all);
    let mut editing = use_signal(|| None::<AgentTemplate>);
    let mut error_msg = use_signal(|| None::<String>);

    let mut reload = move || {
        templates.set(template_manager::load_all());
    };

    rsx! {
        div {
            // ── Hero ──
            div { class: "page-hero",
                div { class: "icon-tile icon-tile-lg tile-indigo", dangerous_inner_html: icons::DOC_STACK }
                div { class: "hero-title", "模板" }
                div { class: "hero-desc",
                    "保存常用的 Agent 启动配置。新建 Agent 时可从模板一键预填；初始 prompt 会在启动后复制到剪贴板，粘贴即可使用。"
                }
            }

            div { class: "hero-toolbar",
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        editing.set(Some(AgentTemplate::new(
                            String::new(),
                            AgentType::ClaudeCode,
                            PermissionMode::Default,
                        )));
                        error_msg.set(None);
                    },
                    "＋ 新建模板"
                }
                button {
                    class: "btn btn-ghost",
                    onclick: move |_| on_close.call(()),
                    "返回"
                }
            }

            if let Some(err) = error_msg() {
                div { style: "color: #ff3b30; font-size: 12px; margin-bottom: 12px;", "{err}" }
            }

            // ── Template list ──
            div { class: "section",
                div { class: "section-label", "模板列表" }
                div { class: "grouped-card",
                    if templates().is_empty() {
                        div { class: "grouped-row",
                            div { class: "row-label", style: "color: #86868b;",
                                "暂无模板 — 点击右上角「新建模板」开始"
                            }
                        }
                    }
                    {templates().iter().map(|t| {
                        let tpl = t.clone();
                        let tpl_edit = t.clone();
                        let tpl_id = t.id.clone();
                        let (tile_cls, glyph) = match tpl.agent_type {
                            AgentType::ClaudeCode => ("icon-tile icon-tile-md tile-orange", icons::LAYERS),
                            AgentType::Codex => ("icon-tile icon-tile-md tile-teal", icons::LAYERS),
                        };
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "{tile_cls}", dangerous_inner_html: glyph }
                                div { class: "row-content",
                                    div { class: "row-label-bold", "{tpl.name}" }
                                    div { class: "row-sub",
                                        span { "{tpl.agent_type.label()}" }
                                        " · "
                                        span { "{tpl.permission_mode.label()}" }
                                        if tpl.initial_prompt.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false) {
                                            " · "
                                            span { "含初始 prompt" }
                                        }
                                    }
                                }
                                button {
                                    class: "btn-ghost",
                                    style: "font-size: 12px; padding: 4px 10px;",
                                    onclick: move |_| {
                                        editing.set(Some(tpl_edit.clone()));
                                        error_msg.set(None);
                                    },
                                    "编辑"
                                }
                                button {
                                    class: "btn-remove",
                                    onclick: move |_| {
                                        if let Err(e) = template_manager::delete(&tpl_id) {
                                            error_msg.set(Some(e));
                                        } else {
                                            reload();
                                        }
                                    },
                                    "删除"
                                }
                            }
                        }
                    })}
                }
            }
        }

        // ── Edit dialog ──
        if let Some(current) = editing() {
            TemplateEditor {
                initial: current,
                on_save: move |saved: AgentTemplate| {
                    match template_manager::save(&saved) {
                        Ok(()) => {
                            editing.set(None);
                            reload();
                        }
                        Err(e) => error_msg.set(Some(e)),
                    }
                },
                on_cancel: move |_| editing.set(None),
            }
        }
    }
}

/// Inline editor dialog for an `AgentTemplate`.
#[component]
fn TemplateEditor(
    initial: AgentTemplate,
    on_save: EventHandler<AgentTemplate>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut name = use_signal(|| initial.name.clone());
    let mut agent_type_idx = use_signal(|| match initial.agent_type {
        AgentType::ClaudeCode => 0usize,
        AgentType::Codex => 1,
    });
    let mut permission_idx = use_signal(|| match initial.permission_mode {
        PermissionMode::Default => 0usize,
        PermissionMode::DangerouslySkipPermissions => 1,
        PermissionMode::Plan => 2,
    });
    let mut prompt = use_signal(|| initial.initial_prompt.clone().unwrap_or_default());
    let mut local_error = use_signal(|| None::<String>);

    let agent_types = [AgentType::ClaudeCode, AgentType::Codex];
    let permissions = [
        PermissionMode::Default,
        PermissionMode::DangerouslySkipPermissions,
        PermissionMode::Plan,
    ];

    rsx! {
        div { class: "dialog-overlay",
            onclick: move |_| on_cancel.call(()),
            div { class: "dialog",
                style: "max-width: 520px;",
                onclick: move |e| e.stop_propagation(),
                h2 { "编辑模板" }

                div { class: "form-group",
                    label { "名称" }
                    input {
                        class: "form-select",
                        style: "width: 100%; padding: 6px 8px;",
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                    }
                }

                div { class: "form-group",
                    label { "Agent 类型" }
                    select {
                        class: "form-select",
                        onchange: move |e| {
                            if let Ok(i) = e.value().parse::<usize>() {
                                agent_type_idx.set(i);
                            }
                        },
                        for (i, at) in agent_types.iter().enumerate() {
                            option { value: "{i}", selected: agent_type_idx() == i, "{at.label()}" }
                        }
                    }
                }

                div { class: "form-group",
                    label { "权限模式" }
                    select {
                        class: "form-select",
                        onchange: move |e| {
                            if let Ok(i) = e.value().parse::<usize>() {
                                permission_idx.set(i);
                            }
                        },
                        for (i, pm) in permissions.iter().enumerate() {
                            option { value: "{i}", selected: permission_idx() == i, "{pm.label()}" }
                        }
                    }
                }

                div { class: "form-group",
                    label { "初始 Prompt（可选，启动后复制到剪贴板）" }
                    textarea {
                        class: "form-select",
                        style: "width: 100%; min-height: 100px; padding: 8px; font-family: inherit;",
                        value: "{prompt}",
                        oninput: move |e| prompt.set(e.value()),
                    }
                }

                if let Some(err) = local_error() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-bottom: 12px;", "{err}" }
                }

                div { class: "dialog-actions",
                    button { class: "btn-ghost", onclick: move |_| on_cancel.call(()), "取消" }
                    button {
                        class: "btn btn-primary",
                        onclick: {
                            let initial = initial.clone();
                            let agent_types = agent_types.clone();
                            let permissions = permissions.clone();
                            move |_| {
                                let trimmed = name().trim().to_string();
                                if trimmed.is_empty() {
                                    local_error.set(Some("请填写模板名称".to_string()));
                                    return;
                                }
                                let prompt_val = prompt();
                                let prompt_opt = if prompt_val.trim().is_empty() {
                                    None
                                } else {
                                    Some(prompt_val)
                                };
                                let saved = AgentTemplate {
                                    id: initial.id.clone(),
                                    name: trimmed,
                                    agent_type: agent_types[agent_type_idx()].clone(),
                                    permission_mode: permissions[permission_idx()].clone(),
                                    model: initial.model.clone(),
                                    initial_prompt: prompt_opt,
                                    tags: initial.tags.clone(),
                                };
                                on_save.call(saved);
                            }
                        },
                        "保存"
                    }
                }
            }
        }
    }
}
