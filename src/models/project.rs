use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub claude_dir_names: Vec<String>,
    pub agent_count: usize,
    pub last_active: Option<DateTime<Utc>>,
    pub session_count: usize,
}

impl Project {
    pub fn display_name(&self) -> &str {
        &self.name
    }
}
