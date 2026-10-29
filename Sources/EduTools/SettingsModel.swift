import AppKit
import Combine
import Foundation

struct SettingsColor: Codable, Equatable, Hashable {
    var red: Double
    var green: Double
    var blue: Double
    var alpha: Double

    init(red: Double, green: Double, blue: Double, alpha: Double = 1) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }

    init(_ color: NSColor) {
        let converted = color.usingColorSpace(.deviceRGB) ?? .black
        red = Double(converted.redComponent)
        green = Double(converted.greenComponent)
        blue = Double(converted.blueComponent)
        alpha = Double(converted.alphaComponent)
    }

    var nsColor: NSColor {
        NSColor(
            deviceRed: CGFloat(red),
            green: CGFloat(green),
            blue: CGFloat(blue),
            alpha: CGFloat(alpha)
        )
    }
}

struct GeneralSettings: Codable, Equatable {
    var launchAtLogin: Bool
    var clearsInkWhenAnnotationModeEnds: Bool
    var autoArchiveEnabled: Bool
    var archiveFolderPath: String?
    var displayDisconnectPolicy: String
    var automaticallyRestoresDisconnectedDisplays: Bool
}

enum PaletteStyle: String, Codable, CaseIterable, Hashable {
    case radial
    case linear

    var displayName: String {
        switch self {
        case .radial: return "원형"
        case .linear: return "가로"
        }
    }

    var iconName: String {
        switch self {
        case .radial: return "circle.grid.3x3.fill"
        case .linear: return "rectangle.split.3x1"
        }
    }
}

enum RadialPaletteGlassStyle {
    static let defaultOpacity = 0.62
    static let minimumOpacity = 0.55
    static let maximumOpacity = 0.90

    static let primaryWhiteComponent: CGFloat = 0.10
    static let disabledContentOpacity = 0.55
    static let selectedTintOpacity = 0.72

    static let primaryContentColor = NSColor(
        calibratedWhite: primaryWhiteComponent,
        alpha: 1
    )
    static let selectedContentColor = NSColor(calibratedWhite: 1, alpha: 1)
    static let regularBorderColor = NSColor(
        calibratedWhite: primaryWhiteComponent,
        alpha: 0.28
    )
    static let focusOuterColor = NSColor(
        calibratedWhite: primaryWhiteComponent,
        alpha: 0.88
    )
    static let colorSelectionOuterColor = NSColor(
        calibratedWhite: primaryWhiteComponent,
        alpha: 0.90
    )

    static func contentColor(isSelected: Bool, isEnabled: Bool) -> NSColor {
        if !isEnabled {
            return primaryContentColor.withAlphaComponent(CGFloat(disabledContentOpacity))
        }
        return isSelected ? selectedContentColor : primaryContentColor
    }

    /// 근사 — 실제 면은 뒤 배경과 0.55...0.90 알파로 혼합된다.
    static func estimatedBackdropColor(selected: Bool) -> NSColor {
        selected
            ? .controlAccentColor
            : NSColor(
                calibratedWhite: AttrIconBackgroundEstimate.lightSurfaceWhiteComponent,
                alpha: 1
            )
    }

    // The white face is alpha-composited above the glass, so even if the material
    // renders black its encoded sRGB value is at least a: a * 1 + (1 - a) * glass >= a.
    // On black, L = ((a + 0.055) / 1.055)^2.4; near-black 0.10 content therefore has
    // 5.22:1 contrast at a = 0.55 and 6.54:1 at the default a = 0.62.
    static func surfaceFaceColor(opacity: Double) -> NSColor {
        NSColor(calibratedWhite: 1, alpha: CGFloat(opacity))
    }
}

struct ToolMemorySetting: Codable, Equatable {
    var colorIndex: Int
    var size: Double
}

enum FavoritesDefaults {
    static let maximumCount = 8
}

struct FavoriteTool: Codable, Equatable {
    let tool: String
    let colorIndex: Int?
    let size: Double
    let arrowStyle: String?
    let eraserMode: String?
    let eraserAreaShape: String?

    var inkTool: InkTool? {
        InkTool(rawValue: tool)
    }

    var decodedArrowStyle: ArrowStyle? {
        arrowStyle.flatMap(ArrowStyle.init(rawValue:))
    }

    var decodedEraserMode: EraserMode? {
        guard let eraserMode else { return nil }
        return EraserMode.allCases.first { mode in
            switch mode {
            case .partial: return eraserMode == "partial"
            case .stroke: return eraserMode == "stroke"
            case .area: return eraserMode == "area"
            }
        }
    }

    var decodedEraserAreaShape: EraserAreaShape? {
        guard let eraserAreaShape else { return nil }
        return EraserAreaShape.allCases.first { shape in
            switch shape {
            case .rectangle: return eraserAreaShape == "rectangle"
            case .lasso: return eraserAreaShape == "lasso"
            }
        }
    }

