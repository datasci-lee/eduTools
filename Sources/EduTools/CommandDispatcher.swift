import AppKit
import Foundation

final class AppCommandDispatcher {
    private let state: AppState
    weak var paletteController: PaletteController?
    weak var radialPaletteController: RadialPaletteController?
    weak var helpController: HelpController?
    weak var overlayController: OverlayController?
    weak var timerCoordinator: TimerCoordinator?
    weak var zoomController: ZoomController?
    weak var presenterEffectsController: PresenterEffectsController?
    weak var exportController: ExportController?
    weak var displayApplier: DisplayApplier?

    init(state: AppState) {
        self.state = state
    }

    static func supports(_ command: AppCommand) -> Bool {
        switch command {
        case .toggleAnnotationMode,
             .togglePassThroughMode,
             .toggleZoom,
             .toggleSpotlight,
             .toggleCursorHighlight,
             .toggleKeystrokeDisplay,
             .toggleHelpSharing,
             .applyDisplayPreset,
             .toggleLessonTimer,
             .showBreakTimerSetup,
             .toggleBoard,
             .previousBoardPage,
             .nextBoardPage,
             .selectTool,
             .useLaser,
             .selectOrCycleEraser,
             .toggleEraserTool,
             .toggleEraserAreaShape,
             .toggleShapeRecognition,
             .selectColor,
             .adjustWidth,
             .undo,
             .redo,
             .clearAll,
             .deleteSelection,
             .saveCurrentContent,
             .copyCurrentImage,
             .togglePalette,
             .toggleRadialPalette,
             .stepBack,
             .showHelp,
             .adjustZoom,
             .resetZoom,
             .toggleZoomFreeze,
             .stopZoom,
             .beginZoomPreview,
             .cycleZoomFollowMode:
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
            helpController?.shortcutDidPress(
                keyCode: definition.binding(in: scope)?.keyCode
            )
            return
        }
        guard !isRepeat else { return }
        if definition.command == .togglePassThroughMode {
            guard scope == .annotation,
                  let binding = definition.binding(in: scope),
                  binding.modifiers.isEmpty
            else {
                return
            }
            if let displayUUID {
                overlayController?.finishActiveInteraction(on: displayUUID)
            } else {
                overlayController?.finishAllActiveInteractions()
            }
            state.beginKeyPassThrough(keyCode: binding.keyCode)
            return
        }
        execute(definition.command, scope: scope, displayUUID: displayUUID)
    }

    func keyUp(definition: ShortcutDefinition) {
        switch definition.command {
        case .showHelp:
            helpController?.shortcutDidRelease()
        case .beginZoomPreview:
            zoomController?.finishPreview()
        case .togglePassThroughMode:
            guard let binding = definition.binding(in: .annotation),
                  binding.modifiers.isEmpty
            else {
                return
            }
            state.endKeyPassThrough(keyCode: binding.keyCode)
        default:
            break
        }
    }

    func shouldConsume(_ command: AppCommand) -> Bool {
        switch command {
        case .toggleRadialPalette:
            return radialPaletteController?.canToggleExpansionFromKeyboard == true
        default:
            return true
        }
    }

    func execute(
        _ command: AppCommand,
        scope: ShortcutScope = .annotation,
        displayUUID: String? = nil
    ) {
        switch command {
        case .toggleAnnotationMode:
            overlayController?.finishAllActiveInteractions()
            state.toggleAnnotationMode()
        case .togglePassThroughMode:
            overlayController?.finishAllActiveInteractions()
            state.togglePassThroughMode()
        case .toggleZoom:
            zoomController?.toggleZoom()
        case .toggleSpotlight:
            state.toggleSpotlight()
        case .toggleCursorHighlight:
            state.toggleCursorHighlight()
        case .toggleKeystrokeDisplay:
            presenterEffectsController?.toggleKeystrokeDisplay()
        case .toggleHelpSharing:
            state.toggleHelpSharing()
        case let .applyDisplayPreset(index):
            displayApplier?.applyPreset(at: index)
        case .toggleLessonTimer:
            timerCoordinator?.toggleLessonTimer()
        case .showBreakTimerSetup:
            timerCoordinator?.openBreakTimerSetup()
        case let .toggleBoard(kind):
            guard let targetDisplayUUID = overlayController?.displayUUIDUnderMouse()
                ?? displayUUID
                ?? state.activeDisplayUUID
            else {
                NSLog("EduTools: board command could not find a target display")
                return
            }
            state.toggleBoard(kind, on: targetDisplayUUID)
            overlayController?.focusPanelForBoard(on: targetDisplayUUID)
        case .previousBoardPage:
            if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID {
                _ = state.moveToPreviousBoardPage(on: targetDisplayUUID)
            }
        case .nextBoardPage:
            if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID {
                _ = state.moveToNextBoardPage(on: targetDisplayUUID)
            }
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
        case .toggleEraserTool:
            state.toggleEraserTool()
        case .toggleEraserAreaShape:
            state.toggleEraserAreaShape()
        case .toggleShapeRecognition:
            state.toggleShapeRecognition()
        case let .selectColor(index):
            state.selectColor(at: index)
        case let .adjustWidth(delta):
            if presenterEffectsController?.adjustSpotlightRadius(by: delta) != true {
                state.adjustCurrentToolWidth(by: CGFloat(delta))
            }
        case .undo:
            if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID {
                _ = state.undo(on: targetDisplayUUID)
            }
        case .redo:
            if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID {
                _ = state.redo(on: targetDisplayUUID)
            }
        case .clearAll:
            overlayController?.finishAllActiveInteractions()
            _ = state.clearAllInk()
        case .deleteSelection:
            if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID {
                _ = state.deleteSelection(on: targetDisplayUUID)
            }
        case .saveCurrentContent:
            exportController?.saveCurrentContent()
        case .copyCurrentImage:
            exportController?.copyCurrentImage()
        case .togglePalette:
            state.togglePaletteVisibility()
        case .toggleRadialPalette:
            radialPaletteController?.toggleExpansionFromKeyboard()
        case .stepBack:
            if state.isPassThroughModeEnabled {
                state.setPassThroughModeEnabled(false)
                return
            } else if radialPaletteController?.collapseIfExpanded() == true {
                return
            } else if helpController?.isVisible == true {
                helpController?.close()
            } else if let targetDisplayUUID = displayUUID ?? state.activeDisplayUUID,
                      state.clearSelection(on: targetDisplayUUID)
            {
                return
            } else if state.currentTool != .pen {
                state.selectTool(.pen)
            } else {
                overlayController?.finishAllActiveInteractions()
                state.setAnnotationModeEnabled(false)
            }
        case .showHelp:
            helpController?.showPinned()
        case let .adjustZoom(steps):
            zoomController?.adjustMagnification(by: steps)
        case .resetZoom:
            zoomController?.resetMagnification()
        case .toggleZoomFreeze:
            zoomController?.toggleFreeze()
        case .stopZoom:
            zoomController?.stopOrCancelPreview()
        case .beginZoomPreview:
            zoomController?.beginPreview()
        case .cycleZoomFollowMode:
            zoomController?.cycleFollowMode()
        }
    }

    @discardableResult
    func forwardToUnderlyingApplication(_ event: NSEvent) -> Bool {
        overlayController?.forwardToUnderlyingApplication(event) ?? false
    }
}
