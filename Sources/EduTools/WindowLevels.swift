import AppKit

enum AppWindowLevel {
    static let overlay = NSWindow.Level.screenSaver
    static let effect = level(aboveOverlayBy: 1)
    static let tool = level(aboveOverlayBy: 2)
    static let guidance = level(aboveOverlayBy: 3)
    static let dialog = level(aboveOverlayBy: 4)

    private static func level(aboveOverlayBy offset: Int) -> NSWindow.Level {
        NSWindow.Level(rawValue: overlay.rawValue + offset)
    }
}

extension NSWindow {
    func configureAsAppDialog() {
        level = AppWindowLevel.dialog
        collectionBehavior.formUnion([.moveToActiveSpace, .fullScreenAuxiliary])
    }

    /// `runModal()` resets the window to the modal-panel level (8) as it starts, which would put the
    /// dialog under the overlay. A level set after the session has begun sticks, so queue it behind
    /// the start; the main queue keeps running inside the modal loop.
    func raiseAfterModalStarts() {
        DispatchQueue.main.async { [weak self] in
            self?.level = AppWindowLevel.dialog
        }
    }
}

extension NSAlert {
    @discardableResult
    func runAboveOverlay() -> NSApplication.ModalResponse {
        window.configureAsAppDialog()
        window.raiseAfterModalStarts()
        return runModal()
    }
}

extension NSSavePanel {
    @discardableResult
    func runAboveOverlay() -> NSApplication.ModalResponse {
        configureAsAppDialog()
        raiseAfterModalStarts()
        return runModal()
    }
}