    func isValid(colorPresetCount: Int) -> Bool {
        guard let inkTool, AttrIconToolCatalog.manifestKeyByTool[inkTool] != nil else {
            return false
        }

        let hasValidColor: Bool
        if inkTool == .eraser {
            hasValidColor = colorIndex == nil
        } else {
            hasValidColor = colorIndex.map { (0..<colorPresetCount).contains($0) } == true
        }
        guard hasValidColor else { return false }

        switch inkTool {
        case .eraser:
            return (Double(EraserDefaults.minimumDiameter)...Double(EraserDefaults.maximumDiameter))
                .contains(size)
                && arrowStyle == nil
                && decodedEraserMode != nil
                && decodedEraserAreaShape != nil
        case .laser:
            return (InkDefaults.minimumLaserTrailDuration...InkDefaults.maximumLaserTrailDuration)
                .contains(size)
                && arrowStyle == nil
                && eraserMode == nil
                && eraserAreaShape == nil
        case .arrow:
            return inkTool.memorySlot?.sizeRange.contains(size) == true
                && decodedArrowStyle != nil
                && eraserMode == nil
                && eraserAreaShape == nil
        case .pen, .highlighter, .rectangle, .ellipse, .text:
            return inkTool.memorySlot?.sizeRange.contains(size) == true
                && arrowStyle == nil
                && eraserMode == nil
                && eraserAreaShape == nil
        case .select, .cover:
            return false
        }
    }
}

struct PerToolMemorySettings: Codable, Equatable {
    static let initialColorIndex = 0

    static let `default`: PerToolMemorySettings = {
        let pen = ToolMemorySetting(colorIndex: initialColorIndex, size: 3)
        return PerToolMemorySettings(
            pen: pen,
            highlighter: ToolMemorySetting(colorIndex: initialColorIndex, size: 18),
            arrow: pen,
            rectangle: pen,
            ellipse: pen,
            text: ToolMemorySetting(colorIndex: initialColorIndex, size: 24)
        )
    }()

    var pen: ToolMemorySetting
    var highlighter: ToolMemorySetting
    var arrow: ToolMemorySetting
    var rectangle: ToolMemorySetting
    var ellipse: ToolMemorySetting
    var text: ToolMemorySetting

    static func migratingLegacySizes(
        penWidth: Double?,
        highlighterWidth: Double?,
        textFontSize: Double?
    ) -> PerToolMemorySettings {
        var result = Self.default
        if let penWidth {
            result.pen.size = penWidth
            result.arrow.size = penWidth
            result.rectangle.size = penWidth
            result.ellipse.size = penWidth
        }
        if let highlighterWidth {
            result.highlighter.size = highlighterWidth
        }
        if let textFontSize {
            result.text.size = textFontSize
        }
        return result
    }

    mutating func clampColorIndices(presetCount: Int) {
        guard presetCount > 0 else { return }
        let validRange = 0...(presetCount - 1)
        for keyPath in [
            \PerToolMemorySettings.pen,
            \PerToolMemorySettings.highlighter,
            \PerToolMemorySettings.arrow,
            \PerToolMemorySettings.rectangle,
            \PerToolMemorySettings.ellipse,
            \PerToolMemorySettings.text,
        ] {
            self[keyPath: keyPath].colorIndex = min(
                max(self[keyPath: keyPath].colorIndex, validRange.lowerBound),
                validRange.upperBound
            )
        }
    }
}

struct ToolSettings: Codable, Equatable {
    var colors: [SettingsColor] {
        didSet {
            perToolMemory.clampColorIndices(presetCount: colors.count)
        }
    }
    var highlighterOpacity: Double
    var laserColor: SettingsColor
    var laserSize: Double
    var laserTrailDuration: Double
    var perToolMemory: PerToolMemorySettings
    var favorites: [FavoriteTool]
    var shapeRecognitionEnabled: Bool
    var paletteStyle: PaletteStyle
    var paletteGlassOpacity: Double

    init(
        colors: [SettingsColor],
        highlighterOpacity: Double,
        laserColor: SettingsColor,
        laserSize: Double,
        laserTrailDuration: Double,
        perToolMemory: PerToolMemorySettings = .default,
        favorites: [FavoriteTool] = [],
        shapeRecognitionEnabled: Bool,
        paletteStyle: PaletteStyle = .radial,
        paletteGlassOpacity: Double = RadialPaletteGlassStyle.defaultOpacity
    ) {
        self.colors = colors
        self.highlighterOpacity = highlighterOpacity
        self.laserColor = laserColor
        self.laserSize = laserSize
        self.laserTrailDuration = laserTrailDuration
        self.perToolMemory = perToolMemory
        self.favorites = favorites.isEmpty
            ? []
            : Self.normalizedFavorites(favorites, colorPresetCount: colors.count)
        self.shapeRecognitionEnabled = shapeRecognitionEnabled
        self.paletteStyle = paletteStyle
        self.paletteGlassOpacity = paletteGlassOpacity
    }

    private enum CodingKeys: String, CodingKey {
        case colors
        case highlighterOpacity
        case laserColor
        case laserSize
        case laserTrailDuration
        case perToolMemory
        case favorites
        case shapeRecognitionEnabled
        case paletteStyle
        case paletteGlassOpacity
    }

