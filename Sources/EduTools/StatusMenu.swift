import AppKit
import Carbon

final class StatusMenu: NSObject {
    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let statusItem: NSStatusItem
    private let modeItem: NSMenuItem
    private let toolItem: NSMenuItem
    private let zoomItem: NSMenuItem
    private let spotlightItem: NSMenuItem
    private let cursorHighlightItem: NSMenuItem
    private let keystrokeDisplayItem: NSMenuItem
    private let helpSharingItem: NSMenuItem
    private let lessonTimerItem: NSMenuItem
    private var observerIdentifier: UUID?
    private var toolObserverIdentifier: UUID?
    private var zoomObserverIdentifier: UUID?
    private var presenterEffectsObserverIdentifier: UUID?
    private var sharingSettingsObserverIdentifier: UUID?
    private var hotkeyRegistrationFailed = false
    private var accessibilityTrusted = false

    init(state: AppState, dispatcher: AppCommandDispatcher) {
        self.state = state
        self.dispatcher = dispatcher
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        modeItem = NSMenuItem(title: "판서 모드", action: nil, keyEquivalent: "")
        toolItem = NSMenuItem(title: "도구: 펜", action: nil, keyEquivalent: "")
        zoomItem = NSMenuItem(title: "화면 줌", action: nil, keyEquivalent: "")
        spotlightItem = NSMenuItem(title: "스포트라이트", action: nil, keyEquivalent: "")
        cursorHighlightItem = NSMenuItem(title: "커서 강조", action: nil, keyEquivalent: "")
        keystrokeDisplayItem = NSMenuItem(title: "키 입력 표시", action: nil, keyEquivalent: "")
        helpSharingItem = NSMenuItem(
            title: "도움말을 공유에 표시",
            action: nil,
            keyEquivalent: ""
        )
        lessonTimerItem = NSMenuItem(title: "수업 타이머 시작", action: nil, keyEquivalent: "")
        super.init()

        modeItem.target = self
        modeItem.action = #selector(toggleAnnotationMode(_:))
        applyShortcut(.toggleAnnotationMode, scope: .global, to: modeItem)

        let menu = NSMenu()
        menu.addItem(modeItem)
        menu.addItem(toolItem)
        zoomItem.target = self
        zoomItem.action = #selector(toggleZoom(_:))
        applyShortcut(.toggleZoom, scope: .global, to: zoomItem)
        menu.addItem(zoomItem)

        spotlightItem.target = self
        spotlightItem.action = #selector(toggleSpotlight(_:))
        applyShortcut(.toggleSpotlight, scope: .global, to: spotlightItem)
        menu.addItem(spotlightItem)

        cursorHighlightItem.target = self
        cursorHighlightItem.action = #selector(toggleCursorHighlight(_:))
        menu.addItem(cursorHighlightItem)

        keystrokeDisplayItem.target = self
        keystrokeDisplayItem.action = #selector(toggleKeystrokeDisplay(_:))
        menu.addItem(keystrokeDisplayItem)
        menu.addItem(.separator())

        let coverItem = NSMenuItem(
            title: "화면 가리개 도구",
            action: #selector(selectCoverTool(_:)),
            keyEquivalent: ""
        )
        coverItem.target = self
        menu.addItem(coverItem)
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

        let savePNGItem = NSMenuItem(
            title: "PNG로 저장",
            action: #selector(savePNG(_:)),
            keyEquivalent: ""
        )
        savePNGItem.target = self
        menu.addItem(savePNGItem)

        let saveBoardPDFItem = NSMenuItem(
            title: "보드를 PDF로 저장",
            action: #selector(saveBoardPDF(_:)),
            keyEquivalent: ""
        )
        saveBoardPDFItem.target = self
        menu.addItem(saveBoardPDFItem)

        let copyImageItem = NSMenuItem(
            title: "클립보드 복사",
            action: #selector(copyCurrentImage(_:)),
            keyEquivalent: ""
        )
        copyImageItem.target = self
        applyShortcut(.copyCurrentImage, scope: .annotation, to: copyImageItem)
        menu.addItem(copyImageItem)
        menu.addItem(.separator())

        let archiveSettingsItem = NSMenuItem(
            title: "보관·복원 설정",
            action: nil,
            keyEquivalent: ""
        )
        let archiveSettingsMenu = NSMenu()
        let autoArchiveItem = NSMenuItem(
            title: "지우기 전 자동 보관",
            action: #selector(toggleAutoArchive(_:)),
            keyEquivalent: ""
        )
        autoArchiveItem.target = self
        autoArchiveItem.state = state.autoArchiveEnabled ? .on : .off
        archiveSettingsMenu.addItem(autoArchiveItem)

        let clearOnExitItem = NSMenuItem(
            title: "판서 모드를 끌 때 내용 삭제",
            action: #selector(toggleClearOnAnnotationExit(_:)),
            keyEquivalent: ""
        )
        clearOnExitItem.target = self
        clearOnExitItem.state = state.clearsInkWhenAnnotationModeEnds ? .on : .off
        archiveSettingsMenu.addItem(clearOnExitItem)

        let autoRestoreItem = NSMenuItem(
            title: "다시 연결하면 자동 복원",
            action: #selector(toggleAutomaticRestore(_:)),
            keyEquivalent: ""
        )
        autoRestoreItem.target = self
        autoRestoreItem.state = state.automaticallyRestoresDisconnectedDisplays ? .on : .off
        archiveSettingsMenu.addItem(autoRestoreItem)
        archiveSettingsMenu.addItem(.separator())

        let disconnectPolicyItem = NSMenuItem(
            title: "화면 연결 해제 시",
            action: nil,
            keyEquivalent: ""
        )
        let disconnectPolicyMenu = NSMenu()
        for policy in DisplayDisconnectPolicy.allCases {
            let item = NSMenuItem(
                title: policy.displayName,
                action: #selector(selectDisconnectPolicy(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = policy.rawValue
            item.state = state.displayDisconnectPolicy == policy ? .on : .off
            disconnectPolicyMenu.addItem(item)
        }
        disconnectPolicyItem.submenu = disconnectPolicyMenu
        archiveSettingsMenu.addItem(disconnectPolicyItem)
        archiveSettingsItem.submenu = archiveSettingsMenu
        menu.addItem(archiveSettingsItem)
        menu.addItem(.separator())

        let helpItem = NSMenuItem(
            title: "도움말",
            action: #selector(showHelp(_:)),
            keyEquivalent: ""
        )
        applyShortcut(.showHelp, scope: .global, to: helpItem)
        helpItem.target = self
        menu.addItem(helpItem)

        helpSharingItem.target = self
        helpSharingItem.action = #selector(toggleHelpSharing(_:))
        menu.addItem(helpSharingItem)
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
        zoomObserverIdentifier = state.addZoomObserver { [weak self] session in
            self?.zoomItem.state = session == nil ? .off : .on
            self?.zoomItem.title = session == nil ? "화면 줌" : "화면 줌 종료"
        }
        presenterEffectsObserverIdentifier = state.addPresenterEffectsObserver { [weak self] in
            self?.updatePresenterEffects()
        }
        sharingSettingsObserverIdentifier = state.addSharingSettingsObserver { [weak self] isEnabled in
            self?.helpSharingItem.state = isEnabled ? .on : .off
        }
    }

    func setHotkeyRegistrationStatus(_ status: OSStatus) {
        hotkeyRegistrationFailed = hotkeyRegistrationFailed || status != noErr
        updateAnnotationMode(state.isAnnotationModeEnabled)
    }

    func setLessonTimerDisplay(_ title: String) {
        lessonTimerItem.title = title
    }

    func setAccessibilityTrusted(_ trusted: Bool) {
        accessibilityTrusted = trusted
        updatePresenterEffects()
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

    private func updatePresenterEffects() {
        spotlightItem.state = state.isSpotlightEnabled ? .on : .off
        cursorHighlightItem.state = state.isCursorHighlightEnabled ? .on : .off
        keystrokeDisplayItem.state = state.isKeystrokeDisplayEnabled ? .on : .off
        keystrokeDisplayItem.title = accessibilityTrusted ? "키 입력 표시" : "⚠ 키 입력 표시"
    }

    @objc private func toggleAnnotationMode(_ sender: Any?) {
        dispatcher.execute(.toggleAnnotationMode, scope: .global)
    }

    @objc private func toggleZoom(_ sender: Any?) {
        dispatcher.execute(.toggleZoom, scope: .global)
    }

    @objc private func toggleSpotlight(_ sender: Any?) {
        dispatcher.execute(.toggleSpotlight, scope: .global)
    }

    @objc private func toggleCursorHighlight(_ sender: Any?) {
        dispatcher.execute(.toggleCursorHighlight, scope: .global)
    }

    @objc private func toggleKeystrokeDisplay(_ sender: Any?) {
        dispatcher.execute(.toggleKeystrokeDisplay, scope: .global)
    }

    @objc private func selectCoverTool(_ sender: Any?) {
        dispatcher.execute(.selectTool(.cover), scope: .annotation)
    }

    @objc private func toggleHelpSharing(_ sender: Any?) {
        dispatcher.execute(.toggleHelpSharing, scope: .global)
    }

    @objc private func clearAllInk(_ sender: Any?) {
        dispatcher.execute(.clearAll, scope: .annotation)
    }

    @objc private func toggleLessonTimer(_ sender: Any?) {
        dispatcher.execute(.toggleLessonTimer, scope: .global)
    }

    @objc private func openBreakTimer(_ sender: Any?) {
        dispatcher.execute(.showBreakTimerSetup, scope: .global)
    }

    @objc private func clearCurrentDisplayInk(_ sender: Any?) {
        dispatcher.overlayController?.finishAllActiveInteractions()
        state.clearCurrentDisplayInk()
    }

    @objc private func savePNG(_ sender: Any?) {
        dispatcher.exportController?.savePNG()
    }

    @objc private func saveBoardPDF(_ sender: Any?) {
        dispatcher.exportController?.saveBoardPDF()
    }

    @objc private func copyCurrentImage(_ sender: Any?) {
        dispatcher.execute(.copyCurrentImage, scope: .annotation)
    }

    @objc private func toggleAutoArchive(_ sender: NSMenuItem) {
        state.setAutoArchiveEnabled(!state.autoArchiveEnabled)
        sender.state = state.autoArchiveEnabled ? .on : .off
    }

    @objc private func toggleClearOnAnnotationExit(_ sender: NSMenuItem) {
        state.setClearsInkWhenAnnotationModeEnds(!state.clearsInkWhenAnnotationModeEnds)
        sender.state = state.clearsInkWhenAnnotationModeEnds ? .on : .off
    }

    @objc private func toggleAutomaticRestore(_ sender: NSMenuItem) {
        state.setAutomaticallyRestoresDisconnectedDisplays(
            !state.automaticallyRestoresDisconnectedDisplays
        )
        sender.state = state.automaticallyRestoresDisconnectedDisplays ? .on : .off
    }

    @objc private func selectDisconnectPolicy(_ sender: NSMenuItem) {
        guard let rawValue = sender.representedObject as? String,
              let policy = DisplayDisconnectPolicy(rawValue: rawValue)
        else {
            NSLog("EduTools: invalid display disconnect policy menu item")
            return
        }
        state.setDisplayDisconnectPolicy(policy)
        for item in sender.menu?.items ?? [] {
            item.state = item === sender ? .on : .off
        }
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
        if let zoomObserverIdentifier {
            state.removeZoomObserver(zoomObserverIdentifier)
        }
        if let presenterEffectsObserverIdentifier {
            state.removePresenterEffectsObserver(presenterEffectsObserverIdentifier)
        }
        if let sharingSettingsObserverIdentifier {
            state.removeSharingSettingsObserver(sharingSettingsObserverIdentifier)
        }
        NSStatusBar.system.removeStatusItem(statusItem)
    }
}

