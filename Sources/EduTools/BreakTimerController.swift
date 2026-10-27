import AppKit
import Foundation

private final class BreakTimerPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private final class BreakTimerRootView: NSVisualEffectView {
    var allowsWindowDrag = false
    override var mouseDownCanMoveWindow: Bool { allowsWindowDrag }
}

private final class BreakProgressView: NSView {
    var progress: CGFloat = 1 {
        didSet { needsDisplay = true }
    }
    var accentColor = NSColor.controlAccentColor {
        didSet { needsDisplay = true }
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        let lineWidth: CGFloat = 14
        let drawingRect = bounds.insetBy(dx: lineWidth, dy: lineWidth)
        NSColor.separatorColor.withAlphaComponent(0.35).setStroke()
        let background = NSBezierPath(ovalIn: drawingRect)
        background.lineWidth = lineWidth
        background.stroke()

        let clampedProgress = min(max(progress, 0), 1)
        guard clampedProgress > 0 else { return }
        accentColor.setStroke()
        let foreground = NSBezierPath()
        foreground.appendArc(
            withCenter: CGPoint(x: bounds.midX, y: bounds.midY),
            radius: min(drawingRect.width, drawingRect.height) / 2,
            startAngle: 90,
            endAngle: 90 - 360 * clampedProgress,
            clockwise: true
        )
        foreground.lineWidth = lineWidth
        foreground.lineCapStyle = .round
        foreground.stroke()
    }
}

final class BreakTimerController: NSObject {
    private let engine = BreakTimerEngine()
    private let panel: BreakTimerPanel
    private let setupPanel: NSPanel
    private let rootView = BreakTimerRootView()
    private let remainingLabel = NSTextField(labelWithString: "10:00")
    private let resumeLabel = NSTextField(labelWithString: "수업 재개 시각")
    private let currentTimeLabel = NSTextField(labelWithString: "현재 시각")
    private let progressView = BreakProgressView()
    private let timePicker = NSDatePicker()
    private let warningLabel = NSTextField(labelWithString: "오늘 시각이 지났습니다")
    private let collapseButton = NSButton()
    private var refreshTimer: Timer?
    private var notificationObservers: [NSObjectProtocol] = []
    private var quickEndDate: Date?
    private var targetDisplayUUID: String?
    private var isCollapsed = false

    override init() {
        panel = BreakTimerPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        setupPanel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 430, height: 250),
            styleMask: [.titled, .closable, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        super.init()

        configureTimerPanel()
        configureSetupPanel()
        observeSystemEvents()
        startDisplayRefreshTimer()
    }

    func openSetup() {
        let now = Date()
        let proposed = engine.phase == .running
            ? engine.endDate ?? BreakTimerEngine.endDate(addingMinutes: 10, to: now)
            : BreakTimerEngine.endDate(addingMinutes: 10, to: now)
        quickEndDate = proposed
        timePicker.dateValue = proposed
        warningLabel.isHidden = true

        if let screen = screenUnderMouse() ?? NSScreen.screens.first {
            let frame = setupPanel.frame
            setupPanel.setFrameOrigin(
                CGPoint(
                    x: screen.visibleFrame.midX - frame.width / 2,
                    y: screen.visibleFrame.midY - frame.height / 2
                )
            )
        } else {
            NSLog("EduTools: break timer setup could not find a screen")
        }
        NSApp.activate()
        setupPanel.makeKeyAndOrderFront(nil)
    }

    func startQuick(minutes: Int) {
        let now = Date()
        let endDate = BreakTimerEngine.endDate(addingMinutes: minutes, to: now)
        do {
            try engine.start(until: endDate, now: now)
            showTimer(on: screenUnderMouse() ?? NSScreen.screens.first)
            refresh(now: now)
        } catch {
            NSLog("EduTools: automatic break timer rejected its future end date: %@", String(describing: error))
        }
    }

