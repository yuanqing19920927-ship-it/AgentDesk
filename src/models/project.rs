use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub claude_dir_names: Vec<String>,
    /// Absolute paths to Codex rollout JSONL files bound to this
    /// project via their `session_meta.payload.cwd` field. Populated
    /// by `project_scanner::scan_projects` from `codex_scanner`.
    pub codex_session_files: Vec<PathBuf>,
    pub agent_count: usize,
    pub last_active: Option<DateTime<Utc>>,
    pub session_count: usize,
}

impl Project {
    pub fn display_name(&self) -> &str {
        &self.name
    }
}
