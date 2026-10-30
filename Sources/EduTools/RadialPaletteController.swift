import AppKit
import Carbon

private final class RadialPalettePanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

private final class RadialPaletteTooltipPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

enum RadialPaletteGroupID: Int, CaseIterable {
    case writing
    case shapes
    case erasing
    case boardAndSettings
    case presenting
    case favorites
}

enum RadialPaletteEntryAction {
    case command(AppCommand)
    case selectEraserMode(EraserMode)
    case openSettings
    case saveFavorite
    case applyFavorite(index: Int, favorite: FavoriteTool)
}

struct RadialPaletteEntry {
    let action: RadialPaletteEntryAction
    let fallbackName: String?
    let fallbackIconKey: String?
    let fallbackIconName: String?
    let isEnabled: Bool

    init(
        action: RadialPaletteEntryAction,
        fallbackName: String?,
        fallbackIconKey: String? = nil,
        fallbackIconName: String?,
        isEnabled: Bool = true
    ) {
        self.action = action
        self.fallbackName = fallbackName
        self.fallbackIconKey = fallbackIconKey
        self.fallbackIconName = fallbackIconName
        self.isEnabled = isEnabled
    }

    static func command(_ command: AppCommand) -> RadialPaletteEntry {
        RadialPaletteEntry(action: .command(command), fallbackName: nil, fallbackIconName: nil)
    }

    static func eraserMode(_ mode: EraserMode) -> RadialPaletteEntry {
        let iconKey: String
        let iconName: String
        switch mode {
        case .partial:
            iconKey = "tool.eraser"
            iconName = "eraser"
        case .stroke:
            iconKey = "eraser.mode.stroke"
            iconName = "eraser.line.dashed"
        case .area:
            iconKey = AppIcon.Key.optionAreaRectangle
            iconName = "rectangle.dashed"
        }
        return RadialPaletteEntry(
            action: .selectEraserMode(mode),
            fallbackName: mode.displayName,
            fallbackIconKey: iconKey,
            fallbackIconName: iconName
        )
    }

    static let openSettings = RadialPaletteEntry(
        action: .openSettings,
        fallbackName: "설정 열기",
        fallbackIconName: "gearshape"
    )

    static func saveFavorite(name: String, isEnabled: Bool) -> RadialPaletteEntry {
        RadialPaletteEntry(
            action: .saveFavorite,
            fallbackName: name,
            fallbackIconName: "plus",
            isEnabled: isEnabled
        )
    }

    static func favorite(
        _ favorite: FavoriteTool,
        index: Int,
        name: String
    ) -> RadialPaletteEntry {
        guard let tool = favorite.inkTool,
              let sourceEntry = RadialPaletteCatalog.path(for: tool)?.entry
        else {
            preconditionFailure("A valid favorite tool is missing from the radial palette catalog")
        }
        return RadialPaletteEntry(
            action: .applyFavorite(index: index, favorite: favorite),
            fallbackName: name,
            fallbackIconName: sourceEntry.iconName
        )
    }

    var definition: ShortcutDefinition? {
        guard case let .command(command) = action else { return nil }
        return ShortcutCatalog.definition(for: command)
    }

    var identifier: String {
        if let definition {
            return definition.id
        }
        switch action {
        case let .selectEraserMode(mode): return "eraser.mode.\(mode.rawValue)"
        case .openSettings: return "palette.settings"
        case .saveFavorite: return "palette.favorite.save"
        case let .applyFavorite(index, _): return "palette.favorite.\(index)"
        case .command:
            preconditionFailure("A palette command is missing from ShortcutCatalog")
        }
    }

    var name: String {
        definition?.name ?? fallbackName ?? identifier
    }

    var iconKey: String {
        definition?.id ?? fallbackIconKey ?? identifier
    }

    var iconName: String {
        definition?.iconName ?? fallbackIconName ?? "circle"
    }

    var shortcut: String {
        if case .selectEraserMode = action {
            return ShortcutCatalog.definition(for: .selectOrCycleEraser)?.displayKeys ?? ""
        }
        return definition?.displayKeys ?? ""
    }

    var inkTool: InkTool? {
        switch action {
        case let .command(.selectTool(tool)): return tool
        case .command(.useLaser): return .laser
        case .selectEraserMode: return .eraser
        default: return nil
        }
    }

    var eraserMode: EraserMode? {
        guard case let .selectEraserMode(mode) = action else { return nil }
        return mode
    }

    var favorite: FavoriteTool? {
        guard case let .applyFavorite(_, favorite) = action else { return nil }
        return favorite
    }
}

private struct RadialPaletteGroupDefinition {
    let id: RadialPaletteGroupID
    let iconKey: String
    let name: String
    let iconName: String
    let isEnabled: Bool
    let entries: [RadialPaletteEntry]
}

