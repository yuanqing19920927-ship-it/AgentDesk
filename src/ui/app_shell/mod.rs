use dioxus::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::models::{Agent, Project, SessionSummary};
use crate::services::{agent_detector, project_scanner, session_reader};
use crate::ui::styles::GLOBAL_CSS;

mod sidebar;
mod dashboard;
mod new_agent_dialog;

use sidebar::Sidebar;
use dashboard::Dashboard;
use new_agent_dialog::NewAgentDialog;

#[component]
pub fn AppShell() -> Element {
    let mut projects = use_signal(Vec::<Project>::new);
    let mut agents = use_signal(Vec::<Agent>::new);
    let mut selected_idx = use_signal(|| None::<usize>);
    let mut sessions = use_signal(Vec::<SessionSummary>::new);
    let session_load_gen = use_hook(|| Arc::new(AtomicU64::new(0)));
    let mut show_new_agent = use_signal(|| false);

    // Initial load
    use_hook(move || {
        spawn(async move {
            let scanned = tokio::task::spawn_blocking(project_scanner::scan_projects)
                .await.unwrap_or_default();
            let detected = tokio::task::spawn_blocking(agent_detector::detect_agents)
                .await.unwrap_or_default();
            projects.set(scanned);
            agents.set(detected);
        });
    });

    // Periodic agent refresh every 3 seconds
    use_hook(move || {
        spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let detected = tokio::task::spawn_blocking(agent_detector::detect_agents)
                    .await.unwrap_or_default();
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
                selected_idx: selected_idx(),
                on_select: move |i: usize| selected_idx.set(Some(i)),
            }
            div { class: "main-panel",
                if let Some(project) = selected_project.clone() {
                    Dashboard {
                        project: project.clone(),
                        agents: project_agents.clone(),
                        sessions: sessions().clone(),
                        on_new_agent: move |_| show_new_agent.set(true),
                    }
                } else {
                    div { class: "empty-state",
                        h2 { "Welcome to AgentDesk" }
                        p { "Select a project from the sidebar to get started." }
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
