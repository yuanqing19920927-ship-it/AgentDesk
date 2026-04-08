use crate::models::{Agent, AgentStatus};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};

fn state_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".agentdesk").join("island_state.json")
}

fn overlay_binary() -> PathBuf {
    // Look for the binary next to the main executable, then in helpers/
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_dir = exe.parent().unwrap_or(std::path::Path::new("."));

    // Check next to binary
    let beside = exe_dir.join("island-overlay");
    if beside.exists() { return beside; }

    // Check in project helpers/
    let helpers = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("helpers").join("island-overlay");
    if helpers.exists() { return helpers; }

    // Fallback
    beside
}

/// Write current agent states to the shared JSON file
pub fn write_island_state(agents: &[Agent]) {
    let entries: Vec<serde_json::Value> = agents.iter()
        .filter(|a| !a.is_subagent)
        .map(|a| {
            let project = a.cwd.as_ref()
                .and_then(|c| c.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            serde_json::json!({
                "pid": a.pid,
                "type": a.agent_type.label(),
                "status": match a.status { AgentStatus::Busy => "busy", AgentStatus::Idle => "idle" },
                "cpu": a.cpu_percent,
                "project": project,
            })
        })
        .collect();

    let dir = dirs::home_dir().unwrap_or_default().join(".agentdesk");
    let _ = fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        let _ = fs::write(state_path(), json);
    }
}

/// Start the island overlay process
pub fn start_overlay() -> Option<Child> {
    let bin = overlay_binary();
    if !bin.exists() {
        eprintln!("[AgentDesk] island-overlay binary not found at {:?}", bin);
        return None;
    }
    Command::new(&bin).spawn().ok()
}

/// Stop the island overlay process
pub fn stop_overlay(child: &mut Option<Child>) {
    if let Some(ref mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
    }
    *child = None;
    // Clean up state file
    let _ = fs::remove_file(state_path());
}
