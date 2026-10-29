import AppKit
import Foundation

enum AttrIconConstants {
    static let minimumContrastRatio = 1.8
    static let darkHaloAlpha = 0.55
    static let lightHaloAlpha = 0.7
    static let haloOffset: Double = 0.5
    static let lightBackgroundLuminance = 0.5
}

struct AttrIconColor: Hashable {
    let red: Double
    let green: Double
    let blue: Double
    let alpha: Double

    init(red: Double, green: Double, blue: Double, alpha: Double = 1) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }
}

enum AttrIconPure {
    enum ConditionError: Error, Equatable {
        case malformed(String)
        case unknownAttribute(String)
    }

    static func widthLevel(value: Double, lowerBound: Double, upperBound: Double) -> Int {
        guard upperBound > lowerBound else { return 2 }
        let normalized = min(max((value - lowerBound) / (upperBound - lowerBound), 0), 1)
        return 1 + min(3, Int(floor(4 * normalized)))
    }

    static func conditionMatches(
        _ expression: String,
        attributes: [String: String]
    ) throws -> Bool {
        let condition = expression.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !condition.isEmpty else { throw ConditionError.malformed(expression) }

        if let range = condition.range(of: "!=") {
            let name = String(condition[..<range.lowerBound])
            let expected = String(condition[range.upperBound...])
            guard isIdentifier(name), isIdentifier(expected) else {
                throw ConditionError.malformed(expression)
            }
            guard let actual = attributes[name] else {
                throw ConditionError.unknownAttribute(name)
            }
            return actual != expected
        }

        if let range = condition.range(of: "=") {
            let name = String(condition[..<range.lowerBound])
            let expected = String(condition[range.upperBound...])
            guard isIdentifier(name), isIdentifier(expected) else {
                throw ConditionError.malformed(expression)
            }
            guard let actual = attributes[name] else {
                throw ConditionError.unknownAttribute(name)
            }
            return actual == expected
        }

        if condition.hasPrefix("!") {
            let name = String(condition.dropFirst())
            guard isIdentifier(name) else { throw ConditionError.malformed(expression) }
            guard let actual = attributes[name] else {
                throw ConditionError.unknownAttribute(name)
            }
            return actual == "false"
        }

        guard isIdentifier(condition) else { throw ConditionError.malformed(expression) }
        guard let actual = attributes[condition] else {
            throw ConditionError.unknownAttribute(condition)
        }
        return actual == "true"
    }

    static func relativeLuminance(_ color: AttrIconColor) -> Double {
        func linearized(_ component: Double) -> Double {
            let clamped = min(max(component, 0), 1)
            return clamped <= 0.03928
                ? clamped / 12.92
                : pow((clamped + 0.055) / 1.055, 2.4)
        }

        return 0.2126 * linearized(color.red)
            + 0.7152 * linearized(color.green)
            + 0.0722 * linearized(color.blue)
    }

    static func contrastRatio(_ first: AttrIconColor, _ second: AttrIconColor) -> Double {
        let firstLuminance = relativeLuminance(first)
        let secondLuminance = relativeLuminance(second)
        let lighter = max(firstLuminance, secondLuminance)
        let darker = min(firstLuminance, secondLuminance)
        return (lighter + 0.05) / (darker + 0.05)
    }

    static func needsHalo(
        foreground: AttrIconColor,
        background: AttrIconColor
    ) -> Bool {
        contrastRatio(foreground, background) < AttrIconConstants.minimumContrastRatio
    }

    private static func isIdentifier(_ value: String) -> Bool {
        guard let first = value.unicodeScalars.first,
              CharacterSet.letters.union(CharacterSet(charactersIn: "_")).contains(first)
        else {
            return false
        }
        return value.unicodeScalars.dropFirst().allSatisfy {
            CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "_-")).contains($0)
        }
    }
}

struct AttrIconConfiguration: Hashable {
    struct Attribute: Hashable {
        let name: String
        let value: String
    }

    let toolKey: String
    let attributes: [Attribute]
    let color: AttrIconColor?

    init(toolKey: String, attributes: [String: String], color: AttrIconColor?) {
        self.toolKey = toolKey
        self.attributes = attributes.map { Attribute(name: $0.key, value: $0.value) }
            .sorted { $0.name < $1.name }
        self.color = color
    }

