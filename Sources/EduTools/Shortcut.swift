import AppKit
import Carbon

struct PortableShortcut: Equatable {
    struct KeyDescriptor: Equatable, Hashable {
        let token: String
        let keyCode: UInt16
        let keyEquivalent: String
        let keyCap: String
    }

    private static let keys: [KeyDescriptor] = {
        let letters: [(String, Int)] = [
            ("A", kVK_ANSI_A), ("B", kVK_ANSI_B), ("C", kVK_ANSI_C),
            ("D", kVK_ANSI_D), ("E", kVK_ANSI_E), ("F", kVK_ANSI_F),
            ("G", kVK_ANSI_G), ("H", kVK_ANSI_H), ("I", kVK_ANSI_I),
            ("J", kVK_ANSI_J), ("K", kVK_ANSI_K), ("L", kVK_ANSI_L),
            ("M", kVK_ANSI_M), ("N", kVK_ANSI_N), ("O", kVK_ANSI_O),
            ("P", kVK_ANSI_P), ("Q", kVK_ANSI_Q), ("R", kVK_ANSI_R),
            ("S", kVK_ANSI_S), ("T", kVK_ANSI_T), ("U", kVK_ANSI_U),
            ("V", kVK_ANSI_V), ("W", kVK_ANSI_W), ("X", kVK_ANSI_X),
            ("Y", kVK_ANSI_Y), ("Z", kVK_ANSI_Z),
        ]
        let digits: [(String, Int)] = [
            ("0", kVK_ANSI_0), ("1", kVK_ANSI_1), ("2", kVK_ANSI_2),
            ("3", kVK_ANSI_3), ("4", kVK_ANSI_4), ("5", kVK_ANSI_5),
            ("6", kVK_ANSI_6), ("7", kVK_ANSI_7), ("8", kVK_ANSI_8),
            ("9", kVK_ANSI_9),
        ]
        var result = letters.map {
            KeyDescriptor(token: $0.0, keyCode: UInt16($0.1), keyEquivalent: $0.0.lowercased(), keyCap: $0.0)
        }
        result += digits.map {
            KeyDescriptor(token: $0.0, keyCode: UInt16($0.1), keyEquivalent: $0.0, keyCap: $0.0)
        }
        result += [
            KeyDescriptor(token: "/", keyCode: UInt16(kVK_ANSI_Slash), keyEquivalent: "/", keyCap: "/"),
            KeyDescriptor(token: "[", keyCode: UInt16(kVK_ANSI_LeftBracket), keyEquivalent: "[", keyCap: "["),
            KeyDescriptor(token: "]", keyCode: UInt16(kVK_ANSI_RightBracket), keyEquivalent: "]", keyCap: "]"),
            KeyDescriptor(token: "=", keyCode: UInt16(kVK_ANSI_Equal), keyEquivalent: "=", keyCap: "="),
            KeyDescriptor(token: "-", keyCode: UInt16(kVK_ANSI_Minus), keyEquivalent: "-", keyCap: "-"),
            KeyDescriptor(token: "Left", keyCode: UInt16(kVK_LeftArrow), keyEquivalent: "←", keyCap: "←"),
            KeyDescriptor(token: "Right", keyCode: UInt16(kVK_RightArrow), keyEquivalent: "→", keyCap: "→"),
            KeyDescriptor(token: "Up", keyCode: UInt16(kVK_UpArrow), keyEquivalent: "↑", keyCap: "↑"),
            KeyDescriptor(token: "Down", keyCode: UInt16(kVK_DownArrow), keyEquivalent: "↓", keyCap: "↓"),
            KeyDescriptor(token: "Delete", keyCode: UInt16(kVK_Delete), keyEquivalent: "\u{8}", keyCap: "⌫"),
            KeyDescriptor(token: "Tab", keyCode: UInt16(kVK_Tab), keyEquivalent: "\t", keyCap: "Tab"),
            KeyDescriptor(token: "Escape", keyCode: UInt16(kVK_Escape), keyEquivalent: "\u{1b}", keyCap: "Esc"),
            KeyDescriptor(token: "Space", keyCode: UInt16(kVK_Space), keyEquivalent: " ", keyCap: "Space"),
            KeyDescriptor(token: "Return", keyCode: UInt16(kVK_Return), keyEquivalent: "\r", keyCap: "Return"),
        ]
        return result
    }()

    let key: KeyDescriptor
    let usesAppModifier: Bool
    let modifiers: NSEvent.ModifierFlags

