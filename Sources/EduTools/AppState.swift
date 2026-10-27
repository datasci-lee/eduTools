import AppKit
import Foundation

final class AppState {
    typealias AnnotationModeObserver = (Bool) -> Void
    typealias ToolObserver = (InkTool) -> Void
    typealias ToolSettingsObserver = () -> Void
    typealias PaletteVisibilityObserver = (Bool) -> Void
    typealias InkObserver = (_ displayUUID: String, _ change: InkLayerChange) -> Void
    typealias BoardObserver = () -> Void
    typealias ZoomObserver = (ZoomSessionState?) -> Void

    enum InkLayerChange {
        case added(InkStroke)
        case rebuilt
    }

    private var annotationModeObservers: [UUID: AnnotationModeObserver] = [:]
    private var toolObservers: [UUID: ToolObserver] = [:]
    private var toolSettingsObservers: [UUID: ToolSettingsObserver] = [:]
    private var paletteVisibilityObservers: [UUID: PaletteVisibilityObserver] = [:]
    private var inkObservers: [UUID: InkObserver] = [:]
    private var boardObservers: [UUID: BoardObserver] = [:]
    private var zoomObservers: [UUID: ZoomObserver] = [:]
    private var selectedStrokeIdentifiersByDisplay: [String: Set<UUID>] = [:]
    private let inkState = InkLayerState()
    private let boardDocuments: [BoardKind: BoardDocument] = [
        .white: BoardDocument(),
        .black: BoardDocument(),
    ]
    private var knownDisplayUUIDs: Set<String> = []
    private let defaults: UserDefaults
    private var zoomPreviewOrigin: (center: CGPoint, magnification: CGFloat)?

