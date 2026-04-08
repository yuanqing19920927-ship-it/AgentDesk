use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum AgentType {
    ClaudeCode,
    Codex,
}

impl AgentType {
    pub fn label(&self) -> &str {
        match self {
            AgentType::ClaudeCode => "Claude Code",
            AgentType::Codex => "Codex",
        }
    }

    pub fn command(&self) -> &str {
        match self {
            AgentType::ClaudeCode => "claude",
            AgentType::Codex => "codex",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PermissionMode {
    Default,
    DangerouslySkipPermissions,
    Plan,
}

impl PermissionMode {
    pub fn flag(&self) -> Option<&str> {
        match self {
            PermissionMode::Default => None,
            PermissionMode::DangerouslySkipPermissions => Some("--dangerously-skip-permissions"),
            PermissionMode::Plan => Some("--plan"),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            PermissionMode::Default => "Default",
            PermissionMode::DangerouslySkipPermissions => "Skip Permissions",
            PermissionMode::Plan => "Plan Mode",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentStatus {
    Running,
    Idle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub pid: u32,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub project_root: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
}
