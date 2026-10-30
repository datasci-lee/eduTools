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
    static let expandedDiameter: CGFloat = 302
    // Focus-ring overhang plus room for the active-arc glow.
    static let windowMargin: CGFloat = 10
    static let centerRadius: CGFloat = collapsedDiameter / 2
    static let groupInnerRadius: CGFloat = 28
    static let groupOuterRadius: CGFloat = 58
    static let entryInnerRadius: CGFloat = 64
    static let entryOuterRadius: CGFloat = 100
    static let optionInnerRadius: CGFloat = 113
    static let optionOuterRadius: CGFloat = 149
    static let dragThreshold: CGFloat = 4
    static let commandHoldDuration: TimeInterval = 0.5
    static let sectorGap: CGFloat = 2 * .pi / 180
    static let propertyGap: CGFloat = 9 * .pi / 180
    static let tooltipGap: CGFloat = 8
    static let cellStrokeWidth: CGFloat = 0.8
    static let hoverStrokeWidth: CGFloat = 1.8
    static let selectedStrokeWidth: CGFloat = 1.2
    static let writingWidths: [CGFloat] = [1, 3, 5, 8, 12]
    static let highlighterWidths: [CGFloat] = [6, 12, 18, 24, 30]
    static let textSizes: [CGFloat] = [16, 24, 36, 48, 72]
    static let highlighterOpacities: [CGFloat] = [0.2, 0.4, 0.6, 0.8, 1]
    static let laserTrailDurations: [TimeInterval] = [0.3, 0.7, 1, 2]
    static let colorPreviewDiameter: CGFloat = 20
    static let previewSlotWidth: CGFloat = 18
    static let previewSlotHeight: CGFloat = 15
    static let previewLabelFontSize: CGFloat = 7.5
    static let previewSpacing: CGFloat = 0
    static let previewOutlineOuterWidth: CGFloat = 1
    static let previewOutlineInnerWidth: CGFloat = 0.35
    static let strokePreviewLength: CGFloat = 16
    static let strokePreviewMinimumWidth: CGFloat = 1
    static let strokePreviewMaximumWidth: CGFloat = 8
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

struct RadialPaletteRenderState {
    let isExpanded: Bool
    let showsKeyCaps: Bool
    let cells: [RadialPaletteRenderCell]
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
        cells: [],
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

private struct RadialArcShape: Shape {
    let radius: CGFloat
    let startAngle: CGFloat
    let endAngle: CGFloat

    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.addArc(
            center: CGPoint(x: rect.midX, y: rect.midY),
            radius: radius,
            startAngle: .radians(Double(startAngle - .pi / 2)),
            endAngle: .radians(Double(endAngle - .pi / 2)),
            clockwise: false
        )
        return path
    }
}

private struct RadialPaletteArcSegment: Identifiable {
    let id: String
    let radius: CGFloat
    let startAngle: CGFloat
    let endAngle: CGFloat
}

private extension View {
    func radialPalettePreviewOutline<Outline: Shape>(
        in outline: Outline
    ) -> some View {
        overlay {
            outline.stroke(
                Color(nsColor: RadialPaletteArcStyle.previewOutlineOuterColor),
                lineWidth: RadialPaletteMetrics.previewOutlineOuterWidth
            )
        }
        .overlay {
            outline.stroke(
                Color(nsColor: RadialPaletteArcStyle.previewOutlineInnerColor),
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
                    groupRing(in: bounds)
                    entryCapsuleTrack()
                    inactiveArcTracks()
                    ForEach(model.state.cells) { cell in
                        cellView(cell, in: bounds)
                    }
                }
                centerView(in: bounds)
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
        }
        .background(Color.clear)
        .allowsHitTesting(false)
    }

    private func groupRing(in bounds: CGRect) -> some View {
        let thickness = RadialPaletteMetrics.groupOuterRadius
            - RadialPaletteMetrics.groupInnerRadius
        let centerlineDiameter = RadialPaletteMetrics.groupOuterRadius
            + RadialPaletteMetrics.groupInnerRadius
        return ZStack {
            Circle()
                .stroke(surfaceColor, lineWidth: thickness)
                .frame(width: centerlineDiameter, height: centerlineDiameter)
            Circle()
                .strokeBorder(
                    Color(nsColor: RadialPaletteArcStyle.outerBorderColor),
                    lineWidth: RadialPaletteMetrics.cellStrokeWidth
                )
                .frame(
                    width: RadialPaletteMetrics.groupOuterRadius * 2,
                    height: RadialPaletteMetrics.groupOuterRadius * 2
                )
        }
        .frame(width: bounds.width, height: bounds.height)
    }

