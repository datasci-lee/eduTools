import AppKit
import Foundation

enum AttrIconBackgroundEstimate {
    static let lightSurfaceWhiteComponent: CGFloat = 0.93
    static let darkSystemSurfaceWhiteComponent: CGFloat = 0.18
}

enum AppIcon {
    enum Key {
        static let optionOn = "option.on"
        static let optionOff = "option.off"
        static let optionArrowSingle = "option.arrow.single"
        static let optionArrowDouble = "option.arrow.double"
        static let optionAreaRectangle = "option.area.rectangle"
        static let optionAreaLasso = "option.area.lasso"
        static let statusAnnotationOn = "status.annotation.on"
        static let statusAnnotationOff = "status.annotation.off"

        fileprivate static let fixed: Set<String> = [
            optionOn,
            optionOff,
            optionArrowSingle,
            optionArrowDouble,
            optionAreaRectangle,
            optionAreaLasso,
            statusAnnotationOn,
            statusAnnotationOff,
        ]
    }

    private struct CacheKey: Hashable {
        let key: String
        let isSelected: Bool
    }

    private enum CachedImage {
        case image(NSImage)
        case unavailable
    }

    private static let iconDirectoryName = "Icons"
    private static let supportedExtensions = ["svg", "pdf"]
    private static var cache: [CacheKey: CachedImage] = [:]
    private static var didAuditBundledNames = false

    static func image(
        key: String,
        fallbackSystemName: String,
        selected: Bool = false,
        accessibilityDescription: String? = nil
    ) -> NSImage? {
        let cacheKey = CacheKey(key: key, isSelected: selected)
        if let cached = cache[cacheKey] {
            switch cached {
            case let .image(image): return image
            case .unavailable: return nil
            }
        }

        let image = loadImage(
            key: key,
            fallbackSystemName: fallbackSystemName,
            selected: selected,
            accessibilityDescription: accessibilityDescription
        )
        cache[cacheKey] = image.map(CachedImage.image) ?? .unavailable
        return image
    }

    static func image(
        key: String,
        fallbackSystemName: String,
        selected: Bool = false,
        attributeConfiguration: AttrIconConfiguration,
        pointSize: CGFloat,
        uiColor: NSColor,
        backgroundColor: NSColor,
        visualState: AttrIconVisualState,
        accessibilityDescription: String? = nil
    ) -> NSImage? {
        if let image = AttrIcon.image(
            configuration: attributeConfiguration,
            pointSize: pointSize,
            uiColor: uiColor,
            backgroundColor: backgroundColor,
            visualState: visualState,
            accessibilityDescription: accessibilityDescription
        ) {
            return image
        }

        guard let fallback = image(
            key: key,
            fallbackSystemName: fallbackSystemName,
            selected: selected,
            accessibilityDescription: accessibilityDescription
        ), let color = AttrIcon.colorComponents(uiColor)
        else {
            return nil
        }
        let size = NSSize(width: pointSize, height: pointSize)
        let tinted = AttrIcon.tintedImage(base: fallback, size: size, color: color)
        tinted.isTemplate = false
        tinted.accessibilityDescription = accessibilityDescription
        return tinted
    }

    static func effectiveAppearanceDidChange() {
        AttrIcon.effectiveAppearanceDidChange()
    }

    static func hasCustomAsset(key: String, selected: Bool = false) -> Bool {
        if selected, assetURL(named: "\(key).selected") != nil {
            return true
        }
        return assetURL(named: key) != nil
    }

