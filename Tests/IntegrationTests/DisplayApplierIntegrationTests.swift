import CoreGraphics
import Foundation
import Testing
import TestSupport

@testable import EduTools

extension FakeDisplayLayer: DisplayConfigurationApplying
where
  Operation == DisplayConfigurationOperations.Operation,
  Option == DisplayConfigurationOption,
  Failure == DisplayConfigurationError
{}

private typealias TestDisplayLayer = FakeDisplayLayer<
  DisplayConfigurationOperations.Operation,
  DisplayConfigurationOption,
  DisplayConfigurationError
>

private final class FakeMonotonicClock {
  var value: TimeInterval = 100
}

private final class FakeDisplayPresetArchiveController: DisplayPresetArchiveControlling {
  private(set) var archiveRequests: [Set<String>] = []
  private(set) var restoredDisplayUUIDs: [Set<String>] = []
  private(set) var deferredTransitions: [Bool] = []
  private(set) var isRestorationDeferred = false

  func archiveDisconnectedDisplaysForPreset(_ displayUUIDs: Set<String>) -> Set<String>? {
    archiveRequests.append(displayUUIDs)
    return displayUUIDs
  }

  func restoreDisplaysArchivedForFailedPreset(_ displayUUIDs: Set<String>) {
    restoredDisplayUUIDs.append(displayUUIDs)
  }

  func setDisplayPresetRestorationDeferred(_ isDeferred: Bool, resumePending: Bool) {
    isRestorationDeferred = isDeferred
    deferredTransitions.append(isDeferred)
  }
}

private final class DisplayMessageRecorder {
  struct Message: Equatable {
    let title: String
    let body: String
  }

  private(set) var messages: [Message] = []

  func record(title: String, body: String) {
    messages.append(Message(title: title, body: body))
  }
}

private final class DisplayApplierHarness {
  let defaults: IsolatedDefaults
  let layer = TestDisplayLayer()
  let clock = FakeMonotonicClock()
  let archiveController: DisplayPresetArchiveControlling
  let messageRecorder = DisplayMessageRecorder()
  let settingsStore: SettingsStore
  let applier: DisplayApplier

  let desiredOperations: [DisplayConfigurationOperations.Operation] = [
    .unmirror(displayID: 2),
    .setOrigin(displayID: 2, origin: DisplayPoint(x: 1_920, y: 0)),
  ]
  let snapshotOperations: [DisplayConfigurationOperations.Operation] = [
    .unmirror(displayID: 2),
    .setOrigin(displayID: 2, origin: .zero),
  ]

  init(
    onlineProvider: @escaping () throws -> [OnlineDisplay] = { [] },
    archiveController: DisplayPresetArchiveControlling? = nil
  ) throws {
    defaults = try IsolatedDefaults()
    settingsStore = SettingsStore(defaults: defaults.value)
    let resolvedArchiveController: DisplayPresetArchiveControlling
    if let archiveController {
      resolvedArchiveController = archiveController
    } else {
      let state = AppState(defaults: defaults.value, settingsStore: settingsStore)
      resolvedArchiveController = ArchiveController(state: state, settingsStore: settingsStore)
    }
    self.archiveController = resolvedArchiveController
    applier = DisplayApplier(
      settingsStore: settingsStore,
      archiveController: resolvedArchiveController,
      displayConfigurationLayer: layer,
      onlineProvider: onlineProvider,
      now: { [clock] in clock.value },
      messagePresenter: { [messageRecorder] title, body in
        messageRecorder.record(title: title, body: body)
      }
    )
  }

  func begin() -> Result<Void, DisplayConfigurationError> {
    applier.beginApplying(
      desiredOperations: desiredOperations,
      snapshotOperations: snapshotOperations
    )
  }
}

