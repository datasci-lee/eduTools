import AppKit
import CoreGraphics
import Foundation

enum DisplayConfigurationOperations {
    enum Operation: Equatable {
        case unmirror(displayID: UInt32)
        case mirror(displayID: UInt32, masterID: UInt32)
        case setOrigin(displayID: UInt32, origin: DisplayPoint)
    }

    static func build(from plan: DisplayPlan) -> [Operation] {
        let outsideDisplayIDs = Set(plan.outsidePlacements.map(\.displayID))
        let unmirrorOperations = Set(plan.unmirrorDisplayIDs).sorted().map {
            Operation.unmirror(displayID: $0)
        }
        let mirrorOperations = plan.mirrors.sorted {
            $0.slaveID == $1.slaveID
                ? $0.masterID < $1.masterID
                : $0.slaveID < $1.slaveID
        }.map {
            Operation.mirror(displayID: $0.slaveID, masterID: $0.masterID)
        }
        let presetOriginOperations = plan.originsByMaster.keys
            .filter { !outsideDisplayIDs.contains($0) }
            .sorted()
            .compactMap { displayID -> Operation? in
                guard let origin = plan.originsByMaster[displayID] else { return nil }
                return .setOrigin(displayID: displayID, origin: origin)
            }
        let outsideOriginOperations = plan.outsidePlacements.map {
            Operation.setOrigin(displayID: $0.displayID, origin: $0.origin)
        }
        return unmirrorOperations
            + mirrorOperations
            + presetOriginOperations
            + outsideOriginOperations
    }
}

struct DisplayPresetMenuEntry {
    let index: Int
    let name: String
    let isCurrent: Bool
    let isEnabled: Bool
    let disabledReason: String?
}

final class DisplayApplier {
    private enum State {
        case idle
        case applying
        case awaitingConfirmation(ConfirmationSession)
    }

    private final class ConfirmationSession {
        let desiredOperations: [DisplayConfigurationOperations.Operation]
        let snapshotOperations: [DisplayConfigurationOperations.Operation]
        let panelController: DisplayConfirmationPanelController

        init(
            desiredOperations: [DisplayConfigurationOperations.Operation],
            snapshotOperations: [DisplayConfigurationOperations.Operation],
            panelController: DisplayConfirmationPanelController
        ) {
            self.desiredOperations = desiredOperations
            self.snapshotOperations = snapshotOperations
            self.panelController = panelController
        }
    }

    private enum ConfirmationDecision {
        case keep
        case revert
    }

    private let settingsStore: SettingsStore
    private let archiveController: ArchiveController
    private var state = State.idle

    init(settingsStore: SettingsStore, archiveController: ArchiveController) {
        self.settingsStore = settingsStore
        self.archiveController = archiveController
    }

    func menuEntries() -> [DisplayPresetMenuEntry] {
        let presets = settingsStore.settings.display.presets
        guard !presets.isEmpty else { return [] }

        let stateReason: String?
        switch state {
        case .idle:
            stateReason = nil
        case .applying:
            stateReason = "다른 화면 구성을 적용하는 중입니다"
        case .awaitingConfirmation:
            stateReason = "현재 화면 구성을 먼저 유지하거나 되돌리세요"
        }

        let online: [OnlineDisplay]
        do {
            online = try OnlineDisplayProvider.current()
        } catch {
            return disabledEntries(
                presets: presets,
                reason: error.localizedDescription
            )
        }
        guard !online.isEmpty else {
            return disabledEntries(presets: presets, reason: "연결된 모니터가 없습니다")
        }

        return presets.enumerated().map { index, preset in
            let plan = DisplayPlanner.plan(preset: preset, online: online)
            let planReason: String?
            if !plan.layoutIssues.isEmpty {
                planReason = plan.layoutIssues.joined(separator: " · ")
            } else if plan.mainDisplayID == nil {
                planReason = "주 디스플레이를 정할 수 없습니다"
            } else {
                planReason = nil
            }
            let disabledReason = stateReason ?? planReason
            return DisplayPresetMenuEntry(
                index: index,
                name: preset.name,
                isCurrent: plan.isApplicable && preset.matchesCurrent(online: online),
                isEnabled: disabledReason == nil,
                disabledReason: disabledReason
            )
        }
    }

