import AppKit

// === AgentDesk Menu Bar Status Item ===
// Native macOS NSStatusItem in the system menu bar
// Shows agent count + status, click to expand details

class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    var statusItem: NSStatusItem!
    var timer: Timer?
    var parentPid: pid_t = 0
    var agents: [[String: Any]] = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        parentPid = getppid()

        // Create status item in the system menu bar
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        updateStatusItem()

        // Poll every 2s
        timer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            if kill(self.parentPid, 0) != 0 {
                NSApp.terminate(nil)
                return
            }
            self.updateStatusItem()
        }

        signal(SIGTERM) { _ in NSApp.terminate(nil) }
    }

    func applicationWillTerminate(_ notification: Notification) {
        timer?.invalidate()
        NSStatusBar.system.removeStatusItem(statusItem)
        try? FileManager.default.removeItem(atPath: stateFilePath())
    }

    func stateFilePath() -> String {
        NSHomeDirectory() + "/.agentdesk/island_state.json"
    }

    func updateStatusItem() {
        // Read state
        if let data = try? Data(contentsOf: URL(fileURLWithPath: stateFilePath())),
           let json = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] {
            agents = json
        } else {
            agents = []
        }

        let busyCount = agents.filter { ($0["status"] as? String) == "busy" }.count
        let idleCount = agents.filter { ($0["status"] as? String) == "idle" }.count
        let total = busyCount + idleCount

        // Update button appearance
        guard let button = statusItem.button else { return }

        let attributed = NSMutableAttributedString()

        if total == 0 {
            let text = NSAttributedString(string: "🤖 待机", attributes: [
                .font: NSFont.systemFont(ofSize: 12, weight: .medium),
                .foregroundColor: NSColor.secondaryLabelColor
            ])
            attributed.append(text)
        } else {
            // Busy indicator
            if busyCount > 0 {
                let dot = NSAttributedString(string: "🟠 ", attributes: [
                    .font: NSFont.systemFont(ofSize: 10)
                ])
                attributed.append(dot)
                let count = NSAttributedString(string: "\(busyCount)工作 ", attributes: [
                    .font: NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .bold),
                    .foregroundColor: NSColor.labelColor
                ])
                attributed.append(count)
            }

            // Idle indicator
            if idleCount > 0 {
                let dot = NSAttributedString(string: "🟢 ", attributes: [
                    .font: NSFont.systemFont(ofSize: 10)
                ])
                attributed.append(dot)
                let count = NSAttributedString(string: "\(idleCount)空闲", attributes: [
                    .font: NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .medium),
                    .foregroundColor: NSColor.secondaryLabelColor
                ])
                attributed.append(count)
            }
        }

        button.attributedTitle = attributed

        // Build menu
        let menu = NSMenu()
        menu.delegate = self

        if total == 0 {
            let item = NSMenuItem(title: "暂无活跃 Agent", action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
        } else {
            // Header
            let header = NSMenuItem(title: "活跃 Agent (\(total))", action: nil, keyEquivalent: "")
            header.isEnabled = false
            menu.addItem(header)
            menu.addItem(NSMenuItem.separator())

            for agent in agents {
                let name = agent["project"] as? String ?? "unknown"
                let status = agent["status"] as? String ?? "idle"
                let cpu = agent["cpu"] as? Double ?? 0
                let agentType = agent["type"] as? String ?? "Agent"
                let pid = agent["pid"] as? Int ?? 0

                let icon = status == "busy" ? "🟠" : "🟢"
                let statusText = status == "busy" ? "工作中" : "空闲"
                let title = "\(icon) \(name) — \(agentType) [\(statusText) CPU:\(String(format: "%.0f", cpu))%]"

                let item = NSMenuItem(title: title, action: #selector(agentClicked(_:)), keyEquivalent: "")
                item.target = self
                item.representedObject = agent
                item.toolTip = "PID: \(pid)"
                menu.addItem(item)
            }
        }

        menu.addItem(NSMenuItem.separator())

        let openApp = NSMenuItem(title: "打开 AgentDesk", action: #selector(openAgentDesk), keyEquivalent: "")
        openApp.target = self
        menu.addItem(openApp)

        let quit = NSMenuItem(title: "隐藏状态栏图标", action: #selector(quitApp), keyEquivalent: "q")
        quit.target = self
        menu.addItem(quit)

        statusItem.menu = menu
    }

    @objc func agentClicked(_ sender: NSMenuItem) {
        guard let agent = sender.representedObject as? [String: Any] else { return }
        // Try to focus terminal via tty
        // For now, just activate the terminal app
        NSWorkspace.shared.launchApplication("iTerm")
    }

    @objc func openAgentDesk() {
        // Bring AgentDesk window to front
        let apps = NSRunningApplication.runningApplications(withBundleIdentifier: "com.agentdesk.app")
        if let app = apps.first {
            app.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
        } else {
            // Fallback: activate by name
            for app in NSWorkspace.shared.runningApplications {
                if app.localizedName == "agentdesk" || app.localizedName == "AgentDesk" {
                    app.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
                    break
                }
            }
        }
    }

    @objc func quitApp() {
        NSApp.terminate(nil)
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)  // No dock icon
app.run()
