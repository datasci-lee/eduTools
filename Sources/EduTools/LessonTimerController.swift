import AppKit
import Foundation

private final class LessonTimerPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

private final class LessonTimerDragView: NSVisualEffectView {
    var mouseDownHandler: ((NSEvent) -> Void)?
    var mouseDraggedHandler: ((NSEvent) -> Void)?
    var mouseUpHandler: ((NSEvent) -> Void)?

    override var mouseDownCanMoveWindow: Bool { false }

    override func hitTest(_ point: NSPoint) -> NSView? {
        bounds.contains(point) ? self : nil
    }

    override func mouseDown(with event: NSEvent) {
        mouseDownHandler?(event)
    }

    override func mouseDragged(with event: NSEvent) {
        mouseDraggedHandler?(event)
    }

    override func mouseUp(with event: NSEvent) {
        mouseUpHandler?(event)
    }
}

private final class LessonTimerDragLabel: NSTextField {
    override var mouseDownCanMoveWindow: Bool { false }
}

final class LessonTimerController: NSObject, NSMenuDelegate {
    private enum DefaultsKey {
        static let positions = "EduTools.lessonTimer.positions"
        static let displayUUID = "EduTools.lessonTimer.displayUUID"
    }

    enum SizePreset: String, CaseIterable {
        case small
        case medium
        case large

        var title: String {
            switch self {
            case .small: return "소"
            case .medium: return "중"
            case .large: return "대"
            }
        }

        var size: CGSize {
            switch self {
            case .small: return CGSize(width: 140, height: 64)
            case .medium: return CGSize(width: 180, height: 80)
            case .large: return CGSize(width: 240, height: 104)
            }
        }
    }

    enum DisplayMode: String, CaseIterable {
        case remaining
        case elapsed
    }

    private let engine = LessonTimerEngine()
    private let defaults: UserDefaults
    private let positionStore: ScreenPositionStore
    private let settingsStore: SettingsStore
    private let panel: LessonTimerPanel
    private let rootView = LessonTimerDragView()
    private let timeLabel = LessonTimerDragLabel(labelWithString: "50:00")
    private let modeLabel = LessonTimerDragLabel(labelWithString: "수업 타이머")
    private let pauseItem = NSMenuItem()
    private let displayModeItem = NSMenuItem()
    private let autoBreakItem = NSMenuItem()
    private let presetsMenu = NSMenu(title: "프리셋")
    private var refreshTimer: Timer?
    private var blinkTimer: Timer?
    private var notificationObservers: [NSObjectProtocol] = []
    private var currentDisplayUUID: String?
    private var isApplyingFrame = false
    private var blinkCount = 0
    private var displayMode: DisplayMode
    private var dragStartMouseLocation: CGPoint?
    private var dragStartPanelOrigin: CGPoint?
    private var settingsObserverIdentifier: UUID?

    var onDisplayTextChange: ((String) -> Void)?
    var onAutoBreakRequested: ((Int) -> Void)?

    init(defaults: UserDefaults = .standard, settingsStore: SettingsStore? = nil) {
        self.defaults = defaults
        positionStore = ScreenPositionStore(
            defaults: defaults,
            positionsKey: DefaultsKey.positions,
            displayUUIDKey: DefaultsKey.displayUUID
        )
        let resolvedSettingsStore = settingsStore ?? SettingsStore(defaults: defaults)
        self.settingsStore = resolvedSettingsStore
        displayMode = DisplayMode(
            rawValue: resolvedSettingsStore.settings.timer.lessonDisplayMode
        ) ?? .remaining
        let savedSize = SizePreset(rawValue: resolvedSettingsStore.settings.timer.lessonBadgeSize)
            ?? .medium
        panel = LessonTimerPanel(
            contentRect: NSRect(origin: .zero, size: savedSize.size),
            styleMask: [.borderless, .nonactivatingPanel, .resizable],
            backing: .buffered,
            defer: false
        )
        super.init()

        configurePanel()
        buildContextMenu()
        restorePosition()
        observeSystemEvents()
        startDisplayRefreshTimer()
        refresh(now: Date())
        settingsObserverIdentifier = resolvedSettingsStore.addObserver { [weak self] settings in
            guard let self else { return }
            self.panel.alphaValue = CGFloat(settings.timer.lessonBadgeOpacity)
            if let size = SizePreset(rawValue: settings.timer.lessonBadgeSize) {
                self.resize(to: size.size)
            }
            self.displayMode = DisplayMode(rawValue: settings.timer.lessonDisplayMode) ?? .remaining
            self.rebuildPresetMenu()
            self.refresh(now: Date())
        }
    }

