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

/// Extract tty from ps aux line (column 6, e.g. "s003", "??" means no tty)
fn extract_tty(parts: &[&str]) -> Option<String> {
    if parts.len() > 6 {
        let tty = parts[6];
        if tty != "??" {
            return Some(tty.to_string());
        }
    }
    None
}

/// Extract CPU% from ps aux line (column 2)
fn extract_cpu(parts: &[&str]) -> f32 {
    if parts.len() > 2 {
        parts[2].parse().unwrap_or(0.0)
    } else {
        0.0
    }
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
    let tty = extract_tty(&parts);
    let cpu = extract_cpu(&parts);
    Some(Agent {
        pid,
        agent_type: AgentType::ClaudeCode,
        status: AgentStatus::from_cpu(cpu),
        cpu_percent: cpu,
        project_root: None,
        cwd,
        tty,
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
    let tty = extract_tty(&parts);
    let cpu = extract_cpu(&parts);
    Some(Agent {
        pid,
        agent_type: AgentType::Codex,
        status: AgentStatus::from_cpu(cpu),
        cpu_percent: cpu,
        project_root: None,
        cwd,
        tty,
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

/// Activate the iTerm2/Terminal window containing the given tty
pub fn focus_agent_terminal(tty: &str) -> Result<(), String> {
    let tty_device = format!("/dev/tty{}", tty);

    if std::path::Path::new("/Applications/iTerm.app").exists() {
        let script = format!(
            r#"tell application "iTerm2"
    activate
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                if tty of s is "{}" then
                    select t
                    tell w to select
                    return "found"
                end if
            end repeat
        end repeat
    end repeat
    return "not found"
end tell"#,
            tty_device
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("osascript 执行失败: {}", e))?;

        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if result.contains("not found") {
            return Err("未找到对应的终端窗口".to_string());
        }
    } else {
        // Fallback: just activate Terminal.app
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "Terminal" to activate"#)
            .output();
    }

    Ok(())
}
