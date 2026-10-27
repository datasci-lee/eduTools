import AppKit
import Foundation

enum ArrowStyle: String, Equatable {
    case single
    case double

    var displayName: String {
        switch self {
        case .single: return "한쪽"
        case .double: return "양쪽"
        }
    }

    var next: ArrowStyle {
        self == .single ? .double : .single
    }
}

enum InkShapeKind: Equatable {
    case line
    case arrow(ArrowStyle)
    case rectangle
    case ellipse
}

struct InkTextContent: Equatable {
    let string: String
    let fontSize: CGFloat
}

enum InkStrokeContent: Equatable {
    case freehand
    case shape(InkShapeKind)
    case text(InkTextContent)
    case cover
}

enum EraserAreaShape: CaseIterable {
    case rectangle
    case lasso

    var displayName: String {
        switch self {
        case .rectangle: return "사각형"
        case .lasso: return "올가미"
        }
    }

    var next: EraserAreaShape {
        self == .rectangle ? .lasso : .rectangle
    }
}

enum ShapeRecognizer {
    static func recognize(points: [CGPoint]) -> InkShapeKind? {
        guard points.count >= 2 else { return nil }
        let bounds = points.reduce(CGRect.null) {
            $0.union(CGRect(origin: $1, size: .zero))
        }
        let diagonal = hypot(bounds.width, bounds.height)
        guard diagonal > 0 else { return nil }

        let pathLength = zip(points, points.dropFirst()).reduce(CGFloat.zero) {
            $0 + distance($1.0, $1.1)
        }
        let endpointDistance = distance(points[0], points[points.count - 1])
        if endpointDistance >= diagonal * 0.8,
           maximumLineDeviation(points) <= diagonal * 0.08,
           pathLength <= endpointDistance * 1.18
        {
            return .line
        }

        guard points.count >= 5,
              endpointDistance <= diagonal * 0.24,
              bounds.width >= diagonal * 0.18,
              bounds.height >= diagonal * 0.18
        else {
            return nil
        }

        if resemblesRectangle(points, bounds: bounds, pathLength: pathLength) {
            return .rectangle
        }
        if resemblesEllipse(points, bounds: bounds, pathLength: pathLength) {
            return .ellipse
        }
        return nil
    }

    static func correctedStroke(from stroke: InkStroke, canvasSize: CGSize) -> InkStroke? {
        guard stroke.content == .freehand,
              canvasSize.width > 0,
              canvasSize.height > 0
        else {
            return nil
        }
        let screenPoints = stroke.points.map {
            InkGeometry.screenPoint(from: $0.location, in: canvasSize)
        }
        guard let shape = recognize(points: screenPoints),
              let first = stroke.points.first,
              let last = stroke.points.last
        else {
            return nil
        }

        let correctedPoints: [InkPoint]
        switch shape {
        case .line:
            correctedPoints = [first, last]
        case .rectangle, .ellipse:
            let bounds = screenPoints.reduce(CGRect.null) {
                $0.union(CGRect(origin: $1, size: .zero))
            }
            correctedPoints = [
                InkPoint(
                    location: InkGeometry.normalizedPoint(
                        from: CGPoint(x: bounds.minX, y: bounds.minY),
                        in: canvasSize
                    ),
                    pressure: 1,
                    timestamp: first.timestamp
                ),
                InkPoint(
                    location: InkGeometry.normalizedPoint(
                        from: CGPoint(x: bounds.maxX, y: bounds.maxY),
                        in: canvasSize
                    ),
                    pressure: 1,
                    timestamp: last.timestamp
                ),
            ]
        case .arrow:
            return nil
        }

        return InkStroke(
            identifier: stroke.identifier,
            tool: .shape,
            color: stroke.color,
            width: stroke.width,
            usesPressure: false,
            points: correctedPoints,
            content: .shape(shape)
        )
    }