    func applyPreset(at index: Int) {
        precondition(Thread.isMainThread, "Display presets must be applied on the main thread")
        switch state {
        case .idle:
            break
        case .applying:
            NSSound.beep()
            return
        case let .awaitingConfirmation(session):
            session.panelController.appendStatus("다른 프리셋을 적용하려면 먼저 유지하거나 되돌리세요.")
            NSSound.beep()
            return
        }

        let presets = settingsStore.settings.display.presets
        guard presets.indices.contains(index) else {
            presentMessage(
                title: "화면 구성을 적용할 수 없습니다",
                message: "프리셋 \(index + 1) 이 없습니다."
            )
            return
        }

        state = .applying
        let online: [OnlineDisplay]
        do {
            online = try OnlineDisplayProvider.current()
        } catch {
            state = .idle
            presentMessage(
                title: "화면 구성을 읽지 못했습니다",
                message: error.localizedDescription
            )
            return
        }
        guard !online.isEmpty else {
            state = .idle
            presentMessage(
                title: "화면 구성을 적용할 수 없습니다",
                message: "연결된 모니터가 없습니다."
            )
            return
        }

        let preset = presets[index]
        let plan = DisplayPlanner.plan(preset: preset, online: online)
        guard plan.isApplicable, let mainDisplayID = plan.mainDisplayID else {
            state = .idle
            let reason = plan.layoutIssues.isEmpty
                ? "주 디스플레이를 정할 수 없습니다."
                : plan.layoutIssues.joined(separator: "\n")
            presentMessage(title: "\(preset.name)을 적용할 수 없습니다", message: reason)
            return
        }

        let snapshot: DisplayPlan
        do {
            snapshot = try DisplayConfigurationSnapshot.make(from: online)
        } catch {
            state = .idle
            presentMessage(
                title: "직전 화면 구성을 보관하지 못했습니다",
                message: error.localizedDescription
            )
            return
        }

        let desiredMirrorDisplayIDs = Set(plan.mirrors.map(\.slaveID))
        let disappearingDisplayUUIDs = Set(online.compactMap { display -> String? in
            display.currentMirrorMasterID == nil && desiredMirrorDisplayIDs.contains(display.id)
                ? display.uuid
                : nil
        })
        guard let archivedDisplayUUIDs = archiveController.archiveDisconnectedDisplaysForPreset(
            disappearingDisplayUUIDs
        ) else {
            state = .idle
            return
        }

        let desiredOperations = DisplayConfigurationOperations.build(from: plan)
        let snapshotOperations = DisplayConfigurationOperations.build(from: snapshot)
        archiveController.setDisplayPresetRestorationDeferred(true)
        switch DisplayConfigurationLayer.apply(desiredOperations, option: .appOnly) {
        case .success:
            break
        case let .failure(error):
            state = .idle
            archiveController.setDisplayPresetRestorationDeferred(false)
            archiveController.restoreDisplaysArchivedForFailedPreset(archivedDisplayUUIDs)
            presentMessage(
                title: "화면 구성을 적용하지 못했습니다",
                message: error.localizedDescription
            )
            return
        }

        let skippedMonitorNames = plan.missingPresetMonitors.map(\.name)
        let panelController = DisplayConfirmationPanelController(
            interval: DisplayPresetDefaults.manualConfirmationInterval,
            skippedMonitorNames: skippedMonitorNames,
            decisionHandler: { [weak self] decision in
                switch decision {
                case .keep:
                    self?.handleConfirmationDecision(.keep)
                case .revert:
                    self?.handleConfirmationDecision(.revert)
                }
            }
        )
        let session = ConfirmationSession(
            desiredOperations: desiredOperations,
            snapshotOperations: snapshotOperations,
            panelController: panelController
        )
        state = .awaitingConfirmation(session)
        panelController.show(on: mainDisplayID)
    }