    var attributeValues: [String: String] {
        Dictionary(uniqueKeysWithValues: attributes.map { ($0.name, $0.value) })
    }
}

struct AttrIconVisualState: Hashable {
    let isSelected: Bool
    let isEnabled: Bool
    let isHovered: Bool
    let isPassThrough: Bool

    static let normal = AttrIconVisualState(
        isSelected: false,
        isEnabled: true,
        isHovered: false,
        isPassThrough: false
    )
}

enum AttrIconToolCatalog {
    // The only app-tool-to-manifest-name table. Manifest layers remain entirely data-driven.
    static let manifestKeyByTool: [InkTool: String] = [
        .pen: "tool.pen",
        .highlighter: "tool.highlighter",
        .text: "tool.text",
        .arrow: "tool.arrow",
        .rectangle: "tool.rect",
        .ellipse: "tool.circle",
        .eraser: "tool.eraser",
        .laser: "tool.laser",
    ]
}

enum AttrIcon {
    private struct Manifest: Decodable {
        let grid: Int
        let widthLevels: [String: String]
        let tools: [String: ToolDefinition]
    }

    private struct ToolDefinition: Decodable {
        let attrs: [String]
        let layers: [LayerDefinition]
    }

    private struct LayerDefinition: Decodable {
        let nameTemplate: String
        let colorKind: String
        let condition: String?

        init(from decoder: Decoder) throws {
            var values = try decoder.unkeyedContainer()
            nameTemplate = try values.decode(String.self)
            colorKind = try values.decode(String.self)
            condition = values.isAtEnd ? nil : try values.decode(String.self)
            guard values.isAtEnd else {
                throw DecodingError.dataCorruptedError(
                    in: values,
                    debugDescription: "An attr icon layer must contain two or three values"
                )
            }
        }
    }

    private struct RenderCacheKey: Hashable {
        let configuration: AttrIconConfiguration
        let visualState: AttrIconVisualState
        let pointSize: Double
        let uiColor: AttrIconColor
        let backgroundColor: AttrIconColor
    }

    private struct Repository {
        let directoryURL: URL?
        let manifest: Manifest?
        let invalidTools: Set<String>
    }

    private enum CachedLayer {
        case image(NSImage)
        case unavailable
    }

    private struct Snapshot {
        let tool: InkTool
        let color: NSColor?
        let size: Double
        let arrowStyle: ArrowStyle?
        let eraserMode: EraserMode?
        let eraserAreaShape: EraserAreaShape?
    }

    static var manifestKeyByTool: [InkTool: String] {
        AttrIconToolCatalog.manifestKeyByTool
    }

    private static var repositoryResourceURL = Bundle.main.resourceURL
    private static var repository = loadRepository(resourceURL: repositoryResourceURL)
    private static var renderedImages: [RenderCacheKey: NSImage] = [:]
    private static var decodedLayers: [String: CachedLayer] = [:]
    private static var loggedReasons: Set<String> = []

    static func configuration(
        for tool: InkTool,
        state: AppState,
        settings: AppSettings
    ) -> AttrIconConfiguration? {
        if let favorite = state.favoriteSnapshot(for: tool) {
            return configuration(for: favorite, settings: settings)
        }
        guard tool == .laser else { return nil }
        return configuration(
            for: Snapshot(
                tool: .laser,
                color: state.laserColor,
                size: state.laserTrailDuration,
                arrowStyle: nil,
                eraserMode: nil,
                eraserAreaShape: nil
            ),
            settings: settings
        )
    }

    static func configuration(
        for favorite: FavoriteTool,
        settings: AppSettings
    ) -> AttrIconConfiguration? {
        guard favorite.isValid(colorPresetCount: settings.tools.colors.count),
              let tool = favorite.inkTool
        else {
            return nil
        }
        let color = favorite.colorIndex.map { settings.tools.colors[$0].nsColor }
        return configuration(
            for: Snapshot(
                tool: tool,
                color: color,
                size: favorite.size,
                arrowStyle: favorite.decodedArrowStyle,
                eraserMode: favorite.decodedEraserMode,
                eraserAreaShape: favorite.decodedEraserAreaShape
            ),
            settings: settings
        )
    }

