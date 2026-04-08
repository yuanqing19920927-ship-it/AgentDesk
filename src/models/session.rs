use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SessionRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub uuid: Option<String>,
    pub cwd: Option<String>,
    #[serde(rename = "gitBranch")]
    pub git_branch: Option<String>,
    pub message: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionSummary {
    pub session_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub message_count: usize,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub preview: Option<String>,
}
