import Foundation

final class AppCommandDispatcher {
    private let state: AppState
    weak var paletteController: PaletteController?
    weak var helpController: HelpController?

    init(state: AppState) {
        self.state = state
    }

    static func supports(_ command: AppCommand) -> Bool {
        switch command {
        case .toggleAnnotationMode,
             .selectTool,
             .useLaser,
             .selectOrCycleEraser,
             .selectColor,
             .adjustWidth,
             .undo,
             .redo,
             .clearAll,
             .togglePalette,
             .stepBack,
             .showHelp:
            return true
        }
    }

    func keyDown(
        definition: ShortcutDefinition,
        scope: ShortcutScope,
        displayUUID: String? = nil,
        isRepeat: Bool = false
    ) {
        if definition.command == .showHelp {
            guard !isRepeat else { return }
            helpController?.shortcutDidPress()
            return
        }
        guard !isRepeat else { return }
        execute(definition.command, scope: scope, displayUUID: displayUUID)
    }

    func keyUp(definition: ShortcutDefinition) {
        guard definition.command == .showHelp else { return }
        helpController?.shortcutDidRelease()
    }

    func execute(
        _ command: AppCommand,
        scope: ShortcutScope = .annotation,
        displayUUID: String? = nil
    ) {
        switch command {
        case .toggleAnnotationMode:
            state.toggleAnnotationMode()
        case let .selectTool(tool):
            state.selectTool(tool)
        case .useLaser:
            if scope == .global {
                state.activateLaserShortcut()
            } else {
                state.selectTool(.laser)
            }
        case .selectOrCycleEraser:
            _ = state.selectOrCycleEraser()
        case let .selectColor(index):
            state.selectColor(at: index)
        case let .adjustWidth(delta):
            state.adjustCurrentToolWidth(by: CGFloat(delta))
        case .undo:
            if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID {
                _ = state.undo(on: targetDisplayUUID)
            }
        case .redo:
            if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID {
                _ = state.redo(on: targetDisplayUUID)
            }
        case .clearAll:
            _ = state.clearAllInk()
        case .togglePalette:
            state.togglePaletteVisibility()
        case .stepBack:
            if helpController?.isVisible == true {
                helpController?.close()
            } else if state.currentTool != .pen {
                state.selectTool(.pen)
            } else {
                state.setAnnotationModeEnabled(false)
            }
        case .showHelp:
            helpController?.showPinned()
        }
    }
}
