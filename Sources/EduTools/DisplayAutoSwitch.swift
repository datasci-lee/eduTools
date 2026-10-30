import AppKit
import Foundation

enum DisplayAutoSwitchDecision: Equatable {
    case ignore
    case apply(index: Int)
    case retry(after: TimeInterval)
}

enum DisplayAutoSwitchPolicy {
    static func decide(
        online: [OnlineDisplay],
        presets: [DisplayPreset],
        enabled: Bool,
        lastJudgedUUIDs: Set<String>?,
        lastAutoApplyAt: TimeInterval?,
        now: TimeInterval,
        cooldown: TimeInterval = DisplayPresetDefaults.automaticApplyCooldown
    ) -> DisplayAutoSwitchDecision {
        let onlineUUIDs = Set(online.map(\.uuid))
        guard onlineUUIDs != lastJudgedUUIDs else { return .ignore }
        guard enabled else { return .ignore }

        if let lastAutoApplyAt {
            let elapsed = now - lastAutoApplyAt
            if elapsed < cooldown {
                return .retry(after: cooldown - elapsed)
            }
        }

        guard let index = presets.firstIndex(where: { $0.matchesAutoSet(online: online) }) else {
            return .ignore
        }
        guard !presets[index].matchesCurrent(online: online) else { return .ignore }
        return .apply(index: index)
    }
}

final class DisplayAutoSwitchCoordinator {
    typealias Schedule = (TimeInterval, @escaping () -> Void) -> DispatchWorkItem

    private let settingsStore: SettingsStore
    private let applier: DisplayApplier
    private let onlineProvider: () throws -> [OnlineDisplay]
    private let now: () -> TimeInterval
    private let schedule: Schedule
    private var screenParametersObserver: NSObjectProtocol?
    private var pendingWorkItem: DispatchWorkItem?
    private var lastJudgedUUIDs: Set<String>?
    private var lastAutoApplyAt: TimeInterval?

    init(
        settingsStore: SettingsStore,
        applier: DisplayApplier,
        onlineProvider: @escaping () throws -> [OnlineDisplay] = {
            try OnlineDisplayProvider.current()
        },
        now: @escaping () -> TimeInterval = {
            ProcessInfo.processInfo.systemUptime
        },
        schedule: @escaping Schedule = { delay, action in
            let workItem = DispatchWorkItem(block: action)
            DispatchQueue.main.asyncAfter(
                deadline: .now() + delay,
                execute: workItem
            )
            return workItem
        }
    ) {
        self.settingsStore = settingsStore
        self.applier = applier
        self.onlineProvider = onlineProvider
        self.now = now
        self.schedule = schedule
    }

    func start() {
        precondition(Thread.isMainThread, "Display auto-switch must start on the main thread")
        guard screenParametersObserver == nil else { return }

        do {
            lastJudgedUUIDs = Set(try onlineProvider().map(\.uuid))
        } catch {
            NSLog(
                "EduTools: failed to initialize display auto-switch state: %@",
                error.localizedDescription
            )
        }

        screenParametersObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.scheduleJudgment(after: DisplayPresetDefaults.connectionStabilityInterval)
        }
    }

    func stop() {
        pendingWorkItem?.cancel()
        pendingWorkItem = nil
        if let screenParametersObserver {
            NotificationCenter.default.removeObserver(screenParametersObserver)
            self.screenParametersObserver = nil
        }
    }

    deinit {
        stop()
    }

    private func scheduleJudgment(after delay: TimeInterval) {
        precondition(Thread.isMainThread, "Display auto-switch must schedule on the main thread")
        pendingWorkItem?.cancel()
        pendingWorkItem = schedule(delay) { [weak self] in
            self?.judgeCurrentConnectionSet()
        }
    }

    private func judgeCurrentConnectionSet() {
        precondition(Thread.isMainThread, "Display auto-switch must judge on the main thread")
        pendingWorkItem = nil

        guard applier.state == .idle else {
            scheduleJudgment(after: DisplayPresetDefaults.connectionStabilityInterval)
            return
        }

        let online: [OnlineDisplay]
        do {
            online = try onlineProvider()
        } catch {
            NSLog(
                "EduTools: failed to read displays for auto-switch: %@",
                error.localizedDescription
            )
            return
        }

        let currentUUIDs = Set(online.map(\.uuid))
        let currentTime = now()
        let decision = DisplayAutoSwitchPolicy.decide(
            online: online,
            presets: settingsStore.settings.display.presets,
            enabled: settingsStore.settings.display.autoSwitchEnabled,
            lastJudgedUUIDs: lastJudgedUUIDs,
            lastAutoApplyAt: lastAutoApplyAt,
            now: currentTime
        )

        switch decision {
        case .ignore:
            lastJudgedUUIDs = currentUUIDs
        case let .apply(index):
            lastAutoApplyAt = currentTime
            lastJudgedUUIDs = currentUUIDs
            applier.applyPresetAutomatically(at: index)
        case let .retry(after: delay):
            scheduleJudgment(after: delay)
        }
    }
}
