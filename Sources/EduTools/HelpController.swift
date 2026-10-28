import AppKit

private final class HelpPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private final class HelpRootView: NSVisualEffectView {
    var contentFrame: (() -> CGRect)?
    var backgroundClickHandler: (() -> Void)?
    var keyDownHandler: ((NSEvent) -> Bool)?
    var keyUpHandler: ((NSEvent) -> Bool)?

    override var acceptsFirstResponder: Bool { true }

    override func mouseDown(with event: NSEvent) {
        let location = convert(event.locationInWindow, from: nil)
        if contentFrame?().contains(location) != true {
            backgroundClickHandler?()
            return
        }
        super.mouseDown(with: event)
    }

    override func keyDown(with event: NSEvent) {
        if keyDownHandler?(event) == true { return }
        super.keyDown(with: event)
    }

    override func keyUp(with event: NSEvent) {
        if keyUpHandler?(event) == true { return }
        super.keyUp(with: event)
    }
}

private final class HelpSearchField: NSSearchField {
    var keyDownHandler: ((NSEvent) -> Bool)?

    override func keyDown(with event: NSEvent) {
        if keyDownHandler?(event) == true { return }
        super.keyDown(with: event)
    }
}

private final class HelpCardsDocumentView: NSView {
    override var isFlipped: Bool { true }
}

private final class HelpItemButton: NSButton {
    init(
        definition: ShortcutDefinition,
        displayName: String,
        isHighlighted: Bool,
        target: AnyObject,
        action: Selector
    ) {
        super.init(frame: .zero)
        self.target = target
        self.action = action
        identifier = NSUserInterfaceItemIdentifier(definition.id)
        title = ""
        isBordered = false
        wantsLayer = true
        layer?.cornerRadius = 7
        layer?.backgroundColor = isHighlighted
            ? NSColor.controlAccentColor.withAlphaComponent(0.28).cgColor
            : NSColor.clear.cgColor
        setAccessibilityLabel("\(displayName), \(definition.displayKeys)")

        let iconView = NSImageView(
            image: NSImage(
                systemSymbolName: definition.iconName,
                accessibilityDescription: displayName
            ) ?? NSImage()
        )
        iconView.contentTintColor = isHighlighted ? .controlAccentColor : .labelColor
        iconView.translatesAutoresizingMaskIntoConstraints = false

        let nameLabel = NSTextField(labelWithString: displayName)
        nameLabel.font = .systemFont(ofSize: 13, weight: isHighlighted ? .semibold : .regular)
        nameLabel.lineBreakMode = .byTruncatingTail
        nameLabel.translatesAutoresizingMaskIntoConstraints = false

        let keyLabel = NSTextField(labelWithString: definition.displayKeys)
        keyLabel.font = .monospacedSystemFont(ofSize: 12, weight: .medium)
        keyLabel.alignment = .right
        keyLabel.textColor = .secondaryLabelColor
        keyLabel.setContentCompressionResistancePriority(.required, for: .horizontal)
        keyLabel.translatesAutoresizingMaskIntoConstraints = false

        addSubview(iconView)
        addSubview(nameLabel)
        addSubview(keyLabel)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 30),
            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 17),
            iconView.heightAnchor.constraint(equalToConstant: 17),
            nameLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: 7),
            nameLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            keyLabel.leadingAnchor.constraint(greaterThanOrEqualTo: nameLabel.trailingAnchor, constant: 8),
            keyLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            keyLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }
}

final class HelpController: NSObject {
    private enum DefaultsKey {
        static let didShowFirstRunHelp = "EduTools.help.didShowFirstRun"
    }

    private enum Layout {
        static let cardMinimumWidth: CGFloat = 320
        static let cardSpacing: CGFloat = 10
        static let contentHorizontalInset: CGFloat = 18
        static let contentTopInset: CGFloat = 18
        static let contentBottomInset: CGFloat = 18
        static let headerCardsSpacing: CGFloat = 14
        static let visibleHorizontalMargin: CGFloat = 40
        static let visibleVerticalMargin: CGFloat = 30
        static let initialContentHeight: CGFloat = 120
        static let columnCandidates = [4, 2, 1]
    }