    private static func resemblesRectangle(
        _ points: [CGPoint],
        bounds: CGRect,
        pathLength: CGFloat
    ) -> Bool {
        let shorterSide = min(bounds.width, bounds.height)
        let edgeTolerance = shorterSide * 0.14
        let cornerTolerance = hypot(bounds.width, bounds.height) * 0.22
        let perimeter = 2 * (bounds.width + bounds.height)
        guard pathLength >= perimeter * 0.72,
              pathLength <= perimeter * 1.35,
              points.allSatisfy({ point in
                  min(
                      abs(point.x - bounds.minX),
                      abs(point.x - bounds.maxX),
                      abs(point.y - bounds.minY),
                      abs(point.y - bounds.maxY)
                  ) <= edgeTolerance
              })
        else {
            return false
        }

        let corners = [
            CGPoint(x: bounds.minX, y: bounds.minY),
            CGPoint(x: bounds.maxX, y: bounds.minY),
            CGPoint(x: bounds.maxX, y: bounds.maxY),
            CGPoint(x: bounds.minX, y: bounds.maxY),
        ]
        return corners.allSatisfy { corner in
            points.contains { distance($0, corner) <= cornerTolerance }
        }
    }

    private static func resemblesEllipse(
        _ points: [CGPoint],
        bounds: CGRect,
        pathLength: CGFloat
    ) -> Bool {
        let aspect = bounds.width / bounds.height
        guard aspect >= 0.65, aspect <= 1.55 else { return false }
        let radiusX = bounds.width / 2
        let radiusY = bounds.height / 2
        let center = CGPoint(x: bounds.midX, y: bounds.midY)
        let errors = points.map { point -> CGFloat in
            let normalizedX = (point.x - center.x) / radiusX
            let normalizedY = (point.y - center.y) / radiusY
            return abs(hypot(normalizedX, normalizedY) - 1)
        }
        let averageError = errors.reduce(0, +) / CGFloat(errors.count)
        let circumference = .pi * (
            3 * (radiusX + radiusY)
                - sqrt((3 * radiusX + radiusY) * (radiusX + 3 * radiusY))
        )
        return averageError <= 0.16
            && (errors.max() ?? .infinity) <= 0.38
            && pathLength >= circumference * 0.68
            && pathLength <= circumference * 1.35
    }

    private static func maximumLineDeviation(_ points: [CGPoint]) -> CGFloat {
        guard let first = points.first, let last = points.last else { return .infinity }
        return points.map { pointToSegmentDistance($0, first, last) }.max() ?? .infinity
    }

    private static func pointToSegmentDistance(
        _ point: CGPoint,
        _ start: CGPoint,
        _ end: CGPoint
    ) -> CGFloat {
        let delta = CGPoint(x: end.x - start.x, y: end.y - start.y)
        let lengthSquared = delta.x * delta.x + delta.y * delta.y
        guard lengthSquared > 0 else { return distance(point, start) }
        let projection = max(
            0,
            min(1, ((point.x - start.x) * delta.x + (point.y - start.y) * delta.y) / lengthSquared)
        )
        return distance(
            point,
            CGPoint(x: start.x + delta.x * projection, y: start.y + delta.y * projection)
        )
    }

    private static func distance(_ first: CGPoint, _ second: CGPoint) -> CGFloat {
        hypot(first.x - second.x, first.y - second.y)
    }
}

enum InkItemGeometry {
    static func screenPoints(for stroke: InkStroke, canvasSize: CGSize) -> [CGPoint] {
        let points = stroke.points.map {
            InkGeometry.screenPoint(from: $0.location, in: canvasSize)
        }
        guard let first = points.first, let last = points.last else { return [] }

        switch stroke.content {
        case .freehand:
            return points
        case let .shape(kind):
            switch kind {
            case .line, .arrow:
                return [first, last]
            case .rectangle:
                let bounds = CGRect(
                    x: min(first.x, last.x),
                    y: min(first.y, last.y),
                    width: abs(last.x - first.x),
                    height: abs(last.y - first.y)
                )
                let corners = [
                    CGPoint(x: bounds.minX, y: bounds.minY),
                    CGPoint(x: bounds.maxX, y: bounds.minY),
                    CGPoint(x: bounds.maxX, y: bounds.maxY),
                    CGPoint(x: bounds.minX, y: bounds.maxY),
                ]
                return corners + [corners[0]]
            case .ellipse:
                let center = CGPoint(x: (first.x + last.x) / 2, y: (first.y + last.y) / 2)
                let radiusX = abs(last.x - first.x) / 2
                let radiusY = abs(last.y - first.y) / 2
                return (0...32).map { index in
                    let angle = CGFloat(index) / 32 * 2 * .pi
                    return CGPoint(
                        x: center.x + cos(angle) * radiusX,
                        y: center.y + sin(angle) * radiusY
                    )
                }
            }
        case .text, .cover:
            let bounds = CGRect(
                x: min(first.x, last.x),
                y: min(first.y, last.y),
                width: abs(last.x - first.x),
                height: abs(last.y - first.y)
            )
            let corners = [
                CGPoint(x: bounds.minX, y: bounds.minY),
                CGPoint(x: bounds.maxX, y: bounds.minY),
                CGPoint(x: bounds.maxX, y: bounds.maxY),
                CGPoint(x: bounds.minX, y: bounds.maxY),
            ]
            return corners + [corners[0]]
        }
    }

