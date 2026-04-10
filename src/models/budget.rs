//! Module 6 — budget settings and alert status.
//!
//! Budgets are **cumulative spend caps** expressed in USD. The user
//! sets a limit per project and/or a global limit, and AgentDesk warns
//! or escalates as accumulated cost approaches that cap.
//!
//! Why cumulative, not monthly? Claude Code / Codex session files are
//! append-only JSONL, and doing an accurate month filter requires
//! parsing every message timestamp. For an MVP alerting path,
//! cumulative-cap-with-manual-reset is honest and ships fast. Monthly
//! rollover can be layered on later without breaking the file format.
//!
//! Persisted as JSON under `~/.agentdesk/budget.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_warn_percent() -> f64 {
    80.0
}

/// User-editable settings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BudgetSettings {
    /// Global cap across all projects, in USD. `None` = no global cap.
    #[serde(default)]
    pub global_usd: Option<f64>,
    /// Per-project caps keyed by the canonical project root path string.
    /// `None`/missing entry = no per-project cap.
    #[serde(default)]
    pub per_project: HashMap<String, f64>,
    /// Percentage (0-100) at which the budget turns from `Ok` to `Warn`.
    /// Defaults to 80.
    #[serde(default = "default_warn_percent")]
    pub warn_at_percent: f64,
}

impl Default for BudgetSettings {
    fn default() -> Self {
        Self {
            global_usd: None,
            per_project: HashMap::new(),
            warn_at_percent: default_warn_percent(),
        }
    }
}

impl BudgetSettings {
    /// Look up a per-project limit, if one is set.
    pub fn project_limit(&self, project_root: &str) -> Option<f64> {
        self.per_project.get(project_root).copied()
    }
}

/// One of three alert tiers used to colour the UI.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetLevel {
    /// Under the warn threshold — green.
    Ok,
    /// At or over the warn threshold but under the limit — yellow.
    Warn,
    /// At or over the limit — red.
    Exceeded,
    /// No limit set — grey.
    None,
}

impl BudgetLevel {
    pub fn css_class(&self) -> &'static str {
        match self {
            BudgetLevel::Ok => "budget-ok",
            BudgetLevel::Warn => "budget-warn",
            BudgetLevel::Exceeded => "budget-exceeded",
            BudgetLevel::None => "budget-none",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BudgetLevel::Ok => "正常",
            BudgetLevel::Warn => "接近预算",
            BudgetLevel::Exceeded => "已超预算",
            BudgetLevel::None => "未设置",
        }
    }
}

/// Computed status for a single scope (project or global). Holds both
/// the raw used/limit numbers and the derived level, so the UI does
/// not need to recompute thresholds itself.
#[derive(Clone, Debug, PartialEq)]
pub struct BudgetStatus {
    pub scope_label: String,
    pub used_usd: f64,
    pub limit_usd: Option<f64>,
    /// Ratio as percent (0.0-…); `None` when `limit_usd` is `None`.
    pub percent: Option<f64>,
    pub level: BudgetLevel,
}

impl BudgetStatus {
    /// Build a status. `warn_at_percent` is the threshold for the
    /// `Warn` tier (e.g. 80.0 means yellow from 80% to 100%).
    pub fn compute(
        scope_label: impl Into<String>,
        used_usd: f64,
        limit_usd: Option<f64>,
        warn_at_percent: f64,
    ) -> Self {
        let (percent, level) = match limit_usd {
            None => (None, BudgetLevel::None),
            Some(limit) if limit <= 0.0 => (None, BudgetLevel::None),
            Some(limit) => {
                let pct = used_usd / limit * 100.0;
                let lvl = if pct >= 100.0 {
                    BudgetLevel::Exceeded
                } else if pct >= warn_at_percent {
                    BudgetLevel::Warn
                } else {
                    BudgetLevel::Ok
                };
                (Some(pct), lvl)
            }
        };
        Self {
            scope_label: scope_label.into(),
            used_usd,
            limit_usd,
            percent,
            level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_none_when_no_limit() {
        let s = BudgetStatus::compute("proj", 12.3, None, 80.0);
        assert_eq!(s.level, BudgetLevel::None);
        assert!(s.percent.is_none());
    }

    #[test]
    fn status_ok_warn_exceeded_boundaries() {
        // 50% of 10 = 5 — Ok
        let s = BudgetStatus::compute("proj", 5.0, Some(10.0), 80.0);
        assert_eq!(s.level, BudgetLevel::Ok);
        // 80% exact — Warn (boundary is inclusive)
        let s = BudgetStatus::compute("proj", 8.0, Some(10.0), 80.0);
        assert_eq!(s.level, BudgetLevel::Warn);
        // 100% exact — Exceeded
        let s = BudgetStatus::compute("proj", 10.0, Some(10.0), 80.0);
        assert_eq!(s.level, BudgetLevel::Exceeded);
        // Over limit
        let s = BudgetStatus::compute("proj", 15.0, Some(10.0), 80.0);
        assert_eq!(s.level, BudgetLevel::Exceeded);
    }

    #[test]
    fn zero_or_negative_limit_treated_as_none() {
        let s = BudgetStatus::compute("proj", 5.0, Some(0.0), 80.0);
        assert_eq!(s.level, BudgetLevel::None);
        let s = BudgetStatus::compute("proj", 5.0, Some(-1.0), 80.0);
        assert_eq!(s.level, BudgetLevel::None);
    }

    #[test]
    fn settings_default_warn_is_80() {
        let s = BudgetSettings::default();
        assert_eq!(s.warn_at_percent, 80.0);
        assert!(s.global_usd.is_none());
        assert!(s.per_project.is_empty());
    }

    #[test]
    fn settings_roundtrip_json() {
        let mut s = BudgetSettings::default();
        s.global_usd = Some(100.0);
        s.per_project.insert("/a/b".to_string(), 25.0);
        s.warn_at_percent = 90.0;
        let json = serde_json::to_string(&s).unwrap();
        let back: BudgetSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