    func toggleFromShortcut() {
        engine.toggleDefault(
            now: Date(),
            defaultDuration: TimeInterval(settingsStore.settings.timer.lessonDefaultMinutes * 60)
        )
        showPanel()
        refresh(now: Date())
    }

    func startCountdown(minutes: Int) {
        engine.startCountdown(duration: TimeInterval(minutes * 60), now: Date())
        showPanel()
        refresh(now: Date())
    }

    func startCountUp() {
        engine.startCountUp(now: Date())
        showPanel()
        refresh(now: Date())
    }

    func setDisplayTextHandler(_ handler: @escaping (String) -> Void) {
        onDisplayTextChange = handler
        handler(statusText(now: Date()))
    }

    private func configurePanel() {
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.isMovableByWindowBackground = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = AppWindowLevel.tool
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = .none
        panel.contentMinSize = SizePreset.small.size
        panel.contentMaxSize = CGSize(width: 420, height: 180)
        panel.alphaValue = savedOpacity()

        rootView.material = .hudWindow
        rootView.blendingMode = .withinWindow
        rootView.state = .active
        rootView.wantsLayer = true
        rootView.layer?.cornerRadius = 14
        rootView.layer?.masksToBounds = true
        rootView.mouseDownHandler = { [weak self] _ in
            self?.beginDraggingPanel()
        }
        rootView.mouseDraggedHandler = { [weak self] _ in
            self?.continueDraggingPanel()
        }
        rootView.mouseUpHandler = { [weak self] event in
            self?.endDraggingPanel(snappingDisabled: event.modifierFlags.contains(.option))
        }
        panel.contentView = rootView

        timeLabel.font = .monospacedDigitSystemFont(ofSize: 34, weight: .bold)
        timeLabel.textColor = .labelColor
        timeLabel.alignment = .center
        timeLabel.maximumNumberOfLines = 1
        timeLabel.translatesAutoresizingMaskIntoConstraints = false
        timeLabel.setContentCompressionResistancePriority(.required, for: .vertical)

        modeLabel.font = .systemFont(ofSize: 10, weight: .medium)
        modeLabel.textColor = .secondaryLabelColor
        modeLabel.alignment = .center
        modeLabel.translatesAutoresizingMaskIntoConstraints = false

        let stack = NSStackView(views: [timeLabel, modeLabel])
        stack.orientation = .vertical
        stack.alignment = .centerX
        stack.spacing = 0
        stack.translatesAutoresizingMaskIntoConstraints = false
        rootView.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: rootView.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: rootView.centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: rootView.leadingAnchor, constant: 8),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: rootView.trailingAnchor, constant: -8),
        ])
    }

    private func buildContextMenu() {
        let menu = NSMenu(title: "수업 타이머")
        menu.delegate = self

        for minutes in settingsStore.settings.timer.lessonPresets {
            let item = NSMenuItem(
                title: "\(minutes)분",
                action: #selector(startPreset(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = NSNumber(value: minutes)
            presetsMenu.addItem(item)
        }
        let presetItem = NSMenuItem(title: "프리셋", action: nil, keyEquivalent: "")
        presetItem.submenu = presetsMenu
        menu.addItem(presetItem)

        let countUpItem = NSMenuItem(
            title: "카운트업",
            action: #selector(startCountUpFromMenu(_:)),
            keyEquivalent: ""
        )
        countUpItem.target = self
        menu.addItem(countUpItem)
        menu.addItem(.separator())

        pauseItem.target = self
        pauseItem.action = #selector(togglePause(_:))
        menu.addItem(pauseItem)
        let resetItem = NSMenuItem(
            title: "리셋",
            action: #selector(resetTimer(_:)),
            keyEquivalent: ""
        )
        resetItem.target = self
        menu.addItem(resetItem)
        menu.addItem(.separator())

        let sizes = NSMenu(title: "크기")
        for preset in SizePreset.allCases {
            let item = NSMenuItem(
                title: preset.title,
                action: #selector(selectSize(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = preset.rawValue
            sizes.addItem(item)
        }
        let sizeItem = NSMenuItem(title: "크기", action: nil, keyEquivalent: "")
        sizeItem.submenu = sizes
        menu.addItem(sizeItem)

        let opacityMenu = NSMenu(title: "투명도")
        let opacityRange = SettingsRanges.lessonBadgeOpacity
        let maximumOpacityPercent = Int(opacityRange.upperBound * 100)
        let minimumOpacityPercent = Int(opacityRange.lowerBound * 100)
        for percent in stride(from: maximumOpacityPercent, through: minimumOpacityPercent, by: -20) {
            let item = NSMenuItem(
                title: "\(percent)%",
                action: #selector(selectOpacity(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = NSNumber(value: Double(percent) / 100)
            opacityMenu.addItem(item)
        }
        let opacityItem = NSMenuItem(title: "투명도", action: nil, keyEquivalent: "")
        opacityItem.submenu = opacityMenu
        menu.addItem(opacityItem)

        displayModeItem.target = self
        displayModeItem.action = #selector(toggleDisplayMode(_:))
        menu.addItem(displayModeItem)

        let resetPositionItem = NSMenuItem(
            title: "위치 초기화",
            action: #selector(resetPosition(_:)),
            keyEquivalent: ""
        )
        resetPositionItem.target = self
        menu.addItem(resetPositionItem)
        menu.addItem(.separator())

        autoBreakItem.target = self
        autoBreakItem.action = #selector(toggleAutoBreak(_:))
        menu.addItem(autoBreakItem)
        rootView.menu = menu
    }

    private func rebuildPresetMenu() {
        presetsMenu.removeAllItems()
        for minutes in settingsStore.settings.timer.lessonPresets {
            let item = NSMenuItem(
                title: "\(minutes)분",
                action: #selector(startPreset(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = NSNumber(value: minutes)
            presetsMenu.addItem(item)
        }
    }

    func menuWillOpen(_ menu: NSMenu) {
        pauseItem.title = engine.phase == .paused ? "재시작" : "일시정지"
        pauseItem.isEnabled = engine.isActive
        displayModeItem.title = displayMode == .remaining ? "경과 시간 표시" : "남은 시간 표시"
        autoBreakItem.title = "종료 후 \(autoBreakMinutes())분 쉬는 시간 자동 시작"
        autoBreakItem.state = settingsStore.settings.timer.autoStartBreak ? .on : .off
    }

    @objc private func startPreset(_ sender: NSMenuItem) {
        guard let minutes = (sender.representedObject as? NSNumber)?.intValue else {
            NSLog("EduTools: lesson timer preset has no minute value")
            return
        }
        startCountdown(minutes: minutes)
    }

    @objc private func startCountUpFromMenu(_ sender: Any?) {
        startCountUp()
    }

    @objc private func togglePause(_ sender: Any?) {
        let now = Date()
        if engine.phase == .paused {
            engine.resume(now: now)
        } else {
            engine.pause(now: now)
        }
        refresh(now: now)
    }

    @objc private func resetTimer(_ sender: Any?) {
        engine.reset(now: Date())
        showPanel()
        refresh(now: Date())
    }

    @objc private func selectSize(_ sender: NSMenuItem) {
        guard let rawValue = sender.representedObject as? String,
              let preset = SizePreset(rawValue: rawValue)
        else {
            NSLog("EduTools: lesson timer size menu has no preset")
            return
        }
        settingsStore.update { $0.timer.lessonBadgeSize = preset.rawValue }
    }

    @objc private func selectOpacity(_ sender: NSMenuItem) {
        guard let opacity = (sender.representedObject as? NSNumber)?.doubleValue else {
            NSLog("EduTools: lesson timer opacity menu has no value")
            return
        }
        let range = SettingsRanges.lessonBadgeOpacity
        settingsStore.update {
            $0.timer.lessonBadgeOpacity = min(max(opacity, range.lowerBound), range.upperBound)
        }
    }

    @objc private func toggleDisplayMode(_ sender: Any?) {
        displayMode = displayMode == .remaining ? .elapsed : .remaining
        settingsStore.update { $0.timer.lessonDisplayMode = displayMode.rawValue }
        refresh(now: Date())
    }

    @objc private func resetPosition(_ sender: Any?) {
        guard let screen = NSScreen.screens.first else {
            NSLog("EduTools: lesson timer could not reset position because no screen is available")
            return
        }
        placePanel(on: screen, identifier: displayUUID(for: screen), ratio: CGPoint(x: 0.95, y: 0.88))
        saveCurrentPosition()
    }

    @objc private func toggleAutoBreak(_ sender: Any?) {
        let enabled = !settingsStore.settings.timer.autoStartBreak
        settingsStore.update { $0.timer.autoStartBreak = enabled }
        autoBreakItem.state = enabled ? .on : .off
    }

    private func showPanel() {
        ensureCurrentScreenExists()
        panel.orderFrontRegardless()
    }

    private func startDisplayRefreshTimer() {
        let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            self?.refresh(now: Date())
        }
        RunLoop.main.add(timer, forMode: .common)
        refreshTimer = timer
    }

    private func refresh(now: Date) {
        let didFinish = engine.synchronize(now: now)
        let value: TimeInterval
        if displayMode == .remaining, case .countdown = engine.mode {
            value = engine.remaining(at: now)
            modeLabel.stringValue = engine.phase == .paused ? "남은 시간 · 일시정지" : "남은 시간"
        } else {
            value = engine.elapsed(at: now)
            modeLabel.stringValue = engine.phase == .paused ? "경과 시간 · 일시정지" : "경과 시간"
        }
        timeLabel.stringValue = Self.formattedDuration(value)

        if engine.phase == .finished {
            timeLabel.textColor = .systemRed
            modeLabel.stringValue = "수업 종료"
        } else if case .countdown = engine.mode, engine.remaining(at: now) <= 5 * 60 {
            timeLabel.textColor = .systemOrange
        } else {
            timeLabel.textColor = .labelColor
        }

        updateLabelFont()
        onDisplayTextChange?(statusText(now: now))
        if didFinish {
            handleCompletion()
        }
    }

    private func statusText(now: Date) -> String {
        switch engine.phase {
        case .stopped:
            return "수업 타이머 시작"
        case .finished:
            return "수업 타이머: 00:00"
        case .running, .paused:
            let value: TimeInterval
            if displayMode == .remaining, case .countdown = engine.mode {
                value = engine.remaining(at: now)
            } else {
                value = engine.elapsed(at: now)
            }
            let paused = engine.phase == .paused ? " · 일시정지" : ""
            return "수업 타이머: \(Self.formattedDuration(value))\(paused)"
        }
    }

    private func handleCompletion() {
        TimerSound.playCompletion(named: settingsStore.settings.timer.notificationSound)
        startBlinking()
        if settingsStore.settings.timer.autoStartBreak {
            onAutoBreakRequested?(autoBreakMinutes())
        }
    }

    private func startBlinking() {
        blinkTimer?.invalidate()
        blinkCount = 0
        let timer = Timer(timeInterval: 0.35, repeats: true) { [weak self] timer in
            guard let self else {
                timer.invalidate()
                return
            }
            blinkCount += 1
            rootView.layer?.backgroundColor = blinkCount.isMultiple(of: 2)
                ? NSColor.systemRed.withAlphaComponent(0.45).cgColor
                : NSColor.clear.cgColor
            if blinkCount >= 8 {
                timer.invalidate()
                rootView.layer?.backgroundColor = NSColor.clear.cgColor
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        blinkTimer = timer
    }

    private func updateLabelFont() {
        let size = min(64, max(24, panel.contentLayoutRect.height * 0.43))
        timeLabel.font = .monospacedDigitSystemFont(ofSize: size, weight: .bold)
    }

    private func savedOpacity() -> CGFloat {
        CGFloat(settingsStore.settings.timer.lessonBadgeOpacity)
    }

    private func autoBreakMinutes() -> Int {
        settingsStore.settings.timer.breakDefaultMinutes
    }

    private func observeSystemEvents() {
        notificationObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSWindow.didResizeNotification,
                object: panel,
                queue: .main
            ) { [weak self] _ in
                self?.updateLabelFont()
                self?.saveCurrentPosition()
            }
        )
        notificationObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                self?.ensureCurrentScreenExists()
            }
        )
        notificationObservers.append(
            NSWorkspace.shared.notificationCenter.addObserver(
                forName: NSWorkspace.didWakeNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                self?.refresh(now: Date())
            }
        )
    }

    private func resize(to size: CGSize) {
        let center = CGPoint(x: panel.frame.midX, y: panel.frame.midY)
        var frame = NSRect(
            x: center.x - size.width / 2,
            y: center.y - size.height / 2,
            width: size.width,
            height: size.height
        )
        if let screen = screenContainingPanel() {
            frame.origin = TimerGeometry.snappedOrigin(
                proposed: frame.origin,
                windowSize: frame.size,
                visibleFrame: screen.visibleFrame,
                snappingDisabled: true
            )
        }
        isApplyingFrame = true
        panel.setFrame(frame, display: true)
        isApplyingFrame = false
        updateLabelFont()
        saveCurrentPosition()
    }

    private func beginDraggingPanel() {
        dragStartMouseLocation = NSEvent.mouseLocation
        dragStartPanelOrigin = panel.frame.origin
    }

    private func continueDraggingPanel() {
        guard let dragStartMouseLocation, let dragStartPanelOrigin else { return }
        let location = NSEvent.mouseLocation
        isApplyingFrame = true
        panel.setFrameOrigin(
            CGPoint(
                x: dragStartPanelOrigin.x + location.x - dragStartMouseLocation.x,
                y: dragStartPanelOrigin.y + location.y - dragStartMouseLocation.y
            )
        )
        isApplyingFrame = false
    }

    private func endDraggingPanel(snappingDisabled: Bool) {
        dragStartMouseLocation = nil
        dragStartPanelOrigin = nil
        guard let screen = screenContainingPanel() else {
            NSLog("EduTools: lesson timer could not find a screen after moving")
            return
        }
        let snapped = TimerGeometry.snappedOrigin(
            proposed: panel.frame.origin,
            windowSize: panel.frame.size,
            visibleFrame: screen.visibleFrame,
            snappingDisabled: snappingDisabled
        )
        isApplyingFrame = true
        panel.setFrameOrigin(snapped)
        isApplyingFrame = false
        saveCurrentPosition()
    }

    private func restorePosition() {
        let screensByIdentifier = screensByDisplayUUID()
        let savedIdentifier = defaults.string(forKey: DefaultsKey.displayUUID)
        let ratio = positionStore.ratio(for: savedIdentifier)
        if let savedIdentifier, let screen = screensByIdentifier[savedIdentifier] {
            placePanel(on: screen, identifier: savedIdentifier, ratio: ratio)
        } else if let primaryScreen = NSScreen.screens.first {
            placePanel(
                on: primaryScreen,
                identifier: displayUUID(for: primaryScreen),
                ratio: ratio
            )
        } else {
            NSLog("EduTools: lesson timer could not restore position because no screen is available")
        }
    }

    private func placePanel(on screen: NSScreen, identifier: String?, ratio: CGPoint?) {
        let visibleFrame = screen.visibleFrame
        let origin = restoredOrigin(
            ratio: ratio,
            default: CGPoint(x: 0.95, y: 0.88),
            visibleFrame: visibleFrame,
            size: panel.frame.size
        )
        isApplyingFrame = true
        panel.setFrameOrigin(origin)
        isApplyingFrame = false
        currentDisplayUUID = identifier
        if let identifier {
            defaults.set(identifier, forKey: DefaultsKey.displayUUID)
        }
    }

    private func saveCurrentPosition() {
        guard !isApplyingFrame,
              let screen = screenContainingPanel(),
              let identifier = displayUUID(for: screen)
        else {
            return
        }
        let ratio = savedRatio(
            origin: panel.frame.origin,
            visibleFrame: screen.visibleFrame,
            size: panel.frame.size
        )
        positionStore.save(ratio: ratio, displayUUID: identifier)
        currentDisplayUUID = identifier
    }

    private func screenContainingPanel() -> NSScreen? {
        if let screen = panel.screen {
            return screen
        }
        let center = CGPoint(x: panel.frame.midX, y: panel.frame.midY)
        return NSScreen.screens.first { $0.frame.contains(center) }
    }

    private func ensureCurrentScreenExists() {
        if let currentDisplayUUID,
           let screen = NSScreen.screens.first(where: { displayUUID(for: $0) == currentDisplayUUID }) {
            guard !screen.frame.intersects(panel.frame) else { return }
            placePanel(
                on: screen,
                identifier: currentDisplayUUID,
                ratio: positionStore.ratio(for: currentDisplayUUID)
            )
            return
        }

        guard let primaryScreen = NSScreen.screens.first else {
            NSLog("EduTools: lesson timer could not move after a display change because no screen is available")
            return
        }
        let priorRatio = positionStore.ratio(for: currentDisplayUUID)
        placePanel(
            on: primaryScreen,
            identifier: displayUUID(for: primaryScreen),
            ratio: priorRatio
        )
        saveCurrentPosition()
    }

    static func formattedDuration(_ interval: TimeInterval) -> String {
        let totalSeconds = max(0, Int(ceil(interval)))
        return String(format: "%02d:%02d", totalSeconds / 60, totalSeconds % 60)
    }

    deinit {
        if let settingsObserverIdentifier {
            settingsStore.removeObserver(settingsObserverIdentifier)
        }
        refreshTimer?.invalidate()
        blinkTimer?.invalidate()
        for observer in notificationObservers {
            NotificationCenter.default.removeObserver(observer)
            NSWorkspace.shared.notificationCenter.removeObserver(observer)
        }
        panel.close()
    }
}

enum TimerSound {
    static func playCompletion(named name: String) {
        if let sound = NSSound(named: NSSound.Name(name)) {
            sound.play()
        } else {
            NSLog("EduTools: %@ system sound was unavailable; using the system beep", name)
            NSSound.beep()
        }
    }
}
