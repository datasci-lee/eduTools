import AppKit
import Carbon
import QuartzCore

private final class LaserRenderer {
    let layer = CALayer()

    private var currentDot: CAShapeLayer?
    private var previousLocation: CGPoint?

    func move(to location: CGPoint) {
        if let previousLocation {
            addFadingDot(at: previousLocation)
        }

        let dot = currentDot ?? makeDot(radius: 8, opacity: 1)
        if currentDot == nil {
            currentDot = dot
            layer.addSublayer(dot)
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        dot.position = location
        dot.opacity = 1
        CATransaction.commit()
        previousLocation = location
    }

    func hideCurrent() {
        currentDot?.removeFromSuperlayer()
        currentDot = nil
        previousLocation = nil
    }

    func clear() {
        layer.sublayers?.forEach { $0.removeFromSuperlayer() }
        currentDot = nil
        previousLocation = nil
    }

    private func addFadingDot(at location: CGPoint) {
        let dot = makeDot(radius: 5, opacity: 0.7)
        dot.position = location
        layer.addSublayer(dot)

        CATransaction.begin()
        CATransaction.setAnimationDuration(InkDefaults.laserTrailDuration)
        CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .easeOut))
        CATransaction.setCompletionBlock { [weak dot] in
            dot?.removeFromSuperlayer()
        }
        dot.opacity = 0
        CATransaction.commit()
    }

    private func makeDot(radius: CGFloat, opacity: Float) -> CAShapeLayer {
        let dot = CAShapeLayer()
        let diameter = radius * 2
        dot.bounds = CGRect(x: 0, y: 0, width: diameter, height: diameter)
        dot.path = CGPath(ellipseIn: dot.bounds, transform: nil)
        dot.fillColor = NSColor.systemRed.cgColor
        dot.shadowColor = NSColor.systemRed.cgColor
        dot.shadowOpacity = 0.9
        dot.shadowRadius = radius
        dot.shadowOffset = .zero
        dot.opacity = opacity
        return dot
    }
}

