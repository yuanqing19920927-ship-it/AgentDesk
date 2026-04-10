use dioxus::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::models::{Agent, AgentTemplate, NotificationEventType, Project, SessionSummary};
use crate::models::AgentStatus;
use crate::services::{agent_detector, island, notifier, project_manager, project_scanner, session_reader, template_manager};
use crate::ui::styles::GLOBAL_CSS;

mod command_palette;
mod sidebar;
mod dashboard;
mod dynamic_island;
mod home_dashboard;
mod instruction_dialog;
mod memory_view;
mod new_agent_dialog;
mod notification_center;
mod settings;
mod templates;
// mod workflows_section; // 模块 5 暂缓：工作流功能先隐藏，代码保留在磁盘

use command_palette::CommandPalette;
use sidebar::Sidebar;
use dashboard::Dashboard;
use home_dashboard::HomeDashboard;
// dynamic_island module kept for potential future in-app use
use new_agent_dialog::NewAgentDialog;
use notification_center::NotificationCenter;
use settings::SettingsPanel;
use templates::TemplatesPanel;

#[component]
pub fn AppShell() -> Element {
    let mut projects = use_signal(Vec::<Project>::new);
    let mut agents = use_signal(Vec::<Agent>::new);
    let mut selected_idx = use_signal(|| None::<usize>);
    let mut sessions = use_signal(Vec::<SessionSummary>::new);
    let session_load_gen = use_hook(|| Arc::new(AtomicU64::new(0)));
    let mut show_new_agent = use_signal(|| false);
    let mut show_settings = use_signal(|| false);
    let mut show_templates = use_signal(|| false);
    let mut show_palette = use_signal(|| false);
    let mut show_notifications = use_signal(|| false);
    let mut unread_count = use_signal(notifier::unread_count);
    let mut templates = use_signal(Vec::<AgentTemplate>::new);

    // Load templates once on mount — cheap disk read, refreshed when the
    // palette is reopened below so new templates show up without restart.
    use_hook(move || {
        let loaded = template_manager::load_all();
        templates.set(loaded);
    });

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

    // Periodic agent refresh every 3 seconds + debounced status change notifications
    //
    // Problem: Agents briefly go idle between subtasks (API calls, tool switches),
    // causing false "task completed" notifications.
    // Solution: When Busy→Idle, start a 30s cooldown. Only notify if the agent stays
    // idle for the full cooldown. During cooldown, report as "busy" to the island.
    use_hook(move || {
        spawn(async move {
            let mut prev_states: std::collections::HashMap<u32, AgentStatus> = std::collections::HashMap::new();
            let mut prev_pids: std::collections::HashSet<u32> = std::collections::HashSet::new();
            // PID → (when it first went idle, agent label, project name)
            let mut idle_cooldowns: std::collections::HashMap<u32, (std::time::Instant, String, String)> = std::collections::HashMap::new();
            let idle_threshold = std::time::Duration::from_secs(3);

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let mut detected = tokio::task::spawn_blocking(agent_detector::detect_agents)
                    .await.unwrap_or_default();

                let current_pids: std::collections::HashSet<u32> = detected.iter().map(|a| a.pid).collect();
                let now = std::time::Instant::now();

                for agent in &detected {
                    let prev = prev_states.get(&agent.pid);

                    match (&agent.status, prev) {
                        // Busy→Idle: start cooldown (don't notify yet)
                        (AgentStatus::Idle, Some(AgentStatus::Busy)) => {
                            let label = agent.agent_type.label().to_string();
                            let project = agent.cwd.as_ref()
                                .and_then(|c| c.file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            idle_cooldowns.entry(agent.pid)
                                .or_insert((now, label, project));
                        }
                        // Back to Busy: cancel cooldown (was just a subtask gap)
                        (AgentStatus::Busy, _) => {
                            idle_cooldowns.remove(&agent.pid);
                        }
                        _ => {}
                    }
                }

                // Check cooldowns: notify if idle for the full threshold
                let expired: Vec<u32> = idle_cooldowns.iter()
                    .filter(|(pid, (since, _, _))| {
                        now.duration_since(*since) >= idle_threshold
                            && current_pids.contains(pid)
                    })
                    .map(|(pid, _)| *pid)
                    .collect();

                for pid in expired {
                    if let Some((_, label, project)) = idle_cooldowns.remove(&pid) {
                        let project_root = detected
                            .iter()
                            .find(|a| a.pid == pid)
                            .and_then(|a| a.project_root.clone());
                        notifier::send_event(
                            NotificationEventType::AgentCompleted,
                            "AgentDesk",
                            &format!("{} 任务完成 ({})", label, project),
                            project_root.as_deref(),
                        );
                    }
                }

                // Agents that disappeared while in cooldown — they exited mid-task
                let disappeared: Vec<u32> = prev_pids.iter()
                    .filter(|pid| !current_pids.contains(pid))
                    .copied()
                    .collect();

                for pid in disappeared {
                    idle_cooldowns.remove(&pid);
                    if let Some(prev_status) = prev_states.get(&pid) {
                        if *prev_status == AgentStatus::Busy {
                            notifier::send_event(
                                NotificationEventType::AgentExited,
                                "AgentDesk",
                                &format!("Agent (PID {}) 已退出", pid),
                                None,
                            );
                        }
                    }
                }

                // During cooldown, override status to Busy for island display
                for agent in &mut detected {
                    if idle_cooldowns.contains_key(&agent.pid) {
                        agent.status = AgentStatus::Busy;
                    }
                }

                // Update tracking state (use original detected status, not overridden)
                prev_states.clear();
                for agent in &detected {
                    // Store the real status (before cooldown override) for next-cycle comparison
                    let real_status = if idle_cooldowns.contains_key(&agent.pid) {
                        AgentStatus::Idle
                    } else {
                        agent.status.clone()
                    };
                    prev_states.insert(agent.pid, real_status);
                }
                prev_pids = current_pids;

                island::write_island_state(&detected);
                agents.set(detected);
                unread_count.set(notifier::unread_count());
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
        // Invisible strip along the top for dragging the window by the titlebar area.
        // wry/WKWebView on macOS does NOT support CSS `-webkit-app-region: drag`,
        // so we listen for mousedown and call the programmatic drag() API instead.
        div {
            class: "titlebar-drag",
            onmousedown: move |_| { dioxus::desktop::window().drag(); },
        }
        // Window-level key listener — Cmd+K toggles the command palette.
        // This is intentionally *not* a true OS-level global hotkey
        // (that would need objc2 + Accessibility permission). It only
        // fires when the AgentDesk window has focus.
        div { class: "app-container",
            tabindex: "0",
            onkeydown: move |e: KeyboardEvent| {
                let mods = e.modifiers();
                let cmd = mods.meta() || mods.ctrl();
                if cmd && e.key() == Key::Character("k".into()) {
                    e.prevent_default();
                    let next = !show_palette();
                    if next {
                        // Refresh template list on each open so newly-saved
                        // templates appear without restarting the app.
                        templates.set(template_manager::load_all());
                    }
                    show_palette.set(next);
                }
            },
            Sidebar {
                projects: projects_with_agents.clone(),
                selected_idx: if show_settings() || show_templates() { None } else { selected_idx() },
                unread_count: unread_count(),
                on_select: move |i: usize| {
                    show_settings.set(false);
                    show_templates.set(false);
                    selected_idx.set(Some(i));
                },
                on_settings: move |_| {
                    show_templates.set(false);
                    show_settings.set(true);
                    selected_idx.set(None);
                },
                on_templates: move |_| {
                    show_settings.set(false);
                    show_templates.set(true);
                    selected_idx.set(None);
                },
                on_notifications: move |_| {
                    // Opening the panel counts as "seeing" the new items:
                    // refresh the unread badge once they've marked-as-read
                    // or dismissed entries from inside the panel.
                    show_notifications.set(true);
                },
            }
            div { class: "main-panel",
                    if show_settings() {
                        SettingsPanel {
                            on_close: move |_| show_settings.set(false),
                            on_refresh: move |_| load_all_projects(),
                        }
                    } else if show_templates() {
                        TemplatesPanel {
                            on_close: move |_| show_templates.set(false),
                        }
                    } else if let Some(project) = selected_project.clone() {
                        {
                            let home_dir = dirs::home_dir().unwrap_or_default();
                            let is_home = project.root == home_dir;
                            let key_str = project.root.display().to_string();
                            if is_home {
                                rsx! {
                                    HomeDashboard {
                                        key: "{key_str}",
                                        projects: projects_with_agents.clone(),
                                        agents: agents().clone(),
                                    }
                                }
                            } else {
                                rsx! {
                                    Dashboard {
                                        // Keying on project root forces a full
                                        // remount on project switch so that
                                        // use_hook-driven signals (cost, health,
                                        // audit) recompute against the new root.
                                        key: "{key_str}",
                                        project: project.clone(),
                                        agents: project_agents.clone(),
                                        sessions: sessions().clone(),
                                        on_new_agent: move |_| show_new_agent.set(true),
                                    }
                                }
                            }
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
        if show_notifications() {
            {
                // Snapshot project roots so the jump handler can resolve
                // them to an index without borrowing `projects_with_agents`
                // from the outer scope.
                let project_roots: Vec<std::path::PathBuf> = projects_with_agents
                    .iter()
                    .map(|p| p.root.clone())
                    .collect();
                rsx! {
                    NotificationCenter {
                        on_close: move |_| {
                            show_notifications.set(false);
                            unread_count.set(notifier::unread_count());
                        },
                        on_jump_project: move |root: String| {
                            let target = std::path::PathBuf::from(&root);
                            if let Some(idx) = project_roots.iter().position(|p| p == &target) {
                                show_settings.set(false);
                                show_templates.set(false);
                                show_notifications.set(false);
                                selected_idx.set(Some(idx));
                                unread_count.set(notifier::unread_count());
                            }
                        },
                    }
                }
            }
        }
        if show_palette() {
            {
                let palette_projects = projects_with_agents.clone();
                // Home dashboard navigation: find the project whose root
                // matches $HOME. Falls back to selecting index 0 if missing.
                let home_dir = dirs::home_dir().unwrap_or_default();
                let home_idx = palette_projects
                    .iter()
                    .position(|p| p.root == home_dir)
                    .unwrap_or(0);
                rsx! {
                    CommandPalette {
                        projects: palette_projects.clone(),
                        agents: agents().clone(),
                        templates: templates().clone(),
                        on_close: move |_| show_palette.set(false),
                        on_select_project: move |i: usize| {
                            show_settings.set(false);
                            show_templates.set(false);
                            selected_idx.set(Some(i));
                        },
                        on_open_settings: move |_| {
                            show_templates.set(false);
                            show_settings.set(true);
                            selected_idx.set(None);
                        },
                        on_open_templates: move |_| {
                            show_settings.set(false);
                            show_templates.set(true);
                            selected_idx.set(None);
                        },
                        on_new_agent: move |_| show_new_agent.set(true),
                        on_home_dashboard: move |_| {
                            show_settings.set(false);
                            show_templates.set(false);
                            selected_idx.set(Some(home_idx));
                        },
                    }
                }
            }
        }
    }
}
