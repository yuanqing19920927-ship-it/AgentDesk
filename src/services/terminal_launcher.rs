use crate::models::{AgentType, PermissionMode};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Launch an Agent with an optional initial prompt.
///
/// If `initial_prompt` is `Some`, the prompt is copied to the system
/// clipboard after the terminal window is opened, so the user can paste it
/// with ⌘V once the REPL is ready. We intentionally avoid AppleScript
/// `write text` injection for free-form prompts — the design doc (module 4)
/// calls out the REPL race condition and defers reliable injection to P3.
pub fn launch_agent_with_prompt(
    project_dir: &Path,
    agent_type: &AgentType,
    permission_mode: &PermissionMode,
    initial_prompt: Option<&str>,
) -> Result<(), String> {
    launch_agent(project_dir, agent_type, permission_mode)?;
    if let Some(prompt) = initial_prompt {
        let trimmed = prompt.trim();
        if !trimmed.is_empty() {
            if let Err(e) = copy_to_clipboard(trimmed) {
                eprintln!("[AgentDesk] clipboard copy failed: {}", e);
            }
        }
    }
    Ok(())
}

/// Copy a string to the macOS clipboard via `pbcopy`. We pipe via stdin
/// (not argv) so arbitrary prompt content cannot affect the command line.
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn pbcopy: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("failed to write to pbcopy: {}", e))?;
    }
    let status = child
        .wait()
        .map_err(|e| format!("pbcopy wait failed: {}", e))?;
    if !status.success() {
        return Err(format!("pbcopy exited with {}", status));
    }
    Ok(())
}

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

    eprintln!("[AgentDesk] Launching agent in: {}", dir_str);
    eprintln!("[AgentDesk] Command: {}", full_cmd);
    eprintln!("[AgentDesk] Script:\n{}", script);

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    eprintln!("[AgentDesk] osascript exit: {}", output.status);
    if !output.stdout.is_empty() {
        eprintln!("[AgentDesk] stdout: {}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprintln!("[AgentDesk] stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript failed: {}", stderr));
    }

    Ok(())
}

/// Launch an arbitrary shell command line in a new terminal window,
/// after first `cd`ing to `project_dir`. Unlike `launch_agent`, the
/// caller is fully responsible for the command string — typically a
/// wrapper that sets up env vars, writes sentinel files, and then
/// execs the agent. Used by the workflow engine (module 5) so it can
/// inject `AGENTDESK_LAUNCH_TOKEN` and pid/exit tracking.
///
/// The command string is **not** validated or escaped by this
/// function: the engine controls it in full and must itself use
/// `quoted form of` for any paths it interpolates.
pub fn launch_wrapped_command(
    project_dir: &Path,
    full_cmd: &str,
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
    let script = if is_iterm_installed() {
        build_iterm_script(&dir_str, full_cmd)
    } else {
        build_terminal_script(&dir_str, full_cmd)
    };
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
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
        delay 0.5
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

/// Best-effort: bring the terminal window whose cwd matches `cwd` to
/// the front. We don't know which iTerm2 / Terminal session hosts a
/// given agent, so we just activate the app — the user can Cmd+` to
/// cycle sessions from there. This is good enough for the command
/// palette's "jump to running agent" action.
///
/// Returns `Err` if the terminal app isn't installed or osascript
/// failed. Callers should treat errors as non-fatal.
pub fn focus_terminal_for_cwd(_cwd: &Path) -> Result<(), String> {
    let app = if is_iterm_installed() { "iTerm2" } else { "Terminal" };
    let script = format!(r#"tell application "{}" to activate"#, app);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("osascript spawn: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