    private static func configuration(
        for snapshot: Snapshot,
        settings: AppSettings
    ) -> AttrIconConfiguration? {
        let tool = snapshot.tool
        guard let toolKey = manifestKeyByTool[tool] else { return nil }

        func snapshotColor() -> AttrIconColor? {
            guard let color = snapshot.color, let components = colorComponents(color) else {
                logOnce(
                    "invalid-snapshot-color.\(toolKey)",
                    "attr icon \(toolKey) has no valid snapshot color; using the general icon"
                )
                return nil
            }
            return components
        }

        func snapshotWidth(in range: ClosedRange<CGFloat>) -> String {
            String(AttrIconPure.widthLevel(
                value: snapshot.size,
                lowerBound: Double(range.lowerBound),
                upperBound: Double(range.upperBound)
            ))
        }

        switch tool {
        case .pen, .arrow, .rectangle, .ellipse:
            guard let color = snapshotColor() else { return nil }
            let width = snapshotWidth(in: requiredRange(RadialPaletteMetrics.writingWidths))
            var attributes = ["width": width]
            if tool == .arrow {
                guard let arrowStyle = snapshot.arrowStyle else { return nil }
                attributes["heads"] = arrowStyle.rawValue
            } else if tool == .rectangle || tool == .ellipse {
                attributes["fill"] = "false"
            }
            return AttrIconConfiguration(toolKey: toolKey, attributes: attributes, color: color)

        case .highlighter:
            guard let color = snapshotColor() else { return nil }
            let width = snapshotWidth(in: requiredRange(RadialPaletteMetrics.highlighterWidths))
            return AttrIconConfiguration(
                toolKey: toolKey,
                attributes: ["width": width],
                color: color
            )

        case .text:
            guard let color = snapshotColor() else { return nil }
            let width = snapshotWidth(in: requiredRange(RadialPaletteMetrics.textSizes))
            return AttrIconConfiguration(
                toolKey: toolKey,
                attributes: ["width": width],
                color: color
            )

        case .eraser:
            guard let lowerBound = settings.eraser.presetDiameters.min(),
                  let upperBound = settings.eraser.presetDiameters.max()
            else {
                logOnce(
                    "missing-eraser-presets.\(toolKey)",
                    "attr icon \(toolKey) has no size presets; using the general icon"
                )
                return nil
            }
            let width = AttrIconPure.widthLevel(
                value: snapshot.size,
                lowerBound: lowerBound,
                upperBound: upperBound
            )
            guard let eraserMode = snapshot.eraserMode,
                  let eraserAreaShape = snapshot.eraserAreaShape
            else {
                return nil
            }
            let mode: String
            switch eraserMode {
            case .partial: mode = "brush"
            case .stroke: mode = "stroke"
            case .area:
                mode = eraserAreaShape == .rectangle ? "rect" : "lasso"
            }
            return AttrIconConfiguration(
                toolKey: toolKey,
                attributes: ["width": String(width), "mode": mode],
                color: nil
            )

        case .laser:
            guard let color = snapshotColor() else { return nil }
            return AttrIconConfiguration(
                toolKey: toolKey,
                attributes: ["trail": "true"],
                color: color
            )

        case .select, .cover:
            return nil
        }
    }

