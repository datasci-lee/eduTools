import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

enum PassThroughDefaults {
    static let initialClickTimeout: TimeInterval = 10
    static let interactionIdleTimeout: TimeInterval = 3
    static let keyStatePollingInterval: TimeInterval = 1.0 / 60.0
    static let keyStatePollingTrustWindow: TimeInterval = 0.150
    static let keyMaximumHoldDuration: TimeInterval = 30
}

enum PassThroughModeSource: Equatable {
    case timed
    case key(CGKeyCode)
}

private func passThroughKeyEventTapCallback(
    proxy _: CGEventTapProxy,
    type: CGEventType,
    event: CGEvent,
    userInfo: UnsafeMutableRawPointer?
) -> Unmanaged<CGEvent>? {
    guard let userInfo else { return Unmanaged.passUnretained(event) }
    let state = Unmanaged<AppState>.fromOpaque(userInfo).takeUnretainedValue()
    return state.handlePassThroughKeyEvent(type: type, event: event)
        ? nil
        : Unmanaged.passUnretained(event)
}

enum DisplayDisconnectPolicy: String, CaseIterable {
    case archive
    case moveToPrimary
    case delete

    var displayName: String {
        switch self {
        case .archive: return "보관"
        case .moveToPrimary: return "주 화면으로 옮기기"
        case .delete: return "삭제"
        }
    }
}

final class AppState {
    typealias AnnotationModeObserver = (Bool) -> Void
    typealias PassThroughModeObserver = (Bool) -> Void
    typealias ToolObserver = (InkTool) -> Void
    typealias ToolSettingsObserver = () -> Void
    typealias PaletteVisibilityObserver = (Bool) -> Void
    typealias InkObserver = (_ displayUUID: String, _ change: InkLayerChange) -> Void
    typealias BoardObserver = () -> Void
    typealias ZoomObserver = (ZoomSessionState?) -> Void
    typealias PresenterEffectsObserver = () -> Void
    typealias SharingSettingsObserver = (Bool) -> Void

    enum InkLayerChange {
        case added(InkStroke)
        case rebuilt
    }

    private var annotationModeObservers: [UUID: AnnotationModeObserver] = [:]
    private var passThroughModeObservers: [UUID: PassThroughModeObserver] = [:]
    private var toolObservers: [UUID: ToolObserver] = [:]
    private var toolSettingsObservers: [UUID: ToolSettingsObserver] = [:]
    private var paletteVisibilityObservers: [UUID: PaletteVisibilityObserver] = [:]
    private var inkObservers: [UUID: InkObserver] = [:]
    private var boardObservers: [UUID: BoardObserver] = [:]
    private var zoomObservers: [UUID: ZoomObserver] = [:]
    private var presenterEffectsObservers: [UUID: PresenterEffectsObserver] = [:]
    private var sharingSettingsObservers: [UUID: SharingSettingsObserver] = [:]
    private var selectedStrokeIdentifiersByDisplay: [String: Set<UUID>] = [:]
    private let inkState = InkLayerState()
    private let boardDocuments: [BoardKind: BoardDocument] = [
        .white: BoardDocument(),
        .black: BoardDocument(),
    ]
    private var knownDisplayUUIDs: Set<String> = []
    private let defaults: UserDefaults
    private let settingsStore: SettingsStore
    private var settingsObserverIdentifier: UUID?
    private var zoomPreviewOrigin: (center: CGPoint, magnification: CGFloat)?
    private var lastColorTool = InkTool.pen
    private var previousToolBeforeEraser = InkTool.pen
    private var passThroughGlobalMonitor: Any?
    private var passThroughTimeoutWorkItem: DispatchWorkItem?
    private var passThroughMaximumHoldWorkItem: DispatchWorkItem?
    private var passThroughKeyPollingTimer: Timer?
    private var passThroughKeyStartedAt: TimeInterval?
    private var passThroughKeyEventTap: CFMachPort?
    private var passThroughKeyEventTapSource: CFRunLoopSource?
    private var didLogPassThroughEventTapUnavailable = false

    private static let passThroughEventMask: NSEvent.EventTypeMask = [
        .leftMouseDown,
        .leftMouseUp,
        .rightMouseDown,
        .rightMouseUp,
        .otherMouseDown,
        .otherMouseUp,
        .scrollWheel,
    ]
    private static let passThroughKeyModifierFlags: CGEventFlags = [
        .maskCommand,
        .maskControl,
        .maskAlternate,
        .maskShift,
    ]

    var archiveBeforeRemoval: ((Set<String>) -> Bool)?

    private enum DefaultsKey {
        static let lastZoomMagnification = "EduTools.zoom.lastMagnification"
    }

    init(defaults: UserDefaults = .standard, settingsStore: SettingsStore? = nil) {
        self.defaults = defaults
        let resolvedSettingsStore = settingsStore ?? SettingsStore(defaults: defaults)
        self.settingsStore = resolvedSettingsStore
        let settings = resolvedSettingsStore.settings
        showsHelpInSharing = settings.help.showsHelpInSharing
        autoArchiveEnabled = settings.general.autoArchiveEnabled
        clearsInkWhenAnnotationModeEnds = settings.general.clearsInkWhenAnnotationModeEnds
        displayDisconnectPolicy = DisplayDisconnectPolicy(
            rawValue: settings.general.displayDisconnectPolicy
        ) ?? .archive
        automaticallyRestoresDisconnectedDisplays = settings.general.automaticallyRestoresDisconnectedDisplays
        settingsObserverIdentifier = resolvedSettingsStore.addObserver { [weak self] settings in
            self?.apply(settings)
        }
    }

    private(set) var activeDisplayUUID: String?
    private(set) var presentedBoard: BoardPresentation?
    private(set) var blackboardBackground = BlackboardBackground.black
    private(set) var boardBackgroundPattern = BoardBackgroundPattern.none
    private(set) var whiteboardColor = AppSettings.default.board.whiteboardColor.nsColor
    private(set) var blackboardColor = AppSettings.default.board.blackboardColor.nsColor
    private(set) var autoArchiveEnabled: Bool
    private(set) var clearsInkWhenAnnotationModeEnds: Bool
    private(set) var displayDisconnectPolicy: DisplayDisconnectPolicy
    private(set) var automaticallyRestoresDisconnectedDisplays: Bool
    private(set) var zoomSession: ZoomSessionState? {
        didSet {
            guard oldValue != zoomSession else { return }
            for observer in zoomObservers.values {
                observer(zoomSession)
            }
        }
    }

