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

private enum RadialPaletteGroupID: Int, CaseIterable {
    case writing
    case shapes
    case erasing
    case boardAndSettings
    case presenting
    case favorites
}

private enum RadialPaletteEntryAction {
    case command(AppCommand)
    case openSettings
    case setPaletteStyle(PaletteStyle)
}

private struct RadialPaletteEntry {
    let action: RadialPaletteEntryAction
    let fallbackName: String?
    let fallbackIconName: String?

    static func command(_ command: AppCommand) -> RadialPaletteEntry {
        RadialPaletteEntry(action: .command(command), fallbackName: nil, fallbackIconName: nil)
    }

    static let openSettings = RadialPaletteEntry(
        action: .openSettings,
        fallbackName: "설정 열기",
        fallbackIconName: "gearshape"
    )

    static func paletteStyle(_ style: PaletteStyle) -> RadialPaletteEntry {
        RadialPaletteEntry(
            action: .setPaletteStyle(style),
            fallbackName: style.displayName,
            fallbackIconName: style.iconName
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
        case .openSettings: return "palette.settings"
        case let .setPaletteStyle(style): return "palette.style.\(style.rawValue)"
        case .command:
            preconditionFailure("A palette command is missing from ShortcutCatalog")
        }
    }

    var name: String {
        definition?.name ?? fallbackName ?? identifier
    }

    var iconName: String {
        definition?.iconName ?? fallbackIconName ?? "circle"
    }

    var shortcut: String {
        definition?.displayKeys ?? ""
    }

    var inkTool: InkTool? {
        switch action {
        case let .command(.selectTool(tool)): return tool
        case .command(.useLaser): return .laser
        case .command(.selectOrCycleEraser): return .eraser
        default: return nil
        }
    }
}

private struct RadialPaletteGroupDefinition {
    let id: RadialPaletteGroupID
    let name: String
    let iconName: String
    let isEnabled: Bool
    let entries: [RadialPaletteEntry]
}

