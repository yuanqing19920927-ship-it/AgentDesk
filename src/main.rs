use dioxus::prelude::*;

mod models;
mod services;
mod ui;

use ui::app_shell::AppShell;

fn main() {
    // Kill any leftover island overlay
    let _ = std::process::Command::new("pkill").args(["-f", "island-overlay"]).output();

    let cfg = dioxus::desktop::Config::new()
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_title("AgentDesk")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 720.0))
                .with_always_on_top(false),
        );

    let _cleanup = IslandCleanup;
    LaunchBuilder::desktop().with_cfg(cfg).launch(AppShell);
}

struct IslandCleanup;
impl Drop for IslandCleanup {
    fn drop(&mut self) {
        let _ = std::process::Command::new("pkill").args(["-f", "island-overlay"]).output();
        let _ = std::fs::remove_file(
            dirs::home_dir().unwrap_or_default().join(".agentdesk").join("island_state.json")
        );
    }
}