    var isZoomEnabled: Bool {
        zoomSession != nil
    }

    private(set) var isAnnotationModeEnabled = false {
        didSet {
            guard oldValue != isAnnotationModeEnabled else { return }
            if !isAnnotationModeEnabled {
                setPassThroughModeEnabled(false)
                clearAllSelections()
                dismissBoard()
            }
            for observer in annotationModeObservers.values {
                observer(isAnnotationModeEnabled)
            }
        }
    }

    private(set) var passThroughModeSource: PassThroughModeSource? {
        didSet {
            let wasEnabled = oldValue != nil
            let isEnabled = passThroughModeSource != nil
            guard wasEnabled != isEnabled else { return }
            for observer in passThroughModeObservers.values {
                observer(isEnabled)
            }
        }
    }

    var isPassThroughModeEnabled: Bool {
        passThroughModeSource != nil
    }

    private(set) var currentTool = InkTool.pen {
        didSet {
            guard oldValue != currentTool else { return }
            if oldValue != .eraser, currentTool == .eraser {
                previousToolBeforeEraser = oldValue
            }
            if currentTool.memorySlot != nil {
                lastColorTool = currentTool
            }
            for observer in toolObservers.values {
                observer(currentTool)
            }
            notifyToolSettingsObservers()
        }
    }

    var selectedColorIndex: Int {
        selectedColorIndex(for: currentTool)
            ?? selectedColorIndex(for: lastColorTool)
            ?? PerToolMemorySettings.initialColorIndex
    }

    var currentWidth: CGFloat {
        requiredWidth(for: currentTool)
    }

    var penWidth: CGFloat {
        requiredWidth(for: .pen)
    }

    var highlighterWidth: CGFloat {
        requiredWidth(for: .highlighter)
    }

