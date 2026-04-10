//! Module 7 — import / export of templates and combo presets.
//!
//! Serializes a *bundle* (one or more templates, optionally with combo
//! presets that reference them) to a single JSON file so the user can
//! share configs across machines or back them up outside `~/.agentdesk`.
//!
//! Design notes:
//! * On export we write a tagged envelope (`kind: "agentdesk-bundle-v1"`)
//!   so future versions can evolve the schema without silently loading
//!   incompatible files.
//! * On import we **always** regenerate ids — both template ids and
//!   preset ids — and then rewrite every `ComboItem.template_id` in the
//!   imported presets to point at the new ids. This lets a user import
//!   the same bundle twice without the second import overwriting the
//!   first, and keeps each user's id namespace internally consistent.
//! * File picker dialogs use `osascript`. Cancellation is reported as a
//!   `None` return from `pick_*` helpers, not as an error.
//!
//! Out of scope for this module: conflict detection by name (if the
//! user imports a bundle containing a template whose name already
//! exists, both survive — the user can rename or delete either one
//! afterward). Doing name-level dedupe would require a UI prompt; we'd
//! rather accept duplicates than guess.

use crate::models::{AgentTemplate, ComboItem, ComboPreset};
use crate::services::{preset_manager, template_manager};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

const BUNDLE_KIND: &str = "agentdesk-bundle-v1";

/// Serialized envelope. Accepts bundles where either array is empty —
/// exporting a single template produces an empty `presets` list, and
/// exporting a preset with no templates (unusual but possible) produces
/// an empty `templates` list.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bundle {
    pub kind: String,
    pub exported_at: DateTime<Utc>,
    #[serde(default)]
    pub templates: Vec<AgentTemplate>,
    #[serde(default)]
    pub presets: Vec<ComboPreset>,
}

/// Summary returned to the UI after a successful import so the user
/// can see exactly what landed on disk.
#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub templates_imported: Vec<String>,
    pub presets_imported: Vec<String>,
    pub warnings: Vec<String>,
}

impl ImportReport {
    pub fn total(&self) -> usize {
        self.templates_imported.len() + self.presets_imported.len()
    }
}

/// Build a bundle containing exactly one template.
pub fn bundle_from_template(template: &AgentTemplate) -> Bundle {
    Bundle {
        kind: BUNDLE_KIND.to_string(),
        exported_at: Utc::now(),
        templates: vec![template.clone()],
        presets: Vec::new(),
    }
}

/// Build a bundle containing one preset plus every template it
/// references. `all_templates` is the current on-disk template list —
/// pass `template_manager::load_all()`. Referenced templates that are
/// missing from disk are silently dropped (the resulting bundle would
/// fail to launch those items after import, which is the same failure
/// mode as the original preset; there's nothing we can do about it
/// here).
pub fn bundle_from_preset(preset: &ComboPreset, all_templates: &[AgentTemplate]) -> Bundle {
    // Collect unique referenced template ids in preset order so the
    // bundle is deterministic.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut templates = Vec::new();
    for item in &preset.items {
        if !seen.insert(item.template_id.clone()) {
            continue;
        }
        if let Some(t) = all_templates.iter().find(|t| t.id == item.template_id) {
            templates.push(t.clone());
        }
    }
    Bundle {
        kind: BUNDLE_KIND.to_string(),
        exported_at: Utc::now(),
        templates,
        presets: vec![preset.clone()],
    }
}

/// Prompt for a destination file via `osascript choose file name`,
/// then write the bundle there as pretty JSON.
///
/// Returns `Ok(None)` if the user cancelled the dialog; `Ok(Some(path))`
/// if the file was written.
pub fn export_bundle_with_dialog(
    bundle: &Bundle,
    default_name: &str,
) -> Result<Option<PathBuf>, String> {
    let Some(target) = pick_save_path(default_name)? else {
        return Ok(None);
    };
    let json = serde_json::to_string_pretty(bundle)
        .map_err(|e| format!("序列化导出包失败: {}", e))?;
    std::fs::write(&target, json).map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(Some(target))
}

