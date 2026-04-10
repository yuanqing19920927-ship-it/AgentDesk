//! Module 11 — project health dashboard data model.
//!
//! A `ProjectHealth` rollup surfaces the signals that tell the user at a
//! glance whether a project is alive and well:
//!
//! * recent git activity (commits in the last 7/30 days)
//! * recent agent activity (sessions in the last 7 days)
//! * whether project memory is enabled and populated
//! * active agent count right now
//!
//! An overall `HealthStatus` is derived from these via a simple
//! "positive signals" heuristic so the UI can render a single dot
//! without the user having to interpret numbers.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthStatus {
    Green,
    Yellow,
    Red,
}

impl HealthStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Green => "健康",
            Self::Yellow => "注意",
            Self::Red => "停滞",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Green => "health-green",
            Self::Yellow => "health-yellow",
            Self::Red => "health-red",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectHealth {
    pub overall: HealthStatus,
    /// Commits in the last 7 days (0 if not a git repo).
    pub commits_7d: u64,
    /// Commits in the last 30 days (0 if not a git repo).
    pub commits_30d: u64,
    /// Days since last commit (`None` if not a git repo or no commits).
    pub last_commit_age_days: Option<u64>,
    /// Number of memory entries currently indexed.
    pub memory_entries: usize,
    /// Whether project memory indexing is enabled for this project.
    pub memory_enabled: bool,
    /// Sessions (JSONL files) touched in the last 7 days.
    pub sessions_7d: u64,
    /// Agents currently running for this project.
    pub active_agents: usize,
    /// Human-readable hints explaining the status.
    pub hints: Vec<String>,
}
