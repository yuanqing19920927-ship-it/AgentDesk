//! Module 9 — audit snapshot data model.
//!
//! An `AuditSnapshot` is a git-centric point-in-time capture: the HEAD
//! SHA, branch, porcelain status parsed into modified / added / deleted /
//! untracked lists, and a wall-clock timestamp. Snapshots are append-only
//! and live on disk as JSON files so the UI can build a timeline without
//! holding state in memory.
//!
//! `AuditDiff` is the delta between two snapshots (or a snapshot and the
//! current working tree) used by the "对比" view in the dashboard.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditSnapshot {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub label: Option<String>,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub modified: Vec<String>,
    pub added: Vec<String>,
    pub deleted: Vec<String>,
    pub renamed: Vec<String>,
    pub untracked: Vec<String>,
}

impl AuditSnapshot {
    /// Total number of dirty paths (anything that would make `git
    /// status` non-empty). Used as the timeline row's "dirty" chip.
    pub fn dirty_count(&self) -> usize {
        self.modified.len()
            + self.added.len()
            + self.deleted.len()
            + self.renamed.len()
            + self.untracked.len()
    }

    pub fn short_sha(&self) -> String {
        self.head_sha
            .as_deref()
            .map(|s| s.chars().take(7).collect())
            .unwrap_or_else(|| "—".to_string())
    }
}

/// Delta between two snapshots (or a snapshot and the current state).
#[derive(Clone, Debug)]
pub struct AuditDiff {
    /// Files that appeared in `new` but not in `old`.
    pub files_added: Vec<String>,
    /// Files that were present in `old` but are gone in `new`.
    pub files_removed: Vec<String>,
    /// Files present in both but whose status kind changed.
    pub files_changed: Vec<String>,
    /// HEAD SHA in the older snapshot.
    pub old_sha: Option<String>,
    /// HEAD SHA in the newer snapshot / current state.
    pub new_sha: Option<String>,
    /// True when old and new point to different commits.
    pub head_changed: bool,
}
