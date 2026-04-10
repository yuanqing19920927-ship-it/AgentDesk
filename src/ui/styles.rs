pub const GLOBAL_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Helvetica Neue", sans-serif;
    background-color: #f5f5f7; color: #1d1d1f; font-size: 13px;
    -webkit-font-smoothing: antialiased; line-height: 1.45;
}
.app-container {
    display: flex; height: 100vh; width: 100vw;
    /* Warm neutral window background. Both the sidebar (frosted) and the
       main panel sit on top of this; there is no hard divider between
       them — each module floats as its own layer. */
    background: #ececef;
}
/* Draggable strip at the very top so users can grab anywhere along the
   titlebar area (traffic lights aside) to move the window. 28px matches
   the macOS titlebar height. We avoid the 72px reserved for the traffic
   lights on the left edge. Dragging is implemented via onmousedown ->
   dioxus::desktop::window().drag() (wry/WKWebView does not honor CSS
   -webkit-app-region). */
.titlebar-drag {
    position: fixed; top: 0; left: 72px; right: 0; height: 28px;
    z-index: 9999;
    background: transparent;
    cursor: default;
}

/* ══════════════════════════════
   Scrollbars — macOS overlay style
   ══════════════════════════════ */
::-webkit-scrollbar { width: 11px; height: 11px; background: transparent; }
::-webkit-scrollbar-track { background: transparent; border: none; }
::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.18);
    border: 3px solid transparent;
    background-clip: content-box;
    border-radius: 10px;
    min-height: 40px;
}
::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.38);
    background-clip: content-box;
}
::-webkit-scrollbar-corner { background: transparent; }

/* ══════════════════════════════
   Icon tiles — macOS Settings style
   ══════════════════════════════ */
.icon-tile {
    display: inline-flex; align-items: center; justify-content: center;
    color: #fff; flex-shrink: 0;
    box-shadow: 0 0.5px 0 rgba(255,255,255,0.35) inset,
                0 0.5px 1.5px rgba(0,0,0,0.1);
}
.icon-tile svg { display: block; }
.icon-tile-xs { width: 18px; height: 18px; border-radius: 4px; }
.icon-tile-sm { width: 22px; height: 22px; border-radius: 5px; }
.icon-tile-md { width: 28px; height: 28px; border-radius: 6px; }
.icon-tile-lg { width: 56px; height: 56px; border-radius: 13px;
    box-shadow: 0 0.5px 0 rgba(255,255,255,0.35) inset,
                0 2px 6px rgba(0,0,0,0.12); }