    private enum LegacySizeCodingKeys: String, CodingKey {
        case penWidth
        case highlighterWidth
        case textFontSize
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        colors = try container.decode([SettingsColor].self, forKey: .colors)
        highlighterOpacity = try container.decode(Double.self, forKey: .highlighterOpacity)
        laserColor = try container.decode(SettingsColor.self, forKey: .laserColor)
        laserSize = try container.decode(Double.self, forKey: .laserSize)
        laserTrailDuration = try container.decode(Double.self, forKey: .laserTrailDuration)
        if let decodedMemory = try container.decodeIfPresent(
            PerToolMemorySettings.self,
            forKey: .perToolMemory
        ) {
            perToolMemory = decodedMemory
        } else {
            let legacyContainer = try decoder.container(keyedBy: LegacySizeCodingKeys.self)
            perToolMemory = .migratingLegacySizes(
                penWidth: try legacyContainer.decodeIfPresent(Double.self, forKey: .penWidth),
                highlighterWidth: try legacyContainer.decodeIfPresent(
                    Double.self,
                    forKey: .highlighterWidth
                ),
                textFontSize: try legacyContainer.decodeIfPresent(
                    Double.self,
                    forKey: .textFontSize
                )
            )
        }
        let decodedFavorites = try container.decodeIfPresent(
            [FavoriteTool].self,
            forKey: .favorites
        ) ?? []
        favorites = decodedFavorites.isEmpty
            ? []
            : Self.normalizedFavorites(decodedFavorites, colorPresetCount: colors.count)
        shapeRecognitionEnabled = try container.decode(Bool.self, forKey: .shapeRecognitionEnabled)
        paletteStyle = try container.decodeIfPresent(PaletteStyle.self, forKey: .paletteStyle)
            ?? .radial
        paletteGlassOpacity = try container.decodeIfPresent(
            Double.self,
            forKey: .paletteGlassOpacity
        ) ?? RadialPaletteGlassStyle.defaultOpacity
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(colors, forKey: .colors)
        try container.encode(highlighterOpacity, forKey: .highlighterOpacity)
        try container.encode(laserColor, forKey: .laserColor)
        try container.encode(laserSize, forKey: .laserSize)
        try container.encode(laserTrailDuration, forKey: .laserTrailDuration)
        try container.encode(perToolMemory, forKey: .perToolMemory)
        try container.encode(favorites, forKey: .favorites)
        try container.encode(shapeRecognitionEnabled, forKey: .shapeRecognitionEnabled)
        try container.encode(paletteStyle, forKey: .paletteStyle)
        try container.encode(paletteGlassOpacity, forKey: .paletteGlassOpacity)
    }

    private static func normalizedFavorites(
        _ favorites: [FavoriteTool],
        colorPresetCount: Int
    ) -> [FavoriteTool] {
        var result: [FavoriteTool] = []
        for favorite in favorites where favorite.isValid(colorPresetCount: colorPresetCount) {
            guard !result.contains(favorite), result.count < FavoritesDefaults.maximumCount else {
                continue
            }
            result.append(favorite)
        }
        if result.count != favorites.count {
            NSLog(
                "EduTools: ignored %d invalid, duplicate, or excess favorite tool settings",
                favorites.count - result.count
            )
        }
        return result
    }
}

struct EraserGroupSettings: Codable, Equatable {
    var mode: String
    var diameter: Double
    var presetDiameters: [Double]
    var hitRule: String
    var targetFilter: String
    var areaShape: String
}

struct ZoomGroupSettings: Codable, Equatable {
    static let defaultPreviewDimming = 0.62

    var startingMagnification: Double
    var maximumMagnification: Double
    var magnificationStep: Double
    var followMode: String
    var followSmoothness: Double
    var liveByDefault: Bool
    var previewDimming: Double

    init(
        startingMagnification: Double,
        maximumMagnification: Double,
        magnificationStep: Double,
        followMode: String,
        followSmoothness: Double,
        liveByDefault: Bool,
        previewDimming: Double = ZoomGroupSettings.defaultPreviewDimming
    ) {
        self.startingMagnification = startingMagnification
        self.maximumMagnification = maximumMagnification
        self.magnificationStep = magnificationStep
        self.followMode = followMode
        self.followSmoothness = followSmoothness
        self.liveByDefault = liveByDefault
        self.previewDimming = previewDimming
    }

    private enum CodingKeys: String, CodingKey {
        case startingMagnification
        case maximumMagnification
        case magnificationStep
        case followMode
        case followSmoothness
        case liveByDefault
        case previewDimming
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        startingMagnification = try container.decode(Double.self, forKey: .startingMagnification)
        maximumMagnification = try container.decode(Double.self, forKey: .maximumMagnification)
        magnificationStep = try container.decode(Double.self, forKey: .magnificationStep)
        followMode = try container.decode(String.self, forKey: .followMode)
        followSmoothness = try container.decode(Double.self, forKey: .followSmoothness)
        liveByDefault = try container.decode(Bool.self, forKey: .liveByDefault)
        previewDimming = try container.decodeIfPresent(Double.self, forKey: .previewDimming)
            ?? Self.defaultPreviewDimming
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(startingMagnification, forKey: .startingMagnification)
        try container.encode(maximumMagnification, forKey: .maximumMagnification)
        try container.encode(magnificationStep, forKey: .magnificationStep)
        try container.encode(followMode, forKey: .followMode)
        try container.encode(followSmoothness, forKey: .followSmoothness)
        try container.encode(liveByDefault, forKey: .liveByDefault)
        try container.encode(previewDimming, forKey: .previewDimming)
    }
}

