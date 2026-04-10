//! Module 6 — budget settings persistence.
//!
//! Reads and writes `~/.agentdesk/budget.json` using the same atomic
//! tmp-file-and-rename pattern as template / preset storage. On read
//! errors we return `BudgetSettings::default()` — missing file or
//! corrupt JSON should not prevent the Dashboard from rendering; the
//! user can simply re-enter their caps.

use crate::models::{BudgetLevel, BudgetSettings, BudgetStatus};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn budget_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdesk")
        .join("budget.json")
}

/// Load the persisted settings, or a fresh default on any error.
pub fn load() -> BudgetSettings {
    let path = budget_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return BudgetSettings::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Save settings atomically. Creates the parent directory if needed.
pub fn save(settings: &BudgetSettings) -> Result<(), String> {
    let final_path = budget_path();
    let parent = final_path
        .parent()
        .ok_or_else(|| "预算配置路径无效".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;

    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化预算配置失败: {}", e))?;

    let tmp_path = final_path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("写入预算配置失败: {}", e))?;
        file.sync_all().ok();
    }

    fs::rename(&tmp_path, &final_path)
        .map_err(|e| format!("重命名预算配置失败: {}", e))?;
    Ok(())
}

/// Convenience: compute the per-project status given current used USD.
pub fn project_status(
    settings: &BudgetSettings,
    project_root: &str,
    used_usd: f64,
) -> BudgetStatus {
    BudgetStatus::compute(
        format!("项目预算：{}", project_root),
        used_usd,
        settings.project_limit(project_root),
        settings.warn_at_percent,
    )
}

/// Convenience: global status. `total_used_usd` must be the sum of
/// costs across every project the caller cares about.
#[allow(dead_code)]
pub fn global_status(settings: &BudgetSettings, total_used_usd: f64) -> BudgetStatus {
    BudgetStatus::compute(
        "全局预算",
        total_used_usd,
        settings.global_usd,
        settings.warn_at_percent,
    )
}

/// Merge-update a project limit. Passing `None` removes the entry,
/// passing `Some(limit)` sets/overwrites it. Persists immediately.
pub fn set_project_limit(project_root: &str, limit: Option<f64>) -> Result<BudgetSettings, String> {
    let mut settings = load();
    match limit {
        Some(v) if v > 0.0 => {
            settings.per_project.insert(project_root.to_string(), v);
        }
        _ => {
            settings.per_project.remove(project_root);
        }
    }
    save(&settings)?;
    Ok(settings)
}

/// Merge-update the global limit.
#[allow(dead_code)]
pub fn set_global_limit(limit: Option<f64>) -> Result<BudgetSettings, String> {
    let mut settings = load();
    settings.global_usd = match limit {
        Some(v) if v > 0.0 => Some(v),
        _ => None,
    };
    save(&settings)?;
    Ok(settings)
}

/// Merge-update warn threshold. Clamped to the sensible range
/// `[1.0, 100.0]` to avoid pathological UI states.
pub fn set_warn_percent(pct: f64) -> Result<BudgetSettings, String> {
    let mut settings = load();
    settings.warn_at_percent = pct.clamp(1.0, 100.0);
    save(&settings)?;
    Ok(settings)
}

/// Quick one-liner the toast/alert layer uses to decide if a
/// notification should fire.
#[allow(dead_code)]
pub fn should_alert(status: &BudgetStatus) -> bool {
    matches!(status.level, BudgetLevel::Warn | BudgetLevel::Exceeded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_status_reports_limit_from_settings() {
        let mut s = BudgetSettings::default();
        s.per_project.insert("/a/b".to_string(), 20.0);
        s.warn_at_percent = 50.0;

        let st = project_status(&s, "/a/b", 12.0);
        assert_eq!(st.limit_usd, Some(20.0));
        assert!(matches!(st.level, BudgetLevel::Warn));

        let st = project_status(&s, "/a/b", 25.0);
        assert!(matches!(st.level, BudgetLevel::Exceeded));

        let st = project_status(&s, "/unknown", 100.0);
        assert!(matches!(st.level, BudgetLevel::None));
    }

    #[test]
    fn should_alert_fires_on_warn_and_exceeded() {
        let ok = BudgetStatus::compute("x", 1.0, Some(10.0), 80.0);
        assert!(!should_alert(&ok));
        let warn = BudgetStatus::compute("x", 9.0, Some(10.0), 80.0);
        assert!(should_alert(&warn));
        let exceeded = BudgetStatus::compute("x", 11.0, Some(10.0), 80.0);
        assert!(should_alert(&exceeded));
    }
}
