import AppKit

private final class PalettePanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

private final class PaletteDragHandle: NSView {
    override var mouseDownCanMoveWindow: Bool { true }

    init() {
        super.init(frame: .zero)
        let label = NSTextField(labelWithString: "⠿")
        label.textColor = .secondaryLabelColor
        label.font = .systemFont(ofSize: 16, weight: .semibold)
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            widthAnchor.constraint(equalToConstant: 28),
            heightAnchor.constraint(equalToConstant: 28),
            label.centerXAnchor.constraint(equalTo: centerXAnchor),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        setAccessibilityLabel("도구 팔레트 이동")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }
}

final class PaletteController: NSObject {
    static let windowLevel = AppWindowLevel.tool

    private enum Orientation: String {
        case horizontal
        case vertical
    }

    private enum DefaultsKey {
        static let orientation = "EduTools.palette.orientation"
        static let positions = "EduTools.palette.positions"
        static let displayUUID = "EduTools.palette.displayUUID"
    }

    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let settingsStore: SettingsStore
    private let defaults: UserDefaults
    private let panel: PalettePanel
    private let rootView = NSVisualEffectView()
    private let controlsStack = NSStackView()
    private let eraserModeLabel = NSTextField(labelWithString: "")
    private let widthLabel = NSTextField(labelWithString: "")
    private var recognitionButton: NSButton?
    private var eraserAreaShapeButton: NSButton?
    private var toolButtons: [InkTool: NSButton] = [:]
    private var boardButtons: [BoardKind: NSButton] = [:]
    private var colorButtons: [NSButton] = []
    private var definitionByButtonIdentifier: [String: ShortcutDefinition] = [:]
    private var annotationObserverIdentifier: UUID?
    private var settingsObserverIdentifier: UUID?
    private var visibilityObserverIdentifier: UUID?
    private var boardObserverIdentifier: UUID?
    private var paletteStyleObserverIdentifier: UUID?
    private var notificationObservers: [NSObjectProtocol] = []
    private var orientation: Orientation
    private var isApplyingFrame = false
    private var currentDisplayUUID: String?

    init(
        state: AppState,
        dispatcher: AppCommandDispatcher,
        settingsStore: SettingsStore,
        defaults: UserDefaults = .standard
    ) {
        self.state = state
        self.dispatcher = dispatcher
        self.settingsStore = settingsStore
        self.defaults = defaults
        orientation = Orientation(rawValue: defaults.string(forKey: DefaultsKey.orientation) ?? "")
            ?? .horizontal
        panel = PalettePanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        super.init()

        configurePanel()
        buildControls()
        applyOrientation(restoringPosition: false)
        restorePosition()
        observeStateAndScreens()
    }

    private func configurePanel() {
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isMovable = true
        panel.isMovableByWindowBackground = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = Self.windowLevel
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = .none

        rootView.material = .hudWindow
        rootView.blendingMode = .withinWindow
        rootView.state = .active
        rootView.wantsLayer = true
        rootView.layer?.cornerRadius = 12
        rootView.layer?.masksToBounds = true
        panel.contentView = rootView

        let menu = NSMenu()
        let orientationItem = NSMenuItem(
            title: "세로/가로 전환",
            action: #selector(toggleOrientation(_:)),
            keyEquivalent: ""
        )
        orientationItem.target = self
        menu.addItem(orientationItem)
        rootView.menu = menu
    }

