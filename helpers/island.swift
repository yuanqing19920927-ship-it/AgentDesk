import AppKit
import WebKit

// === Dynamic Island Overlay for AgentDesk ===
// Positioned right below the macOS notch, centered

class IslandPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    var panel: IslandPanel!
    var webView: WKWebView!
    var timer: Timer?
    var parentPid: pid_t = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Track parent process to auto-quit when main app exits
        parentPid = getppid()

        guard let screen = NSScreen.main else { return }
        let sf = screen.frame
        let vf = screen.visibleFrame  // excludes menu bar and dock

        // The notch area is between screen top and visible frame top
        let menuBarBottom = vf.maxY  // bottom of menu bar = top of visible area
        let notchCenterX = sf.midX

        // Island: positioned just below menu bar, centered on screen
        let islandWidth: CGFloat = 300
        let islandHeight: CGFloat = 36
        let x = notchCenterX - islandWidth / 2
        let y = menuBarBottom - islandHeight - 2  // just below menu bar

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
        panel.hasShadow = false
        panel.isMovableByWindowBackground = false
        panel.ignoresMouseEvents = false

        let config = WKWebViewConfiguration()
        webView = WKWebView(frame: NSRect(x: 0, y: 0, width: islandWidth, height: islandHeight), configuration: config)
        webView.setValue(false, forKey: "drawsBackground")
        panel.contentView = webView
        panel.orderFront(nil)

        updateIsland()

        // Poll every 2s + check if parent still alive
        timer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            // Auto-quit if parent process died
            if kill(self.parentPid, 0) != 0 {
                self.cleanup()
                NSApp.terminate(nil)
                return
            }
            self.updateIsland()
        }

        // Also handle SIGTERM gracefully
        signal(SIGTERM) { _ in
            NSApp.terminate(nil)
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        cleanup()
    }

    func cleanup() {
        timer?.invalidate()
        let path = NSHomeDirectory() + "/.agentdesk/island_state.json"
        try? FileManager.default.removeItem(atPath: path)
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

        var pillsHtml = ""
        for a in busyAgents {
            let name = a["project"] as? String ?? ""
            let cpu = a["cpu"] as? Double ?? 0
            pillsHtml += "<span class='pill busy'>\(esc(name)) <span class='cpu'>\(String(format: "%.0f", cpu))%</span></span>"
        }
        for a in idleAgents {
            let name = a["project"] as? String ?? ""
            pillsHtml += "<span class='pill idle'>\(esc(name))</span>"
        }

        let html: String
        if total == 0 {
            html = buildHtml(body: "<span class='empty'>💤 暂无活跃 Agent</span>")
        } else {
            var summary = ""
            if busyCount > 0 { summary += "<span class='dot busy-dot'></span><b>\(busyCount)</b><span class='lbl'>工作中</span>" }
            if idleCount > 0 { summary += "<span class='dot idle-dot'></span><b>\(idleCount)</b><span class='lbl'>空闲</span>" }
            html = buildHtml(body: "\(summary)<span class='sep'></span>\(pillsHtml)")
        }

        webView.loadHTMLString(html, baseURL: nil)

        // Resize and reposition
        let width: CGFloat = total == 0 ? 190 : min(CGFloat(160 + total * 85), 550)
        if let screen = NSScreen.main {
            let vf = screen.visibleFrame
            let x = screen.frame.midX - width / 2
            let y = vf.maxY - 36 - 2
            panel.setFrame(NSRect(x: x, y: y, width: width, height: 36), display: true, animate: true)
        }
    }

    func esc(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;")
         .replacingOccurrences(of: "<", with: "&lt;")
         .replacingOccurrences(of: ">", with: "&gt;")
    }

    func buildHtml(body: String) -> String {
        """
        <!DOCTYPE html><html><head><style>
        *{margin:0;padding:0;box-sizing:border-box}
        html,body{background:transparent;overflow:hidden;height:100%}
        body{display:flex;align-items:center;justify-content:center;
             font-family:-apple-system,BlinkMacSystemFont,sans-serif;-webkit-font-smoothing:antialiased}
        .island{background:rgba(20,20,22,0.88);-webkit-backdrop-filter:blur(30px);
                border-radius:18px;padding:0 12px;height:32px;
                display:inline-flex;align-items:center;gap:7px;color:#fff;font-size:11px;
                box-shadow:0 1px 6px rgba(0,0,0,0.25)}
        .dot{width:6px;height:6px;border-radius:50%;display:inline-block;margin-right:1px}
        .busy-dot{background:#ff9500;animation:p 1.5s ease-in-out infinite}
        .idle-dot{background:#34c759}
        @keyframes p{0%,100%{opacity:1}50%{opacity:.4}}
        b{font-size:12px;font-weight:700}
        .lbl{color:#86868b;font-size:9px;margin-right:3px}
        .sep{width:1px;height:12px;background:#3a3a3c}
        .pill{display:inline-flex;align-items:center;gap:2px;padding:1px 7px;
              border-radius:9px;font-size:10px;font-weight:600}
        .pill.busy{background:rgba(255,149,0,.2);color:#ff9500}
        .pill.idle{background:rgba(52,199,89,.15);color:#34c759}
        .cpu{font-size:9px;opacity:.7}
        .empty{color:#636366;font-size:10px}
        </style></head><body><div class="island">\(body)</div></body></html>
        """
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
