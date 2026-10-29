import Foundation
import AppKit

private struct SelfTestRunner {
    private(set) var passed = 0
    private(set) var failures: [String] = []

    mutating func expect(_ condition: @autoclosure () -> Bool, _ message: String) {
        if condition() {
            passed += 1
        } else {
            failures.append(message)
        }
    }
}

func runSelfTests() -> Int32 {
    var runner = SelfTestRunner()

    let mouseEntered = NSEvent.enterExitEvent(
        with: .mouseEntered,
        location: .zero,
        modifierFlags: [],
        timestamp: 0,
        windowNumber: 0,
        context: nil,
        eventNumber: 0,
        trackingNumber: 0,
        userData: nil
    )!
    let mouseExited = NSEvent.enterExitEvent(
        with: .mouseExited,
        location: .zero,
        modifierFlags: [],
        timestamp: 0,
        windowNumber: 0,
        context: nil,
        eventNumber: 0,
        trackingNumber: 0,
        userData: nil
    )!
    // Mouse-event reads require a WindowServer session (and throw headlessly), so verify only guarded enter/exit reads and the event-kind table here.
    runner.expect(
        !PointerEventKind.isEraserTip(mouseEntered)
            && !PointerEventKind.isEraserTip(mouseExited)
            && !PointerEventKind.canQueryPointingDevice(.mouseEntered)
            && !PointerEventKind.canQueryPointingDevice(.mouseExited)
            && PointerEventKind.canQueryPointingDevice(.mouseMoved)
            && PointerEventKind.canQueryPointingDevice(.leftMouseDown)
            && PointerEventKind.canQueryPointingDevice(.rightMouseDown)
            && PointerEventKind.canQueryPointingDevice(.tabletProximity)
            && !PointerEventKind.canQueryPointingDevice(.cursorUpdate),
        "synthetic enter and exit events should guard device queries and the event-kind table should remain complete"
    )

    if runner.failures.isEmpty {
        print("SelfTest: \(runner.passed) passed, 0 failed")
        return 0
    }

    for failure in runner.failures {
        FileHandle.standardError.write(Data("SelfTest failure: \(failure)\n".utf8))
    }
    FileHandle.standardError.write(
        Data("SelfTest: \(runner.passed) passed, \(runner.failures.count) failed\n".utf8)
    )
    return 1
}