    private struct LayoutMetrics {
        let columnCount: Int
        let contentWidth: CGFloat
        let maximumContentHeight: CGFloat
        let centerOffset: CGPoint
    }

    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    private let defaults: UserDefaults
    private let panel: HelpPanel
    private let rootView = HelpRootView()
    private let contentView = NSVisualEffectView()
    private let titleLabel = NSTextField(labelWithString: "")
    private let searchField = HelpSearchField()
    private let headerStack = NSStackView()
    private let cardsScrollView = NSScrollView()
    private let cardsDocumentView = HelpCardsDocumentView()
    private let cardsStack = NSStackView()
    private var contentCenterXConstraint: NSLayoutConstraint!
    private var contentCenterYConstraint: NSLayoutConstraint!
    private var contentWidthConstraint: NSLayoutConstraint!
    private var contentHeightConstraint: NSLayoutConstraint!
    private var definitionByIdentifier: [String: ShortcutDefinition] = [:]
    private var toolSettingsObserverIdentifier: UUID?
    private var annotationObserverIdentifier: UUID?
    private var zoomObserverIdentifier: UUID?
    private var sharingSettingsObserverIdentifier: UUID?
    private var screenObserver: NSObjectProtocol?
    private var pressedAt: TimeInterval?
    private var activeShortcutKeyCode: UInt16?
    private var ignoresNextRelease = false
    private var isPinned = false
    private var previousApplication: NSRunningApplication?

    private(set) var isVisible = false

    init(
        state: AppState,
        dispatcher: AppCommandDispatcher,
        defaults: UserDefaults = .standard
    ) {
        self.state = state
        self.dispatcher = dispatcher
        self.defaults = defaults
        let initialScreen = screenUnderMouse() ?? NSScreen.screens.first
        panel = HelpPanel(
            contentRect: initialScreen?.frame ?? .zero,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false,
            screen: initialScreen
        )
        super.init()
        configurePanel()
        buildLayout()
        observeStateAndScreens()
    }

    func showFirstRunIfNeeded() {
        guard !defaults.bool(forKey: DefaultsKey.didShowFirstRunHelp) else { return }
        defaults.set(true, forKey: DefaultsKey.didShowFirstRunHelp)
        showPinned()
    }

    func showPinned() {
        if isVisible {
            close()
            return
        }
        pressedAt = nil
        isPinned = true
        show()
    }

    func shortcutDidPress(keyCode: UInt16? = nil) {
        activeShortcutKeyCode = keyCode
        if isVisible, isPinned {
            ignoresNextRelease = true
            close()
            return
        }
        guard pressedAt == nil else { return }
        pressedAt = ProcessInfo.processInfo.systemUptime
        isPinned = false
        show()
    }

    func shortcutDidRelease() {
        activeShortcutKeyCode = nil
        if ignoresNextRelease {
            ignoresNextRelease = false
            return
        }
        guard let pressedAt else { return }
        self.pressedAt = nil
        let heldDuration = ProcessInfo.processInfo.systemUptime - pressedAt
        if heldDuration >= 0.3 {
            close()
        } else {
            isPinned = true
            updateContent()
            panel.makeFirstResponder(searchField)
        }
    }

    func close() {
        guard isVisible else { return }
        isVisible = false
        isPinned = false
        pressedAt = nil
        searchField.stringValue = ""
        panel.orderOut(nil)
        definitionByIdentifier.removeAll()

        if let previousApplication, !state.isAnnotationModeEnabled, !state.isZoomEnabled {
            let activated = previousApplication.activate(from: .current, options: [])
            if !activated {
                NSLog(
                    "EduTools: failed to return focus after help to process %d",
                    previousApplication.processIdentifier
                )
            }
        }
        previousApplication = nil
    }