struct BoardGroupSettings: Codable, Equatable {
    var whiteboardColor: SettingsColor
    var blackboardColor: SettingsColor
    var backgroundPattern: String
}

struct TimerGroupSettings: Codable, Equatable {
    var lessonPresets: [Int]
    var lessonDefaultMinutes: Int
    var breakDefaultMinutes: Int
    var notificationSound: String
    var breakMessage: String
    var breakBackgroundColor: SettingsColor
    var playlistURLs: [String]
    var defaultPlaylistIndex: Int
    var musicEnabled: Bool
    var musicBrowser: String
    var fadeEnabled: Bool
    var fadeDuration: Double
    var autoStartBreak: Bool
    var lessonBadgeSize: String
    var lessonBadgeOpacity: Double
    var lessonDisplayMode: String
}

enum AppModifierSetting: String, Codable, CaseIterable, Hashable {
    case controlOption
    case controlOptionShift

    var displayName: String {
        switch self {
        case .controlOption: return "⌃⌥"
        case .controlOptionShift: return "⌃⌥⇧"
        }
    }
}

struct ShortcutGroupSettings: Codable, Equatable {
    var appModifier: AppModifierSetting
    var overrides: [String: String]
}

struct HelpGroupSettings: Codable, Equatable {
    var showsHelpInSharing: Bool
}

struct AppSettings: Codable, Equatable {
    var general: GeneralSettings
    var tools: ToolSettings
    var eraser: EraserGroupSettings
    var zoom: ZoomGroupSettings
    var board: BoardGroupSettings
    var timer: TimerGroupSettings
    var display: DisplaySettings
    var shortcuts: ShortcutGroupSettings
    var help: HelpGroupSettings

    init(
        general: GeneralSettings,
        tools: ToolSettings,
        eraser: EraserGroupSettings,
        zoom: ZoomGroupSettings,
        board: BoardGroupSettings,
        timer: TimerGroupSettings,
        display: DisplaySettings = .default,
        shortcuts: ShortcutGroupSettings,
        help: HelpGroupSettings
    ) {
        self.general = general
        self.tools = tools
        self.eraser = eraser
        self.zoom = zoom
        self.board = board
        self.timer = timer
        self.display = display
        self.shortcuts = shortcuts
        self.help = help
    }

    private enum CodingKeys: String, CodingKey {
        case general
        case tools
        case eraser
        case zoom
        case board
        case timer
        case display
        case shortcuts
        case help
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        general = try container.decode(GeneralSettings.self, forKey: .general)
        tools = try container.decode(ToolSettings.self, forKey: .tools)
        eraser = try container.decode(EraserGroupSettings.self, forKey: .eraser)
        zoom = try container.decode(ZoomGroupSettings.self, forKey: .zoom)
        board = try container.decode(BoardGroupSettings.self, forKey: .board)
        timer = try container.decode(TimerGroupSettings.self, forKey: .timer)
        display = try container.decodeIfPresent(DisplaySettings.self, forKey: .display)
            ?? .default
        shortcuts = try container.decode(ShortcutGroupSettings.self, forKey: .shortcuts)
        help = try container.decode(HelpGroupSettings.self, forKey: .help)
    }

    static let `default` = AppSettings(
        general: GeneralSettings(
            launchAtLogin: false,
            clearsInkWhenAnnotationModeEnds: false,
            autoArchiveEnabled: true,
            archiveFolderPath: nil,
            displayDisconnectPolicy: DisplayDisconnectPolicy.archive.rawValue,
            automaticallyRestoresDisconnectedDisplays: false
        ),
        tools: ToolSettings(
            colors: [
                SettingsColor(red: 1, green: 0.231, blue: 0.188),
                SettingsColor(red: 1, green: 0.584, blue: 0),
                SettingsColor(red: 1, green: 0.8, blue: 0),
                SettingsColor(red: 0.204, green: 0.78, blue: 0.349),
                SettingsColor(red: 0, green: 0.478, blue: 1),
                SettingsColor(red: 1, green: 1, blue: 1),
            ],
            highlighterOpacity: 0.4,
            laserColor: SettingsColor(red: 1, green: 0.231, blue: 0.188),
            laserSize: 8,
            laserTrailDuration: 1,
            shapeRecognitionEnabled: true
        ),
        eraser: EraserGroupSettings(
            mode: "partial",
            diameter: 24,
            presetDiameters: [12, 24, 60],
            hitRule: "touching",
            targetFilter: "all",
            areaShape: "rectangle"
        ),
        zoom: ZoomGroupSettings(
            startingMagnification: 2,
            maximumMagnification: 8,
            magnificationStep: 0.25,
            followMode: ZoomFollowMode.edge.rawValue,
            followSmoothness: 0.22,
            liveByDefault: true,
            previewDimming: ZoomGroupSettings.defaultPreviewDimming
        ),
        board: BoardGroupSettings(
            whiteboardColor: SettingsColor(red: 1, green: 1, blue: 1),
            blackboardColor: SettingsColor(red: 0, green: 0, blue: 0),
            backgroundPattern: "none"
        ),
        timer: TimerGroupSettings(
            lessonPresets: [25, 45, 50, 90],
            lessonDefaultMinutes: 50,
            breakDefaultMinutes: 10,
            notificationSound: "Glass",
            breakMessage: "수업 재개 시각",
            breakBackgroundColor: SettingsColor(red: 0.12, green: 0.12, blue: 0.14),
            playlistURLs: [],
            defaultPlaylistIndex: 0,
            musicEnabled: false,
            musicBrowser: BreakMusicBrowser.chrome.rawValue,
            fadeEnabled: true,
            fadeDuration: 60,
            autoStartBreak: false,
            lessonBadgeSize: "medium",
            lessonBadgeOpacity: 0.9,
            lessonDisplayMode: "remaining"
        ),
        display: .default,
        shortcuts: ShortcutGroupSettings(appModifier: .controlOption, overrides: [:]),
        help: HelpGroupSettings(showsHelpInSharing: false)
    )
}

