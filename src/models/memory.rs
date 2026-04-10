use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One indexed memory entry, derived from a user/assistant exchange in a
/// session JSONL file. Each entry is uniquely identified by the source
/// message `uuid` so re-indexing is idempotent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MemoryEntry {
    /// Generated short id, e.g. "mem_001".
    pub id: String,
    /// Originating message uuid from the JSONL record. Primary dedup key.
    pub uuid: String,
    /// Originating session id (JSONL filename stem).
    pub session_id: String,
    /// ISO-8601 timestamp from the source record.
    pub timestamp: Option<DateTime<Utc>>,
    /// Git branch at the time of the message, if present.
    pub branch: Option<String>,
    /// Auto-extracted topics (currently empty — reserved for future
    /// LLM-based classification).
    #[serde(default)]
    pub topics: Vec<String>,
    /// Short summary (currently the first user message, truncated and
    /// sensitive-filtered).
    pub summary: String,
    /// Naive keywords extracted from the summary.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Pointer into the session archive markdown, e.g. "sessions/2026-04-08.md#mem_001".
    pub file_ref: String,
}

/// Cursor tracking incremental scan state for one JSONL file.
///
/// Fields mirror the design doc so re-indexing can safely resume from the
/// last processed byte while detecting inode or size changes that indicate
/// the file was replaced or truncated.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Cursor {
    pub byte_offset: u64,
    pub inode: u64,
    pub file_size: u64,
    pub last_uuid: Option<String>,
}

/// Top-level on-disk index format.
///
/// Stored either at `{project}/.agentdesk/index.json` (project-local mode)
/// or at `~/.agentdesk/projects/{path_hash}/index.json` (user-level
/// fallback when the project is not on the approved whitelist or git
/// guards refuse the write).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MemoryIndex {
    #[serde(default = "default_version")]
    pub version: u32,
    pub last_scan: Option<DateTime<Utc>>,
    /// JSONL filename → cursor.
    #[serde(default)]
    pub cursors: HashMap<String, Cursor>,
    #[serde(default)]
    pub entries: Vec<MemoryEntry>,
}

fn default_version() -> u32 {
    1
}

impl Default for MemoryIndex {
    fn default() -> Self {
        Self {
            version: 1,
            last_scan: None,
            cursors: HashMap::new(),
            entries: Vec::new(),
        }
    }
}

impl MemoryIndex {
    /// True if the index already contains an entry for this source message uuid.
    pub fn has_uuid(&self, uuid: &str) -> bool {
        self.entries.iter().any(|e| e.uuid == uuid)
    }

    /// Generate the next sequential entry id (`mem_001`, `mem_002` ...).
    pub fn next_entry_id(&self) -> String {
        format!("mem_{:03}", self.entries.len() + 1)
    }
}
