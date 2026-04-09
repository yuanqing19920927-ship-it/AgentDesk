import AppKit
import Darwin
import Foundation
import QuartzCore

// MARK: - Data Model

struct AgentEntry: Decodable {
    let pid: Int
    let type: String
    let status: String
    let cpu: Double
    let project: String
    let tty: String?

    var isBusy: Bool { status == "busy" }
    var statusLabel: String { isBusy ? "工作中" : "空闲" }
    var accentColor: NSColor { isBusy ? .systemOrange : .systemGreen }
}

// MARK: - NSScreen Notch Detection

extension NSScreen {
    var hasNotch: Bool {
        if #available(macOS 12.0, *) {
            return safeAreaInsets.top > 0
        }
        return false
    }

    var notchSize: NSSize? {
        guard hasNotch else { return nil }
        if #available(macOS 12.0, *) {
            guard let leftWidth = auxiliaryTopLeftArea?.width,
                  let rightWidth = auxiliaryTopRightArea?.width else { return nil }
            let notchWidth = frame.width - leftWidth - rightWidth
            let notchHeight = safeAreaInsets.top
            return NSSize(width: notchWidth, height: notchHeight)
        }
        return nil
    }

    /// The visual menu bar height (using NSStatusBar.thickness for accuracy).
    var menuBarHeight: CGFloat {
        NSStatusBar.system.thickness
    }
}

// MARK: - Notch Shape Layer

/// Creates a CAShapeLayer that mimics the MacBook notch shape with smooth curves.
/// The shape is a rounded rectangle that extends from the top edge, with the top
/// corners having a tighter radius (blending into the physical notch) and the
/// bottom corners having a larger, more prominent radius.
final class NotchShapeLayer: CAShapeLayer {
    @objc dynamic var topCornerRadius: CGFloat = 6 {
        didSet { updatePath() }
    }
    @objc dynamic var bottomCornerRadius: CGFloat = 20 {
        didSet { updatePath() }
    }

    override var bounds: CGRect {
        didSet { updatePath() }
    }

    private func updatePath() {
        let w = bounds.width
        let h = bounds.height
        guard w > 0, h > 0 else { return }

        let tr = topCornerRadius   // "top" = flush with screen top = maxY in macOS coords
        let br = bottomCornerRadius // "bottom" = visible rounded bottom = minY in macOS coords

        // macOS CALayer: origin at bottom-left, Y increases upward.
        // "Top of screen" = y=h, "bottom of island" = y=0.
        // Top edge is flat (flush with screen top).
        // Top corners are concave (shape narrows toward screen edge).
        // Bottom corners are convex (visible rounded corners).
        let path = CGMutablePath()

        // Start at top-left (screen top = y=h)
        path.move(to: CGPoint(x: 0, y: h))

        // Top-left concave curve: from (0, h) curving inward-down to (tr, h - tr)
        path.addQuadCurve(
            to: CGPoint(x: tr, y: h - tr),
            control: CGPoint(x: tr, y: h)
        )

        // Left edge going down to bottom-left area
        path.addLine(to: CGPoint(x: tr, y: br))

        // Bottom-left convex curve: from (tr, br) curving out to (tr + br, 0)
        path.addQuadCurve(
            to: CGPoint(x: tr + br, y: 0),
            control: CGPoint(x: tr, y: 0)
        )

        // Bottom edge
        path.addLine(to: CGPoint(x: w - tr - br, y: 0))

        // Bottom-right convex curve
        path.addQuadCurve(
            to: CGPoint(x: w - tr, y: br),
            control: CGPoint(x: w - tr, y: 0)
        )

        // Right edge going up
        path.addLine(to: CGPoint(x: w - tr, y: h - tr))

        // Top-right concave curve: from (w - tr, h - tr) curving up to (w, h)
        path.addQuadCurve(
            to: CGPoint(x: w, y: h),
            control: CGPoint(x: w - tr, y: h)
        )

        // Top edge back to start
        path.addLine(to: CGPoint(x: 0, y: h))
        path.closeSubpath()

        self.path = path
    }
}

// MARK: - Floating Panel

final class FloatingPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

// MARK: - Clickable Container

final class ClickableContainerView: NSView {
    var onClick: (() -> Void)?
    var normalBackgroundColor: NSColor = .clear {
        didSet { updateBackground() }
    }
    var hoverBackgroundColor: NSColor = .clear {
        didSet { updateBackground() }
    }