private enum RadialPaletteCatalog {
    static let groups: [RadialPaletteGroupDefinition] = [
        RadialPaletteGroupDefinition(
            id: .writing,
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
            name: "지우기",
            iconName: "eraser",
            isEnabled: true,
            entries: [
                .command(.selectOrCycleEraser),
                .command(.undo),
                .command(.redo),
                .command(.clearAll),
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .boardAndSettings,
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
                .command(.showHelp),
                .openSettings,
                .paletteStyle(.radial),
                .paletteStyle(.linear),
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .presenting,
            name: "발표",
            iconName: "person.fill.viewfinder",
            isEnabled: true,
            entries: [
                .command(.useLaser),
                .command(.toggleSpotlight),
                .command(.selectTool(.cover)),
                .command(.toggleZoom),
                .command(.toggleCursorHighlight),
            ]
        ),
        RadialPaletteGroupDefinition(
            id: .favorites,
            name: "즐겨찾기",
            iconName: "star.fill",
            isEnabled: false,
            entries: []
        ),
    ]

    static func definition(for id: RadialPaletteGroupID) -> RadialPaletteGroupDefinition {
        guard let definition = groups.first(where: { $0.id == id }) else {
            preconditionFailure("Missing radial palette group \(id)")
        }
        return definition
    }

    static func path(for tool: InkTool) -> (
        group: RadialPaletteGroupDefinition,
        entry: RadialPaletteEntry
    )? {
        for group in groups {
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

private struct RadialPaletteOption {
    let identifier: String
    let name: String
    let shortLabel: String
    let iconName: String?
    let color: NSColor?
    let shortcut: String
    let isSelected: Bool
    let action: () -> Void
}

private struct RadialPaletteOptionGroup {
    let name: String
    let options: [RadialPaletteOption]
}

private struct RadialPaletteCell {
    enum Action {
        case group(RadialPaletteGroupID)
        case entry(RadialPaletteEntry)
        case option(RadialPaletteOption)
    }

    let identifier: String
    let ring: RadialPaletteRing
    let geometry: RadialPaletteSectorGeometry
    let name: String
    let iconName: String?
    let shortLabel: String
    let shortcut: String
    let propertyName: String?
    let color: NSColor?
    let isSelected: Bool
    let isEnabled: Bool
    let action: Action

    var innerRadius: CGFloat { geometry.innerRadius }
    var outerRadius: CGFloat { geometry.outerRadius }
    var startAngle: CGFloat { geometry.startAngle }
    var endAngle: CGFloat { geometry.endAngle }
}

private enum RadialPaletteHitTarget {
    case center
    case cell(RadialPaletteCell)

    var identifier: String {
        switch self {
        case .center: return "palette.center"
        case let .cell(cell): return cell.identifier
        }
    }
}

private enum RadialPaletteNavigationKey: Equatable {
    case shallower
    case deeper
    case previous
    case next
}

private enum RadialPaletteFocus: Equatable {
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
private func nextRadialPaletteFocus(
    in cells: [RadialPaletteCell],
    from current: RadialPaletteFocus,
    for key: RadialPaletteNavigationKey
) -> RadialPaletteFocus {
    let enabledCells = cells.filter(\.isEnabled)

    func selectedCell(in ring: RadialPaletteRing) -> RadialPaletteCell? {
        enabledCells.first { $0.ring == ring && $0.isSelected }
    }

    func selectedToolCell() -> RadialPaletteCell? {
        enabledCells.first { cell in
            guard cell.ring == .entry, cell.isSelected,
                  case let .entry(entry) = cell.action
            else {
                return false
            }
            return entry.inkTool != nil
        }
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
                return focus(for: selectedToolCell()) ?? current
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
                return focus(for: selectedToolCell() ?? firstCell(in: .entry)) ?? current
            case .entry:
                guard cell.isSelected,
                      case let .entry(entry) = cell.action,
                      entry.inkTool != nil
                else {
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
    private let panel: RadialPaletteTooltipPanel
    private let label = NSTextField(labelWithString: "")

    init() {
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

        let glassView = NSGlassEffectView(frame: .zero)
        glassView.style = .regular
        glassView.cornerRadius = 7
        let root = NSView(frame: .zero)
        label.font = .systemFont(ofSize: 12, weight: .medium)
        label.textColor = .labelColor
        label.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 9),
            label.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -9),
            label.topAnchor.constraint(equalTo: root.topAnchor, constant: 6),
            label.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -6),
        ])
        glassView.contentView = root
        panel.contentView = glassView
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
    private enum DefaultsKey {
        static let positions = "EduTools.radialPalette.positions"
        static let displayUUID = "EduTools.radialPalette.displayUUID"
    }

    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let settingsStore: SettingsStore
    private let defaults: UserDefaults
    private let openSettings: () -> Void
    private let panel: RadialPalettePanel
    private let paletteView = RadialPaletteView(frame: .zero)
    private let tooltip = RadialPaletteTooltip()
    private var selectedGroup = RadialPaletteGroupID.writing
    private var selectedEntryIdentifier: String?
    private var isExpanded = false
    private var showsKeyCaps = false
    private var keyboardFocus: RadialPaletteFocus?
    private var showsKeyboardFocus = false
    private var consumedPaletteKeyCodes: Set<UInt16> = []
    private var restingCenter = CGPoint.zero
    private var currentDisplayUUID: String?
    private var annotationObserverIdentifier: UUID?
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
        self.openSettings = openSettings
        panel = RadialPalettePanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        super.init()
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
        toolObserverIdentifier = state.addToolObserver { [weak self] _ in
            self?.synchronizePathWithCurrentTool()
            self?.paletteView.redraw()
        }
        toolSettingsObserverIdentifier = state.addToolSettingsObserver { [weak self] in
            self?.paletteView.redraw()
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
        settingsObserverIdentifier = settingsStore.addObserver { [weak self] _ in
            self?.updateVisibility()
            self?.paletteView.redraw()
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

    private func consumePaletteKeyboardEvent(_ event: NSEvent) -> Bool {
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
           entry.inkTool != nil,
           selectedEntryIdentifier != entry.identifier,
           !optionGroups(for: entry).isEmpty
        {
            // Selecting a tool here is intentionally the same dispatcher-backed action as a click.
            activate(entry)
            cells = layoutCells()
        }
        let next = nextRadialPaletteFocus(in: cells, from: current, for: key)

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

    fileprivate func activate(_ target: RadialPaletteHitTarget) {
        switch target {
        case .center:
            isExpanded ? collapse() : expand()
        case let .cell(cell):
            guard cell.isEnabled else { return }
            switch cell.action {
            case let .group(group):
                selectedGroup = group
                selectedEntryIdentifier = nil
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
                selectedEntryIdentifier = entry.identifier
            }
        case .openSettings:
            openSettings()
        case let .setPaletteStyle(style):
            settingsStore.update { $0.tools.paletteStyle = style }
        }
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
        guard let path = RadialPaletteCatalog.path(for: state.currentTool) else { return }
        selectedGroup = path.group.id
        selectedEntryIdentifier = path.entry.identifier
    }

    private func restorePosition() {
        let screensByIdentifier = Dictionary(
            uniqueKeysWithValues: NSScreen.screens.compactMap { screen in
                displayUUID(for: screen).map { ($0, screen) }
            }
        )
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

        let visible = targetScreen.visibleFrame
        let availableWidth = max(0, visible.width - RadialPaletteMetrics.collapsedDiameter)
        let availableHeight = max(0, visible.height - RadialPaletteMetrics.collapsedDiameter)
        let saved = savedRatio(for: currentDisplayUUID)
        let xRatio = min(max(saved?.x ?? 0.5, 0), 1)
        let yRatio = min(max(saved?.y ?? 0.85, 0), 1)
        restingCenter = CGPoint(
            x: visible.minX + RadialPaletteMetrics.centerRadius + availableWidth * xRatio,
            y: visible.minY + RadialPaletteMetrics.centerRadius + availableHeight * yRatio
        )
        applyPanelFrame(on: targetScreen)
    }

    private func savedRatio(for identifier: String?) -> CGPoint? {
        guard let identifier,
              let positions = defaults.dictionary(forKey: DefaultsKey.positions),
              let entry = positions[identifier] as? [String: Any],
              let x = entry["x"] as? NSNumber,
              let y = entry["y"] as? NSNumber
        else {
            return nil
        }
        return CGPoint(x: x.doubleValue, y: y.doubleValue)
    }

    private func saveCurrentPosition() {
        guard let screen = screen(containing: restingCenter) ?? panel.screen,
              let identifier = displayUUID(for: screen)
        else {
            NSLog("EduTools: radial palette could not save its position without a screen")
            return
        }
        let visible = screen.visibleFrame
        let availableWidth = max(1, visible.width - RadialPaletteMetrics.collapsedDiameter)
        let availableHeight = max(1, visible.height - RadialPaletteMetrics.collapsedDiameter)
        let origin = CGPoint(
            x: restingCenter.x - RadialPaletteMetrics.centerRadius,
            y: restingCenter.y - RadialPaletteMetrics.centerRadius
        )
        let xRatio = min(max((origin.x - visible.minX) / availableWidth, 0), 1)
        let yRatio = min(max((origin.y - visible.minY) / availableHeight, 0), 1)
        var positions = defaults.dictionary(forKey: DefaultsKey.positions) ?? [:]
        positions[identifier] = ["x": xRatio, "y": yRatio]
        defaults.set(positions, forKey: DefaultsKey.positions)
        defaults.set(identifier, forKey: DefaultsKey.displayUUID)
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
        let diameter = isExpanded
            ? RadialPaletteMetrics.expandedDiameter
            : RadialPaletteMetrics.collapsedDiameter
        let center = isExpanded
            ? clampedCenter(restingCenter, diameter: diameter, in: screen.visibleFrame)
            : clampedCenter(restingCenter, diameter: diameter, in: screen.visibleFrame)
        let frame = CGRect(
            x: center.x - diameter / 2,
            y: center.y - diameter / 2,
            width: diameter,
            height: diameter
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

    fileprivate func hitTarget(at point: CGPoint, in bounds: CGRect) -> RadialPaletteHitTarget? {
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

    private func layoutCells() -> [RadialPaletteCell] {
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
                    iconName: group.iconName,
                    shortLabel: group.name,
                    shortcut: "",
                    propertyName: nil,
                    color: nil,
                    isSelected: group.id == selectedGroup,
                    isEnabled: group.isEnabled,
                    action: .group(group.id)
                )
            )
        }

        let group = RadialPaletteCatalog.definition(for: selectedGroup)
        if !group.entries.isEmpty {
            let count = CGFloat(group.entries.count)
            let span = min(max(count * 24 * .pi / 180, 72 * .pi / 180), 200 * .pi / 180)
            let slice = span / count
            let groupCenter = CGFloat(group.id.rawValue) * groupSlice
            let start = groupCenter - span / 2
            for (index, entry) in group.entries.enumerated() {
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
                        iconName: entry.iconName,
                        shortLabel: "",
                        shortcut: entry.shortcut,
                        propertyName: nil,
                        color: nil,
                        isSelected: entryIsSelected(entry),
                        isEnabled: true,
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
                            iconName: option.iconName,
                            shortLabel: option.shortLabel,
                            shortcut: option.shortcut,
                            propertyName: optionGroup.name,
                            color: option.color,
                            isSelected: option.isSelected,
                            isEnabled: true,
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
        return RadialPaletteCatalog.definition(for: selectedGroup).entries.first {
            $0.identifier == selectedEntryIdentifier
        }
    }

    private func entryIsSelected(_ entry: RadialPaletteEntry) -> Bool {
        if let tool = entry.inkTool {
            return state.currentTool == tool
        }
        switch entry.action {
        case .command(.toggleSpotlight): return state.isSpotlightEnabled
        case .command(.toggleCursorHighlight): return state.isCursorHighlightEnabled
        case .command(.toggleZoom): return state.isZoomEnabled
        case let .command(.toggleBoard(kind)): return state.presentedBoard?.kind == kind
        case let .setPaletteStyle(style): return settingsStore.settings.tools.paletteStyle == style
        default: return entry.identifier == selectedEntryIdentifier
        }
    }

    private func optionGroups(for entry: RadialPaletteEntry) -> [RadialPaletteOptionGroup] {
        guard let tool = entry.inkTool else { return [] }
        switch tool {
        case .pen:
            let recognitionDefinition = requiredDefinition(for: .toggleShapeRecognition)
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.writingWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "pen.width",
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            ), booleanOptions(
                name: recognitionDefinition.name,
                propertyName: "보정",
                idPrefix: "pen.recognition",
                selected: state.isShapeRecognitionEnabled,
                apply: { [weak state = self.state] in state?.setShapeRecognitionEnabled($0) }
            )]
        case .highlighter:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.highlighterWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "highlighter.width",
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            ), opacityOptions()]
        case .text:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.textSizes,
                selected: rememberedWidth(for: tool),
                propertyName: "크기",
                idPrefix: "text.size",
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            )]
        case .arrow:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.writingWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "arrow.width",
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            ), arrowStyleOptions()]
        case .rectangle, .ellipse:
            return [colorOptions(), widthOptions(
                values: RadialPaletteMetrics.writingWidths,
                selected: rememberedWidth(for: tool),
                propertyName: "굵기",
                idPrefix: "shape.width",
                apply: { [weak state = self.state] in state?.setWidth($0, for: tool) }
            )]
        case .eraser:
            return eraserOptions()
        case .laser:
            return [laserColorOptions(), laserTrailOptions()]
        case .select, .cover:
            return []
        }
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
                shortLabel: String(index + 1),
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
        apply: @escaping (CGFloat) -> Void
    ) -> RadialPaletteOptionGroup {
        let options = values.map { value in
            let label = formattedNumber(value)
            return RadialPaletteOption(
                identifier: "\(idPrefix).\(label)",
                name: "\(propertyName) \(label)",
                shortLabel: label,
                iconName: nil,
                color: nil,
                shortcut: "",
                isSelected: abs(selected - value) < 0.001,
                action: { apply(value) }
            )
        }
        return RadialPaletteOptionGroup(name: propertyName, options: options)
    }