final class InkView: NSView {
    private struct ActiveStroke {
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

    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let displayUUID: String
    private let requestKeyFocus: () -> Void
    private let boardBackgroundLayer = CALayer()
    private let boardPatternLayer = CAShapeLayer()
    private let completedLayer = CALayer()
    private let activeLayer = CALayer()
    private let laserRenderer = LaserRenderer()
    private let boardPageIndicatorLayer = CATextLayer()
    private let eraserCursorLayer = CAShapeLayer()
    private let eraserModeLayer = CATextLayer()
    private var activeStroke: ActiveStroke?
    private var activeEraser: ActiveEraser?
    private var lastPointerLocation: CGPoint?
    private var eraserModePresentation = 0
    private var trackingAreaReference: NSTrackingArea?
    private var inkObserverIdentifier: UUID?
    private var modeObserverIdentifier: UUID?
    private var toolObserverIdentifier: UUID?
    private var boardObserverIdentifier: UUID?

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
        layer?.addSublayer(completedLayer)
        layer?.addSublayer(activeLayer)
        layer?.addSublayer(laserRenderer.layer)
        layer?.addSublayer(boardPageIndicatorLayer)
        configureEraserLayers()
        layer?.addSublayer(eraserCursorLayer)
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
                self?.laserRenderer.clear()
                self?.eraserCursorLayer.isHidden = true
            }
        }
        toolObserverIdentifier = state.addToolObserver { [weak self] tool in
            if tool != .laser {
                self?.laserRenderer.clear()
            }
            if tool == .eraser, let location = self?.lastPointerLocation {
                self?.showEraserCursor(at: location)
            } else if tool != .eraser {
                self?.eraserCursorLayer.isHidden = true
            }
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
        completedLayer.frame = bounds
        activeLayer.frame = bounds
        laserRenderer.layer.frame = bounds
        layoutEraserModeLayer()
        CATransaction.commit()
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

    override func mouseEntered(with event: NSEvent) {
        requestKeyFocus()
        let location = localLocation(for: event)
        lastPointerLocation = location
        if usesEraser(for: event) {
            showEraserCursor(at: location)
        } else if state.currentTool == .laser {
            laserRenderer.move(to: location)
        }
    }

    override func mouseExited(with event: NSEvent) {
        laserRenderer.hideCurrent()
        eraserCursorLayer.isHidden = true
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
                laserRenderer.move(to: location)
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
            return
        case .eraser:
            return
        }

        let point = inkPoint(for: event)
        activeStroke = ActiveStroke(
            tool: strokeTool,
            color: state.selectedColor,
            width: width,
            isStraight: event.modifierFlags.contains(.shift),
            usesPressure: isTabletEvent(event),
            points: [point]
        )
        renderActiveStroke()
    }

    override func mouseDragged(with event: NSEvent) {
        let location = localLocation(for: event)
        lastPointerLocation = location
        if activeEraser != nil {
            appendActiveEraserPoint(location)
            showEraserCursor(at: location)
            return
        }
        appendActivePoint(from: event)
    }

    override func mouseUp(with event: NSEvent) {
        if activeEraser != nil {
            let location = localLocation(for: event)
            lastPointerLocation = location
            appendActiveEraserPoint(location)
            finishActiveEraser()
            showEraserCursor(at: location)
            return
        }
        guard activeStroke != nil else { return }
        appendActivePoint(from: event)
        finishActiveStroke()
    }

    override func scrollWheel(with event: NSEvent) {
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

    private func appendActivePoint(from event: NSEvent) {
        guard var stroke = activeStroke else { return }
        let point = inkPoint(for: event)
        if stroke.points.last?.location != point.location || stroke.points.count == 1 {
            stroke.points.append(point)
        }
        stroke.usesPressure = stroke.usesPressure || isTabletEvent(event)
        activeStroke = stroke
        renderActiveStroke()
    }

    private func beginEraser(at point: CGPoint) {
        activeStroke = nil
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
            let last = activeEraser.points.last ?? first
            gesture = .area(
                CGRect(
                    x: first.x,
                    y: first.y,
                    width: last.x - first.x,
                    height: last.y - first.y
                )
            )
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
        let points = effectivePoints(for: activeStroke)
        let stroke = InkStroke(
            tool: activeStroke.tool,
            color: activeStroke.color,
            width: activeStroke.width,
            usesPressure: activeStroke.tool == .pen && activeStroke.usesPressure,
            points: points
        )
        self.activeStroke = nil
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        state.addStroke(stroke, to: displayUUID)
    }

    private func cancelActiveStroke() {
        activeStroke = nil
        activeEraser = nil
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
    }

    private func renderActiveEraser() {
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        guard let activeEraser,
              activeEraser.settings.mode == .area,
              let first = activeEraser.points.first,
              let last = activeEraser.points.last
        else {
            return
        }

        let rect = CGRect(
            x: first.x,
            y: first.y,
            width: last.x - first.x,
            height: last.y - first.y
        ).standardized
        let shape = CAShapeLayer()
        shape.frame = bounds
        shape.path = CGPath(rect: rect, transform: nil)
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

        boardBackgroundLayer.backgroundColor = BoardAppearance.backgroundColor(
            for: kind,
            blackboardBackground: state.blackboardBackground
        ).cgColor
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
        let stroke = InkStroke(
            tool: activeStroke.tool,
            color: activeStroke.color,
            width: activeStroke.width,
            usesPressure: activeStroke.tool == .pen && activeStroke.usesPressure,
            points: effectivePoints(for: activeStroke)
        )
        let container = makeVectorLayer(for: stroke)
        activeLayer.addSublayer(container)
    }

    private func rebuildCompletedLayers() {
        completedLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
        for stroke in state.strokes(for: displayUUID) {
            addCompletedStrokeLayer(for: stroke)
        }
    }

    private func addCompletedStrokeLayer(for stroke: InkStroke) {
        guard bounds.width > 0, bounds.height > 0,
              let layer = makeBitmapLayer(for: stroke)
        else {
            return
        }
        completedLayer.addSublayer(layer)
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
            cachedLayer.opacity = Float(InkDefaults.highlighterOpacity)
            cachedLayer.compositingFilter = "multiplyBlendMode"
        }
        return cachedLayer
    }

    private func makeVectorLayer(for stroke: InkStroke) -> CALayer {
        let container = CALayer()
        container.frame = bounds
        let samples = renderedSamples(for: stroke)
        addShapeLayers(for: stroke, samples: samples, to: container)
        if stroke.tool == .highlighter {
            container.opacity = Float(InkDefaults.highlighterOpacity)
            container.compositingFilter = "multiplyBlendMode"
        }
        return container
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

    private func path(for points: [CGPoint]) -> CGPath {
        let path = CGMutablePath()
        if let first = points.first {
            path.move(to: first)
            for point in points.dropFirst() {
                path.addLine(to: point)
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
        convert(event.locationInWindow, from: nil)
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
