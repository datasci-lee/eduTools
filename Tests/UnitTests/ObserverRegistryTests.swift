import Foundation
import Testing

@testable import EduTools

@Suite("관찰자 레지스트리")
struct ObserverRegistryTests {
  @Test("관찰자는 고유 식별자로 등록되고 해제된 관찰자는 통지받지 않는다")
  func addNotifyAndRemoveObservers() {
    var registry = ObserverRegistry<(String) -> Void>()
    var notifications: [String] = []
    let firstIdentifier = registry.add { notifications.append("first:\($0)") }
    let secondIdentifier = registry.add { notifications.append("second:\($0)") }

    #expect(firstIdentifier != secondIdentifier)
    registry.forEach { $0("initial") }
    #expect(Set(notifications) == ["first:initial", "second:initial"])

    notifications.removeAll()
    registry.remove(firstIdentifier)
    registry.remove(UUID())
    registry.forEach { $0("remaining") }
    #expect(notifications == ["second:remaining"])
  }
}