    static func image(
        configuration: AttrIconConfiguration,
        pointSize: CGFloat,
        uiColor: NSColor,
        backgroundColor: NSColor,
        visualState: AttrIconVisualState,
        accessibilityDescription: String?
    ) -> NSImage? {
        guard let uiComponents = colorComponents(uiColor),
              let backgroundComponents = colorComponents(backgroundColor)
        else {
            logOnce(
                "unresolved-ui-color",
                "attr icon colors could not be resolved; using the general icon"
            )
            return nil
        }
        let cacheKey = RenderCacheKey(
            configuration: configuration,
            visualState: visualState,
            pointSize: Double(pointSize),
            uiColor: uiComponents,
            backgroundColor: backgroundComponents
        )
        if let cached = renderedImages[cacheKey] {
            cached.accessibilityDescription = accessibilityDescription
            return cached
        }

        guard let manifest = repository.manifest else {
            logOnce(
                "manifest-unavailable",
                "attr icon manifest is unavailable; using general icons"
            )
            return nil
        }
        guard !repository.invalidTools.contains(configuration.toolKey),
              let tool = manifest.tools[configuration.toolKey]
        else {
            logOnce(
                "invalid-tool.\(configuration.toolKey)",
                "attr icon \(configuration.toolKey) is unavailable or incomplete; using the general icon"
            )
            return nil
        }

        let attributes = configuration.attributeValues
        for requiredAttribute in tool.attrs {
            if requiredAttribute == "color" {
                guard configuration.color != nil else {
                    logOnce(
                        "missing-attribute.\(configuration.toolKey).color",
                        "attr icon \(configuration.toolKey) is missing color; using the general icon"
                    )
                    return nil
                }
            } else if attributes[requiredAttribute] == nil {
                logOnce(
                    "missing-attribute.\(configuration.toolKey).\(requiredAttribute)",
                    "attr icon \(configuration.toolKey) is missing \(requiredAttribute); using the general icon"
                )
                return nil
            }
        }

        var layers: [(image: NSImage, color: AttrIconColor, hasHalo: Bool)] = []
        for layer in tool.layers {
            if let condition = layer.condition {
                do {
                    guard try AttrIconPure.conditionMatches(condition, attributes: attributes) else {
                        continue
                    }
                } catch {
                    logOnce(
                        "condition.\(configuration.toolKey).\(condition)",
                        "attr icon \(configuration.toolKey) has unknown condition \(condition); using the general icon"
                    )
                    return nil
                }
            }

            guard let layerName = resolvedLayerName(
                template: layer.nameTemplate,
                attributes: attributes
            ) else {
                logOnce(
                    "template.\(configuration.toolKey).\(layer.nameTemplate)",
                    "attr icon \(configuration.toolKey) could not resolve layer \(layer.nameTemplate); using the general icon"
                )
                return nil
            }
            guard let baseImage = decodedLayer(named: layerName) else {
                logOnce(
                    "layer.\(configuration.toolKey).\(layerName)",
                    "attr icon layer \(layerName).svg is missing or unreadable; using the general icon for \(configuration.toolKey)"
                )
                return nil
            }

            let layerColor: AttrIconColor
            switch layer.colorKind {
            case "ui":
                layerColor = uiComponents
            case "attr":
                guard let color = configuration.color else {
                    logOnce(
                        "attr-color.\(configuration.toolKey)",
                        "attr icon \(configuration.toolKey) needs an attribute color; using the general icon"
                    )
                    return nil
                }
                layerColor = color
            default:
                logOnce(
                    "color-kind.\(configuration.toolKey).\(layer.colorKind)",
                    "attr icon \(configuration.toolKey) has unknown color kind \(layer.colorKind); using the general icon"
                )
                return nil
            }
            let hasHalo = layer.colorKind == "attr"
                && !layerName.hasSuffix(".fill")
                && AttrIconPure.needsHalo(
                    foreground: layerColor,
                    background: backgroundComponents
                )
            layers.append((baseImage, layerColor, hasHalo))
        }

        let size = NSSize(width: pointSize, height: pointSize)
        let image = NSImage(size: size, flipped: false) { bounds in
            for layer in layers {
                if layer.hasHalo {
                    let haloColor = haloColor(for: backgroundComponents)
                    let haloImage = tintedImage(
                        base: layer.image,
                        size: size,
                        color: haloColor
                    )
                    let offset = CGFloat(AttrIconConstants.haloOffset)
                    for delta in [
                        CGPoint(x: -offset, y: 0),
                        CGPoint(x: offset, y: 0),
                        CGPoint(x: 0, y: -offset),
                        CGPoint(x: 0, y: offset),
                    ] {
                        haloImage.draw(
                            in: bounds.offsetBy(dx: delta.x, dy: delta.y),
                            from: .zero,
                            operation: .sourceOver,
                            fraction: 1
                        )
                    }
                }
                tintedImage(base: layer.image, size: size, color: layer.color).draw(
                    in: bounds,
                    from: .zero,
                    operation: .sourceOver,
                    fraction: 1
                )
            }
            return true
        }
        image.isTemplate = false
        image.accessibilityDescription = accessibilityDescription
        renderedImages[cacheKey] = image
        return image
    }

