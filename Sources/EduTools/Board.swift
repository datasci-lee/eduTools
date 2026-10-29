import AppKit
import Foundation

enum BoardKind: String, CaseIterable, Hashable, Codable {
    case white
    case black

    var displayName: String {
        switch self {
        case .white: return "화이트보드"
        case .black: return "블랙보드"
        }
    }
}

enum BlackboardBackground: Hashable {
    case black
    case chalkGreen
}

enum BoardBackgroundPattern: String, CaseIterable, Hashable {
    case none
    case grid
    case lines

    var displayName: String {
        switch self {
        case .none: return "없음"
        case .grid: return "격자"
        case .lines: return "줄"
        }
    }
}

struct BoardPresentation: Equatable {
    let kind: BoardKind
    let displayUUID: String
}

struct BoardPagePosition: Equatable {
    let current: Int
    let total: Int
}

enum BoardAppearance {
    static func backgroundColor(
        for kind: BoardKind,
        blackboardBackground: BlackboardBackground
    ) -> NSColor {
        switch kind {
        case .white:
            return .white
        case .black:
            switch blackboardBackground {
            case .black:
                return .black
            case .chalkGreen:
                return NSColor(deviceRed: 0.055, green: 0.18, blue: 0.13, alpha: 1)
            }
        }
    }

    static func patternColor(for kind: BoardKind) -> NSColor {
        switch kind {
        case .white:
            return NSColor.black.withAlphaComponent(0.12)
        case .black:
            return NSColor.white.withAlphaComponent(0.18)
        }
    }

    static func displayColor(for storedColor: NSColor, on kind: BoardKind?) -> NSColor {
        guard kind == .black, isBlack(storedColor) else { return storedColor }
        return .white
    }

    static func isBlack(_ color: NSColor) -> Bool {
        guard let rgb = color.usingColorSpace(.deviceRGB) else { return false }
        return rgb.redComponent <= 0.001
            && rgb.greenComponent <= 0.001
            && rgb.blueComponent <= 0.001
    }
}

final class BoardDocument: Codable {
    private var pages: [InkLayerState] = [InkLayerState()]
    private(set) var currentPageIndex = 0

    var pagePosition: BoardPagePosition {
        BoardPagePosition(current: currentPageIndex + 1, total: pages.count)
    }

    var strokes: [InkStroke] {
        currentPage.strokes
    }

    var allPageStrokes: [[InkStroke]] {
        pages.map(\.strokes)
    }

    private enum CodingKeys: String, CodingKey {
        case pages
        case currentPageIndex
    }

    init() {}

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let decodedPages = try container.decode([InkLayerState].self, forKey: .pages)
        let decodedIndex = try container.decode(Int.self, forKey: .currentPageIndex)
        guard !decodedPages.isEmpty, decodedPages.indices.contains(decodedIndex) else {
            throw DecodingError.dataCorruptedError(
                forKey: .currentPageIndex,
                in: container,
                debugDescription: "Board must contain the selected page"
            )
        }
        pages = decodedPages
        currentPageIndex = decodedIndex
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(pages, forKey: .pages)
        try container.encode(currentPageIndex, forKey: .currentPageIndex)
    }

    func addStroke(_ stroke: InkStroke) {
        currentPage.execute(.addStroke(stroke))
    }

    @discardableResult
    func replaceStrokes(with strokes: [InkStroke]) -> Bool {
        let before = currentPage.strokes
        guard before != strokes else { return false }
        currentPage.execute(.replaceStrokes(before: before, after: strokes))
        return true
    }

    @discardableResult
    func clear() -> Bool {
        replaceStrokes(with: [])
    }

    @discardableResult
    func undo() -> Bool {
        currentPage.undo()
    }

    @discardableResult
    func redo() -> Bool {
        currentPage.redo()
    }

    @discardableResult
    func moveToPreviousPage() -> Bool {
        guard currentPageIndex > 0 else { return false }
        currentPageIndex -= 1
        return true
    }

    @discardableResult
    func moveToNextPage() -> Bool {
        if currentPageIndex == pages.index(before: pages.endIndex) {
            pages.append(InkLayerState())
        }
        currentPageIndex += 1
        return true
    }

    private var currentPage: InkLayerState {
        pages[currentPageIndex]
    }
}
