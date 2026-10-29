import AppKit
import Combine
import SwiftUI

struct RadialPaletteSectorGeometry {
    let innerRadius: CGFloat
    let outerRadius: CGFloat
    let startAngle: CGFloat
    let endAngle: CGFloat

    func contains(radius: CGFloat, clockwiseAngle: CGFloat) -> Bool {
        guard radius >= innerRadius, radius <= outerRadius else { return false }
        let relative = Self.normalizedAngle(clockwiseAngle - startAngle)
        return relative <= endAngle - startAngle
    }

    func inset(by amount: CGFloat) -> RadialPaletteSectorGeometry {
        RadialPaletteSectorGeometry(
            innerRadius: innerRadius + amount,
            outerRadius: max(innerRadius + amount, outerRadius - amount),
            startAngle: startAngle,
            endAngle: endAngle
        )
    }

    private static func normalizedAngle(_ angle: CGFloat) -> CGFloat {
        let full = 2 * CGFloat.pi
        let result = angle.truncatingRemainder(dividingBy: full)
        return result >= 0 ? result : result + full
    }
}

enum RadialPaletteMetrics {
    static let collapsedDiameter: CGFloat = 44
    static let expandedDiameter: CGFloat = 280
    // 3pt focus-ring overhang plus room for the Liquid Glass edge light and shadow.
    static let windowMargin: CGFloat = 10
    static let centerRadius: CGFloat = collapsedDiameter / 2
    static let groupInnerRadius: CGFloat = 28
    static let groupOuterRadius: CGFloat = 58
    static let entryInnerRadius: CGFloat = 64
    static let entryOuterRadius: CGFloat = 96
    static let optionInnerRadius: CGFloat = 102
    static let optionOuterRadius: CGFloat = 138
    // 속성 라벨 칩이 가운데 칸의 안쪽 가장자리에 걸치므로 내용을 바깥으로 비킨다.
    static let optionContentRadialOffset: CGFloat = 5
    static let dragThreshold: CGFloat = 4
    static let commandHoldDuration: TimeInterval = 0.5
    static let sectorGap: CGFloat = 2 * .pi / 180
    static let propertyGap: CGFloat = 4 * .pi / 180
    static let tooltipGap: CGFloat = 8
    static let propertyLabelFontSize: CGFloat = 9.5
    static let propertyLabelPadding: CGFloat = 6
    static let cellStrokeWidth: CGFloat = 0.8
    static let hoverStrokeWidth: CGFloat = 1.8
    static let selectedStrokeWidth: CGFloat = 1.2
    static let colorSelectionOuterWidth: CGFloat = 4.2
    static let colorSelectionInnerWidth: CGFloat = 1.8
    static let focusOuterWidth: CGFloat = 6
    static let focusInnerWidth: CGFloat = 2.4
    static let writingWidths: [CGFloat] = [1, 3, 5, 8, 12]
    static let highlighterWidths: [CGFloat] = [6, 12, 18, 24, 30]
    static let textSizes: [CGFloat] = [16, 24, 36, 48, 72]
    static let highlighterOpacities: [CGFloat] = [0.2, 0.4, 0.6, 0.8, 1]
    static let laserTrailDurations: [TimeInterval] = [0.3, 0.7, 1, 2]
    static let colorPreviewDiameter: CGFloat = 14
    static let previewSlotWidth: CGFloat = 18
    static let previewSlotHeight: CGFloat = 15
    static let previewLabelFontSize: CGFloat = 7.5
    static let previewSpacing: CGFloat = 0
    static let previewOutlineOuterWidth: CGFloat = 1
    static let previewOutlineInnerWidth: CGFloat = 0.35
    static let strokePreviewLength: CGFloat = 16
    static let strokePreviewMinimumWidth: CGFloat = 1.5
    static let strokePreviewMaximumWidth: CGFloat = 6
    static let textPreviewMinimumSize: CGFloat = 8
    static let textPreviewMaximumSize: CGFloat = 14
    static let opacityPreviewWidth: CGFloat = 16
    static let opacityPreviewHeight: CGFloat = 10
    static let opacityPreviewCornerRadius: CGFloat = 2
    static let eraserPreviewMinimumDiameter: CGFloat = 5
    static let eraserPreviewMaximumDiameter: CGFloat = 15
    static let eraserPreviewStrokeWidth: CGFloat = 1.2
}

