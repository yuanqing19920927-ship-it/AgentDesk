use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Coarse delivery level for notifications.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Every event passes (subject to per-type filter and quiet hours).
    All,
    /// Only events whose type is considered an error pass through.
    ErrorsOnly,
    /// Nothing is delivered.
    Mute,
}

impl NotificationLevel {
    pub fn label(&self) -> &'static str {
        match self {
            NotificationLevel::All => "全部通知",
            NotificationLevel::ErrorsOnly => "仅错误",
            NotificationLevel::Mute => "静音",
        }
    }
}

/// Event types emitted by the app. Each can be toggled individually by
/// the user in settings.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotificationEventType {
    /// Agent stayed idle long enough to be considered done.
    AgentCompleted,
    /// Agent process disappeared from the process table.
    AgentExited,
    /// Memory indexer raised an error (write failure, drift detected…).
    MemoryError,
    /// Catch-all for user-visible errors.
    GenericError,
}

impl NotificationEventType {
    pub fn label(&self) -> &'static str {
        match self {
            NotificationEventType::AgentCompleted => "Agent 任务完成",
            NotificationEventType::AgentExited => "Agent 退出",
            NotificationEventType::MemoryError => "记忆索引错误",
            NotificationEventType::GenericError => "通用错误",
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(
            self,
            NotificationEventType::MemoryError
                | NotificationEventType::GenericError
                | NotificationEventType::AgentExited
        )
    }

    pub fn all() -> &'static [NotificationEventType] {
        &[
            NotificationEventType::AgentCompleted,
            NotificationEventType::AgentExited,
            NotificationEventType::MemoryError,
            NotificationEventType::GenericError,
        ]
    }
}

/// Inclusive/exclusive quiet-hours window expressed in local-time minutes
/// from midnight. When `start == end`, the window is considered disabled.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuietHours {
    pub enabled: bool,
    /// Start minute of day (0..1440).
    pub start_min: u32,
    /// End minute of day (0..1440). May wrap past midnight: if
    /// `end_min < start_min` the window crosses midnight.
    pub end_min: u32,
}

/// Rules applied before any notification is delivered. Persisted to
/// `~/.agentdesk/notification_rules.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NotificationRules {
    pub global_level: NotificationLevel,
    /// Event type → enabled flag.
    #[serde(default)]
    pub event_enabled: HashMap<String, bool>,
    /// Per-project overrides: canonical path → level.
    #[serde(default)]
    pub per_project: HashMap<String, NotificationLevel>,
    #[serde(default)]
    pub quiet_hours: QuietHours,
}

impl Default for NotificationRules {
    fn default() -> Self {
        let mut event_enabled = HashMap::new();
        for t in NotificationEventType::all() {
            event_enabled.insert(format!("{:?}", t), true);
        }
        Self {
            global_level: NotificationLevel::All,
            event_enabled,
            per_project: HashMap::new(),
            quiet_hours: QuietHours::default(),
        }
    }
}

impl NotificationRules {
    pub fn event_enabled(&self, t: NotificationEventType) -> bool {
        self.event_enabled
            .get(&format!("{:?}", t))
            .copied()
            .unwrap_or(true)
    }

    pub fn set_event_enabled(&mut self, t: NotificationEventType, on: bool) {
        self.event_enabled.insert(format!("{:?}", t), on);
    }
}

/// One entry in the notification history ring buffer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NotificationEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: NotificationEventType,
    pub title: String,
    pub message: String,
    /// Project canonical root if the event is project-scoped.
    #[serde(default)]
    pub project_root: Option<String>,
    /// True after the user has acknowledged the notification in-app.
    #[serde(default)]
    pub read: bool,
    /// True when the rule engine suppressed delivery to macOS (still
    /// logged so the user can review what was filtered).
    #[serde(default)]
    pub suppressed: bool,
}
