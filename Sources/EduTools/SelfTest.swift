import Foundation
import Carbon

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

    let pen = Shortcut(serialized: "AppMod+P")
    runner.expect(pen != nil, "AppMod+P should parse")
    runner.expect(pen?.serialized == "AppMod+P", "AppMod+P should serialize unchanged")
    runner.expect(pen?.displayString == "⌃⌥P", "AppMod+P should use the macOS display form")
    runner.expect(pen?.keyCode == 35, "P should use physical keyCode 35")
    runner.expect(
        pen?.keyCode == UInt32(kVK_ANSI_P),
        "P should use the ANSI physical key constant"
    )
    runner.expect(
        pen?.carbonModifiers == Shortcut.appModifierCarbonFlags,
        "AppMod should map to Control+Option Carbon flags"
    )

    if let pen {
        let reconstructed = Shortcut(
            keyCode: pen.keyCode,
            carbonModifiers: pen.carbonModifiers
        )
        runner.expect(
            reconstructed?.serialized == "AppMod+P",
            "P should round-trip from Carbon values"
        )
    } else {
        runner.expect(false, "P Carbon round-trip could not run")
    }

    let help = Shortcut(serialized: "AppMod+/")
    runner.expect(help != nil, "AppMod+/ should parse")
    runner.expect(help?.serialized == "AppMod+/", "AppMod+/ should serialize unchanged")
    runner.expect(help?.keyCode == 44, "/ should use physical keyCode 44")

    if let help {
        let reconstructed = Shortcut(
            keyCode: help.keyCode,
            carbonModifiers: help.carbonModifiers
        )
        runner.expect(
            reconstructed?.serialized == "AppMod+/",
            "/ should round-trip from Carbon values"
        )
    } else {
        runner.expect(false, "/ Carbon round-trip could not run")
    }

    let invalidValues = [
        "",
        "P",
        "AppMod+",
        "AppMod+p",
        "AppMod+PP",
        "AppMod+P+Z",
        "Control+Option+P",
    ]
    for value in invalidValues {
        runner.expect(Shortcut(serialized: value) == nil, "\(value) should be rejected")
    }
    runner.expect(
        Shortcut(keyCode: UInt32(kVK_ANSI_P), carbonModifiers: 0) == nil,
        "P without AppMod should be rejected"
    )

    let catalogBindings = ShortcutCatalog.definitions.flatMap(\.bindings)
    let bindingKeys = catalogBindings.map(\.uniquenessKey)
    runner.expect(
        Set(bindingKeys).count == bindingKeys.count,
        "shortcut catalog should not contain duplicate keys in the same scope"
    )

    let annotationDefinitions = ShortcutCatalog.definitions.filter {
        $0.binding(in: .annotation) != nil
    }
    runner.expect(
        annotationDefinitions.allSatisfy { AppCommandDispatcher.supports($0.command) },
        "every annotation shortcut should be connected to the command dispatcher"
    )

    let koreanFilterResults = ShortcutCatalog.filtered(
        ShortcutCatalog.definitions,
        query: "형광펜"
    )
    runner.expect(
        koreanFilterResults.map(\.id) == ["tool.highlighter"],
        "help filtering should match a Korean feature name"
    )

    let keyFilterResults = ShortcutCatalog.filtered(
        ShortcutCatalog.definitions,
        query: "["
    )
    runner.expect(
        keyFilterResults.map(\.id) == ["tool.width.decrease"],
        "help filtering should match a displayed shortcut key"
    )

    let screenSize = CGSize(width: 1_920, height: 1_080)
    let screenPoint = CGPoint(x: 384, y: 810)
    let normalizedPoint = InkGeometry.normalizedPoint(from: screenPoint, in: screenSize)
    let roundTrippedPoint = InkGeometry.screenPoint(from: normalizedPoint, in: screenSize)
    runner.expect(
        hypot(roundTrippedPoint.x - screenPoint.x, roundTrippedPoint.y - screenPoint.y) < 0.000_001,
        "normalized and screen coordinates should round-trip"
    )

    let zoomStepAndClampPassed = ZoomGeometry.adjustedMagnification(2, by: 1) == 2.25
        && ZoomGeometry.adjustedMagnification(1.25, by: -1) == 1.25
        && ZoomGeometry.adjustedMagnification(8, by: 1) == 8
    runner.expect(
        zoomStepAndClampPassed,
        "zoom magnification should move by 0.25 and stay within 1.25 through 8"
    )

    let zoomScreenSize = CGSize(width: 1_000, height: 800)
    let edgeFollowCenter = CGPoint(x: 500, y: 400)
    let centerPointerResult = ZoomGeometry.followedCenter(
        current: edgeFollowCenter,
        pointer: CGPoint(x: 500, y: 400),
        screenSize: zoomScreenSize,
        magnification: 2,
        mode: .edge,
        smoothness: 1
    )
    let edgePointerResult = ZoomGeometry.followedCenter(
        current: edgeFollowCenter,
        pointer: CGPoint(x: 740, y: 400),
        screenSize: zoomScreenSize,
        magnification: 2,
        mode: .edge,
        smoothness: 1
    )
    runner.expect(
        centerPointerResult == edgeFollowCenter && edgePointerResult == CGPoint(x: 540, y: 400),
        "edge following should stay still in the center and move only past the inner edge"
    )

    runner.expect(
        ZoomGeometry.clampedCenter(
            CGPoint(x: 1, y: 799),
            screenSize: zoomScreenSize,
            magnification: 2
        ) == CGPoint(x: 250, y: 600),
        "zoom center should clamp the viewport to every screen edge"
    )

    let frozenOriginalPoint = ZoomGeometry.originalPoint(
        fromZoomed: CGPoint(x: 250, y: 600),
        in: CGSize(width: 1_000, height: 800),
        sourceRect: CGRect(x: 250, y: 200, width: 500, height: 400)
    )
    let frozenNormalizedPoint = InkGeometry.normalizedPoint(
        from: frozenOriginalPoint,
        in: zoomScreenSize
    )
    runner.expect(
        frozenNormalizedPoint == CGPoint(x: 0.375, y: 0.625),
        "frozen zoom input should map back to the original screen coordinate"
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