    init?(serialized: String) {
        let components = serialized.split(separator: "+", omittingEmptySubsequences: false).map(String.init)
        guard let token = components.last,
              let key = Self.keys.first(where: { $0.token == token })
        else {
            return nil
        }
        let modifierTokens = Array(components.dropLast())
        guard Set(modifierTokens).count == modifierTokens.count else { return nil }
        let allowed = Set(["AppMod", "Primary", "Shift", "Control", "Option"])
        guard modifierTokens.allSatisfy(allowed.contains),
              !(modifierTokens.contains("AppMod") && modifierTokens.count > 1)
        else {
            return nil
        }
        var modifiers: NSEvent.ModifierFlags = []
        if modifierTokens.contains("Primary") { modifiers.insert(.command) }
        if modifierTokens.contains("Shift") { modifiers.insert(.shift) }
        if modifierTokens.contains("Control") { modifiers.insert(.control) }
        if modifierTokens.contains("Option") { modifiers.insert(.option) }
        self.key = key
        usesAppModifier = modifierTokens.contains("AppMod")
        self.modifiers = modifiers
    }

    var serialized: String {
        var tokens: [String] = []
        if usesAppModifier {
            tokens.append("AppMod")
        } else {
            if modifiers.contains(.control) { tokens.append("Control") }
            if modifiers.contains(.option) { tokens.append("Option") }
            if modifiers.contains(.shift) { tokens.append("Shift") }
            if modifiers.contains(.command) { tokens.append("Primary") }
        }
        tokens.append(key.token)
        return tokens.joined(separator: "+")
    }

    func eventModifiers(appModifier: AppModifierSetting) -> NSEvent.ModifierFlags {
        guard usesAppModifier else { return modifiers }
        switch appModifier {
        case .controlOption: return [.control, .option]
        case .controlOptionShift: return [.control, .option, .shift]
        }
    }

    func displayString(appModifier: AppModifierSetting) -> String {
        let flags = eventModifiers(appModifier: appModifier)
        var prefix = ""
        if flags.contains(.control) { prefix += "⌃" }
        if flags.contains(.option) { prefix += "⌥" }
        if flags.contains(.shift) { prefix += "⇧" }
        if flags.contains(.command) { prefix += "⌘" }
        return prefix + key.keyCap
    }

    static func keyToken(for keyCode: UInt16) -> String? {
        keys.first(where: { $0.keyCode == keyCode })?.token
    }

    static func serialized(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> String? {
        guard let token = keyToken(for: keyCode) else { return nil }
        let relevant: NSEvent.ModifierFlags = [.command, .shift, .control, .option]
        let flags = modifiers.intersection(relevant)
        var tokens: [String] = []
        if flags.contains(.control) { tokens.append("Control") }
        if flags.contains(.option) { tokens.append("Option") }
        if flags.contains(.shift) { tokens.append("Shift") }
        if flags.contains(.command) { tokens.append("Primary") }
        tokens.append(token)
        return tokens.joined(separator: "+")
    }
}

struct Shortcut: Equatable, Hashable {
    static let appModifierCarbonFlags = UInt32(controlKey) | UInt32(optionKey)

    let keyCode: UInt32
    let carbonModifiers: UInt32
    private let keyToken: String
    private let appModifier: AppModifierSetting

    init?(serialized: String, appModifier: AppModifierSetting = .controlOption) {
        guard let portable = PortableShortcut(serialized: serialized),
              portable.usesAppModifier
        else {
            return nil
        }

        keyCode = UInt32(portable.key.keyCode)
        switch appModifier {
        case .controlOption:
            carbonModifiers = Self.appModifierCarbonFlags
        case .controlOptionShift:
            carbonModifiers = Self.appModifierCarbonFlags | UInt32(shiftKey)
        }
        keyToken = portable.key.token
        self.appModifier = appModifier
    }

    init?(keyCode: UInt32, carbonModifiers: UInt32) {
        guard carbonModifiers == Self.appModifierCarbonFlags,
              let keyToken = PortableShortcut.keyToken(for: UInt16(keyCode))
        else {
            return nil
        }

        self.keyCode = keyCode
        self.carbonModifiers = carbonModifiers
        self.keyToken = keyToken
        appModifier = .controlOption
    }

    var serialized: String {
        "AppMod+\(keyToken)"
    }

