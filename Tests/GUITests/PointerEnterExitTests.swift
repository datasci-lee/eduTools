import AppKit
import Testing

@testable import EduTools
import TestSupport

@MainActor
@Suite("포인터 진입·이탈", .serialized)
struct PointerEnterExitTests {
  @Test("합성 enter와 exit는 pointingDeviceType을 조회하지 않고 처리된다")
  func enterAndExitAvoidPointingDeviceQueries() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let suiteName = "EduTools.GUITests.Pointer.\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let state = AppState(defaults: defaults)
    let dispatcher = AppCommandDispatcher(state: state)
    var focusRequestCount = 0
    let view = InkView(
      frame: CGRect(x: 0, y: 0, width: 120, height: 80),
      state: state,
      dispatcher: dispatcher,
      displayUUID: "gui-pointer-enter-exit",
      requestKeyFocus: { focusRequestCount += 1 }
    )
    let host = OffscreenWindowHost(view: view, size: view.frame.size)
    let entered = try SyntheticEvents.enterExitEvent(
      type: .mouseEntered,
      location: CGPoint(x: 20, y: 20),
      in: host.window,
      timestamp: 1
    )
    let exited = try SyntheticEvents.enterExitEvent(
      type: .mouseExited,
      location: CGPoint(x: 30, y: 30),
      in: host.window,
      timestamp: 2,
      eventNumber: 2
    )

    view.mouseEntered(with: entered)
    view.mouseExited(with: exited)

    #expect(focusRequestCount == 1)
    #expect(state.strokes(for: "gui-pointer-enter-exit").isEmpty)
  }
}
