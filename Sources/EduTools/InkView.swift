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

    private let state: AppState
    private let displayUUID: String
    private let requestKeyFocus: () -> Void
    private let completedLayer = CALayer()
    private let activeLayer = CALayer()
    private let laserRenderer = LaserRenderer()
    private var activeStroke: ActiveStroke?
    private var trackingAreaReference: NSTrackingArea?
    private var inkObserverIdentifier: UUID?
    private var modeObserverIdentifier: UUID?
    private var toolObserverIdentifier: UUID?

    override var acceptsFirstResponder: Bool { true }

    init(
        frame frameRect: NSRect,
        state: AppState,
        displayUUID: String,
        requestKeyFocus: @escaping () -> Void
    ) {
        self.state = state
        self.displayUUID = displayUUID
        self.requestKeyFocus = requestKeyFocus
        super.init(frame: frameRect)

        wantsLayer = true
        layer?.masksToBounds = true
        layer?.addSublayer(completedLayer)
        layer?.addSublayer(activeLayer)
        layer?.addSublayer(laserRenderer.layer)

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
            }
        }
        toolObserverIdentifier = state.addToolObserver { [weak self] tool in
            if tool != .laser {
                self?.laserRenderer.clear()
            }
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
        completedLayer.frame = bounds
        activeLayer.frame = bounds
        laserRenderer.layer.frame = bounds
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
            rebuildCompletedLayers()
        }
    }

    func refreshForDisplayChange() {
        rebuildCompletedLayers()
    }

    override func mouseEntered(with event: NSEvent) {
        requestKeyFocus()
        if state.currentTool == .laser {
            laserRenderer.move(to: localLocation(for: event))
        }
    }

    override func mouseExited(with event: NSEvent) {
        laserRenderer.hideCurrent()
    }

    override func mouseMoved(with event: NSEvent) {
        requestKeyFocus()
        guard state.isAnnotationModeEnabled, state.currentTool == .laser else { return }
        laserRenderer.move(to: localLocation(for: event))
    }

    override func mouseDown(with event: NSEvent) {
        requestKeyFocus()
        guard state.isAnnotationModeEnabled else { return }

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
        appendActivePoint(from: event)
    }

    override func mouseUp(with event: NSEvent) {
        guard activeStroke != nil else { return }
        appendActivePoint(from: event)
        finishActiveStroke()
    }

    override func keyDown(with event: NSEvent) {
        let keyCode = Int(event.keyCode)
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

        if keyCode == kVK_ANSI_Z, modifiers.contains(.command) {
            if modifiers.contains(.shift) {
                _ = state.redo(on: displayUUID)
            } else {
                _ = state.undo(on: displayUUID)
            }
            return
        }

        let unsupportedModifiers: NSEvent.ModifierFlags = [.command, .control, .option]
        guard modifiers.intersection(unsupportedModifiers).isEmpty else {
            super.keyDown(with: event)
            return
        }

        switch keyCode {
        case kVK_ANSI_P:
            state.selectTool(.pen)
        case kVK_ANSI_H:
            state.selectTool(.highlighter)
        case kVK_ANSI_L:
            state.selectTool(.laser)
        case kVK_ANSI_1, kVK_ANSI_2, kVK_ANSI_3,
             kVK_ANSI_4, kVK_ANSI_5, kVK_ANSI_6:
            let colorKeyCodes = [
                kVK_ANSI_1,
                kVK_ANSI_2,
                kVK_ANSI_3,
                kVK_ANSI_4,
                kVK_ANSI_5,
                kVK_ANSI_6,
            ]
            if let colorIndex = colorKeyCodes.firstIndex(of: keyCode) {
                state.selectColor(at: colorIndex)
            }
        case kVK_ANSI_LeftBracket:
            state.adjustCurrentToolWidth(by: -1)
        case kVK_ANSI_RightBracket:
            state.adjustCurrentToolWidth(by: 1)
        default:
            super.keyDown(with: event)
        }
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
        activeLayer.sublayers?.forEach { $0.removeFromSuperlayer() }
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
        context.setStrokeColor(stroke.color.deviceRGBColor.cgColor)
        context.setFillColor(stroke.color.deviceRGBColor.cgColor)

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
            shape.fillColor = stroke.color.deviceRGBColor.cgColor
            container.addSublayer(shape)
            return
        }

        if !stroke.usesPressure {
            let shape = CAShapeLayer()
            shape.path = path(for: samples.map { $0.location })
            shape.fillColor = nil
            shape.strokeColor = stroke.color.deviceRGBColor.cgColor
            shape.lineWidth = stroke.width
            shape.lineJoin = .round
            shape.lineCap = stroke.tool == .pen ? .round : .butt
            container.addSublayer(shape)
            return
        }

        let shape = CAShapeLayer()
        shape.path = variableWidthPath(for: stroke, samples: samples)
        shape.fillColor = stroke.color.deviceRGBColor.cgColor
        container.addSublayer(shape)
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
    }
}

private extension NSColor {
    var deviceRGBColor: NSColor {
        usingColorSpace(.deviceRGB) ?? self
    }
}