    private var tracking: NSTrackingArea?
    private var hovering = false {
        didSet { updateBackground() }
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        translatesAutoresizingMaskIntoConstraints = false
        layer?.cornerCurve = .continuous
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func mouseDown(with event: NSEvent) {
        onClick?()
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking {
            removeTrackingArea(tracking)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeAlways, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) {
        hovering = true
    }

    override func mouseExited(with event: NSEvent) {
        hovering = false
    }

    private func updateBackground() {
        layer?.backgroundColor = (hovering ? hoverBackgroundColor : normalBackgroundColor).cgColor
    }
}

// MARK: - Island Content View

final class IslandContentView: NSView {
    var onToggleExpanded: (() -> Void)?
    var onOpenApp: (() -> Void)?
    var onQuitApp: (() -> Void)?
    var onFocusAgent: ((AgentEntry) -> Void)?

    private let rootStack = NSStackView()
    private var contentInsets = NSEdgeInsets(top: 10, left: 14, bottom: 10, right: 14)
    private let maxVisibleAgents = 5

    private var agents: [AgentEntry] = []
    private var expanded = false

    // Stored constraints so we can update insets dynamically
    private var leadingConstraint: NSLayoutConstraint!
    private var trailingConstraint: NSLayoutConstraint!
    private var topConstraint: NSLayoutConstraint!
    private var bottomConstraint: NSLayoutConstraint!

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true

        rootStack.orientation = .vertical
        rootStack.alignment = .leading
        rootStack.spacing = 6
        rootStack.translatesAutoresizingMaskIntoConstraints = false

        addSubview(rootStack)
        leadingConstraint = rootStack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: contentInsets.left)
        trailingConstraint = rootStack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -contentInsets.right)
        topConstraint = rootStack.topAnchor.constraint(equalTo: topAnchor, constant: contentInsets.top)
        bottomConstraint = rootStack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -contentInsets.bottom)
        NSLayoutConstraint.activate([leadingConstraint, trailingConstraint, topConstraint, bottomConstraint])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var intrinsicContentSize: NSSize {
        fittingSize
    }

    override var fittingSize: NSSize {
        layoutSubtreeIfNeeded()
        let content = rootStack.fittingSize
        return NSSize(
            width: content.width + contentInsets.left + contentInsets.right,
            height: content.height + contentInsets.top + contentInsets.bottom
        )
    }

    func render(agents: [AgentEntry], expanded: Bool, collapsedHeight: CGFloat = 24) {
        self.agents = agents
        self.expanded = expanded
        if expanded {
            // Left/right must exceed NotchShape's topCornerRadius (19pt) to avoid clipping
            contentInsets = NSEdgeInsets(top: 12, left: 48, bottom: 14, right: 48)
        } else {
            // Vertically center content within menu bar height
            let vPad = max(2, (collapsedHeight - 20) / 2)
            contentInsets = NSEdgeInsets(top: vPad, left: 8, bottom: vPad, right: 8)
        }
        rebuild()
    }

    private func rebuild() {
        rootStack.arrangedSubviews.forEach {
            rootStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }

        // Update all inset constraints
        leadingConstraint.constant = contentInsets.left
        trailingConstraint.constant = -contentInsets.right
        topConstraint.constant = contentInsets.top
        bottomConstraint.constant = -contentInsets.bottom

        rootStack.addArrangedSubview(makeSummaryRow())

        if expanded {
            if agents.isEmpty {
                rootStack.addArrangedSubview(makeHintLabel("暂无活跃 Agent，点按下方按钮返回主应用。"))
            } else {
                for agent in agents.prefix(maxVisibleAgents) {
                    rootStack.addArrangedSubview(makeAgentRow(agent))
                }
                if agents.count > maxVisibleAgents {
                    rootStack.addArrangedSubview(
                        makeHintLabel("还有 \(agents.count - maxVisibleAgents) 个 Agent，回到 AgentDesk 查看全部。")
                    )
                }
                if agents.contains(where: { $0.tty != nil }) {
                    rootStack.addArrangedSubview(makeHintLabel("点按条目可直接跳转到对应终端。"))
                }
            }

            rootStack.addArrangedSubview(makeDivider())
            rootStack.addArrangedSubview(makeFooterRow())

            // Make all rows stretch to fill the stack width
            for view in rootStack.arrangedSubviews {
                view.widthAnchor.constraint(equalTo: rootStack.widthAnchor).isActive = true
            }
        }

        invalidateIntrinsicContentSize()
        needsLayout = true
        layoutSubtreeIfNeeded()
    }

    private func makeSummaryRow() -> NSView {
        let total = agents.count
        let busyCount = agents.filter(\.isBusy).count
        let idleCount = total - busyCount

        let row = ClickableContainerView()
        row.normalBackgroundColor = NSColor.white.withAlphaComponent(0.06)
        row.hoverBackgroundColor = NSColor.white.withAlphaComponent(0.12)
        row.layer?.cornerRadius = 14
        row.onClick = { [weak self] in self?.onToggleExpanded?() }

        let stack = NSStackView()
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = 6
        stack.translatesAutoresizingMaskIntoConstraints = false

        if total == 0 {
            stack.addArrangedSubview(makeDot(NSColor.systemGray))
            stack.addArrangedSubview(makeLabel(
                "暂无活跃 Agent",
                font: .systemFont(ofSize: 11, weight: .medium),
                color: NSColor(calibratedWhite: 0.88, alpha: 0.96)
            ))
        } else {
            if busyCount > 0 {
                stack.addArrangedSubview(makeCountPill(text: "\(busyCount) 工作中", tint: .systemOrange))
            }
            if idleCount > 0 {
                stack.addArrangedSubview(makeCountPill(text: "\(idleCount) 空闲", tint: .systemGreen))
            }

            let spacer = NSView()
            spacer.translatesAutoresizingMaskIntoConstraints = false
            spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
            stack.addArrangedSubview(spacer)

            stack.addArrangedSubview(makeLabel(
                "\(total) 个 Agent",
                font: .monospacedDigitSystemFont(ofSize: 10, weight: .medium),
                color: NSColor(calibratedWhite: 0.72, alpha: 0.96)
            ))
        }

        let chevron = makeLabel(
            expanded ? "▲" : "▼",
            font: .systemFont(ofSize: 8, weight: .bold),
            color: NSColor(calibratedWhite: 0.6, alpha: 0.96)
        )
        stack.addArrangedSubview(chevron)

        row.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: row.leadingAnchor, constant: 10),
            stack.trailingAnchor.constraint(equalTo: row.trailingAnchor, constant: -10),
            stack.topAnchor.constraint(equalTo: row.topAnchor, constant: 6),
            stack.bottomAnchor.constraint(equalTo: row.bottomAnchor, constant: -6),
            row.widthAnchor.constraint(greaterThanOrEqualToConstant: total == 0 ? 170 : 240),
        ])

        return row
    }

    private func makeAgentRow(_ agent: AgentEntry) -> NSView {
        let row = ClickableContainerView()
        row.normalBackgroundColor = agent.accentColor.withAlphaComponent(0.14)
        row.hoverBackgroundColor = agent.accentColor.withAlphaComponent(0.24)
        row.layer?.cornerRadius = 12
        row.toolTip = agent.tty == nil ? "打开 AgentDesk" : "跳转到对应终端"
        row.onClick = { [weak self] in self?.onFocusAgent?(agent) }

        let content = NSStackView()
        content.orientation = .horizontal
        content.alignment = .centerY
        content.spacing = 8
        content.translatesAutoresizingMaskIntoConstraints = false

        content.addArrangedSubview(makeDot(agent.accentColor))

        let textStack = NSStackView()
        textStack.orientation = .vertical
        textStack.alignment = .leading
        textStack.spacing = 1
        textStack.translatesAutoresizingMaskIntoConstraints = false

        let projectLabel = makeLabel(
            agent.project,
            font: .systemFont(ofSize: 12, weight: .semibold),
            color: .white
        )
        projectLabel.lineBreakMode = .byTruncatingTail
        projectLabel.maximumNumberOfLines = 1
        textStack.addArrangedSubview(projectLabel)

        let meta = "\(agent.statusLabel) · \(agent.type) · CPU \(Int(agent.cpu.rounded()))%"
        textStack.addArrangedSubview(makeLabel(
            meta,
            font: .monospacedDigitSystemFont(ofSize: 9, weight: .medium),
            color: NSColor(calibratedWhite: 0.82, alpha: 0.92)
        ))

        content.addArrangedSubview(textStack)

        let spacer = NSView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        content.addArrangedSubview(spacer)

        content.addArrangedSubview(makeTag(text: agent.tty == nil ? "应用" : "终端"))

        row.addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: row.leadingAnchor, constant: 14),
            content.trailingAnchor.constraint(equalTo: row.trailingAnchor, constant: -14),
            content.topAnchor.constraint(equalTo: row.topAnchor, constant: 8),
            content.bottomAnchor.constraint(equalTo: row.bottomAnchor, constant: -8),
            row.widthAnchor.constraint(greaterThanOrEqualToConstant: 320),
        ])

        return row
    }

    private func makeFooterRow() -> NSView {
        let row = NSStackView()
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 8
        row.translatesAutoresizingMaskIntoConstraints = false

        // "打开 AgentDesk" button (left)
        let openBtn = ClickableContainerView()
        openBtn.normalBackgroundColor = NSColor.white.withAlphaComponent(0.08)
        openBtn.hoverBackgroundColor = NSColor.white.withAlphaComponent(0.16)
        openBtn.layer?.cornerRadius = 12
        openBtn.onClick = { [weak self] in self?.onOpenApp?() }

        let openLabel = makeLabel(
            "打开 AgentDesk",
            font: .systemFont(ofSize: 11, weight: .semibold),
            color: .white
        )
        openLabel.translatesAutoresizingMaskIntoConstraints = false
        openBtn.addSubview(openLabel)
        NSLayoutConstraint.activate([
            openLabel.leadingAnchor.constraint(equalTo: openBtn.leadingAnchor, constant: 12),
            openLabel.trailingAnchor.constraint(equalTo: openBtn.trailingAnchor, constant: -12),
            openLabel.topAnchor.constraint(equalTo: openBtn.topAnchor, constant: 8),
            openLabel.bottomAnchor.constraint(equalTo: openBtn.bottomAnchor, constant: -8),
        ])
        row.addArrangedSubview(openBtn)

        // Spacer
        let spacer = NSView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        row.addArrangedSubview(spacer)

        // "退出" button (right)
        let quitBtn = ClickableContainerView()
        quitBtn.normalBackgroundColor = NSColor.systemRed.withAlphaComponent(0.15)
        quitBtn.hoverBackgroundColor = NSColor.systemRed.withAlphaComponent(0.30)
        quitBtn.layer?.cornerRadius = 12
        quitBtn.onClick = { [weak self] in self?.onQuitApp?() }

        let quitLabel = makeLabel(
            "退出",
            font: .systemFont(ofSize: 11, weight: .semibold),
            color: NSColor.systemRed
        )
        quitLabel.translatesAutoresizingMaskIntoConstraints = false
        quitBtn.addSubview(quitLabel)
        NSLayoutConstraint.activate([
            quitLabel.leadingAnchor.constraint(equalTo: quitBtn.leadingAnchor, constant: 12),
            quitLabel.trailingAnchor.constraint(equalTo: quitBtn.trailingAnchor, constant: -12),
            quitLabel.topAnchor.constraint(equalTo: quitBtn.topAnchor, constant: 8),
            quitLabel.bottomAnchor.constraint(equalTo: quitBtn.bottomAnchor, constant: -8),
        ])
        row.addArrangedSubview(quitBtn)

        return row
    }

    private func makeDivider() -> NSView {
        let divider = NSBox()
        divider.boxType = .custom
        divider.isTransparent = false
        divider.fillColor = NSColor.white.withAlphaComponent(0.08)
        divider.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            divider.heightAnchor.constraint(equalToConstant: 1),
        ])
        return divider
    }

    private func makeHintLabel(_ text: String) -> NSView {
        let label = makeLabel(
            text,
            font: .systemFont(ofSize: 10, weight: .medium),
            color: NSColor(calibratedWhite: 0.7, alpha: 0.95)
        )
        label.maximumNumberOfLines = 0
        label.lineBreakMode = .byWordWrapping
        label.setContentCompressionResistancePriority(.required, for: .vertical)
        return label
    }

    private func makeCountPill(text: String, tint: NSColor) -> NSView {
        let pill = NSStackView()
        pill.orientation = .horizontal
        pill.alignment = .centerY
        pill.spacing = 5
        pill.edgeInsets = NSEdgeInsets(top: 3, left: 7, bottom: 3, right: 7)
        pill.translatesAutoresizingMaskIntoConstraints = false
        pill.wantsLayer = true
        pill.layer?.backgroundColor = tint.withAlphaComponent(0.18).cgColor
        pill.layer?.cornerRadius = 10
        pill.layer?.cornerCurve = .continuous

        pill.addArrangedSubview(makeDot(tint))
        pill.addArrangedSubview(makeLabel(
            text,
            font: .monospacedDigitSystemFont(ofSize: 10, weight: .bold),
            color: .white
        ))
        return pill
    }

    private func makeTag(text: String) -> NSView {
        let tag = NSStackView()
        tag.orientation = .horizontal
        tag.alignment = .centerY
        tag.edgeInsets = NSEdgeInsets(top: 2, left: 6, bottom: 2, right: 6)
        tag.translatesAutoresizingMaskIntoConstraints = false
        tag.wantsLayer = true
        tag.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.1).cgColor
        tag.layer?.cornerRadius = 8
        tag.layer?.cornerCurve = .continuous
        tag.addArrangedSubview(makeLabel(
            text,
            font: .systemFont(ofSize: 9, weight: .semibold),
            color: NSColor(calibratedWhite: 0.88, alpha: 0.98)
        ))
        return tag
    }

    private func makeDot(_ color: NSColor) -> NSView {
        let dot = NSView()
        dot.wantsLayer = true
        dot.layer?.backgroundColor = color.cgColor
        dot.layer?.cornerRadius = 3.5
        dot.layer?.cornerCurve = .continuous
        dot.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            dot.widthAnchor.constraint(equalToConstant: 7),
            dot.heightAnchor.constraint(equalToConstant: 7),
        ])
        return dot
    }

    private func makeLabel(_ text: String, font: NSFont, color: NSColor) -> NSTextField {
        let label = NSTextField(labelWithString: text)
        label.font = font
        label.textColor = color
        label.backgroundColor = .clear
        label.drawsBackground = false
        label.isBezeled = false
        label.isEditable = false
        label.isSelectable = false
        label.setContentCompressionResistancePriority(.defaultHigh, for: .horizontal)
        label.translatesAutoresizingMaskIntoConstraints = false
        return label
    }
}

