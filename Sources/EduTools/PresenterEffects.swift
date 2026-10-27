import AppKit
import ApplicationServices
import Carbon

enum KeystrokeDisplayFilter {
    static func displayString(
        keyCode: UInt16,
        charactersIgnoringModifiers: String?,
        modifierFlags: NSEvent.ModifierFlags
    ) -> String? {
        let relevantModifiers = modifierFlags.intersection([.control, .option, .shift, .command])
        guard !relevantModifiers.intersection([.control, .option, .command]).isEmpty,
              let key = keyLabel(
                  keyCode: keyCode,
                  charactersIgnoringModifiers: charactersIgnoringModifiers
              )
        else {
            return nil
        }

        var result = ""
        if relevantModifiers.contains(.control) { result += "⌃" }
        if relevantModifiers.contains(.option) { result += "⌥" }
        if relevantModifiers.contains(.shift) { result += "⇧" }
        if relevantModifiers.contains(.command) { result += "⌘" }
        return result + key
    }

    static func displayString(for event: NSEvent) -> String? {
        displayString(
            keyCode: event.keyCode,
            charactersIgnoringModifiers: event.charactersIgnoringModifiers,
            modifierFlags: event.modifierFlags
        )
    }

    private static func keyLabel(
        keyCode: UInt16,
        charactersIgnoringModifiers: String?
    ) -> String? {
        switch Int(keyCode) {
        case kVK_Return, kVK_ANSI_KeypadEnter: return "↩"
        case kVK_Tab: return "Tab"
        case kVK_Space: return "Space"
        case kVK_Delete: return "⌫"
        case kVK_ForwardDelete: return "⌦"
        case kVK_Escape: return "Esc"
        case kVK_LeftArrow: return "←"
        case kVK_RightArrow: return "→"
        case kVK_UpArrow: return "↑"
        case kVK_DownArrow: return "↓"
        default:
            guard let charactersIgnoringModifiers,
                  !charactersIgnoringModifiers.isEmpty,
                  charactersIgnoringModifiers.unicodeScalars.allSatisfy({ scalar in
                      !CharacterSet.controlCharacters.contains(scalar)
                          && !CharacterSet.whitespacesAndNewlines.contains(scalar)
                  })
            else {
                return nil
            }
            return charactersIgnoringModifiers.uppercased()
        }
    }
}

private final class PresenterEffectsPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

private final class PresenterEffectsView: NSView {
    private struct Ripple {
        let center: CGPoint
        let startedAt: TimeInterval
        let color: NSColor
    }

    var spotlightEnabled = false {
        didSet { needsDisplay = true }
    }
    var cursorHighlightEnabled = false {
        didSet { needsDisplay = true }
    }
    var spotlightRadius: CGFloat = 160 {
        didSet { needsDisplay = true }
    }
    var pointerLocation = CGPoint.zero {
        didSet { needsDisplay = true }
    }
    var transientContentDidChange: (() -> Void)?

    private var ripples: [Ripple] = []
    private var rippleTimer: Timer?
    private var keystrokeDisplay: String?
    private var keystrokeDismissal: DispatchWorkItem?

    override var isOpaque: Bool { false }

    var hasTransientContent: Bool {
        !ripples.isEmpty || keystrokeDisplay != nil
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        if spotlightEnabled, let context = NSGraphicsContext.current?.cgContext {
            NSColor.black.withAlphaComponent(0.72).setFill()
            bounds.fill()
            context.saveGState()
            context.setBlendMode(.clear)
            let diameter = spotlightRadius * 2
            context.fillEllipse(
                in: CGRect(
                    x: pointerLocation.x - spotlightRadius,
                    y: pointerLocation.y - spotlightRadius,
                    width: diameter,
                    height: diameter
                )
            )
            context.restoreGState()
        }

        if cursorHighlightEnabled {
            let circle = NSBezierPath(
                ovalIn: CGRect(
                    x: pointerLocation.x - 18,
                    y: pointerLocation.y - 18,
                    width: 36,
                    height: 36
                )
            )
            NSColor.systemYellow.withAlphaComponent(0.3).setFill()
            circle.fill()
            NSColor.systemOrange.withAlphaComponent(0.9).setStroke()
            circle.lineWidth = 2
            circle.stroke()
        }

        drawRipples()
        drawKeystrokeDisplay()
    }

