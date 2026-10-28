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
}

extension NSAlert {
    @discardableResult
    func runAboveOverlay() -> NSApplication.ModalResponse {
        window.configureAsAppDialog()
        return runModal()
    }
}

extension NSSavePanel {
    @discardableResult
    func runAboveOverlay() -> NSApplication.ModalResponse {
        configureAsAppDialog()
        return runModal()
    }
}
