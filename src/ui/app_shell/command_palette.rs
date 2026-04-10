//! Module 12.1 — Command Palette (Cmd+K).
//!
//! Spotlight-style modal overlay that fuzzy-searches across:
//! * projects (jump to project dashboard)
//! * running agents (focus terminal)
//! * agent templates (open templates panel — launching a template
//!   requires picking a project, which is a separate flow)
//! * static actions (new agent, settings, templates, home dashboard)
//!
//! Intentionally scoped to *in-app* shortcuts — true global hotkeys
//! that work when the app is unfocused need objc2 /
//! NSEvent.addGlobalMonitorForEvents + accessibility permission and
//! are tracked separately.
//!
//! Keyboard model:
//! * Cmd+K (or Ctrl+K) anywhere in the window → open
//! * ↑ / ↓ navigate, ↵ activate, Esc close
//! * Typing filters the result list live

use dioxus::prelude::*;

use crate::models::{Agent, AgentTemplate, Project};
use crate::services::terminal_launcher;

/// One row in the filtered result list.
#[derive(Clone, Debug)]
pub struct PaletteItem {
    pub kind: PaletteKind,
    /// Primary display label (matched against the query).
    pub label: String,
    /// Secondary dim text (path, type, etc.) — not matched.
    pub detail: String,
    /// One-char emoji/icon prefix so users can visually distinguish
    /// result kinds at a glance without needing coloured category
    /// headers.
    pub glyph: &'static str,
}

#[derive(Clone, Debug)]
pub enum PaletteKind {
    /// Jump to the dashboard for the project at this index in the
    /// shared `projects_with_agents` vec.
    Project { index: usize },
    /// Focus the terminal window hosting this running agent.
    Agent { pid: u32 },
    /// Open the templates management panel (we don't launch directly
    /// because that requires picking a project + additional context).
    Template { template_id: String },
    /// Open new-agent dialog.
    NewAgent,
    /// Open settings.
    Settings,
    /// Open templates.
    Templates,
    /// Jump to home directory dashboard.
    HomeDashboard,
}

/// Props — the palette is a leaf component that receives the
/// ambient state it needs via callbacks so AppShell stays in charge
/// of navigation and signal updates.
#[derive(Props, Clone, PartialEq)]
pub struct CommandPaletteProps {
    pub projects: Vec<Project>,
    pub agents: Vec<Agent>,
    pub templates: Vec<AgentTemplate>,
    pub on_close: EventHandler<()>,
    pub on_select_project: EventHandler<usize>,
    pub on_open_settings: EventHandler<()>,
    pub on_open_templates: EventHandler<()>,
    pub on_new_agent: EventHandler<()>,
    pub on_home_dashboard: EventHandler<()>,
}