    private func opacityOptions() -> RadialPaletteOptionGroup {
        let options = RadialPaletteMetrics.highlighterOpacities.map { value in
            let percent = Int((value * 100).rounded())
            return RadialPaletteOption(
                identifier: "highlighter.opacity.\(percent)",
                name: "투명도 \(percent)%",
                shortLabel: "\(percent)",
                iconName: nil,
                color: nil,
                shortcut: "",
                isSelected: abs(state.highlighterOpacity - value) < 0.001,
                action: { [weak state = self.state] in state?.setHighlighterOpacity(value) }
            )
        }
        return RadialPaletteOptionGroup(name: "투명도", options: options)
    }

    private func booleanOptions(
        name: String,
        propertyName: String,
        idPrefix: String,
        selected: Bool,
        apply: @escaping (Bool) -> Void
    ) -> RadialPaletteOptionGroup {
        let options = [false, true].map { value in
            RadialPaletteOption(
                identifier: "\(idPrefix).\(value)",
                name: "\(name) \(value ? "켬" : "끔")",
                shortLabel: value ? "켬" : "끔",
                iconName: value ? "checkmark" : "xmark",
                color: nil,
                shortcut: "",
                isSelected: selected == value,
                action: { apply(value) }
            )
        }
        return RadialPaletteOptionGroup(name: propertyName, options: options)
    }