    @ViewBuilder
    private func cellView(_ cell: RadialPaletteRenderCell, in bounds: CGRect) -> some View {
        switch cell.ring {
        case .group:
            groupCellView(cell, in: bounds)
        case .entry, .option:
            nodeCellView(cell, in: bounds)
        }
    }

    private func groupCellView(
        _ cell: RadialPaletteRenderCell,
        in bounds: CGRect
    ) -> some View {
        let shape = RadialSectorShape(geometry: cell.geometry)
        let contentPosition = nodePosition(for: cell, in: bounds)
        return ZStack {
            if cell.isHovered, cell.isEnabled {
                shape.fill(Color(nsColor: RadialPaletteArcStyle.hoverFillColor))
            }

            if cell.isSelected {
                activeArc(
                    for: cell,
                    radius: cell.geometry.outerRadius - RadialPaletteArcStyle.arcWidth / 2
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

    private func nodeCellView(
        _ cell: RadialPaletteRenderCell,
        in bounds: CGRect
    ) -> some View {
        let diameter = nodeDiameter(for: cell)
        let circle = Circle()
        let position = nodePosition(for: cell, in: bounds)
        let hasNodeSurface = cell.ring == .option && cell.color == nil
        let showsSelectedEntryHighlight = cell.ring == .entry && cell.isSelected
        let showsHoverHighlight = cell.isHovered && cell.isEnabled
            && !showsSelectedEntryHighlight
        return ZStack {
            if cell.ring == .option {
                edgeArc(for: cell)
            }

            ZStack {
                if hasNodeSurface {
                    circle.fill(surfaceColor)
                }

                if showsSelectedEntryHighlight {
                    circle.fill(Color(nsColor: RadialPaletteArcStyle.selectedNodeFillColor))
                } else if showsHoverHighlight {
                    circle.fill(Color(nsColor: RadialPaletteArcStyle.hoverFillColor))
                }

                if hasNodeSurface {
                    circle.stroke(
                        Color(nsColor: RadialPaletteArcStyle.outerBorderColor),
                        lineWidth: RadialPaletteMetrics.cellStrokeWidth
                    )
                }

                if cell.isFocused, cell.color == nil {
                    circle.stroke(
                        Color(nsColor: cell.ring == .entry
                            ? RadialPaletteArcStyle.entryFocusRingColor
                            : RadialPaletteArcStyle.activeIconColor),
                        lineWidth: cell.ring == .entry
                            ? RadialPaletteArcStyle.entryFocusRingWidth
                            : RadialPaletteArcStyle.nodeFocusRingWidth
                    )
                }

                if model.state.showsKeyCaps, !cell.shortcut.isEmpty {
                    keyCap(cell.shortcut)
                } else {
                    cellContent(cell)
                }
            }
            .frame(width: diameter, height: diameter)
            .scaleEffect(cell.ring == .option && cell.isSelected ? 1.08 : 1)
            .position(position)
        }
        .frame(width: bounds.width, height: bounds.height)
    }

    @ViewBuilder
    private func edgeArc(for cell: RadialPaletteRenderCell) -> some View {
        let isActive = cell.isSelected
            || cell.isFocused
            || (cell.isHovered && cell.isEnabled)
        if isActive {
            RadialArcShape(
                radius: arcTrackRadius(for: cell),
                startAngle: cell.geometry.startAngle,
                endAngle: cell.geometry.endAngle
            )
            .stroke(
                Color(nsColor: RadialPaletteArcStyle.activeColor),
                style: StrokeStyle(
                    lineWidth: RadialPaletteArcStyle.arcWidth,
                    lineCap: .round
                )
            )
            .shadow(
                color: Color(nsColor: RadialPaletteArcStyle.activeGlowColor),
                radius: RadialPaletteArcStyle.activeGlowRadius
            )
            .opacity(cell.isEnabled ? 1 : RadialPaletteGlassStyle.disabledContentOpacity)
        }
    }

    private func inactiveArcTracks() -> some View {
        ZStack {
            ForEach(optionTrackSegments()) { segment in
                RadialArcShape(
                    radius: segment.radius,
                    startAngle: segment.startAngle,
                    endAngle: segment.endAngle
                )
                .stroke(
                    Color(nsColor: RadialPaletteArcStyle.inactiveArcColor),
                    style: StrokeStyle(
                        lineWidth: RadialPaletteArcStyle.arcWidth,
                        lineCap: .round
                    )
                )
                .shadow(color: Color.black.opacity(0.28), radius: 1.5)
            }
        }
    }

    @ViewBuilder
    private func entryCapsuleTrack() -> some View {
        let cells = cells(in: .entry)
        if let first = cells.first, let last = cells.last {
            let nodeRadius = nodeOrbitRadius(for: first)
            let requestedWidth = RadialPaletteArcStyle.entryNodeMaximumDiameter
                + RadialPaletteArcStyle.entryCapsulePadding
            // Keep the capsule inside the entry ring so it cannot touch the option track.
            let maximumWidth = 2 * (first.geometry.outerRadius - nodeRadius)
            let lineWidth = min(requestedWidth, maximumWidth)
            let fillWidth = max(0, lineWidth - 2 * RadialPaletteMetrics.cellStrokeWidth)
            let shape = RadialArcShape(
                radius: nodeRadius,
                startAngle: first.geometry.startAngle,
                endAngle: last.geometry.endAngle
            )

            shape.stroke(
                Color(nsColor: RadialPaletteArcStyle.outerBorderColor),
                style: StrokeStyle(lineWidth: lineWidth, lineCap: .round)
            )
            shape.stroke(
                Color(nsColor: RadialPaletteArcStyle.surfaceColor(
                    opacity: model.glassOpacity * RadialPaletteArcStyle.entryCapsuleOpacityMultiplier
                )),
                style: StrokeStyle(lineWidth: fillWidth, lineCap: .round)
            )
        }
    }

    private func optionTrackSegments() -> [RadialPaletteArcSegment] {
        let ringCells = cells(in: .option)
        guard let first = ringCells.first else { return [] }

        let sectionThreshold = RadialPaletteMetrics.sectorGap
            + RadialPaletteMetrics.propertyGap / 2
        var segments: [RadialPaletteArcSegment] = []
        var sectionStart = first.geometry.startAngle
        var sectionEnd = first.geometry.endAngle
        var sectionIndex = 0

        for cell in ringCells.dropFirst() {
            if cell.geometry.startAngle - sectionEnd > sectionThreshold {
                segments.append(RadialPaletteArcSegment(
                    id: "option.\(sectionIndex)",
                    radius: arcTrackRadius(for: first),
                    startAngle: sectionStart,
                    endAngle: sectionEnd
                ))
                sectionIndex += 1
                sectionStart = cell.geometry.startAngle
            }
            sectionEnd = cell.geometry.endAngle
        }
        segments.append(RadialPaletteArcSegment(
            id: "option.\(sectionIndex)",
            radius: arcTrackRadius(for: first),
            startAngle: sectionStart,
            endAngle: sectionEnd
        ))
        return segments
    }

    private func cells(in ring: RadialPaletteRing) -> [RadialPaletteRenderCell] {
        model.state.cells
            .filter { $0.ring == ring }
            .sorted { $0.geometry.startAngle < $1.geometry.startAngle }
    }

    private func activeArc(
        for cell: RadialPaletteRenderCell,
        radius: CGFloat
    ) -> some View {
        RadialArcShape(
            radius: radius,
            startAngle: cell.geometry.startAngle,
            endAngle: cell.geometry.endAngle
        )
        .stroke(
            Color(nsColor: RadialPaletteArcStyle.activeColor),
            style: StrokeStyle(
                lineWidth: RadialPaletteArcStyle.arcWidth,
                lineCap: .round
            )
        )
        .shadow(
            color: Color(nsColor: RadialPaletteArcStyle.activeGlowColor),
            radius: RadialPaletteArcStyle.activeGlowRadius
        )
        .opacity(cell.isEnabled ? 1 : RadialPaletteGlassStyle.disabledContentOpacity)
    }

    private func nodeDiameter(for cell: RadialPaletteRenderCell) -> CGFloat {
        let radius = nodeOrbitRadius(for: cell)
        let arcLength = (cell.geometry.endAngle - cell.geometry.startAngle) * radius
        let maximum = cell.ring == .entry
            ? RadialPaletteArcStyle.entryNodeMaximumDiameter
            : RadialPaletteArcStyle.optionNodeMaximumDiameter
        return max(0, min(maximum, arcLength - RadialPaletteArcStyle.nodeArcPadding))
    }

    private func nodePosition(
        for cell: RadialPaletteRenderCell,
        in bounds: CGRect
    ) -> CGPoint {
        RadialSectorShape.point(
            center: CGPoint(x: bounds.midX, y: bounds.midY),
            radius: cell.ring == .group
                ? (cell.geometry.innerRadius + cell.geometry.outerRadius) / 2
                : nodeOrbitRadius(for: cell),
            angle: (cell.geometry.startAngle + cell.geometry.endAngle) / 2
        )
    }

    private func nodeOrbitRadius(for cell: RadialPaletteRenderCell) -> CGFloat {
        cell.geometry.innerRadius + RadialPaletteArcStyle.nodeOrbitOffset
    }

    private func arcTrackRadius(for cell: RadialPaletteRenderCell) -> CGFloat {
        nodeOrbitRadius(for: cell) - RadialPaletteArcStyle.arcTrackOffset
    }

    private var surfaceColor: Color {
        Color(nsColor: RadialPaletteArcStyle.surfaceColor(opacity: model.glassOpacity))
    }

    @ViewBuilder
    private func cellContent(_ cell: RadialPaletteRenderCell) -> some View {
        let contentColor = contentColor(for: cell)
        let icon = iconImage(for: cell)
        switch cell.ring {
        case .group:
            if let icon {
                Image(nsImage: icon)
                    .renderingMode(
                        cell.attributeConfiguration == nil ? .template : .original
                    )
                    .resizable()
                    .scaledToFit()
                    .frame(width: 13, height: 13)
                    .foregroundStyle(contentColor)
                    .accessibilityLabel(Text(cell.accessibilityLabel))
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
                    .radialPalettePreviewOutline(in: Circle())
                    .overlay {
                        if cell.isSelected || cell.isFocused {
                            Circle().stroke(
                                Color(nsColor: RadialPaletteArcStyle.activeIconColor),
                                lineWidth: RadialPaletteArcStyle.nodeFocusRingWidth
                            )
                        }
                    }
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
                .radialPalettePreviewOutline(in: Capsule())
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
                    color: Color(nsColor: RadialPaletteArcStyle.previewOutlineInnerColor),
                    radius: 0.5
                )
                .shadow(
                    color: Color(nsColor: RadialPaletteArcStyle.previewOutlineOuterColor),
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
            .radialPalettePreviewOutline(in: sample)
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
                backgroundColor: RadialPaletteArcStyle.estimatedBackdropColor(
                    selected: cell.isSelected
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

    private func centerView(in bounds: CGRect) -> some View {
        let state = model.state
        let circle = Circle()
        return ZStack {
            circle.fill(surfaceColor)

            if state.centerIsPassThroughEnabled {
                circle.fill(
                    Color(nsColor: RadialPaletteArcStyle.activeColor)
                        .opacity(RadialPaletteGlassStyle.selectedTintOpacity)
                )
            }

            if state.centerIsHovered {
                circle.fill(Color(nsColor: RadialPaletteArcStyle.hoverFillColor))
            }

            circle.stroke(
                Color(nsColor: RadialPaletteArcStyle.outerBorderColor),
                lineWidth: RadialPaletteMetrics.cellStrokeWidth
            )

            if state.centerIsFocused {
                circle.stroke(
                    Color(nsColor: RadialPaletteArcStyle.activeIconColor),
                    lineWidth: RadialPaletteArcStyle.nodeFocusRingWidth
                )
            }

            circle
                .inset(by: state.centerIsFocused ? 2 : 0)
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
                        Color(nsColor: RadialPaletteArcStyle.iconColor(
                            isActive: state.centerIsPassThroughEnabled,
                            isEnabled: true
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
            .foregroundStyle(Color(nsColor: RadialPaletteArcStyle.activeIconColor))
            .padding(.horizontal, 5)
            .padding(.vertical, 3)
            .frame(minWidth: 18, minHeight: 18)
            .background {
                RoundedRectangle(cornerRadius: 5)
                    .fill(surfaceColor)
                    .overlay {
                        RoundedRectangle(cornerRadius: 5)
                            .stroke(
                                Color(nsColor: RadialPaletteArcStyle.outerBorderColor),
                                lineWidth: RadialPaletteMetrics.cellStrokeWidth
                            )
                    }
            }
            .fixedSize()
    }

    private func contentColor(for cell: RadialPaletteRenderCell) -> Color {
        Color(nsColor: contentNSColor(for: cell))
    }

    private func contentNSColor(for cell: RadialPaletteRenderCell) -> NSColor {
        RadialPaletteArcStyle.iconColor(
            isActive: cell.isSelected && cell.color == nil,
            isEnabled: cell.isEnabled
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
                uiColor: RadialPaletteArcStyle.iconColor(
                    isActive: state.centerIsPassThroughEnabled,
                    isEnabled: true
                ),
                backgroundColor: RadialPaletteArcStyle.estimatedBackdropColor(
                    selected: state.centerIsPassThroughEnabled
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

}
