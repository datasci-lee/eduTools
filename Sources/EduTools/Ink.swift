import AppKit
import Foundation

enum InkTool: String, Hashable, Codable {
    case pen
    case highlighter
    case laser
    case eraser
    case arrow
    case rectangle
    case ellipse
    case text
    case select
    case cover

    var displayName: String {
        switch self {
        case .pen: return "펜"
        case .highlighter: return "형광펜"
        case .laser: return "레이저"
        case .eraser: return "지우개"
        case .arrow: return "화살표"
        case .rectangle: return "사각형"
        case .ellipse: return "원"
        case .text: return "텍스트"
        case .select: return "선택"
        case .cover: return "화면 가리개"
        }
    }
}

enum InkStrokeTool: String, Equatable, Codable {
    case pen
    case highlighter
    case shape
    case text
    case cover
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
    static let textFontSize: CGFloat = 24
    static let minimumTextFontSize: CGFloat = 8
    static let maximumTextFontSize: CGFloat = 96
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
    let areaShape: EraserAreaShape

    init(
        diameter: CGFloat,
        mode: EraserMode,
        hitRule: EraserHitRule,
        targetFilter: EraserTargetFilter,
        areaShape: EraserAreaShape = .rectangle
    ) {
        self.diameter = diameter
        self.mode = mode
        self.hitRule = hitRule
        self.targetFilter = targetFilter
        self.areaShape = areaShape
    }
}

struct InkPoint: Equatable, Codable {
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

struct InkStroke: Equatable, Codable {
    let identifier: UUID
    let tool: InkStrokeTool
    let color: NSColor
    let width: CGFloat
    let usesPressure: Bool
    let points: [InkPoint]
    let pointBounds: CGRect
    let content: InkStrokeContent

    init(
        identifier: UUID = UUID(),
        tool: InkStrokeTool,
        color: NSColor,
        width: CGFloat,
        usesPressure: Bool,
        points: [InkPoint],
        content: InkStrokeContent = .freehand
    ) {
        self.identifier = identifier
        self.tool = tool
        self.color = color
        self.width = width
        self.usesPressure = usesPressure
        self.points = points
        self.content = content
        pointBounds = points.reduce(CGRect.null) { bounds, point in
            bounds.union(CGRect(origin: point.location, size: .zero))
        }
    }

    static func == (lhs: InkStroke, rhs: InkStroke) -> Bool {
        lhs.identifier == rhs.identifier
            && lhs.tool == rhs.tool
            && lhs.color == rhs.color
            && lhs.width == rhs.width
            && lhs.usesPressure == rhs.usesPressure
            && lhs.points == rhs.points
            && lhs.content == rhs.content
    }

    private enum CodingKeys: String, CodingKey {
        case identifier
        case tool
        case colorComponents
        case width
        case usesPressure
        case points
        case content
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        identifier = try container.decode(UUID.self, forKey: .identifier)
        tool = try container.decode(InkStrokeTool.self, forKey: .tool)
        let components = try container.decode([CGFloat].self, forKey: .colorComponents)
        guard components.count == 4 else {
            throw DecodingError.dataCorruptedError(
                forKey: .colorComponents,
                in: container,
                debugDescription: "Ink color must contain red, green, blue, and alpha components"
            )
        }
        color = NSColor(
            deviceRed: components[0],
            green: components[1],
            blue: components[2],
            alpha: components[3]
        )
        width = try container.decode(CGFloat.self, forKey: .width)
        usesPressure = try container.decode(Bool.self, forKey: .usesPressure)
        points = try container.decode([InkPoint].self, forKey: .points)
        content = try container.decode(InkStrokeContent.self, forKey: .content)
        pointBounds = points.reduce(CGRect.null) { bounds, point in
            bounds.union(CGRect(origin: point.location, size: .zero))
        }
    }

    func encode(to encoder: Encoder) throws {
        guard let rgb = color.usingColorSpace(.deviceRGB) else {
            throw EncodingError.invalidValue(
                color,
                EncodingError.Context(
                    codingPath: encoder.codingPath,
                    debugDescription: "Ink color could not be converted to device RGB"
                )
            )
        }
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(identifier, forKey: .identifier)
        try container.encode(tool, forKey: .tool)
        try container.encode(
            [rgb.redComponent, rgb.greenComponent, rgb.blueComponent, rgb.alphaComponent],
            forKey: .colorComponents
        )
        try container.encode(width, forKey: .width)
        try container.encode(usesPressure, forKey: .usesPressure)
        try container.encode(points, forKey: .points)
        try container.encode(content, forKey: .content)
    }
}

private enum InkLayerKey: Hashable, Codable {
    case legacy
    case display(String)
}

enum InkCommand: Codable {
    case addStroke(InkStroke)
    case replaceStrokes(before: [InkStroke], after: [InkStroke])
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
        case let .replaceStrokes(_, after):
            layers[.legacy] = after
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
        case let .replaceStrokes(before, _):
            layers[.legacy] = before
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

