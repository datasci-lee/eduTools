import AppKit
import Testing

@testable import EduTools

enum PointerEventScenario: Sendable {
  case leftMouseDown
  case leftMouseUp
  case leftMouseDragged
  case rightMouseDown
  case rightMouseUp
  case rightMouseDragged
  case otherMouseDown
  case otherMouseUp
  case otherMouseDragged
  case mouseMoved
  case tabletProximity
  case mouseEntered
  case mouseExited
  case cursorUpdate

  var eventType: NSEvent.EventType {
    switch self {
    case .leftMouseDown: return .leftMouseDown
    case .leftMouseUp: return .leftMouseUp
    case .leftMouseDragged: return .leftMouseDragged
    case .rightMouseDown: return .rightMouseDown
    case .rightMouseUp: return .rightMouseUp
    case .rightMouseDragged: return .rightMouseDragged
    case .otherMouseDown: return .otherMouseDown
    case .otherMouseUp: return .otherMouseUp
    case .otherMouseDragged: return .otherMouseDragged
    case .mouseMoved: return .mouseMoved
    case .tabletProximity: return .tabletProximity
    case .mouseEntered: return .mouseEntered
    case .mouseExited: return .mouseExited
    case .cursorUpdate: return .cursorUpdate
    }
  }
}

struct PointerEventExpectation: Sendable {
  let scenario: PointerEventScenario
  let canQueryPointingDevice: Bool
}

@Suite("포인터 이벤트")
struct PointerEventsTests {
  @Test(
    "이벤트 종류 표는 장치 종류를 안전하게 조회할 수 있는 이벤트만 허용한다",
    arguments: [
      PointerEventExpectation(scenario: .leftMouseDown, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .leftMouseUp, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .leftMouseDragged, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .rightMouseDown, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .rightMouseUp, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .rightMouseDragged, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .otherMouseDown, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .otherMouseUp, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .otherMouseDragged, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .mouseMoved, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .tabletProximity, canQueryPointingDevice: true),
      PointerEventExpectation(scenario: .mouseEntered, canQueryPointingDevice: false),
      PointerEventExpectation(scenario: .mouseExited, canQueryPointingDevice: false),
      PointerEventExpectation(scenario: .cursorUpdate, canQueryPointingDevice: false),
    ]
  )
  func pointingDeviceQueryPermissionMatchesEventKind(_ expectation: PointerEventExpectation) {
    #expect(
      PointerEventKind.canQueryPointingDevice(expectation.scenario.eventType)
        == expectation.canQueryPointingDevice
    )
  }
}