    func applicationWillTerminate() {
        precondition(Thread.isMainThread, "Display rollback must run on the main thread")
        guard case let .awaitingConfirmation(session) = state else { return }
        session.panelController.close()
        let result = DisplayConfigurationLayer.apply(
            session.snapshotOperations,
            option: .appOnly
        )
        state = .idle
        archiveController.setDisplayPresetRestorationDeferred(
            false,
            resumePending: false
        )
        if case let .failure(error) = result {
            NSLog(
                "EduTools: display rollback during termination failed; app-only configuration will revert on exit: %@",
                error.localizedDescription
            )
        }
    }

    private func handleConfirmationDecision(_ decision: ConfirmationDecision) {
        precondition(Thread.isMainThread, "Display confirmation must run on the main thread")
        guard case let .awaitingConfirmation(session) = state else { return }
        session.panelController.close()

        switch decision {
        case .keep:
            switch DisplayConfigurationLayer.apply(
                session.desiredOperations,
                option: .permanent
            ) {
            case .success:
                state = .idle
                archiveController.setDisplayPresetRestorationDeferred(false)
            case let .failure(keepError):
                let rollbackResult = DisplayConfigurationLayer.apply(
                    session.snapshotOperations,
                    option: .appOnly
                )
                state = .idle
                archiveController.setDisplayPresetRestorationDeferred(false)
                switch rollbackResult {
                case .success:
                    presentMessage(
                        title: "화면 구성을 유지하지 못했습니다",
                        message: "영구 적용에 실패해 직전 구성으로 되돌렸습니다.\n\n\(keepError.localizedDescription)"
                    )
                case let .failure(rollbackError):
                    presentRollbackFailure(
                        primaryError: keepError,
                        rollbackError: rollbackError
                    )
                }
            }
        case .revert:
            let rollbackResult = DisplayConfigurationLayer.apply(
                session.snapshotOperations,
                option: .appOnly
            )
            state = .idle
            archiveController.setDisplayPresetRestorationDeferred(false)
            if case let .failure(error) = rollbackResult {
                presentRollbackFailure(primaryError: nil, rollbackError: error)
            }
        }
    }

    private func disabledEntries(
        presets: [DisplayPreset],
        reason: String
    ) -> [DisplayPresetMenuEntry] {
        presets.enumerated().map { index, preset in
            DisplayPresetMenuEntry(
                index: index,
                name: preset.name,
                isCurrent: false,
                isEnabled: false,
                disabledReason: reason
            )
        }
    }

    private func presentRollbackFailure(
        primaryError: Error?,
        rollbackError: Error
    ) {
        var details: [String] = []
        if let primaryError {
            details.append(primaryError.localizedDescription)
        }
        details.append(rollbackError.localizedDescription)
        details.append("EduTools를 종료하면 임시(.forAppOnly) 화면 구성은 시스템이 자동으로 복구합니다.")
        presentMessage(
            title: "직전 화면 구성으로 되돌리지 못했습니다",
            message: details.joined(separator: "\n\n")
        )
    }

    private func presentMessage(title: String, message: String) {
        NSLog("EduTools: %@: %@", title, message)
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "확인")
        NSApp.activate()
        alert.runAboveOverlay()
    }
}

private enum DisplayConfigurationSnapshot {
    enum SnapshotError: LocalizedError {
        case mainDisplayUnavailable

        var errorDescription: String? {
            "원점 (0,0)의 독립 주 디스플레이를 찾지 못했습니다."
        }
    }

