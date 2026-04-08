use std::process::Command;

/// Send a macOS notification via osascript
pub fn send_notification(title: &str, message: &str) {
    let script = format!(
        r#"display notification "{}" with title "{}""#,
        escape(message),
        escape(title),
    );
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
