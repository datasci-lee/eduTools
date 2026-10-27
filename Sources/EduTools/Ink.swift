import AppKit
import Foundation

enum InkTool: String {
    case pen
    case highlighter
    case laser
    case eraser

    var displayName: String {
        switch self {
        case .pen: return "펜"
        case .highlighter: return "형광펜"
        case .laser: return "레이저"
        case .eraser: return "지우개"
        }
    }
}

enum InkStrokeTool: Equatable {
    case pen
    case highlighter
}

enum InkDefaults {
    static let colors: [NSColor] = [
        .systemRed,
        .systemOrange,
        .systemYellow,
        .systemGreen,
        .systemBlue,
        .white,
    ]
    static let initialColorIndex = 0
    static let penWidth: CGFloat = 3
    static let highlighterWidth: CGFloat = 18
    static let highlighterOpacity: CGFloat = 0.4
    static let minimumWidth: CGFloat = 1
    static let maximumWidth: CGFloat = 30
    static let laserTrailDuration: TimeInterval = 1
    static let undoLimit = 100
}

enum EraserMode: CaseIterable {
    case partial
    case stroke
    case area

    var displayName: String {
        switch self {
        case .partial: return "부분 지우기"
        case .stroke: return "획 단위 지우기"
        case .area: return "영역 지우기"
        }
    }

    var next: EraserMode {
        switch self {
        case .partial: return .stroke
        case .stroke: return .area
        case .area: return .partial
        }
    }
}

enum EraserHitRule {
    case touching
    case contained
}

enum EraserTargetFilter {
    case all
    case penOnly
    case highlighterOnly

    func includes(_ tool: InkStrokeTool) -> Bool {
        switch self {
        case .all:
            return true
        case .penOnly:
            return tool == .pen
        case .highlighterOnly:
            return tool == .highlighter
        }
    }
}

enum EraserDefaults {
    static let minimumDiameter: CGFloat = 8
    static let maximumDiameter: CGFloat = 200
    static let presetDiameters: [CGFloat] = [12, 24, 60]
    static let defaultDiameter = presetDiameters[1]
    static let diameterAdjustmentStep: CGFloat = 2
    static let defaultMode = EraserMode.partial
    static let defaultHitRule = EraserHitRule.touching
    static let defaultTargetFilter = EraserTargetFilter.all
}

struct EraserSettings {
    let diameter: CGFloat
    let mode: EraserMode
    let hitRule: EraserHitRule
    let targetFilter: EraserTargetFilter
}

struct InkPoint: Equatable {
    let location: CGPoint
    let pressure: CGFloat
    let timestamp: TimeInterval

    init(location: CGPoint, pressure: CGFloat, timestamp: TimeInterval) {
        self.location = CGPoint(
            x: min(max(location.x, 0), 1),
            y: min(max(location.y, 0), 1)
        )
        self.pressure = min(max(pressure, 0), 1)
        self.timestamp = timestamp
    }
}

struct InkStroke {
    let identifier: UUID
    let tool: InkStrokeTool
    let color: NSColor
    let width: CGFloat
    let usesPressure: Bool
    let points: [InkPoint]
    let pointBounds: CGRect

    init(
        identifier: UUID = UUID(),
        tool: InkStrokeTool,
        color: NSColor,
        width: CGFloat,
        usesPressure: Bool,
        points: [InkPoint]
    ) {
        self.identifier = identifier
        self.tool = tool
        self.color = color
        self.width = width
        self.usesPressure = usesPressure
        self.points = points
        pointBounds = points.reduce(CGRect.null) { bounds, point in
            bounds.union(CGRect(origin: point.location, size: .zero))
        }
    }
}

private enum InkLayerKey: Hashable {
    case legacy
    case display(String)
}

enum InkCommand {
    case addStroke(InkStroke)
    case addDisplayStroke(displayUUID: String, stroke: InkStroke)
    case replaceDisplayStrokes(
        displayUUID: String,
        before: [InkStroke],
        after: [InkStroke]
    )
    case replaceAllDisplayStrokes(
        before: [String: [InkStroke]],
        after: [String: [InkStroke]]
    )

    fileprivate func apply(to layers: inout [InkLayerKey: [InkStroke]]) {
        switch self {
        case let .addStroke(stroke):
            layers[.legacy, default: []].append(stroke)
        case let .addDisplayStroke(displayUUID, stroke):
            layers[.display(displayUUID), default: []].append(stroke)
        case let .replaceDisplayStrokes(displayUUID, _, after):
            layers[.display(displayUUID)] = after
        case let .replaceAllDisplayStrokes(_, after):
            replaceDisplayLayers(in: &layers, with: after)
        }
    }

    fileprivate func revert(on layers: inout [InkLayerKey: [InkStroke]]) {
        switch self {
        case let .addStroke(stroke):
            guard let index = layers[.legacy]?.lastIndex(where: {
                $0.identifier == stroke.identifier
            }) else {
                return
            }
            layers[.legacy]?.remove(at: index)
        case let .addDisplayStroke(displayUUID, stroke):
            let key = InkLayerKey.display(displayUUID)
            guard let index = layers[key]?.lastIndex(where: {
                $0.identifier == stroke.identifier
            }) else {
                return
            }
            layers[key]?.remove(at: index)
        case let .replaceDisplayStrokes(displayUUID, before, _):
            layers[.display(displayUUID)] = before
        case let .replaceAllDisplayStrokes(before, _):
            replaceDisplayLayers(in: &layers, with: before)
        }
    }

