use crate::models::{Agent, AgentStatus, AgentType};
use std::path::PathBuf;
use std::process::Command;

pub fn detect_agents() -> Vec<Agent> {
    let mut agents = Vec::new();
    let output = match Command::new("ps").args(["aux"]).output() {
        Ok(o) => o,
        Err(_) => return agents,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(agent) = parse_claude_code(line) {
            agents.push(agent);
        } else if let Some(agent) = parse_codex(line) {
            agents.push(agent);
        }
    }
    agents
}

fn parse_claude_code(line: &str) -> Option<Agent> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 11 {
        return None;
    }
    let pid: u32 = parts[1].parse().ok()?;
    let command_and_args = parts[10..].join(" ");

    if !command_and_args.contains("node") {
        return None;
    }
    if !command_and_args.contains("/claude")
        && !command_and_args.contains(" claude ")
        && !command_and_args.ends_with(" claude")
    {
        return None;
    }
    if command_and_args.contains("mcp-servers") || command_and_args.contains("codex-reviewer") {
        return None;
    }

    let cwd = get_process_cwd(pid);
    Some(Agent {
        pid,
        agent_type: AgentType::ClaudeCode,
        status: AgentStatus::Running,
        project_root: None,
        cwd,
    })
}

fn parse_codex(line: &str) -> Option<Agent> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 11 {
        return None;
    }
    let pid: u32 = parts[1].parse().ok()?;
    let command_and_args = parts[10..].join(" ");

    if !command_and_args.contains("codex") {
        return None;
    }
    if command_and_args.contains("mcp-servers") || command_and_args.contains("codex-reviewer") {
        return None;
    }

    let cwd = get_process_cwd(pid);
    Some(Agent {
        pid,
        agent_type: AgentType::Codex,
        status: AgentStatus::Running,
        project_root: None,
        cwd,
    })
}

fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path != "/" {
                return Some(PathBuf::from(path));
            }
        }
    }
    None
}