    private(set) var highlighterOpacity = CGFloat(AppSettings.default.tools.highlighterOpacity)
    private(set) var laserColor = AppSettings.default.tools.laserColor.nsColor
    private(set) var laserSize = CGFloat(AppSettings.default.tools.laserSize)
    private(set) var laserTrailDuration = AppSettings.default.tools.laserTrailDuration
    var textFontSize: CGFloat {
        requiredWidth(for: .text)
    }
    private(set) var arrowStyle = ArrowStyle.single {
        didSet {
            guard oldValue != arrowStyle else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var isShapeRecognitionEnabled = true {
        didSet {
            guard oldValue != isShapeRecognitionEnabled else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var eraserDiameter = EraserDefaults.defaultDiameter {
        didSet {
            guard oldValue != eraserDiameter else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var eraserMode = EraserDefaults.defaultMode {
        didSet {
            guard oldValue != eraserMode else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var eraserAreaShape = EraserAreaShape.rectangle {
        didSet {
            guard oldValue != eraserAreaShape else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var eraserHitRule = EraserDefaults.defaultHitRule
    private(set) var eraserTargetFilter = EraserDefaults.defaultTargetFilter
    private(set) var isSpotlightEnabled = false {
        didSet {
            guard oldValue != isSpotlightEnabled else { return }
            notifyPresenterEffectsObservers()
        }
    }
    private(set) var isCursorHighlightEnabled = false {
        didSet {
            guard oldValue != isCursorHighlightEnabled else { return }
            notifyPresenterEffectsObservers()
        }
    }
    private(set) var isKeystrokeDisplayEnabled = false {
        didSet {
            guard oldValue != isKeystrokeDisplayEnabled else { return }
            notifyPresenterEffectsObservers()
        }
    }
    private(set) var showsHelpInSharing: Bool {
        didSet {
            guard oldValue != showsHelpInSharing else { return }
            for observer in sharingSettingsObservers.values {
                observer(showsHelpInSharing)
            }
        }
    }

    private(set) var isPaletteVisible = true {
        didSet {
            guard oldValue != isPaletteVisible else { return }
            for observer in paletteVisibilityObservers.values {
                observer(isPaletteVisible)
            }
        }
    }

    var selectedColor: NSColor {
        let colors = settingsStore.settings.tools.colors
        guard colors.indices.contains(selectedColorIndex) else { return .systemRed }
        return colors[selectedColorIndex].nsColor
    }

    func selectedColorIndex(for tool: InkTool) -> Int? {
        guard let slot = tool.memorySlot else { return nil }
        return settingsStore.settings.tools.perToolMemory[keyPath: slot.keyPath].colorIndex
    }

    func width(for tool: InkTool) -> CGFloat? {
        guard let slot = tool.memorySlot else { return nil }
        return CGFloat(settingsStore.settings.tools.perToolMemory[keyPath: slot.keyPath].size)
    }

    private func requiredWidth(for tool: InkTool) -> CGFloat {
        guard let width = width(for: tool) else {
            preconditionFailure("Tool does not have a remembered size: \(tool.rawValue)")
        }
        return width
    }

    var colorPresets: [NSColor] {
        settingsStore.settings.tools.colors.map(\.nsColor)
    }

    var eraserSettings: EraserSettings {
        EraserSettings(
            diameter: eraserDiameter,
            mode: eraserMode,
            hitRule: eraserHitRule,
            targetFilter: eraserTargetFilter,
            areaShape: eraserAreaShape
        )
    }

    func toggleAnnotationMode() {
        setAnnotationModeEnabled(!isAnnotationModeEnabled)
    }

    func setAnnotationModeEnabled(_ isEnabled: Bool) {
        guard isAnnotationModeEnabled != isEnabled else { return }
        if !isEnabled, clearsInkWhenAnnotationModeEnds, hasVisibleInk {
            guard clearAllInk() else { return }
        }
        isAnnotationModeEnabled = isEnabled
    }

    func togglePassThroughMode() {
        switch passThroughModeSource {
        case nil:
            setPassThroughModeEnabled(true)
        case .timed:
            setPassThroughModeEnabled(false)
        case .key:
            break
        }
    }

    func setPassThroughModeEnabled(_ isEnabled: Bool) {
        if isEnabled {
            guard passThroughModeSource == nil else { return }
            guard isAnnotationModeEnabled, presentedBoard == nil else { return }
            guard installPassThroughGlobalMonitor() else {
                NSLog("EduTools: pass-through mode was not enabled because its global mouse monitor failed")
                return
            }
            passThroughModeSource = .timed
            schedulePassThroughTimeout(after: PassThroughDefaults.initialClickTimeout)
        } else {
            endPassThroughMode()
        }
    }

    func beginKeyPassThrough(keyCode: CGKeyCode) {
        guard isAnnotationModeEnabled, presentedBoard == nil else { return }
        if passThroughModeSource == .key(keyCode) { return }

        removePassThroughRuntimeResources()
        passThroughModeSource = .key(keyCode)

        if !installPassThroughGlobalMonitor() {
            NSLog("EduTools: pass-through key safety mouse monitor could not be installed")
        }
        installPassThroughKeyEventTap()
        startPassThroughKeyStatePolling()
        schedulePassThroughMaximumHoldTimeout()
    }

    func endKeyPassThrough(keyCode: CGKeyCode) {
        guard passThroughModeSource == .key(keyCode) else { return }
        endPassThroughMode()
    }

    private func endPassThroughMode() {
        guard passThroughModeSource != nil else { return }
        passThroughModeSource = nil
        removePassThroughRuntimeResources()
    }

    private func installPassThroughGlobalMonitor() -> Bool {
        guard passThroughGlobalMonitor == nil else { return true }
        guard let monitor = NSEvent.addGlobalMonitorForEvents(
            matching: Self.passThroughEventMask,
            handler: { [weak self] _ in
                self?.passThroughInteractionDidOccur()
            }
        ) else {
            return false
        }
        passThroughGlobalMonitor = monitor
        return true
    }

    private func removePassThroughGlobalMonitor() {
        guard let passThroughGlobalMonitor else { return }
        NSEvent.removeMonitor(passThroughGlobalMonitor)
        self.passThroughGlobalMonitor = nil
    }

    private func passThroughInteractionDidOccur() {
        switch passThroughModeSource {
        case .timed, .key:
            schedulePassThroughTimeout(after: PassThroughDefaults.interactionIdleTimeout)
        case nil:
            break
        }
    }

    private func schedulePassThroughTimeout(after interval: TimeInterval) {
        passThroughTimeoutWorkItem?.cancel()
        let workItem = DispatchWorkItem { [weak self] in
            self?.passThroughTimeoutDidFire()
        }
        passThroughTimeoutWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + interval, execute: workItem)
    }

    private func passThroughTimeoutDidFire() {
        passThroughTimeoutWorkItem = nil
        guard passThroughModeSource != nil else { return }
        if NSEvent.pressedMouseButtons != 0 {
            schedulePassThroughTimeout(after: PassThroughDefaults.interactionIdleTimeout)
            return
        }
        endPassThroughMode()
    }

    private func schedulePassThroughMaximumHoldTimeout() {
        passThroughMaximumHoldWorkItem?.cancel()
        let workItem = DispatchWorkItem { [weak self] in
            guard case .key = self?.passThroughModeSource else { return }
            self?.endPassThroughMode()
        }
        passThroughMaximumHoldWorkItem = workItem
        DispatchQueue.main.asyncAfter(
            deadline: .now() + PassThroughDefaults.keyMaximumHoldDuration,
            execute: workItem
        )
    }

    private func startPassThroughKeyStatePolling() {
        passThroughKeyPollingTimer?.invalidate()
        passThroughKeyStartedAt = ProcessInfo.processInfo.systemUptime
        let timer = Timer(
            timeInterval: PassThroughDefaults.keyStatePollingInterval,
            repeats: true
        ) { [weak self] _ in
            self?.pollPassThroughKeyState()
        }
        passThroughKeyPollingTimer = timer
        RunLoop.main.add(timer, forMode: .common)
    }

    private func pollPassThroughKeyState() {
        guard case let .key(keyCode) = passThroughModeSource,
              let passThroughKeyStartedAt
        else {
            stopPassThroughKeyStatePolling()
            return
        }
        guard !CGEventSource.keyState(.combinedSessionState, key: keyCode) else { return }

        let elapsed = ProcessInfo.processInfo.systemUptime - passThroughKeyStartedAt
        if elapsed <= PassThroughDefaults.keyStatePollingTrustWindow {
            let trustWindowMilliseconds = Int(
                PassThroughDefaults.keyStatePollingTrustWindow * 1_000
            )
            NSLog(
                "EduTools: pass-through key-state polling was disabled because the held key appeared released within %d ms",
                trustWindowMilliseconds
            )
            stopPassThroughKeyStatePolling()
            return
        }
        endKeyPassThrough(keyCode: keyCode)
    }

    private func stopPassThroughKeyStatePolling() {
        passThroughKeyPollingTimer?.invalidate()
        passThroughKeyPollingTimer = nil
        passThroughKeyStartedAt = nil
    }

    private func installPassThroughKeyEventTap() {
        guard passThroughKeyEventTap == nil else { return }
        guard AXIsProcessTrusted() else {
            logPassThroughEventTapUnavailableOnce(
                "Accessibility permission is not granted"
            )
            return
        }

        let eventMask = (CGEventMask(1) << CGEventType.keyDown.rawValue)
            | (CGEventMask(1) << CGEventType.keyUp.rawValue)
        guard let eventTap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: eventMask,
            callback: passThroughKeyEventTapCallback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else {
            logPassThroughEventTapUnavailableOnce("the active event tap could not be created")
            return
        }
        guard let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, eventTap, 0) else {
            CFMachPortInvalidate(eventTap)
            logPassThroughEventTapUnavailableOnce("the event tap run-loop source could not be created")
            return
        }

        passThroughKeyEventTap = eventTap
        passThroughKeyEventTapSource = source
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: eventTap, enable: true)
    }

    fileprivate func handlePassThroughKeyEvent(type: CGEventType, event: CGEvent) -> Bool {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if case .key = passThroughModeSource, let passThroughKeyEventTap {
                CGEvent.tapEnable(tap: passThroughKeyEventTap, enable: true)
            }
            return false
        }
        guard type == .keyDown || type == .keyUp,
              case let .key(targetKeyCode) = passThroughModeSource
        else {
            return false
        }

        let eventKeyCode = CGKeyCode(event.getIntegerValueField(.keyboardEventKeycode))
        guard eventKeyCode == targetKeyCode else { return false }
        let hasModifiers = !event.flags.intersection(Self.passThroughKeyModifierFlags).isEmpty
        if type == .keyUp {
            endKeyPassThrough(keyCode: targetKeyCode)
        }
        return !hasModifiers
    }

    private func removePassThroughKeyEventTap() {
        if let passThroughKeyEventTapSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), passThroughKeyEventTapSource, .commonModes)
            self.passThroughKeyEventTapSource = nil
        }
        if let passThroughKeyEventTap {
            CFMachPortInvalidate(passThroughKeyEventTap)
            self.passThroughKeyEventTap = nil
        }
    }

    private func logPassThroughEventTapUnavailableOnce(_ reason: String) {
        guard !didLogPassThroughEventTapUnavailable else { return }
        didLogPassThroughEventTapUnavailable = true
        NSLog("EduTools: pass-through key suppression is unavailable because \(reason)")
    }

    private func removePassThroughRuntimeResources() {
        passThroughTimeoutWorkItem?.cancel()
        passThroughTimeoutWorkItem = nil
        passThroughMaximumHoldWorkItem?.cancel()
        passThroughMaximumHoldWorkItem = nil
        stopPassThroughKeyStatePolling()
        removePassThroughKeyEventTap()
        removePassThroughGlobalMonitor()
    }

    func setAutoArchiveEnabled(_ isEnabled: Bool) {
        settingsStore.update { $0.general.autoArchiveEnabled = isEnabled }
    }

    func setClearsInkWhenAnnotationModeEnds(_ isEnabled: Bool) {
        settingsStore.update { $0.general.clearsInkWhenAnnotationModeEnds = isEnabled }
    }

    func setDisplayDisconnectPolicy(_ policy: DisplayDisconnectPolicy) {
        settingsStore.update { $0.general.displayDisconnectPolicy = policy.rawValue }
    }

    func setAutomaticallyRestoresDisconnectedDisplays(_ isEnabled: Bool) {
        settingsStore.update {
            $0.general.automaticallyRestoresDisconnectedDisplays = isEnabled
        }
    }

    func togglePaletteVisibility() {
        isPaletteVisible.toggle()
    }

    func toggleSpotlight() {
        isSpotlightEnabled.toggle()
    }

    func toggleCursorHighlight() {
        isCursorHighlightEnabled.toggle()
    }

    func setKeystrokeDisplayEnabled(_ isEnabled: Bool) {
        isKeystrokeDisplayEnabled = isEnabled
    }

    func toggleHelpSharing() {
        settingsStore.update { $0.help.showsHelpInSharing.toggle() }
    }

    func toggleBoard(_ kind: BoardKind, on displayUUID: String) {
        _ = clearSelection(on: displayUUID)
        knownDisplayUUIDs.insert(displayUUID)
        activeDisplayUUID = displayUUID
        if presentedBoard?.kind == kind {
            presentedBoard = nil
        } else {
            setPassThroughModeEnabled(false)
            presentedBoard = BoardPresentation(kind: kind, displayUUID: displayUUID)
        }
        notifyBoardObservers()
    }

    func dismissBoard() {
        guard presentedBoard != nil else { return }
        presentedBoard = nil
        notifyBoardObservers()
    }

    func relocatePresentedBoard(to displayUUID: String) {
        guard let presentedBoard, presentedBoard.displayUUID != displayUUID else { return }
        knownDisplayUUIDs.insert(displayUUID)
        activeDisplayUUID = displayUUID
        self.presentedBoard = BoardPresentation(
            kind: presentedBoard.kind,
            displayUUID: displayUUID
        )
        notifyBoardObservers()
    }

    func boardKind(on displayUUID: String) -> BoardKind? {
        guard presentedBoard?.displayUUID == displayUUID else { return nil }
        return presentedBoard?.kind
    }

    func boardPagePosition(on displayUUID: String) -> BoardPagePosition? {
        guard let document = presentedBoardDocument(on: displayUUID) else { return nil }
        return document.pagePosition
    }

    @discardableResult
    func moveToPreviousBoardPage(on displayUUID: String) -> Bool {
        guard let document = presentedBoardDocument(on: displayUUID),
              document.moveToPreviousPage()
        else {
            return false
        }
        _ = clearSelection(on: displayUUID)
        notifyBoardObservers()
        return true
    }

    @discardableResult
    func moveToNextBoardPage(on displayUUID: String) -> Bool {
        guard let document = presentedBoardDocument(on: displayUUID),
              document.moveToNextPage()
        else {
            return false
        }
        _ = clearSelection(on: displayUUID)
        notifyBoardObservers()
        return true
    }

    func setBlackboardBackground(_ background: BlackboardBackground) {
        let color: SettingsColor
        switch background {
        case .black:
            color = SettingsColor(red: 0, green: 0, blue: 0)
        case .chalkGreen:
            color = SettingsColor(red: 0.055, green: 0.18, blue: 0.13)
        }
        settingsStore.update { $0.board.blackboardColor = color }
    }

    func setBoardBackgroundPattern(_ pattern: BoardBackgroundPattern) {
        settingsStore.update {
            switch pattern {
            case .none: $0.board.backgroundPattern = "none"
            case .grid: $0.board.backgroundPattern = "grid"
            case .lines: $0.board.backgroundPattern = "lines"
            }
        }
    }

    func boardBackgroundColor(for kind: BoardKind) -> NSColor {
        switch kind {
        case .white: return whiteboardColor
        case .black: return blackboardColor
        }
    }

    func selectTool(_ tool: InkTool) {
        if tool == .arrow, currentTool == .arrow {
            arrowStyle = arrowStyle.next
            return
        }
        if tool != .select {
            clearAllSelections()
        }
        currentTool = tool
    }

    func toggleShapeRecognition() {
        settingsStore.update { $0.tools.shapeRecognitionEnabled.toggle() }
    }

    func setShapeRecognitionEnabled(_ isEnabled: Bool) {
        settingsStore.update { $0.tools.shapeRecognitionEnabled = isEnabled }
    }

    func setPenWidth(_ width: CGFloat) {
        setWidth(width, for: .pen)
    }

    func setHighlighterWidth(_ width: CGFloat) {
        setWidth(width, for: .highlighter)
    }

    func setHighlighterOpacity(_ opacity: CGFloat) {
        let clamped = min(
            max(opacity, InkDefaults.minimumHighlighterOpacity),
            InkDefaults.maximumHighlighterOpacity
        )
        settingsStore.update { $0.tools.highlighterOpacity = Double(clamped) }
    }

    func setTextFontSize(_ size: CGFloat) {
        setWidth(size, for: .text)
    }

    func setWidth(_ width: CGFloat, for tool: InkTool) {
        guard let slot = tool.memorySlot else { return }
        let clamped = min(max(Double(width), slot.sizeRange.lowerBound), slot.sizeRange.upperBound)
        settingsStore.update {
            $0.tools.perToolMemory[keyPath: slot.keyPath].size = clamped
        }
    }

    func setLaserColor(_ color: NSColor) {
        settingsStore.update { $0.tools.laserColor = SettingsColor(color) }
    }

    func setLaserTrailDuration(_ duration: TimeInterval) {
        let clamped = min(
            max(duration, InkDefaults.minimumLaserTrailDuration),
            InkDefaults.maximumLaserTrailDuration
        )
        settingsStore.update { $0.tools.laserTrailDuration = clamped }
    }

    func setArrowStyle(_ style: ArrowStyle) {
        arrowStyle = style
    }

    func toggleEraserAreaShape() {
        settingsStore.update {
            $0.eraser.areaShape = eraserAreaShape.next == .rectangle ? "rectangle" : "lasso"
        }
    }

    func setEraserAreaShape(_ shape: EraserAreaShape) {
        settingsStore.update {
            $0.eraser.areaShape = shape == .rectangle ? "rectangle" : "lasso"
        }
    }

    @discardableResult
    func selectOrCycleEraser() -> EraserMode {
        if currentTool == .eraser {
            setEraserMode(eraserMode.next)
        } else {
            selectTool(.eraser)
        }
        return eraserMode
    }

    func toggleEraserTool() {
        if currentTool == .eraser {
            selectTool(previousToolBeforeEraser)
        } else {
            selectTool(.eraser)
        }
    }

    func setEraserMode(_ mode: EraserMode) {
        settingsStore.update {
            switch mode {
            case .partial: $0.eraser.mode = "partial"
            case .stroke: $0.eraser.mode = "stroke"
            case .area: $0.eraser.mode = "area"
            }
        }
    }

    func setEraserHitRule(_ hitRule: EraserHitRule) {
        settingsStore.update {
            $0.eraser.hitRule = hitRule == .touching ? "touching" : "contained"
        }
    }

    func setEraserTargetFilter(_ targetFilter: EraserTargetFilter) {
        settingsStore.update {
            switch targetFilter {
            case .all: $0.eraser.targetFilter = "all"
            case .penOnly: $0.eraser.targetFilter = "penOnly"
            case .highlighterOnly: $0.eraser.targetFilter = "highlighterOnly"
            }
        }
    }

    func setEraserDiameter(_ diameter: CGFloat) {
        let clamped = min(
            max(diameter, EraserDefaults.minimumDiameter),
            EraserDefaults.maximumDiameter
        )
        settingsStore.update { $0.eraser.diameter = Double(clamped) }
    }

    @discardableResult
    func selectEraserDiameterPreset(at index: Int) -> Bool {
        let presets = settingsStore.settings.eraser.presetDiameters
        guard presets.indices.contains(index) else { return false }
        setEraserDiameter(CGFloat(presets[index]))
        return true
    }

    func activateLaserShortcut() {
        if isAnnotationModeEnabled, currentTool == .laser {
            setAnnotationModeEnabled(false)
        } else {
            selectTool(.laser)
            setAnnotationModeEnabled(true)
        }
    }

    func selectColor(at index: Int) {
        let colors = settingsStore.settings.tools.colors
        guard colors.indices.contains(index) else { return }
        if currentTool == .laser {
            setLaserColor(colors[index].nsColor)
        } else if let slot = currentTool.memorySlot {
            settingsStore.update {
                $0.tools.perToolMemory[keyPath: slot.keyPath].colorIndex = index
            }
        }
    }

    func adjustCurrentToolWidth(by delta: CGFloat) {
        if let width = width(for: currentTool) {
            setWidth(width + delta, for: currentTool)
            return
        }
        switch currentTool {
        case .eraser:
            setEraserDiameter(eraserDiameter + delta * EraserDefaults.diameterAdjustmentStep)
        case .pen, .highlighter, .arrow, .rectangle, .ellipse, .text,
             .laser, .select, .cover:
            break
        }
    }

    func strokes(for displayUUID: String) -> [InkStroke] {
        knownDisplayUUIDs.insert(displayUUID)
        if let document = presentedBoardDocument(on: displayUUID) {
            return document.strokes
        }
        return inkState.strokes(for: displayUUID)
    }

    func addStroke(_ stroke: InkStroke, to displayUUID: String) {
        knownDisplayUUIDs.insert(displayUUID)
        if let document = presentedBoardDocument(on: displayUUID) {
            document.addStroke(stroke)
        } else {
            inkState.execute(.addDisplayStroke(displayUUID: displayUUID, stroke: stroke))
        }
        notifyInkObservers(displayUUID: displayUUID, change: .added(stroke))
    }

    @discardableResult
    func replaceStrokes(_ after: [InkStroke], on displayUUID: String) -> Bool {
        knownDisplayUUIDs.insert(displayUUID)
        let before = strokes(for: displayUUID)
        guard before != after else { return false }
        if let document = presentedBoardDocument(on: displayUUID) {
            _ = document.replaceStrokes(with: after)
        } else {
            inkState.execute(
                .replaceDisplayStrokes(
                    displayUUID: displayUUID,
                    before: before,
                    after: after
                )
            )
        }
        let availableIdentifiers = Set(after.map(\.identifier))
        selectedStrokeIdentifiersByDisplay[displayUUID] = selectedStrokeIdentifiers(
            on: displayUUID
        ).intersection(availableIdentifiers)
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
        return true
    }

    @discardableResult
    func removeTopmostCover(at normalizedPoint: CGPoint, on displayUUID: String) -> Bool {
        let before = strokes(for: displayUUID)
        let after = ScreenCoverEngine.removingTopmostCover(
            at: normalizedPoint,
            from: before
        )
        return replaceStrokes(after, on: displayUUID)
    }

    func selectedStrokeIdentifiers(on displayUUID: String) -> Set<UUID> {
        selectedStrokeIdentifiersByDisplay[displayUUID] ?? []
    }

    func setSelectedStrokeIdentifiers(_ identifiers: Set<UUID>, on displayUUID: String) {
        knownDisplayUUIDs.insert(displayUUID)
        let available = Set(strokes(for: displayUUID).map(\.identifier))
        let selection = identifiers.intersection(available)
        guard selectedStrokeIdentifiersByDisplay[displayUUID] != selection else { return }
        selectedStrokeIdentifiersByDisplay[displayUUID] = selection
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
    }

    @discardableResult
    func clearSelection(on displayUUID: String) -> Bool {
        guard selectedStrokeIdentifiersByDisplay[displayUUID]?.isEmpty == false else {
            return false
        }
        selectedStrokeIdentifiersByDisplay[displayUUID] = []
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
        return true
    }

    @discardableResult
    func deleteSelection(on displayUUID: String) -> Bool {
        let selected = selectedStrokeIdentifiers(on: displayUUID)
        guard !selected.isEmpty else { return false }
        let after = strokes(for: displayUUID).filter { !selected.contains($0.identifier) }
        selectedStrokeIdentifiersByDisplay[displayUUID] = []
        return replaceStrokes(after, on: displayUUID)
    }

    @discardableResult
    func erase(
        _ gesture: EraserGesture,
        settings: EraserSettings,
        on displayUUID: String,
        canvasSize: CGSize
    ) -> Bool {
        knownDisplayUUIDs.insert(displayUUID)
        let before = strokes(for: displayUUID)
        let after = EraserEngine.erase(
            strokes: before,
            gesture: gesture,
            settings: settings,
            canvasSize: canvasSize
        )
        return replaceStrokes(after, on: displayUUID)
    }

    @discardableResult
    func clearInk(on displayUUID: String) -> Bool {
        knownDisplayUUIDs.insert(displayUUID)
        let before = strokes(for: displayUUID)
        guard !before.isEmpty else { return false }
        if autoArchiveEnabled,
           archiveBeforeRemoval?([displayUUID]) == false
        {
            return false
        }
        let archivedBefore = strokes(for: displayUUID)
        if let document = presentedBoardDocument(on: displayUUID) {
            _ = document.clear()
        } else {
            inkState.execute(
                .replaceDisplayStrokes(
                    displayUUID: displayUUID,
                    before: archivedBefore,
                    after: []
                )
            )
        }
        selectedStrokeIdentifiersByDisplay[displayUUID] = []
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
        return true
    }

    @discardableResult
    func clearCurrentDisplayInk() -> Bool {
        guard let activeDisplayUUID else { return false }
        return clearInk(on: activeDisplayUUID)
    }

    @discardableResult
    func clearAllInk() -> Bool {
        if let presentedBoard {
            let document = boardDocument(for: presentedBoard.kind)
            guard !document.strokes.isEmpty else { return false }
            if autoArchiveEnabled,
               archiveBeforeRemoval?([presentedBoard.displayUUID]) == false
            {
                return false
            }
            guard document.clear() else { return false }
            selectedStrokeIdentifiersByDisplay[presentedBoard.displayUUID] = []
            notifyInkObservers(displayUUID: presentedBoard.displayUUID, change: .rebuilt)
            return true
        }
        let before = inkState.allDisplayStrokes
        let affectedDisplays = Set(before.compactMap { entry in
            entry.value.isEmpty ? nil : entry.key
        })
        guard !affectedDisplays.isEmpty else { return false }
        if autoArchiveEnabled,
           archiveBeforeRemoval?(affectedDisplays) == false
        {
            return false
        }
        inkState.execute(.replaceAllDisplayStrokes(before: before, after: [:]))
        selectedStrokeIdentifiersByDisplay.removeAll(keepingCapacity: true)
        notifyAllInkObservers()
        return true
    }

    var hasVisibleInk: Bool {
        if let presentedBoard {
            return !boardDocument(for: presentedBoard.kind).strokes.isEmpty
        }
        return inkState.allDisplayStrokes.values.contains { !$0.isEmpty }
    }

    func screenStrokes(for displayUUID: String) -> [InkStroke] {
        inkState.strokes(for: displayUUID)
    }

    func hasRestorableScreenState(for displayUUID: String) -> Bool {
        inkState.hasRestorableState(for: displayUUID)
    }

    func encodedScreenInk(for displayUUID: String) throws -> Data {
        try Self.archiveEncoder.encode(inkState.state(for: displayUUID))
    }

    func encodedVisibleContent(on displayUUID: String) throws -> Data {
        if let document = presentedBoardDocument(on: displayUUID) {
            return try Self.archiveEncoder.encode(document)
        }
        return try encodedScreenInk(for: displayUUID)
    }

    func restoreScreenInk(from data: Data, to displayUUID: String) throws {
        let archivedState = try JSONDecoder().decode(InkLayerState.self, from: data)
        guard archivedState.hasRestorableState(for: displayUUID) else {
            throw CocoaError(.fileReadCorruptFile)
        }
        knownDisplayUUIDs.insert(displayUUID)
        inkState.mergeDisplayState(archivedState, displayUUID: displayUUID)
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
    }

    func discardScreenInk(on displayUUID: String) {
        inkState.removeDisplay(displayUUID)
        selectedStrokeIdentifiersByDisplay.removeValue(forKey: displayUUID)
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
    }

    @discardableResult
    func moveScreenInk(from sourceDisplayUUID: String, to destinationDisplayUUID: String) -> Bool {
        guard sourceDisplayUUID != destinationDisplayUUID else { return false }
        let sourceStrokes = inkState.strokes(for: sourceDisplayUUID)
        guard !sourceStrokes.isEmpty else {
            inkState.removeDisplay(sourceDisplayUUID)
            return false
        }
        let before = inkState.allDisplayStrokes
        var after = before
        after.removeValue(forKey: sourceDisplayUUID)
        after[destinationDisplayUUID] = (before[destinationDisplayUUID] ?? []) + sourceStrokes
        inkState.execute(.replaceAllDisplayStrokes(before: before, after: after))
        knownDisplayUUIDs.insert(destinationDisplayUUID)
        selectedStrokeIdentifiersByDisplay.removeValue(forKey: sourceDisplayUUID)
        selectedStrokeIdentifiersByDisplay[destinationDisplayUUID] = []
        notifyInkObservers(displayUUID: sourceDisplayUUID, change: .rebuilt)
        notifyInkObservers(displayUUID: destinationDisplayUUID, change: .rebuilt)
        return true
    }

    func boardPages(for kind: BoardKind) -> [[InkStroke]] {
        boardDocument(for: kind).allPageStrokes
    }

    private static var archiveEncoder: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return encoder
    }

    func setActiveDisplay(_ displayUUID: String) {
        knownDisplayUUIDs.insert(displayUUID)
        activeDisplayUUID = displayUUID
    }

    func beginZoom(
        on displayUUID: String,
        screenSize: CGSize,
        center: CGPoint
    ) {
        knownDisplayUUIDs.insert(displayUUID)
        activeDisplayUUID = displayUUID
        let savedMagnification = (defaults.object(
            forKey: DefaultsKey.lastZoomMagnification
        ) as? NSNumber).map { CGFloat($0.doubleValue) }
        let zoomSettings = settingsStore.settings.zoom
        let startingMagnification = CGFloat(zoomSettings.startingMagnification)
        let magnification = steppedZoomMagnification(
            savedMagnification ?? startingMagnification
        )
        zoomPreviewOrigin = nil
        zoomSession = ZoomSessionState(
            displayUUID: displayUUID,
            screenSize: screenSize,
            magnification: magnification,
            startingMagnification: startingMagnification,
            center: ZoomGeometry.clampedCenter(
                center,
                screenSize: screenSize,
                magnification: magnification
            ),
            followMode: ZoomFollowMode(rawValue: zoomSettings.followMode) ?? .edge,
            followSmoothness: CGFloat(zoomSettings.followSmoothness),
            previewDimming: CGFloat(zoomSettings.previewDimming),
            isFrozen: !zoomSettings.liveByDefault,
            isPreviewing: false
        )
    }

    func endZoom() {
        zoomPreviewOrigin = nil
        zoomSession = nil
    }

    @discardableResult
    func adjustZoomMagnification(by steps: Int) -> CGFloat? {
        guard var session = zoomSession else { return nil }
        let step = CGFloat(settingsStore.settings.zoom.magnificationStep)
        session.magnification = steppedZoomMagnification(
            session.magnification + CGFloat(steps) * step
        )
        session.center = ZoomGeometry.clampedCenter(
            session.center,
            screenSize: session.screenSize,
            magnification: session.magnification
        )
        defaults.set(
            Double(session.magnification),
            forKey: DefaultsKey.lastZoomMagnification
        )
        zoomSession = session
        return session.magnification
    }

    @discardableResult
    func resetZoomMagnification() -> CGFloat? {
        guard var session = zoomSession else { return nil }
        session.magnification = steppedZoomMagnification(session.startingMagnification)
        session.center = ZoomGeometry.clampedCenter(
            session.center,
            screenSize: session.screenSize,
            magnification: session.magnification
        )
        defaults.set(
            Double(session.magnification),
            forKey: DefaultsKey.lastZoomMagnification
        )
        zoomSession = session
        return session.magnification
    }

    @discardableResult
    func cycleZoomFollowMode() -> ZoomFollowMode? {
        guard var session = zoomSession else { return nil }
        session.followMode = session.followMode.next
        zoomSession = session
        return session.followMode
    }

    @discardableResult
    func toggleZoomFrozen() -> Bool? {
        guard var session = zoomSession, !session.isPreviewing else { return nil }
        session.isFrozen.toggle()
        zoomSession = session
        return session.isFrozen
    }

    func beginZoomPreview() {
        guard var session = zoomSession, !session.isPreviewing else { return }
        zoomPreviewOrigin = (session.center, session.magnification)
        session.isPreviewing = true
        zoomSession = session
    }

    func finishZoomPreview(cancelled: Bool) {
        guard var session = zoomSession, session.isPreviewing else { return }
        if cancelled, let zoomPreviewOrigin {
            session.center = zoomPreviewOrigin.center
            session.magnification = zoomPreviewOrigin.magnification
        }
        session.isPreviewing = false
        zoomPreviewOrigin = nil
        defaults.set(
            Double(session.magnification),
            forKey: DefaultsKey.lastZoomMagnification
        )
        zoomSession = session
    }

    func setZoomCenter(_ center: CGPoint) {
        guard var session = zoomSession else { return }
        session.center = ZoomGeometry.clampedCenter(
            center,
            screenSize: session.screenSize,
            magnification: session.magnification
        )
        zoomSession = session
    }

    func advanceZoomFollow(toward pointer: CGPoint) {
        guard var session = zoomSession,
              !session.isFrozen,
              !session.isPreviewing
        else {
            return
        }
        session.center = ZoomGeometry.followedCenter(
            current: session.center,
            pointer: pointer,
            screenSize: session.screenSize,
            magnification: session.magnification,
            mode: session.followMode,
            smoothness: session.followSmoothness
        )
        zoomSession = session
    }

    private func steppedZoomMagnification(_ value: CGFloat) -> CGFloat {
        let zoom = settingsStore.settings.zoom
        let step = CGFloat(zoom.magnificationStep)
        let rounded = (value / step).rounded() * step
        return min(
            max(rounded, ZoomDefaults.minimumMagnification),
            CGFloat(zoom.maximumMagnification)
        )
    }

    @discardableResult
    func undo(on displayUUID: String) -> Bool {
        knownDisplayUUIDs.insert(displayUUID)
        selectedStrokeIdentifiersByDisplay[displayUUID] = []
        if let document = presentedBoardDocument(on: displayUUID) {
            guard document.undo() else { return false }
            notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
            return true
        }
        guard inkState.undo() else { return false }
        notifyAllInkObservers()
        return true
    }

    @discardableResult
    func redo(on displayUUID: String) -> Bool {
        knownDisplayUUIDs.insert(displayUUID)
        selectedStrokeIdentifiersByDisplay[displayUUID] = []
        if let document = presentedBoardDocument(on: displayUUID) {
            guard document.redo() else { return false }
            notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
            return true
        }
        guard inkState.redo() else { return false }
        notifyAllInkObservers()
        return true
    }

    @discardableResult
    func addAnnotationModeObserver(_ observer: @escaping AnnotationModeObserver) -> UUID {
        let identifier = UUID()
        annotationModeObservers[identifier] = observer
        observer(isAnnotationModeEnabled)
        return identifier
    }

    func removeAnnotationModeObserver(_ identifier: UUID) {
        annotationModeObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addPassThroughModeObserver(_ observer: @escaping PassThroughModeObserver) -> UUID {
        let identifier = UUID()
        passThroughModeObservers[identifier] = observer
        observer(isPassThroughModeEnabled)
        return identifier
    }

    func removePassThroughModeObserver(_ identifier: UUID) {
        passThroughModeObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addToolObserver(_ observer: @escaping ToolObserver) -> UUID {
        let identifier = UUID()
        toolObservers[identifier] = observer
        observer(currentTool)
        return identifier
    }

    func removeToolObserver(_ identifier: UUID) {
        toolObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addToolSettingsObserver(_ observer: @escaping ToolSettingsObserver) -> UUID {
        let identifier = UUID()
        toolSettingsObservers[identifier] = observer
        observer()
        return identifier
    }

    func removeToolSettingsObserver(_ identifier: UUID) {
        toolSettingsObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addPaletteVisibilityObserver(_ observer: @escaping PaletteVisibilityObserver) -> UUID {
        let identifier = UUID()
        paletteVisibilityObservers[identifier] = observer
        observer(isPaletteVisible)
        return identifier
    }

    func removePaletteVisibilityObserver(_ identifier: UUID) {
        paletteVisibilityObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addInkObserver(_ observer: @escaping InkObserver) -> UUID {
        let identifier = UUID()
        inkObservers[identifier] = observer
        return identifier
    }

    func removeInkObserver(_ identifier: UUID) {
        inkObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addBoardObserver(_ observer: @escaping BoardObserver) -> UUID {
        let identifier = UUID()
        boardObservers[identifier] = observer
        observer()
        return identifier
    }

    func removeBoardObserver(_ identifier: UUID) {
        boardObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addZoomObserver(_ observer: @escaping ZoomObserver) -> UUID {
        let identifier = UUID()
        zoomObservers[identifier] = observer
        observer(zoomSession)
        return identifier
    }

    func removeZoomObserver(_ identifier: UUID) {
        zoomObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addPresenterEffectsObserver(_ observer: @escaping PresenterEffectsObserver) -> UUID {
        let identifier = UUID()
        presenterEffectsObservers[identifier] = observer
        observer()
        return identifier
    }

    func removePresenterEffectsObserver(_ identifier: UUID) {
        presenterEffectsObservers.removeValue(forKey: identifier)
    }

    @discardableResult
    func addSharingSettingsObserver(_ observer: @escaping SharingSettingsObserver) -> UUID {
        let identifier = UUID()
        sharingSettingsObservers[identifier] = observer
        observer(showsHelpInSharing)
        return identifier
    }

    func removeSharingSettingsObserver(_ identifier: UUID) {
        sharingSettingsObservers.removeValue(forKey: identifier)
    }

    private func notifyInkObservers(displayUUID: String, change: InkLayerChange) {
        for observer in inkObservers.values {
            observer(displayUUID, change)
        }
    }

    private func notifyToolSettingsObservers() {
        for observer in toolSettingsObservers.values {
            observer()
        }
    }

    private func notifyPresenterEffectsObservers() {
        for observer in presenterEffectsObservers.values {
            observer()
        }
    }

    private func notifyBoardObservers() {
        for observer in boardObservers.values {
            observer()
        }
    }

    private func notifyAllInkObservers() {
        for displayUUID in knownDisplayUUIDs {
            notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
        }
    }

    private func apply(_ settings: AppSettings) {
        autoArchiveEnabled = settings.general.autoArchiveEnabled
        clearsInkWhenAnnotationModeEnds = settings.general.clearsInkWhenAnnotationModeEnds
        displayDisconnectPolicy = DisplayDisconnectPolicy(
            rawValue: settings.general.displayDisconnectPolicy
        ) ?? .archive
        automaticallyRestoresDisconnectedDisplays = settings.general.automaticallyRestoresDisconnectedDisplays
        showsHelpInSharing = settings.help.showsHelpInSharing

        highlighterOpacity = CGFloat(settings.tools.highlighterOpacity)
        laserColor = settings.tools.laserColor.nsColor
        laserSize = CGFloat(settings.tools.laserSize)
        laserTrailDuration = settings.tools.laserTrailDuration
        isShapeRecognitionEnabled = settings.tools.shapeRecognitionEnabled

        switch settings.eraser.mode {
        case "stroke": eraserMode = .stroke
        case "area": eraserMode = .area
        default: eraserMode = .partial
        }
        eraserDiameter = CGFloat(settings.eraser.diameter)
        eraserHitRule = settings.eraser.hitRule == "contained" ? .contained : .touching
        switch settings.eraser.targetFilter {
        case "penOnly": eraserTargetFilter = .penOnly
        case "highlighterOnly": eraserTargetFilter = .highlighterOnly
        default: eraserTargetFilter = .all
        }
        eraserAreaShape = settings.eraser.areaShape == "lasso" ? .lasso : .rectangle

        whiteboardColor = settings.board.whiteboardColor.nsColor
        blackboardColor = settings.board.blackboardColor.nsColor
        switch settings.board.backgroundPattern {
        case "grid": boardBackgroundPattern = .grid
        case "lines": boardBackgroundPattern = .lines
        default: boardBackgroundPattern = .none
        }
        blackboardBackground = settings.board.blackboardColor.green > settings.board.blackboardColor.red
            ? .chalkGreen
            : .black

        notifyToolSettingsObservers()
        notifyBoardObservers()
    }

    private func clearAllSelections() {
        let selectedDisplays = selectedStrokeIdentifiersByDisplay.compactMap { entry in
            entry.value.isEmpty ? nil : entry.key
        }
        guard !selectedDisplays.isEmpty else { return }
        for displayUUID in selectedDisplays {
            selectedStrokeIdentifiersByDisplay[displayUUID] = []
            notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
        }
    }

    private func presentedBoardDocument(on displayUUID: String) -> BoardDocument? {
        guard let presentedBoard, presentedBoard.displayUUID == displayUUID else { return nil }
        return boardDocument(for: presentedBoard.kind)
    }

    private func boardDocument(for kind: BoardKind) -> BoardDocument {
        guard let document = boardDocuments[kind] else {
            preconditionFailure("Missing board document for \(kind)")
        }
        return document
    }

    deinit {
        removePassThroughRuntimeResources()
        if let settingsObserverIdentifier {
            settingsStore.removeObserver(settingsObserverIdentifier)
        }
    }
}

