pub const GLOBAL_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Helvetica Neue", sans-serif;
    background-color: #f5f5f7; color: #1d1d1f; font-size: 13px;
    -webkit-font-smoothing: antialiased; line-height: 1.45;
}
.app-container { display: flex; height: 100vh; width: 100vw; }

/* ══════════════════════════════
   SIDEBAR — macOS Settings style
   ══════════════════════════════ */
.sidebar {
    width: 240px; min-width: 240px;
    background-color: rgba(244, 244, 246, 0.95);
    backdrop-filter: blur(20px); -webkit-backdrop-filter: blur(20px);
    border-right: 0.5px solid #c7c7cc;
    display: flex; flex-direction: column;
    padding: 10px 0; user-select: none;
}
.sidebar-section-label {
    font-size: 11px; font-weight: 600; color: #86868b;
    padding: 16px 20px 6px; letter-spacing: 0.2px;
}

/* Home item — prominent top card */
.home-item {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 12px; margin: 2px 10px 6px;
    border-radius: 8px; cursor: pointer;
    transition: background 0.1s;
}
.home-item:hover { background: rgba(0,0,0,0.04); }
.home-item.selected { background: #007aff; }
.home-item.selected .home-name,
.home-item.selected .project-meta,
.home-item.selected .project-meta span { color: #fff !important; }
.home-item.selected .home-icon-box { background: rgba(255,255,255,0.2); }
.home-item.selected .agent-badge { background: rgba(255,255,255,0.25); }
.home-icon-box {
    width: 28px; height: 28px; border-radius: 6px;
    background: linear-gradient(135deg, #5ac8fa, #007aff);
    display: flex; align-items: center; justify-content: center;
    font-size: 14px; flex-shrink: 0;
}
.home-info { flex: 1; min-width: 0; }
.home-name { font-weight: 600; font-size: 13px; color: #1d1d1f; }

/* Project list */
.project-list { flex: 1; overflow-y: auto; }
.project-item {
    display: flex; align-items: center; gap: 10px;
    padding: 7px 12px; margin: 1px 10px;
    border-radius: 8px; cursor: pointer;
    transition: background 0.1s;
}
.project-item:hover { background: rgba(0,0,0,0.04); }
.project-item.selected { background: #007aff; }
.project-item.selected .project-name,
.project-item.selected .project-meta,
.project-item.selected .project-meta span { color: #fff !important; }
.project-item.selected .project-icon-box { background: rgba(255,255,255,0.2); }
.project-item.selected .agent-badge { background: rgba(255,255,255,0.25); color: #fff; }
.project-item.selected .custom-badge,
.project-item.selected .nick-badge { background: rgba(255,255,255,0.2); color: #fff; }

.project-icon-box {
    width: 28px; height: 28px; border-radius: 6px;
    background: linear-gradient(135deg, #34c759, #30d158);
    display: flex; align-items: center; justify-content: center;
    font-size: 13px; flex-shrink: 0; color: #fff; font-weight: 700;
}
.project-icon-box.orange { background: linear-gradient(135deg, #ff9500, #ff6b00); }
.project-icon-box.purple { background: linear-gradient(135deg, #af52de, #5856d6); }
.project-icon-box.pink { background: linear-gradient(135deg, #ff2d55, #ff375f); }
.project-icon-box.teal { background: linear-gradient(135deg, #5ac8fa, #64d2ff); }
.project-item-info { flex: 1; min-width: 0; }
.project-name-row { display: flex; align-items: center; gap: 5px; }
.project-name { font-weight: 600; font-size: 13px; color: #1d1d1f; }
.project-meta { display: flex; gap: 5px; font-size: 10px; color: #aeaeb2; margin-top: 1px; align-items: center; }
.agent-badge {
    background: #34c759; color: #fff;
    padding: 0 5px; border-radius: 7px; font-size: 9px; font-weight: 700; line-height: 15px;
}
.custom-badge {
    font-size: 9px; font-weight: 500; color: #ff9500;
    background: #fff3e0; padding: 0 4px; border-radius: 3px; line-height: 15px;
}
.nick-badge {
    font-size: 9px; font-weight: 500; color: #86868b;
    background: #ebebed; padding: 0 4px; border-radius: 3px; line-height: 15px;
}
.nickname-input {
    width: 100%; padding: 3px 6px; font-size: 13px; font-weight: 600;
    border: 1.5px solid #007aff; border-radius: 5px; outline: none;
    background: #fff; color: #1d1d1f;
}
.sidebar-add-btn {
    background: none; border: none; cursor: pointer; font-size: 18px;
    color: #007aff; padding: 0 4px; transition: opacity 0.1s;
}
.sidebar-add-btn:hover { opacity: 0.7; }

/* ══════════════════════════════
   MAIN PANEL — right side
   ══════════════════════════════ */
.main-panel { flex: 1; overflow-y: auto; padding: 28px 36px; background: #fff; }
.page-header { margin-bottom: 28px; }
.page-header-info h1 { font-size: 26px; font-weight: 700; color: #1d1d1f; margin-bottom: 6px; }
.page-header-info .path { font-size: 11px; color: #86868b; word-break: break-all; }
.page-header-actions { margin-top: 12px; }

/* ── Section — macOS grouped rows ── */
.section { margin-bottom: 28px; }
.section-label {
    font-size: 13px; font-weight: 600; color: #86868b;
    padding: 0 0 8px; letter-spacing: 0.1px;
}

/* Grouped card — rounded container for rows */
.grouped-card {
    background: #fff; border: 0.5px solid #d1d1d6;
    border-radius: 10px; overflow: hidden;
}
.grouped-row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 11px 16px; min-height: 44px;
    border-bottom: 0.5px solid #e5e5ea;
}
.grouped-row:last-child { border-bottom: none; }
.grouped-row:hover { background: rgba(0,0,0,0.015); }
.grouped-row-clickable { cursor: pointer; }
.grouped-row-clickable:hover { background: rgba(0,0,0,0.03); }

.row-label { font-size: 13px; color: #1d1d1f; font-weight: 400; }
.row-label-bold { font-size: 13px; color: #1d1d1f; font-weight: 600; }
.row-value { font-size: 13px; color: #86868b; display: flex; align-items: center; gap: 6px; }
.row-sub { font-size: 11px; color: #86868b; margin-top: 2px; }
.row-content { flex: 1; min-width: 0; }

/* Status indicators */
.status-dot {
    width: 8px; height: 8px; border-radius: 50%;
    display: inline-block; margin-right: 6px; flex-shrink: 0;
}
.status-dot.busy { background: #ff9500; animation: pulse 1.5s ease-in-out infinite; }
.status-dot.idle { background: #34c759; }
@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
}
.status-tag {
    font-size: 10px; font-weight: 600; padding: 0 6px;
    border-radius: 4px; line-height: 16px;
}
.status-tag.busy { background: #fff3e0; color: #ff9500; }
.status-tag.idle { background: #e8f5e9; color: #34c759; }

.subagent-row { background: #f9f9fb; }
.sub-badge {
    font-size: 9px; color: #86868b; background: #ebebed;
    padding: 0 5px; border-radius: 3px; line-height: 15px;
}

/* ── Stats row ── */
.stats-grid {
    display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 10px; margin-bottom: 16px;
}
.stat-card {
    background: #f9f9fb; border: 0.5px solid #e5e5ea;
    border-radius: 10px; padding: 14px; text-align: center;
}
.stat-value { font-size: 26px; font-weight: 700; line-height: 1.1; }
.stat-value.blue { color: #007aff; }
.stat-value.green { color: #34c759; }
.stat-value.orange { color: #ff9500; }
.stat-label { font-size: 11px; color: #86868b; margin-top: 4px; }

/* ── Buttons ── */
.btn {
    padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer;
    font-size: 13px; font-weight: 500; transition: all 0.1s;
    display: inline-flex; align-items: center; gap: 4px;
}
.btn-primary { background: #007aff; color: #fff; }
.btn-primary:hover { background: #0066d6; }
.btn-primary:disabled { background: #b0b0b5; cursor: not-allowed; }
.btn-sm { padding: 4px 10px; font-size: 11px; }
.btn-focus-terminal {
    background: transparent; border: none; cursor: pointer;
    font-size: 11px; font-weight: 500; color: #007aff; padding: 2px 0;
}
.btn-focus-terminal:hover { text-decoration: underline; }

/* ── Empty state ── */
.empty-state { text-align: center; color: #86868b; padding: 80px 20px; }
.empty-state h2 { color: #1d1d1f; font-size: 20px; font-weight: 600; margin-bottom: 8px; }

/* ── Summary ── */
.summary-text { font-size: 13px; color: #3a3a3c; line-height: 1.6; white-space: pre-line; }

/* ── Session expandable ── */
.session-arrow {
    font-size: 10px; color: #86868b; width: 16px; flex-shrink: 0;
    text-align: center; transition: transform 0.15s;
}
.session-header-row {
    display: flex; align-items: center; gap: 4px; cursor: pointer; flex: 1;
}
.session-time { font-size: 11px; color: #86868b; white-space: nowrap; width: 90px; flex-shrink: 0; }
.session-branch {
    font-size: 10px; color: #007aff; background: #eef4ff;
    padding: 0 5px; border-radius: 3px; flex-shrink: 0;
}
.session-msgs { font-size: 10px; color: #aeaeb2; white-space: nowrap; flex-shrink: 0; }
.session-preview-text {
    font-size: 12px; color: #636366; margin-top: 4px; padding-left: 20px; line-height: 1.4;
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
}
.session-detail {
    margin-top: 10px; padding-top: 10px; border-top: 0.5px solid #e5e5ea;
    max-height: 500px; overflow-y: auto;
}
.msg-bubble { margin-bottom: 8px; padding: 8px 12px; border-radius: 8px; font-size: 13px; line-height: 1.5; }
.msg-user { background: #007aff0a; border-left: 3px solid #007aff; }
.msg-assistant { background: #f2f2f7; border-left: 3px solid #34c759; }
.msg-header { display: flex; justify-content: space-between; margin-bottom: 3px; }
.msg-role { font-weight: 600; font-size: 11px; color: #3a3a3c; }
.msg-time { font-size: 10px; color: #aeaeb2; }
.msg-content { font-size: 12px; color: #1d1d1f; white-space: pre-wrap; word-break: break-word; }

/* Grouped row variant for sessions */
.grouped-row.session-expanded { background: #f9f9fb; }

/* ── Doc list ── */
.doc-icon { font-size: 15px; flex-shrink: 0; width: 20px; text-align: center; }
.doc-name { font-weight: 500; font-size: 13px; color: #007aff; }
.doc-path { font-size: 11px; color: #86868b; }

/* ── Dialog ── */
.dialog-overlay {
    position: fixed; top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.25); backdrop-filter: blur(4px); -webkit-backdrop-filter: blur(4px);
    display: flex; align-items: center; justify-content: center; z-index: 100;
}
.dialog {
    background: #f6f6f6; border: 0.5px solid #d1d1d6; border-radius: 12px;
    padding: 22px; width: 380px; max-width: 90vw;
    box-shadow: 0 12px 36px rgba(0,0,0,0.12);
}
.dialog h2 { margin-bottom: 16px; font-size: 16px; font-weight: 700; color: #1d1d1f; }
.form-group { margin-bottom: 12px; }
.form-group label { display: block; margin-bottom: 4px; font-weight: 500; font-size: 12px; color: #3a3a3c; }
.form-select {
    width: 100%; padding: 6px 10px;
    background: #fff; border: 0.5px solid #d1d1d6; border-radius: 6px;
    color: #1d1d1f; font-size: 13px; -webkit-appearance: menulist;
}
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
.btn-ghost {
    background: #fff; color: #1d1d1f; border: 0.5px solid #d1d1d6;
    padding: 6px 14px; border-radius: 6px; cursor: pointer; font-size: 13px; font-weight: 500;
}
.btn-ghost:hover { background: #f2f2f7; }
.warning-box {
    background: #fef7e0; border: 0.5px solid #f5c518; border-radius: 8px;
    padding: 10px 12px; margin-bottom: 12px; font-size: 12px;
}
.warning-title { color: #8a6d00; font-weight: 600; font-size: 12px; }
.warning-text { color: #6b5300; margin-top: 3px; font-size: 11px; }

/* ── Context menu ── */
.ctx-backdrop { position: fixed; top: 0; left: 0; right: 0; bottom: 0; z-index: 199; }
.ctx-menu {
    position: fixed; z-index: 200;
    background: rgba(252,252,252,0.95); backdrop-filter: blur(20px); -webkit-backdrop-filter: blur(20px);
    border: 0.5px solid #c7c7cc; border-radius: 8px; padding: 4px;
    min-width: 170px;
    box-shadow: 0 8px 30px rgba(0,0,0,0.12), 0 0 0 0.5px rgba(0,0,0,0.04);
}
.ctx-menu-item {
    padding: 5px 10px; font-size: 12px; cursor: pointer;
    border-radius: 4px; display: flex; align-items: center; gap: 6px;
}
.ctx-menu-item:hover { background: #007aff; color: #fff; }
.ctx-menu-danger { color: #ff3b30; }
.ctx-menu-danger:hover { background: #ff3b30; color: #fff; }

/* ── Sidebar footer ── */
.sidebar-footer {
    border-top: 0.5px solid #d1d1d6; padding: 8px 10px;
    flex-shrink: 0;
}
.sidebar-footer-btn {
    display: flex; align-items: center; gap: 6px;
    padding: 7px 12px; border-radius: 8px; cursor: pointer;
    font-size: 13px; color: #86868b; transition: background 0.1s;
}
.sidebar-footer-btn:hover { background: rgba(0,0,0,0.04); }

/* ── Remove button in settings ── */
.btn-remove {
    background: transparent; border: none; cursor: pointer;
    font-size: 12px; color: #ff3b30; font-weight: 500;
    padding: 2px 0;
}
.btn-remove:hover { text-decoration: underline; }
.btn-ghost {
    background: #fff; color: #1d1d1f; border: 0.5px solid #d1d1d6;
    padding: 6px 14px; border-radius: 6px; cursor: pointer; font-size: 13px; font-weight: 500;
}
.btn-ghost:hover { background: #f2f2f7; }
"#;
