//! Notification service: rule engine + history ring buffer + macOS delivery.
//!
//! Call `send_event` from anywhere in the app — it checks the current
//! rules (global level, per-project override, event-type toggle, quiet
//! hours), logs the attempt to history, and dispatches to macOS via
//! `osascript` when allowed.

use crate::models::{NotificationEvent, NotificationEventType, NotificationLevel, NotificationRules};
use chrono::{Local, Timelike, Utc};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

const MAX_HISTORY: usize = 500;

fn config_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".agentdesk")
}

fn rules_path() -> PathBuf {
    config_dir().join("notification_rules.json")
}

fn history_path() -> PathBuf {
    config_dir().join("notifications.json")
}

// ──────────────────────── rules ────────────────────────

pub fn load_rules() -> NotificationRules {
    fs::read_to_string(rules_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_rules(rules: &NotificationRules) -> Result<(), String> {
    fs::create_dir_all(config_dir())
        .map_err(|e| format!("创建配置目录失败: {}", e))?;
    let json = serde_json::to_string_pretty(rules)
        .map_err(|e| format!("序列化通知规则失败: {}", e))?;
    write_atomic(&rules_path(), &json)
}

// ──────────────────────── history ────────────────────────

static HISTORY_LOCK: Mutex<()> = Mutex::new(());

pub fn load_history() -> Vec<NotificationEvent> {
    fs::read_to_string(history_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_history(events: &[NotificationEvent]) -> Result<(), String> {
    fs::create_dir_all(config_dir())
        .map_err(|e| format!("创建配置目录失败: {}", e))?;
    let json = serde_json::to_string_pretty(events)
        .map_err(|e| format!("序列化通知历史失败: {}", e))?;
    write_atomic(&history_path(), &json)
}

fn push_history(mut event: NotificationEvent) {
    let _lock = HISTORY_LOCK.lock().unwrap();
    let mut events = load_history();
    // Normalize timestamp to ensure strict monotonicity for display.
    if event.timestamp > Utc::now() {
        event.timestamp = Utc::now();
    }
    events.push(event);
    // Ring buffer: keep only the most recent MAX_HISTORY entries.
    if events.len() > MAX_HISTORY {
        let excess = events.len() - MAX_HISTORY;
        events.drain(0..excess);
    }
    let _ = save_history(&events);
}

/// Mark every notification as read. Returns the number of entries
/// mutated — 0 when the history is already empty.
#[allow(dead_code)]
pub fn mark_all_read() -> usize {
    let _lock = HISTORY_LOCK.lock().unwrap();
    let mut events = load_history();
    let mut touched = 0usize;
    for e in events.iter_mut() {
        if !e.read {
            e.read = true;
            touched += 1;
        }
    }
    if touched > 0 {
        let _ = save_history(&events);
    }
    touched
}

/// Number of unread notifications in the ring buffer.
#[allow(dead_code)]
pub fn unread_count() -> usize {
    load_history().iter().filter(|e| !e.read).count()
}

/// Clear the entire history (used from settings).
#[allow(dead_code)]
pub fn clear_history() -> Result<(), String> {
    let _lock = HISTORY_LOCK.lock().unwrap();
    save_history(&[])
}

/// Mark a single notification as read, keyed by timestamp. Used by the
/// in-app notification center when the user clicks an individual entry.
#[allow(dead_code)]
pub fn mark_read(timestamp: chrono::DateTime<Utc>) -> bool {
    let _lock = HISTORY_LOCK.lock().unwrap();
    let mut events = load_history();
    let mut touched = false;
    for e in events.iter_mut() {
        if e.timestamp == timestamp && !e.read {
            e.read = true;
            touched = true;
            break;
        }
    }
    if touched {
        let _ = save_history(&events);
    }
    touched
}

/// Delete a single notification by timestamp. Returns true when a
/// matching entry was removed.
#[allow(dead_code)]
pub fn delete_event(timestamp: chrono::DateTime<Utc>) -> bool {
    let _lock = HISTORY_LOCK.lock().unwrap();
    let mut events = load_history();
    let before = events.len();
    events.retain(|e| e.timestamp != timestamp);
    if events.len() != before {
        let _ = save_history(&events);
        true
    } else {
        false
    }
}

// ──────────────────────── rule evaluation ────────────────────────

/// Decide whether a notification should be delivered to macOS given the
/// current rules. The event is always logged to history even when
/// suppressed so the user can audit filtered notifications.
fn should_deliver(
    rules: &NotificationRules,
    event_type: NotificationEventType,
    project_root: Option<&Path>,
) -> bool {
    // Per-type filter
    if !rules.event_enabled(event_type) {
        return false;
    }

    // Per-project override (takes precedence over global level)
    let effective_level = project_root
        .and_then(|p| {
            let key = p.to_string_lossy().to_string();
            rules.per_project.get(&key).copied()
        })
        .unwrap_or(rules.global_level);

    match effective_level {
        NotificationLevel::Mute => return false,
        NotificationLevel::ErrorsOnly if !event_type.is_error() => return false,
        _ => {}
    }

    // Quiet hours
    if rules.quiet_hours.enabled && in_quiet_hours(&rules.quiet_hours) {
        // Errors still break through quiet hours so serious failures are
        // surfaced — tuneable later if users want strict silence.
        if !event_type.is_error() {
            return false;
        }
    }

    true
}

fn in_quiet_hours(q: &crate::models::QuietHours) -> bool {
    let now = Local::now();
    let mins = now.hour() * 60 + now.minute();
    if q.start_min == q.end_min {
        return false;
    }
    if q.start_min < q.end_min {
        mins >= q.start_min && mins < q.end_min
    } else {
        // Wraps past midnight.
        mins >= q.start_min || mins < q.end_min
    }
}

// ──────────────────────── public entry points ────────────────────────

/// Rule-aware notification entry point. Logs to history and delivers
/// via macOS when the rules allow.
pub fn send_event(
    event_type: NotificationEventType,
    title: &str,
    message: &str,
    project_root: Option<&Path>,
) {
    let rules = load_rules();
    let allowed = should_deliver(&rules, event_type, project_root);

    let event = NotificationEvent {
        timestamp: Utc::now(),
        event_type,
        title: title.to_string(),
        message: message.to_string(),
        project_root: project_root.map(|p| p.to_string_lossy().to_string()),
        read: false,
        suppressed: !allowed,
    };
    push_history(event);

    if allowed {
        deliver_macos(title, message);
    }
}

/// Legacy entry point kept for any caller that still wants a best-effort
/// notification without rule evaluation. Prefer `send_event`.
pub fn send_notification(title: &str, message: &str) {
    send_event(
        NotificationEventType::GenericError,
        title,
        message,
        None,
    );
}

fn deliver_macos(title: &str, message: &str) {
    let script = format!(
        r#"display notification "{}" with title "{}""#,
        escape(message),
        escape(title),
    );
    let _ = Command::new("osascript").arg("-e").arg(&script).output();
}

// ──────────────────────── helpers ────────────────────────

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension(
        format!(
            "{}.tmp",
            path.extension().and_then(|e| e.to_str()).unwrap_or("tmp")
        ),
    );
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        f.write_all(content.as_bytes())
            .map_err(|e| format!("写入文件失败: {}", e))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| format!("重命名文件失败: {}", e))
}
