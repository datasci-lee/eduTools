import AppKit
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

    var keyEquivalent: String {
        keyToken.lowercased()
    }
}

enum AppMode: String, Hashable {
    case global
    case annotation
}

enum ShortcutGroup: Int, CaseIterable {
    case tools
    case editing
    case screenBoardZoom
    case timerAndOther

    var displayName: String {
        switch self {
        case .tools: return "도구"
        case .editing: return "편집"
        case .screenBoardZoom: return "화면·보드·줌"
        case .timerAndOther: return "타이머·기타"
        }
    }
}

enum AppCommand: Hashable {
    case toggleAnnotationMode
    case selectTool(InkTool)
    case useLaser
    case selectOrCycleEraser
    case selectColor(Int)
    case adjustWidth(Int)
    case undo
    case redo
    case clearAll
    case togglePalette
    case stepBack
    case showHelp
}

enum ShortcutScope: String, Hashable {
    case global
    case annotation
}

struct ShortcutBinding {
    let scope: ShortcutScope
    let keyCode: UInt16
    let modifiers: NSEvent.ModifierFlags
    let keyEquivalent: String
    let keyCap: String
    let serializedGlobalShortcut: String?

    static func global(_ serialized: String) -> ShortcutBinding? {
        guard let shortcut = Shortcut(serialized: serialized) else { return nil }
        return ShortcutBinding(
            scope: .global,
            keyCode: UInt16(shortcut.keyCode),
            modifiers: [.control, .option],
            keyEquivalent: shortcut.keyEquivalent,
            keyCap: shortcut.displayString,
            serializedGlobalShortcut: serialized
        )
    }

    static func annotation(
        keyCode: Int,
        keyEquivalent: String,
        modifiers: NSEvent.ModifierFlags = [],
        keyCap: String
    ) -> ShortcutBinding {
        ShortcutBinding(
            scope: .annotation,
            keyCode: UInt16(keyCode),
            modifiers: modifiers,
            keyEquivalent: keyEquivalent,
            keyCap: keyCap,
            serializedGlobalShortcut: nil
        )
    }

    var uniquenessKey: String {
        "\(scope.rawValue):\(keyCode):\(modifiers.rawValue)"
    }

    func matches(_ event: NSEvent) -> Bool {
        let relevantModifiers: NSEvent.ModifierFlags = [.command, .shift, .control, .option]
        return event.keyCode == keyCode
            && event.modifierFlags.intersection(relevantModifiers) == modifiers
    }
}

struct ShortcutDefinition {
    let id: String
    let name: String
    let iconName: String
    let group: ShortcutGroup
    let modes: Set<AppMode>
    let command: AppCommand
    let bindings: [ShortcutBinding]

    var displayKeys: String {
        bindings.map(\.keyCap).joined(separator: " / ")
    }

    func binding(in scope: ShortcutScope) -> ShortcutBinding? {
        bindings.first { $0.scope == scope }
    }
}

