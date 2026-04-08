pub const GLOBAL_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Helvetica Neue", sans-serif;
    background-color: #f5f5f7;
    color: #1d1d1f;
    font-size: 13px;
    -webkit-font-smoothing: antialiased;
}
.app-container { display: flex; height: 100vh; width: 100vw; }

/* macOS translucent sidebar */
.sidebar {
    width: 260px; min-width: 260px;
    background-color: rgba(246, 246, 246, 0.92);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border-right: 1px solid #d1d1d6;
    display: flex; flex-direction: column; overflow-y: auto;
}
.sidebar-header {
    padding: 14px 16px;
    font-size: 13px; font-weight: 700; color: #86868b;
    text-transform: uppercase; letter-spacing: 0.5px;
    border-bottom: 1px solid #e5e5ea;
}
.sidebar-title {
    font-size: 20px; font-weight: 700; color: #1d1d1f;
    padding: 16px 16px 8px; letter-spacing: 0; text-transform: none;
}
.main-panel { flex: 1; overflow-y: auto; padding: 24px 28px; background-color: #ffffff; }

/* Project list items */
.project-item {
    padding: 10px 16px; cursor: pointer;
    border-radius: 8px; margin: 2px 8px;
    transition: background-color 0.12s ease;
}
.project-item:hover { background-color: rgba(0, 0, 0, 0.04); }
.project-item.selected {
    background-color: #007aff; color: #ffffff;
}
.project-item.selected .project-path,
.project-item.selected .project-meta,
.project-item.selected .project-meta span { color: rgba(255, 255, 255, 0.75) !important; }
.project-item.selected .project-name { color: #ffffff; }
.project-item.selected .agent-badge { background-color: rgba(255,255,255,0.25); color: #fff; }

.project-name { font-weight: 600; font-size: 13px; margin-bottom: 2px; color: #1d1d1f; }
.project-path { font-size: 11px; color: #86868b; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.project-meta { display: flex; gap: 8px; font-size: 11px; color: #86868b; margin-top: 3px; }

.agent-badge {
    background-color: #34c759; color: #fff;
    padding: 1px 7px; border-radius: 10px; font-size: 10px; font-weight: 600;
}

/* Section titles */
.section-title { font-size: 22px; font-weight: 700; color: #1d1d1f; margin-bottom: 16px; }

/* Cards */
.card {
    background-color: #ffffff;
    border: 1px solid #e5e5ea;
    border-radius: 10px; padding: 14px 16px; margin-bottom: 10px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.04);
}
.agent-card { display: flex; align-items: center; justify-content: space-between; }
.agent-info { display: flex; flex-direction: column; gap: 3px; }
.agent-type { font-weight: 600; font-size: 13px; color: #007aff; }
.agent-pid { font-size: 11px; color: #86868b; }
.agent-cwd { font-size: 11px; color: #86868b; }

.status-dot {
    width: 8px; height: 8px; border-radius: 50%;
    background-color: #34c759; display: inline-block; margin-right: 6px;
}

/* Buttons */
.btn {
    padding: 7px 14px; border-radius: 6px; border: none; cursor: pointer;
    font-size: 13px; font-weight: 500; transition: all 0.12s ease;
}
.btn-primary { background-color: #007aff; color: #ffffff; }
.btn-primary:hover { background-color: #0066d6; }
.btn-primary:disabled { background-color: #b0b0b5; cursor: not-allowed; }

.empty-state { text-align: center; color: #86868b; padding: 60px 20px; }
.empty-state h2 { color: #1d1d1f; font-size: 20px; margin-bottom: 8px; }

/* Sessions */
.session-item { padding: 10px 0; border-bottom: 1px solid #f2f2f7; }
.session-preview { font-size: 12px; color: #636366; margin-top: 4px; line-height: 1.4; }
.session-meta { font-size: 11px; color: #86868b; }

/* Dialog */
.dialog-overlay {
    position: fixed; top: 0; left: 0; right: 0; bottom: 0;
    background-color: rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(4px); -webkit-backdrop-filter: blur(4px);
    display: flex; align-items: center; justify-content: center; z-index: 100;
}
.dialog {
    background-color: #f6f6f6;
    border: 1px solid #d1d1d6;
    border-radius: 12px; padding: 24px;
    width: 400px; max-width: 90vw;
    box-shadow: 0 14px 40px rgba(0,0,0,0.15), 0 0 0 0.5px rgba(0,0,0,0.08);
}
.dialog h2 { margin-bottom: 16px; font-size: 17px; font-weight: 700; color: #1d1d1f; }

.form-group { margin-bottom: 14px; }
.form-group label { display: block; margin-bottom: 5px; font-weight: 600; font-size: 12px; color: #3a3a3c; }
.form-select {
    width: 100%; padding: 7px 10px;
    background-color: #ffffff; border: 1px solid #d1d1d6;
    border-radius: 6px; color: #1d1d1f; font-size: 13px;
    -webkit-appearance: menulist;
}
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
.btn-ghost {
    background: #ffffff; color: #1d1d1f; border: 1px solid #d1d1d6;
    padding: 7px 14px; border-radius: 6px; cursor: pointer; font-size: 13px; font-weight: 500;
}
.btn-ghost:hover { background: #f2f2f7; }

.warning-box {
    background: #fff3cd; border: 1px solid #ffc107; border-radius: 8px;
    padding: 12px; margin-bottom: 12px; font-size: 12px;
}
.warning-title { color: #856404; font-weight: 600; }
.warning-text { color: #664d03; margin-top: 4px; }

/* Stats row */
.stats-row { display: flex; gap: 20px; }
.stat-item { text-align: center; }
.stat-value { font-size: 28px; font-weight: 700; }
.stat-value.blue { color: #007aff; }
.stat-value.green { color: #34c759; }
.stat-value.orange { color: #ff9500; }
.stat-label { font-size: 11px; color: #86868b; margin-top: 2px; }
"#;