enum RadialPalettePreview: Equatable {
    case strokeWidth(relative: CGFloat, color: NSColor)
    case textSize(relative: CGFloat, color: NSColor)
    case opacity(CGFloat, color: NSColor)
    case eraserDiameter(relative: CGFloat)

    static func relativeSize(of value: CGFloat, in values: [CGFloat]) -> CGFloat {
        guard let minimum = values.min(),
              let maximum = values.max(),
              maximum > minimum
        else {
            return 0.5
        }
        return min(max((value - minimum) / (maximum - minimum), 0), 1)
    }
}

struct RadialPaletteRenderCell: Identifiable {
    let id: String
    let ring: RadialPaletteRing
    let geometry: RadialPaletteSectorGeometry
    let iconKey: String?
    let iconName: String?
    let attributeConfiguration: AttrIconConfiguration?
    let accessibilityLabel: String
    let shortLabel: String
    let shortcut: String
    let color: NSColor?
    let preview: RadialPalettePreview?
    let isSelected: Bool
    let isEnabled: Bool
    let isHovered: Bool
    let isFocused: Bool

    init(
        id: String,
        ring: RadialPaletteRing,
        geometry: RadialPaletteSectorGeometry,
        iconKey: String?,
        iconName: String?,
        attributeConfiguration: AttrIconConfiguration?,
        accessibilityLabel: String,
        shortLabel: String,
        shortcut: String,
        color: NSColor?,
        preview: RadialPalettePreview? = nil,
        isSelected: Bool,
        isEnabled: Bool,
        isHovered: Bool,
        isFocused: Bool
    ) {
        self.id = id
        self.ring = ring
        self.geometry = geometry
        self.iconKey = iconKey
        self.iconName = iconName
        self.attributeConfiguration = attributeConfiguration
        self.accessibilityLabel = accessibilityLabel
        self.shortLabel = shortLabel
        self.shortcut = shortcut
        self.color = color
        self.preview = preview
        self.isSelected = isSelected
        self.isEnabled = isEnabled
        self.isHovered = isHovered
        self.isFocused = isFocused
    }
}

struct RadialPaletteRenderPropertyLabel: Identifiable {
    let id: String
    let name: String
    let angle: CGFloat
    let radius: CGFloat
}

struct RadialPaletteRenderState {
    let isExpanded: Bool
    let showsKeyCaps: Bool
    let faceTone: RadialPaletteFaceTone
    let cells: [RadialPaletteRenderCell]
    let propertyLabels: [RadialPaletteRenderPropertyLabel]
    let centerIconKey: String?
    let centerIconName: String?
    let centerAttributeConfiguration: AttrIconConfiguration?
    let centerAccessibilityLabel: String?
    let centerShortcut: String
    let centerColor: NSColor
    let centerStrokeWidth: CGFloat
    let centerIsPassThroughEnabled: Bool
    let centerIsHovered: Bool
    let centerIsFocused: Bool

    static let empty = RadialPaletteRenderState(
        isExpanded: false,
        showsKeyCaps: false,
        faceTone: .light,
        cells: [],
        propertyLabels: [],
        centerIconKey: nil,
        centerIconName: nil,
        centerAttributeConfiguration: nil,
        centerAccessibilityLabel: nil,
        centerShortcut: "",
        centerColor: .clear,
        centerStrokeWidth: 2,
        centerIsPassThroughEnabled: false,
        centerIsHovered: false,
        centerIsFocused: false
    )
}

