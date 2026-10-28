import AppKit
import Carbon
import QuartzCore

private final class LaserRenderer {
    let layer = CALayer()
    var color = NSColor.systemRed {
        didSet {
            layer.shadowColor = color.cgColor
            trailLayer.strokeColor = color.cgColor
        }
    }
    var radius: CGFloat = 8 {
        didSet {
            layer.shadowRadius = radius
            trailLayer.lineWidth = trailWidth
        }
    }
    var trailDuration: TimeInterval = 1

    /// The trail is one stroked path whose oldest points retract after `trailDuration`. One path has
    /// no joints, so nothing darkens where pieces would overlap, and round caps close both ends.
    private let trailLayer = CAShapeLayer()
    private var trails: [[(location: CGPoint, time: TimeInterval)]] = []
    private var isTrailing = false
    private var retractTimer: Timer?
    private var currentDot: CAShapeLayer?

    private var trailWidth: CGFloat { max(4, radius * 1.25) }

    init() {
        layer.shadowColor = color.cgColor
        layer.shadowOpacity = 0.9
        layer.shadowRadius = radius
        layer.shadowOffset = .zero
        trailLayer.fillColor = nil
        trailLayer.strokeColor = color.cgColor
        trailLayer.lineWidth = trailWidth
        trailLayer.lineCap = .round
        trailLayer.lineJoin = .round
        trailLayer.opacity = 0.7
        layer.addSublayer(trailLayer)
    }

    /// Hovering shows only the dot; a trail is left only while pressed (mouse button or pen contact).
    func move(to location: CGPoint, leavesTrail: Bool) {
        if leavesTrail {
            if !isTrailing {
                trails.append([])
                isTrailing = true
            }
            trails[trails.count - 1].append((location, ProcessInfo.processInfo.systemUptime))
            startRetracting()
            redrawTrail()
        } else {
            isTrailing = false
        }

        let dot = currentDot ?? makeDot(radius: radius, opacity: 1)
        if currentDot == nil {
            currentDot = dot
            layer.addSublayer(dot)
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        dot.position = location
        dot.opacity = 1
        CATransaction.commit()
    }

    func hideCurrent() {
        currentDot?.removeFromSuperlayer()
        currentDot = nil
        isTrailing = false
    }

    func clear() {
        retractTimer?.invalidate()
        retractTimer = nil
        trails.removeAll()
        isTrailing = false
        currentDot?.removeFromSuperlayer()
        currentDot = nil
        redrawTrail()
    }

    private func startRetracting() {
        guard retractTimer == nil else { return }
        let timer = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            self?.retract()
        }
        // .common keeps the trail retracting while AppKit is tracking a drag.
        RunLoop.main.add(timer, forMode: .common)
        retractTimer = timer
    }

    private func retract() {
        let cutoff = ProcessInfo.processInfo.systemUptime - trailDuration
        for index in trails.indices {
            // Keep the last expired point so the tail ends exactly at the cutoff instead of jumping.
            if let firstFresh = trails[index].firstIndex(where: { $0.time >= cutoff }), firstFresh > 1 {
                trails[index].removeFirst(firstFresh - 1)
            } else if trails[index].last.map({ $0.time < cutoff }) == true {
                trails[index].removeAll()
            }
        }
        let activeIndex = isTrailing ? trails.count - 1 : nil
        trails = trails.enumerated().filter { !$0.element.isEmpty || $0.offset == activeIndex }.map(\.element)
        if trails.allSatisfy(\.isEmpty), !isTrailing {
            trails.removeAll()
            retractTimer?.invalidate()
            retractTimer = nil
        }
        redrawTrail()
    }

    private func redrawTrail() {
        let path = CGMutablePath()
        for trail in trails where trail.count > 1 {
            path.move(to: trail[0].location)
            for point in trail.dropFirst() {
                path.addLine(to: point.location)
            }
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        trailLayer.path = path
        CATransaction.commit()
    }

    private func makeDot(radius: CGFloat, opacity: Float) -> CAShapeLayer {
        let dot = CAShapeLayer()
        let diameter = radius * 2
        dot.bounds = CGRect(x: 0, y: 0, width: diameter, height: diameter)
        dot.path = CGPath(ellipseIn: dot.bounds, transform: nil)
        dot.fillColor = color.cgColor
        dot.opacity = opacity
        return dot
    }
}

private final class AnnotationTextView: NSTextView {
    var commitHandler: (() -> Void)?
    var cancelHandler: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        if event.keyCode == UInt16(kVK_Escape) {
            cancelHandler?()
            return
        }
        if (event.keyCode == UInt16(kVK_Return)
            || event.keyCode == UInt16(kVK_ANSI_KeypadEnter)),
           !event.modifierFlags.contains(.shift)
        {
            if hasMarkedText() {
                super.keyDown(with: event)
            } else {
                commitHandler?()
            }
            return
        }
        super.keyDown(with: event)
    }
}

final class InkView: NSView {
    var keyDownInterceptor: ((NSEvent) -> Bool)?
    var keyUpInterceptor: ((NSEvent) -> Bool)?
    var scrollWheelInterceptor: ((NSEvent) -> Bool)?
    var magnifyInterceptor: ((NSEvent) -> Bool)?

    private struct ActiveStroke {
        let identifier: UUID
        let tool: InkStrokeTool
        let color: NSColor
        let width: CGFloat
        let isStraight: Bool
        var usesPressure: Bool
        var points: [InkPoint]
    }

    private struct ActiveEraser {
        let settings: EraserSettings
        var points: [CGPoint]
    }

    private struct ActiveShape {
        let kind: InkShapeKind
        let color: NSColor
        let width: CGFloat
        let start: InkPoint
        let isConstrained: Bool
        var end: InkPoint
    }

    private struct ActiveCover {
        let start: InkPoint
        var end: InkPoint
    }

    private enum SelectionInteraction {
        case lasso(points: [CGPoint])
        case move(start: CGPoint, originals: [InkStroke])
        case resize(anchor: CGPoint, sourceBounds: CGRect, originals: [InkStroke])
    }

    private struct TextEntrySession {
        let origin: CGPoint
        let view: AnnotationTextView
    }

    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let displayUUID: String
    private let requestKeyFocus: () -> Void
    private let boardBackgroundLayer = CALayer()
    private let boardPatternLayer = CAShapeLayer()
    private let transformedInkLayer = CALayer()
    private let completedLayer = CALayer()
    private let activeLayer = CALayer()
    private let selectionLayer = CALayer()
    private let laserRenderer = LaserRenderer()
    private let boardPageIndicatorLayer = CATextLayer()
    private let eraserCursorLayer = CAShapeLayer()
    private let eraserModeLayer = CATextLayer()
    private var activeStroke: ActiveStroke?
    private var activeEraser: ActiveEraser?
    private var activeShape: ActiveShape?
    private var activeCover: ActiveCover?
    private var selectionInteraction: SelectionInteraction?
    private var selectionPreview: [InkStroke]?
    private var textEntrySession: TextEntrySession?
    private var recognitionWorkItem: DispatchWorkItem?
    private var recognizedStroke: InkStroke?
    private var lastPointerLocation: CGPoint?
    private var eraserModePresentation = 0
    private var trackingAreaReference: NSTrackingArea?
    private var inkObserverIdentifier: UUID?
    private var modeObserverIdentifier: UUID?
    private var toolObserverIdentifier: UUID?
    private var toolSettingsObserverIdentifier: UUID?
    private var boardObserverIdentifier: UUID?
    private var presentationSourceRect: CGRect?

    override var acceptsFirstResponder: Bool { true }

