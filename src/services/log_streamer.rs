//! Module 10 — agent log streamer.
//!
//! Produces a richer view of a session JSONL than `session_reader`:
//! text, thinking, tool_use, and tool_result blocks are all surfaced
//! as typed `StreamItem`s so the log viewer can render them
//! differently and filter by kind.
//!
//! Intentionally stateless — a full re-read on each poll is cheap
//! (session files rarely exceed a few MB) and makes "live" mode
//! trivial: just call `read_session_stream` again.

use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;

/// One entry in the stream view. `Text` is a plain text message from
/// either side; `Thinking` is Claude's internal reasoning block;
/// `ToolUse` is a tool invocation by the assistant; `ToolResult` is
/// the response (injected as a synthetic user message in the JSONL).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamKind {
    Text,
    Thinking,
    ToolUse,
    ToolResult,
}

impl StreamKind {
    pub fn label(&self) -> &'static str {
        match self {
            StreamKind::Text => "消息",
            StreamKind::Thinking => "思考",
            StreamKind::ToolUse => "工具调用",
            StreamKind::ToolResult => "工具结果",
        }
    }
}

#[derive(Clone, Debug)]
pub struct StreamItem {
    pub role: String, // "user" or "assistant"
    pub kind: StreamKind,
    pub tool_name: Option<String>,
    pub content: String,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Read every message from a session JSONL and flatten the content
/// blocks into a single timeline of `StreamItem`s.
pub fn read_session_stream(claude_project_dir: &Path, session_id: &str) -> Vec<StreamItem> {
    let jsonl_path = claude_project_dir.join(format!("{}.jsonl", session_id));
    let content = match fs::read_to_string(&jsonl_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut items = Vec::new();
    for line in content.lines() {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let record_type = record
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let role = match record_type {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };
        let ts = record
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let Some(message) = record.get("message") else { continue };

        // Direct string content → single Text item.
        if let Some(s) = message.get("content").and_then(|c| c.as_str()) {
            if !s.trim().is_empty() {
                items.push(StreamItem {
                    role: role.to_string(),
                    kind: StreamKind::Text,
                    tool_name: None,
                    content: s.to_string(),
                    timestamp: ts,
                });
            }
            continue;
        }

        // Array of content blocks.
        if let Some(arr) = message.get("content").and_then(|c| c.as_array()) {
            for block in arr {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                            if !t.trim().is_empty() {
                                items.push(StreamItem {
                                    role: role.to_string(),
                                    kind: StreamKind::Text,
                                    tool_name: None,
                                    content: t.to_string(),
                                    timestamp: ts,
                                });
                            }
                        }
                    }
                    "thinking" => {
                        if let Some(t) = block.get("thinking").and_then(|t| t.as_str()) {
                            if !t.trim().is_empty() {
                                items.push(StreamItem {
                                    role: role.to_string(),
                                    kind: StreamKind::Thinking,
                                    tool_name: None,
                                    content: t.to_string(),
                                    timestamp: ts,
                                });
                            }
                        }
                    }
                    "tool_use" => {
                        let tool_name = block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                        let input = block
                            .get("input")
                            .map(|v| pretty_json(v))
                            .unwrap_or_default();
                        items.push(StreamItem {
                            role: role.to_string(),
                            kind: StreamKind::ToolUse,
                            tool_name,
                            content: input,
                            timestamp: ts,
                        });
                    }
                    "tool_result" => {
                        // tool_result.content may itself be a string or
                        // an array of `{type:text, text}` blocks.
                        let body = extract_tool_result(block);
                        items.push(StreamItem {
                            role: role.to_string(),
                            kind: StreamKind::ToolResult,
                            tool_name: None,
                            content: body,
                            timestamp: ts,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    items
}

fn extract_tool_result(block: &serde_json::Value) -> String {
    let content = match block.get("content") {
        Some(c) => c,
        None => return String::new(),
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                parts.push(t.to_string());
            }
        }
        return parts.join("\n");
    }
    pretty_json(content)
}

fn pretty_json(v: &serde_json::Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// Export a session's stream to a Markdown document. Used by the
/// "导出为 Markdown" button in the log viewer.
pub fn export_as_markdown(items: &[StreamItem]) -> String {
    let mut out = String::new();
    out.push_str("# Agent 会话日志\n\n");
    for item in items {
        let ts = item
            .timestamp
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "—".to_string());
        let role = if item.role == "user" { "用户" } else { "助手" };
        let tool = item
            .tool_name
            .as_deref()
            .map(|n| format!(" ({})", n))
            .unwrap_or_default();
        out.push_str(&format!("## {} · {} · {}{}\n\n", ts, role, item.kind.label(), tool));
        match item.kind {
            StreamKind::ToolUse | StreamKind::ToolResult => {
                out.push_str("```\n");
                out.push_str(&item.content);
                out.push_str("\n```\n\n");
            }
            _ => {
                out.push_str(&item.content);
                out.push_str("\n\n");
            }
        }
    }
    out
}
