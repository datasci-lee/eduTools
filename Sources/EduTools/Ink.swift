import AppKit
import Foundation

enum InkTool: String {
    case pen
    case highlighter
    case laser

    var displayName: String {
        switch self {
        case .pen: return "펜"
        case .highlighter: return "형광펜"
        case .laser: return "레이저"
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
    }
}

enum InkCommand {
    case addStroke(InkStroke)

    func apply(to strokes: inout [InkStroke]) {
        switch self {
        case let .addStroke(stroke):
            strokes.append(stroke)
        }
    }

    func revert(on strokes: inout [InkStroke]) {
        switch self {
        case let .addStroke(stroke):
            guard let index = strokes.lastIndex(where: { $0.identifier == stroke.identifier }) else {
                return
            }
            strokes.remove(at: index)
        }
    }
}

final class InkLayerState {
    private(set) var strokes: [InkStroke] = []
    private var undoCommands: [InkCommand] = []
    private var redoCommands: [InkCommand] = []
    private let undoLimit: Int

    init(undoLimit: Int = InkDefaults.undoLimit) {
        self.undoLimit = undoLimit
    }

    func execute(_ command: InkCommand) {
        command.apply(to: &strokes)
        undoCommands.append(command)
        if undoCommands.count > undoLimit {
            undoCommands.removeFirst(undoCommands.count - undoLimit)
        }
        redoCommands.removeAll(keepingCapacity: true)
    }

    @discardableResult
    func undo() -> Bool {
        guard let command = undoCommands.popLast() else { return false }
        command.revert(on: &strokes)
        redoCommands.append(command)
        return true
    }

    @discardableResult
    func redo() -> Bool {
        guard let command = redoCommands.popLast() else { return false }
        command.apply(to: &strokes)
        undoCommands.append(command)
        return true
    }

    var canRedo: Bool {
        !redoCommands.isEmpty
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
