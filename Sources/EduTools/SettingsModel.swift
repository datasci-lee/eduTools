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

struct ToolSettings: Codable, Equatable {
    var colors: [SettingsColor]
    var penWidth: Double
    var highlighterWidth: Double
    var highlighterOpacity: Double
    var laserColor: SettingsColor
    var laserSize: Double
    var laserTrailDuration: Double
    var textFontSize: Double
    var shapeRecognitionEnabled: Bool
    var paletteStyle: PaletteStyle

    init(
        colors: [SettingsColor],
        penWidth: Double,
        highlighterWidth: Double,
        highlighterOpacity: Double,
        laserColor: SettingsColor,
        laserSize: Double,
        laserTrailDuration: Double,
        textFontSize: Double,
        shapeRecognitionEnabled: Bool,
        paletteStyle: PaletteStyle = .radial
    ) {
        self.colors = colors
        self.penWidth = penWidth
        self.highlighterWidth = highlighterWidth
        self.highlighterOpacity = highlighterOpacity
        self.laserColor = laserColor
        self.laserSize = laserSize
        self.laserTrailDuration = laserTrailDuration
        self.textFontSize = textFontSize
        self.shapeRecognitionEnabled = shapeRecognitionEnabled
        self.paletteStyle = paletteStyle
    }

    private enum CodingKeys: String, CodingKey {
        case colors
        case penWidth
        case highlighterWidth
        case highlighterOpacity
        case laserColor
        case laserSize
        case laserTrailDuration
        case textFontSize
        case shapeRecognitionEnabled
        case paletteStyle
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        colors = try container.decode([SettingsColor].self, forKey: .colors)
        penWidth = try container.decode(Double.self, forKey: .penWidth)
        highlighterWidth = try container.decode(Double.self, forKey: .highlighterWidth)
        highlighterOpacity = try container.decode(Double.self, forKey: .highlighterOpacity)
        laserColor = try container.decode(SettingsColor.self, forKey: .laserColor)
        laserSize = try container.decode(Double.self, forKey: .laserSize)
        laserTrailDuration = try container.decode(Double.self, forKey: .laserTrailDuration)
        textFontSize = try container.decode(Double.self, forKey: .textFontSize)
        shapeRecognitionEnabled = try container.decode(Bool.self, forKey: .shapeRecognitionEnabled)
        paletteStyle = try container.decodeIfPresent(PaletteStyle.self, forKey: .paletteStyle)
            ?? .radial
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
    var shortcuts: ShortcutGroupSettings
    var help: HelpGroupSettings

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
            penWidth: 3,
            highlighterWidth: 18,
            highlighterOpacity: 0.4,
            laserColor: SettingsColor(red: 1, green: 0.231, blue: 0.188),
            laserSize: 8,
            laserTrailDuration: 1,
            textFontSize: 24,
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
        guard (1...30).contains(settings.tools.penWidth),
              (1...30).contains(settings.tools.highlighterWidth),
              (0.05...1).contains(settings.tools.highlighterOpacity),
              (2...50).contains(settings.tools.laserSize),
              (0.1...10).contains(settings.tools.laserTrailDuration),
              (8...96).contains(settings.tools.textFontSize)
        else {
            throw SettingsValidationError.invalidValue("도구 범위")
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

        for (identifier, portable) in settings.shortcuts.overrides {
            guard !identifier.isEmpty, PortableShortcut(serialized: portable) != nil else {
                throw SettingsValidationError.invalidValue("단축키 \(identifier)")
            }
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
        case .shortcuts: return "단축키"
        case .help: return "도움말"
        }
    }
}

struct SettingsPayload: Codable, Equatable {
    var general: GeneralSettings?
    var tools: ToolSettings?
    var eraser: EraserGroupSettings?
    var zoom: ZoomGroupSettings?
    var board: BoardGroupSettings?
    var timer: TimerGroupSettings?
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
        return importedValues.keys.sorted().compactMap { key in
            guard currentValues[key] != importedValues[key] else { return nil }
            return SettingsChange(
                id: key,
                currentValue: currentValues[key] ?? "—",
                importedValue: importedValues[key] ?? "—"
            )
        }
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
