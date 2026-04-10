use dioxus::prelude::*;
use crate::models::{AgentTemplate, AgentType, PermissionMode, Project};
use crate::services::{template_manager, terminal_launcher};

#[component]
pub fn NewAgentDialog(
    project: Option<Project>,
    projects: Vec<Project>,
    on_close: EventHandler<()>,
) -> Element {
    let mut selected_project_idx = use_signal(|| if project.is_some() { Some(0usize) } else { None });
    let mut agent_type_idx = use_signal(|| 0usize);
    let mut permission_idx = use_signal(|| 0usize);
    let mut error_msg = use_signal(|| None::<String>);
    let mut launching = use_signal(|| false);
    let mut confirm_dangerous = use_signal(|| false);

    // Load templates once when the dialog mounts.
    let templates = use_hook(template_manager::load_all);
    // Selected template (by index into `templates`). None = no template.
    let mut selected_template_idx = use_signal(|| None::<usize>);
    // Initial prompt carried by the chosen template (copied to clipboard on launch).
    let mut active_prompt = use_signal(|| None::<String>);

    let agent_types = [AgentType::ClaudeCode, AgentType::Codex];
    let permissions = [PermissionMode::Default, PermissionMode::DangerouslySkipPermissions, PermissionMode::Plan];

    let effective_projects: Vec<Project> = if let Some(ref p) = project {
        let mut list = vec![p.clone()];
        for proj in &projects {
            if proj.root != p.root { list.push(proj.clone()); }
        }
        list
    } else {
        projects.clone()
    };

    // Apply a template to the form fields. Clears when `idx` is None.
    let mut apply_template = move |idx: Option<usize>, tpls: &Vec<AgentTemplate>| {
        selected_template_idx.set(idx);
        let Some(i) = idx else {
            active_prompt.set(None);
            return;
        };
        let Some(t) = tpls.get(i) else { return };
        agent_type_idx.set(match t.agent_type {
            AgentType::ClaudeCode => 0,
            AgentType::Codex => 1,
        });
        permission_idx.set(match t.permission_mode {
            PermissionMode::Default => 0,
            PermissionMode::DangerouslySkipPermissions => 1,
            PermissionMode::Plan => 2,
        });
        if t.permission_mode != PermissionMode::DangerouslySkipPermissions {
            confirm_dangerous.set(false);
        }
        active_prompt.set(t.initial_prompt.clone().filter(|s| !s.trim().is_empty()));
    };

    rsx! {
        div { class: "dialog-overlay",
            onclick: move |_| on_close.call(()),
            div { class: "dialog",
                onclick: move |e| e.stop_propagation(),
                h2 { "新建 Agent" }

                if !templates.is_empty() {
                    div { class: "form-group",
                        label { "从模板" }
                        select {
                            class: "form-select",
                            onchange: {
                                let tpls = templates.clone();
                                move |e: Event<FormData>| {
                                    let v = e.value();
                                    if v.is_empty() {
                                        apply_template(None, &tpls);
                                    } else if let Ok(i) = v.parse::<usize>() {
                                        apply_template(Some(i), &tpls);
                                    }
                                }
                            },
                            option { value: "", selected: selected_template_idx().is_none(), "— 不使用模板 —" }
                            for (i, t) in templates.iter().enumerate() {
                                option { value: "{i}", selected: selected_template_idx() == Some(i), "{t.name}" }
                            }
                        }
                    }
                }

                div { class: "form-group",
                    label { "项目" }
                    select {
                        class: "form-select",
                        onchange: move |e| {
                            if let Ok(idx) = e.value().parse::<usize>() {
                                selected_project_idx.set(Some(idx));
                            }
                        },
                        for (i, p) in effective_projects.iter().enumerate() {
                            option {
                                value: "{i}",
                                selected: selected_project_idx() == Some(i),
                                "{p.name} — {p.root.display()}"
                            }
                        }
                    }
                }

                div { class: "form-group",
                    label { "Agent 类型" }
                    select {
                        class: "form-select",
                        onchange: move |e| {
                            if let Ok(idx) = e.value().parse::<usize>() {
                                agent_type_idx.set(idx);
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
                            if let Ok(idx) = e.value().parse::<usize>() {
                                permission_idx.set(idx);
                                if idx != 1 { confirm_dangerous.set(false); }
                            }
                        },
                        for (i, pm) in permissions.iter().enumerate() {
                            option { value: "{i}", selected: permission_idx() == i, "{pm.label()}" }
                        }
                    }
                }

                if let Some(p) = active_prompt() {
                    div { style: "background: #f2f2f7; border-radius: 8px; padding: 10px; margin-bottom: 12px; font-size: 11px; color: #3a3a3c; max-height: 100px; overflow: auto; white-space: pre-wrap;",
                        div { style: "font-weight: 600; margin-bottom: 4px; color: #1d1d1f;", "初始 Prompt（启动后复制到剪贴板）" }
                        "{p}"
                    }
                }

                if permission_idx() == 1 {
                    div { class: "warning-box",
                        p { class: "warning-title", "警告：跳过权限检查将禁用所有安全防护" }
                        p { class: "warning-text", "Agent 将可以在不经确认的情况下执行任意命令。" }
                        label { style: "display: flex; align-items: center; gap: 8px; margin-top: 8px; cursor: pointer;",
                            input {
                                r#type: "checkbox",
                                checked: confirm_dangerous(),
                                onchange: move |e| confirm_dangerous.set(e.checked()),
                            }
                            "我已了解风险"
                        }
                    }
                }

                if let Some(ref err) = error_msg() {
                    div { style: "color: #ff3b30; font-size: 12px; margin-bottom: 12px;", "{err}" }
                }

                div { class: "dialog-actions",
                    button { class: "btn-ghost", onclick: move |_| on_close.call(()), "取消" }
                    button {
                        class: "btn btn-primary",
                        disabled: launching() || selected_project_idx().is_none() || (permission_idx() == 1 && !confirm_dangerous()),
                        onclick: {
                            let effective_projects = effective_projects.clone();
                            let agent_types = agent_types.clone();
                            let permissions = permissions.clone();
                            move |_| {
                                let proj_idx = match selected_project_idx() { Some(i) => i, None => return };
                                let proj = match effective_projects.get(proj_idx) { Some(p) => p.clone(), None => return };
                                let at = agent_types[agent_type_idx()].clone();
                                let pm = permissions[permission_idx()].clone();
                                let prompt_for_launch = active_prompt();
                                launching.set(true);
                                error_msg.set(None);
                                spawn(async move {
                                    let result = tokio::task::spawn_blocking(move || {
                                        terminal_launcher::launch_agent_with_prompt(
                                            &proj.root,
                                            &at,
                                            &pm,
                                            prompt_for_launch.as_deref(),
                                        )
                                    }).await;
                                    match result {
                                        Ok(Ok(())) => on_close.call(()),
                                        Ok(Err(e)) => { error_msg.set(Some(e)); launching.set(false); }
                                        Err(e) => { error_msg.set(Some(format!("任务错误: {}", e))); launching.set(false); }
                                    }
                                });
                            }
                        },
                        if launching() { "启动中..." } else { "启动 Agent" }
                    }
                }
            }
        }
    }
}
