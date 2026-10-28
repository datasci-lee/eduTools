import AppKit

// Kept app-independent so an external harness can compile this file alone with synthetic events.
enum PointerEventKind {
    static func isEraserTip(_ event: NSEvent) -> Bool {
        canQueryPointingDevice(event.type) && event.pointingDeviceType == .eraser
    }

    static func canQueryPointingDevice(_ type: NSEvent.EventType) -> Bool {
        switch type {
        case .leftMouseDown,
             .leftMouseUp,
             .leftMouseDragged,
             .rightMouseDown,
             .rightMouseUp,
             .rightMouseDragged,
             .otherMouseDown,
             .otherMouseUp,
             .otherMouseDragged,
             .mouseMoved,
             .tabletProximity:
            return true
        default:
            return false
        }
    }
}