    static func auditBundledNames(radialPaletteKeys: Set<String>) {
        guard !didAuditBundledNames else { return }
        didAuditBundledNames = true

        guard let directoryURL = iconDirectoryURL,
              FileManager.default.fileExists(atPath: directoryURL.path)
        else {
            return
        }

        let urls: [URL]
        do {
            urls = try FileManager.default.contentsOfDirectory(
                at: directoryURL,
                includingPropertiesForKeys: [.isRegularFileKey],
                options: [.skipsHiddenFiles]
            )
        } catch {
            NSLog("EduTools: failed to inspect bundled icon names: %@", error.localizedDescription)
            return
        }

        let knownKeys = Set(ShortcutCatalog.definitions.map(\.id))
            .union(radialPaletteKeys)
            .union(Key.fixed)
        let supportedURLs = urls.filter {
            supportedExtensions.contains($0.pathExtension.lowercased())
        }
        let regularAssetKeys = Set(supportedURLs.compactMap { url -> String? in
            let stem = url.deletingPathExtension().lastPathComponent
            return stem.hasSuffix(".selected") ? nil : stem
        })
        var invalidNames: [String] = []

        for url in urls {
            guard supportedExtensions.contains(url.pathExtension.lowercased()) else {
                invalidNames.append(url.lastPathComponent)
                continue
            }
            let stem = url.deletingPathExtension().lastPathComponent
            let isSelected = stem.hasSuffix(".selected")
            let key = isSelected ? String(stem.dropLast(".selected".count)) : stem
            if !knownKeys.contains(key) || (isSelected && !regularAssetKeys.contains(key)) {
                invalidNames.append(url.lastPathComponent)
            }
        }

        if !invalidNames.isEmpty {
            NSLog(
                "EduTools: unrecognized bundled icon asset names: %@",
                invalidNames.sorted().joined(separator: ", ")
            )
        }
    }

    private static var iconDirectoryURL: URL? {
        Bundle.main.resourceURL?.appendingPathComponent(iconDirectoryName, isDirectory: true)
    }

    private static func loadImage(
        key: String,
        fallbackSystemName: String,
        selected: Bool,
        accessibilityDescription: String?
    ) -> NSImage? {
        if selected, let selectedURL = assetURL(named: "\(key).selected") {
            return decodedImage(
                at: selectedURL,
                fallbackSystemName: fallbackSystemName,
                accessibilityDescription: accessibilityDescription
            )
        }
        if let regularURL = assetURL(named: key) {
            return decodedImage(
                at: regularURL,
                fallbackSystemName: fallbackSystemName,
                accessibilityDescription: accessibilityDescription
            )
        }
        return fallbackImage(
            systemName: fallbackSystemName,
            accessibilityDescription: accessibilityDescription
        )
    }

    private static func assetURL(named name: String) -> URL? {
        guard let directoryURL = iconDirectoryURL else { return nil }
        for pathExtension in supportedExtensions {
            let url = directoryURL
                .appendingPathComponent(name)
                .appendingPathExtension(pathExtension)
            if FileManager.default.fileExists(atPath: url.path) {
                return url
            }
        }
        return nil
    }

    private static func decodedImage(
        at url: URL,
        fallbackSystemName: String,
        accessibilityDescription: String?
    ) -> NSImage? {
        if let image = NSImage(contentsOf: url), !image.representations.isEmpty {
            image.isTemplate = true
            image.accessibilityDescription = accessibilityDescription
            return image
        }
        NSLog(
            "EduTools: failed to decode icon asset %@; using SF Symbol %@",
            url.lastPathComponent,
            fallbackSystemName
        )
        return fallbackImage(
            systemName: fallbackSystemName,
            accessibilityDescription: accessibilityDescription
        )
    }

    private static func fallbackImage(
        systemName: String,
        accessibilityDescription: String?
    ) -> NSImage? {
        guard let image = NSImage(
            systemSymbolName: systemName,
            accessibilityDescription: accessibilityDescription
        ) else {
            NSLog("EduTools: unavailable SF Symbol fallback %@", systemName)
            return nil
        }
        image.isTemplate = true
        return image
    }

    /// The linear palette uses a system surface, so its estimate follows effective appearance.
    static func estimatedSystemSurfaceColor(selected: Bool) -> NSColor {
        if selected {
            return .controlAccentColor
        }
        let match = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua])
        let whiteComponent = match == .darkAqua
            ? AttrIconBackgroundEstimate.darkSystemSurfaceWhiteComponent
            : AttrIconBackgroundEstimate.lightSurfaceWhiteComponent
        return NSColor(calibratedWhite: whiteComponent, alpha: 1)
    }
}