    static func make(from online: [OnlineDisplay]) throws -> DisplayPlan {
        guard let mainDisplay = online.first(where: {
            $0.currentMirrorMasterID == nil
                && approximatelyEqual($0.origin, .zero)
        }) else {
            throw SnapshotError.mainDisplayUnavailable
        }
        let independentDisplays = online.filter { $0.currentMirrorMasterID == nil }
        let origins = Dictionary(uniqueKeysWithValues: independentDisplays.map {
            ($0.id, $0.origin)
        })
        let mirrors = online.compactMap { display -> DisplayMirrorPlan? in
            guard let masterID = display.currentMirrorMasterID else { return nil }
            return DisplayMirrorPlan(slaveID: display.id, masterID: masterID)
        }
        return DisplayPlan(
            mainDisplayID: mainDisplay.id,
            originsByMaster: origins,
            mirrors: mirrors,
            unmirrorDisplayIDs: online.map(\.id),
            outsidePlacements: [],
            missingPresetMonitors: [],
            screens: [],
            layoutIssues: []
        )
    }

    private static func approximatelyEqual(
        _ left: DisplayPoint,
        _ right: DisplayPoint
    ) -> Bool {
        abs(left.x - right.x) < 0.5 && abs(left.y - right.y) < 0.5
    }
}

private enum DisplayConfigurationLayer {
    enum Option {
        case appOnly
        case permanent

        var cgValue: CGConfigureOption {
            switch self {
            case .appOnly: return .forAppOnly
            case .permanent: return .permanently
            }
        }

        var logName: String {
            switch self {
            case .appOnly: return "forAppOnly"
            case .permanent: return "permanently"
            }
        }
    }

    enum ConfigurationError: LocalizedError {
        case invalidOrigin(displayID: UInt32, origin: DisplayPoint)
        case beginFailed(error: CGError, cancellationError: CGError?)
        case missingConfiguration
        case operationFailed(
            operation: String,
            error: CGError,
            cancellationError: CGError?
        )
        case completionFailed(option: String, error: CGError)

        var errorDescription: String? {
            switch self {
            case let .invalidOrigin(displayID, origin):
                return "모니터 \(displayID)의 원점 (\(origin.x), \(origin.y))을 정수 좌표로 적용할 수 없습니다."
            case let .beginFailed(error, cancellationError):
                return errorMessage(
                    "화면 구성 트랜잭션을 시작하지 못했습니다",
                    error: error,
                    cancellationError: cancellationError
                )
            case .missingConfiguration:
                return "화면 구성 트랜잭션이 유효한 구성 객체를 반환하지 않았습니다."
            case let .operationFailed(operation, error, cancellationError):
                return errorMessage(
                    "화면 구성 작업에 실패했습니다: \(operation)",
                    error: error,
                    cancellationError: cancellationError
                )
            case let .completionFailed(option, error):
                return "화면 구성 트랜잭션을 \(option) 옵션으로 완료하지 못했습니다. CoreGraphics 오류 \(error.rawValue)"
            }
        }

        private func errorMessage(
            _ prefix: String,
            error: CGError,
            cancellationError: CGError?
        ) -> String {
            var message = "\(prefix). CoreGraphics 오류 \(error.rawValue)"
            if let cancellationError {
                message += "; 취소도 실패했습니다(CoreGraphics 오류 \(cancellationError.rawValue))"
            }
            return message
        }
    }

    private enum PreparedOperation {
        case unmirror(displayID: UInt32)
        case mirror(displayID: UInt32, masterID: UInt32)
        case setOrigin(displayID: UInt32, x: Int32, y: Int32)

        var description: String {
            switch self {
            case let .unmirror(displayID):
                return "unmirror display \(displayID)"
            case let .mirror(displayID, masterID):
                return "mirror display \(displayID) to \(masterID)"
            case let .setOrigin(displayID, x, y):
                return "set display \(displayID) origin to (\(x), \(y))"
            }
        }
    }