final class RadialPaletteRenderModel: ObservableObject {
    @Published var glassOpacity = RadialPaletteGlassStyle.defaultOpacity
    @Published var state = RadialPaletteRenderState.empty
}

final class RadialPaletteRenderingView: NSView {
    private let hostingView: NSHostingView<RadialPaletteGlassView>

    init(model: RadialPaletteRenderModel) {
        hostingView = NSHostingView(rootView: RadialPaletteGlassView(model: model))
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        hostingView.wantsLayer = true
        hostingView.layer?.backgroundColor = NSColor.clear.cgColor
        hostingView.frame = bounds
        hostingView.autoresizingMask = [.width, .height]
        addSubview(hostingView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isOpaque: Bool { false }

    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }
}

struct RadialSectorShape: Shape {
    let geometry: RadialPaletteSectorGeometry

    func path(in rect: CGRect) -> Path {
        let center = CGPoint(x: rect.midX, y: rect.midY)
        var path = Path()
        path.addArc(
            center: center,
            radius: geometry.outerRadius,
            startAngle: .radians(Double(geometry.startAngle - .pi / 2)),
            endAngle: .radians(Double(geometry.endAngle - .pi / 2)),
            clockwise: false
        )
        path.addLine(to: Self.point(
            center: center,
            radius: geometry.innerRadius,
            angle: geometry.endAngle
        ))
        path.addArc(
            center: center,
            radius: geometry.innerRadius,
            startAngle: .radians(Double(geometry.endAngle - .pi / 2)),
            endAngle: .radians(Double(geometry.startAngle - .pi / 2)),
            clockwise: true
        )
        path.closeSubpath()
        return path
    }

    static func point(center: CGPoint, radius: CGFloat, angle: CGFloat) -> CGPoint {
        CGPoint(
            x: center.x + sin(angle) * radius,
            y: center.y - cos(angle) * radius
        )
    }
}

private struct RadialPaletteGlassSurfaceModifier<Surface: Shape>: ViewModifier {
    let glass: Glass
    let surface: Surface
    let faceOpacity: Double?
    let faceTone: RadialPaletteFaceTone

    func body(content: Content) -> some View {
        content
            .glassEffect(glass, in: surface)
            .overlay {
                if let faceOpacity {
                    surface.fill(
                        Color(nsColor: RadialPaletteGlassStyle.surfaceFaceColor(
                            opacity: faceOpacity,
                            tone: faceTone
                        ))
                    )
                }
            }
    }
}

private extension View {
    func radialPaletteGlass<Surface: Shape>(
        _ glass: Glass,
        in surface: Surface,
        faceOpacity: Double? = nil,
        tone: RadialPaletteFaceTone
    ) -> some View {
        modifier(RadialPaletteGlassSurfaceModifier(
            glass: glass,
            surface: surface,
            faceOpacity: faceOpacity,
            faceTone: tone
        ))
    }

    func radialPalettePreviewOutline<Outline: Shape>(
        in outline: Outline,
        tone: RadialPaletteFaceTone
    ) -> some View {
        overlay {
            outline.stroke(
                Color(nsColor: RadialPaletteGlassStyle.regularBorderColor(tone: tone)),
                lineWidth: RadialPaletteMetrics.previewOutlineOuterWidth
            )
        }
        .overlay {
            outline.stroke(
                Color(nsColor: RadialPaletteGlassStyle.previewOutlineInnerColor(tone: tone)),
                lineWidth: RadialPaletteMetrics.previewOutlineInnerWidth
            )
        }
    }
}

struct RadialPaletteGlassView: View {
    @ObservedObject var model: RadialPaletteRenderModel

