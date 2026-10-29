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

    runner.expect(
        AttrIconPure.widthLevel(value: 0, lowerBound: 1, upperBound: 12) == 1
            && AttrIconPure.widthLevel(value: 1, lowerBound: 1, upperBound: 12) == 1
            && AttrIconPure.widthLevel(value: 3.749, lowerBound: 1, upperBound: 12) == 1
            && AttrIconPure.widthLevel(value: 3.75, lowerBound: 1, upperBound: 12) == 2
            && AttrIconPure.widthLevel(value: 6.5, lowerBound: 1, upperBound: 12) == 3
            && AttrIconPure.widthLevel(value: 9.25, lowerBound: 1, upperBound: 12) == 4
            && AttrIconPure.widthLevel(value: 12, lowerBound: 1, upperBound: 12) == 4
            && AttrIconPure.widthLevel(value: 13, lowerBound: 1, upperBound: 12) == 4
            && AttrIconPure.widthLevel(value: 5, lowerBound: 5, upperBound: 5) == 2,
        "attribute icon width levels should clamp values and preserve every quartile boundary"
    )

    let unknownAttrIconConditionRejected: Bool
    do {
        _ = try AttrIconPure.conditionMatches("fill", attributes: [:])
        unknownAttrIconConditionRejected = false
    } catch let error as AttrIconPure.ConditionError {
        unknownAttrIconConditionRejected = error == .unknownAttribute("fill")
    } catch {
        unknownAttrIconConditionRejected = false
    }
    runner.expect(
        (try? AttrIconPure.conditionMatches("fill", attributes: ["fill": "true"])) == true
            && (try? AttrIconPure.conditionMatches("!trail", attributes: ["trail": "false"]))
                == true
            && (try? AttrIconPure.conditionMatches("trail", attributes: ["trail": "true"]))
                == true
            && (try? AttrIconPure.conditionMatches("mode=brush", attributes: ["mode": "brush"]))
                == true
            && (try? AttrIconPure.conditionMatches("mode=rect", attributes: ["mode": "rect"]))
                == true
            && (try? AttrIconPure.conditionMatches("mode=lasso", attributes: ["mode": "lasso"]))
                == true
            && (try? AttrIconPure.conditionMatches("mode!=brush", attributes: ["mode": "stroke"]))
                == true
            && (try? AttrIconPure.conditionMatches("fill", attributes: ["fill": "false"]))
                == false
            && (try? AttrIconPure.conditionMatches("!trail", attributes: ["trail": "true"]))
                == false
            && (try? AttrIconPure.conditionMatches("trail", attributes: ["trail": "false"]))
                == false
            && (try? AttrIconPure.conditionMatches("mode=brush", attributes: ["mode": "stroke"]))
                == false
            && (try? AttrIconPure.conditionMatches("mode!=brush", attributes: ["mode": "brush"]))
                == false
            && unknownAttrIconConditionRejected,
        "attribute icon conditions should match booleans and values while rejecting unknown attributes"
    )

    let attrIconWhite = AttrIconColor(red: 1, green: 1, blue: 1)
    let attrIconBlack = AttrIconColor(red: 0, green: 0, blue: 0)
    let attrIconLightGray = AttrIconColor(red: 0.93, green: 0.93, blue: 0.93)
    let attrIconBlackWhiteRatio = AttrIconPure.contrastRatio(attrIconBlack, attrIconWhite)
    runner.expect(
        abs(AttrIconPure.contrastRatio(attrIconWhite, attrIconWhite) - 1) < 0.000_001
            && AttrIconPure.needsHalo(foreground: attrIconWhite, background: attrIconWhite)
            && abs(AttrIconPure.contrastRatio(attrIconBlack, attrIconBlack) - 1) < 0.000_001
            && AttrIconPure.needsHalo(foreground: attrIconBlack, background: attrIconBlack)
            && abs(attrIconBlackWhiteRatio - 21) < 0.000_001
            && !AttrIconPure.needsHalo(foreground: attrIconBlack, background: attrIconWhite)
            && abs(AttrIconPure.contrastRatio(attrIconWhite, attrIconBlack)
                - attrIconBlackWhiteRatio) < 0.000_001
            && abs(AttrIconPure.contrastRatio(attrIconWhite, attrIconLightGray) - 1.169) < 0.001
            && AttrIconPure.needsHalo(foreground: attrIconWhite, background: attrIconLightGray)
            && abs(AttrIconPure.contrastRatio(attrIconBlack, attrIconLightGray) - 17.96) < 0.01
            && !AttrIconPure.needsHalo(foreground: attrIconBlack, background: attrIconLightGray),
        "attribute icon contrast should be symmetric and apply halos below the threshold"
    )

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

    let timerNow = Date(timeIntervalSinceReferenceDate: 1_000_000)
    let remainingEngine = LessonTimerEngine()
    remainingEngine.startCountdown(duration: 600, now: timerNow)
    runner.expect(
        remainingEngine.remaining(at: timerNow.addingTimeInterval(125)) == 475,
        "lesson remaining time should equal end date minus the injected current time"
    )

    let pausedEngine = LessonTimerEngine()
    pausedEngine.startCountdown(duration: 600, now: timerNow)
    pausedEngine.pause(now: timerNow.addingTimeInterval(100))
    let remainedFixed = pausedEngine.remaining(at: timerNow.addingTimeInterval(400)) == 500
    pausedEngine.resume(now: timerNow.addingTimeInterval(400))
    let continuedAfterResume = pausedEngine.remaining(at: timerNow.addingTimeInterval(450)) == 450
    runner.expect(
        remainedFixed && continuedAfterResume,
        "a paused lesson timer should stay fixed and continue from that value after resume"
    )

    runner.expect(
        BreakTimerEngine.endDate(addingMinutes: 10, to: timerNow)
            == timerNow.addingTimeInterval(600),
        "the +10 minute break button should calculate its end date from the current clock"
    )

    var timerCalendar = Calendar(identifier: .gregorian)
    timerCalendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let afterDirectTime = timerCalendar.date(
        from: DateComponents(year: 2026, month: 10, day: 28, hour: 10, minute: 51)
    )!
    runner.expect(
        BreakTimerEngine.endDate(
            hour: 10,
            minute: 50,
            now: afterDirectTime,
            calendar: timerCalendar
        ) == nil,
        "a direct break time earlier than now should be rejected as already passed today"
    )

    let snapFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let snapSize = CGSize(width: 10, height: 10)
    let insideSnap = TimerGeometry.snappedOrigin(
        proposed: CGPoint(x: 16, y: 40),
        windowSize: snapSize,
        visibleFrame: snapFrame,
        snappingDisabled: false
    )
    let outsideSnap = TimerGeometry.snappedOrigin(
        proposed: CGPoint(x: 17, y: 40),
        windowSize: snapSize,
        visibleFrame: snapFrame,
        snappingDisabled: false
    )
    runner.expect(
        insideSnap.x == 0 && outsideSnap.x == 17,
        "lesson badge snapping should distinguish positions inside and outside 16 pixels"
    )

    let playlistURLValidationPassed = BreakMusicURL.validated(
        "https://music.youtube.com/playlist?list=valid"
    ) != nil
        && BreakMusicURL.validated("https://music.youtube.com.evil.example/playlist") == nil
        && BreakMusicURL.validated("http://music.youtube.com/playlist") == nil
    runner.expect(
        playlistURLValidationPassed,
        "break music should accept only https://music.youtube.com/ playlist URLs"
    )

    let fadeOriginalVolume = Float32(0.8)
    let fadeNow = Date(timeIntervalSinceReferenceDate: 2_000_000)
    let volumeDuringFade = BreakMusicFade.volume(
        originalVolume: fadeOriginalVolume,
        now: fadeNow,
        endDate: fadeNow.addingTimeInterval(30),
        duration: 60
    )
    let volumeAfterEndDateExtension = BreakMusicFade.volume(
        originalVolume: fadeOriginalVolume,
        now: fadeNow,
        endDate: fadeNow.addingTimeInterval(120),
        duration: 60
    )
    runner.expect(
        abs(volumeDuringFade - 0.4) < 0.000_001
            && abs(volumeAfterEndDateExtension - fadeOriginalVolume) < 0.000_001,
        "break music fade should be linear and move when the break end date changes"
    )

    runner.expect(
        AppleScriptString.literal("https://music.youtube.com/a\\\"b\n")
            == "\"https://music.youtube.com/a\\\\\\\"b\\n\"",
        "break music should escape URL text before inserting it into AppleScript"
    )

    func displayFixture(
        id: UInt32,
        size: DisplaySize,
        kind: DisplayKind = .external
    ) -> (online: OnlineDisplay, monitor: PresetMonitor) {
        let online = OnlineDisplay(
            id: id,
            uuid: "selftest-display-\(id)",
            vendor: 1_000 + id,
            model: 2_000 + id,
            serial: 3_000 + id,
            name: "SelfTest Display \(id)",
            kind: kind,
            size: size,
            origin: .zero,
            currentMirrorMasterID: nil
        )
        return (online, PresetMonitor(identity: online.identity))
    }

    let displayOne = displayFixture(
        id: 1,
        size: DisplaySize(width: 2560, height: 1440)
    )
    let displayTwo = displayFixture(
        id: 2,
        size: DisplaySize(width: 1920, height: 1080)
    )
    let displayThree = displayFixture(
        id: 3,
        size: DisplaySize(width: 1280, height: 720)
    )
    let horizontalPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Horizontal",
            main: PresetScreen(monitors: [displayOne.monitor]),
            secondary1: PresetScreen(
                monitors: [displayTwo.monitor],
                cell: GridCell(x: -1, y: 0)
            ),
            secondary2: PresetScreen(
                monitors: [displayThree.monitor],
                cell: GridCell(x: 1, y: 0)
            )
        ),
        online: [displayOne.online, displayTwo.online, displayThree.online]
    )
    let verticalPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Vertical",
            main: PresetScreen(monitors: [displayOne.monitor]),
            secondary1: PresetScreen(
                monitors: [displayTwo.monitor],
                cell: GridCell(x: 0, y: -1)
            ),
            secondary2: PresetScreen(
                monitors: [displayThree.monitor],
                cell: GridCell(x: 0, y: 1)
            )
        ),
        online: [displayOne.online, displayTwo.online, displayThree.online]
    )
    let cornerPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Corner",
            main: PresetScreen(monitors: [displayOne.monitor]),
            secondary1: PresetScreen(
                monitors: [displayTwo.monitor],
                cell: GridCell(x: -1, y: 0)
            ),
            secondary2: PresetScreen(
                monitors: [displayThree.monitor],
                cell: GridCell(x: 0, y: 1)
            )
        ),
        online: [displayOne.online, displayTwo.online, displayThree.online]
    )
    let gapMain = displayFixture(
        id: 11,
        size: DisplaySize(width: 1920, height: 1080)
    )
    let gapBelow = displayFixture(
        id: 12,
        size: DisplaySize(width: 2560, height: 1080)
    )
    let gapRight = displayFixture(
        id: 13,
        size: DisplaySize(width: 1280, height: 720)
    )
    let gapRegressionPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Gap regression",
            main: PresetScreen(monitors: [gapMain.monitor]),
            secondary1: PresetScreen(
                monitors: [gapBelow.monitor],
                cell: GridCell(x: 0, y: 1)
            ),
            secondary2: PresetScreen(
                monitors: [gapRight.monitor],
                cell: GridCell(x: 1, y: 0)
            )
        ),
        online: [gapMain.online, gapBelow.online, gapRight.online]
    )
    runner.expect(
        horizontalPlan.originsByMaster[displayTwo.online.id]
            == DisplayPoint(x: -1920, y: 0)
            && horizontalPlan.originsByMaster[displayOne.online.id] == .zero
            && horizontalPlan.originsByMaster[displayThree.online.id]
                == DisplayPoint(x: 2560, y: 0)
            && verticalPlan.originsByMaster[displayTwo.online.id]
                == DisplayPoint(x: 0, y: -1080)
            && verticalPlan.originsByMaster[displayThree.online.id]
                == DisplayPoint(x: 0, y: 1440)
            && cornerPlan.originsByMaster[displayThree.online.id]
                == DisplayPoint(x: 0, y: 1440)
            && gapRegressionPlan.originsByMaster[gapRight.online.id]
                == DisplayPoint(x: 1920, y: 0),
        "display grid planning should place all tested layouts edge-to-edge from the origin"
    )

    let absentMain = displayFixture(
        id: 21,
        size: DisplaySize(width: 1920, height: 1080)
    )
    let connectedLeft = displayFixture(
        id: 22,
        size: DisplaySize(width: 2560, height: 1440)
    )
    let connectedRight = displayFixture(
        id: 23,
        size: DisplaySize(width: 1280, height: 720)
    )
    let missingMainPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Missing main",
            main: PresetScreen(monitors: [absentMain.monitor]),
            secondary1: PresetScreen(
                monitors: [connectedLeft.monitor],
                cell: GridCell(x: -1, y: 0)
            ),
            secondary2: PresetScreen(
                monitors: [connectedRight.monitor],
                cell: GridCell(x: 1, y: 0)
            )
        ),
        online: [connectedLeft.online, connectedRight.online]
    )
    let overlapMain = displayFixture(
        id: 24,
        size: DisplaySize(width: 1920, height: 1080)
    )
    let overlapRight = displayFixture(
        id: 25,
        size: DisplaySize(width: 1280, height: 1440)
    )
    let overlapBelow = displayFixture(
        id: 26,
        size: DisplaySize(width: 3000, height: 720)
    )
    let overlapPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Overlap",
            main: PresetScreen(monitors: [overlapMain.monitor]),
            secondary1: PresetScreen(
                monitors: [overlapRight.monitor],
                cell: GridCell(x: 1, y: 0)
            ),
            secondary2: PresetScreen(
                monitors: [overlapBelow.monitor],
                cell: GridCell(x: 0, y: 1)
            )
        ),
        online: [overlapMain.online, overlapRight.online, overlapBelow.online]
    )
    runner.expect(
        missingMainPlan.mainDisplayID == connectedLeft.online.id
            && missingMainPlan.originsByMaster[connectedLeft.online.id] == .zero
            && missingMainPlan.originsByMaster[connectedRight.online.id]
                == DisplayPoint(x: 2560, y: 0)
            && !overlapPlan.layoutIssues.isEmpty
            && !overlapPlan.isApplicable,
        "display planning should anchor the connected main display at zero and reject overlaps"
    )

    let mirrorPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Mirrored main",
            main: PresetScreen(
                monitors: [displayOne.monitor, displayTwo.monitor, displayThree.monitor]
            )
        ),
        online: [displayOne.online, displayTwo.online, displayThree.online]
    )
    let extendedMirrorPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Mirrored secondary",
            main: PresetScreen(monitors: [displayOne.monitor]),
            secondary1: PresetScreen(
                monitors: [displayTwo.monitor, displayThree.monitor],
                cell: GridCell(x: 1, y: 0)
            )
        ),
        online: [displayOne.online, displayTwo.online, displayThree.online]
    )
    let missingFirst = displayFixture(
        id: 31,
        size: DisplaySize(width: 1920, height: 1080)
    )
    let succeedingMaster = displayFixture(
        id: 32,
        size: DisplaySize(width: 1920, height: 1080)
    )
    let successionPlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Master succession",
            main: PresetScreen(monitors: [missingFirst.monitor, succeedingMaster.monitor])
        ),
        online: [succeedingMaster.online]
    )
    let outsideDisplay = displayFixture(
        id: 33,
        size: DisplaySize(width: 1024, height: 768),
        kind: .virtual
    )
    let outsidePlan = DisplayPlanner.plan(
        preset: DisplayPreset(
            name: "Outside display",
            main: PresetScreen(monitors: [displayOne.monitor])
        ),
        online: [displayOne.online, outsideDisplay.online]
    )
    let outsidePlacement = outsidePlan.outsidePlacements.first
    runner.expect(
        mirrorPlan.mainDisplayID == displayOne.online.id
            && mirrorPlan.mirrors == [
                DisplayMirrorPlan(slaveID: displayTwo.online.id, masterID: displayOne.online.id),
                DisplayMirrorPlan(slaveID: displayThree.online.id, masterID: displayOne.online.id),
            ]
            && extendedMirrorPlan.mirrors == [
                DisplayMirrorPlan(slaveID: displayThree.online.id, masterID: displayTwo.online.id),
            ]
            && extendedMirrorPlan.originsByMaster[displayTwo.online.id]
                == DisplayPoint(x: 2560, y: 0)
            && successionPlan.mainDisplayID == succeedingMaster.online.id
            && successionPlan.missingPresetMonitors == [missingFirst.monitor.identity]
            && outsidePlan.outsidePlacements.count == 1
            && outsidePlacement?.displayID == outsideDisplay.online.id
            && (outsidePlacement?.origin.x ?? -1) >= 2560,
        "display planning should preserve mirror, master succession, missing, and outside monitor rules"
    )

    let duplicateCellPreset = DisplayPreset(
        name: "Duplicate cell",
        main: PresetScreen(monitors: [displayOne.monitor]),
        secondary1: PresetScreen(monitors: [displayTwo.monitor])
    )
    let diagonalPreset = DisplayPreset(
        name: "Diagonal",
        main: PresetScreen(monitors: [displayOne.monitor]),
        secondary1: PresetScreen(
            monitors: [displayTwo.monitor],
            cell: GridCell(x: 1, y: 1)
        )
    )
    let connectedPreset = DisplayPreset(
        name: "Connected",
        main: PresetScreen(monitors: [displayOne.monitor]),
        secondary1: PresetScreen(
            monitors: [displayTwo.monitor],
            cell: GridCell(x: 1, y: 0)
        ),
        secondary2: PresetScreen(
            monitors: [displayThree.monitor],
            cell: GridCell(x: 1, y: 1)
        )
    )
    runner.expect(
        DisplayGrid.validationError(for: duplicateCellPreset) == .duplicateGridCell
            && DisplayGrid.validationError(for: diagonalPreset) == .disconnectedGrid
            && DisplayGrid.validationError(for: connectedPreset) == nil,
        "display grid validation should reject duplicate or diagonal cells and accept connected cells"
    )

    var autoMonitorOne = displayOne.monitor
    autoMonitorOne.autoMatch = true
    var autoMonitorTwo = displayTwo.monitor
    autoMonitorTwo.autoMatch = true
    let automaticPresetA = DisplayPreset(
        name: "Automatic A",
        main: PresetScreen(monitors: [autoMonitorOne, autoMonitorTwo])
    )
    let automaticPresetB = DisplayPreset(
        name: "Automatic B",
        main: PresetScreen(monitors: [autoMonitorOne, autoMonitorTwo])
    )
    let noAutomaticMonitors = DisplayPreset(
        name: "No automatic monitors",
        main: PresetScreen(monitors: [displayOne.monitor])
    )
    runner.expect(
        DisplayPlanner.presetForConnectedSet(
            connected: [displayOne.online, displayTwo.online],
            presets: [automaticPresetA, automaticPresetB]
        )?.id == automaticPresetA.id
            && DisplayPlanner.presetForConnectedSet(
                connected: [displayOne.online, displayTwo.online],
                presets: [automaticPresetB, automaticPresetA]
            )?.id == automaticPresetB.id
            && DisplayPlanner.presetForConnectedSet(
                connected: [displayOne.online, displayTwo.online, displayThree.online],
                presets: [automaticPresetA]
            ) == nil
            && DisplayPlanner.presetForConnectedSet(
                connected: [displayOne.online],
                presets: [automaticPresetA]
            ) == nil
            && DisplayPlanner.presetForConnectedSet(
                connected: [displayOne.online],
                presets: [noAutomaticMonitors]
            ) == nil,
        "automatic display preset selection should require an exact set and preserve list priority"
    )

    let hardwareFixture = displayFixture(
        id: 41,
        size: DisplaySize(width: 1920, height: 1080)
    )
    var hardwareMatchedOnline = hardwareFixture.online
    hardwareMatchedOnline.uuid = "different-hardware-uuid"
    let hardwareMatches = DisplayMatcher.matches(
        monitors: [hardwareFixture.monitor],
        online: [hardwareMatchedOnline]
    )
    let zeroSerialFixture = displayFixture(
        id: 42,
        size: DisplaySize(width: 1920, height: 1080)
    )
    var zeroSerialMonitor = zeroSerialFixture.monitor
    zeroSerialMonitor.identity.uuid = "preset-zero-serial"
    zeroSerialMonitor.identity.serial = 0
    var zeroSerialOnline = zeroSerialFixture.online
    zeroSerialOnline.uuid = "online-zero-serial"
    zeroSerialOnline.serial = 0
    let zeroSerialMatches = DisplayMatcher.matches(
        monitors: [zeroSerialMonitor],
        online: [zeroSerialOnline]
    )
    let duplicateOnlineMatches = DisplayMatcher.matches(
        monitors: [
            hardwareFixture.monitor,
            PresetMonitor(identity: hardwareFixture.monitor.identity),
        ],
        online: [hardwareFixture.online]
    )
    runner.expect(
        hardwareMatches.count == 1
            && hardwareMatches.first?.onlineIndex == 0
            && zeroSerialMatches.isEmpty
            && duplicateOnlineMatches.count == 1
            && Set(duplicateOnlineMatches.map(\.onlineIndex)).count
                == duplicateOnlineMatches.count,
        "display matching should use valid hardware identity without reusing an online display"
    )

    let settingsJSON = try? JSONEncoder().encode(AppSettings.default)
    let settingsRoundTrip = settingsJSON.flatMap {
        try? JSONDecoder().decode(AppSettings.self, from: $0)
    }
    runner.expect(
        settingsRoundTrip == AppSettings.default,
        "settings JSON should round-trip without changing values"
    )

    var outOfRangeSettings = AppSettings.default
    outOfRangeSettings.zoom.maximumMagnification = 9
    let rejectedOutOfRange: Bool
    do {
        try SettingsValidator.validate(outOfRangeSettings)
        rejectedOutOfRange = false
    } catch {
        rejectedOutOfRange = true
    }
    runner.expect(
        rejectedOutOfRange,
        "settings validation should reject a zoom value outside 1.25 through 8"
    )

    var compatibleSettingsDecoded = false
    if let settingsJSON,
       var object = try? JSONSerialization.jsonObject(with: settingsJSON) as? [String: Any]
    {
        object["futureSetting"] = ["enabled": true]
        object.removeValue(forKey: "display")
        if var zoom = object["zoom"] as? [String: Any] {
            zoom.removeValue(forKey: "previewDimming")
            object["zoom"] = zoom
        }
        if var tools = object["tools"] as? [String: Any] {
            let memory = AppSettings.default.tools.perToolMemory
            tools["penWidth"] = memory.pen.size
            tools["highlighterWidth"] = memory.highlighter.size
            tools["textFontSize"] = memory.text.size
            tools.removeValue(forKey: "perToolMemory")
            tools.removeValue(forKey: "paletteGlassOpacity")
            object["tools"] = tools
        }
        if let data = try? JSONSerialization.data(withJSONObject: object),
           let decoded = try? JSONDecoder().decode(AppSettings.self, from: data)
        {
            compatibleSettingsDecoded = decoded == AppSettings.default
        }
    }
    runner.expect(
        compatibleSettingsDecoded,
        "settings decoding should ignore unknown keys and default missing compatible values"
    )

    var computerSpecificSettings = AppSettings.default
    computerSpecificSettings.general.archiveFolderPath = "/Volumes/Classroom/Archive"
    computerSpecificSettings.display = DisplaySettings(
        autoSwitchEnabled: true,
        presets: [
            DisplayPreset(
                name: "교실",
                main: PresetScreen(
                    monitors: [
                        PresetMonitor(
                            identity: DisplayIdentity(
                                uuid: "display-test-uuid",
                                vendor: 1,
                                model: 2,
                                serial: 3,
                                name: "교실 모니터",
                                kind: .external
                            ),
                            autoMatch: true
                        )
                    ]
                )
            ),
        ]
    )
    let defaultExport = try? SettingsTransfer.exportDocument(settings: computerSpecificSettings)
    let defaultExportData = defaultExport.flatMap { try? SettingsTransfer.encode($0) }
    let exportedSettingsObject = defaultExportData.flatMap {
        try? JSONSerialization.jsonObject(with: $0) as? [String: Any]
    }
    let exportedPayloadObject = exportedSettingsObject?["payload"] as? [String: Any]
    let exportedDisplayObject = exportedPayloadObject?["display"] as? [String: Any]
    let importedDefaultExport = defaultExport.flatMap {
        try? SettingsTransfer.appliedSettings(
            current: .default,
            document: $0,
            selectedBundles: Set($0.includedBundles)
        )
    }
    runner.expect(
        defaultExport?.includesComputerSpecific == false
            && defaultExport?.payload.general?.archiveFolderPath == nil
            && defaultExport?.payload.display?.presets == computerSpecificSettings.display.presets
            && defaultExport?.includedBundles.contains(.display) == true
            && exportedDisplayObject?["autoSwitchEnabled"] == nil
            && importedDefaultExport?.display.presets == computerSpecificSettings.display.presets
            && importedDefaultExport?.display.autoSwitchEnabled == false,
        "settings export should include display presets but exclude computer-specific switches and paths"
    )

    runner.expect(
        ShortcutConflictDetector.hasAvoidedConflict("AppMod+D"),
        "known avoided global shortcuts should be reported as conflicts"
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
