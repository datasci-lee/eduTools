import AppKit
import ColorSync
import CoreGraphics

private func displayReconfigurationCallback(
    _ display: CGDirectDisplayID,
    _ flags: CGDisplayChangeSummaryFlags,
    _ userInfo: UnsafeMutableRawPointer?
) {
    guard let userInfo else { return }
    let controller = Unmanaged<OverlayController>.fromOpaque(userInfo).takeUnretainedValue()
    controller.displayConfigurationDidChange()
}

private final class OverlayPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

final class OverlayController {
    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private var panelsByDisplayUUID: [String: OverlayPanel] = [:]
    private var annotationObserverIdentifier: UUID?
    private var passThroughObserverIdentifier: UUID?
    private var refreshWorkItem: DispatchWorkItem?
    private var callbackRegistered = false
    private var screenParametersObserver: NSObjectProtocol?
    private var previouslyFrontmostApplication: NSRunningApplication?
    weak var archiveController: ArchiveController?

    init(state: AppState, dispatcher: AppCommandDispatcher) {
        self.state = state
        self.dispatcher = dispatcher

        let status = CGDisplayRegisterReconfigurationCallback(
            displayReconfigurationCallback,
            Unmanaged.passUnretained(self).toOpaque()
        )
        callbackRegistered = status == .success
        if status != .success {
            NSLog("EduTools: failed to register display callback (CGError %d)", status.rawValue)
        }

        // The CG callback can fire before `NSScreen.screens` reflects the change (Sidecar passes
        // through intermediate sizes while connecting); AppKit's notification fires after it has.
        screenParametersObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.displayConfigurationDidChange()
        }

