import Foundation

final class AppState {
    typealias AnnotationModeObserver = (Bool) -> Void

    private var observers: [UUID: AnnotationModeObserver] = [:]

    private(set) var isAnnotationModeEnabled = false {
        didSet {
            guard oldValue != isAnnotationModeEnabled else { return }
            for observer in observers.values {
                observer(isAnnotationModeEnabled)
            }
        }
    }

    func toggleAnnotationMode() {
        isAnnotationModeEnabled.toggle()
    }

    @discardableResult
    func addAnnotationModeObserver(_ observer: @escaping AnnotationModeObserver) -> UUID {
        let identifier = UUID()
        observers[identifier] = observer
        observer(isAnnotationModeEnabled)
        return identifier
    }

    func removeAnnotationModeObserver(_ identifier: UUID) {
        observers.removeValue(forKey: identifier)
    }
}