    var body: some View {
        GeometryReader { proxy in
            let bounds = CGRect(origin: .zero, size: proxy.size)
            ZStack {
                if model.state.isExpanded {
                    ForEach(model.state.cells) { cell in
                        cellView(cell, in: bounds)
                    }
                    ForEach(model.state.propertyLabels) { label in
                        propertyLabel(label, in: bounds)
                    }
                }
                centerView(in: bounds)
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
        }
        .background(Color.clear)
        .allowsHitTesting(false)
    }

    private func cellView(_ cell: RadialPaletteRenderCell, in bounds: CGRect) -> some View {
        let shape = RadialSectorShape(geometry: cell.geometry)
        let contentPosition = RadialSectorShape.point(
            center: CGPoint(x: bounds.midX, y: bounds.midY),
            radius: (cell.geometry.innerRadius + cell.geometry.outerRadius) / 2
                + (cell.ring == .option ? RadialPaletteMetrics.optionContentRadialOffset : 0),
            angle: (cell.geometry.startAngle + cell.geometry.endAngle) / 2
        )
        return ZStack {
            Color.clear
                .frame(width: bounds.width, height: bounds.height)
                .radialPaletteGlass(
                    glass(for: cell),
                    in: shape,
                    faceOpacity: cell.isSelected && cell.color == nil
                        ? nil
                        : model.glassOpacity,
                    tone: model.state.faceTone
                )

            shape.stroke(baseBorderColor(for: cell), lineWidth: baseBorderWidth(for: cell))

            if cell.isFocused {
                shape.stroke(
                    Color(nsColor: RadialPaletteGlassStyle.focusOuterColor(
                        tone: model.state.faceTone
                    )),
                    lineWidth: RadialPaletteMetrics.focusOuterWidth
                )
                shape.stroke(
                    Color(nsColor: RadialPaletteGlassStyle.selectedContentColor),
                    lineWidth: RadialPaletteMetrics.focusInnerWidth
                )
            }

            if cell.color != nil, cell.isSelected {
                let selectionShape = RadialSectorShape(
                    geometry: cell.geometry.inset(by: cell.isFocused ? 4 : 0)
                )
                selectionShape.stroke(
                    Color(nsColor: RadialPaletteGlassStyle.colorSelectionOuterColor(
                        tone: model.state.faceTone
                    )),
                    lineWidth: RadialPaletteMetrics.colorSelectionOuterWidth
                )
                selectionShape.stroke(
                    Color(nsColor: RadialPaletteGlassStyle.selectedContentColor),
                    lineWidth: RadialPaletteMetrics.colorSelectionInnerWidth
                )
            }

            if model.state.showsKeyCaps, !cell.shortcut.isEmpty {
                keyCap(cell.shortcut)
                    .position(contentPosition)
            } else {
                cellContent(cell)
                    .position(contentPosition)
            }
        }
        .frame(width: bounds.width, height: bounds.height)
    }

    @ViewBuilder
    private func cellContent(_ cell: RadialPaletteRenderCell) -> some View {
        let contentColor = contentColor(for: cell)
        let icon = iconImage(for: cell)
        switch cell.ring {
        case .group:
            ZStack {
                if let icon {
                    Image(nsImage: icon)
                        .renderingMode(
                            cell.attributeConfiguration == nil ? .template : .original
                        )
                        .resizable()
                        .scaledToFit()
                        .frame(width: 13, height: 13)
                        .foregroundStyle(contentColor)
                        .offset(y: -4)
                        .accessibilityLabel(Text(cell.accessibilityLabel))
                }
                Text(cell.shortLabel)
                    .font(.system(size: 7.5, weight: .medium))
                    .foregroundStyle(contentColor)
                    .offset(y: 9)
            }
        case .entry:
            if let icon {
                Image(nsImage: icon)
                    .renderingMode(cell.attributeConfiguration == nil ? .template : .original)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 14, height: 14)
                    .foregroundStyle(contentColor)
                    .accessibilityLabel(Text(cell.accessibilityLabel))
            }
        case .option:
            if let color = cell.color {
                Circle()
                    .fill(Color(nsColor: color))
                    .frame(
                        width: RadialPaletteMetrics.colorPreviewDiameter,
                        height: RadialPaletteMetrics.colorPreviewDiameter
                    )
                    .radialPalettePreviewOutline(in: Circle(), tone: model.state.faceTone)
                    .accessibilityLabel(Text(cell.accessibilityLabel))
            } else if let preview = cell.preview {
                VStack(spacing: RadialPaletteMetrics.previewSpacing) {
                    previewContent(preview, contentColor: contentColor)
                        .frame(
                            width: RadialPaletteMetrics.previewSlotWidth,
                            height: RadialPaletteMetrics.previewSlotHeight
                        )
                    Text(cell.shortLabel)
                        .font(.system(
                            size: RadialPaletteMetrics.previewLabelFontSize,
                            weight: .medium
                        ))
                        .foregroundStyle(contentColor)
                }
                .accessibilityLabel(Text(cell.accessibilityLabel))
            } else {
                ZStack {
                    if let icon {
                        Image(nsImage: icon)
                            .renderingMode(
                                cell.attributeConfiguration == nil ? .template : .original
                            )
                            .resizable()
                            .scaledToFit()
                            .frame(width: 10, height: 10)
                            .foregroundStyle(contentColor)
                            .offset(y: -3)
                            .accessibilityLabel(Text(cell.accessibilityLabel))
                    }
                    Text(cell.shortLabel)
                        .font(.system(size: 8, weight: .medium))
                        .foregroundStyle(contentColor)
                        .offset(y: icon == nil ? 0 : 7)
                }
            }
        }
    }