    static func apply(
        _ operations: [DisplayConfigurationOperations.Operation],
        option: Option
    ) -> Result<Void, ConfigurationError> {
        let prepared: [PreparedOperation]
        do {
            prepared = try operations.map(prepare)
        } catch let error as ConfigurationError {
            NSLog("EduTools: display configuration rejected before begin: %@", error.localizedDescription)
            return .failure(error)
        } catch {
            preconditionFailure("Unexpected display operation preparation error")
        }

        var configuration: CGDisplayConfigRef?
        let beginResult = CGBeginDisplayConfiguration(&configuration)
        guard beginResult == .success else {
            let cancellationError = configuration.flatMap(cancel)
            let error = ConfigurationError.beginFailed(
                error: beginResult,
                cancellationError: cancellationError
            )
            NSLog("EduTools: %@", error.localizedDescription)
            return .failure(error)
        }
        guard let configuration else {
            let error = ConfigurationError.missingConfiguration
            NSLog("EduTools: %@", error.localizedDescription)
            return .failure(error)
        }

        for operation in prepared {
            let result: CGError
            switch operation {
            case let .unmirror(displayID):
                result = CGConfigureDisplayMirrorOfDisplay(
                    configuration,
                    displayID,
                    kCGNullDirectDisplay
                )
            case let .mirror(displayID, masterID):
                result = CGConfigureDisplayMirrorOfDisplay(
                    configuration,
                    displayID,
                    masterID
                )
            case let .setOrigin(displayID, x, y):
                result = CGConfigureDisplayOrigin(configuration, displayID, x, y)
            }
            guard result == .success else {
                let cancellationError = cancel(configuration)
                let error = ConfigurationError.operationFailed(
                    operation: operation.description,
                    error: result,
                    cancellationError: cancellationError
                )
                NSLog("EduTools: %@", error.localizedDescription)
                return .failure(error)
            }
        }

        let completeResult = CGCompleteDisplayConfiguration(configuration, option.cgValue)
        guard completeResult == .success else {
            // The SDK contract invalidates `configuration` when complete returns, including failure.
            let error = ConfigurationError.completionFailed(
                option: option.logName,
                error: completeResult
            )
            NSLog("EduTools: %@", error.localizedDescription)
            return .failure(error)
        }
        return .success(())
    }

    private static func prepare(
        _ operation: DisplayConfigurationOperations.Operation
    ) throws -> PreparedOperation {
        switch operation {
        case let .unmirror(displayID):
            return .unmirror(displayID: displayID)
        case let .mirror(displayID, masterID):
            return .mirror(displayID: displayID, masterID: masterID)
        case let .setOrigin(displayID, origin):
            guard let x = exactCoordinate(origin.x), let y = exactCoordinate(origin.y) else {
                throw ConfigurationError.invalidOrigin(displayID: displayID, origin: origin)
            }
            return .setOrigin(displayID: displayID, x: x, y: y)
        }
    }

    private static func exactCoordinate(_ value: Double) -> Int32? {
        guard value.isFinite,
              value.rounded() == value,
              value >= Double(Int32.min),
              value <= Double(Int32.max)
        else {
            return nil
        }
        return Int32(value)
    }

    private static func cancel(_ configuration: CGDisplayConfigRef) -> CGError? {
        let cancelResult = CGCancelDisplayConfiguration(configuration)
        guard cancelResult != .success else { return nil }
        NSLog(
            "EduTools: failed to cancel display configuration (CGError %d)",
            cancelResult.rawValue
        )
        return cancelResult
    }
}

private final class DisplayConfirmationPanel: NSPanel {
    var cancelHandler: (() -> Void)?

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func cancelOperation(_ sender: Any?) {
        cancelHandler?()
    }
}

private final class DisplayConfirmationPanelController: NSObject, NSWindowDelegate {
    enum Decision {
        case keep
        case revert
    }

    private let panel: DisplayConfirmationPanel
    private let countdownLabel: NSTextField
    private let statusLabel: NSTextField
    private let deadline: TimeInterval
    private let decisionHandler: (Decision) -> Void
    private var timer: Timer?
    private var didSendDecision = false

