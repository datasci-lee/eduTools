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

    let curvePoints = [
        InkPoint(location: CGPoint(x: 0.1, y: 0.2), pressure: 1, timestamp: 0),
        InkPoint(location: CGPoint(x: 0.35, y: 0.8), pressure: 1, timestamp: 1),
        InkPoint(location: CGPoint(x: 0.7, y: 0.3), pressure: 1, timestamp: 2),
        InkPoint(location: CGPoint(x: 0.9, y: 0.6), pressure: 1, timestamp: 3),
    ]
    let smoothedCurve = InkGeometry.smoothedPoints(curvePoints)
    let curvePassesOriginalPoints = curvePoints.allSatisfy { original in
        smoothedCurve.contains { sample in
            hypot(
                sample.location.x - original.location.x,
                sample.location.y - original.location.y
            ) < 0.000_001
        }
    }
    runner.expect(curvePassesOriginalPoints, "Catmull-Rom smoothing should pass through source points")

    let penStart = CGPoint(x: 100, y: 100)
    let penProposals = [
        CGPoint(x: 220, y: 112),
        CGPoint(x: 205, y: 198),
        CGPoint(x: 112, y: 220),
    ]
    let expectedPenAngles = [CGFloat(0), CGFloat.pi / 4, CGFloat.pi / 2]
    let penSnapsToThreeDirections = zip(penProposals, expectedPenAngles).allSatisfy {
        proposal, expectedAngle in
        let endpoint = InkGeometry.snappedPenEndpoint(from: penStart, to: proposal)
        let actualAngle = atan2(endpoint.y - penStart.y, endpoint.x - penStart.x)
        return abs(actualAngle - expectedAngle) < 0.000_001
    }
    runner.expect(penSnapsToThreeDirections, "pen Shift snapping should select 0, 45, and 90 degrees")

    let highlighterEndpoint = InkGeometry.snappedHighlighterEndpoint(
        from: CGPoint(x: 90, y: 140),
        to: CGPoint(x: 260, y: 310)
    )
    runner.expect(
        highlighterEndpoint == CGPoint(x: 260, y: 140),
        "highlighter Shift snapping should stay horizontal"
    )

    let recognizedLine = ShapeRecognizer.recognize(
        points: [
            CGPoint(x: 0, y: 0),
            CGPoint(x: 25, y: 1),
            CGPoint(x: 50, y: -1),
            CGPoint(x: 75, y: 1),
            CGPoint(x: 100, y: 0),
        ]
    )
    runner.expect(recognizedLine == .line, "a nearly straight hand stroke should be recognized")

    let recognizedRectangle = ShapeRecognizer.recognize(
        points: [
            CGPoint(x: 0, y: 0),
            CGPoint(x: 50, y: 1),
            CGPoint(x: 100, y: 0),
            CGPoint(x: 99, y: 40),
            CGPoint(x: 100, y: 80),
            CGPoint(x: 50, y: 79),
            CGPoint(x: 0, y: 80),
            CGPoint(x: 1, y: 40),
            CGPoint(x: 0, y: 0),
        ]
    )
    runner.expect(
        recognizedRectangle == .rectangle,
        "a closed four-sided hand stroke should be recognized as a rectangle"
    )

    let circlePoints = (0...24).map { index -> CGPoint in
        let angle = CGFloat(index) / 24 * 2 * .pi
        return CGPoint(x: 50 + cos(angle) * 40, y: 50 + sin(angle) * 40)
    }
    runner.expect(
        ShapeRecognizer.recognize(points: circlePoints) == .ellipse,
        "a closed round hand stroke should be recognized as an ellipse"
    )

    let rejectedScribble = ShapeRecognizer.recognize(
        points: [
            CGPoint(x: 0, y: 0),
            CGPoint(x: 80, y: 80),
            CGPoint(x: 10, y: 70),
            CGPoint(x: 90, y: 10),
            CGPoint(x: 40, y: 60),
        ]
    )
    runner.expect(rejectedScribble == nil, "an open zigzag should not be recognized as a shape")

    let concaveLasso = [
        CGPoint(x: 0, y: 0),
        CGPoint(x: 100, y: 0),
        CGPoint(x: 100, y: 100),
        CGPoint(x: 50, y: 45),
        CGPoint(x: 0, y: 100),
    ]
    runner.expect(
        LassoGeometry.contains(CGPoint(x: 20, y: 30), polygon: concaveLasso)
            && !LassoGeometry.contains(CGPoint(x: 50, y: 70), polygon: concaveLasso),
        "lasso containment should respect a concave polygon boundary"
    )

    let selectionStroke = InkStroke(
        tool: .shape,
        color: .systemBlue,
        width: 3,
        usesPressure: false,
        points: [
            InkPoint(location: CGPoint(x: 0.2, y: 0.2), pressure: 1, timestamp: 0),
            InkPoint(location: CGPoint(x: 0.4, y: 0.4), pressure: 1, timestamp: 1),
        ],
        content: .shape(.rectangle)
    )
    let movedSelection = SelectionTransform.moved(
        [selectionStroke],
        by: CGPoint(x: 0.1, y: -0.05)
    )
    let movedCoordinates = movedSelection.first?.points.map(\.location) ?? []
    runner.expect(
        movedCoordinates.count == 2
            && hypot(movedCoordinates[0].x - 0.3, movedCoordinates[0].y - 0.15) < 0.000_001
            && hypot(movedCoordinates[1].x - 0.5, movedCoordinates[1].y - 0.35) < 0.000_001,
        "selection movement should offset every normalized coordinate"
    )

    let lowerCover = InkStroke(
        tool: .cover,
        color: .systemGray,
        width: 1,
        usesPressure: false,
        points: [
            InkPoint(location: CGPoint(x: 0.1, y: 0.1), pressure: 1, timestamp: 0),
            InkPoint(location: CGPoint(x: 0.6, y: 0.6), pressure: 1, timestamp: 1),
        ],
        content: .cover
    )
    let upperCover = InkStroke(
        tool: .cover,
        color: .systemGray,
        width: 1,
        usesPressure: false,
        points: [
            InkPoint(location: CGPoint(x: 0.3, y: 0.3), pressure: 1, timestamp: 0),
            InkPoint(location: CGPoint(x: 0.8, y: 0.8), pressure: 1, timestamp: 1),
        ],
        content: .cover
    )
    let coverClickResult = ScreenCoverEngine.removingTopmostCover(
        at: CGPoint(x: 0.4, y: 0.4),
        from: [selectionStroke, lowerCover, upperCover]
    )
    runner.expect(
        coverClickResult.map(\.identifier) == [selectionStroke.identifier, lowerCover.identifier],
        "clicking overlapping covers should remove only the topmost matching cover"
    )

    let resizedSelection = SelectionTransform.resized(
        [selectionStroke],
        from: CGRect(x: 0.2, y: 0.2, width: 0.2, height: 0.2),
        to: CGRect(x: 0.1, y: 0.1, width: 0.6, height: 0.4)
    )
    let resizedCoordinates = resizedSelection.first?.points.map(\.location) ?? []
    runner.expect(
        resizedCoordinates.count == 2
            && hypot(resizedCoordinates[0].x - 0.1, resizedCoordinates[0].y - 0.1) < 0.000_001
            && hypot(resizedCoordinates[1].x - 0.7, resizedCoordinates[1].y - 0.5) < 0.000_001,
        "selection resizing should map normalized coordinates into the new bounds"
    )

    let undoLayer = InkLayerState()
    for index in 0..<101 {
        undoLayer.execute(
            .addStroke(
                InkStroke(
                    tool: .pen,
                    color: .systemRed,
                    width: 3,
                    usesPressure: false,
                    points: [
                        InkPoint(
                            location: CGPoint(x: CGFloat(index) / 101, y: 0.5),
                            pressure: 1,
                            timestamp: TimeInterval(index)
                        ),
                    ]
                )
            )
        )
    }
    let completedOneHundredUndos = (0..<100).allSatisfy { _ in undoLayer.undo() }
    runner.expect(
        completedOneHundredUndos && undoLayer.strokes.count == 1,
        "101 recorded strokes should permit 100 undos"
    )

    let redoLayer = InkLayerState()
    let redoStroke = InkStroke(
        tool: .pen,
        color: .systemRed,
        width: 3,
        usesPressure: false,
        points: []
    )
    redoLayer.execute(.addStroke(redoStroke))
    _ = redoLayer.undo()
    redoLayer.execute(
        .addStroke(
            InkStroke(
                tool: .pen,
                color: .systemBlue,
                width: 3,
                usesPressure: false,
                points: []
            )
        )
    )
    runner.expect(!redoLayer.canRedo && !redoLayer.redo(), "a new stroke should clear redo history")

    let eraserCanvas = CGSize(width: 1_000, height: 100)
    let splitStroke = InkStroke(
        tool: .pen,
        color: .systemRed,
        width: 2,
        usesPressure: false,
        points: [
            InkPoint(location: CGPoint(x: 0.1, y: 0.5), pressure: 1, timestamp: 0),
            InkPoint(location: CGPoint(x: 0.9, y: 0.5), pressure: 1, timestamp: 1),
        ]
    )
    let splitResult = EraserEngine.erase(
        strokes: [splitStroke],
        gesture: .sweep(points: [CGPoint(x: 500, y: 50)]),
        settings: EraserSettings(
            diameter: 40,
            mode: .partial,
            hitRule: .touching,
            targetFilter: .all
        ),
        canvasSize: eraserCanvas
    )
    runner.expect(
        splitResult.count == 2
            && splitResult[0].points.allSatisfy { $0.location.x < 0.5 }
            && splitResult[1].points.allSatisfy { $0.location.x > 0.5 },
        "partial erasing through a stroke center should leave two end fragments"
    )

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
    let filteredResult = EraserEngine.erase(
        strokes: [filterPen, filterHighlighter],
        gesture: .sweep(points: [CGPoint(x: 500, y: 50)]),
        settings: EraserSettings(
            diameter: 24,
            mode: .stroke,
            hitRule: .touching,
            targetFilter: .highlighterOnly
        ),
        canvasSize: eraserCanvas
    )
    runner.expect(
        filteredResult.map(\.identifier) == [filterPen.identifier],
        "highlighter-only stroke erasing should preserve a touched pen stroke"
    )

    let archiveDisplayUUID = "archive-display"
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
    let archiveLayer = InkLayerState()
    archiveLayer.execute(
        .addDisplayStroke(displayUUID: archiveDisplayUUID, stroke: archiveFirstStroke)
    )
    archiveLayer.execute(
        .addDisplayStroke(displayUUID: archiveDisplayUUID, stroke: archiveSecondStroke)
    )
    _ = archiveLayer.undo()
    let archiveData = try? JSONEncoder().encode(archiveLayer.state(for: archiveDisplayUUID))
    let decodedArchiveLayer = archiveData.flatMap {
        try? JSONDecoder().decode(InkLayerState.self, from: $0)
    }
    let archiveRoundTripEquivalent = decodedArchiveLayer.map { decoded in
        let initialEquivalent = decoded.strokes(for: archiveDisplayUUID) == [archiveFirstStroke]
        let redoEquivalent = decoded.redo()
            && decoded.strokes(for: archiveDisplayUUID) == [archiveFirstStroke, archiveSecondStroke]
        let undoEquivalent = decoded.undo()
            && decoded.strokes(for: archiveDisplayUUID) == [archiveFirstStroke]
        return initialEquivalent && redoEquivalent && undoEquivalent
    } ?? false
    runner.expect(
        archiveRoundTripEquivalent,
        "display ink and its undo history should survive a JSON round-trip"
    )

    let originalResolution = CGSize(width: 1_000, height: 500)
    let restoredResolution = CGSize(width: 2_000, height: 1_000)
    let restoredNormalizedPoint = decodedArchiveLayer?
        .strokes(for: archiveDisplayUUID)
        .first?
        .points
        .first?
        .location
    let resolutionRestorePreservedRatio = restoredNormalizedPoint.map { point in
        let originalPoint = InkGeometry.screenPoint(from: point, in: originalResolution)
        let restoredPoint = InkGeometry.screenPoint(from: point, in: restoredResolution)
        return originalPoint == CGPoint(x: 250, y: 375)
            && restoredPoint == CGPoint(x: 500, y: 750)
            && originalPoint.x / originalResolution.width
                == restoredPoint.x / restoredResolution.width
            && originalPoint.y / originalResolution.height
                == restoredPoint.y / restoredResolution.height
    } ?? false
    runner.expect(
        resolutionRestorePreservedRatio,
        "restoring on a different resolution should preserve normalized stroke coordinates"
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

    let areaCanvas = CGSize(width: 100, height: 100)
    let insideAreaStroke = InkStroke(
        tool: .pen,
        color: .systemRed,
        width: 2,
        usesPressure: false,
        points: [
            InkPoint(location: CGPoint(x: 0.3, y: 0.5), pressure: 1, timestamp: 0),
            InkPoint(location: CGPoint(x: 0.4, y: 0.5), pressure: 1, timestamp: 1),
        ]
    )
    let crossingAreaStroke = InkStroke(
        tool: .pen,
        color: .systemBlue,
        width: 2,
        usesPressure: false,
        points: [
            InkPoint(location: CGPoint(x: 0.1, y: 0.5), pressure: 1, timestamp: 0),
            InkPoint(location: CGPoint(x: 0.5, y: 0.5), pressure: 1, timestamp: 1),
        ]
    )
    let areaGesture = EraserGesture.area(CGRect(x: 20, y: 20, width: 30, height: 60))
    let touchingAreaResult = EraserEngine.erase(
        strokes: [insideAreaStroke, crossingAreaStroke],
        gesture: areaGesture,
        settings: EraserSettings(
            diameter: 24,
            mode: .area,
            hitRule: .touching,
            targetFilter: .all
        ),
        canvasSize: areaCanvas
    )
    let containedAreaResult = EraserEngine.erase(
        strokes: [insideAreaStroke, crossingAreaStroke],
        gesture: areaGesture,
        settings: EraserSettings(
            diameter: 24,
            mode: .area,
            hitRule: .contained,
            targetFilter: .all
        ),
        canvasSize: areaCanvas
    )
    runner.expect(
        touchingAreaResult.isEmpty
            && containedAreaResult.map(\.identifier) == [crossingAreaStroke.identifier],
        "area erasing should distinguish touching strokes from fully contained strokes"
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

    let blackStroke = InkStroke(
        tool: .pen,
        color: .black,
        width: 3,
        usesPressure: false,
        points: []
    )
    let displayedBlackStrokeColor = BoardAppearance.displayColor(
        for: blackStroke.color,
        on: .black
    ).usingColorSpace(.deviceRGB)
    let blackStrokeDisplaysWhite = displayedBlackStrokeColor.map {
        $0.redComponent >= 0.999
            && $0.greenComponent >= 0.999
            && $0.blueComponent >= 0.999
    } ?? false
    runner.expect(
        BoardAppearance.isBlack(blackStroke.color) && blackStrokeDisplaysWhite,
        "a stored black stroke should display as white on the blackboard"
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
