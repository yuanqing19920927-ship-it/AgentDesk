use dioxus::prelude::*;
use crate::models::{AgentType, PermissionMode, Project};
use crate::services::terminal_launcher;

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

    rsx! {
        div { class: "dialog-overlay",
            onclick: move |_| on_close.call(()),
            div { class: "dialog",
                onclick: move |e| e.stop_propagation(),
                h2 { "New Agent" }

                div { class: "form-group",
                    label { "Project" }
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
                    label { "Agent Type" }
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
                    label { "Permission Mode" }
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

                if permission_idx() == 1 {
                    div { class: "warning-box",
                        p { class: "warning-title", "Warning: Skip Permissions disables all safety checks." }
                        p { class: "warning-text", "The agent can execute any command without confirmation." }
                        label { style: "display: flex; align-items: center; gap: 8px; margin-top: 8px; cursor: pointer;",
                            input {
                                r#type: "checkbox",
                                checked: confirm_dangerous(),
                                onchange: move |e| confirm_dangerous.set(e.checked()),
                            }
                            "I understand the risks"
                        }
                    }
                }

                if let Some(ref err) = error_msg() {
                    div { style: "color: #f38ba8; font-size: 13px; margin-bottom: 12px;", "{err}" }
                }

                div { class: "dialog-actions",
                    button { class: "btn-ghost", onclick: move |_| on_close.call(()), "Cancel" }
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
                                launching.set(true);
                                error_msg.set(None);
                                spawn(async move {
                                    let result = tokio::task::spawn_blocking(move || {
                                        terminal_launcher::launch_agent(&proj.root, &at, &pm)
                                    }).await;
                                    match result {
                                        Ok(Ok(())) => on_close.call(()),
                                        Ok(Err(e)) => { error_msg.set(Some(e)); launching.set(false); }
                                        Err(e) => { error_msg.set(Some(format!("Task error: {}", e))); launching.set(false); }
                                    }
                                });
                            }
                        },
                        if launching() { "Launching..." } else { "Launch Agent" }
                    }
                }
            }
        }
    }
}
