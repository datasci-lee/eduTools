import AppKit
import Foundation

enum EraserGesture {
    case sweep(points: [CGPoint])
    case area(CGRect)
}

enum EraserEngine {
    static func erase(
        strokes: [InkStroke],
        gesture: EraserGesture,
        settings: EraserSettings,
        canvasSize: CGSize
    ) -> [InkStroke] {
        guard canvasSize.width > 0, canvasSize.height > 0 else {
            NSLog("EduTools: eraser ignored a gesture because the canvas has no size")
            return strokes
        }

        switch (settings.mode, gesture) {
        case let (.partial, .sweep(points)):
            return erasePartialStrokes(
                strokes,
                along: points,
                settings: settings,
                canvasSize: canvasSize
            )
        case let (.stroke, .sweep(points)):
            return eraseWholeStrokes(
                strokes,
                along: points,
                settings: settings,
                canvasSize: canvasSize
            )
        case let (.area, .area(rect)):
            return eraseStrokes(
                strokes,
                in: standardized(rect),
                settings: settings,
                canvasSize: canvasSize
            )
        default:
            NSLog("EduTools: eraser ignored a gesture that did not match its mode")
            return strokes
        }
    }

    private static func erasePartialStrokes(
        _ strokes: [InkStroke],
        along eraserPoints: [CGPoint],
        settings: EraserSettings,
        canvasSize: CGSize
    ) -> [InkStroke] {
        guard !eraserPoints.isEmpty else { return strokes }
        let radius = settings.diameter / 2
        let eraserBounds = bounds(of: eraserPoints).insetBy(dx: -radius, dy: -radius)
        var result: [InkStroke] = []

        for stroke in strokes {
            guard settings.targetFilter.includes(stroke.tool),
                  overlapsOrTouches(
                      strokeBounds(stroke, canvasSize: canvasSize),
                      eraserBounds
                  ),
                  let fragments = partialFragments(
                      of: stroke,
                      along: eraserPoints,
                      radius: radius,
                      canvasSize: canvasSize
                  )
            else {
                result.append(stroke)
                continue
            }
            result.append(contentsOf: fragments)
        }
        return result
    }

    private static func partialFragments(
        of stroke: InkStroke,
        along eraserPoints: [CGPoint],
        radius: CGFloat,
        canvasSize: CGSize
    ) -> [InkStroke]? {
        let samples = resampledPoints(
            stroke.points,
            canvasSize: canvasSize,
            maximumSpacing: max(1, radius / 2)
        )
        guard !samples.isEmpty else { return nil }

        var groups: [[InkPoint]] = []
        var currentGroup: [InkPoint] = []
        var removedPoint = false

        for point in samples {
            let screenPoint = InkGeometry.screenPoint(from: point.location, in: canvasSize)
            let strokeRadius = effectiveWidth(of: stroke, pressure: point.pressure) / 2
            if distance(from: screenPoint, to: eraserPoints) <= radius + strokeRadius {
                removedPoint = true
                if !currentGroup.isEmpty {
                    groups.append(currentGroup)
                    currentGroup.removeAll(keepingCapacity: true)
                }
            } else {
                currentGroup.append(point)
            }
        }
        if !currentGroup.isEmpty {
            groups.append(currentGroup)
        }

        guard removedPoint else { return nil }
        return groups.map { points in
            InkStroke(
                tool: stroke.tool,
                color: stroke.color,
                width: stroke.width,
                usesPressure: stroke.usesPressure,
                points: points
            )
        }
    }

    private static func eraseWholeStrokes(
        _ strokes: [InkStroke],
        along eraserPoints: [CGPoint],
        settings: EraserSettings,
        canvasSize: CGSize
    ) -> [InkStroke] {
        guard !eraserPoints.isEmpty else { return strokes }
        let radius = settings.diameter / 2
        let eraserBounds = bounds(of: eraserPoints).insetBy(dx: -radius, dy: -radius)

        return strokes.filter { stroke in
            guard settings.targetFilter.includes(stroke.tool) else { return true }
            let screenBounds = strokeBounds(stroke, canvasSize: canvasSize)
            guard overlapsOrTouches(screenBounds, eraserBounds) else { return true }

            let strokePoints = stroke.points.map {
                InkGeometry.screenPoint(from: $0.location, in: canvasSize)
            }
            let shouldErase: Bool
            switch settings.hitRule {
            case .touching:
                shouldErase = distance(between: strokePoints, and: eraserPoints)
                    <= radius + stroke.width / 2
            case .contained:
                let samples = resampledPoints(
                    stroke.points,
                    canvasSize: canvasSize,
                    maximumSpacing: max(1, radius / 2)
                )
                let availableRadius = radius - stroke.width / 2
                shouldErase = availableRadius >= 0 && !samples.isEmpty && samples.allSatisfy {
                    let point = InkGeometry.screenPoint(from: $0.location, in: canvasSize)
                    return distance(from: point, to: eraserPoints) <= availableRadius
                }
            }
            return !shouldErase
        }
    }

