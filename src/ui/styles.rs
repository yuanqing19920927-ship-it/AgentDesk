pub const GLOBAL_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
    background-color: #1e1e2e; color: #cdd6f4; font-size: 14px;
}
.app-container { display: flex; height: 100vh; width: 100vw; }
.sidebar {
    width: 280px; min-width: 280px; background-color: #181825;
    border-right: 1px solid #313244; display: flex; flex-direction: column; overflow-y: auto;
}
.sidebar-header { padding: 16px; border-bottom: 1px solid #313244; font-size: 18px; font-weight: 700; color: #cba6f7; }
.main-panel { flex: 1; overflow-y: auto; padding: 24px; }
.project-item { padding: 12px 16px; cursor: pointer; border-bottom: 1px solid #313244; transition: background-color 0.15s; }
.project-item:hover { background-color: #313244; }
.project-item.selected { background-color: #45475a; border-left: 3px solid #cba6f7; }
.project-name { font-weight: 600; margin-bottom: 4px; }
.project-path { font-size: 12px; color: #6c7086; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.project-meta { display: flex; gap: 12px; font-size: 12px; color: #a6adc8; margin-top: 4px; }
.agent-badge { background-color: #a6e3a1; color: #1e1e2e; padding: 1px 8px; border-radius: 10px; font-size: 11px; font-weight: 600; }
.section-title { font-size: 20px; font-weight: 700; margin-bottom: 16px; }
.card { background-color: #313244; border-radius: 8px; padding: 16px; margin-bottom: 12px; }
.agent-card { display: flex; align-items: center; justify-content: space-between; }
.agent-info { display: flex; flex-direction: column; gap: 4px; }
.agent-type { font-weight: 600; color: #89b4fa; }
.agent-pid { font-size: 12px; color: #6c7086; }
.agent-cwd { font-size: 12px; color: #a6adc8; }
.status-dot { width: 8px; height: 8px; border-radius: 50%; background-color: #a6e3a1; display: inline-block; margin-right: 6px; }
.btn { padding: 8px 16px; border-radius: 6px; border: none; cursor: pointer; font-size: 14px; font-weight: 600; transition: background-color 0.15s; }
.btn-primary { background-color: #cba6f7; color: #1e1e2e; }
.btn-primary:hover { background-color: #b4befe; }
.empty-state { text-align: center; color: #6c7086; padding: 40px 20px; }
.session-item { padding: 10px 0; border-bottom: 1px solid #45475a; }
.session-preview { font-size: 13px; color: #a6adc8; margin-top: 4px; }
.session-meta { font-size: 12px; color: #6c7086; }
.dialog-overlay { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background-color: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
.dialog { background-color: #1e1e2e; border: 1px solid #313244; border-radius: 12px; padding: 24px; width: 420px; max-width: 90vw; }
.dialog h2 { margin-bottom: 16px; color: #cba6f7; }
.form-group { margin-bottom: 16px; }
.form-group label { display: block; margin-bottom: 6px; font-weight: 600; font-size: 13px; }
.form-select { width: 100%; padding: 8px 12px; background-color: #313244; border: 1px solid #45475a; border-radius: 6px; color: #cdd6f4; font-size: 14px; }
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }
.btn-ghost { background: transparent; color: #a6adc8; border: 1px solid #45475a; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-size: 14px; font-weight: 600; }
.warning-box { background: #45475a; border-left: 3px solid #f38ba8; padding: 10px; margin-bottom: 12px; font-size: 13px; }
.warning-title { color: #f38ba8; font-weight: 600; }
.warning-text { color: #a6adc8; margin-top: 4px; }
"#;