    private func arrowStyleOptions() -> RadialPaletteOptionGroup {
        let styles: [ArrowStyle] = [.single, .double]
        let options = styles.map { style in
            RadialPaletteOption(
                identifier: "arrow.style.\(style.rawValue)",
                name: style.displayName,
                shortLabel: style.displayName,
                iconName: style == .single ? "arrow.right" : "arrow.left.and.right",
                color: nil,
                shortcut: "",
                isSelected: state.arrowStyle == style,
                action: { [weak state = self.state] in state?.setArrowStyle(style) }
            )
        }
        return RadialPaletteOptionGroup(name: "화살촉", options: options)
    }

    private func eraserOptions() -> [RadialPaletteOptionGroup] {
        let modes = EraserMode.allCases.map { mode in
            let shortLabel = eraserModeShortLabel(mode)
            return RadialPaletteOption(
                identifier: "eraser.mode.\(shortLabel)",
                name: mode.displayName,
                shortLabel: shortLabel,
                iconName: nil,
                color: nil,
                shortcut: "",
                isSelected: state.eraserMode == mode,
                action: { [weak state = self.state] in state?.setEraserMode(mode) }
            )
        }
        let sizes = settingsStore.settings.eraser.presetDiameters.enumerated().map {
            index, value in
            let label = formattedNumber(CGFloat(value))
            return RadialPaletteOption(
                identifier: "eraser.size.\(index)",
                name: "지우개 크기 \(label)",
                shortLabel: label,
                iconName: nil,
                color: nil,
                shortcut: "",
                isSelected: abs(state.eraserDiameter - CGFloat(value)) < 0.001,
                action: { [weak state = self.state] in state?.setEraserDiameter(CGFloat(value)) }
            )
        }
        let shapes = EraserAreaShape.allCases.map { shape in
            RadialPaletteOption(
                identifier: "eraser.area.\(shape == .rectangle ? "rectangle" : "lasso")",
                name: shape.displayName,
                shortLabel: shape.displayName,
                iconName: shape == .rectangle ? "rectangle.dashed" : "lasso",
                color: nil,
                shortcut: "",
                isSelected: state.eraserAreaShape == shape,
                action: { [weak state = self.state] in state?.setEraserAreaShape(shape) }
            )
        }
        return [
            RadialPaletteOptionGroup(name: "방식", options: modes),
            RadialPaletteOptionGroup(name: "크기", options: sizes),
            RadialPaletteOptionGroup(name: "영역", options: shapes),
        ]
    }

