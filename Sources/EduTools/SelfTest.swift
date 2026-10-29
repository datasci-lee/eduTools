import Foundation
import AppKit

private struct SelfTestRunner {
    private(set) var passed = 0
    private(set) var failures: [String] = []

    mutating func expect(_ condition: @autoclosure () -> Bool, _ message: String) {
        if condition() {
            passed += 1
        } else {
            failures.append(message)
        }
    }
}

func runSelfTests() -> Int32 {
    var runner = SelfTestRunner()

    if runner.failures.isEmpty {
        print("SelfTest: \(runner.passed) passed, 0 failed")
        return 0
    }

    for failure in runner.failures {
        FileHandle.standardError.write(Data("SelfTest failure: \(failure)\n".utf8))
    }
    FileHandle.standardError.write(
        Data("SelfTest: \(runner.passed) passed, \(runner.failures.count) failed\n".utf8)
    )
    return 1
}
