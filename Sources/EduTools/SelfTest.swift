import Foundation
import Carbon

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

    let pen = Shortcut(serialized: "AppMod+P")
    runner.expect(pen != nil, "AppMod+P should parse")
    runner.expect(pen?.serialized == "AppMod+P", "AppMod+P should serialize unchanged")
    runner.expect(pen?.displayString == "⌃⌥P", "AppMod+P should use the macOS display form")
    runner.expect(pen?.keyCode == 35, "P should use physical keyCode 35")
    runner.expect(
        pen?.keyCode == UInt32(kVK_ANSI_P),
        "P should use the ANSI physical key constant"
    )
    runner.expect(
        pen?.carbonModifiers == Shortcut.appModifierCarbonFlags,
        "AppMod should map to Control+Option Carbon flags"
    )

    if let pen {
        let reconstructed = Shortcut(
            keyCode: pen.keyCode,
            carbonModifiers: pen.carbonModifiers
        )
        runner.expect(
            reconstructed?.serialized == "AppMod+P",
            "P should round-trip from Carbon values"
        )
    } else {
        runner.expect(false, "P Carbon round-trip could not run")
    }

    let help = Shortcut(serialized: "AppMod+/")
    runner.expect(help != nil, "AppMod+/ should parse")
    runner.expect(help?.serialized == "AppMod+/", "AppMod+/ should serialize unchanged")
    runner.expect(help?.keyCode == 44, "/ should use physical keyCode 44")

    if let help {
        let reconstructed = Shortcut(
            keyCode: help.keyCode,
            carbonModifiers: help.carbonModifiers
        )
        runner.expect(
            reconstructed?.serialized == "AppMod+/",
            "/ should round-trip from Carbon values"
        )
    } else {
        runner.expect(false, "/ Carbon round-trip could not run")
    }

    let invalidValues = [
        "",
        "P",
        "AppMod+",
        "AppMod+p",
        "AppMod+PP",
        "AppMod+P+Z",
        "Control+Option+P",
    ]
    for value in invalidValues {
        runner.expect(Shortcut(serialized: value) == nil, "\(value) should be rejected")
    }
    runner.expect(
        Shortcut(keyCode: UInt32(kVK_ANSI_P), carbonModifiers: 0) == nil,
        "P without AppMod should be rejected"
    )

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
