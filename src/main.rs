use dioxus::prelude::*;
use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;

mod models;
mod services;
mod ui;

use ui::app_shell::AppShell;

fn main() {
    let _ = std::process::Command::new("pkill").args(["-f", "island-overlay"]).output();

    let cfg = dioxus::desktop::Config::new()
        .with_close_behaviour(dioxus::desktop::WindowCloseBehaviour::LastWindowHides)
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_title("")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 720.0))
                .with_always_on_top(false)
                // macOS-style hidden title bar: traffic lights float over the
                // content, sidebar extends under them. Matches System Settings.
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true),
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