    private func buildControls() {
        controlsStack.spacing = 6
        controlsStack.translatesAutoresizingMaskIntoConstraints = false
        rootView.addSubview(controlsStack)
        NSLayoutConstraint.activate([
            controlsStack.leadingAnchor.constraint(equalTo: rootView.leadingAnchor, constant: 8),
            controlsStack.trailingAnchor.constraint(equalTo: rootView.trailingAnchor, constant: -8),
            controlsStack.topAnchor.constraint(equalTo: rootView.topAnchor, constant: 8),
            controlsStack.bottomAnchor.constraint(equalTo: rootView.bottomAnchor, constant: -8),
        ])

        controlsStack.addArrangedSubview(PaletteDragHandle())
        addToolButton(command: .selectTool(.pen), tool: .pen)
        addToolButton(command: .selectTool(.highlighter), tool: .highlighter)
        addToolButton(command: .useLaser, tool: .laser)
        addToolButton(command: .selectOrCycleEraser, tool: .eraser)
        addToolButton(command: .selectTool(.arrow), tool: .arrow)
        addToolButton(command: .selectTool(.rectangle), tool: .rectangle)
        addToolButton(command: .selectTool(.ellipse), tool: .ellipse)
        addToolButton(command: .selectTool(.text), tool: .text)
        addToolButton(command: .selectTool(.select), tool: .select)
        addToolButton(command: .selectTool(.cover), tool: .cover)

        let recognitionButton = makeDefinitionButton(command: .toggleShapeRecognition)
        self.recognitionButton = recognitionButton
        controlsStack.addArrangedSubview(recognitionButton)

        eraserModeLabel.font = .systemFont(ofSize: 10, weight: .medium)
        eraserModeLabel.textColor = .secondaryLabelColor
        eraserModeLabel.alignment = .center
        eraserModeLabel.setContentCompressionResistancePriority(.required, for: .horizontal)
        controlsStack.addArrangedSubview(eraserModeLabel)

        let eraserAreaShapeButton = makeDefinitionButton(command: .toggleEraserAreaShape)
        self.eraserAreaShapeButton = eraserAreaShapeButton
        controlsStack.addArrangedSubview(eraserAreaShapeButton)

        addBoardButton(command: .toggleBoard(.white), kind: .white)
        addBoardButton(command: .toggleBoard(.black), kind: .black)

        for index in state.colorPresets.indices {
            let button = makeColorButton(index: index)
            colorButtons.append(button)
            controlsStack.addArrangedSubview(button)
        }

        controlsStack.addArrangedSubview(makeDefinitionButton(command: .adjustWidth(-1)))
        widthLabel.font = .monospacedDigitSystemFont(ofSize: 11, weight: .semibold)
        widthLabel.alignment = .center
        widthLabel.setContentHuggingPriority(.required, for: .horizontal)
        widthLabel.widthAnchor.constraint(greaterThanOrEqualToConstant: 28).isActive = true
        controlsStack.addArrangedSubview(widthLabel)
        controlsStack.addArrangedSubview(makeDefinitionButton(command: .adjustWidth(1)))
        controlsStack.addArrangedSubview(makeDefinitionButton(command: .undo))
        controlsStack.addArrangedSubview(makeDefinitionButton(command: .redo))
        controlsStack.addArrangedSubview(makeDefinitionButton(command: .clearAll))
        controlsStack.addArrangedSubview(makeDefinitionButton(command: .showHelp))
    }

    private func addToolButton(command: AppCommand, tool: InkTool) {
        let button = makeDefinitionButton(command: command)
        toolButtons[tool] = button
        controlsStack.addArrangedSubview(button)
    }

    private func addBoardButton(command: AppCommand, kind: BoardKind) {
        let button = makeDefinitionButton(command: command)
        boardButtons[kind] = button
        controlsStack.addArrangedSubview(button)
    }