    private func laserColorOptions() -> RadialPaletteOptionGroup {
        let options = state.colorPresets.indices.map { index -> RadialPaletteOption in
            let color = state.colorPresets[index]
            let definition = requiredDefinition(for: .selectColor(index))
            return RadialPaletteOption(
                identifier: "laser.color.\(index)",
                name: definition.name,
                shortLabel: String(index + 1),
                iconName: nil,
                color: color,
                shortcut: "",
                isSelected: state.laserColor.isEqual(color),
                action: { [weak state = self.state] in state?.setLaserColor(color) }
            )
        }
        return RadialPaletteOptionGroup(name: "색", options: options)
    }

    private func laserTrailOptions() -> RadialPaletteOptionGroup {
        let options = RadialPaletteMetrics.laserTrailDurations.map { duration in
            let label = formattedNumber(CGFloat(duration))
            return RadialPaletteOption(
                identifier: "laser.trail.\(label)",
                name: "레이저 꼬리 \(label)초",
                shortLabel: label,
                iconName: nil,
                color: nil,
                shortcut: "",
                isSelected: abs(state.laserTrailDuration - duration) < 0.001,
                action: { [weak state = self.state] in state?.setLaserTrailDuration(duration) }
            )
        }
        return RadialPaletteOptionGroup(name: "꼬리", options: options)
    }

