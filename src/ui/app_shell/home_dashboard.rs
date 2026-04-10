//! Global dashboard shown when the user clicks the "主目录" entry in
//! the sidebar. Unlike the per-project `Dashboard`, this view
//! summarises state across every known project:
//!
//! * total project / agent / session / message counts
//! * cumulative USD cost across every project
//! * per-project cost ranking (top 5 by spend)
//! * recent activity list (most recently touched projects)

use chrono::Local;
use dioxus::prelude::*;
use std::collections::HashMap;

use crate::models::{Agent, Project};
use crate::services::cost_tracker;

/// One row in the per-project rollup computed on the background
/// thread. We don't reuse `ProjectCost` directly because we also want
/// the display name and last-active timestamp in the same struct for
/// rendering convenience.
#[derive(Clone, Debug)]
struct ProjectRollup {
    name: String,
    root: String,
    session_count: u64,
    message_count: u64,
    tokens_total: u64,
    cost_usd: f64,
    last_active: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, Default)]
struct GlobalStats {
    total_cost: f64,
    total_sessions: u64,
    total_messages: u64,
    total_tokens: u64,
    per_model_cost: Vec<(String, f64)>,
    projects: Vec<ProjectRollup>,
}

#[component]
pub fn HomeDashboard(projects: Vec<Project>, agents: Vec<Agent>) -> Element {
    let project_count = projects.len();
    let active_agents = agents.len();

    // Heavy work: parse every JSONL across every project. Runs once
    // per mount in the background, same pattern as `Dashboard`.
    let mut stats = use_signal(|| None::<GlobalStats>);
    {
        let projects_for_task = projects.clone();
        use_hook(move || {
            spawn(async move {
                let computed = tokio::task::spawn_blocking(move || {
                    compute_global_stats(&projects_for_task)
                })
                .await
                .ok();
                if let Some(s) = computed {
                    stats.set(Some(s));
                }
            });
        });
    }

    rsx! {
        div {
            // ── Header ──
            div { class: "page-header",
                div { class: "page-header-info",
                    h1 { "主目录总览" }
                    div { class: "path", "跨项目的整体使用与花销统计" }
                }
            }

            // ── High-level counters ──
            div { class: "section",
                div { class: "section-label", "全局总览" }
                div { class: "stats-grid",
                    div { class: "stat-card",
                        div { class: "stat-value blue", "{project_count}" }
                        div { class: "stat-label", "已知项目" }
                    }
                    div { class: "stat-card",
                        div { class: "stat-value green", "{active_agents}" }
                        div { class: "stat-label", "运行中 Agent" }
                    }
                    {
                        let s = stats();
                        let sessions = s.as_ref().map(|s| s.total_sessions).unwrap_or(0);
                        let messages = s.as_ref().map(|s| s.total_messages).unwrap_or(0);
                        rsx! {
                            div { class: "stat-card",
                                div { class: "stat-value blue", "{sessions}" }
                                div { class: "stat-label", "会话总数" }
                            }
                            div { class: "stat-card",
                                div { class: "stat-value orange", "{messages}" }
                                div { class: "stat-label", "助手调用次数" }
                            }
                        }
                    }
                }
            }

            // ── Cost rollup ──
            {
                let s = stats();
                rsx! {
                    div { class: "section",
                        div { class: "section-label", "累计费用与用量" }
                        div { class: "grouped-card",
                            if let Some(s) = s {
                                {render_cost_overview(&s)}
                            } else {
                                div { class: "grouped-row",
                                    div { class: "row-sub", "统计中..." }
                                }
                            }
                        }
                    }
                }
            }

            // ── Top projects by cost ──
            {
                let s = stats();
                rsx! {
                    div { class: "section",
                        div { class: "section-label", "项目花销排行 · Top 5" }
                        div { class: "grouped-card",
                            if let Some(s) = s {
                                if s.projects.is_empty() {
                                    div { class: "grouped-row",
                                        div { class: "row-label", style: "color: #86868b;", "暂无数据" }
                                    }
                                } else {
                                    {s.projects.iter().take(5).map(render_project_row)}
                                }
                            } else {
                                div { class: "grouped-row",
                                    div { class: "row-sub", "统计中..." }
                                }
                            }
                        }
                    }
                }
            }

            // ── Most recent projects ──
            {
                let s = stats();
                rsx! {
                    div { class: "section",
                        div { class: "section-label", "最近活跃项目" }
                        div { class: "grouped-card",
                            if let Some(s) = s {
                                {
                                    let mut recent: Vec<&ProjectRollup> = s.projects.iter().collect();
                                    recent.sort_by(|a, b| b.last_active.cmp(&a.last_active));
                                    let recent = recent.into_iter().take(8).cloned().collect::<Vec<_>>();
                                    if recent.is_empty() {
                                        rsx! {
                                            div { class: "grouped-row",
                                                div { class: "row-label", style: "color: #86868b;", "暂无会话记录" }
                                            }
                                        }
                                    } else {
                                        rsx! {
                                            {recent.into_iter().map(|p| render_recent_row(&p))}
                                        }
                                    }
                                }
                            } else {
                                div { class: "grouped-row",
                                    div { class: "row-sub", "统计中..." }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ──────────────────────── compute ────────────────────────

fn compute_global_stats(projects: &[Project]) -> GlobalStats {
    let mut rollups: Vec<ProjectRollup> = Vec::new();
    let mut total_cost = 0.0_f64;
    let mut total_sessions = 0u64;
    let mut total_messages = 0u64;
    let mut total_tokens = 0u64;
    let mut per_model: HashMap<String, f64> = HashMap::new();

    let home = dirs::home_dir().unwrap_or_default();

    for p in projects {
        // Skip the home "project" — it would double-count because its
        // claude_dir_names is generally empty but semantically it is a
        // container for the others.
        if p.root == home {
            continue;
        }
        if p.claude_dir_names.is_empty() && p.codex_session_files.is_empty() {
            continue;
        }
        let pc = cost_tracker::project_cost(&p.root, &p.claude_dir_names, &p.codex_session_files);
        total_cost += pc.cost_usd;
        total_sessions += pc.session_count;
        total_messages += pc.message_count;
        let tokens_total = pc.tokens.input + pc.tokens.output + pc.tokens.cache_write + pc.tokens.cache_read;
        total_tokens += tokens_total;

        for mb in &pc.models {
            *per_model.entry(mb.model.clone()).or_insert(0.0) += mb.cost_usd;
        }

        rollups.push(ProjectRollup {
            name: p.name.clone(),
            root: p.root.to_string_lossy().to_string(),
            session_count: pc.session_count,
            message_count: pc.message_count,
            tokens_total,
            cost_usd: pc.cost_usd,
            last_active: p.last_active,
        });
    }

    rollups.sort_by(|a, b| b.cost_usd.partial_cmp(&a.cost_usd).unwrap_or(std::cmp::Ordering::Equal));

    let mut per_model_cost: Vec<(String, f64)> = per_model.into_iter().collect();
    per_model_cost.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    GlobalStats {
        total_cost,
        total_sessions,
        total_messages,
        total_tokens,
        per_model_cost,
        projects: rollups,
    }
}

// ──────────────────────── render helpers ────────────────────────

fn render_cost_overview(s: &GlobalStats) -> Element {
    let total = cost_tracker::format_usd(s.total_cost);
    let tokens = cost_tracker::format_tokens(s.total_tokens);
    let msgs = s.total_messages;
    let sessions = s.total_sessions;
    let models = s.per_model_cost.clone();
    let has_models = !models.is_empty();

    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { class: "row-label-bold", "累计支出 {total}" }
                div { class: "row-sub",
                    "覆盖 {sessions} 个会话 · {msgs} 次助手调用 · {tokens} tokens"
                }
            }
        }
        if has_models {
            {models.into_iter().map(|(name, cost)| {
                let short = shorten_model_name(&name);
                let usd = cost_tracker::format_usd(cost);
                rsx! {
                    div { class: "grouped-row",
                        div { class: "row-content",
                            span { class: "row-label", "{short}" }
                        }
                        div { class: "row-value", "{usd}" }
                    }
                }
            })}
        }
    }
}

fn render_project_row(p: &ProjectRollup) -> Element {
    let name = p.name.clone();
    let root = p.root.clone();
    let cost = cost_tracker::format_usd(p.cost_usd);
    let tokens = cost_tracker::format_tokens(p.tokens_total);
    let sessions = p.session_count;
    let messages = p.message_count;

    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { class: "row-label-bold", "{name}" }
                div { class: "row-sub",
                    "{root}"
                }
                div { class: "row-sub",
                    "{sessions} 会话 · {messages} 调用 · {tokens} tokens"
                }
            }
            div { class: "row-value", "{cost}" }
        }
    }
}

fn render_recent_row(p: &ProjectRollup) -> Element {
    let name = p.name.clone();
    let root = p.root.clone();
    let ts = p.last_active
        .map(|t| t.with_timezone(&Local).format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".to_string());
    let sessions = p.session_count;

    rsx! {
        div { class: "grouped-row",
            div { class: "row-content",
                div { class: "row-label-bold", "{name}" }
                div { class: "row-sub", "{root}" }
            }
            div { class: "row-content", style: "flex: 0 0 auto; text-align: right; min-width: 110px;",
                div { class: "row-sub", "{ts}" }
                div { class: "row-sub", "{sessions} 会话" }
            }
        }
    }
}

/// Duplicated from dashboard.rs — kept local to avoid exposing an
/// internal helper as a public API. Converts "claude-opus-4-6" into
/// a compact display label like "Opus 4.6".
fn shorten_model_name(model: &str) -> String {
    let m = model.strip_prefix("claude-").unwrap_or(model);
    let parts: Vec<&str> = m.split('-').collect();
    if parts.is_empty() {
        return model.to_string();
    }
    let family = match parts[0] {
        "opus" => "Opus",
        "sonnet" => "Sonnet",
        "haiku" => "Haiku",
        other => return other.to_string(),
    };
    let version: Vec<String> = parts[1..]
        .iter()
        .filter(|p| !p.chars().all(|c| c.is_ascii_digit()) || p.len() < 5)
        .map(|s| s.to_string())
        .collect();
    if version.is_empty() {
        family.to_string()
    } else {
        format!("{} {}", family, version.join("."))
    }
}