    static func bounds(for stroke: InkStroke, canvasSize: CGSize) -> CGRect {
        let points = screenPoints(for: stroke, canvasSize: canvasSize)
        guard !points.isEmpty else { return .null }
        return points.reduce(CGRect.null) {
            $0.union(CGRect(origin: $1, size: .zero))
        }.insetBy(dx: -stroke.width / 2, dy: -stroke.width / 2)
    }
}

enum LassoGeometry {
    static func contains(_ point: CGPoint, polygon: [CGPoint]) -> Bool {
        guard polygon.count >= 3 else { return false }
        if polygonEdges(polygon).contains(where: { pointOnSegment(point, $0.0, $0.1) }) {
            return true
        }

        var isInside = false
        var previous = polygon[polygon.count - 1]
        for current in polygon {
            let crosses = (current.y > point.y) != (previous.y > point.y)
            if crosses {
                let crossingX = (previous.x - current.x) * (point.y - current.y)
                    / (previous.y - current.y) + current.x
                if point.x < crossingX {
                    isInside.toggle()
                }
            }
            previous = current
        }
        return isInside
    }

    static func selects(
        _ stroke: InkStroke,
        polygon: [CGPoint],
        hitRule: EraserHitRule,
        canvasSize: CGSize
    ) -> Bool {
        guard polygon.count >= 3 else { return false }
        let itemPoints = InkItemGeometry.screenPoints(for: stroke, canvasSize: canvasSize)
        guard !itemPoints.isEmpty else { return false }

        switch hitRule {
        case .contained:
            return itemPoints.allSatisfy { contains($0, polygon: polygon) }
        case .touching:
            if itemPoints.contains(where: { contains($0, polygon: polygon) }) {
                return true
            }
            let itemBounds = InkItemGeometry.bounds(for: stroke, canvasSize: canvasSize)
            if polygon.contains(where: itemBounds.contains) {
                return true
            }
            return polylineEdges(itemPoints).contains { itemEdge in
                polygonEdges(polygon).contains { polygonEdge in
                    segmentsIntersect(itemEdge.0, itemEdge.1, polygonEdge.0, polygonEdge.1)
                }
            }
        }
    }

    private static func polygonEdges(_ polygon: [CGPoint]) -> [(CGPoint, CGPoint)] {
        Array(zip(polygon, polygon.dropFirst() + [polygon[0]]))
    }

    private static func polylineEdges(_ points: [CGPoint]) -> [(CGPoint, CGPoint)] {
        guard points.count > 1 else { return [] }
        return Array(zip(points, points.dropFirst()))
    }

    private static func segmentsIntersect(
        _ firstStart: CGPoint,
        _ firstEnd: CGPoint,
        _ secondStart: CGPoint,
        _ secondEnd: CGPoint
    ) -> Bool {
        let firstA = cross(firstStart, firstEnd, secondStart)
        let firstB = cross(firstStart, firstEnd, secondEnd)
        let secondA = cross(secondStart, secondEnd, firstStart)
        let secondB = cross(secondStart, secondEnd, firstEnd)
        let epsilon = CGFloat(0.000_001)
        if ((firstA > epsilon && firstB < -epsilon) || (firstA < -epsilon && firstB > epsilon)),
           ((secondA > epsilon && secondB < -epsilon) || (secondA < -epsilon && secondB > epsilon)) {
            return true
        }
        return (abs(firstA) <= epsilon && pointOnSegment(secondStart, firstStart, firstEnd))
            || (abs(firstB) <= epsilon && pointOnSegment(secondEnd, firstStart, firstEnd))
            || (abs(secondA) <= epsilon && pointOnSegment(firstStart, secondStart, secondEnd))
            || (abs(secondB) <= epsilon && pointOnSegment(firstEnd, secondStart, secondEnd))
    }