private enum RadialPaletteCatalog {
    static let groups: [RadialPaletteGroupDefinition] = [
        RadialPaletteGroupDefinition(
            id: .writing,
            iconKey: "group.writing",
            name: "작성",
            iconName: "pencil.and.outline",
            isEnabled: true,
            entries: [
                .command(.selectTool(.pen)),
                .command(.selectTool(.highlighter)),
                .command(.selectTool(.text)),
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .shapes,
            iconKey: "group.shapes",
            name: "도형",
            iconName: "square.on.circle",
            isEnabled: true,
            entries: [
                .command(.selectTool(.arrow)),
                .command(.selectTool(.rectangle)),
                .command(.selectTool(.ellipse)),
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .erasing,
            iconKey: "group.erasing",
            name: "지우기",
            iconName: "eraser",
            isEnabled: true,
            entries: [
                .eraserMode(.partial),
                .eraserMode(.stroke),
                .eraserMode(.area),
                .command(.undo),
                .command(.redo),
                .command(.clearAll),
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .boardAndSettings,
            iconKey: "group.board",
            name: "보드·설정",
            iconName: "rectangle.on.rectangle",
            isEnabled: true,
            entries: [
                .command(.toggleBoard(.white)),
                .command(.toggleBoard(.black)),
                .command(.previousBoardPage),
                .command(.nextBoardPage),
                .command(.saveCurrentContent),
                .command(.copyCurrentImage),
                .openSettings,
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .presenting,
            iconKey: "group.presenting",
            name: "발표",
            iconName: "person.fill.viewfinder",
            isEnabled: true,
            entries: [
                .command(.useLaser),
                .command(.toggleSpotlight),
                .command(.selectTool(.cover)),
                .command(.togglePassThroughMode),
                .command(.toggleZoom),
                .command(.toggleCursorHighlight),
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .favorites,
            iconKey: "group.favorites",
            name: "즐겨찾기",
            iconName: "star.fill",
            isEnabled: true,
            entries: []
        ),
    ]

    static var iconKeys: Set<String> {
        Set(groups.flatMap { group in
            [group.iconKey] + group.entries.map(\.iconKey)
        })
    }

    static func definition(for id: RadialPaletteGroupID) -> RadialPaletteGroupDefinition {
        guard let definition = groups.first(where: { $0.id == id }) else {
            preconditionFailure("Missing radial palette group \(id)")
        }
        return definition
    }

    static func path(for tool: InkTool, eraserMode: EraserMode? = nil) -> (
        group: RadialPaletteGroupDefinition,
        entry: RadialPaletteEntry
    )? {
        for group in groups {
            if tool == .eraser,
               let eraserMode,
               let entry = group.entries.first(where: { $0.eraserMode == eraserMode })
            {
                return (group, entry)
            }
            if let entry = group.entries.first(where: { $0.inkTool == tool }) {
                return (group, entry)
            }
        }
        return nil
    }
}

enum RadialPaletteRing: Equatable {
    case group
    case entry
    case option
}

struct RadialPaletteOption {
    let identifier: String
    let name: String
    let shortLabel: String
    let iconKey: String?
    let iconName: String?
    let color: NSColor?
    let preview: RadialPalettePreview?
    let shortcut: String
    let isSelected: Bool
    let isEnabled: Bool
    let action: () -> Void

    init(
        identifier: String,
        name: String,
        shortLabel: String,
        iconKey: String?,
        iconName: String?,
        color: NSColor?,
        preview: RadialPalettePreview? = nil,
        shortcut: String,
        isSelected: Bool,
        isEnabled: Bool = true,
        action: @escaping () -> Void
    ) {
        self.identifier = identifier
        self.name = name
        self.shortLabel = shortLabel
        self.iconKey = iconKey
        self.iconName = iconName
        self.color = color
        self.preview = preview
        self.shortcut = shortcut
        self.isSelected = isSelected
        self.isEnabled = isEnabled
        self.action = action
    }
}

private struct RadialPaletteOptionGroup {
    let name: String
    let options: [RadialPaletteOption]
}

struct RadialPaletteCell {
    enum Action {
        case group(RadialPaletteGroupID)
        case entry(RadialPaletteEntry)
        case option(RadialPaletteOption)
    }

    let identifier: String
    let ring: RadialPaletteRing
    let geometry: RadialPaletteSectorGeometry
    let name: String
    let iconKey: String?
    let iconName: String?
    let shortLabel: String
    let shortcut: String
    let color: NSColor?
    let preview: RadialPalettePreview?
    let isSelected: Bool
    let isEnabled: Bool
    let action: Action

    init(
        identifier: String,
        ring: RadialPaletteRing,
        geometry: RadialPaletteSectorGeometry,
        name: String,
        iconKey: String?,
        iconName: String?,
        shortLabel: String,
        shortcut: String,
        color: NSColor?,
        preview: RadialPalettePreview? = nil,
        isSelected: Bool,
        isEnabled: Bool,
        action: Action
    ) {
        self.identifier = identifier
        self.ring = ring
        self.geometry = geometry
        self.name = name
        self.iconKey = iconKey
        self.iconName = iconName
        self.shortLabel = shortLabel
        self.shortcut = shortcut
        self.color = color
        self.preview = preview
        self.isSelected = isSelected
        self.isEnabled = isEnabled
        self.action = action
    }

    var innerRadius: CGFloat { geometry.innerRadius }
    var outerRadius: CGFloat { geometry.outerRadius }
    var startAngle: CGFloat { geometry.startAngle }
    var endAngle: CGFloat { geometry.endAngle }
}

enum RadialPaletteHitTarget {
    case center
    case cell(RadialPaletteCell)

    var identifier: String {
        switch self {
        case .center: return "palette.center"
        case let .cell(cell): return cell.identifier
        }
    }
}

enum RadialPaletteNavigationKey: Equatable {
    case shallower
    case deeper
    case previous
    case next
}

enum RadialPaletteFocus: Equatable {
    case center
    case cell(String)

    var identifier: String {
        switch self {
        case .center: return "palette.center"
        case let .cell(identifier): return identifier
        }
    }
}

/// Returns a focus derived only from the currently rendered cells. Filtering preserves the
/// `layoutCells()` order, so keyboard navigation cannot drift from the visual/catalog order.
func nextRadialPaletteFocus(
    in cells: [RadialPaletteCell],
    from current: RadialPaletteFocus,
    for key: RadialPaletteNavigationKey,
    selectedEntryIdentifier: String?
) -> RadialPaletteFocus {
    let enabledCells = cells.filter(\.isEnabled)

    func selectedCell(in ring: RadialPaletteRing) -> RadialPaletteCell? {
        enabledCells.first { $0.ring == ring && $0.isSelected }
    }

    func selectedEntryPriority(for cell: RadialPaletteCell) -> Int? {
        guard cell.ring == .entry,
              case let .entry(entry) = cell.action
        else {
            return nil
        }
        if entry.identifier == selectedEntryIdentifier {
            return 0
        }
        if cell.isSelected && entry.inkTool != nil {
            return 1
        }
        return nil
    }

    func selectedEntryCell() -> RadialPaletteCell? {
        for priority in 0...1 {
            if let cell = enabledCells.first(where: { cell in
                selectedEntryPriority(for: cell) == priority
            }) {
                return cell
            }
        }
        return nil
    }

    func firstCell(in ring: RadialPaletteRing) -> RadialPaletteCell? {
        enabledCells.first { $0.ring == ring }
    }

    func focus(for cell: RadialPaletteCell?) -> RadialPaletteFocus? {
        cell.map { .cell($0.identifier) }
    }

    switch key {
    case .shallower:
        switch current {
        case .center:
            return .center
        case let .cell(identifier):
            guard let cell = enabledCells.first(where: { $0.identifier == identifier }) else {
                return current
            }
            switch cell.ring {
            case .group:
                return .center
            case .entry:
                return focus(for: selectedCell(in: .group)) ?? current
            case .option:
                return focus(for: selectedEntryCell()) ?? current
            }
        }

    case .deeper:
        switch current {
        case .center:
            return focus(for: selectedCell(in: .group)) ?? current
        case let .cell(identifier):
            guard let cell = enabledCells.first(where: { $0.identifier == identifier }) else {
                return current
            }
            switch cell.ring {
            case .group:
                return focus(for: selectedEntryCell() ?? firstCell(in: .entry)) ?? current
            case .entry:
                guard selectedEntryPriority(for: cell) != nil else {
                    return current
                }
                return focus(for: firstCell(in: .option)) ?? current
            case .option:
                return current
            }
        }

    case .previous, .next:
        guard case let .cell(identifier) = current,
              let currentCell = enabledCells.first(where: { $0.identifier == identifier })
        else {
            return current
        }
        let ringCells = enabledCells.filter { $0.ring == currentCell.ring }
        guard ringCells.count > 1,
              let index = ringCells.firstIndex(where: { $0.identifier == identifier })
        else {
            return current
        }
        let offset = key == .next ? 1 : -1
        let targetIndex: Int
        if currentCell.ring == .entry {
            targetIndex = min(max(index + offset, 0), ringCells.count - 1)
        } else {
            targetIndex = (index + offset + ringCells.count) % ringCells.count
        }
        return .cell(ringCells[targetIndex].identifier)
    }
}

private final class RadialPaletteTooltip {
    private static let cornerRadius: CGFloat = 7

    private let panel: RadialPaletteTooltipPanel
    private let label = NSTextField(labelWithString: "")
    private let surfaceView = NSView(frame: .zero)

    init(glassOpacity: Double) {
        panel = RadialPaletteTooltipPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = true
        panel.isReleasedWhenClosed = false
        panel.level = NSWindow.Level(rawValue: PaletteController.windowLevel.rawValue + 1)
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = .none

        surfaceView.wantsLayer = true
        surfaceView.layer?.backgroundColor = RadialPaletteArcStyle.surfaceColor(
            opacity: glassOpacity
        ).cgColor
        surfaceView.layer?.borderColor = RadialPaletteArcStyle.outerBorderColor.cgColor
        surfaceView.layer?.borderWidth = RadialPaletteMetrics.cellStrokeWidth
        surfaceView.layer?.cornerRadius = Self.cornerRadius
        surfaceView.layer?.masksToBounds = true
        label.font = .systemFont(ofSize: 12, weight: .medium)
        label.textColor = RadialPaletteArcStyle.activeIconColor
        label.maximumNumberOfLines = 2
        label.usesSingleLineMode = false
        label.translatesAutoresizingMaskIntoConstraints = false
        surfaceView.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: surfaceView.leadingAnchor, constant: 9),
            label.trailingAnchor.constraint(equalTo: surfaceView.trailingAnchor, constant: -9),
            label.topAnchor.constraint(equalTo: surfaceView.topAnchor, constant: 6),
            label.bottomAnchor.constraint(equalTo: surfaceView.bottomAnchor, constant: -6),
        ])
        panel.contentView = surfaceView
    }

    func updateAppearance(glassOpacity: Double) {
        surfaceView.layer?.backgroundColor = RadialPaletteArcStyle.surfaceColor(
            opacity: glassOpacity
        ).cgColor
        label.textColor = RadialPaletteArcStyle.activeIconColor
    }

    func show(
        text: String,
        paletteCenter: CGPoint,
        preferredDirection: CGVector,
        paletteRadius: CGFloat
    ) {
        label.stringValue = text
        label.sizeToFit()
        let size = CGSize(width: ceil(label.frame.width) + 18, height: ceil(label.frame.height) + 12)
        let screen = NSScreen.screens.first(where: { $0.frame.contains(paletteCenter) })
            ?? NSScreen.screens.first
        guard let screen else {
            NSLog("EduTools: radial palette could not show a hint because no screen is available")
            return
        }
        let visible = screen.visibleFrame
        let magnitude = hypot(preferredDirection.dx, preferredDirection.dy)
        let direction = magnitude > 0
            ? CGVector(
                dx: preferredDirection.dx / magnitude,
                dy: preferredDirection.dy / magnitude
            )
            : CGVector(dx: 1, dy: 0)
        let preferredFrame = tooltipFrame(
            size: size,
            paletteCenter: paletteCenter,
            direction: direction,
            paletteRadius: paletteRadius
        )
        let oppositeFrame = tooltipFrame(
            size: size,
            paletteCenter: paletteCenter,
            direction: CGVector(dx: -direction.dx, dy: -direction.dy),
            paletteRadius: paletteRadius
        )
        let chosenFrame = visible.contains(preferredFrame) ? preferredFrame : oppositeFrame
        panel.setFrame(clamped(chosenFrame, to: visible), display: true)
        panel.orderFrontRegardless()
    }

    private func tooltipFrame(
        size: CGSize,
        paletteCenter: CGPoint,
        direction: CGVector,
        paletteRadius: CGFloat
    ) -> CGRect {
        let projectedHalfExtent = abs(direction.dx) * size.width / 2
            + abs(direction.dy) * size.height / 2
        let distance = paletteRadius + RadialPaletteMetrics.tooltipGap + projectedHalfExtent
        let center = CGPoint(
            x: paletteCenter.x + direction.dx * distance,
            y: paletteCenter.y + direction.dy * distance
        )
        return CGRect(
            x: center.x - size.width / 2,
            y: center.y - size.height / 2,
            width: size.width,
            height: size.height
        )
    }

    private func clamped(_ frame: CGRect, to visibleFrame: CGRect) -> CGRect {
        let x = visibleFrame.width >= frame.width
            ? min(max(frame.minX, visibleFrame.minX), visibleFrame.maxX - frame.width)
            : visibleFrame.minX
        let y = visibleFrame.height >= frame.height
            ? min(max(frame.minY, visibleFrame.minY), visibleFrame.maxY - frame.height)
            : visibleFrame.minY
        return CGRect(origin: CGPoint(x: x, y: y), size: frame.size)
    }

    func hide() {
        panel.orderOut(nil)
    }

    deinit {
        panel.close()
    }
}

private final class RadialPaletteView: NSView {
    weak var controller: RadialPaletteController?
    private let renderModel = RadialPaletteRenderModel()
    private lazy var renderingView = RadialPaletteRenderingView(model: renderModel)
    private var trackingAreaReference: NSTrackingArea?
    private var pressedTargetIdentifier: String?
    private var centerMouseDownLocation = CGPoint.zero
    private var centerAtMouseDown = CGPoint.zero
    private var isDraggingCenter = false
    private(set) var hoveredTargetIdentifier: String?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        renderingView.frame = bounds
        renderingView.autoresizingMask = [.width, .height]
        addSubview(renderingView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isOpaque: Bool { false }
    override var mouseDownCanMoveWindow: Bool { false }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func updateTrackingAreas() {
        if let trackingAreaReference {
            removeTrackingArea(trackingAreaReference)
        }
        let tracking = NSTrackingArea(
            rect: bounds,
            options: [.activeAlways, .inVisibleRect, .mouseEnteredAndExited, .mouseMoved],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(tracking)
        trackingAreaReference = tracking
        super.updateTrackingAreas()
    }

    func redraw() {
        controller?.updateRenderModel(renderModel, hoveredIdentifier: hoveredTargetIdentifier)
    }

    func clearHoverForKeyboardNavigation() {
        hoveredTargetIdentifier = nil
    }

    override func mouseDown(with event: NSEvent) {
        guard let controller else { return }
        pressedTargetIdentifier = nil
        isDraggingCenter = false
        let location = convert(event.locationInWindow, from: nil)
        guard let target = controller.hitTarget(at: location, in: bounds) else {
            _ = controller.collapseIfExpanded()
            return
        }
        pressedTargetIdentifier = target.identifier
        if case .center = target {
            centerMouseDownLocation = NSEvent.mouseLocation
            centerAtMouseDown = controller.displayedCenter
            isDraggingCenter = false
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        guard let controller else { return }
        let location = convert(event.locationInWindow, from: nil)
        guard let target = controller.hitTarget(at: location, in: bounds) else { return }
        if case .center = target {
            controller.stepBack()
            return
        }
        _ = controller.removeFavorite(for: target)
    }

    override func mouseDragged(with event: NSEvent) {
        guard pressedTargetIdentifier == "palette.center", controller != nil else { return }
        let current = NSEvent.mouseLocation
        let delta = CGPoint(
            x: current.x - centerMouseDownLocation.x,
            y: current.y - centerMouseDownLocation.y
        )
        if !isDraggingCenter,
           hypot(delta.x, delta.y) >= RadialPaletteMetrics.dragThreshold
        {
            isDraggingCenter = true
        }
        guard isDraggingCenter else { return }
        updateDraggedCenter(using: current)
    }

    override func mouseUp(with event: NSEvent) {
        let pressedIdentifier = pressedTargetIdentifier
        let wasDraggingCenter = isDraggingCenter
        if wasDraggingCenter {
            updateDraggedCenter(using: NSEvent.mouseLocation)
        }
        pressedTargetIdentifier = nil
        isDraggingCenter = false
        guard let controller else { return }
        if wasDraggingCenter {
            controller.finishMovingPalette()
            return
        }
        let location = convert(event.locationInWindow, from: nil)
        guard let target = controller.hitTarget(at: location, in: bounds),
              target.identifier == pressedIdentifier
        else {
            return
        }
        if event.modifierFlags.contains(.option), controller.removeFavorite(for: target) {
            return
        }
        controller.activate(target)
    }

    private func updateDraggedCenter(using screenLocation: CGPoint) {
        guard let controller else { return }
        let delta = CGPoint(
            x: screenLocation.x - centerMouseDownLocation.x,
            y: screenLocation.y - centerMouseDownLocation.y
        )
        controller.movePaletteCenter(
            to: CGPoint(x: centerAtMouseDown.x + delta.x, y: centerAtMouseDown.y + delta.y)
        )
    }

    override func mouseMoved(with event: NSEvent) {
        guard let controller else { return }
        let location = convert(event.locationInWindow, from: nil)
        let target = controller.hitTarget(at: location, in: bounds)
        hoveredTargetIdentifier = target?.identifier
        redraw()
        controller.hover(target, at: NSEvent.mouseLocation)
    }

    override func mouseExited(with event: NSEvent) {
        hoveredTargetIdentifier = nil
        redraw()
        controller?.hover(nil, at: NSEvent.mouseLocation)
    }
}

final class RadialPaletteController: NSObject {
    static var iconKeys: Set<String> { RadialPaletteCatalog.iconKeys }

    private enum DefaultsKey {
        static let positions = "EduTools.radialPalette.positions"
        static let displayUUID = "EduTools.radialPalette.displayUUID"
    }

    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let settingsStore: SettingsStore
    private let defaults: UserDefaults
    private let positionStore: ScreenPositionStore
    private let openSettings: () -> Void
    private let panel: RadialPalettePanel
    private let paletteView = RadialPaletteView(frame: .zero)
    private let tooltip: RadialPaletteTooltip
    private var selectedGroup = RadialPaletteGroupID.writing
    private var selectedEntryIdentifier: String?
    private var isExpanded = false
    private var showsKeyCaps = false
    private var keyboardFocus: RadialPaletteFocus?
    private var showsKeyboardFocus = false
    private var isApplyingFavorite = false
    private var consumedPaletteKeyCodes: Set<UInt16> = []
    private var restingCenter = CGPoint.zero
    private var currentDisplayUUID: String?
    private var annotationObserverIdentifier: UUID?
    private var passThroughObserverIdentifier: UUID?
    private var toolObserverIdentifier: UUID?
    private var toolSettingsObserverIdentifier: UUID?
    private var visibilityObserverIdentifier: UUID?
    private var boardObserverIdentifier: UUID?
    private var presenterObserverIdentifier: UUID?
    private var settingsObserverIdentifier: UUID?
    private var eventMonitors: [Any] = []
    private var notificationObservers: [NSObjectProtocol] = []
    private var commandHoldWorkItem: DispatchWorkItem?

    var displayedCenter: CGPoint {
        CGPoint(x: panel.frame.midX, y: panel.frame.midY)
    }

    var window: NSWindow { panel }

    var canToggleExpansionFromKeyboard: Bool {
        settingsStore.settings.tools.paletteStyle == .radial
    }

    init(
        state: AppState,
        dispatcher: AppCommandDispatcher,
        settingsStore: SettingsStore,
        defaults: UserDefaults = .standard,
        openSettings: @escaping () -> Void
    ) {
        self.state = state
        self.dispatcher = dispatcher
        self.settingsStore = settingsStore
        self.defaults = defaults
        positionStore = ScreenPositionStore(
            defaults: defaults,
            positionsKey: DefaultsKey.positions,
            displayUUIDKey: DefaultsKey.displayUUID
        )
        self.openSettings = openSettings
        tooltip = RadialPaletteTooltip(
            glassOpacity: settingsStore.settings.tools.paletteGlassOpacity
        )
        panel = RadialPalettePanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        super.init()
        updateTooltipAppearance()
        configurePanel()
        restorePosition()
        synchronizePathWithCurrentTool()
        observeStateAndScreens()
        installEventMonitors()
    }

    private func configurePanel() {
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = false
        panel.isMovable = false
        panel.isMovableByWindowBackground = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.acceptsMouseMovedEvents = true
        panel.level = PaletteController.windowLevel
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = .none

        paletteView.controller = self
        paletteView.wantsLayer = true
        paletteView.layer?.backgroundColor = NSColor.clear.cgColor
        panel.contentView = paletteView
        applyPanelFrame()
    }

    private func observeStateAndScreens() {
        annotationObserverIdentifier = state.addAnnotationModeObserver { [weak self] _ in
            self?.updateVisibility()
        }
        passThroughObserverIdentifier = state.addPassThroughModeObserver { [weak self] _ in
            self?.paletteView.redraw()
        }
        toolObserverIdentifier = state.addToolObserver { [weak self] _ in
            guard let self else { return }
            if !self.isApplyingFavorite {
                self.synchronizePathWithCurrentTool()
            }
            self.updateTooltipAppearance()
            self.paletteView.redraw()
        }
        toolSettingsObserverIdentifier = state.addToolSettingsObserver { [weak self] in
            guard let self else { return }
            if self.state.currentTool == .eraser {
                self.synchronizePathWithCurrentTool()
            }
            self.updateTooltipAppearance()
            self.paletteView.redraw()
        }
        visibilityObserverIdentifier = state.addPaletteVisibilityObserver { [weak self] _ in
            self?.updateVisibility()
        }
        boardObserverIdentifier = state.addBoardObserver { [weak self] in
            self?.paletteView.redraw()
        }
        presenterObserverIdentifier = state.addPresenterEffectsObserver { [weak self] in
            self?.paletteView.redraw()
        }
        settingsObserverIdentifier = settingsStore.addObserver { [weak self] settings in
            guard let self else { return }
            self.tooltip.updateAppearance(
                glassOpacity: settings.tools.paletteGlassOpacity
            )
            self.updateVisibility()
            self.paletteView.redraw()
        }
        notificationObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                self?.restorePosition()
                self?.updateVisibility()
            }
        )
    }

    private func installEventMonitors() {
        if let mouseMonitor = NSEvent.addLocalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown],
            handler: { [weak self] event in
                guard let self, self.isExpanded, event.window !== self.panel else { return event }
                self.collapse()
                return nil
            }
        ) {
            eventMonitors.append(mouseMonitor)
        }

        if let keyMonitor = NSEvent.addLocalMonitorForEvents(
            matching: [.flagsChanged, .keyDown, .keyUp],
            handler: { [weak self] event in
                guard let self else { return event }
                if self.consumePaletteKeyboardEvent(event) {
                    self.handleModifierEvent(event)
                    return nil
                }
                self.handleModifierEvent(event)
                return event
            }
        ) {
            eventMonitors.append(keyMonitor)
        }
    }

    func consumePaletteKeyboardEvent(_ event: NSEvent) -> Bool {
        if event.type == .keyUp,
           consumedPaletteKeyCodes.remove(event.keyCode) != nil
        {
            return true
        }

        guard event.type == .keyDown,
              isExpanded,
              panel.isVisible,
              !isTextInputActive,
              event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty,
              let input = paletteKeyboardInput(for: event.keyCode)
        else {
            return false
        }

        consumedPaletteKeyCodes.insert(event.keyCode)
        switch input {
        case let .navigate(key):
            navigatePalette(with: key)
        case .activate:
            if !event.isARepeat {
                activateKeyboardFocus()
            }
        }
        return true
    }

    private enum PaletteKeyboardInput {
        case navigate(RadialPaletteNavigationKey)
        case activate
    }

    private func paletteKeyboardInput(for keyCode: UInt16) -> PaletteKeyboardInput? {
        switch Int(keyCode) {
        case kVK_UpArrow: return .navigate(.deeper)
        case kVK_DownArrow: return .navigate(.shallower)
        case kVK_LeftArrow: return .navigate(.previous)
        case kVK_RightArrow: return .navigate(.next)
        case kVK_Return, kVK_ANSI_KeypadEnter, kVK_Space: return .activate
        default: return nil
        }
    }

    private var isTextInputActive: Bool {
        NSApp.keyWindow?.firstResponder is NSTextView
    }

    private func navigatePalette(with key: RadialPaletteNavigationKey) {
        var cells = layoutCells()
        let current = validKeyboardFocus(in: cells) ?? initialKeyboardFocus(in: cells)
        guard let current else { return }

        if key == .deeper,
           let cell = cell(for: current, in: cells),
           case let .entry(entry) = cell.action,
           selectedEntryIdentifier != entry.identifier,
           entryCanOpenOptions(entry)
        {
            // Selecting an option-bearing entry here follows the same action as a click.
            activate(entry)
            cells = layoutCells()
        }
        let next = nextRadialPaletteFocus(
            in: cells,
            from: current,
            for: key,
            selectedEntryIdentifier: selectedEntryIdentifier
        )

        if (key == .previous || key == .next),
           next != current,
           let cell = cell(for: next, in: cells),
           case .group = cell.action
        {
            activate(.cell(cell))
            cells = layoutCells()
        }

        keyboardFocus = next
        showsKeyboardFocus = true
        paletteView.clearHoverForKeyboardNavigation()
        paletteView.redraw()
        showKeyboardFocusHint(in: cells)
    }

    private func initialKeyboardFocus(in cells: [RadialPaletteCell]) -> RadialPaletteFocus? {
        if let selectedEntryIdentifier,
           let selectedEntryCell = cells.first(where: { cell in
               guard cell.isEnabled, cell.ring == .entry,
                     case let .entry(entry) = cell.action
               else {
                   return false
               }
               return entry.identifier == selectedEntryIdentifier
           })
        {
            return .cell(selectedEntryCell.identifier)
        }
        let selectedToolCell = cells.first { cell in
            guard cell.isEnabled, cell.ring == .entry, cell.isSelected,
                  case let .entry(entry) = cell.action
            else {
                return false
            }
            return entry.inkTool != nil
        }
        if let selectedToolCell {
            return .cell(selectedToolCell.identifier)
        }
        return cells.first { $0.isEnabled && $0.ring == .group && $0.isSelected }
            .map { .cell($0.identifier) }
    }

    private func validKeyboardFocus(in cells: [RadialPaletteCell]) -> RadialPaletteFocus? {
        guard let keyboardFocus else { return nil }
        switch keyboardFocus {
        case .center:
            return .center
        case let .cell(identifier):
            return cells.contains { $0.isEnabled && $0.identifier == identifier }
                ? keyboardFocus
                : nil
        }
    }

    private func cell(
        for focus: RadialPaletteFocus,
        in cells: [RadialPaletteCell]
    ) -> RadialPaletteCell? {
        guard case let .cell(identifier) = focus else { return nil }
        return cells.first { $0.identifier == identifier }
    }

    private func activateKeyboardFocus() {
        guard let focus = validKeyboardFocus(in: layoutCells()) else { return }
        showsKeyboardFocus = true
        paletteView.clearHoverForKeyboardNavigation()
        switch focus {
        case .center:
            activate(.center)
        case .cell:
            guard let cell = cell(for: focus, in: layoutCells()) else { return }
            activate(.cell(cell))
        }
        if isExpanded {
            paletteView.redraw()
            showKeyboardFocusHint(in: layoutCells())
        }
    }

    private func handleModifierEvent(_ event: NSEvent) {
        switch event.type {
        case .flagsChanged:
            let relevant = event.modifierFlags.intersection([.command, .shift, .control, .option])
            if relevant == .command {
                scheduleCommandKeyCaps()
            } else {
                cancelCommandKeyCaps()
            }
        case .keyDown:
            cancelCommandKeyCaps()
        case .keyUp:
            if !event.modifierFlags.contains(.command) {
                cancelCommandKeyCaps()
            }
        default:
            break
        }
    }

    private func scheduleCommandKeyCaps() {
        guard commandHoldWorkItem == nil, !showsKeyCaps else { return }
        let item = DispatchWorkItem { [weak self] in
            guard let self else { return }
            commandHoldWorkItem = nil
            guard NSEvent.modifierFlags.intersection([.command, .shift, .control, .option])
                    == .command
            else {
                return
            }
            showsKeyCaps = true
            paletteView.redraw()
        }
        commandHoldWorkItem = item
        DispatchQueue.main.asyncAfter(
            deadline: .now() + RadialPaletteMetrics.commandHoldDuration,
            execute: item
        )
    }

    private func cancelCommandKeyCaps() {
        commandHoldWorkItem?.cancel()
        commandHoldWorkItem = nil
        guard showsKeyCaps else { return }
        showsKeyCaps = false
        paletteView.redraw()
    }

    private func updateVisibility() {
        let shouldShow = state.isAnnotationModeEnabled
            && state.isPaletteVisible
            && settingsStore.settings.tools.paletteStyle == .radial
        if shouldShow {
            ensureRestingCenterIsOnScreen()
            applyPanelFrame()
            panel.orderFrontRegardless()
        } else {
            collapse()
            panel.orderOut(nil)
        }
    }

    @discardableResult
    func collapseIfExpanded() -> Bool {
        guard isExpanded else { return false }
        collapse()
        return true
    }

    func toggleExpansionFromKeyboard() {
        guard canToggleExpansionFromKeyboard else { return }
        if isExpanded {
            collapse()
            return
        }
        if !state.isPaletteVisible {
            state.togglePaletteVisibility()
        }
        expand()
        showInitialKeyboardFocus()
    }

    private func expand() {
        guard !isExpanded else { return }
        // A hint shown while collapsed is placed beside the small disc and would sit on the rings.
        tooltip.hide()
        synchronizePathWithCurrentTool()
        resetKeyboardNavigation()
        isExpanded = true
        applyPanelFrame()
        paletteView.redraw()
    }

    private func collapse() {
        resetKeyboardNavigation()
        guard isExpanded else {
            tooltip.hide()
            return
        }
        isExpanded = false
        tooltip.hide()
        applyPanelFrame()
        paletteView.redraw()
    }

    private func resetKeyboardNavigation() {
        keyboardFocus = nil
        showsKeyboardFocus = false
    }

    private func showInitialKeyboardFocus() {
        let cells = layoutCells()
        keyboardFocus = initialKeyboardFocus(in: cells)
        showsKeyboardFocus = keyboardFocus != nil
        paletteView.clearHoverForKeyboardNavigation()
        paletteView.redraw()
        showKeyboardFocusHint(in: cells)
    }

    func activate(_ target: RadialPaletteHitTarget) {
        switch target {
        case .center:
            isExpanded ? collapse() : expand()
        case let .cell(cell):
            guard cell.isEnabled else { return }
            switch cell.action {
            case let .group(group):
                selectedGroup = group
                setSelectedEntryIdentifier(nil)
            case let .entry(entry):
                activate(entry)
            case let .option(option):
                option.action()
            }
            paletteView.redraw()
        }
    }

    private func activate(_ entry: RadialPaletteEntry) {
        switch entry.action {
        case let .command(command):
            dispatcher.execute(command, scope: .annotation)
            if entry.inkTool != nil {
                setSelectedEntryIdentifier(entry.identifier)
            }
        case let .selectEraserMode(mode):
            if state.currentTool == .eraser, state.eraserMode == mode {
                if mode == .area {
                    dispatcher.execute(.toggleEraserAreaShape, scope: .annotation)
                }
            } else {
                if state.currentTool != .eraser {
                    dispatcher.execute(.selectTool(.eraser), scope: .annotation)
                }
                state.setEraserMode(mode)
            }
            setSelectedEntryIdentifier(entry.identifier)
        case .openSettings:
            openSettings()
        case .saveFavorite:
            saveCurrentFavorite()
        case let .applyFavorite(_, favorite):
            isApplyingFavorite = true
            defer { isApplyingFavorite = false }
            if state.applyFavorite(favorite) {
                setSelectedEntryIdentifier(entry.identifier)
            }
        }
    }

    fileprivate func stepBack() {
        dispatcher.execute(.stepBack, scope: .annotation)
    }

    private func saveCurrentFavorite() {
        guard let favorite = state.currentFavoriteSnapshot() else { return }
        settingsStore.update { settings in
            guard settings.tools.favorites.count < FavoritesDefaults.maximumCount,
                  !settings.tools.favorites.contains(favorite)
            else {
                return
            }
            settings.tools.favorites.append(favorite)
        }
    }

    @discardableResult
    fileprivate func removeFavorite(for target: RadialPaletteHitTarget) -> Bool {
        guard case let .cell(cell) = target,
              case let .entry(entry) = cell.action,
              case let .applyFavorite(index, favorite) = entry.action
        else {
            return false
        }
        var didRemove = false
        let removed = settingsStore.update { settings in
            guard settings.tools.favorites.indices.contains(index),
                  settings.tools.favorites[index] == favorite
            else {
                return
            }
            settings.tools.favorites.remove(at: index)
            didRemove = true
        }
        guard case .success = removed, didRemove else { return false }
        setSelectedEntryIdentifier(nil)
        keyboardFocus = .cell("entry.palette.favorite.save")
        tooltip.hide()
        paletteView.redraw()
        return true
    }

    fileprivate func hover(_ target: RadialPaletteHitTarget?, at screenPoint: CGPoint) {
        if showsKeyboardFocus {
            showsKeyboardFocus = false
            paletteView.redraw()
        }
        guard let target else {
            tooltip.hide()
            return
        }
        let direction: CGVector
        switch target {
        case .center where isExpanded:
            direction = CGVector(
                dx: screenPoint.x - displayedCenter.x,
                dy: screenPoint.y - displayedCenter.y
            )
        case .center:
            direction = CGVector(dx: 1, dy: 0)
        case let .cell(cell):
            let angle = (cell.startAngle + cell.endAngle) / 2
            direction = CGVector(dx: sin(angle), dy: cos(angle))
        }
        showHint(for: target, preferredDirection: direction)
    }

    private func showKeyboardFocusHint(in cells: [RadialPaletteCell]) {
        guard isExpanded, panel.isVisible, showsKeyboardFocus, let keyboardFocus else { return }
        let target: RadialPaletteHitTarget
        let direction: CGVector
        switch keyboardFocus {
        case .center:
            target = .center
            direction = CGVector(dx: 1, dy: 0)
        case .cell:
            guard let cell = cell(for: keyboardFocus, in: cells) else {
                tooltip.hide()
                return
            }
            target = .cell(cell)
            let angle = (cell.startAngle + cell.endAngle) / 2
            direction = CGVector(dx: sin(angle), dy: cos(angle))
        }
        showHint(for: target, preferredDirection: direction)
    }

    private func showHint(
        for target: RadialPaletteHitTarget,
        preferredDirection: CGVector
    ) {
        if case let .cell(cell) = target, cell.ring == .group {
            tooltip.hide()
            return
        }
        updateTooltipAppearance()
        let name: String
        let shortcut: String
        switch target {
        case .center:
            name = state.currentTool.displayName
            shortcut = centerDefinition()?.displayKeys ?? ""
        case let .cell(cell):
            name = cell.name
            shortcut = cell.shortcut
        }
        let text = shortcut.isEmpty ? name : "\(name)  \(shortcut)"
        tooltip.show(
            text: text,
            paletteCenter: displayedCenter,
            preferredDirection: preferredDirection,
            paletteRadius: isExpanded
                ? RadialPaletteMetrics.expandedDiameter / 2
                : RadialPaletteMetrics.centerRadius
        )
    }

    fileprivate func movePaletteCenter(to proposedCenter: CGPoint) {
        guard let screen = screen(containing: proposedCenter) ?? panel.screen ?? NSScreen.screens.first else {
            NSLog("EduTools: radial palette could not move because no screen is available")
            return
        }
        restingCenter = clampedCenter(
            proposedCenter,
            diameter: RadialPaletteMetrics.collapsedDiameter,
            in: screen.visibleFrame
        )
        currentDisplayUUID = displayUUID(for: screen)
        applyPanelFrame(on: screen)
    }

    fileprivate func finishMovingPalette() {
        saveCurrentPosition()
    }

    private func synchronizePathWithCurrentTool() {
        guard let path = RadialPaletteCatalog.path(
            for: state.currentTool,
            eraserMode: state.currentTool == .eraser ? state.eraserMode : nil
        ) else { return }
        selectedGroup = path.group.id
        setSelectedEntryIdentifier(path.entry.identifier)
    }

    private func setSelectedEntryIdentifier(_ identifier: String?) {
        guard selectedEntryIdentifier != identifier else { return }
        selectedEntryIdentifier = identifier
    }

    private func restorePosition() {
        let screensByIdentifier = screensByDisplayUUID()
        let savedIdentifier = defaults.string(forKey: DefaultsKey.displayUUID)
        let targetScreen: NSScreen
        if let savedIdentifier, let savedScreen = screensByIdentifier[savedIdentifier] {
            targetScreen = savedScreen
            currentDisplayUUID = savedIdentifier
        } else if let primaryScreen = NSScreen.screens.first {
            targetScreen = primaryScreen
            currentDisplayUUID = displayUUID(for: primaryScreen)
        } else {
            NSLog("EduTools: radial palette could not restore its position because no screen is available")
            return
        }

        let diameter = RadialPaletteMetrics.collapsedDiameter
        let origin = restoredOrigin(
            ratio: positionStore.ratio(for: currentDisplayUUID),
            default: CGPoint(x: 0.5, y: 0.85),
            visibleFrame: targetScreen.visibleFrame,
            size: CGSize(width: diameter, height: diameter)
        )
        restingCenter = CGPoint(
            x: origin.x + RadialPaletteMetrics.centerRadius,
            y: origin.y + RadialPaletteMetrics.centerRadius
        )
        applyPanelFrame(on: targetScreen)
    }

    private func saveCurrentPosition() {
        guard let screen = screen(containing: restingCenter) ?? panel.screen,
              let identifier = displayUUID(for: screen)
        else {
            NSLog("EduTools: radial palette could not save its position without a screen")
            return
        }
        let origin = CGPoint(
            x: restingCenter.x - RadialPaletteMetrics.centerRadius,
            y: restingCenter.y - RadialPaletteMetrics.centerRadius
        )
        let diameter = RadialPaletteMetrics.collapsedDiameter
        let ratio = savedRatio(
            origin: origin,
            visibleFrame: screen.visibleFrame,
            size: CGSize(width: diameter, height: diameter)
        )
        positionStore.save(ratio: ratio, displayUUID: identifier)
        currentDisplayUUID = identifier
    }

    private func ensureRestingCenterIsOnScreen() {
        guard screen(containing: restingCenter) == nil else { return }
        restorePosition()
    }

    private func applyPanelFrame(on explicitScreen: NSScreen? = nil) {
        guard let screen = explicitScreen
                ?? screen(containing: restingCenter)
                ?? panel.screen
                ?? NSScreen.screens.first
        else {
            return
        }
        let drawingDiameter = isExpanded
            ? RadialPaletteMetrics.expandedDiameter
            : RadialPaletteMetrics.collapsedDiameter
        let center = clampedCenter(
            restingCenter,
            diameter: drawingDiameter,
            in: screen.visibleFrame
        )
        let windowDiameter = drawingDiameter + 2 * RadialPaletteMetrics.windowMargin
        let frame = CGRect(
            x: center.x - windowDiameter / 2,
            y: center.y - windowDiameter / 2,
            width: windowDiameter,
            height: windowDiameter
        )
        panel.setFrame(frame, display: true)
        paletteView.redraw()
    }

    private func clampedCenter(
        _ center: CGPoint,
        diameter: CGFloat,
        in visibleFrame: CGRect
    ) -> CGPoint {
        let radius = diameter / 2
        let x = visibleFrame.width >= diameter
            ? min(max(center.x, visibleFrame.minX + radius), visibleFrame.maxX - radius)
            : visibleFrame.midX
        let y = visibleFrame.height >= diameter
            ? min(max(center.y, visibleFrame.minY + radius), visibleFrame.maxY - radius)
            : visibleFrame.midY
        return CGPoint(x: x, y: y)
    }

    private func screen(containing point: CGPoint) -> NSScreen? {
        NSScreen.screens.first { $0.frame.contains(point) }
    }

    func hitTarget(at point: CGPoint, in bounds: CGRect) -> RadialPaletteHitTarget? {
        let center = CGPoint(x: bounds.midX, y: bounds.midY)
        let delta = CGPoint(x: point.x - center.x, y: point.y - center.y)
        let radius = hypot(delta.x, delta.y)
        if radius <= RadialPaletteMetrics.centerRadius {
            return .center
        }
        guard isExpanded else { return nil }
        let angle = atan2(delta.x, delta.y)
        for cell in layoutCells()
        where cell.geometry.contains(radius: radius, clockwiseAngle: angle) {
            return .cell(cell)
        }
        return nil
    }

    private func entries(for group: RadialPaletteGroupDefinition) -> [RadialPaletteEntry] {
        guard group.id == .favorites else { return group.entries }

        let favorites = settingsStore.settings.tools.favorites
        let current = state.currentFavoriteSnapshot()
        let isFull = favorites.count >= FavoritesDefaults.maximumCount
        let isDuplicate = current.map(favorites.contains) == true
        let saveStatus: String
        if current == nil {
            saveStatus = "현재 도구 저장 불가"
        } else if isFull {
            saveStatus = "최대 \(FavoritesDefaults.maximumCount)개"
        } else if isDuplicate {
            saveStatus = "이미 저장됨"
        } else {
            saveStatus = ""
        }
        let statusSuffix = saveStatus.isEmpty ? "" : " (\(saveStatus))"
        let saveEntry = RadialPaletteEntry.saveFavorite(
            name: "지금 설정 저장\(statusSuffix) · 저장한 칸을 우클릭하면 삭제됩니다",
            isEnabled: current != nil && !isFull && !isDuplicate
        )
        return [saveEntry] + favorites.enumerated().map { index, favorite in
            .favorite(favorite, index: index, name: favoriteDescription(favorite))
        }
    }

    private func favoriteDescription(_ favorite: FavoriteTool) -> String {
        guard let tool = favorite.inkTool else { return favorite.tool }
        var parts = [tool.displayName]
        if let colorIndex = favorite.colorIndex {
            parts.append("색 \(colorIndex + 1)")
        }
        switch tool {
        case .text:
            parts.append("크기 \(formattedFavoriteValue(favorite.size))")
        case .eraser:
            parts.append("크기 \(formattedFavoriteValue(favorite.size))")
            if let mode = favorite.decodedEraserMode,
               let areaShape = favorite.decodedEraserAreaShape
            {
                switch mode {
                case .partial: parts.append("부분 지우개")
                case .stroke: parts.append("획 지우개")
                case .area: parts.append("영역 지우개(\(areaShape.displayName))")
                }
            }
        case .laser:
            parts.append("꼬리 \(formattedFavoriteValue(favorite.size))초")
        case .arrow:
            parts.append("굵기 \(formattedFavoriteValue(favorite.size))")
            if let style = favorite.decodedArrowStyle {
                parts.append("\(style.displayName) 화살표")
            }
        case .pen, .highlighter, .rectangle, .ellipse:
            parts.append("굵기 \(formattedFavoriteValue(favorite.size))")
        case .select, .cover:
            break
        }
        return parts.joined(separator: " · ")
    }

    private func formattedFavoriteValue(_ value: Double) -> String {
        value.rounded() == value ? String(Int(value)) : String(format: "%.1f", value)
    }

    func layoutCells() -> [RadialPaletteCell] {
        guard isExpanded else { return [] }
        var cells: [RadialPaletteCell] = []
        let groupSlice = 2 * CGFloat.pi / CGFloat(RadialPaletteCatalog.groups.count)
        for (index, group) in RadialPaletteCatalog.groups.enumerated() {
            let centerAngle = CGFloat(index) * groupSlice
            cells.append(
                RadialPaletteCell(
                    identifier: "group.\(group.id.rawValue)",
                    ring: .group,
                    geometry: RadialPaletteSectorGeometry(
                        innerRadius: RadialPaletteMetrics.groupInnerRadius,
                        outerRadius: RadialPaletteMetrics.groupOuterRadius,
                        startAngle: centerAngle - groupSlice / 2
                            + RadialPaletteMetrics.sectorGap,
                        endAngle: centerAngle + groupSlice / 2
                            - RadialPaletteMetrics.sectorGap
                    ),
                    name: group.name,
                    iconKey: group.iconKey,
                    iconName: group.iconName,
                    shortLabel: "",
                    shortcut: "",
                    color: nil,
                    isSelected: group.id == selectedGroup,
                    isEnabled: group.isEnabled,
                    action: .group(group.id)
                )
            )
        }

        let group = RadialPaletteCatalog.definition(for: selectedGroup)
        let groupEntries = entries(for: group)
        if !groupEntries.isEmpty {
            let count = CGFloat(groupEntries.count)
            let span = min(max(count * 24 * .pi / 180, 90 * .pi / 180), 200 * .pi / 180)
            let slice = span / count
            let groupCenter = CGFloat(group.id.rawValue) * groupSlice
            let start = groupCenter - span / 2
            for (index, entry) in groupEntries.enumerated() {
                let cellStart = start + CGFloat(index) * slice + RadialPaletteMetrics.sectorGap
                let cellEnd = start + CGFloat(index + 1) * slice - RadialPaletteMetrics.sectorGap
                cells.append(
                    RadialPaletteCell(
                        identifier: "entry.\(entry.identifier)",
                        ring: .entry,
                        geometry: RadialPaletteSectorGeometry(
                            innerRadius: RadialPaletteMetrics.entryInnerRadius,
                            outerRadius: RadialPaletteMetrics.entryOuterRadius,
                            startAngle: cellStart,
                            endAngle: cellEnd
                        ),
                        name: entry.name,
                        iconKey: iconKey(for: entry),
                        iconName: iconName(for: entry),
                        shortLabel: "",
                        shortcut: entry.shortcut,
                        color: nil,
                        isSelected: entryIsSelected(entry),
                        isEnabled: entry.isEnabled,
                        action: .entry(entry)
                    )
                )
            }
        }

        let optionGroups = selectedEntry().map(optionGroups(for:)) ?? []
        let optionCount = optionGroups.reduce(0) { $0 + $1.options.count }
        if optionCount > 0 {
            let totalGap = CGFloat(optionGroups.count) * RadialPaletteMetrics.propertyGap
            let optionSlice = (2 * .pi - totalGap) / CGFloat(optionCount)
            var cursor = RadialPaletteMetrics.propertyGap / 2
            for optionGroup in optionGroups {
                for option in optionGroup.options {
                    let cellStart = cursor + RadialPaletteMetrics.sectorGap / 2
                    let cellEnd = cursor + optionSlice - RadialPaletteMetrics.sectorGap / 2
                    cells.append(
                        RadialPaletteCell(
                            identifier: "option.\(option.identifier)",
                            ring: .option,
                            geometry: RadialPaletteSectorGeometry(
                                innerRadius: RadialPaletteMetrics.optionInnerRadius,
                                outerRadius: RadialPaletteMetrics.optionOuterRadius,
                                startAngle: cellStart,
                                endAngle: cellEnd
                            ),
                            name: option.name,
                            iconKey: option.iconKey,
                            iconName: option.iconName,
                            shortLabel: option.shortLabel,
                            shortcut: option.shortcut,
                            color: option.color,
                            preview: option.preview,
                            isSelected: option.isSelected,
                            isEnabled: option.isEnabled,
                            action: .option(option)
                        )
                    )
                    cursor += optionSlice
                }
                cursor += RadialPaletteMetrics.propertyGap
            }
        }
        return cells
    }

    private func selectedEntry() -> RadialPaletteEntry? {
        guard let selectedEntryIdentifier else { return nil }
        return entries(for: RadialPaletteCatalog.definition(for: selectedGroup)).first {
            $0.identifier == selectedEntryIdentifier
        }
    }

    private func entryIsSelected(_ entry: RadialPaletteEntry) -> Bool {
        if let favorite = entry.favorite {
            return state.currentFavoriteSnapshot() == favorite
        }
        if let eraserMode = entry.eraserMode {
            return state.currentTool == .eraser && state.eraserMode == eraserMode
        }
        if let tool = entry.inkTool {
            return state.currentTool == tool
        }
        switch entry.action {
        case .command(.toggleSpotlight): return state.isSpotlightEnabled
        case .command(.toggleCursorHighlight): return state.isCursorHighlightEnabled
        case .command(.togglePassThroughMode): return state.isPassThroughModeEnabled
        case .command(.toggleZoom): return state.isZoomEnabled
        case let .command(.toggleBoard(kind)): return state.presentedBoard?.kind == kind
        default: return entry.identifier == selectedEntryIdentifier
        }
    }

    private func optionGroups(for entry: RadialPaletteEntry) -> [RadialPaletteOptionGroup] {
        guard let tool = entry.inkTool else { return [] }
        switch tool {
        case .pen:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.writingWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "pen.width",
                makePreview: { .strokeWidth(relative: $0, color: $1) },
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            )]
        case .highlighter:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.highlighterWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "highlighter.width",
                makePreview: { .strokeWidth(relative: $0, color: $1) },
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            )]
        case .text:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.textSizes,
                selected: rememberedWidth(for: tool),
                propertyName: "크기",
                idPrefix: "text.size",
                makePreview: { .textSize(relative: $0, color: $1) },
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            )]
        case .arrow:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.writingWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "arrow.width",
                makePreview: { .strokeWidth(relative: $0, color: $1) },
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            )]
        case .rectangle, .ellipse:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.writingWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "shape.width",
                makePreview: { .strokeWidth(relative: $0, color: $1) },
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            )]
        case .eraser:
            return eraserOptions(for: entry.eraserMode ?? state.eraserMode)
        case .laser:
            return [laserColorOptions()]
        case .select, .cover:
            return []
        }
    }

    private func entryCanOpenOptions(_ entry: RadialPaletteEntry) -> Bool {
        return entry.inkTool != nil && !optionGroups(for: entry).isEmpty
    }

    private func rememberedWidth(for tool: InkTool) -> CGFloat {
        guard let width = state.width(for: tool) else {
            preconditionFailure("Tool does not have a remembered size: \(tool.rawValue)")
        }
        return width
    }

    private func colorOptions() -> RadialPaletteOptionGroup {
        let options = state.colorPresets.indices.map { index -> RadialPaletteOption in
            let definition = requiredDefinition(for: .selectColor(index))
            return RadialPaletteOption(
                identifier: "ink.color.\(index)",
                name: definition.name,
                shortLabel: "",
                iconKey: nil,
                iconName: nil,
                color: state.colorPresets[index],
                shortcut: definition.displayKeys,
                isSelected: state.selectedColorIndex == index,
                action: { [weak state = self.state] in state?.selectColor(at: index) }
            )
        }
        return RadialPaletteOptionGroup(name: "색", options: options)
    }

    private func widthOptions(
        values: [CGFloat],
        selected: CGFloat,
        propertyName: String,
        idPrefix: String,
        makePreview: (CGFloat, NSColor) -> RadialPalettePreview,
        apply: @escaping (CGFloat) -> Void
    ) -> RadialPaletteOptionGroup {
        let inkColor = state.selectedColor
        let options = values.map { value in
            let label = formattedNumber(value)
            // Tool ranges differ, so normalize within each preset group to keep previews in-cell.
            let relative = RadialPalettePreview.relativeSize(of: value, in: values)
            return RadialPaletteOption(
                identifier: "\(idPrefix).\(label)",
                name: "\(propertyName) \(label)",
                shortLabel: label,
                iconKey: nil,
                iconName: nil,
                color: nil,
                preview: makePreview(relative, inkColor),
                shortcut: "",
                isSelected: abs(selected - value) < 0.001,
                action: { apply(value) }
            )
        }
        return RadialPaletteOptionGroup(name: propertyName, options: options)
    }

    private func eraserOptions(for mode: EraserMode) -> [RadialPaletteOptionGroup] {
        guard mode != .area else { return [] }
        let diameterValues = settingsStore.settings.eraser.presetDiameters.map { CGFloat($0) }
        let sizes = diameterValues.enumerated().map {
            index, value in
            let label = formattedNumber(value)
            return RadialPaletteOption(
                identifier: "eraser.size.\(index)",
                name: "지우개 크기 \(label)",
                shortLabel: label,
                iconKey: nil,
                iconName: nil,
                color: nil,
                preview: .eraserDiameter(relative: RadialPalettePreview.relativeSize(
                    of: value,
                    in: diameterValues
                )),
                shortcut: "",
                isSelected: abs(state.eraserDiameter - value) < 0.001,
                action: { [weak state = self.state] in state?.setEraserDiameter(value) }
            )
        }
        return [RadialPaletteOptionGroup(name: "크기", options: sizes)]
    }

    private func laserColorOptions() -> RadialPaletteOptionGroup {
        let options = state.colorPresets.indices.map { index -> RadialPaletteOption in
            let color = state.colorPresets[index]
            let definition = requiredDefinition(for: .selectColor(index))
            return RadialPaletteOption(
                identifier: "laser.color.\(index)",
                name: definition.name,
                shortLabel: "",
                iconKey: nil,
                iconName: nil,
                color: color,
                shortcut: "",
                isSelected: state.laserColor.isEqual(color),
                action: { [weak state = self.state] in state?.setLaserColor(color) }
            )
        }
        return RadialPaletteOptionGroup(name: "색", options: options)
    }

    private func formattedNumber(_ value: CGFloat) -> String {
        value.rounded() == value ? String(Int(value)) : String(format: "%.1f", value)
    }

    private func requiredDefinition(for command: AppCommand) -> ShortcutDefinition {
        guard let definition = ShortcutCatalog.definition(for: command) else {
            preconditionFailure("Radial palette command is missing from ShortcutCatalog: \(command)")
        }
        return definition
    }

    private func iconKey(for entry: RadialPaletteEntry) -> String {
        guard entry.eraserMode == .area else { return entry.iconKey }
        return state.eraserAreaShape == .rectangle
            ? AppIcon.Key.optionAreaRectangle
            : AppIcon.Key.optionAreaLasso
    }

    private func iconName(for entry: RadialPaletteEntry) -> String {
        guard entry.eraserMode == .area else { return entry.iconName }
        return state.eraserAreaShape == .rectangle ? "rectangle.dashed" : "lasso"
    }

    fileprivate func updateRenderModel(
        _ model: RadialPaletteRenderModel,
        hoveredIdentifier: String?
    ) {
        let glassOpacity = settingsStore.settings.tools.paletteGlassOpacity
        if model.glassOpacity != glassOpacity {
            model.glassOpacity = glassOpacity
        }
        let cells = isExpanded ? layoutCells() : []
        let focusedIdentifier = showsKeyboardFocus ? keyboardFocus?.identifier : nil
        let centerDefinition = state.isPassThroughModeEnabled
            ? requiredDefinition(for: .togglePassThroughMode)
            : centerDefinition()
        model.state = RadialPaletteRenderState(
            isExpanded: isExpanded,
            showsKeyCaps: showsKeyCaps,
            cells: cells.map { cell in
                RadialPaletteRenderCell(
                    id: cell.identifier,
                    ring: cell.ring,
                    geometry: cell.geometry,
                    iconKey: cell.iconKey,
                    iconName: cell.iconName,
                    attributeConfiguration: attributeConfiguration(for: cell),
                    accessibilityLabel: cell.name,
                    shortLabel: cell.shortLabel,
                    shortcut: cell.shortcut,
                    color: cell.color,
                    preview: cell.preview,
                    isSelected: cell.isSelected,
                    isEnabled: cell.isEnabled,
                    isHovered: cell.identifier == hoveredIdentifier,
                    isFocused: cell.identifier == focusedIdentifier
                )
            },
            centerIconKey: centerDefinition?.id,
            centerIconName: centerDefinition?.iconName,
            centerAttributeConfiguration: state.isPassThroughModeEnabled
                ? nil
                : AttrIcon.configuration(
                    for: state.currentTool,
                    state: state,
                    settings: settingsStore.settings
                ),
            centerAccessibilityLabel: centerDefinition?.name,
            centerShortcut: centerDefinition?.displayKeys ?? "",
            centerColor: centerColor(),
            centerStrokeWidth: centerStrokeWidth(),
            centerIsPassThroughEnabled: state.isPassThroughModeEnabled,
            centerIsHovered: hoveredIdentifier == "palette.center",
            centerIsFocused: focusedIdentifier == "palette.center"
        )
    }

    private func attributeConfiguration(
        for cell: RadialPaletteCell
    ) -> AttrIconConfiguration? {
        guard case let .entry(entry) = cell.action else {
            return nil
        }
        if let favorite = entry.favorite {
            return AttrIcon.configuration(
                for: favorite,
                settings: settingsStore.settings
            )
        }
        if entry.eraserMode != nil {
            return nil
        }
        guard let tool = entry.inkTool else { return nil }
        return AttrIcon.configuration(
            for: tool,
            state: state,
            settings: settingsStore.settings
        )
    }

    private func centerDefinition() -> ShortcutDefinition? {
        let command: AppCommand
        switch state.currentTool {
        case .laser: command = .useLaser
        case .eraser: command = .selectOrCycleEraser
        default: command = .selectTool(state.currentTool)
        }
        return ShortcutCatalog.definition(for: command)
    }

    private func centerColor() -> NSColor {
        state.currentTool == .laser ? state.laserColor : state.selectedColor
    }

    private func updateTooltipAppearance() {
        tooltip.updateAppearance(
            glassOpacity: settingsStore.settings.tools.paletteGlassOpacity
        )
    }

    private func centerStrokeWidth() -> CGFloat {
        let value: CGFloat
        switch state.currentTool {
        case .pen, .highlighter, .arrow, .rectangle, .ellipse: value = state.currentWidth
        case .laser: value = state.laserSize
        case .eraser: value = state.eraserDiameter / 3
        case .text: value = state.currentWidth / 3
        case .select, .cover: value = 2
        }
        return min(max(value / 3, 2), 6)
    }

    deinit {
        commandHoldWorkItem?.cancel()
        for monitor in eventMonitors {
            NSEvent.removeMonitor(monitor)
        }
        for observer in notificationObservers {
            NotificationCenter.default.removeObserver(observer)
        }
        if let annotationObserverIdentifier {
            state.removeAnnotationModeObserver(annotationObserverIdentifier)
        }
        if let passThroughObserverIdentifier {
            state.removePassThroughModeObserver(passThroughObserverIdentifier)
        }
        if let toolObserverIdentifier {
            state.removeToolObserver(toolObserverIdentifier)
        }
        if let toolSettingsObserverIdentifier {
            state.removeToolSettingsObserver(toolSettingsObserverIdentifier)
        }
        if let visibilityObserverIdentifier {
            state.removePaletteVisibilityObserver(visibilityObserverIdentifier)
        }
        if let boardObserverIdentifier {
            state.removeBoardObserver(boardObserverIdentifier)
        }
        if let presenterObserverIdentifier {
            state.removePresenterEffectsObserver(presenterObserverIdentifier)
        }
        if let settingsObserverIdentifier {
            settingsStore.removeObserver(settingsObserverIdentifier)
        }
        panel.close()
    }
}
