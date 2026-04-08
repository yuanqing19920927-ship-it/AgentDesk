use dioxus::prelude::*;
use tao::platform::macos::WindowBuilderExtMacOS;

mod models;
mod services;
mod ui;

use ui::app_shell::AppShell;

fn main() {
    let _ = std::process::Command::new("pkill").args(["-f", "island-overlay"]).output();

    let window = dioxus::desktop::WindowBuilder::new()
        .with_title("")
        .with_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 720.0))
        .with_always_on_top(false)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_title_hidden(true);

    let cfg = dioxus::desktop::Config::new().with_window(window);
    LaunchBuilder::desktop().with_cfg(cfg).launch(AppShell);
}