        synchronizePanels()
        annotationObserverIdentifier = state.addAnnotationModeObserver { [weak self] isEnabled in
            self?.setAnnotationModeEnabled(isEnabled)
        }
        passThroughObserverIdentifier = state.addPassThroughModeObserver { [weak self] isEnabled in
            self?.setPassThroughModeEnabled(isEnabled)
        }
    }

    fileprivate func displayConfigurationDidChange() {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            refreshWorkItem?.cancel()

            let workItem = DispatchWorkItem { [weak self] in
                self?.synchronizePanels()
            }
            refreshWorkItem = workItem
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: workItem)
        }
    }

    private func synchronizePanels() {
        var currentScreens: [String: NSScreen] = [:]
        for screen in NSScreen.screens {
            guard let identifier = displayUUID(for: screen) else {
                NSLog("EduTools: screen has no display identifier; overlay was not created")
                continue
            }
            currentScreens[identifier] = screen
        }

        let removedIdentifiers = panelsByDisplayUUID.keys.filter { currentScreens[$0] == nil }
        for identifier in removedIdentifiers {
            (panelsByDisplayUUID[identifier]?.contentView as? InkView)?.finishActiveInteraction()
            archiveController?.displayWasRemoved(
                identifier,
                primaryDisplayUUID: primaryDisplayUUID(in: currentScreens)
            )
            panelsByDisplayUUID.removeValue(forKey: identifier)?.close()
        }

        if let presentedBoard = state.presentedBoard,
           currentScreens[presentedBoard.displayUUID] == nil,
           let fallbackDisplayUUID = preferredDisplayUUID(in: currentScreens)
        {
            state.relocatePresentedBoard(to: fallbackDisplayUUID)
        }

        for (identifier, screen) in currentScreens {
            if let panel = panelsByDisplayUUID[identifier] {
                if panel.frame != screen.frame {
                    panel.setFrame(screen.frame, display: true)
                }
                (panel.contentView as? InkView)?.refreshForDisplayChange()
            } else {
                panelsByDisplayUUID[identifier] = makePanel(
                    for: screen,
                    displayUUID: identifier
                )
            }
            archiveController?.restoreDisconnectedDisplayIfNeeded(identifier)
        }

        if state.isAnnotationModeEnabled, !state.isPassThroughModeEnabled {
            focusPanelUnderMouse()
        }
    }

    private func makePanel(for screen: NSScreen, displayUUID: String) -> OverlayPanel {
        let panel = OverlayPanel(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.isMovableByWindowBackground = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = AppWindowLevel.overlay
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = .readOnly
        panel.ignoresMouseEvents = !state.isAnnotationModeEnabled
            || state.isPassThroughModeEnabled
        panel.acceptsMouseMovedEvents = true

        let inkView = InkView(
            frame: NSRect(origin: .zero, size: screen.frame.size),
            state: state,
            dispatcher: dispatcher,
            displayUUID: displayUUID,
            requestKeyFocus: { [weak self] in
                self?.focusPanel(displayUUID: displayUUID)
            }
        )
        inkView.autoresizingMask = [.width, .height]
        panel.contentView = inkView
        panel.orderFrontRegardless()
        return panel
    }

    private func setAnnotationModeEnabled(_ isEnabled: Bool) {
        if isEnabled {
            let frontmostApplication = NSWorkspace.shared.frontmostApplication
            if frontmostApplication?.processIdentifier != ProcessInfo.processInfo.processIdentifier {
                previouslyFrontmostApplication = frontmostApplication
            }
            NSApp.activate()
        }

        for panel in panelsByDisplayUUID.values {
            panel.ignoresMouseEvents = !isEnabled || state.isPassThroughModeEnabled
            panel.orderFrontRegardless()
        }

        if isEnabled {
            focusPanelUnderMouse()
        } else if let previouslyFrontmostApplication {
            let activated = previouslyFrontmostApplication.activate(
                from: .current,
                options: []
            )
            if !activated {
                NSLog(
                    "EduTools: failed to return focus to process %d",
                    previouslyFrontmostApplication.processIdentifier
                )
            }
            self.previouslyFrontmostApplication = nil
        }
    }

    private func setPassThroughModeEnabled(_ isEnabled: Bool) {
        for panel in panelsByDisplayUUID.values {
            panel.ignoresMouseEvents = !state.isAnnotationModeEnabled || isEnabled
            panel.orderFrontRegardless()
        }
    }

    private func focusPanelUnderMouse() {
        guard let displayUUID = displayUUIDUnderMouse() else {
            return
        }
        focusPanel(displayUUID: displayUUID)
    }

    func displayUUIDUnderMouse() -> String? {
        guard let identifier = displayUUID(at: NSEvent.mouseLocation),
              panelsByDisplayUUID[identifier] != nil
        else {
            return nil
        }
        return identifier
    }

    var connectedDisplayUUIDs: [String] {
        Array(panelsByDisplayUUID.keys)
    }

    func screen(for identifier: String) -> NSScreen? {
        attachedScreen(forDisplayUUID: identifier)
    }

    func finishActiveInteraction(on displayUUID: String) {
        (panelsByDisplayUUID[displayUUID]?.contentView as? InkView)?.finishActiveInteraction()
    }

    func finishAllActiveInteractions() {
        for panel in panelsByDisplayUUID.values {
            (panel.contentView as? InkView)?.finishActiveInteraction()
        }
    }

    func renderedInkImage(
        for displayUUID: String,
        strokes: [InkStroke]? = nil,
        boardKind: BoardKind? = nil,
        pixelSize: CGSize? = nil
    ) -> CGImage? {
        guard let panel = panelsByDisplayUUID[displayUUID],
              let inkView = panel.contentView as? InkView
        else {
            return nil
        }
        let scale = panel.screen?.backingScaleFactor ?? 1
        let targetPixelSize = pixelSize ?? CGSize(
            width: panel.frame.width * scale,
            height: panel.frame.height * scale
        )
        return inkView.renderedImage(
            strokes: strokes,
            boardKind: boardKind,
            pixelSize: targetPixelSize
        )
    }

    func refreshAfterWake() {
        displayConfigurationDidChange()
    }

    func focusPanelForBoard(on displayUUID: String) {
        focusPanel(displayUUID: displayUUID)
    }

    @discardableResult
    func forwardToUnderlyingApplication(_ event: NSEvent) -> Bool {
        guard let previouslyFrontmostApplication,
              !previouslyFrontmostApplication.isTerminated
        else {
            NSLog("EduTools: could not forward a board navigation key without an underlying app")
            return false
        }
        guard let cgEvent = event.cgEvent else {
            NSLog("EduTools: could not create a Core Graphics event for key forwarding")
            return false
        }
        cgEvent.postToPid(previouslyFrontmostApplication.processIdentifier)
        return true
    }

    private func focusPanel(displayUUID: String) {
        guard state.isAnnotationModeEnabled,
              !state.isPassThroughModeEnabled,
              let panel = panelsByDisplayUUID[displayUUID]
        else {
            return
        }
        state.setActiveDisplay(displayUUID)
        panel.makeKeyAndOrderFront(nil)
        panel.makeFirstResponder(panel.contentView)
    }

    private func preferredDisplayUUID(in screens: [String: NSScreen]) -> String? {
        let mouseLocation = NSEvent.mouseLocation
        if let displayUnderMouse = screens.first(where: {
            $0.value.frame.contains(mouseLocation)
        })?.key {
            return displayUnderMouse
        }
        return NSScreen.screens.compactMap(displayUUID(for:)).first
    }

    private func primaryDisplayUUID(in screens: [String: NSScreen]) -> String? {
        guard let primaryScreen = NSScreen.screens.first,
              let identifier = displayUUID(for: primaryScreen),
              screens[identifier] != nil
        else {
            return nil
        }
        return identifier
    }

    deinit {
        refreshWorkItem?.cancel()
        if let screenParametersObserver {
            NotificationCenter.default.removeObserver(screenParametersObserver)
        }
        if let annotationObserverIdentifier {
            state.removeAnnotationModeObserver(annotationObserverIdentifier)
        }
        if let passThroughObserverIdentifier {
            state.removePassThroughModeObserver(passThroughObserverIdentifier)
        }
        if callbackRegistered {
            let status = CGDisplayRemoveReconfigurationCallback(
                displayReconfigurationCallback,
                Unmanaged.passUnretained(self).toOpaque()
            )
            if status != .success {
                NSLog("EduTools: failed to remove display callback (CGError %d)", status.rawValue)
            }
        }
        for panel in panelsByDisplayUUID.values {
            panel.close()
        }
    }
}

