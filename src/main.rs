mod models;
mod services;
mod ui;

use ui::app_shell::AppShell;

fn main() {
    dioxus::launch(AppShell);
}
