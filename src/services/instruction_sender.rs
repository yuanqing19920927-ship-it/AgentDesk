//! Module 12.2 — Quick instruction send.
//!
//! Delivers a short text instruction (usually a slash command) to a
//! running Agent's terminal session. The design doc (section 12) lays
//! out a three-step safety model that this module implements end to
//! end:
//!
//! 1. **PID liveness** — the Agent process must still be running, or
//!    we'd silently write text into whatever tab the user has reused.
//! 2. **cwd stability** — we re-read the process cwd via `lsof` and
//!    compare it to the cwd we saw when the row was rendered. If the
//!    user `cd`'d away, that's a "reused tab" signal and we refuse.
//! 3. **Atomic dispatch** — the tty lookup and `write text` command
//!    run in a single `osascript` invocation so the check→send gap is
//!    a function of AppleScript event latency, not of two round-trips
//!    back to Rust.
//!
//! There's one more guardrail enforced by the caller UI: free-form
//! text must go through a second-confirmation step, while a fixed
//! whitelist of slash commands can be sent directly. The whitelist
//! lives in `SLASH_WHITELIST` below.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Slash commands that are always safe to send without a second
/// confirmation prompt. These are read-only or clearly-scoped actions
/// that the user is already familiar with from the Agent REPL.
///
/// Kept intentionally small — additions need a design review because
/// each entry is, in effect, a trusted path from one button click to
/// arbitrary behaviour inside the REPL.
pub const SLASH_WHITELIST: &[&str] = &[
    "/help",
    "/status",
    "/version",
    "/clear",
    "/compact",
    "/commit",
    "/review-pr",
    "/cost",
    "/model",
    "/config",
];

/// Is this text a whitelist-approved slash command?
///
/// We match on the first whitespace-delimited token so `/commit -m "foo"`
/// is also accepted if `/commit` is whitelisted — arguments are still
/// shell-escaped later so this doesn't let the user smuggle shell
/// metacharacters past the whitelist.
pub fn is_whitelisted(text: &str) -> bool {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return false;
    }
    let head = trimmed.split_whitespace().next().unwrap_or("");
    SLASH_WHITELIST.iter().any(|cmd| *cmd == head)
}

/// Read the current working directory of a live process via `lsof`.
///
/// `lsof -a -p <pid> -d cwd -Fn` prints one record per file descriptor
/// using `F`ield output. The cwd entry has `fcwd` (fd) and `n<path>`
/// (name) lines. We're only interested in the `n` line for that fd.
///
/// Returns `None` if the process is gone or lsof produces no cwd entry.
pub fn read_pid_cwd(pid: u32) -> Option<PathBuf> {
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('n') {
            return Some(PathBuf::from(rest));
        }
    }
    None
}

/// Is the given PID still alive? `kill -0` returns success for a
/// live process and "no such process" for a reaped one.
pub fn is_pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// All reasons an instruction can be refused. Exposed so the UI can
/// render tailored error messages and — more importantly — refuse the
/// dispatch rather than silently doing nothing on the terminal side.
#[derive(Debug, Clone)]
pub enum SendError {
    Empty,
    ContainsControlChars,
    ProcessGone,
    CwdMoved { expected: PathBuf, actual: Option<PathBuf> },
    NoTty,
    Osascript(String),
    SessionNotFound,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Empty => write!(f, "指令为空"),
            SendError::ContainsControlChars => write!(f, "指令包含不可见控制字符，已拒绝"),
            SendError::ProcessGone => write!(f, "目标 Agent 进程已退出"),
            SendError::CwdMoved { expected, actual } => {
                let actual = actual.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "<未知>".into());
                write!(f, "工作目录已变更（期望 {}，实际 {}）", expected.display(), actual)
            }
            SendError::NoTty => write!(f, "Agent 没有可用的 tty，无法定位终端会话"),
            SendError::Osascript(m) => write!(f, "osascript 调用失败: {}", m),
            SendError::SessionNotFound => write!(f, "未找到匹配 tty 的终端会话"),
        }
    }
}