    private enum DefaultsKey {
        static let lastZoomMagnification = "EduTools.zoom.lastMagnification"
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    private(set) var activeDisplayUUID: String?
    private(set) var presentedBoard: BoardPresentation?
    private(set) var blackboardBackground = BlackboardBackground.black
    private(set) var boardBackgroundPattern = BoardBackgroundPattern.none
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
                clearAllSelections()
                dismissBoard()
            }
            for observer in annotationModeObservers.values {
                observer(isAnnotationModeEnabled)
            }
        }
    }

    private(set) var currentTool = InkTool.pen {
        didSet {
            guard oldValue != currentTool else { return }
            for observer in toolObservers.values {
                observer(currentTool)
            }
            notifyToolSettingsObservers()
        }
    }

    private(set) var selectedColorIndex = InkDefaults.initialColorIndex {
        didSet {
            guard oldValue != selectedColorIndex else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var penWidth = InkDefaults.penWidth {
        didSet {
            guard oldValue != penWidth else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var highlighterWidth = InkDefaults.highlighterWidth {
        didSet {
            guard oldValue != highlighterWidth else { return }
            notifyToolSettingsObservers()
        }
    }
    private(set) var textFontSize = InkDefaults.textFontSize {
        didSet {
            guard oldValue != textFontSize else { return }
            notifyToolSettingsObservers()
        }
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

    private(set) var isPaletteVisible = true {
        didSet {
            guard oldValue != isPaletteVisible else { return }
            for observer in paletteVisibilityObservers.values {
                observer(isPaletteVisible)
            }
        }
    }

    var selectedColor: NSColor {
        InkDefaults.colors[selectedColorIndex]
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
        isAnnotationModeEnabled.toggle()
    }

    func setAnnotationModeEnabled(_ isEnabled: Bool) {
        isAnnotationModeEnabled = isEnabled
    }

    func togglePaletteVisibility() {
        isPaletteVisible.toggle()
    }

    func toggleBoard(_ kind: BoardKind, on displayUUID: String) {
        _ = clearSelection(on: displayUUID)
        knownDisplayUUIDs.insert(displayUUID)
        activeDisplayUUID = displayUUID
        if presentedBoard?.kind == kind {
            presentedBoard = nil
        } else {
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
        guard blackboardBackground != background else { return }
        blackboardBackground = background
        notifyBoardObservers()
    }

    func setBoardBackgroundPattern(_ pattern: BoardBackgroundPattern) {
        guard boardBackgroundPattern != pattern else { return }
        boardBackgroundPattern = pattern
        notifyBoardObservers()
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
        isShapeRecognitionEnabled.toggle()
    }

    func toggleEraserAreaShape() {
        eraserAreaShape = eraserAreaShape.next
    }

    @discardableResult
    func selectOrCycleEraser() -> EraserMode {
        if currentTool == .eraser {
            eraserMode = eraserMode.next
        } else {
            selectTool(.eraser)
        }
        return eraserMode
    }

    func setEraserMode(_ mode: EraserMode) {
        eraserMode = mode
    }

    func setEraserHitRule(_ hitRule: EraserHitRule) {
        eraserHitRule = hitRule
    }

    func setEraserTargetFilter(_ targetFilter: EraserTargetFilter) {
        eraserTargetFilter = targetFilter
    }

    func setEraserDiameter(_ diameter: CGFloat) {
        eraserDiameter = min(
            max(diameter, EraserDefaults.minimumDiameter),
            EraserDefaults.maximumDiameter
        )
    }

    @discardableResult
    func selectEraserDiameterPreset(at index: Int) -> Bool {
        guard EraserDefaults.presetDiameters.indices.contains(index) else { return false }
        setEraserDiameter(EraserDefaults.presetDiameters[index])
        return true
    }

    func activateLaserShortcut() {
        if isAnnotationModeEnabled, currentTool == .laser {
            isAnnotationModeEnabled = false
        } else {
            selectTool(.laser)
            isAnnotationModeEnabled = true
        }
    }

    func selectColor(at index: Int) {
        guard InkDefaults.colors.indices.contains(index) else { return }
        selectedColorIndex = index
    }

    func adjustCurrentToolWidth(by delta: CGFloat) {
        switch currentTool {
        case .pen:
            penWidth = min(max(penWidth + delta, InkDefaults.minimumWidth), InkDefaults.maximumWidth)
        case .highlighter:
            highlighterWidth = min(
                max(highlighterWidth + delta, InkDefaults.minimumWidth),
                InkDefaults.maximumWidth
            )
        case .laser:
            break
        case .eraser:
            setEraserDiameter(eraserDiameter + delta * EraserDefaults.diameterAdjustmentStep)
        case .arrow, .rectangle, .ellipse:
            penWidth = min(max(penWidth + delta, InkDefaults.minimumWidth), InkDefaults.maximumWidth)
        case .text:
            textFontSize = min(
                max(textFontSize + delta, InkDefaults.minimumTextFontSize),
                InkDefaults.maximumTextFontSize
            )
        case .select:
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
        if let document = presentedBoardDocument(on: displayUUID) {
            _ = document.clear()
        } else {
            inkState.execute(
                .replaceDisplayStrokes(
                    displayUUID: displayUUID,
                    before: before,
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
            guard document.clear() else { return false }
            selectedStrokeIdentifiersByDisplay[presentedBoard.displayUUID] = []
            notifyInkObservers(displayUUID: presentedBoard.displayUUID, change: .rebuilt)
            return true
        }
        let before = inkState.allDisplayStrokes
        guard before.values.contains(where: { !$0.isEmpty }) else { return false }
        inkState.execute(.replaceAllDisplayStrokes(before: before, after: [:]))
        selectedStrokeIdentifiersByDisplay.removeAll(keepingCapacity: true)
        notifyAllInkObservers()
        return true
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
        let magnification = ZoomGeometry.steppedMagnification(
            savedMagnification ?? ZoomDefaults.startingMagnification
        )
        zoomPreviewOrigin = nil
        zoomSession = ZoomSessionState(
            displayUUID: displayUUID,
            screenSize: screenSize,
            magnification: magnification,
            startingMagnification: ZoomDefaults.startingMagnification,
            center: ZoomGeometry.clampedCenter(
                center,
                screenSize: screenSize,
                magnification: magnification
            ),
            followMode: ZoomDefaults.defaultFollowMode,
            followSmoothness: ZoomDefaults.followSmoothness,
            previewDimming: ZoomDefaults.previewDimming,
            isFrozen: false,
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
        session.magnification = ZoomGeometry.adjustedMagnification(
            session.magnification,
            by: steps
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
        session.magnification = ZoomGeometry.steppedMagnification(
            session.startingMagnification
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
}

