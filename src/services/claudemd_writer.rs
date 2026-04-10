//! Project CLAUDE.md integration.
//!
//! When project-local storage is chosen, we drop a short marker section
//! into the project's `CLAUDE.md` so that in-project Agents know where
//! to find the structured memory without having to spelunk.
//!
//! The section is idempotent: on each scan we look for our marker
//! comments (`<!-- agentdesk-memory:start -->` / `<!-- agentdesk-memory:end -->`),
//! replace the content between them, and leave everything else in
//! `CLAUDE.md` untouched. If no marker is present we append one at the
//! end. Never writes when `CLAUDE.md` would be freshly created **and**
//! the caller is in user-fallback mode — the `memory_indexer` only
//! calls us in project-local mode.

use std::fs;
use std::path::Path;

const START_MARKER: &str = "<!-- agentdesk-memory:start -->";
const END_MARKER: &str = "<!-- agentdesk-memory:end -->";

pub fn ensure_memory_section(project_root: &Path) -> Result<(), String> {
    let claude_md = project_root.join("CLAUDE.md");
    let existing = fs::read_to_string(&claude_md).unwrap_or_default();
    let new_content = merge_section(&existing, &memory_block());
    if new_content == existing {
        return Ok(());
    }
    fs::write(&claude_md, new_content).map_err(|e| format!("写入 CLAUDE.md 失败: {}", e))
}

fn memory_block() -> String {
    format!(
        "{}\n## Project Memory (AgentDesk)\n\n- 项目记忆索引: `.agentdesk/index.json`\n- 结构化记忆: `.agentdesk/memory.md`\n- 会话摘要目录: `.agentdesk/sessions/`\n- 使用方式: 需要历史上下文时，先读取 `.agentdesk/memory.md` 获取概览，再通过 `.agentdesk/index.json` 定位详细记录。\n{}\n",
        START_MARKER, END_MARKER
    )
}

/// Replace the section between our markers in `existing`, or append the
/// block at the end if no markers are found. Preserves trailing newlines.
fn merge_section(existing: &str, block: &str) -> String {
    if let (Some(start), Some(end)) = (existing.find(START_MARKER), existing.find(END_MARKER)) {
        if end > start {
            let end_with_marker = end + END_MARKER.len();
            let mut out = String::new();
            out.push_str(&existing[..start]);
            out.push_str(block.trim_end_matches('\n'));
            out.push_str(&existing[end_with_marker..]);
            return out;
        }
    }
    let mut out = existing.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(block);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_when_no_markers() {
        let input = "# Project\n\nSome content.\n";
        let result = merge_section(input, "<!-- agentdesk-memory:start -->\nA\n<!-- agentdesk-memory:end -->\n");
        assert!(result.contains("<!-- agentdesk-memory:start -->"));
        assert!(result.starts_with("# Project"));
    }

    #[test]
    fn replaces_existing_marker_block() {
        let input = "# Project\n\n<!-- agentdesk-memory:start -->\nOLD\n<!-- agentdesk-memory:end -->\n\nAfter.\n";
        let new_block = "<!-- agentdesk-memory:start -->\nNEW\n<!-- agentdesk-memory:end -->";
        let result = merge_section(input, new_block);
        assert!(result.contains("NEW"));
        assert!(!result.contains("OLD"));
        assert!(result.contains("After."));
    }
}