/// Send `instruction` to the Agent identified by `pid` + `tty`,
/// validating that nothing about the target session has changed since
/// the caller last saw it.
///
/// `expected_cwd` is the cwd the Agent was reported to be in at render
/// time. We re-read it here and refuse if it no longer matches, which
/// is the main line of defence against iTerm tab reuse.
///
/// `instruction` is passed verbatim (no shell wrapping) — the receiver
/// is the Agent REPL, not `bash`. Newlines would submit the prompt
/// mid-command so we collapse them to spaces; other control characters
/// are rejected to avoid terminal escape-sequence injection.
pub fn send_instruction(
    pid: u32,
    tty: Option<&str>,
    expected_cwd: &Path,
    instruction: &str,
) -> Result<(), SendError> {
    let cleaned = sanitize_instruction(instruction)?;

    if !is_pid_alive(pid) {
        return Err(SendError::ProcessGone);
    }
    // cwd check sits as close to the dispatch call as possible to
    // minimise the window between "we verified" and "we sent". We
    // still can't close the window completely without OS support for
    // transactional writes into a pty we don't own — that's a P4
    // item.
    let actual_cwd = read_pid_cwd(pid);
    let ok = actual_cwd
        .as_ref()
        .map(|c| paths_equivalent(c, expected_cwd))
        .unwrap_or(false);
    if !ok {
        return Err(SendError::CwdMoved {
            expected: expected_cwd.to_path_buf(),
            actual: actual_cwd,
        });
    }

    let tty = tty.ok_or(SendError::NoTty)?;
    let tty_device = format!("/dev/tty{}", tty);

    let script = build_iterm_send_script(&tty_device, &cleaned);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| SendError::Osascript(e.to_string()))?;

    if !output.status.success() {
        return Err(SendError::Osascript(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.contains("not found") {
        return Err(SendError::SessionNotFound);
    }
    Ok(())
}

/// Reject blank and control-char-bearing instructions. Newlines are
/// replaced (not rejected) because trailing whitespace is common in
/// copy-paste and users generally expect "send" to strip them.
fn sanitize_instruction(raw: &str) -> Result<String, SendError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SendError::Empty);
    }
    // Any embedded control char other than space is suspicious — ANSI
    // escape sequences start with \x1b and could re-target the cursor
    // or paint arbitrary text.
    let has_ctrl = trimmed.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t');
    if has_ctrl {
        return Err(SendError::ContainsControlChars);
    }
    // Collapse newlines / tabs to single spaces so a multi-line paste
    // is delivered as one prompt rather than submitting mid-way.
    let collapsed: String = trimmed
        .chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect();
    Ok(collapsed)
}

/// Canonicalize both paths before comparing so `/tmp/foo` vs
/// `/private/tmp/foo` doesn't trip us up on macOS.
fn paths_equivalent(a: &Path, b: &Path) -> bool {
    let ca = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let cb = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    ca == cb
}

/// Build the atomic-dispatch AppleScript. We iterate every iTerm2
/// session, match by tty, and `write text` in the same block so
/// AppleScript has to serialise the lookup and the send.
///
/// The instruction is double-quoted into the script — we escape
/// backslashes and double quotes to avoid breaking out of the
/// AppleScript string literal. No other AppleScript interpretation is
/// performed on the contents.
fn build_iterm_send_script(tty_device: &str, instruction: &str) -> String {
    let esc_instr = applescript_escape(instruction);
    let esc_tty = applescript_escape(tty_device);
    format!(
        r#"tell application "iTerm2"
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                if tty of s is "{tty}" then
                    tell s
                        write text "{instr}"
                    end tell
                    return "sent"
                end if
            end repeat
        end repeat
    end repeat
    return "not found"
end tell"#,
        tty = esc_tty,
        instr = esc_instr,
    )
}

fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_accepts_bare_command() {
        assert!(is_whitelisted("/commit"));
        assert!(is_whitelisted("  /commit  "));
    }

    #[test]
    fn whitelist_accepts_command_with_args() {
        assert!(is_whitelisted("/commit -m message"));
    }

    #[test]
    fn whitelist_rejects_freetext() {
        assert!(!is_whitelisted("do a thing"));
        assert!(!is_whitelisted("/unknown"));
    }

    #[test]
    fn sanitize_collapses_newlines() {
        let out = sanitize_instruction("hello\nworld").unwrap();
        assert_eq!(out, "hello world");
    }

    #[test]
    fn sanitize_rejects_empty() {
        assert!(matches!(sanitize_instruction("   "), Err(SendError::Empty)));
    }

    #[test]
    fn sanitize_rejects_escape_chars() {
        assert!(matches!(
            sanitize_instruction("hi\x1b[31mRED"),
            Err(SendError::ContainsControlChars)
        ));
    }

    #[test]
    fn applescript_escape_handles_quotes() {
        assert_eq!(applescript_escape(r#"hello "world""#), r#"hello \"world\""#);
    }
}