    @ViewBuilder
    private func previewContent(
        _ preview: RadialPalettePreview,
        contentColor: Color
    ) -> some View {
        switch preview {
        case let .strokeWidth(relative, color):
            let width = previewDimension(
                relative: relative,
                minimum: RadialPaletteMetrics.strokePreviewMinimumWidth,
                maximum: RadialPaletteMetrics.strokePreviewMaximumWidth
            )
            Capsule()
                .fill(Color(nsColor: color))
                .frame(width: RadialPaletteMetrics.strokePreviewLength, height: width)
                .radialPalettePreviewOutline(in: Capsule(), tone: model.state.faceTone)
        case let .textSize(relative, color):
            let size = previewDimension(
                relative: relative,
                minimum: RadialPaletteMetrics.textPreviewMinimumSize,
                maximum: RadialPaletteMetrics.textPreviewMaximumSize
            )
            Text("가")
                .font(.system(size: size, weight: .semibold))
                .foregroundStyle(Color(nsColor: color))
                .shadow(
                    color: Color(nsColor: RadialPaletteGlassStyle.previewOutlineInnerColor(
                        tone: model.state.faceTone
                    )),
                    radius: 0.5
                )
                .shadow(
                    color: Color(nsColor: RadialPaletteGlassStyle.regularBorderColor(
                        tone: model.state.faceTone
                    )),
                    radius: 1
                )
        case let .opacity(opacity, color):
            let sample = RoundedRectangle(
                cornerRadius: RadialPaletteMetrics.opacityPreviewCornerRadius
            )
            ZStack {
                sample.fill(Color(nsColor: .lightGray))
                sample.fill(Color(nsColor: color).opacity(Double(opacity)))
            }
            .frame(
                width: RadialPaletteMetrics.opacityPreviewWidth,
                height: RadialPaletteMetrics.opacityPreviewHeight
            )
            .radialPalettePreviewOutline(in: sample, tone: model.state.faceTone)
        case let .eraserDiameter(relative):
            let diameter = previewDimension(
                relative: relative,
                minimum: RadialPaletteMetrics.eraserPreviewMinimumDiameter,
                maximum: RadialPaletteMetrics.eraserPreviewMaximumDiameter
            )
            Circle()
                .stroke(contentColor, lineWidth: RadialPaletteMetrics.eraserPreviewStrokeWidth)
                .frame(width: diameter, height: diameter)
        }
    }

    private func previewDimension(
        relative: CGFloat,
        minimum: CGFloat,
        maximum: CGFloat
    ) -> CGFloat {
        minimum + relative * (maximum - minimum)
    }

