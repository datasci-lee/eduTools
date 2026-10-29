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

    let filterPen = InkStroke(
        tool: .pen,
        color: .systemBlue,
        width: 3,
        usesPressure: false,
        points: [InkPoint(location: CGPoint(x: 0.5, y: 0.5), pressure: 1, timestamp: 0)]
    )
    let filterHighlighter = InkStroke(
        tool: .highlighter,
        color: .systemYellow,
        width: 18,
        usesPressure: false,
        points: [InkPoint(location: CGPoint(x: 0.5, y: 0.5), pressure: 1, timestamp: 0)]
    )
    let archiveFirstStroke = InkStroke(
        tool: .pen,
        color: NSColor(deviceRed: 0.2, green: 0.4, blue: 0.8, alpha: 1),
        width: 3,
        usesPressure: true,
        points: [
            InkPoint(location: CGPoint(x: 0.25, y: 0.75), pressure: 0.6, timestamp: 1),
            InkPoint(location: CGPoint(x: 0.5, y: 0.5), pressure: 0.9, timestamp: 2),
        ]
    )
    let archiveSecondStroke = InkStroke(
        tool: .highlighter,
        color: NSColor(deviceRed: 0.9, green: 0.7, blue: 0.1, alpha: 1),
        width: 18,
        usesPressure: false,
        points: [InkPoint(location: CGPoint(x: 0.8, y: 0.2), pressure: 1, timestamp: 3)]
    )
    let movePolicyState = AppState()
    movePolicyState.addStroke(filterPen, to: "primary-display")
    movePolicyState.addStroke(archiveFirstStroke, to: "removed-display")
    movePolicyState.addStroke(archiveSecondStroke, to: "removed-display")
    let movePolicyMerged = movePolicyState.moveScreenInk(
        from: "removed-display",
        to: "primary-display"
    )
    runner.expect(
        movePolicyMerged
            && movePolicyState.screenStrokes(for: "removed-display").isEmpty
            && movePolicyState.screenStrokes(for: "primary-display").map(\.identifier)
                == [
                    filterPen.identifier,
                    archiveFirstStroke.identifier,
                    archiveSecondStroke.identifier,
                ],
        "move-to-primary disconnect policy should merge source ink after destination ink"
    )

    let clearState = AppState()
    clearState.addStroke(filterPen, to: "display-a")
    clearState.addStroke(filterHighlighter, to: "display-b")
    let clearedAll = clearState.clearAllInk()
    let allDisplaysWereCleared = clearState.strokes(for: "display-a").isEmpty
        && clearState.strokes(for: "display-b").isEmpty
    let restoredAll = clearState.undo(on: "display-a")
        && clearState.strokes(for: "display-a").map(\.identifier) == [filterPen.identifier]
        && clearState.strokes(for: "display-b").map(\.identifier) == [filterHighlighter.identifier]
    runner.expect(
        clearedAll && allDisplaysWereCleared && restoredAll,
        "clear all should restore every display with one undo"
    )

    let transitionState = AppState()
    transitionState.addStroke(filterPen, to: "transition-display")
    transitionState.toggleBoard(.white, on: "transition-display")
    transitionState.addStroke(filterHighlighter, to: "transition-display")
    let boardWasVisible = transitionState.strokes(for: "transition-display").map(\.identifier)
        == [filterHighlighter.identifier]
    transitionState.toggleBoard(.white, on: "transition-display")
    let screenWasRestored = transitionState.strokes(for: "transition-display").map(\.identifier)
        == [filterPen.identifier]
    transitionState.toggleBoard(.white, on: "transition-display")
    let boardWasRestored = transitionState.strokes(for: "transition-display").map(\.identifier)
        == [filterHighlighter.identifier]
    runner.expect(
        boardWasVisible && screenWasRestored && boardWasRestored,
        "screen and board strokes should survive repeated board transitions"
    )

    let pageState = AppState()
    pageState.toggleBoard(.white, on: "page-display")
    pageState.addStroke(filterPen, to: "page-display")
    let createdSecondPage = pageState.moveToNextBoardPage(on: "page-display")
    pageState.addStroke(filterHighlighter, to: "page-display")
    let undidSecondPage = pageState.undo(on: "page-display")
        && pageState.strokes(for: "page-display").isEmpty
    let returnedToFirstPage = pageState.moveToPreviousBoardPage(on: "page-display")
        && pageState.strokes(for: "page-display").map(\.identifier) == [filterPen.identifier]
    let undidFirstPage = pageState.undo(on: "page-display")
        && pageState.strokes(for: "page-display").isEmpty
    let secondPageRemainedIndependent = pageState.moveToNextBoardPage(on: "page-display")
        && pageState.strokes(for: "page-display").isEmpty
        && pageState.boardPagePosition(on: "page-display") == BoardPagePosition(current: 2, total: 2)
    runner.expect(
        createdSecondPage
            && undidSecondPage
            && returnedToFirstPage
            && undidFirstPage
            && secondPageRemainedIndependent,
        "each board page should keep an independent undo history"
    )

    let backupDefaults = UserDefaults(suiteName: "EduTools.SelfTest.\(UUID().uuidString)")!
    let backupStore = SettingsBackupStore(defaults: backupDefaults)
    for value in 1...6 {
        var settings = AppSettings.default
        settings.timer.lessonDefaultMinutes = value
        try? backupStore.append(settings)
    }
    let latestBackup = try? backupStore.popLatest()
    runner.expect(
        backupStore.count == 4 && latestBackup?.timer.lessonDefaultMinutes == 6,
        "settings backups should rotate to the most recent five entries"
    )

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