    func addRipple(at center: CGPoint, color: NSColor) {
        ripples.append(
            Ripple(
                center: center,
                startedAt: ProcessInfo.processInfo.systemUptime,
                color: color
            )
        )
        ensureRippleTimer()
        needsDisplay = true
        transientContentDidChange?()
    }

    func showKeystroke(_ displayString: String) {
        keystrokeDismissal?.cancel()
        keystrokeDisplay = displayString
        needsDisplay = true
        transientContentDidChange?()

        let dismissal = DispatchWorkItem { [weak self] in
            self?.keystrokeDisplay = nil
            self?.needsDisplay = true
            self?.transientContentDidChange?()
        }
        keystrokeDismissal = dismissal
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: dismissal)
    }

    func clearKeystroke() {
        keystrokeDismissal?.cancel()
        keystrokeDismissal = nil
        keystrokeDisplay = nil
        needsDisplay = true
        transientContentDidChange?()
    }

    private func drawRipples() {
        let now = ProcessInfo.processInfo.systemUptime
        for ripple in ripples {
            let progress = min(max((now - ripple.startedAt) / 0.65, 0), 1)
            let radius = 12 + CGFloat(progress) * 38
            let path = NSBezierPath(
                ovalIn: CGRect(
                    x: ripple.center.x - radius,
                    y: ripple.center.y - radius,
                    width: radius * 2,
                    height: radius * 2
                )
            )
            ripple.color.withAlphaComponent(1 - progress).setStroke()
            path.lineWidth = 4 - CGFloat(progress) * 2
            path.stroke()
        }
    }

    private func drawKeystrokeDisplay() {
        guard let keystrokeDisplay else { return }
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 28, weight: .semibold),
            .foregroundColor: NSColor.white,
        ]
        let attributed = NSAttributedString(string: keystrokeDisplay, attributes: attributes)
        let textSize = attributed.size()
        let backgroundRect = CGRect(
            x: (bounds.width - textSize.width) / 2 - 22,
            y: 34,
            width: textSize.width + 44,
            height: textSize.height + 20
        )
        let background = NSBezierPath(roundedRect: backgroundRect, xRadius: 12, yRadius: 12)
        NSColor.black.withAlphaComponent(0.78).setFill()
        background.fill()
        attributed.draw(
            at: CGPoint(
                x: backgroundRect.minX + 22,
                y: backgroundRect.minY + 10
            )
        )
    }

    private func ensureRippleTimer() {
        guard rippleTimer == nil else { return }
        let timer = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] timer in
            guard let self else {
                timer.invalidate()
                return
            }
            let now = ProcessInfo.processInfo.systemUptime
            self.ripples.removeAll { now - $0.startedAt >= 0.65 }
            self.needsDisplay = true
            if self.ripples.isEmpty {
                timer.invalidate()
                self.rippleTimer = nil
                self.transientContentDidChange?()
            }
        }
        rippleTimer = timer
        RunLoop.main.add(timer, forMode: .common)
    }

    deinit {
        rippleTimer?.invalidate()
        keystrokeDismissal?.cancel()
    }
}

final class PresenterEffectsController {
    private static let mouseEventMask: NSEvent.EventTypeMask = [
        .mouseMoved,
        .leftMouseDragged,
        .rightMouseDragged,
        .otherMouseDragged,
        .leftMouseDown,
        .rightMouseDown,
        .scrollWheel,
    ]

    private let state: AppState
    private let panel: PresenterEffectsPanel
    private let effectsView = PresenterEffectsView()
    private var stateObserverIdentifier: UUID?
    private var globalMouseMonitor: Any?
    private var localMouseMonitor: Any?
    private var globalKeyMonitor: Any?
    private var localKeyMonitor: Any?
    private var applicationObserver: NSObjectProtocol?
    private var screenObserver: NSObjectProtocol?
    private var accessibilityStatusHandler: ((Bool) -> Void)?
    private(set) var isAccessibilityTrusted: Bool

