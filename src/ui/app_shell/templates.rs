use dioxus::prelude::*;
use crate::models::{AgentTemplate, AgentType, ComboItem, ComboPreset, PermissionMode};
use crate::services::{bundle_io, preset_manager, template_manager};
use crate::ui::icons;

/// Template management panel. Lets the user create, edit and delete agent
/// templates *and* multi-agent combo presets. Presets reference template
/// ids, so editing a template propagates to every combo that uses it.
#[component]
pub fn TemplatesPanel(on_close: EventHandler<()>) -> Element {
    let mut templates = use_signal(template_manager::load_all);
    let mut presets = use_signal(preset_manager::load_all);
    let mut editing_template = use_signal(|| None::<AgentTemplate>);
    let mut editing_preset = use_signal(|| None::<ComboPreset>);
    let mut error_msg = use_signal(|| None::<String>);
    // Transient status line shown below the hero toolbar — used for
    // import/export success notifications that should not block the UI.
    let mut status_msg = use_signal(|| None::<String>);

    let mut reload_templates = move || {
        templates.set(template_manager::load_all());
    };
    let mut reload_presets = move || {
        presets.set(preset_manager::load_all());
    };

    rsx! {
        div {
            // ── Hero ──
            div { class: "page-hero",
                div { class: "icon-tile icon-tile-lg tile-indigo", dangerous_inner_html: icons::DOC_STACK }
                div { class: "hero-title", "模板与组合" }
                div { class: "hero-desc",
                    "保存常用的 Agent 启动配置，或把多个模板打成组合一次性启动。初始 prompt 会在启动后复制到剪贴板，粘贴即可使用。"
                }
            }

            div { class: "hero-toolbar",
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        editing_template.set(Some(AgentTemplate::new(
                            String::new(),
                            AgentType::ClaudeCode,
                            PermissionMode::Default,
                        )));
                        error_msg.set(None);
                    },
                    "＋ 新建模板"
                }
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        editing_preset.set(Some(ComboPreset::new(String::new())));
                        error_msg.set(None);
                    },
                    "＋ 新建组合"
                }
                button {
                    class: "btn btn-ghost",
                    onclick: move |_| {
                        error_msg.set(None);
                        status_msg.set(None);
                        match bundle_io::import_bundle_with_dialog() {
                            Ok(Some(report)) => {
                                let tcount = report.templates_imported.len();
                                let pcount = report.presets_imported.len();
                                let mut msg = format!("导入完成：{} 个模板、{} 个组合", tcount, pcount);
                                if !report.warnings.is_empty() {
                                    msg.push_str(&format!("（{} 条警告）", report.warnings.len()));
                                }
                                status_msg.set(Some(msg));
                                templates.set(template_manager::load_all());
                                presets.set(preset_manager::load_all());
                            }
                            Ok(None) => { /* user cancelled */ }
                            Err(e) => error_msg.set(Some(e)),
                        }
                    },
                    "↓ 导入"
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
            if let Some(msg) = status_msg() {
                div {
                    style: "color: #1c5b17; font-size: 12px; margin-bottom: 12px; \
                            background: #e4f7df; border: 0.5px solid #9dd89a; \
                            padding: 6px 10px; border-radius: 6px; cursor: pointer;",
                    onclick: move |_| status_msg.set(None),
                    "{msg} · 点击关闭"
                }
            }

            // ── Template list ──
            div { class: "section",
                div { class: "section-label", "单体模板" }
                div { class: "grouped-card",
                    if templates().is_empty() {
                        div { class: "grouped-row",
                            div { class: "row-label", style: "color: #86868b;",
                                "暂无模板 — 点击右上角「新建模板」开始"
                            }
                        }
                    }
                    {templates().iter().map(|t| {
                        let tpl_edit = t.clone();
                        let tpl_export = t.clone();
                        let tpl_id = t.id.clone();
                        let (tile_cls, glyph) = match t.agent_type {
                            AgentType::ClaudeCode => ("icon-tile icon-tile-md tile-orange", icons::LAYERS),
                            AgentType::Codex => ("icon-tile icon-tile-md tile-teal", icons::LAYERS),
                        };
                        let name = t.name.clone();
                        let agent_label = t.agent_type.label();
                        let perm_label = t.permission_mode.label();
                        let has_prompt = t.initial_prompt.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "{tile_cls}", dangerous_inner_html: glyph }
                                div { class: "row-content",
                                    div { class: "row-label-bold", "{name}" }
                                    div { class: "row-sub",
                                        span { "{agent_label}" }
                                        " · "
                                        span { "{perm_label}" }
                                        if has_prompt {
                                            " · "
                                            span { "含初始 prompt" }
                                        }
                                    }
                                }
                                button {
                                    class: "btn-ghost",
                                    style: "font-size: 12px; padding: 4px 10px;",
                                    onclick: move |_| {
                                        editing_template.set(Some(tpl_edit.clone()));
                                        error_msg.set(None);
                                    },
                                    "编辑"
                                }
                                button {
                                    class: "btn-ghost",
                                    style: "font-size: 12px; padding: 4px 10px;",
                                    onclick: move |_| {
                                        error_msg.set(None);
                                        status_msg.set(None);
                                        let bundle = bundle_io::bundle_from_template(&tpl_export);
                                        let default_name = format!("agentdesk-template-{}.json", tpl_export.name);
                                        match bundle_io::export_bundle_with_dialog(&bundle, &default_name) {
                                            Ok(Some(path)) => status_msg.set(Some(format!("已导出到 {}", path.display()))),
                                            Ok(None) => { /* cancelled */ }
                                            Err(e) => error_msg.set(Some(e)),
                                        }
                                    },
                                    "导出"
                                }
                                button {
                                    class: "btn-remove",
                                    onclick: move |_| {
                                        if let Err(e) = template_manager::delete(&tpl_id) {
                                            error_msg.set(Some(e));
                                        } else {
                                            reload_templates();
                                        }
                                    },
                                    "删除"
                                }
                            }
                        }
                    })}
                }
            }

            // ── Combo preset list ──
            div { class: "section",
                div { class: "section-label", "组合预设" }
                div { class: "row-sub", style: "color: #86868b; margin-bottom: 6px;",
                    "组合是一串模板的引用。在项目 Dashboard 里点「启动组合」即可按顺序开启多个 Agent 窗口。"
                }
                div { class: "grouped-card",
                    if presets().is_empty() {
                        div { class: "grouped-row",
                            div { class: "row-label", style: "color: #86868b;",
                                "暂无组合 — 先建几个模板，再点「新建组合」挑选它们"
                            }
                        }
                    }
                    {presets().iter().map(|p| {
                        let preset_edit = p.clone();
                        let preset_export = p.clone();
                        let preset_id = p.id.clone();
                        let name = p.name.clone();
                        let item_count = p.items.len();
                        let desc = p.description.clone();
                        let has_desc = !desc.trim().is_empty();
                        rsx! {
                            div { class: "grouped-row",
                                div { class: "icon-tile icon-tile-md tile-purple", dangerous_inner_html: icons::LAYERS }
                                div { class: "row-content",
                                    div { class: "row-label-bold", "{name}" }
                                    div { class: "row-sub",
                                        span { "{item_count} 个模板" }
                                        if has_desc {
                                            " · "
                                            span { "{desc}" }
                                        }
                                    }
                                }
                                button {
                                    class: "btn-ghost",
                                    style: "font-size: 12px; padding: 4px 10px;",
                                    onclick: move |_| {
                                        editing_preset.set(Some(preset_edit.clone()));
                                        error_msg.set(None);
                                    },
                                    "编辑"
                                }
                                button {
                                    class: "btn-ghost",
                                    style: "font-size: 12px; padding: 4px 10px;",
                                    onclick: move |_| {
                                        error_msg.set(None);
                                        status_msg.set(None);
                                        // Re-read templates at export time so the
                                        // bundle captures the latest template
                                        // content, not the cached signal.
                                        let pool = template_manager::load_all();
                                        let bundle = bundle_io::bundle_from_preset(&preset_export, &pool);
                                        let default_name = format!("agentdesk-combo-{}.json", preset_export.name);
                                        match bundle_io::export_bundle_with_dialog(&bundle, &default_name) {
                                            Ok(Some(path)) => status_msg.set(Some(format!("已导出到 {}", path.display()))),
                                            Ok(None) => { /* cancelled */ }
                                            Err(e) => error_msg.set(Some(e)),
                                        }
                                    },
                                    "导出"
                                }
                                button {
                                    class: "btn-remove",
                                    onclick: move |_| {
                                        if let Err(e) = preset_manager::delete(&preset_id) {
                                            error_msg.set(Some(e));
                                        } else {
                                            reload_presets();
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

        // ── Template edit dialog ──
        if let Some(current) = editing_template() {
            TemplateEditor {
                initial: current,
                on_save: move |saved: AgentTemplate| {
                    match template_manager::save(&saved) {
                        Ok(()) => {
                            editing_template.set(None);
                            reload_templates();
                        }
                        Err(e) => error_msg.set(Some(e)),
                    }
                },
                on_cancel: move |_| editing_template.set(None),
            }
        }

        // ── Combo edit dialog ──
        if let Some(current) = editing_preset() {
            ComboPresetEditor {
                initial: current,
                templates: templates(),
                on_save: move |saved: ComboPreset| {
                    match preset_manager::save(&saved) {
                        Ok(()) => {
                            editing_preset.set(None);
                            reload_presets();
                        }
                        Err(e) => error_msg.set(Some(e)),
                    }
                },
                on_cancel: move |_| editing_preset.set(None),
            }
        }
    }
}

/// Inline editor dialog for an `AgentTemplate`. `pub(crate)` so the
/// Dashboard "另存为模板" flow in `dashboard.rs` can reuse it instead of
/// rebuilding a second form.
#[component]
pub(crate) fn TemplateEditor(
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

/// Editor for `ComboPreset`. Uses a two-list UX: available templates
/// on the left, selected combo items on the right with up/down/remove
/// controls. Order matters — `launch_preset` launches items in list
/// order, which tends to affect visual arrangement of terminal
/// windows on screen.
#[component]
fn ComboPresetEditor(
    initial: ComboPreset,
    templates: Vec<AgentTemplate>,
    on_save: EventHandler<ComboPreset>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut name = use_signal(|| initial.name.clone());
    let mut description = use_signal(|| initial.description.clone());
    let mut items = use_signal(|| initial.items.clone());
    let mut local_error = use_signal(|| None::<String>);

    // Build a quick lookup table so the selected-items list can show
    // the underlying template name even when the user set a custom
    // label.
    let templates_sig = use_signal(|| templates.clone());

    let templates_for_available = templates_sig.clone();
    let items_for_available = items.clone();

    rsx! {
        div { class: "dialog-overlay",
            onclick: move |_| on_cancel.call(()),
            div { class: "dialog combo-editor",
                onclick: move |e| e.stop_propagation(),
                h2 { "编辑组合预设" }

                div { class: "form-group",
                    label { "组合名称" }
                    input {
                        class: "form-select",
                        style: "width: 100%; padding: 6px 8px;",
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                    }
                }

                div { class: "form-group",
                    label { "描述（可选）" }
                    input {
                        class: "form-select",
                        style: "width: 100%; padding: 6px 8px;",
                        value: "{description}",
                        oninput: move |e| description.set(e.value()),
                    }
                }

                div { class: "combo-editor-split",
                    // ── Available templates ──
                    div { class: "combo-editor-col",
                        div { class: "combo-editor-col-title", "可选模板" }
                        div { class: "combo-editor-col-body",
                            {
                                let tpls = templates_for_available();
                                if tpls.is_empty() {
                                    rsx! {
                                        div { class: "row-sub", style: "color: #86868b; padding: 6px;",
                                            "没有可用模板 — 先到「单体模板」里新建几个"
                                        }
                                    }
                                } else {
                                    rsx! {
                                        {tpls.iter().map(|t| {
                                            let t_for_click = t.clone();
                                            let t_name = t.name.clone();
                                            let t_agent = t.agent_type.label();
                                            let is_in_combo = items_for_available().iter()
                                                .any(|i| i.template_id == t.id);
                                            let count_in_combo = items_for_available().iter()
                                                .filter(|i| i.template_id == t.id)
                                                .count();
                                            rsx! {
                                                div { class: "combo-editor-tpl-row",
                                                    div { class: "row-content",
                                                        div { class: "row-label-bold", "{t_name}" }
                                                        div { class: "row-sub",
                                                            "{t_agent}"
                                                            if is_in_combo {
                                                                " · "
                                                                span { style: "color: #007aff;", "已添加 ×{count_in_combo}" }
                                                            }
                                                        }
                                                    }
                                                    button {
                                                        class: "btn-ghost",
                                                        style: "font-size: 11px; padding: 4px 8px;",
                                                        onclick: move |_| {
                                                            let mut cur = items();
                                                            cur.push(ComboItem {
                                                                template_id: t_for_click.id.clone(),
                                                                label: None,
                                                            });
                                                            items.set(cur);
                                                        },
                                                        "＋ 加入"
                                                    }
                                                }
                                            }
                                        })}
                                    }
                                }
                            }
                        }
                    }

                    // ── Selected items (ordered) ──
                    div { class: "combo-editor-col",
                        div { class: "combo-editor-col-title", "组合顺序" }
                        div { class: "combo-editor-col-body",
                            {
                                let cur = items();
                                let total = cur.len();
                                if total == 0 {
                                    rsx! {
                                        div { class: "row-sub", style: "color: #86868b; padding: 6px;",
                                            "从左侧点「＋ 加入」添加模板"
                                        }
                                    }
                                } else {
                                    let tpls = templates_sig();
                                    rsx! {
                                        {cur.iter().enumerate().map(|(idx, item)| {
                                            let tpl_name = tpls.iter()
                                                .find(|t| t.id == item.template_id)
                                                .map(|t| t.name.clone())
                                                .unwrap_or_else(|| "⚠ 已删除模板".to_string());
                                            let item_label = item.label.clone().unwrap_or_default();
                                            rsx! {
                                                div { class: "combo-editor-item-row",
                                                    div { class: "row-content",
                                                        div { class: "row-label-bold", "#{idx + 1} · {tpl_name}" }
                                                        input {
                                                            class: "form-select",
                                                            style: "width: 100%; padding: 4px 6px; font-size: 11px; margin-top: 2px;",
                                                            placeholder: "自定义标签（可选）",
                                                            value: "{item_label}",
                                                            oninput: move |e| {
                                                                let mut cur = items();
                                                                if let Some(it) = cur.get_mut(idx) {
                                                                    let v = e.value();
                                                                    it.label = if v.trim().is_empty() { None } else { Some(v) };
                                                                }
                                                                items.set(cur);
                                                            },
                                                        }
                                                    }
                                                    button {
                                                        class: "btn-ghost",
                                                        style: "font-size: 11px; padding: 2px 6px;",
                                                        disabled: idx == 0,
                                                        onclick: move |_| {
                                                            let mut cur = items();
                                                            if idx > 0 { cur.swap(idx - 1, idx); }
                                                            items.set(cur);
                                                        },
                                                        "↑"
                                                    }
                                                    button {
                                                        class: "btn-ghost",
                                                        style: "font-size: 11px; padding: 2px 6px;",
                                                        disabled: idx + 1 >= total,
                                                        onclick: move |_| {
                                                            let mut cur = items();
                                                            if idx + 1 < cur.len() { cur.swap(idx, idx + 1); }
                                                            items.set(cur);
                                                        },
                                                        "↓"
                                                    }
                                                    button {
                                                        class: "btn-remove",
                                                        style: "font-size: 11px; padding: 2px 6px;",
                                                        onclick: move |_| {
                                                            let cur: Vec<ComboItem> = items()
                                                                .into_iter()
                                                                .enumerate()
                                                                .filter_map(|(i, it)| if i == idx { None } else { Some(it) })
                                                                .collect();
                                                            items.set(cur);
                                                        },
                                                        "删"
                                                    }
                                                }
                                            }
                                        })}
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(err) = local_error() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-top: 10px;", "{err}" }
                }

                div { class: "dialog-actions",
                    button { class: "btn-ghost", onclick: move |_| on_cancel.call(()), "取消" }
                    button {
                        class: "btn btn-primary",
                        onclick: {
                            let original = initial.clone();
                            move |_| {
                                let trimmed_name = name().trim().to_string();
                                if trimmed_name.is_empty() {
                                    local_error.set(Some("请填写组合名称".to_string()));
                                    return;
                                }
                                let cur_items = items();
                                if cur_items.is_empty() {
                                    local_error.set(Some("至少需要一个模板".to_string()));
                                    return;
                                }
                                let saved = ComboPreset {
                                    id: original.id.clone(),
                                    name: trimmed_name,
                                    description: description(),
                                    items: cur_items,
                                    tags: original.tags.clone(),
                                    created_at: original.created_at,
                                    updated_at: chrono::Utc::now(),
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
