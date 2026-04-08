pub const GLOBAL_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Helvetica Neue", sans-serif;
    background-color: #f5f5f7; color: #1d1d1f; font-size: 13px;
    -webkit-font-smoothing: antialiased;
    line-height: 1.5;
}
.app-container { display: flex; height: 100vh; width: 100vw; }

/* ── Sidebar ── */
.sidebar {
    width: 260px; min-width: 260px;
    background-color: rgba(246, 246, 246, 0.92);
    backdrop-filter: blur(20px); -webkit-backdrop-filter: blur(20px);
    border-right: 1px solid #d1d1d6;
    display: flex; flex-direction: column; overflow-y: auto;
    user-select: none;
}
.sidebar-title {
    font-size: 15px; font-weight: 700; color: #1d1d1f;
    padding: 20px 16px 4px;
}
.sidebar-section-label {
    font-size: 11px; font-weight: 600; color: #86868b;
    text-transform: uppercase; letter-spacing: 0.3px;
    padding: 12px 16px 6px;
}
.project-list { flex: 1; overflow-y: auto; padding-bottom: 12px; }
.project-item {
    padding: 8px 12px; cursor: pointer;
    border-radius: 6px; margin: 1px 8px;
    transition: background-color 0.1s ease;
}
.project-item:hover { background-color: rgba(0, 0, 0, 0.04); }
.project-item.selected { background-color: #007aff; }
.project-item.selected .project-name { color: #fff; }
.project-item.selected .project-path,
.project-item.selected .project-meta,
.project-item.selected .project-meta span { color: rgba(255,255,255,0.7) !important; }
.project-item.selected .agent-badge { background: rgba(255,255,255,0.22); color: #fff; }

.project-name { font-weight: 500; font-size: 13px; color: #1d1d1f; line-height: 1.3; }
.project-path {
    font-size: 11px; color: #86868b; margin-top: 1px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    max-width: 220px;
    direction: rtl; text-align: left;
}
.project-meta { display: flex; gap: 6px; font-size: 10px; color: #aeaeb2; margin-top: 3px; align-items: center; }
.agent-badge {
    background-color: #34c759; color: #fff;
    padding: 0 6px; border-radius: 8px; font-size: 10px; font-weight: 600;
    line-height: 16px;
}
.meta-sep { color: #d1d1d6; }

/* ── Main panel ── */
.main-panel { flex: 1; overflow-y: auto; padding: 28px 32px; background-color: #fff; }

/* ── Header ── */
.page-header {
    display: flex; justify-content: space-between; align-items: flex-start;
    margin-bottom: 28px; padding-bottom: 20px; border-bottom: 1px solid #f2f2f7;
}
.page-header-info h1 { font-size: 20px; font-weight: 700; color: #1d1d1f; margin-bottom: 4px; }
.page-header-info .path { font-size: 11px; color: #86868b; word-break: break-all; }

/* ── Section ── */
.section { margin-bottom: 28px; }
.section-label {
    font-size: 12px; font-weight: 600; color: #86868b;
    text-transform: uppercase; letter-spacing: 0.3px;
    margin-bottom: 10px;
}

/* ── Cards ── */
.card {
    background: #fff; border: 1px solid #e5e5ea;
    border-radius: 10px; padding: 12px 14px; margin-bottom: 8px;
    box-shadow: 0 0.5px 2px rgba(0,0,0,0.04);
}
.agent-card { display: flex; align-items: center; gap: 10px; }
.agent-status-dot {
    width: 8px; height: 8px; border-radius: 50%; background: #34c759;
    flex-shrink: 0;
}
.agent-card-body { flex: 1; min-width: 0; }
.agent-card-title { font-weight: 600; font-size: 13px; color: #007aff; }
.agent-card-sub {
    font-size: 11px; color: #86868b; margin-top: 1px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}

/* ── Stats ── */
.stats-grid {
    display: grid; grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
    gap: 12px;
}
.stat-card {
    background: #f9f9fb; border: 1px solid #e5e5ea;
    border-radius: 10px; padding: 14px; text-align: center;
}
.stat-value { font-size: 24px; font-weight: 700; line-height: 1.2; }
.stat-value.blue { color: #007aff; }
.stat-value.green { color: #34c759; }
.stat-value.orange { color: #ff9500; }
.stat-label { font-size: 11px; color: #86868b; margin-top: 4px; }

/* ── Session list ── */
.session-row {
    display: flex; align-items: baseline; gap: 8px;
    padding: 9px 0; border-bottom: 1px solid #f2f2f7;
}
.session-row:last-child { border-bottom: none; }
.session-time { font-size: 11px; color: #86868b; white-space: nowrap; flex-shrink: 0; width: 110px; }
.session-branch {
    font-size: 10px; color: #007aff; background: #eef4ff;
    padding: 1px 6px; border-radius: 4px; flex-shrink: 0;
}
.session-msgs { font-size: 10px; color: #aeaeb2; white-space: nowrap; flex-shrink: 0; }
.session-preview-text {
    font-size: 12px; color: #636366; margin-top: 4px; line-height: 1.4;
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;
    overflow: hidden;
}
.session-card { padding: 10px 14px; margin-bottom: 6px; }

/* ── Buttons ── */
.btn {
    padding: 6px 14px; border-radius: 6px; border: none; cursor: pointer;
    font-size: 13px; font-weight: 500; transition: all 0.1s ease;
    display: inline-flex; align-items: center; gap: 4px;
}
.btn-primary { background: #007aff; color: #fff; }
.btn-primary:hover { background: #0066d6; }
.btn-primary:disabled { background: #b0b0b5; cursor: not-allowed; }

/* ── Empty state ── */
.empty-state { text-align: center; color: #86868b; padding: 80px 20px; }
.empty-state h2 { color: #1d1d1f; font-size: 18px; font-weight: 600; margin-bottom: 8px; }
.empty-state p { font-size: 13px; }

/* ── Dialog ── */
.dialog-overlay {
    position: fixed; top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.25); backdrop-filter: blur(4px); -webkit-backdrop-filter: blur(4px);
    display: flex; align-items: center; justify-content: center; z-index: 100;
}
.dialog {
    background: #f6f6f6; border: 1px solid #d1d1d6; border-radius: 12px;
    padding: 22px; width: 380px; max-width: 90vw;
    box-shadow: 0 12px 36px rgba(0,0,0,0.12), 0 0 0 0.5px rgba(0,0,0,0.06);
}
.dialog h2 { margin-bottom: 16px; font-size: 16px; font-weight: 700; color: #1d1d1f; }
.form-group { margin-bottom: 12px; }
.form-group label { display: block; margin-bottom: 4px; font-weight: 500; font-size: 12px; color: #3a3a3c; }
.form-select {
    width: 100%; padding: 6px 10px;
    background: #fff; border: 1px solid #d1d1d6; border-radius: 6px;
    color: #1d1d1f; font-size: 13px; -webkit-appearance: menulist;
}
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
.btn-ghost {
    background: #fff; color: #1d1d1f; border: 1px solid #d1d1d6;
    padding: 6px 14px; border-radius: 6px; cursor: pointer; font-size: 13px; font-weight: 500;
}
.btn-ghost:hover { background: #f2f2f7; }
.warning-box {
    background: #fef7e0; border: 1px solid #f5c518; border-radius: 8px;
    padding: 10px 12px; margin-bottom: 12px; font-size: 12px;
}
.warning-title { color: #8a6d00; font-weight: 600; font-size: 12px; }
.warning-text { color: #6b5300; margin-top: 3px; font-size: 11px; }

/* ── Doc list ── */
.doc-item {
    display: flex; align-items: center; gap: 10px; cursor: pointer;
    transition: background 0.1s ease;
}
.doc-item:hover { background: #f2f2f7; }
.doc-icon { font-size: 18px; flex-shrink: 0; width: 28px; text-align: center; }
.doc-info { flex: 1; min-width: 0; }
.doc-name { font-weight: 500; font-size: 13px; color: #007aff; }
.doc-path {
    font-size: 11px; color: #86868b; margin-top: 1px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
"#;
