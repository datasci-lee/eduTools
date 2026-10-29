import Foundation

public final class FakeDisplayLayer<Operation: Equatable, Option: Equatable, Failure: Error> {
  public struct Call: Equatable {
    public let operations: [Operation]
    public let option: Option

    public init(operations: [Operation], option: Option) {
      self.operations = operations
      self.option = option
    }
  }

  public private(set) var calls: [Call] = []
  public var callObserver: ((Call) -> Void)?

  private var queuedResults: [Result<Void, Failure>] = []

  public init() {}

  public func enqueue(_ result: Result<Void, Failure>) {
    queuedResults.append(result)
  }

  public func apply(
    _ operations: [Operation],
    option: Option
  ) -> Result<Void, Failure> {
    let call = Call(operations: operations, option: option)
    calls.append(call)
    callObserver?(call)
    return queuedResults.isEmpty ? .success(()) : queuedResults.removeFirst()
  }
}
