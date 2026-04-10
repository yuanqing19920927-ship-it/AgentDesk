//! Module 7 — Combo preset model.
//!
//! A **combo preset** is an ordered list of `AgentTemplate` references
//! that the user wants to launch together in a single project. Think
//! "full-stack dev kit": frontend Claude + backend Claude + test runner
//! Codex — one click and all three terminal windows pop up.
//!
//! The preset itself holds **references** (template ids), not a copy
//! of the template payload, so editing a template automatically
//! updates every preset that uses it. This mirrors how spec module 7
//! describes composition: combos "reference" templates.
//!
//! Persisted as JSON under `~/.agentdesk/presets/{id}.json`. Storage
//! is **user-level** (not per-project), because presets are reusable
//! across projects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One entry in a combo — a reference to an `AgentTemplate` plus an
/// optional display label so the same template can appear twice with
/// different UI hints (e.g. "frontend worker" vs "frontend reviewer"
/// both pointing at the same underlying template).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ComboItem {
    /// Id of the `AgentTemplate` to launch.
    pub template_id: String,
    /// Optional human-readable label for this specific slot in the
    /// combo. Falls back to the template's own name when absent.
    #[serde(default)]
    pub label: Option<String>,
}

/// A saved combination of templates. Launched in list order; failures
/// on individual items do not abort the launch of subsequent items
/// (see `preset_manager::launch_preset`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ComboPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Ordered list of template references. Must contain at least one
    /// entry for a preset to be launchable.
    pub items: Vec<ComboItem>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ComboPreset {
    /// Create a new empty combo with a freshly generated id. The
    /// caller is responsible for populating `items` before saving —
    /// `preset_manager::save` will reject an empty combo because it
    /// has no launch semantics.
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: new_preset_id(),
            name,
            description: String::new(),
            items: Vec::new(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Generate a unique id in the same style as template ids — timestamp
/// plus a process-local counter. Good enough for local-only data; no
/// UUID dep required.
pub(crate) fn new_preset_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("preset_{:x}_{:x}", ms, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_preset_has_empty_items_and_fresh_id() {
        let p1 = ComboPreset::new("kit".into());
        let p2 = ComboPreset::new("kit".into());
        assert_eq!(p1.items.len(), 0);
        assert_ne!(p1.id, p2.id);
        assert!(p1.id.starts_with("preset_"));
    }

    #[test]
    fn serde_roundtrip() {
        let mut p = ComboPreset::new("stack".into());
        p.items.push(ComboItem {
            template_id: "tmpl_abc".into(),
            label: Some("frontend".into()),
        });
        p.items.push(ComboItem {
            template_id: "tmpl_xyz".into(),
            label: None,
        });
        p.tags.push("dev".into());
        let json = serde_json::to_string(&p).unwrap();
        let back: ComboPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn label_defaults_to_none_on_deserialize() {
        let json = r#"{
            "id": "preset_1_0",
            "name": "kit",
            "description": "",
            "items": [{"template_id": "t1"}],
            "tags": [],
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }"#;
        let p: ComboPreset = serde_json::from_str(json).unwrap();
        assert_eq!(p.items[0].label, None);
    }
}