    private func formattedNumber(_ value: CGFloat) -> String {
        value.rounded() == value ? String(Int(value)) : String(format: "%.1f", value)
    }

    private func eraserModeShortLabel(_ mode: EraserMode) -> String {
        switch mode {
        case .partial: return "부분"
        case .stroke: return "획"
        case .area: return "영역"
        }
    }

    private func requiredDefinition(for command: AppCommand) -> ShortcutDefinition {
        guard let definition = ShortcutCatalog.definition(for: command) else {
            preconditionFailure("Radial palette command is missing from ShortcutCatalog: \(command)")
        }
        return definition
    }

    fileprivate func updateRenderModel(
        _ model: RadialPaletteRenderModel,
        hoveredIdentifier: String?
    ) {
        let cells = isExpanded ? layoutCells() : []
        let focusedIdentifier = showsKeyboardFocus ? keyboardFocus?.identifier : nil
        let centerDefinition = centerDefinition()
        model.state = RadialPaletteRenderState(
            isExpanded: isExpanded,
            showsKeyCaps: showsKeyCaps,
            cells: cells.map { cell in
                RadialPaletteRenderCell(
                    id: cell.identifier,
                    ring: cell.ring,
                    geometry: cell.geometry,
                    iconName: cell.iconName,
                    shortLabel: cell.shortLabel,
                    shortcut: cell.shortcut,
                    color: cell.color,
                    isSelected: cell.isSelected,
                    isEnabled: cell.isEnabled,
                    isHovered: cell.identifier == hoveredIdentifier,
                    isFocused: cell.identifier == focusedIdentifier
                )
            },
            propertyLabels: renderPropertyLabels(for: cells),
            centerIconName: centerDefinition?.iconName,
            centerShortcut: centerDefinition?.displayKeys ?? "",
            centerColor: centerColor(),
            centerStrokeWidth: centerStrokeWidth(),
            centerIsHovered: hoveredIdentifier == "palette.center",
            centerIsFocused: focusedIdentifier == "palette.center"
        )
    }

    private func renderPropertyLabels(
        for cells: [RadialPaletteCell]
    ) -> [RadialPaletteRenderPropertyLabel] {
        var groups: [(
            name: String,
            start: CGFloat,
            end: CGFloat,
            firstCellStart: CGFloat,
            firstCellEnd: CGFloat
        )] = []
        for cell in cells where cell.ring == .option {
            guard let propertyName = cell.propertyName else { continue }
            if let last = groups.last, last.name == propertyName {
                groups[groups.count - 1].end = cell.endAngle
            } else {
                groups.append((
                    propertyName,
                    cell.startAngle,
                    cell.endAngle,
                    cell.startAngle,
                    cell.endAngle
                ))
            }
        }

        let font = NSFont.systemFont(
            ofSize: RadialPaletteMetrics.propertyLabelFontSize,
            weight: .semibold
        )
        return groups.enumerated().map { index, group in
            let textWidth = ceil(
                (group.name as NSString).size(withAttributes: [.font: font]).width
            )
            let labelRadius = RadialPaletteMetrics.optionInnerRadius + 7
            let availableArcLength = (group.end - group.start) * labelRadius
                - 2 * RadialPaletteMetrics.propertyLabelPadding
            let fitsGroupArc = textWidth <= availableArcLength
            return RadialPaletteRenderPropertyLabel(
                id: "\(index).\(group.name)",
                name: group.name,
                angle: fitsGroupArc
                    ? (group.start + group.end) / 2
                    : (group.firstCellStart + group.firstCellEnd) / 2,
                radius: fitsGroupArc
                    ? labelRadius
                    : (RadialPaletteMetrics.optionInnerRadius
                        + RadialPaletteMetrics.optionOuterRadius) / 2
            )
        }
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
