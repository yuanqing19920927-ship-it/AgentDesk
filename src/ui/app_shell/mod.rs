use dioxus::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::models::{Agent, Project, SessionSummary};
use crate::models::AgentStatus;
use crate::services::{agent_detector, notifier, project_manager, project_scanner, session_reader};
use crate::ui::styles::GLOBAL_CSS;

mod sidebar;
mod dashboard;
mod new_agent_dialog;
mod settings;

use sidebar::Sidebar;
use dashboard::Dashboard;
use new_agent_dialog::NewAgentDialog;
use settings::SettingsPanel;

#[component]
pub fn AppShell() -> Element {
    let mut projects = use_signal(Vec::<Project>::new);
    let mut agents = use_signal(Vec::<Agent>::new);
    let mut selected_idx = use_signal(|| None::<usize>);
    let mut sessions = use_signal(Vec::<SessionSummary>::new);
    let session_load_gen = use_hook(|| Arc::new(AtomicU64::new(0)));
    let mut show_new_agent = use_signal(|| false);
    let mut show_settings = use_signal(|| false);

    // Load projects (auto-discovered + custom)
    let load_all_projects = move || {
        spawn(async move {
            let (scanned, custom) = tokio::task::spawn_blocking(|| {
                let scanned = project_scanner::scan_projects();
                let custom = project_manager::custom_projects_as_models();
                (scanned, custom)
            }).await.unwrap_or_default();

            // Merge: auto-discovered first, then custom (skip duplicates)
            let mut merged = scanned;
            for cp in custom {
                if !merged.iter().any(|p| p.root == cp.root) {
                    merged.push(cp);
                }
            }
            projects.set(merged);
        });
    };

    // Initial load
    use_hook(move || {
        load_all_projects();
        spawn(async move {
            let detected = tokio::task::spawn_blocking(agent_detector::detect_agents)
                .await.unwrap_or_default();
            agents.set(detected);
        });
    });

    // Periodic agent refresh every 3 seconds + status change notifications
    use_hook(move || {
        spawn(async move {
            // Track previous state for change detection
            let mut prev_states: std::collections::HashMap<u32, AgentStatus> = std::collections::HashMap::new();
            let mut prev_pids: std::collections::HashSet<u32> = std::collections::HashSet::new();

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let detected = tokio::task::spawn_blocking(agent_detector::detect_agents)
                    .await.unwrap_or_default();

                let current_pids: std::collections::HashSet<u32> = detected.iter().map(|a| a.pid).collect();

                // Check for status changes: Busy -> Idle = task likely completed
                for agent in &detected {
                    if let Some(prev) = prev_states.get(&agent.pid) {
                        if *prev == AgentStatus::Busy && agent.status == AgentStatus::Idle {
                            let label = agent.agent_type.label().to_string();
                            let cwd_name = agent.cwd.as_ref()
                                .and_then(|c| c.file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            notifier::send_notification(
                                "AgentDesk",
                                &format!("{} 任务完成 ({})", label, cwd_name),
                            );
                        }
                    }
                }

                // Check for agents that disappeared (process exited)
                for pid in &prev_pids {
                    if !current_pids.contains(pid) {
                        if let Some(prev_status) = prev_states.get(pid) {
                            if *prev_status == AgentStatus::Busy {
                                notifier::send_notification(
                                    "AgentDesk",
                                    &format!("Agent (PID {}) 已退出", pid),
                                );
                            }
                        }
                    }
                }

                // Update tracking state
                prev_states.clear();
                for agent in &detected {
                    prev_states.insert(agent.pid, agent.status.clone());
                }
                prev_pids = current_pids;

                agents.set(detected);
            }
        });
    });

    // Match agents to projects
    let projects_with_agents = {
        let mut ps = projects().clone();
        let ag = agents();
        for project in &mut ps {
            project.agent_count = ag.iter()
                .filter(|a| a.cwd.as_ref().is_some_and(|cwd| cwd.starts_with(&project.root)))
                .count();
        }
        ps
    };

    // Load sessions when selected project changes
    {
        let projects_for_sessions = projects_with_agents.clone();
        let gen_ref = session_load_gen.clone();
        use_effect(move || {
            let idx = selected_idx();
            let gen = gen_ref.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(i) = idx {
                if let Some(proj) = projects_for_sessions.get(i) {
                    let claude_dirs = proj.claude_dir_names.clone();
                    let gen_check = gen_ref.clone();
                    spawn(async move {
                        let summaries = tokio::task::spawn_blocking(move || {
                            let home = dirs::home_dir().unwrap_or_default();
                            let mut all = Vec::new();
                            for dir_name in &claude_dirs {
                                let claude_dir = home.join(".claude").join("projects").join(dir_name);
                                all.extend(session_reader::read_all_sessions(&claude_dir));
                            }
                            all.sort_by(|a, b| b.started_at.cmp(&a.started_at));
                            all
                        }).await.unwrap_or_default();
                        if gen_check.load(Ordering::SeqCst) == gen {
                            sessions.set(summaries);
                        }
                    });
                }
            } else {
                sessions.set(Vec::new());
            }
        });
    }

    let selected_project = selected_idx().and_then(|i| projects_with_agents.get(i).cloned());

    let project_agents: Vec<Agent> = if let Some(ref proj) = selected_project {
        agents().iter()
            .filter(|a| a.cwd.as_ref().is_some_and(|cwd| cwd.starts_with(&proj.root)))
            .cloned().collect()
    } else {
        Vec::new()
    };

    rsx! {
        style { {GLOBAL_CSS} }
        div { class: "app-container",
            Sidebar {
                projects: projects_with_agents.clone(),
                selected_idx: if show_settings() { None } else { selected_idx() },
                on_select: move |i: usize| { show_settings.set(false); selected_idx.set(Some(i)); },
                on_settings: move |_| { show_settings.set(true); selected_idx.set(None); },
                on_add_project: move |_| {
                    // Open folder picker and add project
                    spawn(async move {
                        let result = tokio::task::spawn_blocking(|| {
                            if let Some(path) = project_manager::pick_folder() {
                                project_manager::add_custom_project(&path)
                            } else {
                                Err("已取消".to_string())
                            }
                        }).await;

                        match result {
                            Ok(Ok(())) => {
                                // Reload projects
                                load_all_projects();
                            }
                            Ok(Err(_e)) => {
                                // Could show error, for now just ignore (duplicate or cancelled)
                            }
                            Err(_) => {}
                        }
                    });
                },
            }
            div { class: "main-panel",
                if show_settings() {
                    SettingsPanel {
                        on_close: move |_| show_settings.set(false),
                        on_refresh: move |_| load_all_projects(),
                    }
                } else if let Some(project) = selected_project.clone() {
                    Dashboard {
                        project: project.clone(),
                        agents: project_agents.clone(),
                        sessions: sessions().clone(),
                        on_new_agent: move |_| show_new_agent.set(true),
                    }
                } else {
                    div { class: "empty-state",
                        h2 { "欢迎使用 AgentDesk" }
                        p { "从左侧选择一个项目，或点击 ＋ 添加新项目" }
                    }
                }
            }
        }
        if show_new_agent() {
            NewAgentDialog {
                project: selected_project.clone(),
                projects: projects_with_agents.clone(),
                on_close: move |_| show_new_agent.set(false),
            }
        }
    }
}
