import AppKit
import Testing

@testable import EduTools
import TestSupport

@MainActor
@Suite("InkView 입력 파이프라인", .serialized)
struct InkViewPipelineTests {
  @Test("펜 드래그는 기존 명령 스택에 스트로크 하나를 기록한다")
  func penDragRecordsOneStroke() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = InkViewFixture()

    try fixture.send(.leftMouseDown, at: CGPoint(x: 30, y: 40), timestamp: 1)
    try fixture.send(.leftMouseDragged, at: CGPoint(x: 90, y: 80), timestamp: 2)
    try fixture.send(.leftMouseUp, at: CGPoint(x: 150, y: 120), timestamp: 3)

    #expect(fixture.state.strokes(for: fixture.displayUUID).count == 1)
  }

  @Test("우클릭은 현재 도구를 바꾸지 않고 획을 임시 지우개로 지운다")
  func secondaryDragErasesWithoutChangingTool() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = InkViewFixture()
    fixture.state.setEraserMode(.stroke)
    fixture.state.setEraserDiameter(80)

    try fixture.send(.leftMouseDown, at: CGPoint(x: 30, y: 100), timestamp: 1)
    try fixture.send(.leftMouseDragged, at: CGPoint(x: 100, y: 100), timestamp: 2)
    try fixture.send(.leftMouseUp, at: CGPoint(x: 170, y: 100), timestamp: 3)
    try fixture.send(.rightMouseDown, at: CGPoint(x: 100, y: 100), timestamp: 4)
    try fixture.send(.rightMouseUp, at: CGPoint(x: 100, y: 100), timestamp: 5)

    #expect(fixture.state.strokes(for: fixture.displayUUID).isEmpty)
    #expect(fixture.state.currentTool == .pen)
  }

  @Test("동시 버튼은 무시하고 누락된 mouseUp 소유권은 다음 입력에서 복구한다")
  func buttonOwnershipIgnoresOverlapAndRecoversMissingRelease() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = InkViewFixture()

    try fixture.send(.leftMouseDown, at: CGPoint(x: 20, y: 30), timestamp: 1)
    fixture.mouseButtons.value = 1 << 0
    try fixture.send(.rightMouseDown, at: CGPoint(x: 20, y: 30), timestamp: 2)
    fixture.mouseButtons.value = 0
    try fixture.send(.leftMouseDown, at: CGPoint(x: 80, y: 60), timestamp: 3)
    try fixture.send(.leftMouseUp, at: CGPoint(x: 120, y: 80), timestamp: 4)

    let strokes = fixture.state.strokes(for: fixture.displayUUID)
    #expect(strokes.count == 2)
    #expect(strokes.allSatisfy { $0.tool == .pen })
  }

  @Test("라이브 identity와 프레젠테이션 역변환은 같은 입력을 서로 다른 원본 좌표로 기록한다")
  func liveZoomInputMappingUsesIdentityOrPresentationInverse() throws {
    requireWindowServer()
    _ = NSApplication.shared
    let fixture = InkViewFixture(size: CGSize(width: 200, height: 100))

    fixture.view.inputMappingMode = .identity
    try fixture.send(.leftMouseDown, at: CGPoint(x: 40, y: 20), timestamp: 1)
    try fixture.send(.leftMouseUp, at: CGPoint(x: 60, y: 30), timestamp: 2)

    fixture.view.setPresentationSourceRect(
      CGRect(x: 50, y: 25, width: 100, height: 50),
      animated: false
    )
    fixture.view.inputMappingMode = .throughPresentation
    try fixture.send(.leftMouseDown, at: CGPoint(x: 40, y: 20), timestamp: 3)
    try fixture.send(.leftMouseUp, at: CGPoint(x: 60, y: 30), timestamp: 4)

    let strokes = fixture.state.strokes(for: fixture.displayUUID)
    #expect(strokes.count == 2)
    #expect(strokes[0].points.first?.location == CGPoint(x: 0.2, y: 0.2))
    #expect(strokes[1].points.first?.location == CGPoint(x: 0.35, y: 0.35))
  }
}

@MainActor
private final class InkViewFixture {
  final class MouseButtons {
    var value = 0
  }

  let displayUUID = "gui-ink-view"
  let state: AppState
  let view: InkView
  let host: OffscreenWindowHost
  let mouseButtons = MouseButtons()

  private let defaults: UserDefaults
  private let suiteName: String

  init(size: CGSize = CGSize(width: 200, height: 200)) {
    suiteName = "EduTools.GUITests.InkView.\(UUID().uuidString)"
    defaults = UserDefaults(suiteName: suiteName)!
    let settingsStore = SettingsStore(defaults: defaults)
    state = AppState(defaults: defaults, settingsStore: settingsStore)
    let dispatcher = AppCommandDispatcher(state: state)
    view = InkView(
      frame: CGRect(origin: .zero, size: size),
      state: state,
      dispatcher: dispatcher,
      displayUUID: displayUUID,
      requestKeyFocus: {},
      pressedMouseButtons: { [mouseButtons] in mouseButtons.value }
    )
    host = OffscreenWindowHost(view: view, size: size)
    state.setShapeRecognitionEnabled(false)
    state.setAnnotationModeEnabled(true)
  }

  deinit {
    defaults.removePersistentDomain(forName: suiteName)
  }

  func send(
    _ type: NSEvent.EventType,
    at location: CGPoint,
    timestamp: TimeInterval
  ) throws {
    let event = try SyntheticEvents.mouseEvent(
      type: type,
      location: location,
      in: host.window,
      timestamp: timestamp,
      eventNumber: Int(timestamp)
    )
    switch type {
    case .leftMouseDown: view.mouseDown(with: event)
    case .leftMouseDragged: view.mouseDragged(with: event)
    case .leftMouseUp: view.mouseUp(with: event)
    case .rightMouseDown: view.rightMouseDown(with: event)
    case .rightMouseDragged: view.rightMouseDragged(with: event)
    case .rightMouseUp: view.rightMouseUp(with: event)
    default: Issue.record("지원하지 않는 합성 마우스 이벤트: \(type)")
    }
  }
}
