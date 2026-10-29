import Foundation

struct ObserverRegistry<Observer> {
    private var observers: [UUID: Observer] = [:]

    @discardableResult
    mutating func add(_ observer: Observer) -> UUID {
        let identifier = UUID()
        observers[identifier] = observer
        return identifier
    }

    mutating func remove(_ identifier: UUID) {
        observers.removeValue(forKey: identifier)
    }

    func forEach(_ body: (Observer) -> Void) {
        observers.values.forEach(body)
    }
}