/// Prompt for a source file via `osascript choose file`, parse it as a
/// `Bundle`, remap ids, and persist every template + preset it contains
/// via the normal `template_manager::save` / `preset_manager::save`
/// paths so the atomic-write guarantees and directory creation logic
/// stay centralized.
///
/// Returns `Ok(None)` if the user cancelled the dialog.
pub fn import_bundle_with_dialog() -> Result<Option<ImportReport>, String> {
    let Some(source) = pick_open_path()? else {
        return Ok(None);
    };
    let content = std::fs::read_to_string(&source)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    let bundle: Bundle = serde_json::from_str(&content)
        .map_err(|e| format!("解析导入包失败: {}（请确认是 AgentDesk 导出的 JSON）", e))?;

    if bundle.kind != BUNDLE_KIND {
        return Err(format!(
            "文件类型不支持：{}（期望 {}）",
            bundle.kind, BUNDLE_KIND
        ));
    }

    let mut report = ImportReport::default();

    // Pass 1 — remap template ids. We build a `old_id -> new_id` map
    // so presets imported in pass 2 can rewrite their `template_id`
    // references. Without this, the imported preset would still point
    // at the exporter's ids, which are meaningless on this machine.
    let mut id_map: HashMap<String, String> = HashMap::new();
    for mut tpl in bundle.templates {
        let old = tpl.id.clone();
        tpl.id = crate::models::template::new_id();
        id_map.insert(old, tpl.id.clone());
        let name = tpl.name.clone();
        template_manager::save(&tpl)
            .map_err(|e| format!("保存模板 '{}' 失败: {}", name, e))?;
        report.templates_imported.push(name);
    }

    // Pass 2 — remap preset ids and rewrite `template_id` references.
    for mut preset in bundle.presets {
        preset.id = crate::models::preset::new_preset_id();
        preset.updated_at = Utc::now();

        for item in &mut preset.items {
            if let Some(new_id) = id_map.get(&item.template_id) {
                item.template_id = new_id.clone();
            } else {
                // Reference escaped the bundle — warn the user so they
                // know the imported preset has dangling items. We still
                // save the preset so they can repair it in the UI.
                report.warnings.push(format!(
                    "预设 '{}' 引用了未包含的模板 id={}",
                    preset.name, item.template_id
                ));
            }
        }

        let name = preset.name.clone();
        preset_manager::save(&preset)
            .map_err(|e| format!("保存预设 '{}' 失败: {}", name, e))?;
        report.presets_imported.push(name);
    }

    Ok(Some(report))
}

/// Prompt for a save file location. `default_name` is the filename
/// suggestion (without path). Cancel → `Ok(None)`.
fn pick_save_path(default_name: &str) -> Result<Option<PathBuf>, String> {
    // AppleScript `choose file name` returns a POSIX path as text when
    // we convert it explicitly. We sanitise `default_name` to keep it
    // out of trouble inside the quoted AppleScript literal.
    let safe_default = default_name.replace('"', "");
    let script = format!(
        r#"set target to choose file name with prompt "导出 AgentDesk 组合 / 模板" default name "{}"
return POSIX path of target"#,
        safe_default
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("调用 osascript 失败: {}", e))?;

    if !output.status.success() {
        // osascript exits non-zero when the user cancels — stderr will
        // contain "User canceled.", which we don't need to surface.
        return Ok(None);
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        Ok(None)
    } else {
        // `choose file name` does not force an extension — append
        // `.json` if the user omitted one, to keep imports working.
        let mut pb = PathBuf::from(path);
        if pb.extension().is_none() {
            pb.set_extension("json");
        }
        Ok(Some(pb))
    }
}

/// Prompt for an existing file. Cancel → `Ok(None)`.
fn pick_open_path() -> Result<Option<PathBuf>, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(
            r#"set chosen to choose file with prompt "选择要导入的 AgentDesk JSON 文件" of type {"json", "JSON", "public.json"}
return POSIX path of chosen"#,
        )
        .output()
        .map_err(|e| format!("调用 osascript 失败: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentType, PermissionMode};

    #[test]
    fn bundle_from_template_has_one_template_no_presets() {
        let t = AgentTemplate::new(
            "demo".into(),
            AgentType::ClaudeCode,
            PermissionMode::Default,
        );
        let b = bundle_from_template(&t);
        assert_eq!(b.kind, BUNDLE_KIND);
        assert_eq!(b.templates.len(), 1);
        assert!(b.presets.is_empty());
    }

    #[test]
    fn bundle_from_preset_collects_referenced_templates() {
        let t1 = AgentTemplate::new("A".into(), AgentType::ClaudeCode, PermissionMode::Default);
        let t2 = AgentTemplate::new("B".into(), AgentType::Codex, PermissionMode::Default);
        let t3 = AgentTemplate::new("C".into(), AgentType::ClaudeCode, PermissionMode::Default);
        let mut p = ComboPreset::new("kit".into());
        p.items.push(ComboItem { template_id: t1.id.clone(), label: None });
        // Reference t2 twice — should only appear once in the bundle.
        p.items.push(ComboItem { template_id: t2.id.clone(), label: None });
        p.items.push(ComboItem { template_id: t2.id.clone(), label: Some("b-again".into()) });
        // Reference a non-existent template — should be dropped.
        p.items.push(ComboItem { template_id: "ghost".into(), label: None });

        let pool = vec![t1.clone(), t2.clone(), t3.clone()];
        let b = bundle_from_preset(&p, &pool);

        assert_eq!(b.presets.len(), 1);
        assert_eq!(b.templates.len(), 2);
        let ids: Vec<&String> = b.templates.iter().map(|t| &t.id).collect();
        assert!(ids.contains(&&t1.id));
        assert!(ids.contains(&&t2.id));
        assert!(!ids.contains(&&t3.id));
    }

    #[test]
    fn bundle_roundtrips_json() {
        let t = AgentTemplate::new(
            "demo".into(),
            AgentType::ClaudeCode,
            PermissionMode::Default,
        );
        let mut p = ComboPreset::new("kit".into());
        p.items.push(ComboItem { template_id: t.id.clone(), label: None });
        let b = Bundle {
            kind: BUNDLE_KIND.to_string(),
            exported_at: Utc::now(),
            templates: vec![t],
            presets: vec![p],
        };
        let json = serde_json::to_string_pretty(&b).unwrap();
        let back: Bundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, BUNDLE_KIND);
        assert_eq!(back.templates.len(), 1);
        assert_eq!(back.presets.len(), 1);
    }
}
