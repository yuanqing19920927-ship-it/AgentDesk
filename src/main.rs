use dioxus::prelude::*;

mod models;
mod services;
mod ui;

use ui::app_shell::AppShell;

fn main() {
    let cfg = dioxus::desktop::Config::new()
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_title("AgentDesk")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 720.0))
                .with_always_on_top(false),
        );
    LaunchBuilder::desktop().with_cfg(cfg).launch(AppShell);
}