    static func tintedImage(base: NSImage, size: NSSize, color: AttrIconColor) -> NSImage {
        NSImage(size: size, flipped: false) { bounds in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            base.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1)
            context.saveGState()
            context.setBlendMode(.sourceIn)
            context.setFillColor(
                red: CGFloat(color.red),
                green: CGFloat(color.green),
                blue: CGFloat(color.blue),
                alpha: CGFloat(color.alpha)
            )
            context.fill(bounds)
            context.restoreGState()
            return true
        }
    }

    static func colorComponents(_ color: NSColor) -> AttrIconColor? {
        var converted: NSColor?
        NSApp.effectiveAppearance.performAsCurrentDrawingAppearance {
            converted = color.usingColorSpace(.deviceRGB)
        }
        guard let converted else { return nil }
        return AttrIconColor(
            red: Double(converted.redComponent),
            green: Double(converted.greenComponent),
            blue: Double(converted.blueComponent),
            alpha: Double(converted.alphaComponent)
        )
    }

    static func auditBundledAssets(resourceURL: URL? = Bundle.main.resourceURL) {
        if repositoryResourceURL != resourceURL {
            repositoryResourceURL = resourceURL
            repository = loadRepository(resourceURL: resourceURL)
            renderedImages.removeAll(keepingCapacity: true)
            decodedLayers.removeAll(keepingCapacity: true)
        }
        _ = repository
    }

    static func effectiveAppearanceDidChange() {
        renderedImages.removeAll(keepingCapacity: true)
    }

    private static func requiredRange(_ values: [CGFloat]) -> ClosedRange<CGFloat> {
        guard let lowerBound = values.min(), let upperBound = values.max() else {
            preconditionFailure("Attr icon width presets must not be empty")
        }
        return lowerBound...upperBound
    }

    private static func resolvedLayerName(
        template: String,
        attributes: [String: String]
    ) -> String? {
        var result = template
        for (name, value) in attributes {
            result = result.replacingOccurrences(of: "{\(name)}", with: value)
        }
        return result.contains("{") || result.contains("}") ? nil : result
    }

    private static func decodedLayer(named name: String) -> NSImage? {
        if let cached = decodedLayers[name] {
            switch cached {
            case let .image(image): return image
            case .unavailable: return nil
            }
        }
        guard let directoryURL = repository.directoryURL else {
            decodedLayers[name] = .unavailable
            return nil
        }
        let url = directoryURL.appendingPathComponent(name).appendingPathExtension("svg")
        guard FileManager.default.fileExists(atPath: url.path),
              let image = NSImage(contentsOf: url),
              !image.representations.isEmpty
        else {
            decodedLayers[name] = .unavailable
            return nil
        }
        image.isTemplate = true
        decodedLayers[name] = .image(image)
        return image
    }

    private static func haloColor(for background: AttrIconColor) -> AttrIconColor {
        if AttrIconPure.relativeLuminance(background) >= AttrIconConstants.lightBackgroundLuminance {
            return AttrIconColor(red: 0, green: 0, blue: 0, alpha: AttrIconConstants.darkHaloAlpha)
        }
        return AttrIconColor(red: 1, green: 1, blue: 1, alpha: AttrIconConstants.lightHaloAlpha)
    }

    private static func loadRepository(resourceURL: URL?) -> Repository {
        let directoryURL = resourceURL?
            .appendingPathComponent("Icons", isDirectory: true)
            .appendingPathComponent("Attr", isDirectory: true)
        guard let directoryURL else {
            return Repository(directoryURL: nil, manifest: nil, invalidTools: [])
        }
        let manifestURL = directoryURL.appendingPathComponent("manifest.json")
        guard FileManager.default.fileExists(atPath: manifestURL.path) else {
            return Repository(directoryURL: directoryURL, manifest: nil, invalidTools: [])
        }

        let manifest: Manifest
        do {
            manifest = try JSONDecoder().decode(Manifest.self, from: Data(contentsOf: manifestURL))
        } catch {
            logOnce(
                "manifest-parse",
                "failed to parse attr icon manifest: \(error.localizedDescription); using general icons"
            )
            return Repository(directoryURL: directoryURL, manifest: nil, invalidTools: [])
        }

        var invalidTools: Set<String> = []
        let appToolKeys = Set(manifestKeyByTool.values)
        let manifestToolKeys = Set(manifest.tools.keys)
        for key in manifestToolKeys.subtracting(appToolKeys).sorted() {
            logOnce("unmapped-manifest-tool.\(key)", "attr icon manifest tool \(key) has no app mapping; ignoring it")
        }
        for key in appToolKeys.subtracting(manifestToolKeys).sorted() {
            logOnce("missing-manifest-tool.\(key)", "app attr icon tool \(key) is absent from the manifest; ignoring it")
        }

        let fileManager = FileManager.default
        let fileURLs: [URL]
        do {
            fileURLs = try fileManager.contentsOfDirectory(
                at: directoryURL,
                includingPropertiesForKeys: [.isRegularFileKey],
                options: [.skipsHiddenFiles]
            )
        } catch {
            logOnce(
                "directory-audit",
                "failed to inspect attr icon bundle: \(error.localizedDescription); using general icons"
            )
            return Repository(
                directoryURL: directoryURL,
                manifest: manifest,
                invalidTools: Set(manifest.tools.keys)
            )
        }
        let availableNames = Set(fileURLs.compactMap { url in
            url.pathExtension.lowercased() == "svg"
                ? url.deletingPathExtension().lastPathComponent
                : nil
        })
        var expectedNames: Set<String> = []
        let substitutions = [
            "width": manifest.widthLevels.keys.sorted(),
            "heads": [ArrowStyle.single.rawValue, ArrowStyle.double.rawValue],
        ]

        for (toolKey, tool) in manifest.tools {
            for layer in tool.layers {
                if layer.colorKind != "ui", layer.colorKind != "attr" {
                    invalidTools.insert(toolKey)
                    logOnce(
                        "audit-color-kind.\(toolKey).\(layer.colorKind)",
                        "attr icon \(toolKey) has unknown color kind \(layer.colorKind); using the general icon"
                    )
                }
                if let condition = layer.condition {
                    do {
                        _ = try AttrIconPure.conditionMatches(
                            condition,
                            attributes: Dictionary(uniqueKeysWithValues: tool.attrs.map { ($0, "true") })
                        )
                    } catch {
                        invalidTools.insert(toolKey)
                        logOnce(
                            "audit-condition.\(toolKey).\(condition)",
                            "attr icon \(toolKey) has unknown condition \(condition); using the general icon"
                        )
                    }
                }

                guard let expandedNames = expandedLayerNames(
                    template: layer.nameTemplate,
                    substitutions: substitutions
                ) else {
                    invalidTools.insert(toolKey)
                    logOnce(
                        "audit-template.\(toolKey).\(layer.nameTemplate)",
                        "attr icon \(toolKey) has an unknown layer placeholder in \(layer.nameTemplate); using the general icon"
                    )
                    continue
                }
                expectedNames.formUnion(expandedNames)
                let missingNames = Set(expandedNames).subtracting(availableNames)
                if !missingNames.isEmpty {
                    invalidTools.insert(toolKey)
                    logOnce(
                        "audit-missing.\(toolKey).\(layer.nameTemplate)",
                        "attr icon \(toolKey) is missing layers: \(missingNames.sorted().joined(separator: ", ")); using the general icon"
                    )
                }
            }
        }

        let unusedNames = availableNames.subtracting(expectedNames)
        if !unusedNames.isEmpty {
            logOnce(
                "audit-unused",
                "unused attr icon layers: \(unusedNames.sorted().joined(separator: ", "))"
            )
        }
        return Repository(
            directoryURL: directoryURL,
            manifest: manifest,
            invalidTools: invalidTools
        )
    }

    private static func expandedLayerNames(
        template: String,
        substitutions: [String: [String]]
    ) -> [String]? {
        var results = [template]
        while let result = results.first(where: { $0.contains("{") || $0.contains("}") }) {
            guard let start = result.firstIndex(of: "{"),
                  let end = result[start...].firstIndex(of: "}"),
                  start < end
            else {
                return nil
            }
            let name = String(result[result.index(after: start)..<end])
            guard let values = substitutions[name], !values.isEmpty else { return nil }
            results.removeAll { $0 == result }
            for value in values {
                results.append(result.replacingOccurrences(of: "{\(name)}", with: value))
            }
        }
        return results
    }

    private static func logOnce(_ reason: String, _ message: String) {
        guard loggedReasons.insert(reason).inserted else { return }
        NSLog("EduTools: %@", message)
    }
}