// MARK: - Island Container (handles notch shape vs floating capsule)

final class IslandContainerView: NSView {
    let contentView: IslandContentView
    private(set) var maskLayer: NotchShapeLayer?
    private(set) var backgroundLayer: CALayer?
    var isMaskAnimating = false

    init() {
        self.contentView = IslandContentView(frame: .zero)
        super.init(frame: .zero)
        wantsLayer = true
        layer?.masksToBounds = true

        contentView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(contentView)
        NSLayoutConstraint.activate([
            contentView.leadingAnchor.constraint(equalTo: leadingAnchor),
            contentView.trailingAnchor.constraint(equalTo: trailingAnchor),
            contentView.topAnchor.constraint(equalTo: topAnchor),
            contentView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])

        setupAppearance()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private var maskAnimTimer: Timer?

    private func setupAppearance() {
        maskLayer = nil
        backgroundLayer?.removeFromSuperlayer()
        backgroundLayer = nil
        layer?.mask = nil
        layer?.cornerRadius = 0
        layer?.backgroundColor = nil

        let bg = CALayer()
        bg.backgroundColor = NSColor.black.cgColor
        layer?.insertSublayer(bg, at: 0)
        backgroundLayer = bg

        let mask = NotchShapeLayer()
        mask.topCornerRadius = 6
        mask.bottomCornerRadius = 14
        mask.fillColor = NSColor.white.cgColor
        layer?.mask = mask
        maskLayer = mask

        layer?.shadowColor = NSColor.black.cgColor
        layer?.shadowOpacity = 0
        layer?.shadowRadius = 6
        layer?.shadowOffset = .zero
    }

    func updateShadow(expanded: Bool) {
        layer?.shadowOpacity = expanded ? 0.7 : 0
    }

    /// Animate mask between two sizes.
    /// NSView default (non-flipped): y=0 is BOTTOM, y=bounds.height is TOP.
    /// To anchor mask at the top edge: mask.origin.y = containerHeight - maskHeight.
    /// `onFrame` is called each frame with the eased progress (0…1) for synchronizing external state.
    func animateMask(
        fromSize: NSSize, toSize: NSSize,
        fromTR: CGFloat, fromBR: CGFloat,
        toTR: CGFloat, toBR: CGFloat,
        containerHeight: CGFloat,
        duration: TimeInterval = 0.3,
        onFrame: ((CGFloat) -> Void)? = nil,
        completion: @escaping () -> Void
    ) {
        guard let mask = maskLayer else { completion(); return }
        maskAnimTimer?.invalidate()
        isMaskAnimating = true

        let containerW = max(fromSize.width, toSize.width)
        let startTime = CACurrentMediaTime()

        maskAnimTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] timer in
            guard let self, let mask = self.maskLayer else {
                timer.invalidate()
                return
            }
            let elapsed = CACurrentMediaTime() - startTime
            var t = min(elapsed / duration, 1.0)
            t = t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t

            let w = fromSize.width + (toSize.width - fromSize.width) * t
            let h = fromSize.height + (toSize.height - fromSize.height) * t
            let tr = fromTR + (toTR - fromTR) * t
            let br = fromBR + (toBR - fromBR) * t

            CATransaction.begin()
            CATransaction.setDisableActions(true)

            // Non-flipped NSView: y=0 is bottom, y=maxY is top.
            // Anchor mask at top: y = containerH - h (so mask.maxY == containerH always)
            let maskY = containerHeight - h
            mask.frame = CGRect(x: (containerW - w) / 2, y: maskY, width: w, height: h)
            mask.topCornerRadius = tr
            mask.bottomCornerRadius = br
            self.backgroundLayer?.frame = self.bounds

            CATransaction.commit()

            onFrame?(t)

            if t >= 1.0 {
                timer.invalidate()
                self.maskAnimTimer = nil
                self.isMaskAnimating = false
                completion()
            }
        }
    }

    override func layout() {
        super.layout()
        backgroundLayer?.frame = bounds
        if !isMaskAnimating {
            maskLayer?.frame = bounds
        }
    }
}