enum ShortcutCatalog {
    static let definitions: [ShortcutDefinition] = {
        func global(_ serialized: String) -> ShortcutBinding {
            guard let binding = ShortcutBinding.global(serialized) else {
                preconditionFailure("Invalid built-in shortcut: \(serialized)")
            }
            return binding
        }

        var result: [ShortcutDefinition] = [
            ShortcutDefinition(
                id: "annotation.toggle",
                name: "판서 모드 켜기/끄기",
                iconName: "pencil",
                group: .screenBoardZoom,
                modes: [.global],
                command: .toggleAnnotationMode,
                bindings: [global("AppMod+P")]
            ),
            ShortcutDefinition(
                id: "tool.pen",
                name: "펜",
                iconName: "pencil.tip",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.pen),
                bindings: [.annotation(keyCode: kVK_ANSI_P, keyEquivalent: "p", keyCap: "P")]
            ),
            ShortcutDefinition(
                id: "tool.highlighter",
                name: "형광펜",
                iconName: "highlighter",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.highlighter),
                bindings: [.annotation(keyCode: kVK_ANSI_H, keyEquivalent: "h", keyCap: "H")]
            ),
            ShortcutDefinition(
                id: "tool.laser",
                name: "레이저",
                iconName: "laser.burst",
                group: .tools,
                modes: [.global, .annotation],
                command: .useLaser,
                bindings: [
                    global("AppMod+L"),
                    .annotation(keyCode: kVK_ANSI_L, keyEquivalent: "l", keyCap: "L"),
                ]
            ),
            ShortcutDefinition(
                id: "tool.eraser",
                name: "지우개",
                iconName: "eraser",
                group: .tools,
                modes: [.annotation],
                command: .selectOrCycleEraser,
                bindings: [.annotation(keyCode: kVK_ANSI_E, keyEquivalent: "e", keyCap: "E")]
            ),
            ShortcutDefinition(
                id: "tool.width.decrease",
                name: "현재 도구 크기 줄이기",
                iconName: "minus",
                group: .tools,
                modes: [.annotation],
                command: .adjustWidth(-1),
                bindings: [
                    .annotation(keyCode: kVK_ANSI_LeftBracket, keyEquivalent: "[", keyCap: "[")
                ]
            ),
            ShortcutDefinition(
                id: "tool.width.increase",
                name: "현재 도구 크기 키우기",
                iconName: "plus",
                group: .tools,
                modes: [.annotation],
                command: .adjustWidth(1),
                bindings: [
                    .annotation(keyCode: kVK_ANSI_RightBracket, keyEquivalent: "]", keyCap: "]")
                ]
            ),
            ShortcutDefinition(
                id: "edit.undo",
                name: "실행 취소",
                iconName: "arrow.uturn.backward",
                group: .editing,
                modes: [.annotation],
                command: .undo,
                bindings: [
                    .annotation(
                        keyCode: kVK_ANSI_Z,
                        keyEquivalent: "z",
                        modifiers: .command,
                        keyCap: "⌘Z"
                    )
                ]
            ),
            ShortcutDefinition(
                id: "edit.redo",
                name: "다시 실행",
                iconName: "arrow.uturn.forward",
                group: .editing,
                modes: [.annotation],
                command: .redo,
                bindings: [
                    .annotation(
                        keyCode: kVK_ANSI_Z,
                        keyEquivalent: "z",
                        modifiers: [.command, .shift],
                        keyCap: "⇧⌘Z"
                    )
                ]
            ),
            ShortcutDefinition(
                id: "edit.clear-all",
                name: "전체 지우기",
                iconName: "trash",
                group: .editing,
                modes: [.annotation],
                command: .clearAll,
                bindings: [
                    .annotation(
                        keyCode: kVK_Delete,
                        keyEquivalent: "\u{8}",
                        modifiers: .command,
                        keyCap: "⌘⌫"
                    )
                ]
            ),
            ShortcutDefinition(
                id: "palette.toggle",
                name: "도구 팔레트 숨기기/보이기",
                iconName: "rectangle.on.rectangle",
                group: .screenBoardZoom,
                modes: [.annotation],
                command: .togglePalette,
                bindings: [.annotation(keyCode: kVK_Tab, keyEquivalent: "\t", keyCap: "Tab")]
            ),
            ShortcutDefinition(
                id: "annotation.step-back",
                name: "한 단계 나가기",
                iconName: "escape",
                group: .screenBoardZoom,
                modes: [.annotation],
                command: .stepBack,
                bindings: [.annotation(keyCode: kVK_Escape, keyEquivalent: "\u{1b}", keyCap: "Esc")]
            ),
            ShortcutDefinition(
                id: "help.show",
                name: "도움말",
                iconName: "questionmark.circle",
                group: .timerAndOther,
                modes: [.global, .annotation],
                command: .showHelp,
                bindings: [
                    global("AppMod+/"),
                    .annotation(
                        keyCode: kVK_ANSI_Slash,
                        keyEquivalent: "/",
                        modifiers: .shift,
                        keyCap: "?"
                    ),
                ]
            ),
        ]

        let colorKeyCodes = [
            kVK_ANSI_1,
            kVK_ANSI_2,
            kVK_ANSI_3,
            kVK_ANSI_4,
            kVK_ANSI_5,
            kVK_ANSI_6,
        ]
        for (index, keyCode) in colorKeyCodes.enumerated() {
            let key = String(index + 1)
            result.append(
                ShortcutDefinition(
                    id: "color.\(index + 1)",
                    name: "색상 \(index + 1)",
                    iconName: "circle.fill",
                    group: .tools,
                    modes: [.annotation],
                    command: .selectColor(index),
                    bindings: [
                        .annotation(keyCode: keyCode, keyEquivalent: key, keyCap: key)
                    ]
                )
            )
        }
        return result
    }()

    static var globalDefinitions: [(ShortcutDefinition, ShortcutBinding)] {
        definitions.flatMap { definition in
            definition.bindings.compactMap { binding in
                binding.scope == .global ? (definition, binding) : nil
            }
        }
    }

    static func matching(_ event: NSEvent, scope: ShortcutScope) -> ShortcutDefinition? {
        definitions.first { definition in
            definition.bindings.contains { binding in
                binding.scope == scope && binding.matches(event)
            }
        }
    }

    static func matchingKeyUp(_ event: NSEvent, scope: ShortcutScope) -> ShortcutDefinition? {
        definitions.first { definition in
            definition.bindings.contains { binding in
                binding.scope == scope && binding.keyCode == event.keyCode
            }
        }
    }

    static func definition(for command: AppCommand) -> ShortcutDefinition? {
        definitions.first { $0.command == command }
    }

    static func filtered(_ definitions: [ShortcutDefinition], query: String) -> [ShortcutDefinition] {
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuery.isEmpty else { return definitions }
        return definitions.filter { definition in
            definition.name.localizedCaseInsensitiveContains(trimmedQuery)
                || definition.bindings.contains {
                    $0.keyCap.localizedCaseInsensitiveContains(trimmedQuery)
                }
        }
    }
}

