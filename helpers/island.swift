import AppKit
import WebKit

// === AgentDesk Dynamic Island ===
// Floating overlay that sits on top of the macOS notch area
// Auto-adapts to screen size and notch presence

class IslandPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    var panel: IslandPanel!
    var webView: WKWebView!
    var timer: Timer?
    var parentPid: pid_t = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        parentPid = getppid()

        guard let screen = NSScreen.main else { return }
        let sf = screen.frame

        let islandHeight: CGFloat = 32
        let islandWidth: CGFloat = 280

        // Dynamic Island hangs down from the very top of the screen
        // Like iPhone: the black pill is anchored to the top edge
        let x = sf.midX - islandWidth / 2
        let y = sf.maxY - islandHeight  // flush with the top edge of the screen

        let frame = NSRect(x: x, y: y, width: islandWidth, height: islandHeight)

        panel = IslandPanel(
            contentRect: frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        // Above everything, including menu bar
        panel.level = NSWindow.Level(Int(CGShieldingWindowLevel()))
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = false
        panel.isMovableByWindowBackground = false

        let config = WKWebViewConfiguration()
        webView = WKWebView(frame: NSRect(x: 0, y: 0, width: islandWidth, height: islandHeight), configuration: config)
        webView.setValue(false, forKey: "drawsBackground")
        panel.contentView = webView
        panel.orderFront(nil)

        updateIsland()

        timer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            if kill(self.parentPid, 0) != 0 {
                self.cleanup()
                NSApp.terminate(nil)
                return
            }
            self.updateIsland()
        }

        // Listen for screen changes (resolution, display connect/disconnect)
        NotificationCenter.default.addObserver(
            self, selector: #selector(screenChanged),
            name: NSApplication.didChangeScreenParametersNotification, object: nil
        )

        signal(SIGTERM) { _ in NSApp.terminate(nil) }
    }

    @objc func screenChanged() {
        updateIsland()
    }

    func applicationWillTerminate(_ notification: Notification) {
        cleanup()
    }

    func cleanup() {
        timer?.invalidate()
        try? FileManager.default.removeItem(atPath: NSHomeDirectory() + "/.agentdesk/island_state.json")
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

        // Build pills
        var pills = ""
        for a in busyAgents {
            let name = a["project"] as? String ?? "?"
            let cpu = a["cpu"] as? Double ?? 0
            pills += "<span class='p b'>\(esc(name))<span class='c'>\(String(format:"%.0f",cpu))%</span></span>"
        }
        for a in idleAgents {
            let name = a["project"] as? String ?? "?"
            pills += "<span class='p i'>\(esc(name))</span>"
        }

        let body: String
        if total == 0 {
            body = "<span class='e'>暂无活跃 Agent</span>"
        } else {
            var s = ""
            if busyCount > 0 { s += "<span class='d bd'></span><b>\(busyCount)</b><span class='l'>工作中</span>" }
            if idleCount > 0 { s += "<span class='d id'></span><b>\(idleCount)</b><span class='l'>空闲</span>" }
            if !pills.isEmpty { s += "<span class='sp'></span>" }
            body = s + pills
        }

        let html = """
        <!DOCTYPE html><html><head><meta charset="utf-8"><style>
        *{margin:0;padding:0;box-sizing:border-box}
        html,body{background:transparent;overflow:hidden;height:100%}
        body{display:flex;align-items:flex-start;justify-content:center;
             font-family:-apple-system,BlinkMacSystemFont,sans-serif;-webkit-font-smoothing:antialiased}
        .is{background:#000;border-radius:0 0 16px 16px;padding:0 14px;height:30px;
            display:inline-flex;align-items:center;gap:6px;color:#fff;font-size:11px;
            box-shadow:0 2px 8px rgba(0,0,0,0.3)}
        .d{width:6px;height:6px;border-radius:50%;display:inline-block;margin-right:1px}
        .bd{background:#ff9500;animation:p 1.5s ease-in-out infinite}
        .id{background:#34c759}
        @keyframes p{0%,100%{opacity:1}50%{opacity:.35}}
        b{font-size:12px;font-weight:700}
        .l{color:#888;font-size:9px;margin-right:2px}
        .sp{width:1px;height:12px;background:#333}
        .p{display:inline-flex;align-items:center;gap:2px;padding:1px 7px;
           border-radius:9px;font-size:10px;font-weight:600;max-width:90px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
        .p.b{background:rgba(255,149,0,.2);color:#ff9500}
        .p.i{background:rgba(52,199,89,.12);color:#34c759}
        .c{font-size:9px;opacity:.7}
        .e{color:#555;font-size:10px}
        </style></head><body><div class="is">\(body)</div></body></html>
        """

        webView.loadHTMLString(html, baseURL: nil)

        // Resize and reposition — always flush with top edge of screen
        guard let screen = NSScreen.main else { return }
        let sf = screen.frame
        let w: CGFloat = total == 0 ? 180 : min(CGFloat(160 + total * 90), 520)
        let h: CGFloat = 32
        let x = sf.midX - w / 2
        let y = sf.maxY - h  // anchored to top edge
        panel.setFrame(NSRect(x: x, y: y, width: w, height: h), display: true, animate: true)
    }

    func esc(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;")
         .replacingOccurrences(of: "<", with: "&lt;")
         .replacingOccurrences(of: ">", with: "&gt;")
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