enum SettingsValidationError: LocalizedError, Equatable {
    case invalidValue(String)
    case unsupportedFormat(String)
    case unsupportedVersion(Int)

    var errorDescription: String? {
        switch self {
        case let .invalidValue(message): return "설정 값이 올바르지 않습니다: \(message)"
        case let .unsupportedFormat(format): return "지원하지 않는 설정 형식입니다: \(format)"
        case let .unsupportedVersion(version): return "지원하지 않는 설정 버전입니다: \(version)"
        }
    }
}

enum SettingsValidator {
    static func validate(_ settings: AppSettings) throws {
        guard ["archive", "moveToPrimary", "delete"].contains(
            settings.general.displayDisconnectPolicy
        ) else {
            throw SettingsValidationError.invalidValue("화면 연결 해제 정책")
        }
        if let path = settings.general.archiveFolderPath, path.isEmpty {
            throw SettingsValidationError.invalidValue("자동 보관 폴더")
        }

        guard settings.tools.colors.count == 6 else {
            throw SettingsValidationError.invalidValue("색상 프리셋은 6개여야 합니다")
        }
        try settings.tools.colors.forEach(validateColor)
        try validateColor(settings.tools.laserColor)
        guard (0.05...1).contains(settings.tools.highlighterOpacity),
              (2...50).contains(settings.tools.laserSize),
              (0.1...10).contains(settings.tools.laserTrailDuration),
              (RadialPaletteGlassStyle.minimumOpacity...RadialPaletteGlassStyle.maximumOpacity).contains(
                    settings.tools.paletteGlassOpacity
                )
        else {
            throw SettingsValidationError.invalidValue("도구 범위")
        }
        for tool in InkTool.allCases {
            guard let slot = tool.memorySlot else { continue }
            let memory = settings.tools.perToolMemory[keyPath: slot.keyPath]
            guard settings.tools.colors.indices.contains(memory.colorIndex),
                  slot.sizeRange.contains(memory.size)
            else {
                throw SettingsValidationError.invalidValue("\(tool.displayName) 마지막 설정")
            }
        }
        let favorites = settings.tools.favorites
        guard favorites.count <= FavoritesDefaults.maximumCount,
              favorites.allSatisfy({ $0.isValid(colorPresetCount: settings.tools.colors.count) }),
              favorites.enumerated().allSatisfy({ index, favorite in
                  favorites.firstIndex(of: favorite) == index
              })
        else {
            throw SettingsValidationError.invalidValue("즐겨찾기 도구")
        }

        guard ["partial", "stroke", "area"].contains(settings.eraser.mode),
              (8...200).contains(settings.eraser.diameter),
              settings.eraser.presetDiameters.count == 3,
              settings.eraser.presetDiameters.allSatisfy({ (8...200).contains($0) }),
              ["touching", "contained"].contains(settings.eraser.hitRule),
              ["all", "penOnly", "highlighterOnly"].contains(settings.eraser.targetFilter),
              ["rectangle", "lasso"].contains(settings.eraser.areaShape)
        else {
            throw SettingsValidationError.invalidValue("지우개 범위")
        }

        guard (1.25...8).contains(settings.zoom.startingMagnification) else {
            throw SettingsValidationError.invalidValue("시작 배율은 1.25~8배여야 합니다")
        }
        guard (1.25...8).contains(settings.zoom.maximumMagnification) else {
            throw SettingsValidationError.invalidValue("최대 배율은 1.25~8배여야 합니다")
        }
        guard settings.zoom.startingMagnification <= settings.zoom.maximumMagnification else {
            throw SettingsValidationError.invalidValue("최대 배율은 시작 배율보다 작을 수 없습니다")
        }
        guard settings.zoom.magnificationStep == 0.25
                || settings.zoom.magnificationStep == 0.5
        else {
            throw SettingsValidationError.invalidValue("배율 단계는 0.25 또는 0.5여야 합니다")
        }
        guard ZoomFollowMode(rawValue: settings.zoom.followMode) != nil else {
            throw SettingsValidationError.invalidValue("따라가기 방식")
        }
        guard (0...1).contains(settings.zoom.followSmoothness) else {
            throw SettingsValidationError.invalidValue("따라가기 부드러움은 0~1이어야 합니다")
        }
        guard (0...1).contains(settings.zoom.previewDimming) else {
            throw SettingsValidationError.invalidValue("미리보기 회색 농도는 0~1이어야 합니다")
        }

        try validateColor(settings.board.whiteboardColor)
        try validateColor(settings.board.blackboardColor)
        guard ["none", "grid", "lines"].contains(settings.board.backgroundPattern) else {
            throw SettingsValidationError.invalidValue("보드 배경 무늬")
        }

        try validateColor(settings.timer.breakBackgroundColor)
        guard !settings.timer.lessonPresets.isEmpty,
              settings.timer.lessonPresets.allSatisfy({ (1...240).contains($0) }),
              (1...240).contains(settings.timer.lessonDefaultMinutes),
              (1...180).contains(settings.timer.breakDefaultMinutes),
              settings.timer.playlistURLs.count <= BreakMusicSettings.maximumPlaylistCount,
              settings.timer.playlistURLs.allSatisfy({ BreakMusicURL.validated($0) != nil }),
              settings.timer.playlistURLs.isEmpty
                || settings.timer.playlistURLs.indices.contains(settings.timer.defaultPlaylistIndex),
              BreakMusicBrowser(rawValue: settings.timer.musicBrowser) != nil,
              (1...600).contains(settings.timer.fadeDuration),
              ["small", "medium", "large"].contains(settings.timer.lessonBadgeSize),
              (0.4...1).contains(settings.timer.lessonBadgeOpacity),
              ["remaining", "elapsed"].contains(settings.timer.lessonDisplayMode)
        else {
            throw SettingsValidationError.invalidValue("타이머 범위")
        }

        for identifier in settings.shortcuts.overrides.keys {
            guard !identifier.isEmpty else {
                throw SettingsValidationError.invalidValue("단축키 \(identifier)")
            }
        }

        do {
            try DisplayPresetValidator.validate(settings.display)
        } catch {
            throw SettingsValidationError.invalidValue(error.localizedDescription)
        }
    }