    private static func eraseStrokes(
        _ strokes: [InkStroke],
        in area: CGRect,
        settings: EraserSettings,
        canvasSize: CGSize
    ) -> [InkStroke] {
        strokes.filter { stroke in
            guard settings.targetFilter.includes(stroke.tool) else { return true }
            let screenBounds = strokeBounds(stroke, canvasSize: canvasSize)
            guard overlapsOrTouches(screenBounds, area) else { return true }

            let shouldErase: Bool
            switch settings.hitRule {
            case .touching:
                let hitArea = area.insetBy(dx: -stroke.width / 2, dy: -stroke.width / 2)
                let points = stroke.points.map {
                    InkGeometry.screenPoint(from: $0.location, in: canvasSize)
                }
                shouldErase = polyline(points, intersects: hitArea)
            case .contained:
                shouldErase = contains(area, screenBounds)
            }
            return !shouldErase
        }
    }

    private static func strokeBounds(_ stroke: InkStroke, canvasSize: CGSize) -> CGRect {
        guard !stroke.pointBounds.isNull else { return .null }
        let pointBounds = CGRect(
            x: stroke.pointBounds.minX * canvasSize.width,
            y: stroke.pointBounds.minY * canvasSize.height,
            width: stroke.pointBounds.width * canvasSize.width,
            height: stroke.pointBounds.height * canvasSize.height
        )
        return pointBounds.insetBy(dx: -stroke.width / 2, dy: -stroke.width / 2)
    }

    private static func resampledPoints(
        _ points: [InkPoint],
        canvasSize: CGSize,
        maximumSpacing: CGFloat
    ) -> [InkPoint] {
        guard let first = points.first else { return [] }
        var result = [first]

        for (start, end) in zip(points, points.dropFirst()) {
            let startPoint = InkGeometry.screenPoint(from: start.location, in: canvasSize)
            let endPoint = InkGeometry.screenPoint(from: end.location, in: canvasSize)
            let segmentLength = hypot(endPoint.x - startPoint.x, endPoint.y - startPoint.y)
            let steps = max(1, Int(ceil(segmentLength / maximumSpacing)))

            for step in 1...steps {
                let fraction = CGFloat(step) / CGFloat(steps)
                result.append(
                    InkPoint(
                        location: CGPoint(
                            x: start.location.x + (end.location.x - start.location.x) * fraction,
                            y: start.location.y + (end.location.y - start.location.y) * fraction
                        ),
                        pressure: start.pressure + (end.pressure - start.pressure) * fraction,
                        timestamp: start.timestamp
                            + (end.timestamp - start.timestamp) * TimeInterval(fraction)
                    )
                )
            }
        }
        return result
    }

    private static func effectiveWidth(of stroke: InkStroke, pressure: CGFloat) -> CGFloat {
        stroke.usesPressure ? stroke.width * (0.2 + 0.8 * pressure) : stroke.width
    }

    private static func bounds(of points: [CGPoint]) -> CGRect {
        points.reduce(CGRect.null) { bounds, point in
            bounds.union(CGRect(origin: point, size: .zero))
        }
    }

    private static func standardized(_ rect: CGRect) -> CGRect {
        CGRect(
            x: min(rect.minX, rect.maxX),
            y: min(rect.minY, rect.maxY),
            width: abs(rect.width),
            height: abs(rect.height)
        )
    }

    private static func contains(_ outer: CGRect, _ inner: CGRect) -> Bool {
        !inner.isNull
            && inner.minX >= outer.minX
            && inner.maxX <= outer.maxX
            && inner.minY >= outer.minY
            && inner.maxY <= outer.maxY
    }

    private static func overlapsOrTouches(_ first: CGRect, _ second: CGRect) -> Bool {
        !first.isNull
            && !second.isNull
            && first.maxX >= second.minX
            && first.minX <= second.maxX
            && first.maxY >= second.minY
            && first.minY <= second.maxY
    }

    private static func polyline(_ points: [CGPoint], intersects rect: CGRect) -> Bool {
        guard let first = points.first else { return false }
        if rect.contains(first) {
            return true
        }
        for (start, end) in zip(points, points.dropFirst()) {
            if rect.contains(end) || segment(start, end, intersects: rect) {
                return true
            }
        }
        return false
    }

