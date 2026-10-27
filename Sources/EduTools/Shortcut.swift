import Carbon

struct Shortcut: Equatable, Hashable {
    static let appModifierCarbonFlags = UInt32(controlKey) | UInt32(optionKey)

    private static let keyCodes: [String: UInt32] = [
        "B": UInt32(kVK_ANSI_B),
        "L": UInt32(kVK_ANSI_L),
        "M": UInt32(kVK_ANSI_M),
        "P": UInt32(kVK_ANSI_P),
        "S": UInt32(kVK_ANSI_S),
        "Z": UInt32(kVK_ANSI_Z),
        "/": UInt32(kVK_ANSI_Slash),
    ]

    let keyCode: UInt32
    let carbonModifiers: UInt32
    private let keyToken: String

    init?(serialized: String) {
        let components = serialized.split(separator: "+", omittingEmptySubsequences: false)
        guard components.count == 2,
              components[0] == "AppMod",
              let keyCode = Self.keyCodes[String(components[1])]
        else {
            return nil
        }

        self.keyCode = keyCode
        carbonModifiers = Self.appModifierCarbonFlags
        keyToken = String(components[1])
    }

    init?(keyCode: UInt32, carbonModifiers: UInt32) {
        guard carbonModifiers == Self.appModifierCarbonFlags,
              let keyToken = Self.keyCodes.first(where: { $0.value == keyCode })?.key
        else {
            return nil
        }

        self.keyCode = keyCode
        self.carbonModifiers = carbonModifiers
        self.keyToken = keyToken
    }

    var serialized: String {
        "AppMod+\(keyToken)"
    }

    var displayString: String {
        "⌃⌥\(keyToken)"
    }
}