    private static func validateColor(_ color: SettingsColor) throws {
        guard [color.red, color.green, color.blue, color.alpha].allSatisfy({ (0...1).contains($0) }) else {
            throw SettingsValidationError.invalidValue("색상 구성요소")
        }
    }
}

struct SettingsStoreIssue: Identifiable, Equatable {
    enum Kind: Equatable {
        case load
        case save
    }

    let id = UUID()
    let kind: Kind
    let detail: String
}

private enum SettingsPersistenceError: LocalizedError {
    case verificationFailed

    var errorDescription: String? {
        switch self {
        case .verificationFailed:
            return "저장소에서 기록한 값을 다시 확인하지 못했습니다"
        }
    }
}

final class SettingsStore: ObservableObject {
    static let storageKey = "EduTools.settings.v1"
    static let corruptStorageKey = "EduTools.settings.corrupt.v1"

    @Published var settings: AppSettings {
        didSet {
            guard settings != oldValue else { return }
            persist()
            for observer in observers.values {
                observer(settings)
            }
        }
    }
    @Published private(set) var runtimeShortcutWarnings: [String: String] = [:]
    @Published private(set) var updateErrorMessage: String?
    @Published private(set) var pendingIssue: SettingsStoreIssue?

    private let defaults: UserDefaults
    private var observers: [UUID: (AppSettings) -> Void] = [:]

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let data = defaults.data(forKey: Self.storageKey) {
            do {
                let decoded = try JSONDecoder().decode(AppSettings.self, from: data)
                try SettingsValidator.validate(decoded)
                settings = decoded
            } catch {
                settings = .default
                defaults.set(data, forKey: Self.corruptStorageKey)
                let preserved = defaults.data(forKey: Self.corruptStorageKey) == data
                if preserved {
                    defaults.removeObject(forKey: Self.storageKey)
                }
                let preservationDetail = preserved
                    ? "원본은 사용자 기본설정 키 \(Self.corruptStorageKey)에 보존했습니다."
                    : "별도 키 보존에 실패해 원본을 기존 키에 유지했습니다."
                pendingIssue = SettingsStoreIssue(
                    kind: .load,
                    detail: "저장 설정을 읽지 못해 기본값으로 시작합니다. \(error.localizedDescription) \(preservationDetail)"
                )
                NSLog(
                    "EduTools: invalid stored settings; using defaults; preserved=%@: %@",
                    preserved ? "yes" : "no",
                    String(describing: error)
                )
            }
        } else {
            settings = .default
        }
    }

    func apply(_ newSettings: AppSettings) throws {
        try SettingsValidator.validate(newSettings)
        settings = newSettings
    }

    @discardableResult
    func update(
        _ change: (inout AppSettings) -> Void
    ) -> Result<Void, SettingsValidationError> {
        var candidate = settings
        change(&candidate)
        do {
            try apply(candidate)
            updateErrorMessage = nil
            return .success(())
        } catch let error as SettingsValidationError {
            updateErrorMessage = error.localizedDescription
            NSLog("EduTools: rejected invalid settings update: %@", String(describing: error))
            return .failure(error)
        } catch {
            let validationError = SettingsValidationError.invalidValue(error.localizedDescription)
            updateErrorMessage = validationError.localizedDescription
            NSLog("EduTools: rejected invalid settings update: %@", String(describing: error))
            return .failure(validationError)
        }
    }

    func resetToDefaults() {
        settings = .default
    }

    @discardableResult
    func addObserver(_ observer: @escaping (AppSettings) -> Void) -> UUID {
        let identifier = UUID()
        observers[identifier] = observer
        observer(settings)
        return identifier
    }

    func removeObserver(_ identifier: UUID) {
        observers.removeValue(forKey: identifier)
    }

    func setRuntimeShortcutWarnings(_ warnings: [String: String]) {
        runtimeShortcutWarnings = warnings
    }

    func clearUpdateError() {
        updateErrorMessage = nil
    }

    func clearPendingIssue(id: UUID) {
        guard pendingIssue?.id == id else { return }
        pendingIssue = nil
    }

    private func persist() {
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.sortedKeys]
            let data = try encoder.encode(settings)
            defaults.set(data, forKey: Self.storageKey)
            guard defaults.data(forKey: Self.storageKey) == data else {
                throw SettingsPersistenceError.verificationFailed
            }
        } catch {
            pendingIssue = SettingsStoreIssue(
                kind: .save,
                detail: "설정을 저장하지 못했습니다. 현재 변경은 다시 실행하면 사라질 수 있습니다. \(error.localizedDescription)"
            )
            NSLog("EduTools: failed to persist settings: %@", String(describing: error))
        }
    }
}