// MARK: - App Delegate

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var panel: FloatingPanel!
    private var containerView: IslandContainerView!
    private var timer: Timer?
    private var globalClickMonitor: Any?
    private var localClickMonitor: Any?
    private var globalMoveMonitor: Any?
    private var collapseTimer: Timer?

    private var parentPid: pid_t = 0
    private var agents: [AgentEntry] = []
    private var expanded = false
    private var isAnimating = false
    private var stableFrame: NSRect = .zero  // The target frame (not mid-animation)
    private var lastToggleTime: TimeInterval = 0

    // Sizing constants
    private let collapsedWidth: CGFloat = 224
    private let expandedMinWidth: CGFloat = 600
    private let expandedMinHeight: CGFloat = 120

    func applicationDidFinishLaunching(_ notification: Notification) {
        parentPid = getppid()

        containerView = IslandContainerView()
        containerView.contentView.onToggleExpanded = { [weak self] in
            self?.setExpanded(!(self?.expanded ?? false))
        }
        containerView.contentView.onOpenApp = { [weak self] in
            self?.openAgentDesk()
        }
        containerView.contentView.onFocusAgent = { [weak self] agent in
            self?.focus(agent)
        }
        containerView.contentView.onQuitApp = { [weak self] in
            self?.quitAll()
        }

        panel = FloatingPanel(
            contentRect: NSRect(x: 0, y: 0, width: collapsedWidth + 12, height: 38),
            styleMask: [.borderless, .nonactivatingPanel, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        panel.contentView = containerView
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false  // We handle shadow ourselves via CALayer
        panel.level = NSWindow.Level(rawValue: NSWindow.Level.mainMenu.rawValue + 3)
        panel.collectionBehavior = [.canJoinAllSpaces, .stationary, .fullScreenAuxiliary, .ignoresCycle]
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.appearance = NSAppearance(named: .darkAqua)

        refreshState(animated: false)
        panel.orderFrontRegardless()

        installClickMonitors()

        timer = Timer.scheduledTimer(withTimeInterval: 1.5, repeats: true) { [weak self] _ in
            self?.refreshState(animated: false)
        }

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleScreenChange),
            name: NSApplication.didChangeScreenParametersNotification,
            object: nil
        )
    }

    func applicationWillTerminate(_ notification: Notification) {
        timer?.invalidate()
        timer = nil

        if let globalClickMonitor {
            NSEvent.removeMonitor(globalClickMonitor)
        }
        if let localClickMonitor {
            NSEvent.removeMonitor(localClickMonitor)
        }
        if let globalMoveMonitor {
            NSEvent.removeMonitor(globalMoveMonitor)
        }
        collapseTimer?.invalidate()
        panelAnimTimer?.invalidate()

        try? FileManager.default.removeItem(atPath: stateFilePath())
    }

    @objc private func handleScreenChange() {
        refreshState(animated: false)
    }

    private func installClickMonitors() {
        // Click outside to collapse
        globalClickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            guard let self, self.expanded else { return }
            let mousePos = NSEvent.mouseLocation
            if !self.panel.frame.contains(mousePos) {
                self.setExpanded(false)
            }
        }

        localClickMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            guard let self, self.expanded else { return event }
            let mousePos = NSEvent.mouseLocation
            if !self.panel.frame.contains(mousePos) {
                self.setExpanded(false)
            }
            return event
        }

        // Hover: expand on mouse enter, collapse on mouse leave (with delay)
        globalMoveMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved]) { [weak self] _ in
            self?.handleMouseMove()
        }
    }

    private func handleMouseMove() {
        guard !isAnimating else { return }
        let mousePos = NSEvent.mouseLocation
        // Use stableFrame (the last known target frame) to avoid mid-animation glitches
        let hitFrame = stableFrame

        if !expanded {
            // Mouse entered collapsed island → expand
            if hitFrame.contains(mousePos) {
                collapseTimer?.invalidate()
                collapseTimer = nil
                setExpanded(true)
            }
        } else {
            // Use a slightly padded frame for "inside" check to prevent edge flicker
            let padded = hitFrame.insetBy(dx: -8, dy: -8)
            if padded.contains(mousePos) {
                collapseTimer?.invalidate()
                collapseTimer = nil
            } else {
                // Mouse left expanded island → start collapse delay
                if collapseTimer == nil {
                    collapseTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: false) { [weak self] _ in
                        guard let self else { return }
                        let pos = NSEvent.mouseLocation
                        if !self.stableFrame.insetBy(dx: -8, dy: -8).contains(pos) {
                            self.setExpanded(false)
                        }
                        self.collapseTimer = nil
                    }
                }
            }
        }
    }

    private func setExpanded(_ newValue: Bool) {
        guard expanded != newValue, !isAnimating else { return }
        // Debounce: at least 0.5s between toggles
        let now = ProcessInfo.processInfo.systemUptime
        guard now - lastToggleTime > 0.5 else { return }
        lastToggleTime = now
        expanded = newValue

        // Haptic feedback
        NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .default)

        refreshState(animated: true)
    }

    private func refreshState(animated: Bool) {
        if kill(parentPid, 0) != 0 {
            NSApp.terminate(nil)
            return
        }

        // Skip non-animated refresh while animation is in progress
        if !animated && isAnimating { return }

        agents = loadAgents()
        guard let screen = currentScreen() else { return }
        let menuBarH = screen.menuBarHeight
        let oldSize = panel.frame.size

        if !animated {
            // No animation: set everything instantly
            containerView.contentView.render(agents: agents, expanded: expanded, collapsedHeight: menuBarH)
            containerView.updateShadow(expanded: expanded)
            panel.ignoresMouseEvents = !expanded
            let frame = calculateFrame(screen: screen, menuBarH: menuBarH)
            stableFrame = frame
            panel.setFrame(frame, display: false)

            // Explicitly reset mask to cover full panel — don't rely on layout()
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            if let mask = containerView.layer?.mask as? NotchShapeLayer {
                mask.frame = CGRect(origin: .zero, size: frame.size)
                mask.topCornerRadius = expanded ? 19 : 6
                mask.bottomCornerRadius = expanded ? 24 : 14
            }
            containerView.backgroundLayer?.frame = CGRect(origin: .zero, size: frame.size)
            CATransaction.commit()

            panel.display()
            panel.orderFrontRegardless()
            return
        }

        if expanded {
            // EXPANDING: render first to get correct size, then atomic mask setup
            containerView.updateShadow(expanded: true)
            panel.ignoresMouseEvents = false

            // === All changes in one atomic block ===
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            containerView.isMaskAnimating = true

            // 1. Render expanded content FIRST to get correct fitting size
            containerView.contentView.render(agents: agents, expanded: true, collapsedHeight: menuBarH)

            // 2. Now calculate frame with correct expanded content size
            let expandedFrame = calculateFrame(screen: screen, menuBarH: menuBarH)
            stableFrame = expandedFrame

            // 3. Resize panel
            panel.setFrame(expandedFrame, display: false)

            if oldSize != expandedFrame.size {
                isAnimating = true

                // 4. Pin mask at collapsed size, top-anchored in expanded container
                // Non-flipped coords: y=0 is bottom, top-anchor = containerH - maskH
                if let mask = containerView.layer?.mask as? NotchShapeLayer {
                    let pinY = expandedFrame.height - oldSize.height
                    mask.frame = CGRect(
                        x: (expandedFrame.width - oldSize.width) / 2,
                        y: pinY,
                        width: oldSize.width,
                        height: oldSize.height
                    )
                    mask.topCornerRadius = 6
                    mask.bottomCornerRadius = 14
                }

                CATransaction.commit()

                // Single display with correct initial state
                panel.display()
                panel.orderFrontRegardless()

                // Now animate mask from collapsed to expanded
                containerView.animateMask(
                    fromSize: oldSize, toSize: expandedFrame.size,
                    fromTR: 6, fromBR: 14, toTR: 19, toBR: 24,
                    containerHeight: expandedFrame.height,
                    duration: 0.45
                ) { [weak self] in
                    self?.isAnimating = false
                }
            } else {
                // No size change — just reset mask to full bounds
                if let mask = containerView.layer?.mask as? NotchShapeLayer {
                    mask.frame = CGRect(origin: .zero, size: expandedFrame.size)
                    mask.topCornerRadius = 19
                    mask.bottomCornerRadius = 24
                }
                containerView.backgroundLayer?.frame = CGRect(origin: .zero, size: expandedFrame.size)
                containerView.isMaskAnimating = false
                CATransaction.commit()
                panel.display()
                panel.orderFrontRegardless()
            }
        } else {
            // COLLAPSING: keep panel at expanded size, animate mask shrinking, THEN resize
            containerView.updateShadow(expanded: false)
            let collapsedFrame = calculateFrame(screen: screen, menuBarH: menuBarH)
            // Keep stableFrame at expanded size during animation for hover detection;
            // only update to collapsed after animation completes
            let expandedStable = stableFrame

            if oldSize != collapsedFrame.size {
                isAnimating = true
                let screenRef = screen

                // === Phase 1: Height collapse (mask shrinks vertically, width unchanged) ===
                let phase1To = NSSize(width: oldSize.width, height: menuBarH)
                containerView.animateMask(
                    fromSize: oldSize, toSize: phase1To,
                    fromTR: 19, fromBR: 24, toTR: 6, toBR: 14,
                    containerHeight: oldSize.height,
                    duration: 0.30
                ) { [weak self] in
                    guard let self else { return }

                    // Snap panel to menu-bar height, full expanded width
                    let midFrame = NSRect(
                        x: screenRef.frame.midX - oldSize.width / 2,
                        y: screenRef.frame.maxY - menuBarH,
                        width: oldSize.width,
                        height: menuBarH
                    )

                    CATransaction.begin()
                    CATransaction.setDisableActions(true)
                    self.containerView.isMaskAnimating = true
                    self.containerView.contentView.render(
                        agents: self.agents, expanded: false, collapsedHeight: menuBarH
                    )
                    self.panel.ignoresMouseEvents = true
                    self.panel.setFrame(midFrame, display: false)
                    if let mask = self.containerView.layer?.mask as? NotchShapeLayer {
                        mask.frame = CGRect(origin: .zero, size: midFrame.size)
                        mask.topCornerRadius = 6
                        mask.bottomCornerRadius = 14
                    }
                    self.containerView.backgroundLayer?.frame = CGRect(origin: .zero, size: midFrame.size)
                    self.containerView.isMaskAnimating = false
                    CATransaction.commit()
                    self.panel.display()

                    // Recalculate collapsed frame NOW (after rendering collapsed content)
                    // so fittingSize reflects actual collapsed width
                    let finalCollapsed = self.calculateFrame(screen: screenRef, menuBarH: menuBarH)

                    // === Phase 2: Width collapse (panel frame shrinks horizontally) ===
                    self.animatePanelWidth(
                        fromWidth: oldSize.width,
                        toWidth: finalCollapsed.width,
                        height: menuBarH,
                        screen: screenRef,
                        duration: 0.20
                    ) {
                        self.stableFrame = finalCollapsed
                        self.isAnimating = false
                    }
                }
            } else {
                containerView.contentView.render(agents: agents, expanded: false, collapsedHeight: menuBarH)
                panel.ignoresMouseEvents = true
                panel.setFrame(collapsedFrame, display: true)
                stableFrame = collapsedFrame
            }
            panel.orderFrontRegardless()
        }
    }

    private var panelAnimTimer: Timer?

    /// Smoothly animate panel width from `fromWidth` to `toWidth` at fixed height,
    /// keeping the panel centered horizontally at the screen top.
    private func animatePanelWidth(
        fromWidth: CGFloat, toWidth: CGFloat,
        height: CGFloat, screen: NSScreen,
        duration: TimeInterval,
        completion: @escaping () -> Void
    ) {
        panelAnimTimer?.invalidate()
        let startTime = CACurrentMediaTime()
        panelAnimTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] timer in
            guard let self else { timer.invalidate(); return }
            let elapsed = CACurrentMediaTime() - startTime
            var t = min(elapsed / duration, 1.0)
            t = t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t

            let w = fromWidth + (toWidth - fromWidth) * t
            let x = screen.frame.midX - w / 2
            let y = screen.frame.maxY - height
            let f = NSRect(x: x, y: y, width: w, height: height)

            CATransaction.begin()
            CATransaction.setDisableActions(true)
            self.panel.setFrame(f, display: false)
            self.containerView.backgroundLayer?.frame = CGRect(origin: .zero, size: f.size)
            if let mask = self.containerView.layer?.mask as? NotchShapeLayer {
                mask.frame = CGRect(origin: .zero, size: f.size)
            }
            CATransaction.commit()
            self.panel.display()

            if t >= 1.0 {
                timer.invalidate()
                self.panelAnimTimer = nil
                completion()
            }
        }
    }

    private func calculateFrame(screen: NSScreen, menuBarH: CGFloat) -> NSRect {
        let width: CGFloat
        let height: CGFloat

        if expanded {
            let contentSize = containerView.contentView.fittingSize
            width = max(contentSize.width + 38, expandedMinWidth)
            height = max(contentSize.height, expandedMinHeight)
        } else {
            let contentSize = containerView.contentView.fittingSize
            let topInset: CGFloat = 12
            width = max(contentSize.width + topInset, collapsedWidth + topInset)
            height = menuBarH
        }

        let x = screen.frame.midX - width / 2
        let y = screen.frame.maxY - height
        return NSRect(x: x, y: y, width: width, height: height)
    }

    private func currentScreen() -> NSScreen? {
        // Always use the primary screen (the one with the menu bar).
        // NSScreen.screens.first is guaranteed to be the primary display.
        return NSScreen.screens.first ?? NSScreen.main
    }

    private func loadAgents() -> [AgentEntry] {
        let url = URL(fileURLWithPath: stateFilePath())
        guard let data = try? Data(contentsOf: url) else {
            return []
        }

        let decoder = JSONDecoder()
        let decoded = (try? decoder.decode([AgentEntry].self, from: data)) ?? []
        return decoded.sorted { left, right in
            if left.isBusy != right.isBusy {
                return left.isBusy && !right.isBusy
            }
            if left.cpu != right.cpu {
                return left.cpu > right.cpu
            }
            return left.project.localizedCaseInsensitiveCompare(right.project) == .orderedAscending
        }
    }

    private func focus(_ agent: AgentEntry) {
        if let tty = agent.tty, focusTerminal(tty: tty) {
            setExpanded(false)
            return
        }

        openAgentDesk()
    }

    private func stateFilePath() -> String {
        NSHomeDirectory() + "/.agentdesk/island_state.json"
    }

    private func openAgentDesk() {
        if let app = NSRunningApplication(processIdentifier: parentPid) {
            app.activate(options: [.activateAllWindows])
        }
        setExpanded(false)
    }

    private func quitAll() {
        // Send SIGTERM to the parent AgentDesk process, which will clean up and exit
        kill(parentPid, SIGTERM)
        // Then terminate ourselves
        NSApp.terminate(nil)
    }

    private func focusTerminal(tty: String) -> Bool {
        let ttyDevice = "/dev/tty\(tty)"

        if FileManager.default.fileExists(atPath: "/Applications/iTerm.app") {
            let script = """
            tell application "iTerm2"
                activate
                repeat with w in windows
                    repeat with t in tabs of w
                        repeat with s in sessions of t
                            if tty of s is "\(escapeForAppleScript(ttyDevice))" then
                                select t
                                tell w to select
                                return "found"
                            end if
                        end repeat
                    end repeat
                end repeat
                return "not found"
            end tell
            """
            let result = runAppleScript(script) ?? ""
            return result.contains("found")
        }

        let activated = runAppleScript(#"tell application "Terminal" to activate"#) != nil
        return activated
    }

    private func runAppleScript(_ script: String) -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-e", script]

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return nil
        }

        guard process.terminationStatus == 0 else {
            return nil
        }

        let data = stdout.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func escapeForAppleScript(_ text: String) -> String {
        text.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
    }
}

// MARK: - Entry Point

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
