import AppKit
import Carbon

final class StatusMenu: NSObject {
    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let statusItem: NSStatusItem
    private let modeItem: NSMenuItem
    private let toolItem: NSMenuItem
    private let lessonTimerItem: NSMenuItem
    private var observerIdentifier: UUID?
    private var toolObserverIdentifier: UUID?
    private var hotkeyRegistrationFailed = false

    init(state: AppState, dispatcher: AppCommandDispatcher) {
        self.state = state
        self.dispatcher = dispatcher
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        modeItem = NSMenuItem(title: "판서 모드", action: nil, keyEquivalent: "")
        toolItem = NSMenuItem(title: "도구: 펜", action: nil, keyEquivalent: "")
        lessonTimerItem = NSMenuItem(title: "수업 타이머 시작", action: nil, keyEquivalent: "")
        super.init()

        modeItem.target = self
        modeItem.action = #selector(toggleAnnotationMode(_:))
        applyShortcut(.toggleAnnotationMode, scope: .global, to: modeItem)

        let menu = NSMenu()
        menu.addItem(modeItem)
        menu.addItem(toolItem)
        menu.addItem(.separator())

        lessonTimerItem.target = self
        lessonTimerItem.action = #selector(toggleLessonTimer(_:))
        applyShortcut(.toggleLessonTimer, scope: .global, to: lessonTimerItem)
        menu.addItem(lessonTimerItem)

        let breakTimerItem = NSMenuItem(
            title: "쉬는 시간 타이머…",
            action: #selector(openBreakTimer(_:)),
            keyEquivalent: ""
        )
        breakTimerItem.target = self
        applyShortcut(.showBreakTimerSetup, scope: .global, to: breakTimerItem)
        menu.addItem(breakTimerItem)
        menu.addItem(.separator())

        let clearAllItem = NSMenuItem(
            title: "전체 지우기",
            action: #selector(clearAllInk(_:)),
            keyEquivalent: ""
        )
        applyShortcut(.clearAll, scope: .annotation, to: clearAllItem)
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

        let helpItem = NSMenuItem(
            title: "도움말",
            action: #selector(showHelp(_:)),
            keyEquivalent: ""
        )
        applyShortcut(.showHelp, scope: .global, to: helpItem)
        helpItem.target = self
        menu.addItem(helpItem)
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

    func setLessonTimerDisplay(_ title: String) {
        lessonTimerItem.title = title
    }

    private func applyShortcut(
        _ command: AppCommand,
        scope: ShortcutScope,
        to item: NSMenuItem
    ) {
        guard let binding = ShortcutCatalog.definition(for: command)?.binding(in: scope) else {
            NSLog("EduTools: menu item has no shortcut definition for %@", String(describing: command))
            return
        }
        item.keyEquivalent = binding.keyEquivalent
        item.keyEquivalentModifierMask = binding.modifiers
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

    @objc private func toggleLessonTimer(_ sender: Any?) {
        dispatcher.execute(.toggleLessonTimer, scope: .global)
    }

    @objc private func openBreakTimer(_ sender: Any?) {
        dispatcher.execute(.showBreakTimerSetup, scope: .global)
    }

    @objc private func clearCurrentDisplayInk(_ sender: Any?) {
        state.clearCurrentDisplayInk()
    }

    @objc private func showHelp(_ sender: Any?) {
        dispatcher.execute(.showHelp, scope: .global)
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

