import AppKit
import WebKit

// === Dynamic Island Overlay for AgentDesk ===
// A floating borderless panel positioned at the macOS notch area
// Reads agent status from ~/.agentdesk/island_state.json

class IslandPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    var panel: IslandPanel!
    var webView: WKWebView!
    var timer: Timer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Get screen with notch (main screen)
        guard let screen = NSScreen.main else { return }
        let screenFrame = screen.frame

        // Island dimensions
        let islandWidth: CGFloat = 360
        let islandHeight: CGFloat = 38

        // Position: top center, just below the menu bar
        let menuBarHeight: CGFloat = NSStatusBar.system.thickness
        let x = screenFrame.midX - islandWidth / 2
        let y = screenFrame.maxY - menuBarHeight - islandHeight - 4

        let frame = NSRect(x: x, y: y, width: islandWidth, height: islandHeight)

        panel = IslandPanel(
            contentRect: frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.level = .statusBar
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.isMovableByWindowBackground = false
        panel.ignoresMouseEvents = false

        // WebView for rendering
        let config = WKWebViewConfiguration()
        config.preferences.setValue(true, forKey: "developerExtrasEnabled")
        webView = WKWebView(frame: NSRect(x: 0, y: 0, width: islandWidth, height: islandHeight), configuration: config)
        webView.setValue(false, forKey: "drawsBackground")

        panel.contentView = webView
        panel.orderFront(nil)

        // Initial render
        updateIsland()

        // Poll every 2 seconds
        timer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            self?.updateIsland()
        }
    }

    func updateIsland() {
        let stateFile = NSHomeDirectory() + "/.agentdesk/island_state.json"

        var agents: [[String: Any]] = []
        if let data = try? Data(contentsOf: URL(fileURLWithPath: stateFile)),
           let json = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] {
            agents = json
        }

        let busyAgents = agents.filter { ($0["status"] as? String) == "busy" }
        let idleAgents = agents.filter { ($0["status"] as? String) == "idle" }
        let busyCount = busyAgents.count
        let idleCount = idleAgents.count
        let total = busyCount + idleCount

        // Build agent pills HTML
        var pillsHtml = ""
        for a in busyAgents {
            let name = a["project"] as? String ?? ""
            let cpu = a["cpu"] as? Double ?? 0
            pillsHtml += """
            <span class="pill busy">\(esc(name)) <span class="cpu">\(String(format: "%.0f", cpu))%</span></span>
            """
        }
        for a in idleAgents {
            let name = a["project"] as? String ?? ""
            pillsHtml += """
            <span class="pill idle">\(esc(name))</span>
            """
        }

        let html: String
        if total == 0 {
            html = buildHtml(body: """
            <span class="empty">💤 暂无活跃 Agent</span>
            """)
        } else {
            var summary = ""
            if busyCount > 0 {
                summary += """
                <span class="dot busy-dot"></span><b>\(busyCount)</b><span class="label">工作中</span>
                """
            }
            if idleCount > 0 {
                summary += """
                <span class="dot idle-dot"></span><b>\(idleCount)</b><span class="label">空闲</span>
                """
            }
            html = buildHtml(body: """
            \(summary)<span class="sep"></span>\(pillsHtml)
            """)
        }

        webView.loadHTMLString(html, baseURL: nil)

        // Resize island based on content
        let width: CGFloat = total == 0 ? 200 : min(CGFloat(180 + total * 90), 600)
        if let screen = NSScreen.main {
            let menuBarHeight = NSStatusBar.system.thickness
            let x = screen.frame.midX - width / 2
            let y = screen.frame.maxY - menuBarHeight - 38 - 4
            panel.setFrame(NSRect(x: x, y: y, width: width, height: 38), display: true, animate: true)
        }
    }

    func esc(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;")
         .replacingOccurrences(of: "<", with: "&lt;")
         .replacingOccurrences(of: ">", with: "&gt;")
    }

    func buildHtml(body: String) -> String {
        """
        <!DOCTYPE html>
        <html><head><style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        html, body { background: transparent; overflow: hidden; height: 100%; }
        body {
            display: flex; align-items: center; justify-content: center;
            font-family: -apple-system, BlinkMacSystemFont, sans-serif;
            -webkit-font-smoothing: antialiased;
        }
        .island {
            background: rgba(20, 20, 22, 0.92);
            -webkit-backdrop-filter: blur(20px);
            border-radius: 19px;
            padding: 0 14px;
            height: 34px;
            display: inline-flex; align-items: center; gap: 8px;
            color: #fff; font-size: 11px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.3);
        }
        .dot { width: 6px; height: 6px; border-radius: 50%; display: inline-block; margin-right: 2px; }
        .busy-dot { background: #ff9500; animation: pulse 1.5s ease-in-out infinite; }
        .idle-dot { background: #34c759; }
        @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }
        b { font-size: 13px; font-weight: 700; }
        .label { color: #86868b; font-size: 10px; margin-right: 4px; }
        .sep { width: 1px; height: 14px; background: #3a3a3c; }
        .pill {
            display: inline-flex; align-items: center; gap: 3px;
            padding: 2px 8px; border-radius: 10px; font-size: 10px; font-weight: 600;
        }
        .pill.busy { background: rgba(255,149,0,0.2); color: #ff9500; }
        .pill.idle { background: rgba(52,199,89,0.15); color: #34c759; }
        .cpu { font-size: 9px; opacity: 0.8; }
        .empty { color: #86868b; font-size: 11px; }
        </style></head>
        <body><div class="island">\(body)</div></body></html>
        """
    }
}

// === Main ===
let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory) // No dock icon
app.run()
