//! Module 12.2 — Quick instruction dialog.
//!
//! Modal that collects a short text instruction and ships it to a
//! specific running Agent via `instruction_sender`. Two send paths:
//!
//! * **Whitelist slash command** (`/commit`, `/review-pr`, …) — one
//!   click sends immediately. The whitelist lives in
//!   `instruction_sender::SLASH_WHITELIST`.
//! * **Free text** — first click shows a "二次确认" prompt with the
//!   exact text that will be sent; second click dispatches.
//!
//! Keyboard: ⌘+Enter sends (goes through the same two-stage flow for
//! free text); Esc closes.

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::services::instruction_sender::{self, SendError, SLASH_WHITELIST};

/// Target agent descriptor — we only need the subset that
/// `instruction_sender` cares about, so the dialog can stay
/// independent of the full `Agent` model.
#[derive(Clone, Debug, PartialEq)]
pub struct InstructionTarget {
    pub pid: u32,
    pub tty: Option<String>,
    pub cwd: PathBuf,
    pub label: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct InstructionDialogProps {
    pub target: InstructionTarget,
    pub on_close: EventHandler<()>,
}

#[component]
pub fn InstructionDialog(props: InstructionDialogProps) -> Element {
    let mut instruction = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    // `pending_confirm` holds the free-text string awaiting the second
    // confirmation click. `None` means no pending send.
    let mut pending_confirm = use_signal(|| None::<String>);
    let mut success = use_signal(|| false);

    // Snapshot target fields into owned values so the async closure
    // below can move them without borrowing props.
    let target = props.target.clone();

    // Dispatch a validated instruction. Called from both the slash
    // chip clicks (which bypass the second confirmation) and the
    // free-text confirm path.
    let send_now = {
        let target = target.clone();
        move |text: String| {
            error.set(None);
            success.set(false);
            let pid = target.pid;
            let tty = target.tty.clone();
            let cwd = target.cwd.clone();
            let result = instruction_sender::send_instruction(
                pid,
                tty.as_deref(),
                &cwd,
                &text,
            );
            match result {
                Ok(()) => {
                    success.set(true);
                    pending_confirm.set(None);
                    instruction.set(String::new());
                }
                Err(e) => {
                    error.set(Some(display_error(&e)));
                }
            }
        }
    };

    // Click the primary "发送" button. Whitelist entries go straight
    // through; free text goes through the confirm step.
    //
    // Returns a zero-arg closure so the caller can wrap it in
    // whatever event-specific shim onclick / onkeydown expect.
    let primary_click = {
        let mut send_now = send_now.clone();
        move || {
            let text = instruction().trim().to_string();
            if text.is_empty() {
                error.set(Some("指令为空".into()));
                return;
            }
            if instruction_sender::is_whitelisted(&text) {
                send_now(text);
            } else if pending_confirm().as_deref() == Some(text.as_str()) {
                // Same text confirmed — actually send.
                send_now(text);
            } else {
                // First click on free text — stage for confirmation.
                pending_confirm.set(Some(text));
                error.set(None);
            }
        }
    };

    // Click a whitelist chip — immediate send.
    let chip_click = {
        let mut send_now = send_now.clone();
        move |cmd: &'static str| {
            instruction.set(cmd.to_string());
            pending_confirm.set(None);
            send_now(cmd.to_string());
        }
    };

    let title_label = props.target.label.clone();
    let cwd_display = props.target.cwd.display().to_string();
    let pid = props.target.pid;

    rsx! {
        div {
            class: "dialog-overlay",
            onclick: move |_| props.on_close.call(()),
            div {
                class: "dialog instruction-dialog",
                onclick: move |e| e.stop_propagation(),
                onkeydown: move |e: KeyboardEvent| {
                    if e.key() == Key::Escape {
                        e.prevent_default();
                        props.on_close.call(());
                    }
                },
                h2 { "快速指令 · {title_label}" }
                div { class: "instr-meta",
                    "PID {pid} · {cwd_display}"
                }

                // Whitelist chips — one-click send.
                div { class: "instr-section-label", "常用指令" }
                div { class: "instr-chips",
                    {SLASH_WHITELIST.iter().map(|cmd| {
                        let cmd = *cmd;
                        let mut chip_click = chip_click.clone();
                        rsx! {
                            button {
                                key: "{cmd}",
                                class: "instr-chip",
                                onclick: move |_| chip_click(cmd),
                                "{cmd}"
                            }
                        }
                    })}
                }

                // Free-text entry.
                div { class: "instr-section-label", "自定义指令" }
                textarea {
                    class: "instr-textarea",
                    autofocus: true,
                    placeholder: "输入指令…  ⌘+Enter 发送",
                    value: "{instruction()}",
                    oninput: move |e| {
                        instruction.set(e.value());
                        // Any edit invalidates an in-flight confirmation.
                        pending_confirm.set(None);
                    },
                    onkeydown: {
                        let mut primary_click = primary_click.clone();
                        move |e: KeyboardEvent| {
                            let mods = e.modifiers();
                            let cmd = mods.meta() || mods.ctrl();
                            if cmd && e.key() == Key::Enter {
                                e.prevent_default();
                                primary_click();
                            }
                        }
                    },
                }

                // Pending-confirm banner — appears after the first
                // click on free text so the user sees the exact string
                // they're about to dispatch to the REPL.
                if let Some(pending) = pending_confirm() {
                    div { class: "instr-confirm",
                        div { class: "instr-confirm-title", "⚠ 再次确认发送" }
                        div { class: "instr-confirm-body", "{pending}" }
                    }
                }

                if let Some(err) = error() {
                    div { class: "instr-error", "{err}" }
                }
                if success() {
                    div { class: "instr-success", "✓ 已发送" }
                }

                div { class: "dialog-actions",
                    button {
                        class: "btn-ghost",
                        onclick: move |_| props.on_close.call(()),
                        "关闭"
                    }
                    button {
                        class: "btn btn-primary",
                        onclick: {
                            let mut primary_click = primary_click.clone();
                            move |_| primary_click()
                        },
                        {
                            if pending_confirm().is_some() { "确认发送" }
                            else if instruction_sender::is_whitelisted(instruction().trim()) { "发送" }
                            else { "发送 (需确认)" }
                        }
                    }
                }
            }
        }
    }
}

fn display_error(e: &SendError) -> String {
    // Reuse the Display impl from the service — it already produces
    // user-facing Chinese strings.
    e.to_string()
}