    private static func segment(_ start: CGPoint, _ end: CGPoint, intersects rect: CGRect) -> Bool {
        let corners = [
            CGPoint(x: rect.minX, y: rect.minY),
            CGPoint(x: rect.maxX, y: rect.minY),
            CGPoint(x: rect.maxX, y: rect.maxY),
            CGPoint(x: rect.minX, y: rect.maxY),
        ]
        return zip(corners, corners.dropFirst() + [corners[0]]).contains { edgeStart, edgeEnd in
            segmentsIntersect(start, end, edgeStart, edgeEnd)
        }
    }

    private static func distance(from point: CGPoint, to polyline: [CGPoint]) -> CGFloat {
        guard let first = polyline.first else { return .infinity }
        guard polyline.count > 1 else { return hypot(point.x - first.x, point.y - first.y) }
        return zip(polyline, polyline.dropFirst()).reduce(.infinity) { minimum, segment in
            min(minimum, pointToSegmentDistance(point, segment.0, segment.1))
        }
    }

    private static func distance(between first: [CGPoint], and second: [CGPoint]) -> CGFloat {
        guard let firstPoint = first.first, let secondPoint = second.first else { return .infinity }
        if first.count == 1 {
            return distance(from: firstPoint, to: second)
        }
        if second.count == 1 {
            return distance(from: secondPoint, to: first)
        }

        var minimum = CGFloat.infinity
        for firstSegment in zip(first, first.dropFirst()) {
            for secondSegment in zip(second, second.dropFirst()) {
                minimum = min(
                    minimum,
                    segmentDistance(
                        firstSegment.0,
                        firstSegment.1,
                        secondSegment.0,
                        secondSegment.1
                    )
                )
            }
        }
        return minimum
    }

    private static func segmentDistance(
        _ firstStart: CGPoint,
        _ firstEnd: CGPoint,
        _ secondStart: CGPoint,
        _ secondEnd: CGPoint
    ) -> CGFloat {
        if segmentsIntersect(firstStart, firstEnd, secondStart, secondEnd) {
            return 0
        }
        return min(
            pointToSegmentDistance(firstStart, secondStart, secondEnd),
            pointToSegmentDistance(firstEnd, secondStart, secondEnd),
            pointToSegmentDistance(secondStart, firstStart, firstEnd),
            pointToSegmentDistance(secondEnd, firstStart, firstEnd)
        )
    }

    private static func pointToSegmentDistance(
        _ point: CGPoint,
        _ start: CGPoint,
        _ end: CGPoint
    ) -> CGFloat {
        let delta = CGPoint(x: end.x - start.x, y: end.y - start.y)
        let lengthSquared = delta.x * delta.x + delta.y * delta.y
        guard lengthSquared > 0 else { return hypot(point.x - start.x, point.y - start.y) }
        let projection = max(
            0,
            min(1, ((point.x - start.x) * delta.x + (point.y - start.y) * delta.y) / lengthSquared)
        )
        let nearest = CGPoint(x: start.x + delta.x * projection, y: start.y + delta.y * projection)
        return hypot(point.x - nearest.x, point.y - nearest.y)
    }

    private static func segmentsIntersect(
        _ firstStart: CGPoint,
        _ firstEnd: CGPoint,
        _ secondStart: CGPoint,
        _ secondEnd: CGPoint
    ) -> Bool {
        let epsilon = CGFloat(0.000_001)
        let d1 = cross(firstStart, firstEnd, secondStart)
        let d2 = cross(firstStart, firstEnd, secondEnd)
        let d3 = cross(secondStart, secondEnd, firstStart)
        let d4 = cross(secondStart, secondEnd, firstEnd)

        if ((d1 > epsilon && d2 < -epsilon) || (d1 < -epsilon && d2 > epsilon)),
           ((d3 > epsilon && d4 < -epsilon) || (d3 < -epsilon && d4 > epsilon)) {
            return true
        }
        return (abs(d1) <= epsilon && point(secondStart, liesOn: firstStart, firstEnd))
            || (abs(d2) <= epsilon && point(secondEnd, liesOn: firstStart, firstEnd))
            || (abs(d3) <= epsilon && point(firstStart, liesOn: secondStart, secondEnd))
            || (abs(d4) <= epsilon && point(firstEnd, liesOn: secondStart, secondEnd))
    }

    private static func cross(_ start: CGPoint, _ end: CGPoint, _ point: CGPoint) -> CGFloat {
        (end.x - start.x) * (point.y - start.y)
            - (end.y - start.y) * (point.x - start.x)
    }

    private static func point(_ point: CGPoint, liesOn start: CGPoint, _ end: CGPoint) -> Bool {
        point.x >= min(start.x, end.x) - 0.000_001
            && point.x <= max(start.x, end.x) + 0.000_001
            && point.y >= min(start.y, end.y) - 0.000_001
            && point.y <= max(start.y, end.y) + 0.000_001
    }
}