enum SettingsBundle: String, Codable, CaseIterable, Hashable {
    case general
    case tools
    case eraser
    case zoom
    case board
    case timer
    case display
    case shortcuts
    case help

    var displayName: String {
        switch self {
        case .general: return "일반"
        case .tools: return "도구"
        case .eraser: return "지우개"
        case .zoom: return "줌"
        case .board: return "보드"
        case .timer: return "타이머"
        case .display: return "화면 구성"
        case .shortcuts: return "단축키"
        case .help: return "도움말"
        }
    }
}

struct DisplayPresetBundle: Codable, Equatable {
    var presets: [DisplayPreset]
}

struct SettingsPayload: Codable, Equatable {
    var general: GeneralSettings?
    var tools: ToolSettings?
    var eraser: EraserGroupSettings?
    var zoom: ZoomGroupSettings?
    var board: BoardGroupSettings?
    var timer: TimerGroupSettings?
    var display: DisplayPresetBundle?
    var shortcuts: ShortcutGroupSettings?
    var help: HelpGroupSettings?
}

struct SettingsDocument: Codable, Equatable {
    static let format = "kr.datasci.edutools.pansetting"
    static let currentVersion = 1

    var format: String
    var formatVersion: Int
    var appVersion: String
    var createdOS: String
    var includedBundles: [SettingsBundle]
    var includesComputerSpecific: Bool
    var payload: SettingsPayload
}

struct SettingsChange: Identifiable, Equatable {
    let id: String
    let currentValue: String
    let importedValue: String
}

enum SettingsTransfer {
    static func exportDocument(
        settings: AppSettings,
        bundles: Set<SettingsBundle> = Set(SettingsBundle.allCases),
        includeComputerSpecific: Bool = false,
        appVersion: String = Bundle.main.object(
            forInfoDictionaryKey: "CFBundleShortVersionString"
        ) as? String ?? "unknown",
        osVersion: String = ProcessInfo.processInfo.operatingSystemVersionString
    ) throws -> SettingsDocument {
        try SettingsValidator.validate(settings)
        var general = bundles.contains(.general) ? settings.general : nil
        if !includeComputerSpecific {
            general?.archiveFolderPath = nil
        }
        return SettingsDocument(
            format: SettingsDocument.format,
            formatVersion: SettingsDocument.currentVersion,
            appVersion: appVersion,
            createdOS: osVersion,
            includedBundles: SettingsBundle.allCases.filter(bundles.contains),
            includesComputerSpecific: includeComputerSpecific,
            payload: SettingsPayload(
                general: general,
                tools: bundles.contains(.tools) ? settings.tools : nil,
                eraser: bundles.contains(.eraser) ? settings.eraser : nil,
                zoom: bundles.contains(.zoom) ? settings.zoom : nil,
                board: bundles.contains(.board) ? settings.board : nil,
                timer: bundles.contains(.timer) ? settings.timer : nil,
                display: bundles.contains(.display)
                    ? DisplayPresetBundle(presets: settings.display.presets)
                    : nil,
                shortcuts: bundles.contains(.shortcuts) ? settings.shortcuts : nil,
                help: bundles.contains(.help) ? settings.help : nil
            )
        )
    }