    private func makeDefinitionButton(command: AppCommand) -> NSButton {
        guard let definition = ShortcutCatalog.definition(for: command) else {
            preconditionFailure("Palette command is missing from ShortcutCatalog: \(command)")
        }
        let button = NSButton(title: "", target: self, action: #selector(performDefinition(_:)))
        button.identifier = NSUserInterfaceItemIdentifier(definition.id)
        button.image = NSImage(
            systemSymbolName: definition.iconName,
            accessibilityDescription: definition.name
        )
        button.imagePosition = .imageOnly
        button.imageScaling = .scaleProportionallyDown
        button.bezelStyle = .texturedRounded
        button.toolTip = definition.displayKeys.isEmpty
            ? definition.name
            : "\(definition.name) (\(definition.displayKeys))"
        button.setAccessibilityLabel(definition.name)
        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: 32),
            button.heightAnchor.constraint(equalToConstant: 32),
        ])
        definitionByButtonIdentifier[definition.id] = definition
        return button
    }

    private func makeColorButton(index: Int) -> NSButton {
        let command = AppCommand.selectColor(index)
        guard let definition = ShortcutCatalog.definition(for: command) else {
            preconditionFailure("Color command is missing from ShortcutCatalog: \(index)")
        }
        let button = NSButton(title: "", target: self, action: #selector(performDefinition(_:)))
        button.identifier = NSUserInterfaceItemIdentifier(definition.id)
        button.isBordered = false
        button.wantsLayer = true
        button.layer?.backgroundColor = state.colorPresets[index].cgColor
        button.layer?.cornerRadius = 9
        button.toolTip = "\(definition.name) (\(definition.displayKeys))"
        button.setAccessibilityLabel(definition.name)
        NSLayoutConstraint.activate([
            button.widthAnchor.constraint(equalToConstant: 18),
            button.heightAnchor.constraint(equalToConstant: 18),
        ])
        definitionByButtonIdentifier[definition.id] = definition
        return button
    }

    private func observeStateAndScreens() {
        annotationObserverIdentifier = state.addAnnotationModeObserver { [weak self] _ in
            self?.updateVisibility()
        }
        settingsObserverIdentifier = state.addToolSettingsObserver { [weak self] in
            self?.updateSelection()
        }
        visibilityObserverIdentifier = state.addPaletteVisibilityObserver { [weak self] _ in
            self?.updateVisibility()
        }
        boardObserverIdentifier = state.addBoardObserver { [weak self] in
            self?.updateBoardSelection()
        }
        paletteStyleObserverIdentifier = settingsStore.addObserver { [weak self] _ in
            self?.updateVisibility()
        }

        notificationObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSWindow.didMoveNotification,
                object: panel,
                queue: .main
            ) { [weak self] _ in
                self?.saveCurrentPosition()
            }
        )
        notificationObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                self?.screenConfigurationDidChange()
            }
        )
    }

    private func updateVisibility() {
        if state.isAnnotationModeEnabled,
           state.isPaletteVisible,
           settingsStore.settings.tools.paletteStyle == .linear
        {
            ensureCurrentScreenExists()
            panel.orderFrontRegardless()
        } else {
            panel.orderOut(nil)
        }
    }

    private func updateSelection() {
        let shortcutButtons = Array(toolButtons.values) + colorButtons + Array(boardButtons.values)
        for button in shortcutButtons {
            guard let identifier = button.identifier?.rawValue,
                  let definition = ShortcutCatalog.activeDefinitions.first(where: {
                      $0.id == identifier
                  })
            else {
                continue
            }
            button.toolTip = definition.displayKeys.isEmpty
                ? definition.name
                : "\(definition.name) (\(definition.displayKeys))"
        }
        for (tool, button) in toolButtons {
            let isSelected = tool == state.currentTool
            button.state = isSelected ? .on : .off
            button.contentTintColor = isSelected ? .controlAccentColor : .labelColor
        }
        for (index, button) in colorButtons.enumerated() {
            if state.colorPresets.indices.contains(index) {
                button.layer?.backgroundColor = state.colorPresets[index].cgColor
            }
            let isSelected = index == state.selectedColorIndex
            button.layer?.borderWidth = isSelected ? 3 : 1
            button.layer?.borderColor = isSelected
                ? NSColor.controlAccentColor.cgColor
                : NSColor.separatorColor.cgColor
        }
        eraserModeLabel.stringValue = state.eraserMode.displayName
        recognitionButton?.state = state.isShapeRecognitionEnabled ? .on : .off
        recognitionButton?.contentTintColor = state.isShapeRecognitionEnabled
            ? .controlAccentColor
            : .labelColor
        recognitionButton?.toolTip = "손그림 도형 자동 보정: \(state.isShapeRecognitionEnabled ? "켬" : "끔")"
        eraserAreaShapeButton?.title = state.eraserAreaShape == .rectangle ? "□" : "⌁"
        eraserAreaShapeButton?.image = nil
        eraserAreaShapeButton?.imagePosition = .noImage
        eraserAreaShapeButton?.toolTip = "영역 지우개: \(state.eraserAreaShape.displayName)"
        switch state.currentTool {
        case .pen:
            widthLabel.stringValue = formattedSize(state.penWidth)
        case .highlighter:
            widthLabel.stringValue = formattedSize(state.highlighterWidth)
        case .laser:
            widthLabel.stringValue = "—"
        case .eraser:
            widthLabel.stringValue = formattedSize(state.eraserDiameter)
        case .arrow, .rectangle, .ellipse:
            widthLabel.stringValue = formattedSize(state.penWidth)
        case .text:
            widthLabel.stringValue = formattedSize(state.textFontSize)
        case .select, .cover:
            widthLabel.stringValue = "—"
        }
    }

    private func updateBoardSelection() {
        for (kind, button) in boardButtons {
            let isSelected = state.presentedBoard?.kind == kind
            button.state = isSelected ? .on : .off
            button.contentTintColor = isSelected ? .controlAccentColor : .labelColor
        }
    }

    private func formattedSize(_ value: CGFloat) -> String {
        value.rounded() == value ? String(Int(value)) : String(format: "%.1f", value)
    }

    @objc private func performDefinition(_ sender: NSButton) {
        guard let identifier = sender.identifier?.rawValue,
              let definition = definitionByButtonIdentifier[identifier]
        else {
            NSLog("EduTools: palette ignored a button without a shortcut definition")
            return
        }
        dispatcher.execute(definition.command, scope: .annotation)
    }

    @objc private func toggleOrientation(_ sender: Any?) {
        orientation = orientation == .horizontal ? .vertical : .horizontal
        defaults.set(orientation.rawValue, forKey: DefaultsKey.orientation)
        applyOrientation(restoringPosition: true)
    }

    private func applyOrientation(restoringPosition: Bool) {
        controlsStack.orientation = orientation == .horizontal ? .horizontal : .vertical
        controlsStack.alignment = orientation == .horizontal ? .centerY : .centerX
        rootView.layoutSubtreeIfNeeded()
        let fittingSize = rootView.fittingSize
        panel.setContentSize(
            NSSize(width: ceil(fittingSize.width), height: ceil(fittingSize.height))
        )
        if restoringPosition {
            restorePosition()
        }
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
            NSLog("EduTools: palette could not restore its position because no screen is available")
            return
        }
        placePanel(on: targetScreen, identifier: currentDisplayUUID)
    }

    private func placePanel(on screen: NSScreen, identifier: String?) {
        let visibleFrame = screen.visibleFrame
        let availableWidth = max(0, visibleFrame.width - panel.frame.width)
        let availableHeight = max(0, visibleFrame.height - panel.frame.height)
        let saved = savedRatio(for: identifier)
        let xRatio = min(max(saved?.x ?? 0.5, 0), 1)
        let yRatio = min(max(saved?.y ?? 0.85, 0), 1)
        let origin = CGPoint(
            x: visibleFrame.minX + availableWidth * xRatio,
            y: visibleFrame.minY + availableHeight * yRatio
        )
        isApplyingFrame = true
        panel.setFrameOrigin(origin)
        isApplyingFrame = false
        currentDisplayUUID = identifier
        if let identifier {
            defaults.set(identifier, forKey: DefaultsKey.displayUUID)
        }
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
        guard !isApplyingFrame,
              let screen = screenContainingPanel(),
              let identifier = displayUUID(for: screen)
        else {
            return
        }
        let visibleFrame = screen.visibleFrame
        let availableWidth = max(1, visibleFrame.width - panel.frame.width)
        let availableHeight = max(1, visibleFrame.height - panel.frame.height)
        let xRatio = min(max((panel.frame.minX - visibleFrame.minX) / availableWidth, 0), 1)
        let yRatio = min(max((panel.frame.minY - visibleFrame.minY) / availableHeight, 0), 1)
        var positions = defaults.dictionary(forKey: DefaultsKey.positions) ?? [:]
        positions[identifier] = ["x": xRatio, "y": yRatio]
        defaults.set(positions, forKey: DefaultsKey.positions)
        defaults.set(identifier, forKey: DefaultsKey.displayUUID)
        currentDisplayUUID = identifier
    }

    private func screenContainingPanel() -> NSScreen? {
        if let screen = panel.screen {
            return screen
        }
        let center = CGPoint(x: panel.frame.midX, y: panel.frame.midY)
        return NSScreen.screens.first { $0.frame.contains(center) }
    }

    private func screenConfigurationDidChange() {
        ensureCurrentScreenExists()
        updateVisibility()
    }

    private func ensureCurrentScreenExists() {
        guard let currentDisplayUUID,
              NSScreen.screens.contains(where: { displayUUID(for: $0) == currentDisplayUUID })
        else {
            guard let primaryScreen = NSScreen.screens.first else {
                NSLog("EduTools: palette could not move after a display change because no screen is available")
                return
            }
            let identifier = displayUUID(for: primaryScreen)
            placePanel(on: primaryScreen, identifier: identifier)
            return
        }

        if let currentScreen = NSScreen.screens.first(where: {
            displayUUID(for: $0) == currentDisplayUUID
        }) {
            placePanel(on: currentScreen, identifier: currentDisplayUUID)
        }
    }

    deinit {
        if let annotationObserverIdentifier {
            state.removeAnnotationModeObserver(annotationObserverIdentifier)
        }
        if let settingsObserverIdentifier {
            state.removeToolSettingsObserver(settingsObserverIdentifier)
        }
        if let visibilityObserverIdentifier {
            state.removePaletteVisibilityObserver(visibilityObserverIdentifier)
        }
        if let boardObserverIdentifier {
            state.removeBoardObserver(boardObserverIdentifier)
        }
        if let paletteStyleObserverIdentifier {
            settingsStore.removeObserver(paletteStyleObserverIdentifier)
        }
        for observer in notificationObservers {
            NotificationCenter.default.removeObserver(observer)
        }
        panel.close()
    }
}
