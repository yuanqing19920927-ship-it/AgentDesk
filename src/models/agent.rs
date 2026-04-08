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
            PermissionMode::Default => "默认",
            PermissionMode::DangerouslySkipPermissions => "跳过权限检查",
            PermissionMode::Plan => "规划模式",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentStatus {
    /// CPU > 2%, actively processing
    Busy,
    /// CPU <= 2%, waiting for input or idle
    Idle,
}

impl AgentStatus {
    pub fn label(&self) -> &str {
        match self {
            AgentStatus::Busy => "工作中",
            AgentStatus::Idle => "空闲",
        }
    }

    pub fn from_cpu(cpu: f32) -> Self {
        if cpu > 2.0 { AgentStatus::Busy } else { AgentStatus::Idle }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub pid: u32,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub cpu_percent: f32,
    pub project_root: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub tty: Option<String>,
}