@Suite("화면 적용 상태 기계")
struct DisplayApplierIntegrationTests {
  @Test("적용 후 유지는 appOnly 다음 permanent 순서로 실행하고 idle로 돌아간다")
  func keepUsesAppOnlyThenPermanentAndReturnsToIdle() throws {
    let harness = try DisplayApplierHarness()
    var observedStates: [DisplayApplier.State] = []
    harness.layer.callObserver = { _ in observedStates.append(harness.applier.state) }

    #expect(harness.begin().isSuccess)
    #expect(observedStates == [.applying])
    #expect(harness.applier.state == .awaitingConfirmation)
    let outcome = harness.applier.resolveConfirmation(.keep)

    #expect(Self.succeeded(outcome))
    #expect(harness.applier.state == .idle)
    #expect(
      harness.layer.calls == [
        .init(operations: harness.desiredOperations, option: .appOnly),
        .init(operations: harness.desiredOperations, option: .permanent),
      ]
    )
  }

  @Test("되돌리기 버튼은 스냅샷을 appOnly로 적용하고 idle로 돌아간다")
  func revertButtonAppliesSnapshotAndReturnsToIdle() throws {
    let harness = try DisplayApplierHarness()

    #expect(harness.begin().isSuccess)
    let outcome = harness.applier.resolveConfirmation(.revert)

    #expect(Self.succeeded(outcome))
    #expect(harness.applier.state == .idle)
    #expect(
      harness.layer.calls == [
        .init(operations: harness.desiredOperations, option: .appOnly),
        .init(operations: harness.snapshotOperations, option: .appOnly),
      ]
    )
  }

  @Test("Esc 취소는 영구 적용 없이 스냅샷으로 복구한다")
  func escapeCancellationRestoresWithoutPermanentApply() throws {
    let harness = try DisplayApplierHarness()

    #expect(harness.begin().isSuccess)
    let outcome = harness.applier.resolveConfirmation(.cancel)

    #expect(Self.succeeded(outcome))
    #expect(harness.applier.state == .idle)
    #expect(harness.layer.calls.allSatisfy { $0.option == .appOnly })
    #expect(harness.layer.calls.last?.operations == harness.snapshotOperations)
  }

  @Test("15초 만료는 가짜 시계로 즉시 스냅샷 복구를 일으킨다")
  func timeoutUsesInjectedClockAndRestoresSnapshot() throws {
    let harness = try DisplayApplierHarness()

    #expect(harness.begin().isSuccess)
    #expect(harness.applier.confirmationRemainingSeconds == 15)
    harness.clock.value += 15
    let outcome = harness.applier.expireConfirmationIfNeeded()

    #expect(Self.succeeded(outcome))
    #expect(harness.applier.state == .idle)
    #expect(harness.layer.calls.last?.operations == harness.snapshotOperations)
    #expect(harness.layer.calls.allSatisfy { $0.option != .permanent })
  }

  @Test("첫 적용 실패는 확인 상태에 들어가지 않고 idle로 복귀한다")
  func applicationFailureReturnsDirectlyToIdle() throws {
    let harness = try DisplayApplierHarness()
    harness.layer.enqueue(.failure(Self.failure(option: "forAppOnly")))

    let result = harness.begin()

    #expect(result.isFailure)
    #expect(harness.applier.state == .idle)
    #expect(harness.layer.calls.count == 1)
    #expect(harness.layer.calls[0].option == .appOnly)
  }

  @Test("되돌리기 실패도 상태는 idle로 끝나고 permanent는 호출하지 않는다")
  func rollbackFailureIsReportedAfterStateReturnsToIdle() throws {
    let harness = try DisplayApplierHarness()
    harness.layer.enqueue(.success(()))
    harness.layer.enqueue(.failure(Self.failure(option: "forAppOnly")))

    #expect(harness.begin().isSuccess)
    let outcome = harness.applier.resolveConfirmation(.revert)

    #expect(Self.revertFailed(outcome))
    #expect(harness.applier.state == .idle)
    #expect(harness.layer.calls.allSatisfy { $0.option == .appOnly })
  }

  @Test("확인 대기 중 종료는 스냅샷을 appOnly로 복구한다")
  func terminationWhileAwaitingConfirmationRestoresSnapshot() throws {
    let harness = try DisplayApplierHarness()

    #expect(harness.begin().isSuccess)
    let result = harness.applier.rollbackForTermination()

    #expect(result?.isSuccess == true)
    #expect(harness.applier.state == .idle)
    #expect(
      harness.layer.calls == [
        .init(operations: harness.desiredOperations, option: .appOnly),
        .init(operations: harness.snapshotOperations, option: .appOnly),
      ]
    )
  }

  @Test(
    "자동 적용은 패널 없이 검증 성공을 영구 확정하고 불일치를 스냅샷으로 복구한다",
    arguments: [true, false]
  )
  @MainActor
  func automaticApplyVerifiesBeforeKeepingOrRestoresSnapshot(
    verificationMatches: Bool
  ) throws {
    let initialOnline = Self.onlineDisplays(secondIsMirrored: false)
    let verificationOnline = Self.onlineDisplays(secondIsMirrored: verificationMatches)
    var onlineReadCount = 0
    let archiveController = FakeDisplayPresetArchiveController()
    let harness = try DisplayApplierHarness(
      onlineProvider: {
        defer { onlineReadCount += 1 }
        return onlineReadCount == 0 ? initialOnline : verificationOnline
      },
      archiveController: archiveController
    )
    let preset = Self.mirroredPreset(online: initialOnline)
    #expect(harness.settingsStore.update { $0.display.presets = [preset] }.isSuccess)

    harness.applier.applyPresetAutomatically(at: 0)

    let desiredOperations: [DisplayConfigurationOperations.Operation] = [
      .mirror(displayID: 2, masterID: 1),
      .setOrigin(displayID: 1, origin: .zero),
    ]
    let snapshotOperations: [DisplayConfigurationOperations.Operation] = [
      .unmirror(displayID: 1),
      .unmirror(displayID: 2),
      .setOrigin(displayID: 1, origin: .zero),
      .setOrigin(displayID: 2, origin: DisplayPoint(x: 1_920, y: 0)),
    ]
    let expectedCalls: [TestDisplayLayer.Call] = verificationMatches
      ? [
          .init(operations: desiredOperations, option: .appOnly),
          .init(operations: desiredOperations, option: .permanent),
        ]
      : [
          .init(operations: desiredOperations, option: .appOnly),
          .init(operations: snapshotOperations, option: .appOnly),
        ]

    #expect(onlineReadCount == 2)
    #expect(harness.layer.calls == expectedCalls)
    #expect(harness.applier.state == .idle)
    #expect(harness.applier.confirmationRemainingSeconds == nil)
    #expect(archiveController.archiveRequests == [["display-2"]])
    #expect(archiveController.deferredTransitions.first == true)
    #expect(archiveController.deferredTransitions.last == false)
    #expect(!archiveController.isRestorationDeferred)
    if verificationMatches {
      #expect(archiveController.restoredDisplayUUIDs.isEmpty)
      #expect(harness.messageRecorder.messages.isEmpty)
    } else {
      #expect(archiveController.restoredDisplayUUIDs == [["display-2"]])
      #expect(harness.messageRecorder.messages.count == 1)
      #expect(harness.messageRecorder.messages[0].title.contains(preset.name))
    }
  }

  private static func onlineDisplays(secondIsMirrored: Bool) -> [OnlineDisplay] {
    [
      OnlineDisplay(
        id: 1,
        uuid: "display-1",
        vendor: 1,
        model: 1,
        serial: 1,
        name: "주 모니터",
        kind: .builtIn,
        size: DisplaySize(width: 1_920, height: 1_080),
        origin: .zero,
        currentMirrorMasterID: nil
      ),
      OnlineDisplay(
        id: 2,
        uuid: "display-2",
        vendor: 2,
        model: 2,
        serial: 2,
        name: "보조 모니터",
        kind: .external,
        size: DisplaySize(width: 1_920, height: 1_080),
        origin: DisplayPoint(x: 1_920, y: 0),
        currentMirrorMasterID: secondIsMirrored ? 1 : nil
      ),
    ]
  }

  private static func mirroredPreset(online: [OnlineDisplay]) -> DisplayPreset {
    DisplayPreset(
      name: "강의 미러",
      main: PresetScreen(
        monitors: online.map { PresetMonitor(identity: $0.identity) }
      )
    )
  }

  private static func failure(option: String) -> DisplayConfigurationError {
    .completionFailed(option: option, error: .failure)
  }

  private static func succeeded(_ outcome: DisplayApplier.ConfirmationOutcome?) -> Bool {
    if case .success? = outcome { return true }
    return false
  }

  private static func revertFailed(_ outcome: DisplayApplier.ConfirmationOutcome?) -> Bool {
    if case .revertFailed? = outcome { return true }
    return false
  }
}

private extension Result where Success == Void {
  var isSuccess: Bool {
    if case .success = self { return true }
    return false
  }

  var isFailure: Bool { !isSuccess }
}