#[component]
pub fn CommandPalette(props: CommandPaletteProps) -> Element {
    let mut query = use_signal(String::new);
    let mut selected = use_signal(|| 0usize);

    // Rebuild full catalog every render. Cheap — lists are small and
    // the palette is only mounted when open.
    let catalog = build_catalog(&props.projects, &props.agents, &props.templates);
    let q = query();
    let filtered: Vec<PaletteItem> = if q.trim().is_empty() {
        catalog.clone()
    } else {
        fuzzy_filter(&catalog, &q)
    };
    // Keep selection in range after filter shrinks.
    let sel = selected().min(filtered.len().saturating_sub(1));

    // Activation: run the action for the currently-selected item.
    let activate = {
        let filtered = filtered.clone();
        let on_close = props.on_close;
        let on_select_project = props.on_select_project;
        let on_open_settings = props.on_open_settings;
        let on_open_templates = props.on_open_templates;
        let on_new_agent = props.on_new_agent;
        let on_home_dashboard = props.on_home_dashboard;
        let agents = props.agents.clone();
        move |idx: usize| {
            let Some(item) = filtered.get(idx) else { return };
            match &item.kind {
                PaletteKind::Project { index } => {
                    on_select_project.call(*index);
                    on_close.call(());
                }
                PaletteKind::Agent { pid } => {
                    if let Some(a) = agents.iter().find(|a| a.pid == *pid) {
                        if let Some(cwd) = a.cwd.as_ref() {
                            let _ = terminal_launcher::focus_terminal_for_cwd(cwd);
                        }
                    }
                    on_close.call(());
                }
                PaletteKind::Template { .. } => {
                    on_open_templates.call(());
                    on_close.call(());
                }
                PaletteKind::NewAgent => {
                    on_new_agent.call(());
                    on_close.call(());
                }
                PaletteKind::Settings => {
                    on_open_settings.call(());
                    on_close.call(());
                }
                PaletteKind::Templates => {
                    on_open_templates.call(());
                    on_close.call(());
                }
                PaletteKind::HomeDashboard => {
                    on_home_dashboard.call(());
                    on_close.call(());
                }
            }
        }
    };

    // Keyboard navigation — ↑ / ↓ clamp, ↵ activates, Esc closes.
    // Arrow keys are swallowed so the browser doesn't try to scroll
    // the background.
    let key_handler = {
        let filtered_len = filtered.len();
        let on_close = props.on_close;
        let activate = activate.clone();
        move |e: KeyboardEvent| {
            match e.key() {
                Key::Escape => {
                    e.prevent_default();
                    on_close.call(());
                }
                Key::ArrowDown => {
                    e.prevent_default();
                    if filtered_len > 0 {
                        let cur = selected();
                        selected.set((cur + 1).min(filtered_len - 1));
                    }
                }
                Key::ArrowUp => {
                    e.prevent_default();
                    if filtered_len > 0 {
                        let cur = selected();
                        selected.set(cur.saturating_sub(1));
                    }
                }
                Key::Enter => {
                    e.prevent_default();
                    activate(sel);
                }
                _ => {}
            }
        }
    };

    rsx! {
        // Backdrop — click outside closes.
        div {
            class: "palette-backdrop",
            onclick: move |_| props.on_close.call(()),
            div {
                class: "palette-modal",
                // Swallow clicks inside so the backdrop handler doesn't fire.
                onclick: move |e| e.stop_propagation(),
                // Input
                input {
                    class: "palette-input",
                    placeholder: "搜索项目、Agent、模板或操作…",
                    autofocus: true,
                    value: "{q}",
                    oninput: move |e| {
                        query.set(e.value());
                        selected.set(0);
                    },
                    onkeydown: key_handler,
                }
                div { class: "palette-list",
                    if filtered.is_empty() {
                        div { class: "palette-empty", "无匹配项" }
                    } else {
                        {filtered.iter().enumerate().map(|(i, item)| {
                            let active = i == sel;
                            let label = item.label.clone();
                            let detail = item.detail.clone();
                            let glyph = item.glyph;
                            let activate_click = activate.clone();
                            rsx! {
                                div {
                                    key: "{i}",
                                    class: if active { "palette-row palette-row-active" } else { "palette-row" },
                                    onmouseenter: move |_| { selected.set(i); },
                                    onclick: move |_| { activate_click(i); },
                                    span { class: "palette-glyph", "{glyph}" }
                                    div { class: "palette-text",
                                        div { class: "palette-label", "{label}" }
                                        if !detail.is_empty() {
                                            div { class: "palette-detail", "{detail}" }
                                        }
                                    }
                                }
                            }
                        })}
                    }
                }
                div { class: "palette-footer",
                    span { class: "palette-hint", "↑↓ 选择" }
                    span { class: "palette-hint", "↵ 打开" }
                    span { class: "palette-hint", "Esc 关闭" }
                }
            }
        }
    }
}

// ──────────────────────── catalog ────────────────────────