    var displayString: String {
        "\(appModifier.displayName)\(keyToken)"
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
    case toggleSpotlight
    case toggleCursorHighlight
    case toggleKeystrokeDisplay
    case toggleHelpSharing
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
    case saveCurrentContent
    case copyCurrentImage
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
    let portableString: String

    static func global(
        _ serialized: String,
        appModifier: AppModifierSetting = .controlOption
    ) -> ShortcutBinding? {
        guard let shortcut = Shortcut(serialized: serialized, appModifier: appModifier) else {
            return nil
        }
        return ShortcutBinding(
            scope: .global,
            keyCode: UInt16(shortcut.keyCode),
            modifiers: PortableShortcut(serialized: serialized)?.eventModifiers(
                appModifier: appModifier
            ) ?? [],
            keyEquivalent: shortcut.keyEquivalent,
            keyCap: shortcut.displayString,
            serializedGlobalShortcut: serialized,
            portableString: serialized
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
            serializedGlobalShortcut: nil,
            portableString: PortableShortcut.serialized(
                keyCode: UInt16(keyCode),
                modifiers: modifiers
            ) ?? keyCap
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
            serializedGlobalShortcut: nil,
            portableString: PortableShortcut.serialized(
                keyCode: UInt16(keyCode),
                modifiers: modifiers
            ) ?? keyCap
        )
    }

    func replacing(
        portableString: String,
        appModifier: AppModifierSetting
    ) -> ShortcutBinding? {
        guard let portable = PortableShortcut(serialized: portableString) else { return nil }
        if scope == .global, !portable.usesAppModifier { return nil }
        if scope != .global, portable.usesAppModifier { return nil }
        let modifiers = portable.eventModifiers(appModifier: appModifier)
        return ShortcutBinding(
            scope: scope,
            keyCode: portable.key.keyCode,
            modifiers: modifiers,
            keyEquivalent: portable.key.keyEquivalent,
            keyCap: portable.displayString(appModifier: appModifier),
            serializedGlobalShortcut: scope == .global ? portable.serialized : nil,
            portableString: portable.serialized
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
    private static var settingsProvider: () -> ShortcutGroupSettings = {
        AppSettings.default.shortcuts
    }

    static func configure(settingsStore: SettingsStore) {
        settingsProvider = { [weak settingsStore] in
            settingsStore?.settings.shortcuts ?? AppSettings.default.shortcuts
        }
    }

    static func overrideKey(definitionID: String, scope: ShortcutScope) -> String {
        "\(definitionID)|\(scope.rawValue)"
    }

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
                id: "spotlight.toggle",
                name: "스포트라이트 켜기/끄기",
                iconName: "light.beacon.max",
                group: .screenBoardZoom,
                modes: [.global, .annotation],
                command: .toggleSpotlight,
                bindings: [global("AppMod+S")]
            ),
            ShortcutDefinition(
                id: "cursor-highlight.toggle",
                name: "커서 강조 켜기/끄기",
                iconName: "cursorarrow.rays",
                group: .screenBoardZoom,
                modes: [.global],
                command: .toggleCursorHighlight,
                bindings: []
            ),
            ShortcutDefinition(
                id: "keystroke-display.toggle",
                name: "키 입력 표시 켜기/끄기",
                iconName: "command",
                group: .screenBoardZoom,
                modes: [.global],
                command: .toggleKeystrokeDisplay,
                bindings: []
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
                id: "tool.cover",
                name: "화면 가리개",
                iconName: "rectangle.fill",
                group: .tools,
                modes: [.annotation],
                command: .selectTool(.cover),
                bindings: []
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
                id: "export.save",
                name: "PNG 저장 / 보드 PDF 저장",
                iconName: "square.and.arrow.down",
                group: .editing,
                modes: [.annotation],
                command: .saveCurrentContent,
                bindings: [
                    .annotation(
                        keyCode: kVK_ANSI_S,
                        keyEquivalent: "s",
                        modifiers: .command,
                        keyCap: "⌘S"
                    )
                ]
            ),
            ShortcutDefinition(
                id: "export.copy",
                name: "클립보드 복사",
                iconName: "doc.on.doc",
                group: .editing,
                modes: [.annotation],
                command: .copyCurrentImage,
                bindings: [
                    .annotation(
                        keyCode: kVK_ANSI_C,
                        keyEquivalent: "c",
                        modifiers: .command,
                        keyCap: "⌘C"
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

    static var activeDefinitions: [ShortcutDefinition] {
        resolvedDefinitions(for: settingsProvider())
    }

    static func resolvedDefinitions(for settings: ShortcutGroupSettings) -> [ShortcutDefinition] {
        definitions.map { definition in
            let bindings = definition.bindings.compactMap { binding -> ShortcutBinding? in
                let key = overrideKey(definitionID: definition.id, scope: binding.scope)
                let portable = settings.overrides[key] ?? binding.portableString
                guard let resolved = binding.replacing(
                    portableString: portable,
                    appModifier: settings.appModifier
                ) else {
                    NSLog("EduTools: ignored invalid shortcut override for %@", key)
                    return binding.replacing(
                        portableString: binding.portableString,
                        appModifier: settings.appModifier
                    )
                }
                return resolved
            }
            return ShortcutDefinition(
                id: definition.id,
                name: definition.name,
                iconName: definition.iconName,
                group: definition.group,
                modes: definition.modes,
                command: definition.command,
                bindings: bindings
            )
        }
    }

    static var globalDefinitions: [(ShortcutDefinition, ShortcutBinding)] {
        activeDefinitions.flatMap { definition in
            definition.bindings.compactMap { binding in
                binding.scope == .global ? (definition, binding) : nil
            }
        }
    }

    static func matching(_ event: NSEvent, scope: ShortcutScope) -> ShortcutDefinition? {
        activeDefinitions.first { definition in
            definition.bindings.contains { binding in
                binding.scope == scope && binding.matches(event)
            }
        }
    }

    static func definition(for command: AppCommand) -> ShortcutDefinition? {
        activeDefinitions.first { $0.command == command }
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

enum ShortcutConflictDetector {
    private static let avoidedAppModifierKeys: Set<String> = [
        "D", "F", "G", "E", "T", "U", "I", "J", "K", "C",
        "Left", "Right", "Up", "Down", "Return", "Delete",
    ]
    private static let avoidedPortableShortcuts: Set<String> = [
        "Option+Primary+8", "Option+Primary+=", "Option+Primary+-",
        "Control+Space", "Control+Primary+Space",
        "Shift+Primary+3", "Shift+Primary+4", "Shift+Primary+5",
        "Control+Left", "Control+Right", "Control+Up", "Control+Down",
        "Control+Option+Delete", "Control+Option+Tab",
        "Control+Option+Left", "Control+Option+Right",
        "Control+Option+Up", "Control+Option+Down",
    ]

    static func warnings(for settings: ShortcutGroupSettings) -> [String: String] {
        let definitions = ShortcutCatalog.resolvedDefinitions(for: settings)
        var warnings: [String: String] = [:]
        var bindingsByUniquenessKey: [String: [(definitionID: String, scope: ShortcutScope)]] = [:]

        for definition in definitions {
            for binding in definition.bindings {
                let key = ShortcutCatalog.overrideKey(
                    definitionID: definition.id,
                    scope: binding.scope
                )
                bindingsByUniquenessKey[binding.uniquenessKey, default: []].append(
                    (definition.id, binding.scope)
                )
                if isAvoided(binding.portableString) {
                    warnings[key] = "알려진 시스템·앱 단축키와 겹칠 수 있습니다. 대안: \(suggestion(for: definitions))"
                }
            }
        }

        for duplicates in bindingsByUniquenessKey.values where duplicates.count > 1 {
            for duplicate in duplicates {
                let key = ShortcutCatalog.overrideKey(
                    definitionID: duplicate.definitionID,
                    scope: duplicate.scope
                )
                warnings[key] = "같은 범위의 다른 기능과 중복됩니다. 대안: \(suggestion(for: definitions))"
            }
        }
        return warnings
    }

    static func hasAvoidedConflict(_ serialized: String) -> Bool {
        isAvoided(serialized)
    }

    private static func isAvoided(_ serialized: String) -> Bool {
        guard let portable = PortableShortcut(serialized: serialized) else { return false }
        if portable.usesAppModifier {
            return avoidedAppModifierKeys.contains(portable.key.token)
        }
        if avoidedPortableShortcuts.contains(portable.serialized) {
            return true
        }
        let flags = portable.modifiers
        if flags == .control, portable.key.token.first?.isNumber == true {
            return true
        }
        return flags == [.control, .shift]
            && portable.key.token.count == 1
            && portable.key.token.first?.isLetter == true
    }

    private static func suggestion(for definitions: [ShortcutDefinition]) -> String {
        let occupied = Set(definitions.flatMap(\.bindings).filter {
            $0.scope == .global
        }.map(\.portableString))
        let candidates = ["AppMod+P", "AppMod+Z", "AppMod+L", "AppMod+S", "AppMod+M", "AppMod+B", "AppMod+/"]
        return candidates.first(where: { !occupied.contains($0) }) ?? "사용하지 않는 AppMod+문자"
    }
}

