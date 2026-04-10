use crate::models::{SessionRecord, SessionSummary};
use chrono::DateTime;
use std::fs;
use std::path::Path;

pub fn read_session(path: &Path) -> Option<SessionSummary> {
    let content = fs::read_to_string(path).ok()?;
    let mut session_id = None;
    let mut started_at = None;
    let mut message_count = 0usize;
    let mut cwd = None;
    let mut git_branch = None;
    let mut preview = None;

    for line in content.lines() {
        let record: SessionRecord = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if session_id.is_none() {
            session_id = record.session_id.clone();
        }

        match record.record_type.as_str() {
            "user" => {
                message_count += 1;
                if cwd.is_none() {
                    cwd = record.cwd.clone();
                }
                if git_branch.is_none() {
                    git_branch = record.git_branch.clone();
                }
                if started_at.is_none() {
                    if let Some(ts) = &record.timestamp {
                        started_at = DateTime::parse_from_rfc3339(ts)
                            .ok()
                            .map(|dt| dt.with_timezone(&chrono::Utc));
                    }
                }
                if preview.is_none() {
                    preview = extract_preview(&record);
                }
            }
            "assistant" => {
                message_count += 1;
            }
            _ => {}
        }
    }

    let session_id = session_id?;

    Some(SessionSummary {
        session_id,
        started_at,
        message_count,
        cwd,
        git_branch,
        preview,
    })
}

pub fn read_all_sessions(claude_project_dir: &Path) -> Vec<SessionSummary> {
    let entries = match fs::read_dir(claude_project_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut summaries: Vec<SessionSummary> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|e| read_session(&e.path()))
        .collect();

    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    summaries
}

// NOTE: session message loading for the expanded session view now
// lives in `log_streamer::read_session_stream`, which also surfaces
// thinking blocks, tool_use, and tool_result. Kept this file focused
// on session summary metadata.

fn extract_preview(record: &SessionRecord) -> Option<String> {
    let message = record.message.as_ref()?;

    if let Some(s) = message.as_str() {
        return Some(truncate(s, 120));
    }

    if let Some(content) = message.get("content") {
        if let Some(s) = content.as_str() {
            return Some(truncate(s, 120));
        }
        if let Some(arr) = content.as_array() {
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    return Some(truncate(text, 120));
                }
            }
        }
    }

    None
}

/// Unicode-safe truncation (by chars, not bytes)
fn truncate(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}