fn build_catalog(
    projects: &[Project],
    agents: &[Agent],
    templates: &[AgentTemplate],
) -> Vec<PaletteItem> {
    let mut items = Vec::new();

    // Static actions first so they're always reachable without
    // typing — and because they're the shortest labels they naturally
    // sort to the top when fuzzy-matched.
    items.push(PaletteItem {
        kind: PaletteKind::NewAgent,
        label: "新建 Agent".into(),
        detail: "打开创建对话框".into(),
        glyph: "✦",
    });
    items.push(PaletteItem {
        kind: PaletteKind::HomeDashboard,
        label: "主目录总览".into(),
        detail: "跨项目全局仪表盘".into(),
        glyph: "⌂",
    });
    items.push(PaletteItem {
        kind: PaletteKind::Templates,
        label: "Agent 模板".into(),
        detail: "管理启动模板".into(),
        glyph: "▤",
    });
    items.push(PaletteItem {
        kind: PaletteKind::Settings,
        label: "设置".into(),
        detail: "扫描目录、通知、白名单".into(),
        glyph: "⚙",
    });

    // Projects
    for (i, p) in projects.iter().enumerate() {
        items.push(PaletteItem {
            kind: PaletteKind::Project { index: i },
            label: p.name.clone(),
            detail: p.root.display().to_string(),
            glyph: "◉",
        });
    }

    // Running agents — we look up the stored alias (if any) from
    // ~/.agentdesk/agent_names.json so the palette label matches
    // what the user sees in the sidebar. Alias map is small and
    // this is only computed when the palette mounts, so the extra
    // disk read is negligible.
    let alias_map = crate::services::agent_names::load_all();
    for a in agents {
        let project_key = a
            .project_root
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let agent_key = crate::services::agent_names::agent_key(a.tty.as_deref(), a.pid);
        let alias = alias_map
            .get(&project_key)
            .and_then(|m| m.get(&agent_key))
            .cloned();
        let label = match alias {
            Some(n) if !n.is_empty() => format!("{} · {}", a.agent_type.label(), n),
            _ => a.agent_type.label().to_string(),
        };
        let detail = a
            .cwd
            .as_ref()
            .map(|c| c.display().to_string())
            .unwrap_or_else(|| format!("PID {}", a.pid));
        items.push(PaletteItem {
            kind: PaletteKind::Agent { pid: a.pid },
            label,
            detail,
            glyph: "▶",
        });
    }

    // Templates
    for t in templates {
        items.push(PaletteItem {
            kind: PaletteKind::Template {
                template_id: t.id.clone(),
            },
            label: format!("模板 · {}", t.name),
            detail: t.agent_type.label().to_string(),
            glyph: "❏",
        });
    }

    items
}

// ──────────────────────── fuzzy match ────────────────────────

/// Very small subsequence fuzzy match. Scores = lower-is-better, based
/// on (char distance from first match to last match) + (leading offset).
/// We avoid pulling in a full fuzzy-matcher crate because the catalog
/// here is at most ~200 items and the UX doesn't need sophisticated
/// ranking — a subsequence-with-contiguity bonus is more than enough.
fn fuzzy_filter(items: &[PaletteItem], query: &str) -> Vec<PaletteItem> {
    let q: Vec<char> = query.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        return items.to_vec();
    }
    let mut scored: Vec<(i64, PaletteItem)> = Vec::new();
    for item in items {
        if let Some(score) = score_match(&item.label, &q) {
            scored.push((score, item.clone()));
        }
    }
    scored.sort_by_key(|(s, _)| *s);
    scored.into_iter().map(|(_, it)| it).collect()
}

fn score_match(haystack: &str, needle: &[char]) -> Option<i64> {
    let hay: Vec<char> = haystack.to_lowercase().chars().collect();
    let mut ni = 0;
    let mut first: Option<usize> = None;
    let mut last: usize = 0;
    let mut gaps: i64 = 0;
    let mut prev_idx: Option<usize> = None;
    for (hi, hc) in hay.iter().enumerate() {
        if ni < needle.len() && *hc == needle[ni] {
            if first.is_none() {
                first = Some(hi);
            }
            if let Some(p) = prev_idx {
                gaps += (hi - p) as i64 - 1;
            }
            prev_idx = Some(hi);
            last = hi;
            ni += 1;
        }
    }
    if ni < needle.len() {
        return None;
    }
    // Lower is better. We weight the leading offset (to prefer
    // matches that start near the front) and the sum of internal
    // gaps (to prefer contiguous matches).
    let lead = first.unwrap_or(0) as i64;
    let span = (last - first.unwrap_or(0)) as i64;
    Some(lead * 2 + gaps * 3 + span)
}
