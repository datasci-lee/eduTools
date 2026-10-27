import AppKit
import Carbon

final class StatusMenu: NSObject {
    private let state: AppState
    private let statusItem: NSStatusItem
    private let modeItem: NSMenuItem
    private var observerIdentifier: UUID?
    private var hotkeyRegistrationFailed = false

    init(state: AppState) {
        self.state = state
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        modeItem = NSMenuItem(title: "판서 모드", action: nil, keyEquivalent: "p")
        super.init()

        modeItem.target = self
        modeItem.action = #selector(toggleAnnotationMode(_:))
        modeItem.keyEquivalentModifierMask = [.control, .option]

        let menu = NSMenu()
        menu.addItem(modeItem)
        menu.addItem(.separator())

        let quitItem = NSMenuItem(
            title: "EduTools 종료",
            action: #selector(terminateApplication(_:)),
            keyEquivalent: "q"
        )
        quitItem.target = self
        menu.addItem(quitItem)
        statusItem.menu = menu

        observerIdentifier = state.addAnnotationModeObserver { [weak self] isEnabled in
            self?.updateAnnotationMode(isEnabled)
        }
    }

    func setHotkeyRegistrationStatus(_ status: OSStatus) {
        hotkeyRegistrationFailed = status != noErr
        updateAnnotationMode(state.isAnnotationModeEnabled)
    }

    private func updateAnnotationMode(_ isEnabled: Bool) {
        modeItem.state = isEnabled ? .on : .off
        modeItem.title = hotkeyRegistrationFailed ? "⚠ 판서 모드" : "판서 모드"

        let symbolName = isEnabled ? "pencil.circle.fill" : "pencil"
        let description = isEnabled ? "판서 모드 켜짐" : "판서 모드 꺼짐"
        let image = NSImage(systemSymbolName: symbolName, accessibilityDescription: description)
        image?.isTemplate = true
        statusItem.button?.image = image
        statusItem.button?.toolTip = description
    }

    @objc private func toggleAnnotationMode(_ sender: Any?) {
        state.toggleAnnotationMode()
    }

    @objc private func terminateApplication(_ sender: Any?) {
        NSApp.terminate(nil)
    }

    deinit {
        if let observerIdentifier {
            state.removeAnnotationModeObserver(observerIdentifier)
        }
        NSStatusBar.system.removeStatusItem(statusItem)
    }
}

