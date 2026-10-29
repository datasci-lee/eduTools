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

private final class DisplayApplierHarness {
  let defaults: IsolatedDefaults
  let layer = TestDisplayLayer()
  let clock = FakeMonotonicClock()
  let applier: DisplayApplier

  let desiredOperations: [DisplayConfigurationOperations.Operation] = [
    .unmirror(displayID: 2),
    .setOrigin(displayID: 2, origin: DisplayPoint(x: 1_920, y: 0)),
  ]
  let snapshotOperations: [DisplayConfigurationOperations.Operation] = [
    .unmirror(displayID: 2),
    .setOrigin(displayID: 2, origin: .zero),
  ]

  init() throws {
    defaults = try IsolatedDefaults()
    let settingsStore = SettingsStore(defaults: defaults.value)
    let state = AppState(defaults: defaults.value, settingsStore: settingsStore)
    let archiveController = ArchiveController(state: state, settingsStore: settingsStore)
    applier = DisplayApplier(
      settingsStore: settingsStore,
      archiveController: archiveController,
      displayConfigurationLayer: layer,
      now: { [clock] in clock.value }
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