    init(
        frame frameRect: NSRect,
        state: AppState,
        dispatcher: AppCommandDispatcher,
        displayUUID: String,
        requestKeyFocus: @escaping () -> Void
    ) {
        self.state = state
        self.dispatcher = dispatcher
        self.displayUUID = displayUUID
        self.requestKeyFocus = requestKeyFocus
        super.init(frame: frameRect)

        wantsLayer = true
        layer?.masksToBounds = true
        configureBoardLayers()
        layer?.addSublayer(boardBackgroundLayer)
        layer?.addSublayer(boardPatternLayer)
        layer?.addSublayer(transformedInkLayer)
        transformedInkLayer.addSublayer(completedLayer)
        transformedInkLayer.addSublayer(activeLayer)
        transformedInkLayer.addSublayer(laserRenderer.layer)
        transformedInkLayer.addSublayer(selectionLayer)
        configureEraserLayers()
        transformedInkLayer.addSublayer(eraserCursorLayer)
        layer?.addSublayer(boardPageIndicatorLayer)
        layer?.addSublayer(eraserModeLayer)

        inkObserverIdentifier = state.addInkObserver { [weak self] changedDisplayUUID, change in
            guard let self, changedDisplayUUID == self.displayUUID else { return }
            switch change {
            case let .added(stroke):
                self.addCompletedStrokeLayer(for: stroke)
            case .rebuilt:
                self.rebuildCompletedLayers()
            }
        }
        modeObserverIdentifier = state.addAnnotationModeObserver { [weak self] isEnabled in
            if !isEnabled {
                self?.cancelActiveStroke()
                self?.cancelTextEntry()
                self?.laserRenderer.clear()
                self?.eraserCursorLayer.isHidden = true
            }
        }
        toolObserverIdentifier = state.addToolObserver { [weak self] tool in
            if tool != .laser {
                self?.laserRenderer.clear()
            }
            if tool != .text {
                self?.cancelTextEntry()
            }
            if tool == .eraser, let location = self?.lastPointerLocation {
                self?.showEraserCursor(at: location)
            } else if tool != .eraser {
                self?.eraserCursorLayer.isHidden = true
            }
        }
        toolSettingsObserverIdentifier = state.addToolSettingsObserver { [weak self] in
            guard let self else { return }
            self.laserRenderer.color = self.state.laserColor
            self.laserRenderer.radius = self.state.laserSize
            self.laserRenderer.trailDuration = self.state.laserTrailDuration
            self.rebuildCompletedLayers()
        }
        boardObserverIdentifier = state.addBoardObserver { [weak self] in
            guard let self else { return }
            self.cancelActiveStroke()
            self.laserRenderer.clear()
            self.updateBoardPresentation()
            self.rebuildCompletedLayers()
        }
        rebuildCompletedLayers()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        layoutBoardLayers()
        transformedInkLayer.anchorPoint = .zero
        transformedInkLayer.position = .zero
        transformedInkLayer.bounds = bounds
        completedLayer.frame = bounds
        activeLayer.frame = bounds
        laserRenderer.layer.frame = bounds
        selectionLayer.frame = bounds
        layoutEraserModeLayer()
        CATransaction.commit()
        updatePresentationTransform(animated: false)
    }