    init(
        interval: TimeInterval,
        skippedMonitorNames: [String],
        decisionHandler: @escaping (Decision) -> Void
    ) {
        deadline = ProcessInfo.processInfo.systemUptime + interval
        self.decisionHandler = decisionHandler
        panel = DisplayConfirmationPanel(
            contentRect: NSRect(x: 0, y: 0, width: 470, height: 190),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        countdownLabel = NSTextField(wrappingLabelWithString: "")
        statusLabel = NSTextField(wrappingLabelWithString: skippedMonitorNames.isEmpty
            ? ""
            : "건너뛴 모니터: \(skippedMonitorNames.joined(separator: ", "))")
        super.init()

        panel.title = "화면 구성 확인"
        panel.level = AppWindowLevel.dialog
        panel.collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.delegate = self
        panel.cancelHandler = { [weak self] in self?.sendDecision(.revert) }

        let titleLabel = NSTextField(labelWithString: "화면 구성을 유지할까요?")
        titleLabel.font = .boldSystemFont(ofSize: NSFont.systemFontSize + 1)
        countdownLabel.maximumNumberOfLines = 2
        statusLabel.maximumNumberOfLines = 3
        statusLabel.textColor = .secondaryLabelColor
        statusLabel.isHidden = statusLabel.stringValue.isEmpty

        let keepButton = NSButton(
            title: "유지",
            target: self,
            action: #selector(keep(_:))
        )
        keepButton.keyEquivalent = "\r"
        let revertButton = NSButton(
            title: "되돌리기",
            target: self,
            action: #selector(revert(_:))
        )
        revertButton.keyEquivalent = "\u{1b}"
        let buttonRow = NSStackView(views: [NSView(), revertButton, keepButton])
        buttonRow.orientation = .horizontal
        buttonRow.spacing = 8
        buttonRow.distribution = .fill

        let stack = NSStackView(views: [titleLabel, countdownLabel, statusLabel, buttonRow])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.translatesAutoresizingMaskIntoConstraints = false
        buttonRow.translatesAutoresizingMaskIntoConstraints = false

        let contentView = NSView()
        contentView.addSubview(stack)
        panel.contentView = contentView
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 20),
            stack.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -20),
            stack.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 18),
            stack.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -18),
            buttonRow.widthAnchor.constraint(equalTo: stack.widthAnchor),
        ])
        updateCountdown()
    }

    func show(on mainDisplayID: UInt32) {
        let targetScreen = NSScreen.screens.first { screen in
            displayID(for: screen) == mainDisplayID
        }
        if let targetScreen {
            let visibleFrame = targetScreen.visibleFrame
            let origin = NSPoint(
                x: visibleFrame.midX - panel.frame.width / 2,
                y: visibleFrame.midY - panel.frame.height / 2
            )
            panel.setFrameOrigin(origin)
        } else {
            NSLog(
                "EduTools: applied main display %u was not in NSScreen.screens; centering confirmation on the current main screen",
                mainDisplayID
            )
            panel.center()
        }
        let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            self?.updateCountdown()
        }
        self.timer = timer
        RunLoop.main.add(timer, forMode: .common)
        NSApp.activate()
        panel.makeKeyAndOrderFront(nil)
    }

    func appendStatus(_ message: String) {
        if statusLabel.stringValue.isEmpty {
            statusLabel.stringValue = message
        } else if !statusLabel.stringValue.contains(message) {
            statusLabel.stringValue += "\n\(message)"
        }
        statusLabel.isHidden = false
        panel.makeKeyAndOrderFront(nil)
    }

    func close() {
        timer?.invalidate()
        timer = nil
        panel.delegate = nil
        panel.cancelHandler = nil
        panel.orderOut(nil)
        panel.close()
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sendDecision(.revert)
        return false
    }

    @objc private func keep(_ sender: Any?) {
        sendDecision(.keep)
    }

    @objc private func revert(_ sender: Any?) {
        sendDecision(.revert)
    }

    private func updateCountdown() {
        let remaining = max(
            0,
            Int(ceil(deadline - ProcessInfo.processInfo.systemUptime))
        )
        countdownLabel.stringValue = "\(remaining)초 후 자동으로 되돌립니다."
        if remaining == 0 {
            sendDecision(.revert)
        }
    }

    private func sendDecision(_ decision: Decision) {
        guard !didSendDecision else { return }
        didSendDecision = true
        timer?.invalidate()
        decisionHandler(decision)
    }
}