    private func iconImage(for cell: RadialPaletteRenderCell) -> NSImage? {
        guard let iconKey = cell.iconKey, let iconName = cell.iconName else { return nil }
        if let configuration = cell.attributeConfiguration {
            return AppIcon.image(
                key: iconKey,
                fallbackSystemName: iconName,
                selected: cell.isSelected,
                attributeConfiguration: configuration,
                pointSize: 14,
                uiColor: contentNSColor(for: cell),
                backgroundColor: RadialPaletteGlassStyle.estimatedBackdropColor(
                    selected: cell.isSelected,
                    tone: model.state.faceTone
                ),
                visualState: AttrIconVisualState(
                    isSelected: cell.isSelected,
                    isEnabled: cell.isEnabled,
                    isHovered: cell.isHovered,
                    isPassThrough: false
                ),
                accessibilityDescription: cell.accessibilityLabel
            )
        }
        return AppIcon.image(
            key: iconKey,
            fallbackSystemName: iconName,
            selected: cell.isSelected,
            accessibilityDescription: cell.accessibilityLabel
        )
    }

    private func propertyLabel(
        _ label: RadialPaletteRenderPropertyLabel,
        in bounds: CGRect
    ) -> some View {
        let position = RadialSectorShape.point(
            center: CGPoint(x: bounds.midX, y: bounds.midY),
            radius: label.radius,
            angle: label.angle
        )
        return Text(label.name)
            .font(.system(size: RadialPaletteMetrics.propertyLabelFontSize, weight: .semibold))
            .foregroundStyle(Color(nsColor: RadialPaletteGlassStyle.primaryContentColor(
                tone: model.state.faceTone
            )))
            .padding(.horizontal, 4)
            .padding(.vertical, 2)
            .radialPaletteGlass(
                .regular,
                in: Capsule(),
                faceOpacity: model.glassOpacity,
                tone: model.state.faceTone
            )
            .fixedSize()
            .position(position)
    }

    private func centerView(in bounds: CGRect) -> some View {
        let state = model.state
        let circle = Circle()
        var glass = Glass.regular
        if state.centerIsPassThroughEnabled {
            glass = Glass.regular.tint(
                Color.accentColor.opacity(RadialPaletteGlassStyle.selectedTintOpacity)
            )
        }
        if state.centerIsHovered {
            glass = glass.interactive()
        }
        return ZStack {
            Color.clear
                .frame(
                    width: RadialPaletteMetrics.collapsedDiameter,
                    height: RadialPaletteMetrics.collapsedDiameter
                )
                .radialPaletteGlass(
                    glass,
                    in: circle,
                    faceOpacity: state.centerIsPassThroughEnabled ? nil : model.glassOpacity,
                    tone: state.faceTone
                )

            if state.centerIsFocused {
                circle.stroke(
                    Color(nsColor: RadialPaletteGlassStyle.focusOuterColor(tone: state.faceTone)),
                    lineWidth: RadialPaletteMetrics.focusOuterWidth
                )
                circle.stroke(
                    Color(nsColor: RadialPaletteGlassStyle.selectedContentColor),
                    lineWidth: RadialPaletteMetrics.focusInnerWidth
                )
            }

            circle
                .inset(by: state.centerIsFocused ? 4 : 0)
                .stroke(Color(nsColor: state.centerColor), lineWidth: state.centerStrokeWidth)

            if state.showsKeyCaps, !state.centerShortcut.isEmpty {
                keyCap(state.centerShortcut)
            } else if let icon = centerIconImage(for: state)
            {
                Image(nsImage: icon)
                    .renderingMode(
                        state.centerAttributeConfiguration == nil ? .template : .original
                    )
                    .resizable()
                    .scaledToFit()
                    .frame(width: 18, height: 18)
                    .foregroundStyle(
                        Color(nsColor: RadialPaletteGlassStyle.contentColor(
                            isSelected: state.centerIsPassThroughEnabled,
                            isEnabled: true,
                            tone: state.faceTone
                        ))
                    )
                    .accessibilityLabel(
                        Text(state.centerAccessibilityLabel ?? state.centerIconKey ?? "도구")
                    )
            }
        }
        .frame(
            width: RadialPaletteMetrics.collapsedDiameter,
            height: RadialPaletteMetrics.collapsedDiameter
        )
        .position(x: bounds.midX, y: bounds.midY)
    }