    private static func cross(_ start: CGPoint, _ end: CGPoint, _ point: CGPoint) -> CGFloat {
        (end.x - start.x) * (point.y - start.y)
            - (end.y - start.y) * (point.x - start.x)
    }

    private static func pointOnSegment(
        _ point: CGPoint,
        _ start: CGPoint,
        _ end: CGPoint
    ) -> Bool {
        abs(cross(start, end, point)) <= 0.000_001
            && point.x >= min(start.x, end.x) - 0.000_001
            && point.x <= max(start.x, end.x) + 0.000_001
            && point.y >= min(start.y, end.y) - 0.000_001
            && point.y <= max(start.y, end.y) + 0.000_001
    }
}

enum ScreenCoverEngine {
    static func removingTopmostCover(
        at normalizedPoint: CGPoint,
        from strokes: [InkStroke]
    ) -> [InkStroke] {
        guard let index = strokes.lastIndex(where: { stroke in
            guard stroke.content == .cover, stroke.points.count >= 2 else { return false }
            return normalizedBounds(for: stroke).contains(normalizedPoint)
        }) else {
            return strokes
        }

        var result = strokes
        result.remove(at: index)
        return result
    }

    private static func normalizedBounds(for stroke: InkStroke) -> CGRect {
        stroke.points.reduce(CGRect.null) { bounds, point in
            bounds.union(CGRect(origin: point.location, size: .zero))
        }.standardized
    }
}

enum SelectionEngine {
    static func selectedIdentifiers(
        from strokes: [InkStroke],
        polygon: [CGPoint],
        hitRule: EraserHitRule,
        canvasSize: CGSize
    ) -> Set<UUID> {
        Set(strokes.compactMap { stroke in
            LassoGeometry.selects(
                stroke,
                polygon: polygon,
                hitRule: hitRule,
                canvasSize: canvasSize
            ) ? stroke.identifier : nil
        })
    }
}

enum SelectionTransform {
    static func moved(_ strokes: [InkStroke], by delta: CGPoint) -> [InkStroke] {
        strokes.map { stroke in
            replacingPoints(
                in: stroke,
                with: stroke.points.map { point in
                    InkPoint(
                        location: CGPoint(
                            x: point.location.x + delta.x,
                            y: point.location.y + delta.y
                        ),
                        pressure: point.pressure,
                        timestamp: point.timestamp
                    )
                }
            )
        }
    }

    static func resized(
        _ strokes: [InkStroke],
        from sourceBounds: CGRect,
        to destinationBounds: CGRect
    ) -> [InkStroke] {
        guard sourceBounds.width > 0, sourceBounds.height > 0 else { return strokes }
        let scaleX = destinationBounds.width / sourceBounds.width
        let scaleY = destinationBounds.height / sourceBounds.height
        return strokes.map { stroke in
            var content = stroke.content
            if case let .text(text) = content {
                let fontScale = max(0.1, min(abs(scaleX), abs(scaleY)))
                content = .text(
                    InkTextContent(
                        string: text.string,
                        fontSize: max(1, text.fontSize * fontScale)
                    )
                )
            }
            return replacingPoints(
                in: stroke,
                with: stroke.points.map { point in
                    InkPoint(
                        location: CGPoint(
                            x: destinationBounds.minX
                                + (point.location.x - sourceBounds.minX) * scaleX,
                            y: destinationBounds.minY
                                + (point.location.y - sourceBounds.minY) * scaleY
                        ),
                        pressure: point.pressure,
                        timestamp: point.timestamp
                    )
                },
                content: content
            )
        }
    }

    private static func replacingPoints(
        in stroke: InkStroke,
        with points: [InkPoint],
        content: InkStrokeContent? = nil
    ) -> InkStroke {
        InkStroke(
            identifier: stroke.identifier,
            tool: stroke.tool,
            color: stroke.color,
            width: stroke.width,
            usesPressure: stroke.usesPressure,
            points: points,
            content: content ?? stroke.content
        )
    }
}