.tile-gray    { background: linear-gradient(180deg, #a1a1a6, #6e6e73); }
.tile-graphite{ background: linear-gradient(180deg, #8e8e93, #48484a); }
.tile-blue    { background: linear-gradient(180deg, #5ac8fa, #0a84ff); }
.tile-indigo  { background: linear-gradient(180deg, #7c7cf0, #5e5ce6); }
.tile-purple  { background: linear-gradient(180deg, #d67dff, #bf5af2); }
.tile-pink    { background: linear-gradient(180deg, #ff6b8a, #ff2d55); }
.tile-red     { background: linear-gradient(180deg, #ff6b5e, #ff3b30); }
.tile-orange  { background: linear-gradient(180deg, #ffb340, #ff9500); }
.tile-yellow  { background: linear-gradient(180deg, #ffd60a, #ffcc00); color: #6b5300; }
.tile-green   { background: linear-gradient(180deg, #4cd964, #30b653); }
.tile-teal    { background: linear-gradient(180deg, #64d2ff, #30a7c0); }

/* ══════════════════════════════
   Page hero (System Settings-style card)
   ══════════════════════════════ */
.page-hero {
    background: #fff;
    border: 0.5px solid rgba(0, 0, 0, 0.08);
    border-radius: 14px;
    box-shadow: 0 0.5px 2px rgba(0, 0, 0, 0.04);
    padding: 28px 24px 24px; margin-bottom: 20px;
    display: flex; flex-direction: column; align-items: center; text-align: center;
}
.page-hero .hero-title {
    font-size: 22px; font-weight: 700; color: #1d1d1f;
    margin: 14px 0 6px; letter-spacing: -0.2px;
}
.page-hero .hero-desc {
    font-size: 12px; color: #86868b; line-height: 1.55;
    max-width: 560px;
}
.hero-toolbar {
    display: flex; gap: 8px; justify-content: flex-end;
    margin-bottom: 16px;
}

/* Grouped row with leading icon tile: gap between icon and content */
.grouped-row > .icon-tile { margin-right: 12px; }

/* ══════════════════════════════
   SIDEBAR — macOS Settings style
   Frosted vibrancy panel that sits on the window background. No hard
   border on the right edge — the tint difference + backdrop blur does
   all the separation work (like the real System Settings sidebar).
   ══════════════════════════════ */
.sidebar {
    width: 240px; min-width: 240px;
    background: linear-gradient(180deg,
        rgba(244, 244, 248, 0.62) 0%,
        rgba(236, 236, 241, 0.68) 100%);
    backdrop-filter: blur(48px) saturate(180%);
    -webkit-backdrop-filter: blur(48px) saturate(180%);
    display: flex; flex-direction: column;
    /* Reserve space for the traffic lights (top) so content starts below them. */
    padding: 38px 0 0;
    user-select: none;
}
.sidebar-section-label {
    font-size: 13px; font-weight: 700; color: #6e6e73;
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
.home-item.selected .agent-badge { background: rgba(255,255,255,0.25); }
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
    width: 28px; height: 28px; border-radius: 7px;
    background: linear-gradient(180deg, #4cd964, #30b653);
    display: flex; align-items: center; justify-content: center;
    font-size: 13px; flex-shrink: 0; color: #fff; font-weight: 700;
    letter-spacing: -0.3px;
}
.project-icon-box.orange { background: linear-gradient(180deg, #ffb340, #ff8a00); }
.project-icon-box.purple { background: linear-gradient(180deg, #c77dff, #8e55d6); }
.project-icon-box.pink    { background: linear-gradient(180deg, #ff6b8a, #ff2d55); }
.project-icon-box.teal    { background: linear-gradient(180deg, #64d2ff, #30a7c0); }

/* Stereoscopic "glossy tile" treatment applied to sidebar project + home
   icons. Layers: (1) inner top highlight for the glass shine, (2) inner
   bottom shadow for depth, (3) outer hairline ring, (4) outer drop shadow. */
.project-tile-3d {
    box-shadow:
        inset 0 1px 0 rgba(255, 255, 255, 0.55),
        inset 0 -1px 1px rgba(0, 0, 0, 0.18),
        0 0 0 0.5px rgba(0, 0, 0, 0.14),
        0 1px 2px rgba(0, 0, 0, 0.12),
        0 2px 5px -1px rgba(0, 0, 0, 0.10);
    position: relative;
    overflow: hidden;
}
/* Subtle specular highlight across the top edge */
.project-tile-3d::before {
    content: "";
    position: absolute;
    top: 0; left: 0; right: 0; height: 48%;
    background: linear-gradient(180deg,
        rgba(255, 255, 255, 0.28) 0%,
        rgba(255, 255, 255, 0.04) 100%);
    pointer-events: none;
    border-top-left-radius: inherit;
    border-top-right-radius: inherit;
}
.project-tile-3d > * { position: relative; z-index: 1; }
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
.main-panel {
    flex: 1; overflow-y: auto;
    padding: 48px 36px 28px;
    /* Transparent: inherits the shared window background from .app-container
       so the sidebar and content area appear to float on the same surface
       without a hard divider between them. */
    background: transparent;
}
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

/* Grouped card — rounded container for rows. Floats above the window
   background with a soft elevation so the tinted sidebar and the card
   both read as separate layers. */
.grouped-card {
    background: #fff;
    border: 0.5px solid rgba(0, 0, 0, 0.06);
    border-radius: 12px; overflow: hidden;
    box-shadow:
        0 0 0 0.5px rgba(0, 0, 0, 0.04),
        0 1px 3px rgba(0, 0, 0, 0.05),
        0 6px 16px -6px rgba(0, 0, 0, 0.06);
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
.agent-actions { display: flex; gap: 8px; align-items: center; flex-shrink: 0; }
.btn-kill {
    background: transparent; border: none; cursor: pointer;
    font-size: 11px; font-weight: 500; color: #ff3b30; padding: 2px 0;
}
.btn-kill:hover { text-decoration: underline; }
.btn-kill.confirm {
    background: #ff3b30; color: #fff; border-radius: 4px;
    padding: 2px 8px; font-size: 11px;
}
.btn-kill.confirm:hover { background: #d63030; }
.btn-kill-cancel {
    background: transparent; border: none; cursor: pointer;
    font-size: 11px; color: #86868b; padding: 2px 0;
}
.btn-kill-cancel:hover { text-decoration: underline; }

/* ── Context menu extras ── */
.ctx-menu-separator { height: 1px; background: #e5e5ea; margin: 4px 8px; }
.ctx-menu-header { padding: 4px 10px; font-size: 10px; color: #86868b; font-weight: 600; }
.ctx-menu-active { color: #007aff; }
.ctx-menu-active:hover { background: #007aff; color: #fff; }

/* ── Settings group input ── */
.group-input {
    flex: 1; padding: 5px 8px; border: 1px solid #d1d1d6; border-radius: 5px;
    font-size: 13px; background: #fff; color: #1d1d1f; outline: none;
}
.group-input:focus { border-color: #007aff; }
.group-actions { display: flex; gap: 4px; align-items: center; }
.btn-reorder {
    background: #f2f2f7; border: 0.5px solid #d1d1d6; border-radius: 4px;
    width: 24px; height: 24px; cursor: pointer; font-size: 12px;
    color: #007aff; display: flex; align-items: center; justify-content: center;
}
.btn-reorder:hover { background: #e5e5ea; }

/* ══════════════════════════════
   DYNAMIC ISLAND (titlebar embedded)
   ══════════════════════════════ */
.island {
    background: #1d1d1f;
    border-radius: 14px;
    padding: 0 12px;
    height: 28px;
    display: inline-flex; align-items: center; justify-content: center;
    box-shadow: 0 1px 4px rgba(0,0,0,0.12);
    -webkit-app-region: no-drag; /* island itself is clickable, not draggable */
}
.island-empty { background: rgba(0,0,0,0.06); box-shadow: none; }
.island-empty .island-text { color: #aeaeb2; }
.island-empty {
    padding: 0 10px;
}
.island-content {
    display: flex; align-items: center; gap: 8px;
}
.island-icon { font-size: 12px; }
.island-text { font-size: 11px; color: #86868b; }

.island-group {
    display: flex; align-items: center; gap: 3px;
}
.island-dot {
    width: 6px; height: 6px; border-radius: 50%;
}
.island-dot.busy { background: #ff9500; animation: pulse 1.5s ease-in-out infinite; }
.island-dot.idle { background: #34c759; }
.island-count { font-size: 12px; font-weight: 700; color: #fff; }
.island-label { font-size: 9px; color: #86868b; }

.island-sep {
    width: 1px; height: 12px; background: #3a3a3c; flex-shrink: 0;
}

.island-agent {
    display: flex; align-items: center; gap: 3px;
    padding: 1px 7px; border-radius: 10px;
    font-size: 10px;
    -webkit-app-region: no-drag;
}
.island-agent.busy {
    background: rgba(255, 149, 0, 0.2);
    color: #ff9500;
}
.island-agent.idle {
    background: rgba(52, 199, 89, 0.15);
    color: #34c759;
}
.island-agent-name { font-weight: 600; max-width: 80px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.island-agent-cpu { font-size: 9px; opacity: 0.7; }
.island-jump {
    background: none; border: none; cursor: pointer;
    font-size: 9px; color: inherit; opacity: 0.6;
    padding: 0 1px; -webkit-app-region: no-drag;
}
.island-jump:hover { opacity: 1; }
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
/* Kind-specific accents. More muted than role colors so that tool
   activity doesn't dominate the view. */
.msg-kind-thinking { background: #fff7e6; border-left: 3px solid #ffb340; }
.msg-kind-tool-use { background: #f2edff; border-left: 3px solid #af52de; }
.msg-kind-tool-result { background: #ecfdf5; border-left: 3px solid #30b653; }
.msg-header { display: flex; align-items: center; gap: 6px; margin-bottom: 3px; }
.msg-role { font-weight: 600; font-size: 11px; color: #3a3a3c; }
.msg-kind-badge {
    font-size: 9px; font-weight: 600; color: #6e6e73;
    background: rgba(0, 0, 0, 0.06); padding: 1px 6px; border-radius: 4px;
}
.msg-tool-name {
    font-size: 10px; font-weight: 600; color: #5856d6;
    background: #eeedff; padding: 1px 6px; border-radius: 4px;
}
.msg-time { font-size: 10px; color: #aeaeb2; margin-left: auto; }
.msg-content { font-size: 12px; color: #1d1d1f; white-space: pre-wrap; word-break: break-word; }
.msg-code {
    font-family: "SF Mono", Menlo, Consolas, monospace;
    background: rgba(0, 0, 0, 0.04);
    padding: 6px 8px; border-radius: 5px;
    max-height: 240px; overflow: auto;
    margin: 2px 0 0;
}

/* Log viewer filter toolbar */
.log-filter-bar {
    display: flex; gap: 6px; align-items: center;
    padding: 8px 0 10px; flex-wrap: wrap;
}
.log-filter-chip {
    display: inline-flex; align-items: center; gap: 4px;
    font-size: 11px; color: #3a3a3c;
    background: #fff; border: 0.5px solid rgba(0, 0, 0, 0.08);
    padding: 3px 8px; border-radius: 10px;
    cursor: pointer; user-select: none;
}
.log-filter-chip input[type="checkbox"] {
    margin: 0; width: 11px; height: 11px;
}
/* Module 10 补完 — search input + live-tail toggle + per-msg copy */
.log-search-input {
    flex: 1; min-width: 120px;
    font-size: 11px; color: #1d1d1f;
    background: #fff; border: 0.5px solid rgba(0, 0, 0, 0.12);
    padding: 4px 10px; border-radius: 10px;
    outline: none;
}
.log-search-input:focus {
    border-color: #0071e3;
    box-shadow: 0 0 0 2px rgba(0, 113, 227, 0.18);
}
.log-live-chip { background: #fff5ee; border-color: #ffcca3; color: #8a4a00; }
.log-live-chip:hover { background: #ffe9d4; }
.log-search-summary {
    font-size: 10px; color: #6e6e73;
    padding: 4px 0 6px 2px;
}
.msg-copy-btn {
    margin-left: 6px;
    background: transparent;
    border: 0.5px solid rgba(0, 0, 0, 0.12);
    border-radius: 4px;
    padding: 0 6px;
    font-size: 10px;
    color: #6e6e73;
    cursor: pointer;
    line-height: 16px;
}
.msg-copy-btn:hover { background: rgba(0, 0, 0, 0.05); color: #1d1d1f; }

/* ── Health dashboard (Module 11) ── */
.health-dot {
    width: 12px; height: 12px; border-radius: 50%;
    flex-shrink: 0;
    box-shadow:
        inset 0 1px 0 rgba(255, 255, 255, 0.55),
        inset 0 -1px 1px rgba(0, 0, 0, 0.20),
        0 0 0 0.5px rgba(0, 0, 0, 0.12),
        0 1px 2px rgba(0, 0, 0, 0.14);
}
.health-dot.health-green  { background: radial-gradient(circle at 30% 30%, #5edc7f, #30b653); }
.health-dot.health-yellow { background: radial-gradient(circle at 30% 30%, #ffd95a, #f0a020); }
.health-dot.health-red    { background: radial-gradient(circle at 30% 30%, #ff7b7b, #d93025); }
.health-chip {
    display: inline-flex; align-items: center;
    font-size: 10px; font-weight: 600;
    padding: 2px 8px; border-radius: 9px;
    margin-left: auto; flex-shrink: 0;
    letter-spacing: 0.02em;
}
.health-chip.health-green  { background: #e6f8ec; color: #1f7a3a; border: 0.5px solid #b7e6c6; }
.health-chip.health-yellow { background: #fff6e0; color: #8a5a00; border: 0.5px solid #f0dba0; }
.health-chip.health-red    { background: #fde8e8; color: #9a1d1d; border: 0.5px solid #f0b8b8; }
.health-hint { color: #8a8a8e; font-style: normal; }

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
/* Inside a grouped-row the select is a trailing control, not a full-width form field.
   Give it a fixed width so the left-hand row-content keeps its space. */
.grouped-row .form-select {
    width: auto; min-width: 140px; max-width: 220px; flex-shrink: 0;
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
    border-top: 0.5px solid #d1d1d6; padding: 8px 10px 10px;
    flex-shrink: 0;
}
.sidebar-footer-btn {
    display: flex; align-items: center; gap: 9px;
    padding: 6px 10px; border-radius: 8px; cursor: pointer;
    font-size: 13px; font-weight: 500; color: #1d1d1f;
    transition: background 0.1s;
    margin-bottom: 1px;
}
.sidebar-footer-btn:hover { background: rgba(0,0,0,0.05); }

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

/* ── Command palette (Cmd+K) ── */
.palette-backdrop {
    position: fixed; top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.28); backdrop-filter: blur(6px); -webkit-backdrop-filter: blur(6px);
    display: flex; align-items: flex-start; justify-content: center;
    padding-top: 12vh; z-index: 300;
}
.palette-modal {
    background: rgba(252,252,252,0.98);
    border: 0.5px solid #c7c7cc; border-radius: 12px;
    width: 560px; max-width: 92vw; max-height: 70vh;
    display: flex; flex-direction: column;
    box-shadow: 0 24px 60px rgba(0,0,0,0.22), 0 0 0 0.5px rgba(0,0,0,0.05);
    overflow: hidden;
}
.palette-input {
    border: none; outline: none; background: transparent;
    padding: 14px 18px; font-size: 15px; color: #1d1d1f;
    border-bottom: 0.5px solid #e5e5ea;
}
.palette-input::placeholder { color: #86868b; }
.palette-list {
    flex: 1 1 auto; overflow-y: auto; padding: 6px 0;
    max-height: calc(70vh - 96px);
}
.palette-empty {
    padding: 20px; text-align: center; color: #86868b; font-size: 13px;
}
.palette-row {
    display: flex; align-items: center; gap: 12px;
    padding: 8px 16px; cursor: pointer;
    border-left: 2px solid transparent;
}
.palette-row-active {
    background: rgba(0,122,255,0.10);
    border-left-color: #007aff;
}
.palette-glyph {
    flex: 0 0 auto; width: 20px; text-align: center;
    font-size: 13px; color: #86868b;
}
.palette-row-active .palette-glyph { color: #007aff; }
.palette-text { flex: 1 1 auto; min-width: 0; }
.palette-label {
    font-size: 13px; font-weight: 500; color: #1d1d1f;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.palette-detail {
    font-size: 11px; color: #86868b; margin-top: 1px;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.palette-footer {
    display: flex; gap: 14px; padding: 8px 16px;
    border-top: 0.5px solid #e5e5ea; background: #f6f6f6;
    font-size: 11px; color: #86868b;
}
.palette-hint { display: inline-flex; align-items: center; gap: 4px; }

/* ── Instruction dialog (Cmd+Enter) ── */
.instruction-dialog { width: 480px; }
.instr-meta { font-size: 11px; color: #86868b; margin-bottom: 12px; }
.instr-section-label {
    font-size: 11px; font-weight: 600; color: #86868b;
    text-transform: uppercase; letter-spacing: 0.5px;
    margin: 12px 0 6px;
}
.instr-chips {
    display: flex; flex-wrap: wrap; gap: 6px;
    margin-bottom: 4px;
}
.instr-chip {
    background: #fff; color: #007aff;
    border: 0.5px solid #c7c7cc; border-radius: 14px;
    padding: 4px 10px; font-size: 11px; font-weight: 500;
    cursor: pointer; font-family: ui-monospace, SFMono-Regular, monospace;
}
.instr-chip:hover { background: #007aff; color: #fff; border-color: #007aff; }
.instr-textarea {
    width: 100%; min-height: 72px;
    border: 0.5px solid #d1d1d6; border-radius: 6px;
    background: #fff; color: #1d1d1f;
    padding: 8px 10px; font-size: 13px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    resize: vertical; outline: none;
    box-sizing: border-box;
}
.instr-textarea:focus { border-color: #007aff; }
.instr-confirm {
    margin-top: 10px; padding: 8px 10px;
    background: #fef7e0; border: 0.5px solid #f5c518; border-radius: 6px;
    font-size: 11px;
}
.instr-confirm-title { color: #8a6d00; font-weight: 600; margin-bottom: 2px; }
.instr-confirm-body {
    color: #1d1d1f; font-family: ui-monospace, SFMono-Regular, monospace;
    word-break: break-all;
}
.instr-error {
    margin-top: 10px; padding: 6px 10px;
    background: #ffe5e2; border: 0.5px solid #ff3b30; border-radius: 6px;
    color: #8a0f05; font-size: 11px;
}
.instr-success {
    margin-top: 10px; padding: 6px 10px;
    background: #e4f7df; border: 0.5px solid #34c759; border-radius: 6px;
    color: #1c5b17; font-size: 11px; font-weight: 500;
}

/* ───────────── Workflows (module 5.3) ───────────── */
.wf-row-active { background: #eef3ff; }
.wf-node-badges {
    display: flex; flex-wrap: wrap; gap: 4px; margin-top: 4px;
}
.wf-badge {
    display: inline-block; padding: 2px 8px;
    border-radius: 10px; font-size: 10px; font-weight: 500;
    border: 0.5px solid transparent;
}
.wf-badge-pending {
    background: #f2f2f7; color: #6e6e73; border-color: #d1d1d6;
}
.wf-badge-launching {
    background: #fff4e5; color: #a25c00; border-color: #ffcc80;
}
.wf-badge-running {
    background: #e4f0ff; color: #0a4ea8; border-color: #a8c8ff;
}
.wf-badge-completed {
    background: #e4f7df; color: #1c5b17; border-color: #9dd89a;
}
.wf-badge-failed {
    background: #ffe5e2; color: #8a0f05; border-color: #f5a098;
}
.wf-editor {
    width: 560px; max-width: 90vw; max-height: 85vh;
    overflow-y: auto;
}
.wf-editor-section-header {
    display: flex; align-items: center; justify-content: space-between;
    margin: 14px 0 6px 0; font-size: 12px; font-weight: 600;
    color: #1d1d1f;
}
.wf-node-card {
    border: 0.5px solid #d1d1d6; border-radius: 6px;
    padding: 8px; margin-bottom: 6px; background: #fafafa;
}
.wf-edge-row {
    display: flex; align-items: center; gap: 6px;
    margin-bottom: 4px;
}

/* ══════════════════════════════
   Combo preset editor — module 7
   Two-column split: left = 可选模板, right = 已选列表
   ══════════════════════════════ */
.combo-editor {
    width: 720px; max-width: 92vw; max-height: 85vh;
    overflow-y: auto;
}
.combo-editor-split {
    display: flex; gap: 12px; margin-top: 8px;
}
.combo-editor-col {
    flex: 1;
    border: 0.5px solid #d1d1d6;
    border-radius: 6px;
    background: #fafafa;
    display: flex; flex-direction: column;
    min-width: 0;
}
.combo-editor-col-title {
    padding: 6px 10px;
    border-bottom: 0.5px solid #d1d1d6;
    font-size: 11px; font-weight: 600;
    color: #1d1d1f; background: #f2f2f7;
    border-top-left-radius: 6px; border-top-right-radius: 6px;
}
.combo-editor-col-body {
    padding: 4px; max-height: 340px; overflow-y: auto;
}
.combo-editor-tpl-row {
    display: flex; align-items: center; gap: 6px;
    padding: 6px 8px;
    border-bottom: 0.5px solid #f2f2f7;
    font-size: 12px;
}
.combo-editor-tpl-row:last-child { border-bottom: none; }
.combo-editor-tpl-row .tpl-name {
    flex: 1; min-width: 0;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.combo-editor-tpl-row .tpl-badge {
    font-size: 10px; color: #6e6e73;
    background: #e5e5ea; padding: 1px 6px;
    border-radius: 8px;
}
.combo-editor-item-row {
    display: flex; align-items: center; gap: 4px;
    padding: 6px 8px;
    border-bottom: 0.5px solid #f2f2f7;
    font-size: 12px;
}
.combo-editor-item-row:last-child { border-bottom: none; }
.combo-editor-item-row .item-idx {
    font-size: 10px; color: #6e6e73;
    min-width: 18px;
}
.combo-editor-item-row .item-body {
    flex: 1; min-width: 0;
    display: flex; flex-direction: column; gap: 2px;
}
.combo-editor-item-row .item-body .tpl-name {
    font-weight: 500;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.combo-editor-item-row .item-body input {
    font-size: 11px;
    padding: 2px 4px;
    border: 0.5px solid #d1d1d6; border-radius: 4px;
    background: #fff;
}
.combo-editor-empty {
    padding: 20px 8px; text-align: center;
    color: #8e8e93; font-size: 11px;
}

/* ══════════════════════════════
   Budget alerts — module 6
   Progress bar with green / yellow / red tiers based on
   `BudgetStatus::level` css_class().
   ══════════════════════════════ */
.budget-row {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 12px;
}
.budget-row-header {
    display: flex; align-items: center; justify-content: space-between;
    gap: 8px; margin-bottom: 4px;
}
.budget-bar-wrap {
    position: relative;
    flex: 1; height: 8px;
    background: #e5e5ea; border-radius: 4px;
    overflow: hidden;
}
.budget-bar-fill {
    position: absolute; inset: 0 auto 0 0;
    border-radius: 4px;
    transition: width 200ms ease-out;
}
.budget-bar-fill.budget-ok       { background: #34c759; }
.budget-bar-fill.budget-warn     { background: #ffcc00; }
.budget-bar-fill.budget-exceeded { background: #ff3b30; }
.budget-bar-fill.budget-none     { background: #c7c7cc; }
.budget-chip {
    font-size: 10px; font-weight: 600;
    padding: 2px 8px; border-radius: 10px;
    border: 0.5px solid transparent;
}
.budget-chip.budget-ok       { background: #e4f7df; color: #1c5b17; border-color: #9dd89a; }
.budget-chip.budget-warn     { background: #fff6d6; color: #8a5a00; border-color: #f0d366; }
.budget-chip.budget-exceeded { background: #ffe5e2; color: #8a0f05; border-color: #f5a098; }
.budget-chip.budget-none     { background: #f2f2f7; color: #6e6e73; border-color: #d1d1d6; }
.budget-banner {
    position: sticky; top: 0; z-index: 10;
    padding: 8px 14px;
    background: #ffe5e2; color: #8a0f05;
    border: 0.5px solid #f5a098; border-radius: 8px;
    font-size: 12px; font-weight: 600;
    margin-bottom: 10px;
    display: flex; align-items: center; justify-content: space-between;
    gap: 8px;
}
.budget-banner.budget-warn {
    background: #fff6d6; color: #8a5a00; border-color: #f0d366;
}

/* ══════════════════════════════
   Module 8 — Notification Center
   ══════════════════════════════ */
.sidebar-bell-wrap { position: relative; display: inline-flex; }
.sidebar-bell-badge {
    position: absolute;
    top: -4px; right: -6px;
    min-width: 16px; height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: #ff3b30;
    color: #fff;
    font-size: 10px; font-weight: 700;
    line-height: 16px; text-align: center;
    border: 1.5px solid #fff;
    box-sizing: content-box;
}
.btn-xs {
    padding: 3px 9px !important;
    font-size: 11px !important;
}
.btn-link {
    background: transparent; border: none;
    color: #0071e3; cursor: pointer;
    font-size: 11px; padding: 0;
}
.btn-link:hover { text-decoration: underline; }

.notif-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.12);
    z-index: 2000;
}
.notif-panel {
    position: fixed;
    left: 244px; bottom: 52px;
    width: 460px; max-height: 560px;
    background: #fff;
    border: 0.5px solid #d1d1d6;
    border-radius: 12px;
    box-shadow: 0 12px 40px rgba(0,0,0,0.18);
    z-index: 2001;
    display: flex; flex-direction: column;
    overflow: hidden;
}
.notif-header {
    padding: 12px 14px;
    border-bottom: 0.5px solid #e5e5ea;
    display: flex; align-items: center; justify-content: space-between;
    gap: 10px;
}
.notif-title { font-size: 14px; font-weight: 700; color: #1d1d1f; }
.notif-header-actions { display: flex; gap: 6px; }

.notif-tabs {
    display: flex; gap: 4px;
    padding: 8px 14px;
    border-bottom: 0.5px solid #e5e5ea;
    background: #f9f9fb;
}
.notif-tab {
    padding: 4px 12px;
    border-radius: 14px;
    font-size: 12px; font-weight: 500;
    color: #6e6e73;
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
}
.notif-tab:hover { background: #eeeef1; }
.notif-tab-active {
    background: #0071e3;
    color: #fff;
}
.notif-tab-active:hover { background: #0077ed; }

.notif-body {
    flex: 1;
    overflow-y: auto;
    padding: 6px 0;
}
.notif-empty {
    padding: 40px 14px;
    text-align: center;
    color: #8e8e93;
    font-size: 12px;
}
.notif-row {
    display: flex; gap: 10px;
    padding: 10px 14px;
    border-bottom: 0.5px solid #f2f2f7;
    background: #fff;
}
.notif-row:last-child { border-bottom: none; }
.notif-row-read { background: #fafafc; }
.notif-row-error { border-left: 3px solid #ff3b30; padding-left: 11px; }
.notif-row-main { flex: 1; min-width: 0; }
.notif-row-head {
    display: flex; align-items: center; gap: 6px;
    margin-bottom: 3px;
}
.notif-dot {
    width: 7px; height: 7px;
    border-radius: 50%;
    background: #0071e3;
    flex-shrink: 0;
}
.notif-row-title {
    font-size: 12px; font-weight: 600;
    color: #1d1d1f;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.notif-row-read .notif-row-title { color: #6e6e73; font-weight: 500; }
.notif-badge {
    padding: 1px 6px;
    font-size: 10px; font-weight: 600;
    border-radius: 4px;
    background: #e5e5ea; color: #3a3a3c;
    white-space: nowrap;
}
.notif-badge-error {
    background: #ffe5e2; color: #c10b00;
}
.notif-row-time {
    margin-left: auto;
    font-size: 10px;
    color: #8e8e93;
    white-space: nowrap;
}
.notif-row-msg {
    font-size: 11px;
    color: #48484a;
    line-height: 1.45;
    word-break: break-word;
}
.notif-row-read .notif-row-msg { color: #8e8e93; }
.notif-row-foot {
    display: flex; align-items: center; gap: 8px;
    margin-top: 4px;
}
.notif-row-path {
    font-size: 10px;
    color: #8e8e93;
    font-family: "SF Mono", monospace;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    flex: 1;
}
.notif-row-actions {
    display: flex; flex-direction: column; gap: 4px;
    flex-shrink: 0;
}
"#;
