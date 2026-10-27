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
    private var observerIdentifier: UUID?
    private var refreshWorkItem: DispatchWorkItem?
    private var callbackRegistered = false
    private var previouslyFrontmostApplication: NSRunningApplication?

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

        synchronizePanels()
        observerIdentifier = state.addAnnotationModeObserver { [weak self] isEnabled in
            self?.setAnnotationModeEnabled(isEnabled)
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

        if let presentedBoard = state.presentedBoard,
           currentScreens[presentedBoard.displayUUID] == nil,
           let fallbackDisplayUUID = preferredDisplayUUID(in: currentScreens)
        {
            state.relocatePresentedBoard(to: fallbackDisplayUUID)
        }

        let removedIdentifiers = panelsByDisplayUUID.keys.filter { currentScreens[$0] == nil }
        for identifier in removedIdentifiers {
            panelsByDisplayUUID.removeValue(forKey: identifier)?.close()
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
        }

        if state.isAnnotationModeEnabled {
            focusPanelUnderMouse()
        }
    }

    private func makePanel(for screen: NSScreen, displayUUID: String) -> OverlayPanel {
        let panel = OverlayPanel(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false,
            screen: screen
        )
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.isMovableByWindowBackground = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = .screenSaver
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = .readOnly
        panel.ignoresMouseEvents = !state.isAnnotationModeEnabled
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
            panel.ignoresMouseEvents = !isEnabled
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

    private func focusPanelUnderMouse() {
        guard let displayUUID = displayUUIDUnderMouse() else {
            return
        }
        focusPanel(displayUUID: displayUUID)
    }

    func displayUUIDUnderMouse() -> String? {
        let mouseLocation = NSEvent.mouseLocation
        return panelsByDisplayUUID.first(where: { $0.value.frame.contains(mouseLocation) })?.key
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

    deinit {
        refreshWorkItem?.cancel()
        if let observerIdentifier {
            state.removeAnnotationModeObserver(observerIdentifier)
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