    override func updateTrackingAreas() {
        if let trackingAreaReference {
            removeTrackingArea(trackingAreaReference)
        }
        let trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea)
        trackingAreaReference = trackingArea
        super.updateTrackingAreas()
    }

    override func setFrameSize(_ newSize: NSSize) {
        let sizeChanged = frame.size != newSize
        super.setFrameSize(newSize)
        if sizeChanged {
            updateBoardPresentation()
            rebuildCompletedLayers()
        }
    }

    func refreshForDisplayChange() {
        rebuildCompletedLayers()
    }

    func finishActiveInteraction() {
        if textEntrySession != nil {
            commitTextEntry()
        }
        if activeEraser != nil {
            finishActiveEraser()
        }
        if activeShape != nil {
            finishActiveShape()
        }
        if activeCover != nil {
            finishActiveCover()
        }
        if selectionInteraction != nil {
            finishSelectionInteraction()
        }
        if activeStroke != nil {
            finishActiveStroke()
        }
        laserRenderer.clear()
    }

    func renderedImage(
        strokes: [InkStroke]? = nil,
        boardKind: BoardKind? = nil,
        pixelSize: CGSize
    ) -> CGImage? {
        let pixelWidth = Int(pixelSize.width.rounded())
        let pixelHeight = Int(pixelSize.height.rounded())
        guard bounds.width > 0,
              bounds.height > 0,
              pixelWidth > 0,
              pixelHeight > 0,
              let context = CGContext(
                  data: nil,
                  width: pixelWidth,
                  height: pixelHeight,
                  bitsPerComponent: 8,
                  bytesPerRow: 0,
                  space: CGColorSpaceCreateDeviceRGB(),
                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
              )
        else {
            return nil
        }

        context.scaleBy(
            x: CGFloat(pixelWidth) / bounds.width,
            y: CGFloat(pixelHeight) / bounds.height
        )
        if let boardKind {
            context.setFillColor(state.boardBackgroundColor(for: boardKind).cgColor)
            context.fill(bounds)
            drawBoardPattern(for: boardKind, in: context)
        }

        let inkLayer = CALayer()
        inkLayer.frame = bounds
        for stroke in strokes ?? state.strokes(for: displayUUID) {
            switch stroke.content {
            case .freehand:
                if let layer = makeBitmapLayer(for: stroke) {
                    inkLayer.addSublayer(layer)
                }
            case .shape, .text, .cover:
                inkLayer.addSublayer(makeVectorLayer(for: stroke))
            }
        }
        inkLayer.render(in: context)
        return context.makeImage()
    }

    func setPresentationSourceRect(_ sourceRect: CGRect?, animated: Bool) {
        presentationSourceRect = sourceRect
        updatePresentationTransform(animated: animated)
    }

    override func mouseEntered(with event: NSEvent) {
        requestKeyFocus()
        let location = localLocation(for: event)
        lastPointerLocation = location
        if usesEraser(for: event) {
            showEraserCursor(at: location)
        } else if state.currentTool == .laser {
            laserRenderer.move(to: location, leavesTrail: false)
        }
    }

    override func mouseExited(with event: NSEvent) {
        laserRenderer.hideCurrent()
        eraserCursorLayer.isHidden = true
    }

    // A pen lifted out of range comes back wherever it lands; ending the trail here keeps the jump
    // from drawing a streak across the screen.
    override func tabletProximity(with event: NSEvent) {
        if !event.isEnteringProximity {
            laserRenderer.hideCurrent()
        }
        super.tabletProximity(with: event)
    }

    override func mouseMoved(with event: NSEvent) {
        requestKeyFocus()
        let location = localLocation(for: event)
        lastPointerLocation = location
        guard state.isAnnotationModeEnabled else { return }
        if usesEraser(for: event) {
            laserRenderer.hideCurrent()
            showEraserCursor(at: location)
        } else {
            eraserCursorLayer.isHidden = true
            if state.currentTool == .laser {
                laserRenderer.move(to: location, leavesTrail: false)
            }
        }
    }

    override func mouseDown(with event: NSEvent) {
        requestKeyFocus()
        guard state.isAnnotationModeEnabled else { return }

        let location = localLocation(for: event)
        lastPointerLocation = location
        if usesEraser(for: event) {
            beginEraser(at: location)
            showEraserCursor(at: location)
            return
        }

        switch state.currentTool {
        case .select:
            beginSelection(at: location)
            return
        case .text:
            beginTextEntry(
                at: location,
                presentedPoint: convert(event.locationInWindow, from: nil)
            )
            return
        case .arrow, .rectangle, .ellipse:
            beginShape(from: event)
            return
        case .cover:
            let normalizedPoint = InkGeometry.normalizedPoint(from: location, in: bounds.size)
            if state.removeTopmostCover(at: normalizedPoint, on: displayUUID) {
                return
            }
            beginCover(from: event)
            return
        case .pen, .highlighter, .laser, .eraser:
            break
        }

        let strokeTool: InkStrokeTool
        let width: CGFloat
        switch state.currentTool {
        case .pen:
            strokeTool = .pen
            width = state.penWidth
        case .highlighter:
            strokeTool = .highlighter
            width = state.highlighterWidth
        case .laser:
            laserRenderer.move(to: location, leavesTrail: true)
            return
        case .eraser:
            return
        case .arrow, .rectangle, .ellipse, .text, .select, .cover:
            return
        }

        let point = inkPoint(for: event)
        activeStroke = ActiveStroke(
            identifier: UUID(),
            tool: strokeTool,
            color: state.selectedColor,
            width: width,
            isStraight: event.modifierFlags.contains(.shift),
            usesPressure: isTabletEvent(event),
            points: [point]
        )
        renderActiveStroke()
        scheduleRecognitionIfNeeded()
    }

    override func mouseDragged(with event: NSEvent) {
        let location = localLocation(for: event)
        lastPointerLocation = location
        if activeEraser != nil {
            appendActiveEraserPoint(location)
            showEraserCursor(at: location)
            return
        }
        if activeShape != nil {
            updateActiveShape(from: event)
            return
        }
        if activeCover != nil {
            updateActiveCover(from: event)
            return
        }
        if selectionInteraction != nil {
            updateSelectionInteraction(at: location)
            return
        }
        if state.isAnnotationModeEnabled, state.currentTool == .laser {
            laserRenderer.move(to: location, leavesTrail: true)
            return
        }
        appendActivePoint(from: event)
    }

    override func mouseUp(with event: NSEvent) {
        // The pen's eraser tip erases even while the laser is selected; that path finishes below.
        if activeEraser == nil, state.isAnnotationModeEnabled, state.currentTool == .laser {
            laserRenderer.move(to: localLocation(for: event), leavesTrail: false)
            return
        }
        if activeEraser != nil {
            let location = localLocation(for: event)
            lastPointerLocation = location
            appendActiveEraserPoint(location)
            finishActiveEraser()
            showEraserCursor(at: location)
            return
        }
        if activeShape != nil {
            updateActiveShape(from: event)
            finishActiveShape()
            return
        }
        if activeCover != nil {
            updateActiveCover(from: event)
            finishActiveCover()
            return
        }
        if selectionInteraction != nil {
            updateSelectionInteraction(at: localLocation(for: event))
            finishSelectionInteraction()
            return
        }
        guard activeStroke != nil else { return }
        appendActivePoint(from: event)
        finishActiveStroke()
    }

    override func scrollWheel(with event: NSEvent) {
        if state.isSpotlightEnabled { return }
        if scrollWheelInterceptor?(event) == true { return }
        guard state.isAnnotationModeEnabled, state.currentTool == .eraser else {
            super.scrollWheel(with: event)
            return
        }
        let delta = event.scrollingDeltaY
        guard delta != 0 else { return }
        state.adjustCurrentToolWidth(by: delta > 0 ? 1 : -1)
        showEraserCursor(at: localLocation(for: event))
    }

    override func keyDown(with event: NSEvent) {
        if keyDownInterceptor?(event) == true { return }
        guard let definition = ShortcutCatalog.matching(event, scope: .annotation) else {
            super.keyDown(with: event)
            return
        }
        if definition.command.isBoardPageNavigation,
           state.boardKind(on: displayUUID) == nil
        {
            if !dispatcher.forwardToUnderlyingApplication(event) {
                super.keyDown(with: event)
            }
            return
        }

        dispatcher.keyDown(
            definition: definition,
            scope: .annotation,
            displayUUID: displayUUID,
            isRepeat: event.isARepeat
        )
        guard !event.isARepeat else { return }
        switch definition.command {
        case .selectOrCycleEraser:
            showEraserMode(state.eraserMode)
        case .adjustWidth:
            refreshEraserCursorIfNeeded()
        default:
            break
        }
    }

    override func keyUp(with event: NSEvent) {
        if keyUpInterceptor?(event) == true { return }
        guard let definition = ShortcutCatalog.matchingKeyUp(event, scope: .annotation) else {
            super.keyUp(with: event)
            return
        }
        if definition.command.isBoardPageNavigation {
            if state.boardKind(on: displayUUID) == nil,
               !dispatcher.forwardToUnderlyingApplication(event)
            {
                super.keyUp(with: event)
            }
            return
        }
        guard definition.command == .showHelp else {
            super.keyUp(with: event)
            return
        }
        dispatcher.keyUp(definition: definition)
    }

    override func magnify(with event: NSEvent) {
        if magnifyInterceptor?(event) == true { return }
        super.magnify(with: event)
    }

    private func appendActivePoint(from event: NSEvent) {
        guard var stroke = activeStroke else { return }
        let point = inkPoint(for: event)
        var appended = false
        if stroke.points.last?.location != point.location || stroke.points.count == 1 {
            stroke.points.append(point)
            appended = true
        }
        stroke.usesPressure = stroke.usesPressure || isTabletEvent(event)
        activeStroke = stroke
        if appended {
            recognizedStroke = nil
            scheduleRecognitionIfNeeded()
        }
        renderActiveStroke()
    }

    private func beginShape(from event: NSEvent) {
        let point = inkPoint(for: event)
        let kind: InkShapeKind
        switch state.currentTool {
        case .arrow:
            kind = .arrow(state.arrowStyle)
        case .rectangle:
            kind = .rectangle
        case .ellipse:
            kind = .ellipse
        default:
            return
        }
        activeShape = ActiveShape(
            kind: kind,
            color: state.selectedColor,
            width: state.penWidth,
            start: point,
            isConstrained: event.modifierFlags.contains(.shift),
            end: point
        )
        renderActiveShape()
    }

    private func updateActiveShape(from event: NSEvent) {
        guard var shape = activeShape else { return }
        shape.end = inkPoint(for: event)
        activeShape = shape
        renderActiveShape()
    }

    private func finishActiveShape() {
        guard let activeShape else { return }
        let stroke = shapeStroke(for: activeShape)
        self.activeShape = nil
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        state.addStroke(stroke, to: displayUUID)
    }

    private func beginCover(from event: NSEvent) {
        let point = inkPoint(for: event)
        activeCover = ActiveCover(start: point, end: point)
        renderActiveCover()
    }

    private func updateActiveCover(from event: NSEvent) {
        guard var cover = activeCover else { return }
        cover.end = inkPoint(for: event)
        activeCover = cover
        renderActiveCover()
    }

    private func finishActiveCover() {
        guard let activeCover else { return }
        let stroke = coverStroke(for: activeCover)
        self.activeCover = nil
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }

        let coverBounds = InkItemGeometry.bounds(for: stroke, canvasSize: bounds.size)
        guard coverBounds.width >= 4, coverBounds.height >= 4 else { return }
        state.addStroke(stroke, to: displayUUID)
    }

    private func coverStroke(for activeCover: ActiveCover) -> InkStroke {
        InkStroke(
            tool: .cover,
            color: .systemGray,
            width: 1,
            usesPressure: false,
            points: [activeCover.start, activeCover.end],
            content: .cover
        )
    }

    private func renderActiveCover() {
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        guard let activeCover else { return }
        activeLayer.addSublayer(makeVectorLayer(for: coverStroke(for: activeCover)))
    }

    private func beginEraser(at point: CGPoint) {
        recognitionWorkItem?.cancel()
        recognizedStroke = nil
        activeStroke = nil
        activeShape = nil
        activeCover = nil
        selectionInteraction = nil
        activeEraser = ActiveEraser(settings: state.eraserSettings, points: [point])
        renderActiveEraser()
    }

    private func appendActiveEraserPoint(_ point: CGPoint) {
        guard var eraser = activeEraser else { return }
        if eraser.points.last != point {
            eraser.points.append(point)
        }
        activeEraser = eraser
        renderActiveEraser()
    }

    private func finishActiveEraser() {
        guard let activeEraser, let first = activeEraser.points.first else { return }
        let gesture: EraserGesture
        switch activeEraser.settings.mode {
        case .partial, .stroke:
            gesture = .sweep(points: activeEraser.points)
        case .area:
            switch activeEraser.settings.areaShape {
            case .rectangle:
                let last = activeEraser.points.last ?? first
                gesture = .area(
                    CGRect(
                        x: first.x,
                        y: first.y,
                        width: last.x - first.x,
                        height: last.y - first.y
                    )
                )
            case .lasso:
                gesture = .lasso(points: activeEraser.points)
            }
        }

        self.activeEraser = nil
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        _ = state.erase(
            gesture,
            settings: activeEraser.settings,
            on: displayUUID,
            canvasSize: bounds.size
        )
    }

    private func finishActiveStroke() {
        guard let activeStroke else { return }
        recognitionWorkItem?.cancel()
        let stroke = inkStroke(for: activeStroke)
        let corrected = recognizedStroke
        self.activeStroke = nil
        recognizedStroke = nil
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        state.addStroke(stroke, to: displayUUID)
        if let corrected {
            let after = state.strokes(for: displayUUID).map {
                $0.identifier == stroke.identifier ? corrected : $0
            }
            _ = state.replaceStrokes(after, on: displayUUID)
        }
    }

    private func cancelActiveStroke() {
        recognitionWorkItem?.cancel()
        recognitionWorkItem = nil
        recognizedStroke = nil
        activeStroke = nil
        activeEraser = nil
        activeShape = nil
        activeCover = nil
        selectionInteraction = nil
        selectionPreview = nil
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        renderSelection()
    }

    private func inkStroke(for activeStroke: ActiveStroke) -> InkStroke {
        InkStroke(
            identifier: activeStroke.identifier,
            tool: activeStroke.tool,
            color: activeStroke.color,
            width: activeStroke.width,
            usesPressure: activeStroke.tool == .pen && activeStroke.usesPressure,
            points: effectivePoints(for: activeStroke)
        )
    }

    private func scheduleRecognitionIfNeeded() {
        recognitionWorkItem?.cancel()
        recognitionWorkItem = nil
        guard state.isShapeRecognitionEnabled,
              let activeStroke,
              activeStroke.tool == .pen,
              !activeStroke.isStraight,
              activeStroke.points.count >= 2
        else {
            return
        }

        let workItem = DispatchWorkItem { [weak self] in
            guard let self,
                  self.state.isShapeRecognitionEnabled,
                  let current = self.activeStroke,
                  current.identifier == activeStroke.identifier
            else {
                return
            }
            let original = self.inkStroke(for: current)
            self.recognizedStroke = ShapeRecognizer.correctedStroke(
                from: original,
                canvasSize: self.bounds.size
            )
            self.renderActiveStroke()
        }
        recognitionWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5, execute: workItem)
    }

    private func shapeStroke(for activeShape: ActiveShape) -> InkStroke {
        let start = InkGeometry.screenPoint(from: activeShape.start.location, in: bounds.size)
        let proposed = InkGeometry.screenPoint(from: activeShape.end.location, in: bounds.size)
        let endpoint: CGPoint
        if activeShape.isConstrained {
            switch activeShape.kind {
            case .line, .arrow:
                endpoint = InkGeometry.snappedPenEndpoint(from: start, to: proposed)
            case .rectangle, .ellipse:
                endpoint = InkGeometry.snappedSquareEndpoint(from: start, to: proposed)
            }
        } else {
            endpoint = proposed
        }
        let end = InkPoint(
            location: InkGeometry.normalizedPoint(from: endpoint, in: bounds.size),
            pressure: 1,
            timestamp: activeShape.end.timestamp
        )
        return InkStroke(
            tool: .shape,
            color: activeShape.color,
            width: activeShape.width,
            usesPressure: false,
            points: [activeShape.start, end],
            content: .shape(activeShape.kind)
        )
    }

    private func renderActiveShape() {
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        guard let activeShape else { return }
        activeLayer.addSublayer(makeVectorLayer(for: shapeStroke(for: activeShape)))
    }

    private func beginTextEntry(at origin: CGPoint, presentedPoint: CGPoint) {
        cancelTextEntry()
        let availableWidth = max(80, min(320, bounds.width - presentedPoint.x - 12))
        let textView = AnnotationTextView(
            frame: CGRect(x: presentedPoint.x, y: presentedPoint.y, width: availableWidth, height: 110)
        )
        textView.font = .systemFont(ofSize: state.textFontSize)
        textView.textColor = displayColor(for: state.selectedColor)
        textView.insertionPointColor = textView.textColor ?? .labelColor
        textView.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.94)
        textView.drawsBackground = true
        textView.isRichText = false
        textView.isHorizontallyResizable = false
        textView.isVerticallyResizable = true
        textView.textContainer?.widthTracksTextView = true
        textView.textContainerInset = CGSize(width: 5, height: 4)
        textView.wantsLayer = true
        textView.layer?.borderColor = NSColor.controlAccentColor.cgColor
        textView.layer?.borderWidth = 1.5
        textView.layer?.cornerRadius = 5
        textView.commitHandler = { [weak self] in
            self?.commitTextEntry()
        }
        textView.cancelHandler = { [weak self] in
            self?.cancelTextEntry()
        }
        addSubview(textView)
        textEntrySession = TextEntrySession(origin: origin, view: textView)
        window?.makeFirstResponder(textView)
    }

    private func commitTextEntry() {
        guard let session = textEntrySession else { return }
        let string = session.view.string
        guard !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            cancelTextEntry()
            return
        }

        let fontSize = state.textFontSize
        let font = NSFont.systemFont(ofSize: fontSize)
        let measured = (string as NSString).boundingRect(
            with: CGSize(width: 320, height: CGFloat.greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: font]
        ).integral.size
        let textSize = CGSize(
            width: max(fontSize, measured.width + 8),
            height: max(fontSize * 1.2, measured.height + 6)
        )
        let start = InkPoint(
            location: InkGeometry.normalizedPoint(from: session.origin, in: bounds.size),
            pressure: 1,
            timestamp: ProcessInfo.processInfo.systemUptime
        )
        let end = InkPoint(
            location: InkGeometry.normalizedPoint(
                from: CGPoint(x: session.origin.x + textSize.width, y: session.origin.y + textSize.height),
                in: bounds.size
            ),
            pressure: 1,
            timestamp: ProcessInfo.processInfo.systemUptime
        )
        let stroke = InkStroke(
            tool: .text,
            color: state.selectedColor,
            width: 1,
            usesPressure: false,
            points: [start, end],
            content: .text(InkTextContent(string: string, fontSize: fontSize))
        )
        endTextEntry()
        state.addStroke(stroke, to: displayUUID)
    }

    private func cancelTextEntry() {
        guard textEntrySession != nil else { return }
        endTextEntry()
    }

    private func endTextEntry() {
        textEntrySession?.view.removeFromSuperview()
        textEntrySession = nil
        window?.makeFirstResponder(self)
    }

    private func beginSelection(at point: CGPoint) {
        let selected = selectedStrokes()
        if !selected.isEmpty {
            let bounds = selectionBounds(for: selected)
            if let anchor = resizeAnchor(at: point, selectionBounds: bounds) {
                selectionInteraction = .resize(
                    anchor: InkGeometry.normalizedPoint(from: anchor, in: self.bounds.size),
                    sourceBounds: normalizedPointBounds(for: selected),
                    originals: selected
                )
                return
            }
            if bounds.insetBy(dx: -6, dy: -6).contains(point) {
                selectionInteraction = .move(start: point, originals: selected)
                return
            }
        }

        state.setSelectedStrokeIdentifiers([], on: displayUUID)
        selectionInteraction = .lasso(points: [point])
        renderSelection()
    }

    private func updateSelectionInteraction(at point: CGPoint) {
        guard let selectionInteraction else { return }
        switch selectionInteraction {
        case .lasso(var points):
            if points.last != point {
                points.append(point)
            }
            self.selectionInteraction = .lasso(points: points)
        case let .move(start, originals):
            let delta = CGPoint(
                x: (point.x - start.x) / max(bounds.width, 1),
                y: (point.y - start.y) / max(bounds.height, 1)
            )
            selectionPreview = SelectionTransform.moved(originals, by: delta)
            rebuildCompletedLayers()
        case let .resize(anchor, sourceBounds, originals):
            let current = InkGeometry.normalizedPoint(from: point, in: bounds.size)
            let destination = CGRect(
                x: min(anchor.x, current.x),
                y: min(anchor.y, current.y),
                width: max(abs(current.x - anchor.x), 0.001),
                height: max(abs(current.y - anchor.y), 0.001)
            )
            selectionPreview = SelectionTransform.resized(
                originals,
                from: sourceBounds,
                to: destination
            )
            rebuildCompletedLayers()
        }
        renderSelection()
    }

    private func finishSelectionInteraction() {
        guard let selectionInteraction else { return }
        self.selectionInteraction = nil
        switch selectionInteraction {
        case let .lasso(points):
            let selected = SelectionEngine.selectedIdentifiers(
                from: state.strokes(for: displayUUID),
                polygon: points,
                hitRule: state.eraserHitRule,
                canvasSize: bounds.size
            )
            state.setSelectedStrokeIdentifiers(selected, on: displayUUID)
        case .move, .resize:
            guard let selectionPreview else {
                renderSelection()
                return
            }
            let replacements = Dictionary(
                uniqueKeysWithValues: selectionPreview.map { ($0.identifier, $0) }
            )
            let after = state.strokes(for: displayUUID).map {
                replacements[$0.identifier] ?? $0
            }
            self.selectionPreview = nil
            _ = state.replaceStrokes(after, on: displayUUID)
        }
        renderSelection()
    }

    private func selectedStrokes() -> [InkStroke] {
        let identifiers = state.selectedStrokeIdentifiers(on: displayUUID)
        let source = selectionPreview ?? state.strokes(for: displayUUID)
        return source.filter { identifiers.contains($0.identifier) }
    }

    private func normalizedPointBounds(for strokes: [InkStroke]) -> CGRect {
        strokes.reduce(CGRect.null) { result, stroke in
            result.union(stroke.pointBounds)
        }
    }

    private func selectionBounds(for strokes: [InkStroke]) -> CGRect {
        strokes.reduce(CGRect.null) { result, stroke in
            result.union(InkItemGeometry.bounds(for: stroke, canvasSize: bounds.size))
        }
    }

    private func resizeAnchor(at point: CGPoint, selectionBounds: CGRect) -> CGPoint? {
        let pairs = [
            (
                CGPoint(x: selectionBounds.minX, y: selectionBounds.minY),
                CGPoint(x: selectionBounds.maxX, y: selectionBounds.maxY)
            ),
            (
                CGPoint(x: selectionBounds.maxX, y: selectionBounds.minY),
                CGPoint(x: selectionBounds.minX, y: selectionBounds.maxY)
            ),
            (
                CGPoint(x: selectionBounds.maxX, y: selectionBounds.maxY),
                CGPoint(x: selectionBounds.minX, y: selectionBounds.minY)
            ),
            (
                CGPoint(x: selectionBounds.minX, y: selectionBounds.maxY),
                CGPoint(x: selectionBounds.maxX, y: selectionBounds.minY)
            ),
        ]
        return pairs.first(where: { hypot(point.x - $0.0.x, point.y - $0.0.y) <= 10 })?.1
    }

    private func renderSelection() {
        selectionLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        if case let .lasso(points)? = selectionInteraction {
            let lasso = CAShapeLayer()
            lasso.frame = bounds
            lasso.path = path(for: points, closes: true)
            lasso.fillColor = NSColor.controlAccentColor.withAlphaComponent(0.08).cgColor
            lasso.strokeColor = NSColor.controlAccentColor.cgColor
            lasso.lineWidth = 1.5
            lasso.lineDashPattern = [5, 4]
            selectionLayer.addSublayer(lasso)
        }

        let strokes = selectedStrokes()
        guard !strokes.isEmpty else { return }
        let selectedBounds = selectionBounds(for: strokes)
        let outline = CAShapeLayer()
        outline.frame = bounds
        outline.path = CGPath(rect: selectedBounds, transform: nil)
        outline.fillColor = NSColor.clear.cgColor
        outline.strokeColor = NSColor.controlAccentColor.cgColor
        outline.lineWidth = 1.5
        outline.lineDashPattern = [6, 4]
        selectionLayer.addSublayer(outline)

        let handles = [
            CGPoint(x: selectedBounds.minX, y: selectedBounds.minY),
            CGPoint(x: selectedBounds.maxX, y: selectedBounds.minY),
            CGPoint(x: selectedBounds.maxX, y: selectedBounds.maxY),
            CGPoint(x: selectedBounds.minX, y: selectedBounds.maxY),
        ]
        for point in handles {
            let handle = CAShapeLayer()
            handle.bounds = CGRect(x: 0, y: 0, width: 10, height: 10)
            handle.position = point
            handle.path = CGPath(rect: handle.bounds, transform: nil)
            handle.fillColor = NSColor.white.cgColor
            handle.strokeColor = NSColor.controlAccentColor.cgColor
            handle.lineWidth = 1.5
            selectionLayer.addSublayer(handle)
        }
    }

    private func renderActiveEraser() {
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        guard let activeEraser,
              activeEraser.settings.mode == .area,
              let first = activeEraser.points.first
        else {
            return
        }
        let shape = CAShapeLayer()
        shape.frame = bounds
        switch activeEraser.settings.areaShape {
        case .rectangle:
            let last = activeEraser.points.last ?? first
            let rect = CGRect(
                x: first.x,
                y: first.y,
                width: last.x - first.x,
                height: last.y - first.y
            ).standardized
            shape.path = CGPath(rect: rect, transform: nil)
        case .lasso:
            shape.path = path(for: activeEraser.points, closes: true)
        }
        shape.fillColor = NSColor.controlAccentColor.withAlphaComponent(0.12).cgColor
        shape.strokeColor = NSColor.controlAccentColor.cgColor
        shape.lineWidth = 1.5
        shape.lineDashPattern = [6, 4]
        activeLayer.addSublayer(shape)
    }

    private func configureEraserLayers() {
        eraserCursorLayer.fillColor = NSColor.clear.cgColor
        eraserCursorLayer.strokeColor = NSColor.white.cgColor
        eraserCursorLayer.lineWidth = 1.5
        eraserCursorLayer.shadowColor = NSColor.black.cgColor
        eraserCursorLayer.shadowOpacity = 0.8
        eraserCursorLayer.shadowRadius = 1
        eraserCursorLayer.shadowOffset = .zero
        eraserCursorLayer.isHidden = true

        eraserModeLayer.alignmentMode = .center
        eraserModeLayer.fontSize = 15
        eraserModeLayer.foregroundColor = NSColor.white.cgColor
        eraserModeLayer.backgroundColor = NSColor.black.withAlphaComponent(0.72).cgColor
        eraserModeLayer.cornerRadius = 8
        eraserModeLayer.isHidden = true
    }

    private func configureBoardLayers() {
        boardBackgroundLayer.isHidden = true
        boardPatternLayer.fillColor = nil
        boardPatternLayer.lineWidth = 1
        boardPatternLayer.isHidden = true

        boardPageIndicatorLayer.alignmentMode = .center
        boardPageIndicatorLayer.fontSize = 15
        boardPageIndicatorLayer.cornerRadius = 8
        boardPageIndicatorLayer.isHidden = true
    }

    private func updateBoardPresentation() {
        guard let kind = state.boardKind(on: displayUUID),
              let pagePosition = state.boardPagePosition(on: displayUUID)
        else {
            boardBackgroundLayer.isHidden = true
            boardPatternLayer.isHidden = true
            boardPageIndicatorLayer.isHidden = true
            return
        }

        boardBackgroundLayer.backgroundColor = state.boardBackgroundColor(for: kind).cgColor
        boardBackgroundLayer.isHidden = false
        boardPatternLayer.strokeColor = BoardAppearance.patternColor(for: kind).cgColor
        boardPatternLayer.isHidden = state.boardBackgroundPattern == .none
        boardPageIndicatorLayer.string = "\(pagePosition.current) / \(pagePosition.total)"
        boardPageIndicatorLayer.foregroundColor = kind == .white
            ? NSColor.black.withAlphaComponent(0.68).cgColor
            : NSColor.white.withAlphaComponent(0.82).cgColor
        boardPageIndicatorLayer.backgroundColor = kind == .white
            ? NSColor.black.withAlphaComponent(0.08).cgColor
            : NSColor.white.withAlphaComponent(0.12).cgColor
        boardPageIndicatorLayer.isHidden = false
        layoutBoardLayers()
    }

    private func layoutBoardLayers() {
        boardBackgroundLayer.frame = bounds
        boardPatternLayer.frame = bounds
        boardPatternLayer.path = boardPatternPath()
        let indicatorWidth = min(max(bounds.width - 32, 0), 84)
        boardPageIndicatorLayer.frame = CGRect(
            x: max(16, bounds.width - indicatorWidth - 20),
            y: 20,
            width: indicatorWidth,
            height: 30
        )
        boardPageIndicatorLayer.contentsScale = window?.backingScaleFactor
            ?? NSScreen.main?.backingScaleFactor
            ?? 1
    }

    private func boardPatternPath() -> CGPath? {
        guard state.boardKind(on: displayUUID) != nil else { return nil }
        let path = CGMutablePath()
        let spacing: CGFloat = 40
        if state.boardBackgroundPattern == .grid {
            for x in stride(from: spacing, through: bounds.width, by: spacing) {
                path.move(to: CGPoint(x: x, y: 0))
                path.addLine(to: CGPoint(x: x, y: bounds.height))
            }
        }
        if state.boardBackgroundPattern == .grid || state.boardBackgroundPattern == .lines {
            for y in stride(from: spacing, through: bounds.height, by: spacing) {
                path.move(to: CGPoint(x: 0, y: y))
                path.addLine(to: CGPoint(x: bounds.width, y: y))
            }
        }
        return path
    }

    private func drawBoardPattern(for kind: BoardKind, in context: CGContext) {
        guard state.boardBackgroundPattern != .none else { return }
        let spacing: CGFloat = 40
        let path = CGMutablePath()
        if state.boardBackgroundPattern == .grid {
            for x in stride(from: spacing, through: bounds.width, by: spacing) {
                path.move(to: CGPoint(x: x, y: 0))
                path.addLine(to: CGPoint(x: x, y: bounds.height))
            }
        }
        for y in stride(from: spacing, through: bounds.height, by: spacing) {
            path.move(to: CGPoint(x: 0, y: y))
            path.addLine(to: CGPoint(x: bounds.width, y: y))
        }
        context.addPath(path)
        context.setStrokeColor(BoardAppearance.patternColor(for: kind).cgColor)
        context.setLineWidth(1)
        context.strokePath()
    }

    private func showEraserCursor(at location: CGPoint) {
        let diameter = state.eraserDiameter
        let cursorBounds = CGRect(x: 0, y: 0, width: diameter, height: diameter)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        eraserCursorLayer.bounds = cursorBounds
        eraserCursorLayer.position = location
        eraserCursorLayer.path = CGPath(ellipseIn: cursorBounds, transform: nil)
        eraserCursorLayer.isHidden = false
        CATransaction.commit()
    }

    private func refreshEraserCursorIfNeeded() {
        guard state.currentTool == .eraser, let lastPointerLocation else { return }
        showEraserCursor(at: lastPointerLocation)
    }

    private func showEraserMode(_ mode: EraserMode) {
        eraserModePresentation += 1
        let presentation = eraserModePresentation
        eraserModeLayer.string = "지우개: \(mode.displayName)"
        layoutEraserModeLayer()

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        eraserModeLayer.opacity = 1
        eraserModeLayer.isHidden = false
        CATransaction.commit()

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [weak self] in
            guard let self, self.eraserModePresentation == presentation else { return }
            CATransaction.begin()
            CATransaction.setAnimationDuration(0.2)
            CATransaction.setCompletionBlock { [weak self] in
                guard self?.eraserModePresentation == presentation else { return }
                self?.eraserModeLayer.isHidden = true
            }
            self.eraserModeLayer.opacity = 0
            CATransaction.commit()
        }
    }

    private func layoutEraserModeLayer() {
        let width = min(max(bounds.width - 32, 0), 220)
        eraserModeLayer.frame = CGRect(
            x: (bounds.width - width) / 2,
            y: 24,
            width: width,
            height: 32
        )
        eraserModeLayer.contentsScale = window?.backingScaleFactor
            ?? NSScreen.main?.backingScaleFactor
            ?? 1
    }

    private func usesEraser(for event: NSEvent) -> Bool {
        state.currentTool == .eraser || event.pointingDeviceType == .eraser
    }

    private func effectivePoints(for stroke: ActiveStroke) -> [InkPoint] {
        guard stroke.isStraight,
              let first = stroke.points.first,
              let last = stroke.points.last
        else {
            return stroke.points
        }

        let start = InkGeometry.screenPoint(from: first.location, in: bounds.size)
        let proposed = InkGeometry.screenPoint(from: last.location, in: bounds.size)
        let endpoint: CGPoint
        switch stroke.tool {
        case .pen:
            endpoint = InkGeometry.snappedPenEndpoint(from: start, to: proposed)
        case .highlighter:
            endpoint = InkGeometry.snappedHighlighterEndpoint(from: start, to: proposed)
        case .shape, .text, .cover:
            endpoint = proposed
        }
        let normalizedEndpoint = InkGeometry.normalizedPoint(from: endpoint, in: bounds.size)
        return [
            first,
            InkPoint(
                location: normalizedEndpoint,
                pressure: last.pressure,
                timestamp: last.timestamp
            ),
        ]
    }

    private func renderActiveStroke() {
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        guard let activeStroke else { return }
        let stroke = recognizedStroke ?? inkStroke(for: activeStroke)
        let container = makeVectorLayer(for: stroke)
        activeLayer.addSublayer(container)
    }

    private func rebuildCompletedLayers() {
        completedLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        let replacements = Dictionary(
            uniqueKeysWithValues: (selectionPreview ?? []).map { ($0.identifier, $0) }
        )
        for storedStroke in state.strokes(for: displayUUID) {
            let stroke = replacements[storedStroke.identifier] ?? storedStroke
            addCompletedStrokeLayer(for: stroke)
        }
        renderSelection()
    }

    private func addCompletedStrokeLayer(for stroke: InkStroke) {
        guard bounds.width > 0, bounds.height > 0 else { return }
        switch stroke.content {
        case .freehand:
            guard let layer = makeBitmapLayer(for: stroke) else { return }
            completedLayer.addSublayer(layer)
        case .shape, .text, .cover:
            completedLayer.addSublayer(makeVectorLayer(for: stroke))
        }
    }

    private func makeBitmapLayer(for stroke: InkStroke) -> CALayer? {
        let samples = renderedSamples(for: stroke)
        guard !samples.isEmpty else { return nil }

        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
        let radius = stroke.width / 2 + 3
        let pointBounds = samples.reduce(CGRect.null) { partial, point in
            partial.union(CGRect(origin: point.location, size: .zero))
        }
        let drawingBounds = pointBounds.insetBy(dx: -radius, dy: -radius).integral
        let pixelWidth = max(1, Int(ceil(drawingBounds.width * scale)))
        let pixelHeight = max(1, Int(ceil(drawingBounds.height * scale)))
        guard let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: pixelWidth,
            pixelsHigh: pixelHeight,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bitmapFormat: [],
            bytesPerRow: 0,
            bitsPerPixel: 0
        ), let graphicsContext = NSGraphicsContext(bitmapImageRep: bitmap)
        else {
            return nil
        }

        bitmap.size = drawingBounds.size
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = graphicsContext
        // The context was created from a pixel-sized rep, so its CTM is in pixels; map points to pixels.
        graphicsContext.cgContext.scaleBy(x: scale, y: scale)
        graphicsContext.cgContext.translateBy(x: -drawingBounds.minX, y: -drawingBounds.minY)
        draw(stroke: stroke, samples: samples, in: graphicsContext.cgContext)
        graphicsContext.flushGraphics()
        NSGraphicsContext.restoreGraphicsState()

        guard let image = bitmap.cgImage else { return nil }
        let cachedLayer = CALayer()
        cachedLayer.frame = drawingBounds
        cachedLayer.contents = image
        cachedLayer.contentsScale = scale
        cachedLayer.contentsGravity = .resize
        if stroke.tool == .highlighter {
            cachedLayer.opacity = Float(state.highlighterOpacity)
            cachedLayer.compositingFilter = "multiplyBlendMode"
        }
        return cachedLayer
    }

    private func makeVectorLayer(for stroke: InkStroke) -> CALayer {
        let container = CALayer()
        container.frame = bounds
        switch stroke.content {
        case .freehand:
            let samples = renderedSamples(for: stroke)
            addShapeLayers(for: stroke, samples: samples, to: container)
            if stroke.tool == .highlighter {
                container.opacity = Float(state.highlighterOpacity)
                container.compositingFilter = "multiplyBlendMode"
            }
        case let .shape(kind):
            addShapeLayer(for: stroke, kind: kind, to: container)
        case let .text(content):
            addTextLayer(for: stroke, content: content, to: container)
        case .cover:
            addCoverLayer(for: stroke, to: container)
        }
        return container
    }

    private func addCoverLayer(for stroke: InkStroke, to container: CALayer) {
        guard let first = stroke.points.first, let last = stroke.points.last else { return }
        let start = InkGeometry.screenPoint(from: first.location, in: bounds.size)
        let end = InkGeometry.screenPoint(from: last.location, in: bounds.size)
        let cover = CAShapeLayer()
        cover.path = CGPath(
            rect: CGRect(
                x: min(start.x, end.x),
                y: min(start.y, end.y),
                width: abs(end.x - start.x),
                height: abs(end.y - start.y)
            ),
            transform: nil
        )
        cover.fillColor = NSColor.systemGray.withAlphaComponent(0.96).cgColor
        cover.strokeColor = NSColor.separatorColor.cgColor
        cover.lineWidth = 1
        container.addSublayer(cover)
    }

    private func addShapeLayer(
        for stroke: InkStroke,
        kind: InkShapeKind,
        to container: CALayer
    ) {
        guard let first = stroke.points.first,
              let last = stroke.points.last
        else {
            return
        }
        let start = InkGeometry.screenPoint(from: first.location, in: bounds.size)
        let end = InkGeometry.screenPoint(from: last.location, in: bounds.size)
        let path = CGMutablePath()
        switch kind {
        case .line:
            path.move(to: start)
            path.addLine(to: end)
        case let .arrow(style):
            path.move(to: start)
            path.addLine(to: end)
            addArrowHead(to: path, tip: end, tail: start, width: stroke.width)
            if style == .double {
                addArrowHead(to: path, tip: start, tail: end, width: stroke.width)
            }
        case .rectangle:
            path.addRect(
                CGRect(
                    x: min(start.x, end.x),
                    y: min(start.y, end.y),
                    width: abs(end.x - start.x),
                    height: abs(end.y - start.y)
                )
            )
        case .ellipse:
            path.addEllipse(
                in: CGRect(
                    x: min(start.x, end.x),
                    y: min(start.y, end.y),
                    width: abs(end.x - start.x),
                    height: abs(end.y - start.y)
                )
            )
        }

        let shape = CAShapeLayer()
        shape.path = path
        shape.fillColor = nil
        shape.strokeColor = displayColor(for: stroke.color).cgColor
        shape.lineWidth = stroke.width
        shape.lineJoin = .round
        shape.lineCap = .round
        container.addSublayer(shape)
    }

    private func addArrowHead(
        to path: CGMutablePath,
        tip: CGPoint,
        tail: CGPoint,
        width: CGFloat
    ) {
        let angle = atan2(tip.y - tail.y, tip.x - tail.x)
        let length = max(10, width * 4)
        let spread = CGFloat.pi / 7
        path.move(to: tip)
        path.addLine(
            to: CGPoint(
                x: tip.x - cos(angle - spread) * length,
                y: tip.y - sin(angle - spread) * length
            )
        )
        path.move(to: tip)
        path.addLine(
            to: CGPoint(
                x: tip.x - cos(angle + spread) * length,
                y: tip.y - sin(angle + spread) * length
            )
        )
    }

    private func addTextLayer(
        for stroke: InkStroke,
        content: InkTextContent,
        to container: CALayer
    ) {
        guard let first = stroke.points.first,
              let last = stroke.points.last
        else {
            return
        }
        let start = InkGeometry.screenPoint(from: first.location, in: bounds.size)
        let end = InkGeometry.screenPoint(from: last.location, in: bounds.size)
        let textLayer = CATextLayer()
        textLayer.frame = CGRect(
            x: min(start.x, end.x),
            y: min(start.y, end.y),
            width: abs(end.x - start.x),
            height: abs(end.y - start.y)
        )
        textLayer.string = NSAttributedString(
            string: content.string,
            attributes: [
                .font: NSFont.systemFont(ofSize: content.fontSize),
                .foregroundColor: displayColor(for: stroke.color),
            ]
        )
        textLayer.isWrapped = true
        textLayer.alignmentMode = .left
        textLayer.contentsScale = window?.backingScaleFactor
            ?? NSScreen.main?.backingScaleFactor
            ?? 1
        container.addSublayer(textLayer)
    }

    private func renderedSamples(for stroke: InkStroke) -> [(location: CGPoint, pressure: CGFloat)] {
        InkGeometry.smoothedPoints(stroke.points).map { point in
            (
                InkGeometry.screenPoint(from: point.location, in: bounds.size),
                point.pressure
            )
        }
    }

    private func draw(
        stroke: InkStroke,
        samples: [(location: CGPoint, pressure: CGFloat)],
        in context: CGContext
    ) {
        let color = displayColor(for: stroke.color)
        context.setStrokeColor(color.cgColor)
        context.setFillColor(color.cgColor)

        guard samples.count > 1 else {
            guard let sample = samples.first else { return }
            let width = effectiveWidth(for: stroke, pressure: sample.pressure)
            let rect = CGRect(
                x: sample.location.x - width / 2,
                y: sample.location.y - width / 2,
                width: width,
                height: width
            )
            if stroke.tool == .pen {
                context.fillEllipse(in: rect)
            } else {
                context.fill(rect)
            }
            return
        }

        if !stroke.usesPressure {
            let path = path(for: samples.map { $0.location })
            context.addPath(path)
            context.setLineWidth(stroke.width)
            context.setLineJoin(.round)
            context.setLineCap(stroke.tool == .pen ? .round : .butt)
            context.strokePath()
            return
        }

        context.addPath(variableWidthPath(for: stroke, samples: samples))
        context.fillPath()
    }

    private func addShapeLayers(
        for stroke: InkStroke,
        samples: [(location: CGPoint, pressure: CGFloat)],
        to container: CALayer
    ) {
        let color = displayColor(for: stroke.color)
        guard samples.count > 1 else {
            guard let sample = samples.first else { return }
            let width = effectiveWidth(for: stroke, pressure: sample.pressure)
            let shape = CAShapeLayer()
            let rect = CGRect(
                x: sample.location.x - width / 2,
                y: sample.location.y - width / 2,
                width: width,
                height: width
            )
            shape.path = stroke.tool == .pen
                ? CGPath(ellipseIn: rect, transform: nil)
                : CGPath(rect: rect, transform: nil)
            shape.fillColor = color.cgColor
            container.addSublayer(shape)
            return
        }

        if !stroke.usesPressure {
            let shape = CAShapeLayer()
            shape.path = path(for: samples.map { $0.location })
            shape.fillColor = nil
            shape.strokeColor = color.cgColor
            shape.lineWidth = stroke.width
            shape.lineJoin = .round
            shape.lineCap = stroke.tool == .pen ? .round : .butt
            container.addSublayer(shape)
            return
        }

        let shape = CAShapeLayer()
        shape.path = variableWidthPath(for: stroke, samples: samples)
        shape.fillColor = color.cgColor
        container.addSublayer(shape)
    }

    private func displayColor(for storedColor: NSColor) -> NSColor {
        BoardAppearance.displayColor(
            for: storedColor,
            on: state.boardKind(on: displayUUID)
        ).deviceRGBColor
    }

    private func path(for points: [CGPoint], closes: Bool = false) -> CGPath {
        let path = CGMutablePath()
        if let first = points.first {
            path.move(to: first)
            for point in points.dropFirst() {
                path.addLine(to: point)
            }
            if closes, points.count > 2 {
                path.closeSubpath()
            }
        }
        return path
    }

    private func variableWidthPath(
        for stroke: InkStroke,
        samples: [(location: CGPoint, pressure: CGFloat)]
    ) -> CGPath {
        guard samples.count > 1 else { return CGMutablePath() }

        var leftEdge: [CGPoint] = []
        var rightEdge: [CGPoint] = []
        leftEdge.reserveCapacity(samples.count)
        rightEdge.reserveCapacity(samples.count)

        for index in samples.indices {
            let previous = samples[max(index - 1, samples.startIndex)].location
            let next = samples[min(index + 1, samples.index(before: samples.endIndex))].location
            let tangent = CGPoint(x: next.x - previous.x, y: next.y - previous.y)
            let length = hypot(tangent.x, tangent.y)
            let normal = length > 0
                ? CGPoint(x: -tangent.y / length, y: tangent.x / length)
                : .zero
            let halfWidth = effectiveWidth(
                for: stroke,
                pressure: samples[index].pressure
            ) / 2
            leftEdge.append(
                CGPoint(
                    x: samples[index].location.x + normal.x * halfWidth,
                    y: samples[index].location.y + normal.y * halfWidth
                )
            )
            rightEdge.append(
                CGPoint(
                    x: samples[index].location.x - normal.x * halfWidth,
                    y: samples[index].location.y - normal.y * halfWidth
                )
            )
        }

        let path = CGMutablePath()
        path.move(to: leftEdge[0])
        for point in leftEdge.dropFirst() {
            path.addLine(to: point)
        }
        for point in rightEdge.reversed() {
            path.addLine(to: point)
        }
        path.closeSubpath()

        for sample in [samples[0], samples[samples.count - 1]] {
            let width = effectiveWidth(for: stroke, pressure: sample.pressure)
            path.addEllipse(
                in: CGRect(
                    x: sample.location.x - width / 2,
                    y: sample.location.y - width / 2,
                    width: width,
                    height: width
                )
            )
        }
        return path
    }

    private func effectiveWidth(for stroke: InkStroke, pressure: CGFloat) -> CGFloat {
        guard stroke.usesPressure else { return stroke.width }
        return stroke.width * (0.2 + 0.8 * pressure)
    }

    private func inkPoint(for event: NSEvent) -> InkPoint {
        InkPoint(
            location: InkGeometry.normalizedPoint(from: localLocation(for: event), in: bounds.size),
            pressure: isTabletEvent(event) ? CGFloat(event.pressure) : 1,
            timestamp: event.timestamp
        )
    }

    private func localLocation(for event: NSEvent) -> CGPoint {
        let presentedPoint = convert(event.locationInWindow, from: nil)
        guard let sourceRect else { return presentedPoint }
        return ZoomGeometry.originalPoint(
            fromZoomed: presentedPoint,
            in: bounds.size,
            sourceRect: sourceRect
        )
    }

    private var sourceRect: CGRect? {
        presentationSourceRect
    }

    private func updatePresentationTransform(animated: Bool) {
        let transform = presentationSourceRect.map {
            ZoomGeometry.presentationTransform(sourceRect: $0, destinationSize: bounds.size)
        } ?? .identity
        CATransaction.begin()
        if animated {
            CATransaction.setAnimationDuration(ZoomDefaults.previewAnimationDuration)
            CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .easeInEaseOut))
        } else {
            CATransaction.setDisableActions(true)
        }
        transformedInkLayer.setAffineTransform(transform)
        CATransaction.commit()
    }

    private func isTabletEvent(_ event: NSEvent) -> Bool {
        event.subtype == .tabletPoint
    }

    deinit {
        if let inkObserverIdentifier {
            state.removeInkObserver(inkObserverIdentifier)
        }
        if let modeObserverIdentifier {
            state.removeAnnotationModeObserver(modeObserverIdentifier)
        }
        if let toolObserverIdentifier {
            state.removeToolObserver(toolObserverIdentifier)
        }
        if let toolSettingsObserverIdentifier {
            state.removeToolSettingsObserver(toolSettingsObserverIdentifier)
        }
        if let boardObserverIdentifier {
            state.removeBoardObserver(boardObserverIdentifier)
        }
    }
}

private extension NSColor {
    var deviceRGBColor: NSColor {
        usingColorSpace(.deviceRGB) ?? self
    }
}