    fileprivate func command(for displayUUID: String) -> InkCommand? {
        switch self {
        case .addStroke, .replaceStrokes:
            return nil
        case let .addDisplayStroke(commandDisplayUUID, stroke):
            guard commandDisplayUUID == displayUUID else { return nil }
            return .addDisplayStroke(displayUUID: displayUUID, stroke: stroke)
        case let .replaceDisplayStrokes(commandDisplayUUID, before, after):
            guard commandDisplayUUID == displayUUID else { return nil }
            return .replaceDisplayStrokes(
                displayUUID: displayUUID,
                before: before,
                after: after
            )
        case let .replaceAllDisplayStrokes(before, after):
            let displayBefore = before[displayUUID] ?? []
            let displayAfter = after[displayUUID] ?? []
            guard displayBefore != displayAfter else { return nil }
            return .replaceDisplayStrokes(
                displayUUID: displayUUID,
                before: displayBefore,
                after: displayAfter
            )
        }
    }

    fileprivate func excluding(displayUUID: String) -> InkCommand? {
        switch self {
        case .addStroke, .replaceStrokes:
            return self
        case let .addDisplayStroke(commandDisplayUUID, _):
            return commandDisplayUUID == displayUUID ? nil : self
        case let .replaceDisplayStrokes(commandDisplayUUID, _, _):
            return commandDisplayUUID == displayUUID ? nil : self
        case let .replaceAllDisplayStrokes(before, after):
            var filteredBefore = before
            var filteredAfter = after
            filteredBefore.removeValue(forKey: displayUUID)
            filteredAfter.removeValue(forKey: displayUUID)
            guard filteredBefore != filteredAfter else { return nil }
            return .replaceAllDisplayStrokes(before: filteredBefore, after: filteredAfter)
        }
    }
}

final class InkLayerState: Codable {
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

    private enum CodingKeys: String, CodingKey {
        case layers
        case undoCommands
        case redoCommands
        case undoLimit
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        layers = try container.decode([InkLayerKey: [InkStroke]].self, forKey: .layers)
        undoCommands = try container.decode([InkCommand].self, forKey: .undoCommands)
        redoCommands = try container.decode([InkCommand].self, forKey: .redoCommands)
        undoLimit = try container.decode(Int.self, forKey: .undoLimit)
        guard undoLimit > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .undoLimit,
                in: container,
                debugDescription: "Undo limit must be greater than zero"
            )
        }
        if undoCommands.count > undoLimit {
            undoCommands.removeFirst(undoCommands.count - undoLimit)
        }
        if redoCommands.count > undoLimit {
            redoCommands.removeFirst(redoCommands.count - undoLimit)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(layers, forKey: .layers)
        try container.encode(undoCommands, forKey: .undoCommands)
        try container.encode(redoCommands, forKey: .redoCommands)
        try container.encode(undoLimit, forKey: .undoLimit)
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

    func hasRestorableState(for displayUUID: String) -> Bool {
        !strokes(for: displayUUID).isEmpty
            || undoCommands.contains { $0.command(for: displayUUID) != nil }
            || redoCommands.contains { $0.command(for: displayUUID) != nil }
    }

    func state(for displayUUID: String) -> InkLayerState {
        let result = InkLayerState(undoLimit: undoLimit)
        if let strokes = layers[.display(displayUUID)] {
            result.layers[.display(displayUUID)] = strokes
        }
        result.undoCommands = undoCommands.compactMap { $0.command(for: displayUUID) }
        result.redoCommands = redoCommands.compactMap { $0.command(for: displayUUID) }
        return result
    }

    func mergeDisplayState(_ archivedState: InkLayerState, displayUUID: String) {
        layers[.display(displayUUID)] = archivedState.strokes(for: displayUUID)
        undoCommands = undoCommands.compactMap { $0.excluding(displayUUID: displayUUID) }
        redoCommands = redoCommands.compactMap { $0.excluding(displayUUID: displayUUID) }
        undoCommands.append(
            contentsOf: archivedState.undoCommands.compactMap { $0.command(for: displayUUID) }
        )
        redoCommands.append(
            contentsOf: archivedState.redoCommands.compactMap { $0.command(for: displayUUID) }
        )
        if undoCommands.count > undoLimit {
            undoCommands.removeFirst(undoCommands.count - undoLimit)
        }
        if redoCommands.count > undoLimit {
            redoCommands.removeFirst(redoCommands.count - undoLimit)
        }
    }

    func removeDisplay(_ displayUUID: String) {
        layers.removeValue(forKey: .display(displayUUID))
        undoCommands = undoCommands.compactMap { $0.excluding(displayUUID: displayUUID) }
        redoCommands = redoCommands.compactMap { $0.excluding(displayUUID: displayUUID) }
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

    static func snappedSquareEndpoint(from start: CGPoint, to proposed: CGPoint) -> CGPoint {
        let deltaX = proposed.x - start.x
        let deltaY = proposed.y - start.y
        let side = max(abs(deltaX), abs(deltaY))
        return CGPoint(
            x: start.x + (deltaX < 0 ? -side : side),
            y: start.y + (deltaY < 0 ? -side : side)
        )
    }
}
