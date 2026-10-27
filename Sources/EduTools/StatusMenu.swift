import AppKit
import Carbon

final class StatusMenu: NSObject {
    private let state: AppState
    private let statusItem: NSStatusItem
    private let modeItem: NSMenuItem
    private let toolItem: NSMenuItem
    private var observerIdentifier: UUID?
    private var toolObserverIdentifier: UUID?
    private var hotkeyRegistrationFailed = false

    init(state: AppState) {
        self.state = state
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        modeItem = NSMenuItem(title: "판서 모드", action: nil, keyEquivalent: "p")
        toolItem = NSMenuItem(title: "도구: 펜", action: nil, keyEquivalent: "")
        super.init()

        modeItem.target = self
        modeItem.action = #selector(toggleAnnotationMode(_:))
        modeItem.keyEquivalentModifierMask = [.control, .option]

        let menu = NSMenu()
        menu.addItem(modeItem)
        menu.addItem(toolItem)
        menu.addItem(.separator())

        let clearAllItem = NSMenuItem(
            title: "전체 지우기",
            action: #selector(clearAllInk(_:)),
            keyEquivalent: "\u{8}"
        )
        clearAllItem.keyEquivalentModifierMask = .command
        clearAllItem.target = self
        menu.addItem(clearAllItem)

        let clearCurrentDisplayItem = NSMenuItem(
            title: "현재 모니터만 지우기",
            action: #selector(clearCurrentDisplayInk(_:)),
            keyEquivalent: ""
        )
        clearCurrentDisplayItem.target = self
        menu.addItem(clearCurrentDisplayItem)
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
        toolObserverIdentifier = state.addToolObserver { [weak self] tool in
            self?.toolItem.title = "도구: \(tool.displayName)"
        }
    }

    func setHotkeyRegistrationStatus(_ status: OSStatus) {
        hotkeyRegistrationFailed = hotkeyRegistrationFailed || status != noErr
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

    @objc private func clearAllInk(_ sender: Any?) {
        state.clearAllInk()
    }

    @objc private func clearCurrentDisplayInk(_ sender: Any?) {
        state.clearCurrentDisplayInk()
    }

    @objc private func terminateApplication(_ sender: Any?) {
        NSApp.terminate(nil)
    }

    deinit {
        if let observerIdentifier {
            state.removeAnnotationModeObserver(observerIdentifier)
        }
        if let toolObserverIdentifier {
            state.removeToolObserver(toolObserverIdentifier)
        }
        NSStatusBar.system.removeStatusItem(statusItem)
    }
}

