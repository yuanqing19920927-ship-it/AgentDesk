use dioxus::prelude::*;

mod models;
mod services;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div {
            style: "padding: 20px; font-family: -apple-system, BlinkMacSystemFont, sans-serif;",
            h1 { "AgentDesk" }
            p { "Loading..." }
        }
    }
}