    private func configurePanel() {
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = AppWindowLevel.guidance
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.sharingType = state.showsHelpInSharing ? .readOnly : .none

        rootView.material = .hudWindow
        rootView.blendingMode = .behindWindow
        rootView.state = .active
        rootView.wantsLayer = true
        rootView.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.42).cgColor
        rootView.autoresizingMask = [.width, .height]
        rootView.backgroundClickHandler = { [weak self] in
            guard self?.isPinned == true else { return }
            self?.close()
        }
        rootView.keyDownHandler = { [weak self] event in
            self?.handleKeyDown(event) ?? false
        }
        rootView.keyUpHandler = { [weak self] event in
            self?.handleKeyUp(event) ?? false
        }
        panel.contentView = rootView
    }

    private func buildLayout() {
        contentView.material = .popover
        contentView.blendingMode = .withinWindow
        contentView.state = .active
        contentView.wantsLayer = true
        contentView.layer?.cornerRadius = 18
        contentView.layer?.masksToBounds = true
        contentView.translatesAutoresizingMaskIntoConstraints = false
        rootView.addSubview(contentView)

        headerStack.orientation = .vertical
        headerStack.alignment = .leading
        headerStack.spacing = 8
        headerStack.translatesAutoresizingMaskIntoConstraints = false

        titleLabel.font = .systemFont(ofSize: 24, weight: .bold)
        headerStack.addArrangedSubview(titleLabel)

        searchField.placeholderString = "이름 또는 키로 찾기"
        searchField.sendsSearchStringImmediately = true
        searchField.sendsWholeSearchString = false
        searchField.target = self
        searchField.action = #selector(searchDidChange(_:))
        searchField.delegate = self
        searchField.keyDownHandler = { [weak self] event in
            self?.handleKeyDown(event) ?? false
        }
        searchField.widthAnchor.constraint(equalToConstant: 300).isActive = true
        headerStack.addArrangedSubview(searchField)

        cardsStack.orientation = .vertical
        cardsStack.alignment = .leading
        cardsStack.spacing = Layout.cardSpacing
        cardsStack.translatesAutoresizingMaskIntoConstraints = false

        cardsScrollView.drawsBackground = false
        cardsScrollView.borderType = .noBorder
        cardsScrollView.hasHorizontalScroller = false
        cardsScrollView.hasVerticalScroller = true
        cardsScrollView.autohidesScrollers = true
        cardsScrollView.scrollerStyle = .overlay
        cardsScrollView.verticalScrollElasticity = .none
        cardsScrollView.translatesAutoresizingMaskIntoConstraints = false

        cardsDocumentView.translatesAutoresizingMaskIntoConstraints = false
        cardsDocumentView.addSubview(cardsStack)
        cardsScrollView.documentView = cardsDocumentView

        contentView.addSubview(headerStack)
        contentView.addSubview(cardsScrollView)
        contentCenterXConstraint = contentView.centerXAnchor.constraint(
            equalTo: rootView.centerXAnchor
        )
        contentCenterYConstraint = contentView.centerYAnchor.constraint(
            equalTo: rootView.centerYAnchor
        )
        contentWidthConstraint = contentView.widthAnchor.constraint(
            equalToConstant: contentWidth(forColumnCount: 1)
        )
        contentHeightConstraint = contentView.heightAnchor.constraint(
            equalToConstant: Layout.initialContentHeight
        )
        NSLayoutConstraint.activate([
            contentCenterXConstraint,
            contentCenterYConstraint,
            contentWidthConstraint,
            contentHeightConstraint,
            headerStack.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 22),
            headerStack.trailingAnchor.constraint(lessThanOrEqualTo: contentView.trailingAnchor, constant: -22),
            headerStack.topAnchor.constraint(
                equalTo: contentView.topAnchor,
                constant: Layout.contentTopInset
            ),
            cardsScrollView.leadingAnchor.constraint(
                equalTo: contentView.leadingAnchor,
                constant: Layout.contentHorizontalInset
            ),
            cardsScrollView.trailingAnchor.constraint(
                equalTo: contentView.trailingAnchor,
                constant: -Layout.contentHorizontalInset
            ),
            cardsScrollView.topAnchor.constraint(
                equalTo: headerStack.bottomAnchor,
                constant: Layout.headerCardsSpacing
            ),
            cardsScrollView.bottomAnchor.constraint(
                equalTo: contentView.bottomAnchor,
                constant: -Layout.contentBottomInset
            ),
            cardsDocumentView.leadingAnchor.constraint(
                equalTo: cardsScrollView.contentView.leadingAnchor
            ),
            cardsDocumentView.trailingAnchor.constraint(
                equalTo: cardsScrollView.contentView.trailingAnchor
            ),
            cardsDocumentView.topAnchor.constraint(
                equalTo: cardsScrollView.contentView.topAnchor
            ),
            cardsDocumentView.widthAnchor.constraint(
                equalTo: cardsScrollView.contentView.widthAnchor
            ),
            cardsStack.leadingAnchor.constraint(equalTo: cardsDocumentView.leadingAnchor),
            cardsStack.trailingAnchor.constraint(equalTo: cardsDocumentView.trailingAnchor),
            cardsStack.topAnchor.constraint(equalTo: cardsDocumentView.topAnchor),
            cardsStack.bottomAnchor.constraint(equalTo: cardsDocumentView.bottomAnchor),
        ])
        rootView.contentFrame = { [weak self] in
            self?.contentView.frame ?? .zero
        }
    }

    private func observeStateAndScreens() {
        sharingSettingsObserverIdentifier = state.addSharingSettingsObserver { [weak self] isVisible in
            self?.panel.sharingType = isVisible ? .readOnly : .none
        }
        toolSettingsObserverIdentifier = state.addToolSettingsObserver { [weak self] in
            guard self?.isVisible == true else { return }
            self?.updateContent()
        }
        annotationObserverIdentifier = state.addAnnotationModeObserver { [weak self] _ in
            guard self?.isVisible == true else { return }
            self?.updateContent()
        }
        zoomObserverIdentifier = state.addZoomObserver { [weak self] _ in
            guard self?.isVisible == true else { return }
            self?.updateContent()
        }
        screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            guard self?.isVisible == true else { return }
            self?.moveToMouseScreen()
        }
    }

    private func show() {
        moveToMouseScreen()
        if !state.isAnnotationModeEnabled, !state.isZoomEnabled {
            let frontmostApplication = NSWorkspace.shared.frontmostApplication
            if frontmostApplication?.processIdentifier != ProcessInfo.processInfo.processIdentifier {
                previousApplication = frontmostApplication
            }
        }
        isVisible = true
        updateContent()
        NSApp.activate()
        panel.makeKeyAndOrderFront(nil)
        panel.makeFirstResponder(isPinned ? searchField : rootView)
    }

    private func moveToMouseScreen() {
        guard let targetScreen = screenUnderMouse() ?? NSScreen.screens.first else {
            NSLog("EduTools: help could not find a screen")
            return
        }
        panel.setFrame(targetScreen.frame, display: true)
        if isVisible {
            updateContent()
        }
    }

    private func updateContent() {
        if state.isZoomEnabled {
            titleLabel.stringValue = "줌 모드 단축키"
        } else if state.isAnnotationModeEnabled {
            titleLabel.stringValue = "판서 모드 단축키"
        } else {
            titleLabel.stringValue = "전역 단축키"
        }
        searchField.isHidden = !isPinned
        definitionByIdentifier.removeAll()
        for view in cardsStack.arrangedSubviews {
            cardsStack.removeArrangedSubview(view)
            view.removeFromSuperview()
        }

        let activeMode: AppMode = state.isZoomEnabled
            ? .zoom
            : (state.isAnnotationModeEnabled ? .annotation : .global)
        let filteredDefinitions = ShortcutCatalog.filtered(
            ShortcutCatalog.activeDefinitions,
            query: isPinned ? searchField.stringValue : ""
        ).enumerated().sorted { lhs, rhs in
            let lhsIsCurrent = lhs.element.modes.contains(activeMode)
            let rhsIsCurrent = rhs.element.modes.contains(activeMode)
            if lhsIsCurrent != rhsIsCurrent {
                return lhsIsCurrent && !rhsIsCurrent
            }
            return lhs.offset < rhs.offset
        }.map(\.element)
        guard let screen = panel.screen ?? screenUnderMouse() ?? NSScreen.screens.first else {
            NSLog("EduTools: help could not lay out content without a screen")
            return
        }
        let metrics = layoutMetrics(for: screen)
        let groups = orderedGroups()
        for rowStart in stride(from: 0, to: groups.count, by: metrics.columnCount) {
            let row = NSStackView()
            row.orientation = .horizontal
            row.alignment = .top
            row.distribution = .fillEqually
            row.spacing = Layout.cardSpacing
            for group in groups[rowStart..<min(rowStart + metrics.columnCount, groups.count)] {
                let definitions = filteredDefinitions.filter { $0.group == group }
                row.addArrangedSubview(makeCard(group: group, definitions: definitions))
            }
            addFullWidthArrangedSubview(row, to: cardsStack)
        }
        apply(metrics)
    }

    private func contentWidth(forColumnCount columnCount: Int) -> CGFloat {
        (CGFloat(columnCount) * Layout.cardMinimumWidth)
            + (CGFloat(columnCount - 1) * Layout.cardSpacing)
            + (2 * Layout.contentHorizontalInset)
    }

    private func layoutMetrics(for screen: NSScreen) -> LayoutMetrics {
        let visibleFrame = screen.visibleFrame.offsetBy(
            dx: -screen.frame.minX,
            dy: -screen.frame.minY
        )
        let maximumContentWidth = max(
            1,
            visibleFrame.width - (2 * Layout.visibleHorizontalMargin)
        )
        let columnCount = Layout.columnCandidates.first {
            contentWidth(forColumnCount: $0) <= maximumContentWidth
        } ?? 1
        let maximumContentHeight = max(
            1,
            visibleFrame.height - (2 * Layout.visibleVerticalMargin)
        )
        let panelBounds = CGRect(origin: .zero, size: screen.frame.size)
        return LayoutMetrics(
            columnCount: columnCount,
            contentWidth: min(contentWidth(forColumnCount: columnCount), maximumContentWidth),
            maximumContentHeight: maximumContentHeight,
            centerOffset: CGPoint(
                x: visibleFrame.midX - panelBounds.midX,
                y: panelBounds.midY - visibleFrame.midY
            )
        )
    }

    private func addFullWidthArrangedSubview(_ view: NSView, to stack: NSStackView) {
        stack.addArrangedSubview(view)
        view.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    }

    private func apply(_ metrics: LayoutMetrics) {
        contentCenterXConstraint.constant = metrics.centerOffset.x
        contentCenterYConstraint.constant = metrics.centerOffset.y
        contentWidthConstraint.constant = metrics.contentWidth

        rootView.layoutSubtreeIfNeeded()
        let preferredHeight = Layout.contentTopInset
            + headerStack.fittingSize.height
            + Layout.headerCardsSpacing
            + cardsStack.fittingSize.height
            + Layout.contentBottomInset
        contentHeightConstraint.constant = min(preferredHeight, metrics.maximumContentHeight)
        rootView.layoutSubtreeIfNeeded()
        cardsScrollView.reflectScrolledClipView(cardsScrollView.contentView)
    }

    private func orderedGroups() -> [ShortcutGroup] {
        if state.isZoomEnabled {
            return [.screenBoardZoom, .tools, .editing, .timerAndOther]
        }
        if state.isAnnotationModeEnabled {
            return [.tools, .editing, .screenBoardZoom, .timerAndOther]
        }
        return [.screenBoardZoom, .timerAndOther, .tools, .editing]
    }

    private func makeCard(
        group: ShortcutGroup,
        definitions: [ShortcutDefinition]
    ) -> NSView {
        let card = NSView()
        card.wantsLayer = true
        card.layer?.backgroundColor = NSColor.windowBackgroundColor
            .withAlphaComponent(0.66).cgColor
        card.layer?.cornerRadius = 10

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 2
        stack.translatesAutoresizingMaskIntoConstraints = false

        let heading = NSTextField(labelWithString: group.displayName)
        heading.font = .systemFont(ofSize: 14, weight: .bold)
        addFullWidthArrangedSubview(heading, to: stack)

        if definitions.isEmpty {
            let emptyLabel = NSTextField(labelWithString: "일치하는 항목 없음")
            emptyLabel.textColor = .tertiaryLabelColor
            emptyLabel.font = .systemFont(ofSize: 12)
            addFullWidthArrangedSubview(emptyLabel, to: stack)
        } else {
            for definition in definitions {
                let displayName = contextualName(for: definition)
                let button = HelpItemButton(
                    definition: definition,
                    displayName: displayName,
                    isHighlighted: isCurrentTool(definition),
                    target: self,
                    action: #selector(performDefinition(_:))
                )
                definitionByIdentifier[definition.id] = definition
                addFullWidthArrangedSubview(button, to: stack)
            }
        }

        card.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 10),
            stack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -10),
            stack.topAnchor.constraint(equalTo: card.topAnchor, constant: 9),
            stack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -9),
        ])
        return card
    }

    private func contextualName(for definition: ShortcutDefinition) -> String {
        if definition.command == .selectOrCycleEraser {
            return "\(definition.name) · \(state.eraserMode.displayName)"
        }
        if definition.command == .toggleEraserAreaShape {
            return "\(definition.name) · \(state.eraserAreaShape.displayName)"
        }
        if definition.command == .toggleShapeRecognition {
            return "\(definition.name) · \(state.isShapeRecognitionEnabled ? "켬" : "끔")"
        }
        if definition.command == .selectTool(.arrow) {
            return "화살표 · \(state.arrowStyle.displayName)"
        }
        return definition.name
    }

    private func isCurrentTool(_ definition: ShortcutDefinition) -> Bool {
        switch definition.command {
        case let .selectTool(tool):
            return state.currentTool == tool
        case .useLaser:
            return state.currentTool == .laser
        case .selectOrCycleEraser:
            return state.currentTool == .eraser
        case .toggleShapeRecognition:
            return state.isShapeRecognitionEnabled
        case .toggleEraserAreaShape:
            return state.currentTool == .eraser && state.eraserMode == .area
        default:
            return false
        }
    }

    @objc private func searchDidChange(_ sender: NSSearchField) {
        guard isPinned else { return }
        updateContent()
        panel.makeFirstResponder(searchField)
    }

    @objc private func performDefinition(_ sender: NSButton) {
        guard let identifier = sender.identifier?.rawValue,
              let definition = definitionByIdentifier[identifier]
        else {
            NSLog("EduTools: help ignored an item without a shortcut definition")
            return
        }
        let scope: ShortcutScope
        if state.isZoomEnabled, definition.binding(in: .zoom) != nil {
            scope = .zoom
        } else if state.isAnnotationModeEnabled, definition.binding(in: .annotation) != nil {
            scope = .annotation
        } else if definition.binding(in: .global) != nil {
            scope = .global
        } else if definition.binding(in: .annotation) != nil {
            scope = .annotation
        } else {
            scope = .zoom
        }
        dispatcher.execute(definition.command, scope: scope)
        close()
    }

    private func handleKeyDown(_ event: NSEvent) -> Bool {
        if let stepBack = ShortcutCatalog.definition(for: .stepBack)?.binding(in: .annotation),
           stepBack.matches(event) {
            close()
            return true
        }
        if let help = ShortcutCatalog.definition(for: .showHelp)?.binding(in: .annotation),
           help.matches(event) {
            shortcutDidPress(keyCode: event.keyCode)
            return true
        }
        return !isPinned
    }

    private func handleKeyUp(_ event: NSEvent) -> Bool {
        guard let activeShortcutKeyCode,
              event.keyCode == activeShortcutKeyCode
        else {
            return false
        }
        shortcutDidRelease()
        return true
    }

    deinit {
        if let sharingSettingsObserverIdentifier {
            state.removeSharingSettingsObserver(sharingSettingsObserverIdentifier)
        }
        if let toolSettingsObserverIdentifier {
            state.removeToolSettingsObserver(toolSettingsObserverIdentifier)
        }
        if let annotationObserverIdentifier {
            state.removeAnnotationModeObserver(annotationObserverIdentifier)
        }
        if let zoomObserverIdentifier {
            state.removeZoomObserver(zoomObserverIdentifier)
        }
        if let screenObserver {
            NotificationCenter.default.removeObserver(screenObserver)
        }
        panel.close()
    }
}

extension HelpController: NSSearchFieldDelegate {
    // While the search field edits, Esc reaches the field editor as cancelOperation, not keyDown.
    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        guard commandSelector == #selector(NSResponder.cancelOperation(_:)) else { return false }
        close()
        return true
    }
}
