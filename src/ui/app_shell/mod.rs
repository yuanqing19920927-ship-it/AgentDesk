use dioxus::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::models::{Agent, Project, SessionSummary};
use crate::models::AgentStatus;
use crate::services::{agent_detector, island, notifier, project_manager, project_scanner, session_reader};
use crate::ui::styles::GLOBAL_CSS;

mod sidebar;
mod dashboard;
mod dynamic_island;
mod new_agent_dialog;
mod settings;

use sidebar::Sidebar;
use dashboard::Dashboard;
use dynamic_island::DynamicIsland;
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

    // Initial load + start menu bar island overlay
    use_hook(move || {
        load_all_projects();
        spawn(async move {
            let detected = tokio::task::spawn_blocking(|| {
                let ag = agent_detector::detect_agents();
                let _ = island::start_overlay();
                island::write_island_state(&ag);
                ag
            }).await.unwrap_or_default();
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

                island::write_island_state(&detected);
                agents.set(detected);
            }
        });
    });

    // Match agents to projects using best (longest) prefix match
    // Each agent is assigned to the project with the longest matching root path,
    // preventing parent directories (e.g. ~) from claiming agents in subdirectory projects.
    let projects_with_agents = {
        let mut ps = projects().clone();
        let ag = agents();
        let roots: Vec<std::path::PathBuf> = ps.iter().map(|p| p.root.clone()).collect();
        for project in &mut ps {
            project.agent_count = ag.iter()
                .filter(|a| {
                    let Some(cwd) = a.cwd.as_ref() else { return false };
                    if !cwd.starts_with(&project.root) { return false; }
                    // Check no other project root is a longer (more specific) match
                    !roots.iter().any(|other| {
                        other != &project.root
                            && cwd.starts_with(other)
                            && other.as_os_str().len() > project.root.as_os_str().len()
                    })
                })
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
        let all_roots: Vec<std::path::PathBuf> = projects_with_agents.iter().map(|p| p.root.clone()).collect();
        let proj_root = proj.root.clone();
        agents().iter()
            .filter(|a| {
                let Some(cwd) = a.cwd.as_ref() else { return false };
                if !cwd.starts_with(&proj_root) { return false; }
                !all_roots.iter().any(|other| {
                    other != &proj_root
                        && cwd.starts_with(other)
                        && other.as_os_str().len() > proj_root.as_os_str().len()
                })
            })
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
                            p { "从左侧选择一个项目，或在设置中添加新项目" }
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
