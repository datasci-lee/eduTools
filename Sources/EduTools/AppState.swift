import AppKit
import Foundation

final class AppState {
    typealias AnnotationModeObserver = (Bool) -> Void
    typealias ToolObserver = (InkTool) -> Void
    typealias InkObserver = (_ displayUUID: String, _ change: InkLayerChange) -> Void

    enum InkLayerChange {
        case added(InkStroke)
        case rebuilt
    }

    private var annotationModeObservers: [UUID: AnnotationModeObserver] = [:]
    private var toolObservers: [UUID: ToolObserver] = [:]
    private var inkObservers: [UUID: InkObserver] = [:]
    private var inkLayers: [String: InkLayerState] = [:]

    private(set) var isAnnotationModeEnabled = false {
        didSet {
            guard oldValue != isAnnotationModeEnabled else { return }
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
        }
    }

    private(set) var selectedColorIndex = InkDefaults.initialColorIndex
    private(set) var penWidth = InkDefaults.penWidth
    private(set) var highlighterWidth = InkDefaults.highlighterWidth

    var selectedColor: NSColor {
        InkDefaults.colors[selectedColorIndex]
    }

    func toggleAnnotationMode() {
        isAnnotationModeEnabled.toggle()
    }

    func setAnnotationModeEnabled(_ isEnabled: Bool) {
        isAnnotationModeEnabled = isEnabled
    }

    func selectTool(_ tool: InkTool) {
        currentTool = tool
    }

    func activateLaserShortcut() {
        if isAnnotationModeEnabled, currentTool == .laser {
            isAnnotationModeEnabled = false
        } else {
            currentTool = .laser
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
        }
    }

    func strokes(for displayUUID: String) -> [InkStroke] {
        inkLayers[displayUUID]?.strokes ?? []
    }

    func addStroke(_ stroke: InkStroke, to displayUUID: String) {
        let layer = inkLayer(for: displayUUID)
        layer.execute(.addStroke(stroke))
        notifyInkObservers(displayUUID: displayUUID, change: .added(stroke))
    }

    @discardableResult
    func undo(on displayUUID: String) -> Bool {
        guard let layer = inkLayers[displayUUID], layer.undo() else { return false }
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
        return true
    }

    @discardableResult
    func redo(on displayUUID: String) -> Bool {
        guard let layer = inkLayers[displayUUID], layer.redo() else { return false }
        notifyInkObservers(displayUUID: displayUUID, change: .rebuilt)
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
    func addInkObserver(_ observer: @escaping InkObserver) -> UUID {
        let identifier = UUID()
        inkObservers[identifier] = observer
        return identifier
    }

    func removeInkObserver(_ identifier: UUID) {
        inkObservers.removeValue(forKey: identifier)
    }

    private func inkLayer(for displayUUID: String) -> InkLayerState {
        if let layer = inkLayers[displayUUID] {
            return layer
        }
        let layer = InkLayerState()
        inkLayers[displayUUID] = layer
        return layer
    }

    private func notifyInkObservers(displayUUID: String, change: InkLayerChange) {
        for observer in inkObservers.values {
            observer(displayUUID, change)
        }
    }
}

