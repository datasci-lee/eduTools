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
    case zoom
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
    case toggleZoom
    case toggleLessonTimer
    case showBreakTimerSetup
    case toggleBoard(BoardKind)
    case previousBoardPage
    case nextBoardPage
    case selectTool(InkTool)
    case useLaser
    case selectOrCycleEraser
    case toggleEraserAreaShape
    case toggleShapeRecognition
    case selectColor(Int)
    case adjustWidth(Int)
    case undo
    case redo
    case clearAll
    case deleteSelection
    case togglePalette
    case stepBack
    case showHelp
    case adjustZoom(Int)
    case resetZoom
    case toggleZoomFreeze
    case stopZoom
    case beginZoomPreview
    case cycleZoomFollowMode

    var isBoardPageNavigation: Bool {
        switch self {
        case .previousBoardPage, .nextBoardPage:
            return true
        default:
            return false
        }
    }
}

enum ShortcutScope: String, Hashable {
    case global
    case annotation
    case zoom
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

    static func zoom(
        keyCode: Int,
        keyEquivalent: String,
        modifiers: NSEvent.ModifierFlags = [],
        keyCap: String
    ) -> ShortcutBinding {
        ShortcutBinding(
            scope: .zoom,
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
        var seen: Set<String> = []
        return bindings.compactMap { binding in
            seen.insert(binding.keyCap).inserted ? binding.keyCap : nil
        }.joined(separator: " / ")
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
                id: "zoom.toggle",
                name: "화면 줌 켜기/끄기",
                iconName: "plus.magnifyingglass",
                group: .screenBoardZoom,
                modes: [.global],
                command: .toggleZoom,
                bindings: [global("AppMod+Z")]
            ),
            ShortcutDefinition(
                id: "timer.lesson.toggle",
                name: "수업 타이머 시작/일시정지",
                iconName: "timer",
                group: .timerAndOther,
                modes: [.global],
                command: .toggleLessonTimer,
                bindings: [global("AppMod+M")]
            ),
            ShortcutDefinition(
                id: "timer.break.open",
                name: "쉬는 시간 타이머 열기",
                iconName: "cup.and.saucer",
                group: .timerAndOther,
                modes: [.global],
                command: .showBreakTimerSetup,
                bindings: [global("AppMod+B")]
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
                id: "tool.arrow",
                name: "화살표 (재입력: 한쪽/양쪽)",
                iconName: "arrow.right",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.arrow),
                bindings: [.annotation(keyCode: kVK_ANSI_A, keyEquivalent: "a", keyCap: "A")]
            ),
            ShortcutDefinition(
                id: "tool.rectangle",
                name: "사각형",
                iconName: "rectangle",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.rectangle),
                bindings: [.annotation(keyCode: kVK_ANSI_R, keyEquivalent: "r", keyCap: "R")]
            ),
            ShortcutDefinition(
                id: "tool.ellipse",
                name: "원",
                iconName: "circle",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.ellipse),
                bindings: [.annotation(keyCode: kVK_ANSI_O, keyEquivalent: "o", keyCap: "O")]
            ),
            ShortcutDefinition(
                id: "tool.text",
                name: "텍스트",
                iconName: "textformat",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.text),
                bindings: [.annotation(keyCode: kVK_ANSI_T, keyEquivalent: "t", keyCap: "T")]
            ),
            ShortcutDefinition(
                id: "tool.select",
                name: "올가미 선택·이동",
                iconName: "lasso",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.select),
                bindings: [.annotation(keyCode: kVK_ANSI_V, keyEquivalent: "v", keyCap: "V")]
            ),
            ShortcutDefinition(
                id: "tool.shape-recognition",
                name: "손그림 도형 자동 보정",
                iconName: "wand.and.stars",
                group: .tools,
                modes: [.annotation],
                command: .toggleShapeRecognition,
                bindings: []
            ),
            ShortcutDefinition(
                id: "eraser.area-shape",
                name: "영역 지우개 사각형/올가미",
                iconName: "rectangle.dashed",
                group: .tools,
                modes: [.annotation],
                command: .toggleEraserAreaShape,
                bindings: []
            ),
            ShortcutDefinition(
                id: "board.white",
                name: "화이트보드",
                iconName: "rectangle.fill",
                group: .screenBoardZoom,
                modes: [.annotation],
                command: .toggleBoard(.white),
                bindings: [.annotation(keyCode: kVK_ANSI_W, keyEquivalent: "w", keyCap: "W")]
            ),
            ShortcutDefinition(
                id: "board.black",
                name: "블랙보드",
                iconName: "rectangle.inset.filled",
                group: .screenBoardZoom,
                modes: [.annotation],
                command: .toggleBoard(.black),
                bindings: [.annotation(keyCode: kVK_ANSI_B, keyEquivalent: "b", keyCap: "B")]
            ),
            ShortcutDefinition(
                id: "board.page.previous",
                name: "이전 보드 페이지",
                iconName: "chevron.left",
                group: .screenBoardZoom,
                modes: [.annotation],
                command: .previousBoardPage,
                bindings: [
                    .annotation(keyCode: kVK_LeftArrow, keyEquivalent: "←", keyCap: "←")
                ]
            ),
            ShortcutDefinition(
                id: "board.page.next",
                name: "다음 보드 페이지",
                iconName: "chevron.right",
                group: .screenBoardZoom,
                modes: [.annotation],
                command: .nextBoardPage,
                bindings: [
                    .annotation(keyCode: kVK_RightArrow, keyEquivalent: "→", keyCap: "→")
                ]
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
                id: "edit.delete-selection",
                name: "선택한 항목 삭제",
                iconName: "delete.left",
                group: .editing,
                modes: [.annotation],
                command: .deleteSelection,
                bindings: [
                    .annotation(
                        keyCode: kVK_Delete,
                        keyEquivalent: "\u{8}",
                        keyCap: "⌫"
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
                id: "zoom.increase",
                name: "줌 배율 올리기",
                iconName: "plus.magnifyingglass",
                group: .screenBoardZoom,
                modes: [.zoom],
                command: .adjustZoom(1),
                bindings: [
                    .zoom(
                        keyCode: kVK_ANSI_Equal,
                        keyEquivalent: "+",
                        modifiers: .shift,
                        keyCap: "+"
                    ),
                ]
            ),
            ShortcutDefinition(
                id: "zoom.decrease",
                name: "줌 배율 내리기",
                iconName: "minus.magnifyingglass",
                group: .screenBoardZoom,
                modes: [.zoom],
                command: .adjustZoom(-1),
                bindings: [
                    .zoom(keyCode: kVK_ANSI_Minus, keyEquivalent: "-", keyCap: "-")
                ]
            ),
            ShortcutDefinition(
                id: "zoom.reset",
                name: "시작 배율로 되돌리기",
                iconName: "1.magnifyingglass",
                group: .screenBoardZoom,
                modes: [.zoom],
                command: .resetZoom,
                bindings: [.zoom(keyCode: kVK_ANSI_0, keyEquivalent: "0", keyCap: "0")]
            ),
            ShortcutDefinition(
                id: "zoom.freeze",
                name: "화면 멈추고 판서",
                iconName: "pause.circle",
                group: .screenBoardZoom,
                modes: [.zoom],
                command: .toggleZoomFreeze,
                bindings: [.zoom(keyCode: kVK_ANSI_P, keyEquivalent: "p", keyCap: "P")]
            ),
            ShortcutDefinition(
                id: "zoom.preview",
                name: "확대 위치 미리보기",
                iconName: "rectangle.dashed",
                group: .screenBoardZoom,
                modes: [.zoom],
                command: .beginZoomPreview,
                bindings: [.zoom(keyCode: kVK_Space, keyEquivalent: " ", keyCap: "Space")]
            ),
            ShortcutDefinition(
                id: "zoom.follow",
                name: "따라가기 방식 바꾸기",
                iconName: "cursorarrow.motionlines",
                group: .screenBoardZoom,
                modes: [.zoom],
                command: .cycleZoomFollowMode,
                bindings: [.zoom(keyCode: kVK_ANSI_F, keyEquivalent: "f", keyCap: "F")]
            ),
            ShortcutDefinition(
                id: "zoom.stop",
                name: "줌 종료",
                iconName: "escape",
                group: .screenBoardZoom,
                modes: [.zoom],
                command: .stopZoom,
                bindings: [.zoom(keyCode: kVK_Escape, keyEquivalent: "\u{1b}", keyCap: "Esc")]
            ),
            ShortcutDefinition(
                id: "help.show",
                name: "도움말",
                iconName: "questionmark.circle",
                group: .timerAndOther,
                modes: [.global, .annotation, .zoom],
                command: .showHelp,
                bindings: [
                    global("AppMod+/"),
                    .annotation(
                        keyCode: kVK_ANSI_Slash,
                        keyEquivalent: "/",
                        modifiers: .shift,
                        keyCap: "?"
                    ),
                    .zoom(
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