    private func replaceDisplayLayers(
        in layers: inout [InkLayerKey: [InkStroke]],
        with replacements: [String: [InkStroke]]
    ) {
        for key in Array(layers.keys) {
            if case .display = key {
                layers.removeValue(forKey: key)
            }
        }
        for (displayUUID, strokes) in replacements {
            layers[.display(displayUUID)] = strokes
        }
    }
}

final class InkLayerState {
    private var layers: [InkLayerKey: [InkStroke]] = [:]
    private var undoCommands: [InkCommand] = []
    private var redoCommands: [InkCommand] = []
    private let undoLimit: Int

    var strokes: [InkStroke] {
        layers[.legacy] ?? []
    }

    var allDisplayStrokes: [String: [InkStroke]] {
        var result: [String: [InkStroke]] = [:]
        for (key, strokes) in layers {
            if case let .display(displayUUID) = key {
                result[displayUUID] = strokes
            }
        }
        return result
    }

    init(undoLimit: Int = InkDefaults.undoLimit) {
        self.undoLimit = undoLimit
    }

    func execute(_ command: InkCommand) {
        command.apply(to: &layers)
        undoCommands.append(command)
        if undoCommands.count > undoLimit {
            undoCommands.removeFirst(undoCommands.count - undoLimit)
        }
        redoCommands.removeAll(keepingCapacity: true)
    }

    @discardableResult
    func undo() -> Bool {
        guard let command = undoCommands.popLast() else { return false }
        command.revert(on: &layers)
        redoCommands.append(command)
        return true
    }

    @discardableResult
    func redo() -> Bool {
        guard let command = redoCommands.popLast() else { return false }
        command.apply(to: &layers)
        undoCommands.append(command)
        return true
    }

    var canRedo: Bool {
        !redoCommands.isEmpty
    }

    func strokes(for displayUUID: String) -> [InkStroke] {
        layers[.display(displayUUID)] ?? []
    }
}

enum InkGeometry {
    static func normalizedPoint(from point: CGPoint, in size: CGSize) -> CGPoint {
        guard size.width > 0, size.height > 0 else { return .zero }
        return CGPoint(x: point.x / size.width, y: point.y / size.height)
    }

    static func screenPoint(from point: CGPoint, in size: CGSize) -> CGPoint {
        CGPoint(x: point.x * size.width, y: point.y * size.height)
    }

    static func smoothedPoints(_ points: [InkPoint], subdivisions: Int = 8) -> [InkPoint] {
        guard points.count > 1, subdivisions > 0 else { return points }

        var result: [InkPoint] = []
        result.reserveCapacity((points.count - 1) * subdivisions + 1)

        for index in 0..<(points.count - 1) {
            let p0 = points[max(index - 1, 0)]
            let p1 = points[index]
            let p2 = points[index + 1]
            let p3 = points[min(index + 2, points.count - 1)]

            for step in 0..<subdivisions {
                let t = CGFloat(step) / CGFloat(subdivisions)
                let t2 = t * t
                let t3 = t2 * t
                let x = 0.5 * (
                    2 * p1.location.x
                        + (-p0.location.x + p2.location.x) * t
                        + (2 * p0.location.x - 5 * p1.location.x + 4 * p2.location.x - p3.location.x) * t2
                        + (-p0.location.x + 3 * p1.location.x - 3 * p2.location.x + p3.location.x) * t3
                )
                let y = 0.5 * (
                    2 * p1.location.y
                        + (-p0.location.y + p2.location.y) * t
                        + (2 * p0.location.y - 5 * p1.location.y + 4 * p2.location.y - p3.location.y) * t2
                        + (-p0.location.y + 3 * p1.location.y - 3 * p2.location.y + p3.location.y) * t3
                )
                result.append(
                    InkPoint(
                        location: CGPoint(x: x, y: y),
                        pressure: p1.pressure + (p2.pressure - p1.pressure) * t,
                        timestamp: p1.timestamp + (p2.timestamp - p1.timestamp) * TimeInterval(t)
                    )
                )
            }
        }

        result.append(points[points.count - 1])
        return result
    }

    static func snappedPenEndpoint(from start: CGPoint, to proposed: CGPoint) -> CGPoint {
        let delta = CGPoint(x: proposed.x - start.x, y: proposed.y - start.y)
        let distance = hypot(delta.x, delta.y)
        guard distance > 0 else { return start }
        let step = CGFloat.pi / 4
        let angle = (atan2(delta.y, delta.x) / step).rounded() * step
        return CGPoint(
            x: start.x + cos(angle) * distance,
            y: start.y + sin(angle) * distance
        )
    }

    static func snappedHighlighterEndpoint(from start: CGPoint, to proposed: CGPoint) -> CGPoint {
        CGPoint(x: proposed.x, y: start.y)
    }
}