    private func configureTimerPanel() {
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.isMovableByWindowBackground = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 3)
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]

        rootView.material = .underWindowBackground
        rootView.blendingMode = .behindWindow
        rootView.state = .active
        rootView.wantsLayer = true
        rootView.layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.96).cgColor
        panel.contentView = rootView

        remainingLabel.font = .monospacedDigitSystemFont(ofSize: 128, weight: .bold)
        remainingLabel.alignment = .center
        remainingLabel.textColor = .labelColor
        remainingLabel.setContentCompressionResistancePriority(.required, for: .vertical)

        resumeLabel.font = .systemFont(ofSize: 32, weight: .semibold)
        resumeLabel.alignment = .center
        resumeLabel.textColor = .secondaryLabelColor

        currentTimeLabel.font = .monospacedDigitSystemFont(ofSize: 24, weight: .medium)
        currentTimeLabel.alignment = .center
        currentTimeLabel.textColor = .secondaryLabelColor

        progressView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            progressView.widthAnchor.constraint(equalToConstant: 260),
            progressView.heightAnchor.constraint(equalToConstant: 260),
        ])

        let changeButton = NSButton(
            title: "종료 시각 변경",
            target: self,
            action: #selector(changeEndDate(_:))
        )
        changeButton.bezelStyle = .rounded
        collapseButton.title = "작은 창으로 접기"
        collapseButton.target = self
        collapseButton.action = #selector(toggleCollapsed(_:))
        collapseButton.bezelStyle = .rounded
        let endButton = NSButton(
            title: "조기 종료",
            target: self,
            action: #selector(endEarly(_:))
        )
        endButton.bezelStyle = .rounded
        let buttonStack = NSStackView(views: [changeButton, collapseButton, endButton])
        buttonStack.orientation = .horizontal
        buttonStack.alignment = .centerY
        buttonStack.spacing = 12

        let stack = NSStackView(
            views: [remainingLabel, resumeLabel, progressView, currentTimeLabel, buttonStack]
        )
        stack.orientation = .vertical
        stack.alignment = .centerX
        stack.spacing = 20
        stack.translatesAutoresizingMaskIntoConstraints = false
        rootView.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: rootView.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: rootView.centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: rootView.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: rootView.trailingAnchor, constant: -24),
        ])
    }

    private func configureSetupPanel() {
        setupPanel.title = "쉬는 시간 타이머"
        setupPanel.isReleasedWhenClosed = false
        setupPanel.level = .floating

        let contentView = NSView()
        setupPanel.contentView = contentView

        let titleLabel = NSTextField(labelWithString: "수업 재개 시각")
        titleLabel.font = .systemFont(ofSize: 20, weight: .semibold)

        let quickStack = NSStackView()
        quickStack.orientation = .horizontal
        quickStack.spacing = 8
        for minutes in [5, 10, 15] {
            let button = NSButton(
                title: "+\(minutes)분",
                target: self,
                action: #selector(selectQuickDuration(_:))
            )
            button.tag = minutes
            quickStack.addArrangedSubview(button)
        }

        timePicker.datePickerStyle = .textFieldAndStepper
        timePicker.datePickerElements = .hourMinute
        timePicker.target = self
        timePicker.action = #selector(timePickerChanged(_:))

        warningLabel.textColor = .systemRed
        warningLabel.font = .systemFont(ofSize: 12, weight: .medium)
        warningLabel.isHidden = true

        let cancelButton = NSButton(
            title: "취소",
            target: self,
            action: #selector(cancelSetup(_:))
        )
        let startButton = NSButton(
            title: "시작",
            target: self,
            action: #selector(startFromSetup(_:))
        )
        startButton.keyEquivalent = "\r"
        let actionStack = NSStackView(views: [cancelButton, startButton])
        actionStack.orientation = .horizontal
        actionStack.spacing = 8

        let stack = NSStackView(
            views: [titleLabel, quickStack, timePicker, warningLabel, actionStack]
        )
        stack.orientation = .vertical
        stack.alignment = .centerX
        stack.spacing = 16
        stack.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: contentView.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: contentView.centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: contentView.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: contentView.trailingAnchor, constant: -24),
        ])
    }

    @objc private func selectQuickDuration(_ sender: NSButton) {
        let minutes = sender.tag
        guard minutes > 0 else {
            NSLog("EduTools: break timer quick button has no minute value")
            return
        }
        let endDate = BreakTimerEngine.endDate(addingMinutes: minutes, to: Date())
        quickEndDate = endDate
        timePicker.dateValue = endDate
        warningLabel.isHidden = true
    }

    @objc private func timePickerChanged(_ sender: Any?) {
        quickEndDate = nil
        warningLabel.isHidden = true
    }

    @objc private func startFromSetup(_ sender: Any?) {
        let now = Date()
        let candidate: Date?
        if let quickEndDate {
            candidate = quickEndDate
        } else {
            let components = Calendar.current.dateComponents([.hour, .minute], from: timePicker.dateValue)
            if let hour = components.hour, let minute = components.minute {
                candidate = BreakTimerEngine.endDate(
                    hour: hour,
                    minute: minute,
                    now: now,
                    calendar: .current
                )
            } else {
                candidate = nil
                NSLog("EduTools: break timer could not read hour and minute from the picker")
            }
        }

        guard let candidate, candidate > now else {
            warningLabel.stringValue = "오늘 시각이 지났습니다"
            warningLabel.isHidden = false
            return
        }
        do {
            try engine.start(until: candidate, now: now)
            setupPanel.orderOut(nil)
            let targetScreen = screen(for: targetDisplayUUID) ?? screenUnderMouse() ?? NSScreen.screens.first
            showTimer(on: targetScreen)
            refresh(now: now)
        } catch {
            warningLabel.stringValue = "오늘 시각이 지났습니다"
            warningLabel.isHidden = false
            NSLog("EduTools: break timer rejected end date: %@", String(describing: error))
        }
    }

    @objc private func cancelSetup(_ sender: Any?) {
        setupPanel.orderOut(nil)
    }

    @objc private func changeEndDate(_ sender: Any?) {
        openSetup()
    }

    @objc private func toggleCollapsed(_ sender: Any?) {
        isCollapsed.toggle()
        applyPresentationFrame()
    }

    @objc private func endEarly(_ sender: Any?) {
        engine.endEarly()
        panel.orderOut(nil)
        setupPanel.orderOut(nil)
    }

    private func showTimer(on screen: NSScreen?) {
        guard let screen else {
            NSLog("EduTools: break timer could not start because no screen is available")
            return
        }
        targetDisplayUUID = displayUUID(for: screen)
        isCollapsed = false
        applyPresentationFrame()
        panel.makeKeyAndOrderFront(nil)
    }

    private func applyPresentationFrame() {
        guard let screen = screen(for: targetDisplayUUID) ?? NSScreen.screens.first else {
            NSLog("EduTools: break timer could not lay out because no screen is available")
            return
        }
        targetDisplayUUID = displayUUID(for: screen)
        if isCollapsed {
            let size = CGSize(width: 420, height: 230)
            panel.setFrame(
                NSRect(
                    x: screen.visibleFrame.midX - size.width / 2,
                    y: screen.visibleFrame.midY - size.height / 2,
                    width: size.width,
                    height: size.height
                ),
                display: true
            )
            panel.hasShadow = true
            rootView.layer?.cornerRadius = 18
            rootView.layer?.masksToBounds = true
            rootView.allowsWindowDrag = true
            progressView.isHidden = true
            currentTimeLabel.isHidden = true
            remainingLabel.font = .monospacedDigitSystemFont(ofSize: 64, weight: .bold)
            resumeLabel.font = .systemFont(ofSize: 20, weight: .semibold)
            collapseButton.title = "전체 화면으로 펼치기"
        } else {
            panel.setFrame(screen.frame, display: true)
            panel.hasShadow = false
            rootView.layer?.cornerRadius = 0
            rootView.layer?.masksToBounds = false
            rootView.allowsWindowDrag = false
            progressView.isHidden = false
            currentTimeLabel.isHidden = false
            remainingLabel.font = .monospacedDigitSystemFont(ofSize: 128, weight: .bold)
            resumeLabel.font = .systemFont(ofSize: 32, weight: .semibold)
            collapseButton.title = "작은 창으로 접기"
        }
    }

    private func startDisplayRefreshTimer() {
        let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            guard let self, engine.phase != .inactive else { return }
            refresh(now: Date())
        }
        RunLoop.main.add(timer, forMode: .common)
        refreshTimer = timer
    }

    private func refresh(now: Date) {
        let didFinish = engine.synchronize(now: now)
        let remaining = engine.remaining(at: now)
        if engine.phase == .finished {
            remainingLabel.stringValue = "수업 재개"
            resumeLabel.stringValue = "00:00"
            remainingLabel.textColor = .systemRed
            progressView.accentColor = .systemRed
        } else {
            remainingLabel.stringValue = LessonTimerController.formattedDuration(remaining)
            if let endDate = engine.endDate {
                resumeLabel.stringValue = "\(Self.timeString(endDate))에 수업 재개"
            }
            let isNearEnd = remaining <= 60
            remainingLabel.textColor = isNearEnd ? .systemOrange : .labelColor
            progressView.accentColor = isNearEnd ? .systemOrange : .controlAccentColor
        }
        currentTimeLabel.stringValue = "현재 시각 \(Self.timeString(now))"
        progressView.progress = engine.initialDuration > 0
            ? CGFloat(remaining / engine.initialDuration)
            : 0
        if didFinish {
            TimerSound.playCompletion()
        }
    }

    private func observeSystemEvents() {
        notificationObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                self?.screenConfigurationDidChange()
            }
        )
        notificationObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSWindow.didMoveNotification,
                object: panel,
                queue: .main
            ) { [weak self] _ in
                self?.captureCollapsedScreen()
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

    private func captureCollapsedScreen() {
        guard isCollapsed else { return }
        let center = CGPoint(x: panel.frame.midX, y: panel.frame.midY)
        if let screen = NSScreen.screens.first(where: { $0.frame.contains(center) }) {
            targetDisplayUUID = displayUUID(for: screen)
        }
    }

    private func screenConfigurationDidChange() {
        guard engine.phase != .inactive else { return }
        if screen(for: targetDisplayUUID) == nil {
            guard let primaryScreen = NSScreen.screens.first else {
                NSLog("EduTools: break timer could not move after a display change because no screen is available")
                return
            }
            targetDisplayUUID = displayUUID(for: primaryScreen)
        }
        applyPresentationFrame()
    }

    private func screen(for identifier: String?) -> NSScreen? {
        guard let identifier else { return nil }
        return NSScreen.screens.first { displayUUID(for: $0) == identifier }
    }

    private static func timeString(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "ko_KR")
        formatter.dateFormat = "HH:mm"
        return formatter.string(from: date)
    }

    deinit {
        refreshTimer?.invalidate()
        for observer in notificationObservers {
            NotificationCenter.default.removeObserver(observer)
            NSWorkspace.shared.notificationCenter.removeObserver(observer)
        }
        panel.close()
        setupPanel.close()
    }
}