    init(state: AppState) {
        self.state = state
        isAccessibilityTrusted = AXIsProcessTrusted()
        panel = PresenterEffectsPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        configurePanel()

        effectsView.transientContentDidChange = { [weak self] in
            self?.updatePanelVisibility()
        }
        stateObserverIdentifier = state.addPresenterEffectsObserver { [weak self] in
            self?.synchronizeState()
        }
        applicationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.refreshAccessibilityTrust()
        }
        screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.updatePanelFrame()
        }
    }

    func setAccessibilityStatusHandler(_ handler: @escaping (Bool) -> Void) {
        accessibilityStatusHandler = handler
        handler(isAccessibilityTrusted)
    }

    func toggleKeystrokeDisplay() {
        if state.isKeystrokeDisplayEnabled {
            state.setKeystrokeDisplayEnabled(false)
            return
        }

        let trusted = accessibilityTrust(requestPrompt: true)
        updateAccessibilityStatus(trusted)
        guard trusted else {
            state.setKeystrokeDisplayEnabled(false)
            NSLog(
                "EduTools: keystroke display remains disabled because Accessibility permission is unavailable"
            )
            return
        }
        state.setKeystrokeDisplayEnabled(true)
    }

    @discardableResult
    func adjustSpotlightRadius(by steps: Int) -> Bool {
        guard state.isSpotlightEnabled, steps != 0 else { return false }
        let proposed = effectsView.spotlightRadius + CGFloat(steps) * 16
        effectsView.spotlightRadius = min(max(proposed, 60), 500)
        return true
    }

    private func configurePanel() {
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 1)
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = .readOnly
        panel.ignoresMouseEvents = true
        effectsView.autoresizingMask = [.width, .height]
        panel.contentView = effectsView
    }

    private func synchronizeState() {
        effectsView.spotlightEnabled = state.isSpotlightEnabled
        effectsView.cursorHighlightEnabled = state.isCursorHighlightEnabled

        if state.isSpotlightEnabled || state.isCursorHighlightEnabled {
            installMouseMonitorsIfNeeded()
        } else {
            removeMouseMonitors()
        }

        if state.isKeystrokeDisplayEnabled {
            guard isAccessibilityTrusted else {
                NSLog(
                    "EduTools: keystroke display was disabled after Accessibility trust was lost"
                )
                state.setKeystrokeDisplayEnabled(false)
                return
            }
            guard installKeyMonitorsIfNeeded() else {
                NSLog("EduTools: keystroke display was disabled because its event monitor failed")
                state.setKeystrokeDisplayEnabled(false)
                return
            }
        } else {
            removeKeyMonitors()
            effectsView.clearKeystroke()
        }

        updatePointerLocation()
        updatePanelVisibility()
    }

    private func installMouseMonitorsIfNeeded() {
        if globalMouseMonitor == nil {
            globalMouseMonitor = NSEvent.addGlobalMonitorForEvents(
                matching: Self.mouseEventMask
            ) { [weak self] event in
                self?.handleMouseEvent(event)
            }
            if globalMouseMonitor == nil {
                NSLog("EduTools: failed to install the global presenter mouse monitor")
            }
        }
        if localMouseMonitor == nil {
            localMouseMonitor = NSEvent.addLocalMonitorForEvents(
                matching: Self.mouseEventMask
            ) { [weak self] event in
                self?.handleMouseEvent(event)
                return event
            }
            if localMouseMonitor == nil {
                NSLog("EduTools: failed to install the local presenter mouse monitor")
            }
        }
    }

    @discardableResult
    private func installKeyMonitorsIfNeeded() -> Bool {
        if globalKeyMonitor == nil {
            globalKeyMonitor = NSEvent.addGlobalMonitorForEvents(matching: .keyDown) {
                [weak self] event in
                self?.handleKeyEvent(event)
            }
            if globalKeyMonitor == nil {
                NSLog("EduTools: failed to install the global keystroke display monitor")
            }
        }
        if localKeyMonitor == nil {
            localKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) {
                [weak self] event in
                self?.handleKeyEvent(event)
                return event
            }
            if localKeyMonitor == nil {
                NSLog("EduTools: failed to install the local keystroke display monitor")
            }
        }
        return globalKeyMonitor != nil && localKeyMonitor != nil
    }

    private func handleMouseEvent(_ event: NSEvent) {
        updatePointerLocation()
        switch event.type {
        case .leftMouseDown where state.isCursorHighlightEnabled:
            effectsView.addRipple(at: effectsView.pointerLocation, color: .systemBlue)
        case .rightMouseDown where state.isCursorHighlightEnabled:
            effectsView.addRipple(at: effectsView.pointerLocation, color: .systemRed)
        case .scrollWheel where state.isSpotlightEnabled:
            let delta = event.scrollingDeltaY
            if delta != 0 {
                _ = adjustSpotlightRadius(by: delta > 0 ? 1 : -1)
            }
        default:
            break
        }
        updatePanelVisibility()
    }

    private func handleKeyEvent(_ event: NSEvent) {
        guard state.isKeystrokeDisplayEnabled,
              let displayString = KeystrokeDisplayFilter.displayString(for: event)
        else {
            return
        }
        updatePointerLocation()
        effectsView.showKeystroke(displayString)
        updatePanelVisibility()
    }

    private func updatePointerLocation() {
        guard updatePanelFrame() else { return }
        let mouseLocation = NSEvent.mouseLocation
        effectsView.pointerLocation = CGPoint(
            x: mouseLocation.x - panel.frame.minX,
            y: mouseLocation.y - panel.frame.minY
        )
    }

    @discardableResult
    private func updatePanelFrame() -> Bool {
        guard let screen = screenUnderMouse() else {
            return false
        }
        if panel.frame != screen.frame {
            panel.setFrame(screen.frame, display: true)
        }
        return true
    }

    private func updatePanelVisibility() {
        let shouldShow = state.isSpotlightEnabled
            || state.isCursorHighlightEnabled
            || effectsView.hasTransientContent
        if shouldShow, updatePanelFrame() {
            panel.orderFrontRegardless()
        } else {
            panel.orderOut(nil)
        }
    }

    private func refreshAccessibilityTrust() {
        let trusted = AXIsProcessTrusted()
        updateAccessibilityStatus(trusted)
        if !trusted, state.isKeystrokeDisplayEnabled {
            NSLog("EduTools: keystroke display was disabled because Accessibility permission was revoked")
            state.setKeystrokeDisplayEnabled(false)
        }
    }

    private func accessibilityTrust(requestPrompt: Bool) -> Bool {
        let options = [
            kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: requestPrompt
        ] as CFDictionary
        return AXIsProcessTrustedWithOptions(options)
    }

    private func updateAccessibilityStatus(_ trusted: Bool) {
        guard trusted != isAccessibilityTrusted else { return }
        isAccessibilityTrusted = trusted
        accessibilityStatusHandler?(trusted)
    }

    private func removeMouseMonitors() {
        if let globalMouseMonitor {
            NSEvent.removeMonitor(globalMouseMonitor)
            self.globalMouseMonitor = nil
        }
        if let localMouseMonitor {
            NSEvent.removeMonitor(localMouseMonitor)
            self.localMouseMonitor = nil
        }
    }

    private func removeKeyMonitors() {
        if let globalKeyMonitor {
            NSEvent.removeMonitor(globalKeyMonitor)
            self.globalKeyMonitor = nil
        }
        if let localKeyMonitor {
            NSEvent.removeMonitor(localKeyMonitor)
            self.localKeyMonitor = nil
        }
    }

    deinit {
        removeMouseMonitors()
        removeKeyMonitors()
        if let stateObserverIdentifier {
            state.removePresenterEffectsObserver(stateObserverIdentifier)
        }
        if let applicationObserver {
            NotificationCenter.default.removeObserver(applicationObserver)
        }
        if let screenObserver {
            NotificationCenter.default.removeObserver(screenObserver)
        }
        panel.close()
    }
}