    static func encode(_ document: SettingsDocument) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(document)
    }

    static func decode(_ data: Data) throws -> SettingsDocument {
        let decoded = try JSONDecoder().decode(SettingsDocument.self, from: data)
        guard decoded.format == SettingsDocument.format else {
            throw SettingsValidationError.unsupportedFormat(decoded.format)
        }
        let migrated = try migrate(decoded)
        _ = try appliedSettings(
            current: .default,
            document: migrated,
            selectedBundles: Set(migrated.includedBundles)
        )
        return migrated
    }

    static func migrate(_ document: SettingsDocument) throws -> SettingsDocument {
        switch document.formatVersion {
        case 1:
            return document
        default:
            throw SettingsValidationError.unsupportedVersion(document.formatVersion)
        }
    }

    static func appliedSettings(
        current: AppSettings,
        document: SettingsDocument,
        selectedBundles: Set<SettingsBundle>
    ) throws -> AppSettings {
        var result = current
        let payload = document.payload
        if selectedBundles.contains(.general), var imported = payload.general {
            if !document.includesComputerSpecific {
                imported.archiveFolderPath = current.general.archiveFolderPath
            }
            result.general = imported
        }
        if selectedBundles.contains(.tools), let value = payload.tools { result.tools = value }
        if selectedBundles.contains(.eraser), let value = payload.eraser { result.eraser = value }
        if selectedBundles.contains(.zoom), let value = payload.zoom { result.zoom = value }
        if selectedBundles.contains(.board), let value = payload.board { result.board = value }
        if selectedBundles.contains(.timer), let value = payload.timer { result.timer = value }
        if selectedBundles.contains(.display),
           let value = payload.display
        {
            result.display.presets = value.presets
        }
        if selectedBundles.contains(.shortcuts), let value = payload.shortcuts {
            result.shortcuts = value
        }
        if selectedBundles.contains(.help), let value = payload.help { result.help = value }
        try SettingsValidator.validate(result)
        return result
    }

    static func changes(
        current: AppSettings,
        document: SettingsDocument,
        selectedBundles: Set<SettingsBundle>
    ) throws -> [SettingsChange] {
        let imported = try appliedSettings(
            current: current,
            document: document,
            selectedBundles: selectedBundles
        )
        let currentValues = try flattenedValues(current)
        let importedValues = try flattenedValues(imported)
        var result: [SettingsChange] = importedValues.keys.sorted().compactMap { key in
            guard currentValues[key] != importedValues[key] else { return nil }
            return SettingsChange(
                id: key,
                currentValue: currentValues[key] ?? "—",
                importedValue: importedValues[key] ?? "—"
            )
        }
        if selectedBundles.contains(.display),
           let display = document.payload.display,
           current.display.presets != display.presets
        {
            result.removeAll { $0.id.hasPrefix("display.presets") }
            result.append(
                SettingsChange(
                    id: "화면 구성",
                    currentValue: "프리셋 \(current.display.presets.count)개",
                    importedValue: "프리셋 \(display.presets.count)개 추가/교체"
                )
            )
        }
        return result.sorted { $0.id < $1.id }
    }

    private static func flattenedValues<T: Encodable>(_ value: T) throws -> [String: String] {
        let data = try JSONEncoder().encode(value)
        let object = try JSONSerialization.jsonObject(with: data)
        var result: [String: String] = [:]
        flatten(object, path: "", result: &result)
        return result
    }

    private static func flatten(_ value: Any, path: String, result: inout [String: String]) {
        if let dictionary = value as? [String: Any] {
            for key in dictionary.keys.sorted() {
                flatten(dictionary[key] as Any, path: path.isEmpty ? key : "\(path).\(key)", result: &result)
            }
        } else if let array = value as? [Any] {
            result[path] = array.map { String(describing: $0) }.joined(separator: ", ")
        } else if value is NSNull {
            result[path] = "—"
        } else {
            result[path] = String(describing: value)
        }
    }
}

final class SettingsBackupStore {
    static let maximumCount = 5
    private static let storageKey = "EduTools.settings.backups.v1"

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    var count: Int {
        storedData.count
    }

    func append(_ settings: AppSettings) throws {
        try SettingsValidator.validate(settings)
        var backups = storedData
        backups.append(try JSONEncoder().encode(settings))
        if backups.count > Self.maximumCount {
            backups.removeFirst(backups.count - Self.maximumCount)
        }
        defaults.set(backups, forKey: Self.storageKey)
    }

    func popLatest() throws -> AppSettings? {
        var backups = storedData
        guard let data = backups.popLast() else { return nil }
        let settings = try JSONDecoder().decode(AppSettings.self, from: data)
        try SettingsValidator.validate(settings)
        defaults.set(backups, forKey: Self.storageKey)
        return settings
    }

    private var storedData: [Data] {
        defaults.array(forKey: Self.storageKey) as? [Data] ?? []
    }
}