    private func keyCap(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 8.5, weight: .medium))
            .foregroundStyle(Color(nsColor: RadialPaletteGlassStyle.primaryContentColor(
                tone: model.state.faceTone
            )))
            .padding(.horizontal, 5)
            .padding(.vertical, 3)
            .frame(minWidth: 18, minHeight: 18)
            .radialPaletteGlass(
                .regular,
                in: RoundedRectangle(cornerRadius: 5),
                faceOpacity: model.glassOpacity,
                tone: model.state.faceTone
            )
            .fixedSize()
    }

    private func glass(for cell: RadialPaletteRenderCell) -> Glass {
        var glass = Glass.regular
        if cell.isSelected, cell.color == nil {
            glass = Glass.regular.tint(
                Color.accentColor.opacity(RadialPaletteGlassStyle.selectedTintOpacity)
            )
        }
        if cell.isHovered, cell.isEnabled {
            glass = glass.interactive()
        }
        return glass
    }

    private func contentColor(for cell: RadialPaletteRenderCell) -> Color {
        Color(nsColor: contentNSColor(for: cell))
    }

    private func contentNSColor(for cell: RadialPaletteRenderCell) -> NSColor {
        RadialPaletteGlassStyle.contentColor(
            isSelected: cell.isSelected && cell.color == nil,
            isEnabled: cell.isEnabled,
            tone: model.state.faceTone
        )
    }

    private func centerIconImage(for state: RadialPaletteRenderState) -> NSImage? {
        guard let iconKey = state.centerIconKey, let iconName = state.centerIconName else {
            return nil
        }
        if let configuration = state.centerAttributeConfiguration {
            return AppIcon.image(
                key: iconKey,
                fallbackSystemName: iconName,
                selected: true,
                attributeConfiguration: configuration,
                pointSize: 18,
                uiColor: RadialPaletteGlassStyle.contentColor(
                    isSelected: state.centerIsPassThroughEnabled,
                    isEnabled: true,
                    tone: state.faceTone
                ),
                backgroundColor: RadialPaletteGlassStyle.estimatedBackdropColor(
                    selected: state.centerIsPassThroughEnabled,
                    tone: state.faceTone
                ),
                visualState: AttrIconVisualState(
                    isSelected: state.centerIsPassThroughEnabled,
                    isEnabled: true,
                    isHovered: state.centerIsHovered,
                    isPassThrough: state.centerIsPassThroughEnabled
                ),
                accessibilityDescription: state.centerAccessibilityLabel
            )
        }
        return AppIcon.image(
            key: iconKey,
            fallbackSystemName: iconName,
            selected: true,
            accessibilityDescription: state.centerAccessibilityLabel
        )
    }

    private func baseBorderColor(for cell: RadialPaletteRenderCell) -> Color {
        if cell.isHovered, cell.isEnabled {
            return .accentColor
        }
        if cell.isSelected {
            return Color(nsColor: RadialPaletteGlassStyle.selectedContentColor)
        }
        return Color(nsColor: RadialPaletteGlassStyle.regularBorderColor(
            tone: model.state.faceTone
        ))
    }

    private func baseBorderWidth(for cell: RadialPaletteRenderCell) -> CGFloat {
        if cell.isHovered, cell.isEnabled {
            return RadialPaletteMetrics.hoverStrokeWidth
        }
        if cell.isSelected {
            return RadialPaletteMetrics.selectedStrokeWidth
        }
        return RadialPaletteMetrics.cellStrokeWidth
    }
}
