use crate::models::{AgentType, PermissionMode};
use std::path::Path;
use std::process::Command;

pub fn launch_agent(
    project_dir: &Path,
    agent_type: &AgentType,
    permission_mode: &PermissionMode,
) -> Result<(), String> {
    if !project_dir.is_absolute() {
        return Err("Project path must be absolute".to_string());
    }
    if !project_dir.is_dir() {
        return Err(format!(
            "Project directory does not exist: {}",
            project_dir.display()
        ));
    }

    let dir_str = project_dir.to_string_lossy();
    let agent_cmd = agent_type.command();
    let flag = permission_mode.flag().unwrap_or("");
    let full_cmd = if flag.is_empty() {
        agent_cmd.to_string()
    } else {
        format!("{} {}", agent_cmd, flag)
    };

    let script = if is_iterm_installed() {
        build_iterm_script(&dir_str, &full_cmd)
    } else {
        build_terminal_script(&dir_str, &full_cmd)
    };

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript failed: {}", stderr));
    }

    Ok(())
}

fn is_iterm_installed() -> bool {
    Path::new("/Applications/iTerm.app").exists()
}

fn build_iterm_script(dir: &str, cmd: &str) -> String {
    format!(
        r#"tell application "iTerm2"
    activate
    set newWindow to (create window with default profile)
    tell current session of newWindow
        write text "cd " & quoted form of "{}" & " && {}"
    end tell
end tell"#,
        escape_applescript(dir),
        cmd
    )
}

fn build_terminal_script(dir: &str, cmd: &str) -> String {
    format!(
        r#"tell application "Terminal"
    activate
    do script "cd " & quoted form of "{}" & " && {}"
end tell"#,
        escape_applescript(dir),
        cmd
    )
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
