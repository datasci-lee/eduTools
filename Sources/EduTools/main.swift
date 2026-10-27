import AppKit
import Carbon
import Darwin

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let appState = AppState()
    private var statusMenu: StatusMenu?
    private var overlayController: OverlayController?
    private var paletteController: PaletteController?
    private var helpController: HelpController?
    private var timerCoordinator: TimerCoordinator?
    private var zoomController: ZoomController?
    private var presenterEffectsController: PresenterEffectsController?
    private var dispatcher: AppCommandDispatcher?
    private var hotkeyCenter: HotkeyCenter?

    func applicationDidFinishLaunching(_ notification: Notification) {
        _ = NSApp.setActivationPolicy(.accessory)

        let dispatcher = AppCommandDispatcher(state: appState)
        self.dispatcher = dispatcher

        let presenterEffectsController = PresenterEffectsController(state: appState)
        self.presenterEffectsController = presenterEffectsController
        dispatcher.presenterEffectsController = presenterEffectsController

        let helpController = HelpController(state: appState, dispatcher: dispatcher)
        self.helpController = helpController
        dispatcher.helpController = helpController

        let paletteController = PaletteController(state: appState, dispatcher: dispatcher)
        self.paletteController = paletteController
        dispatcher.paletteController = paletteController

        let timerCoordinator = TimerCoordinator()
        self.timerCoordinator = timerCoordinator
        dispatcher.timerCoordinator = timerCoordinator

        let statusMenu = StatusMenu(state: appState, dispatcher: dispatcher)
        self.statusMenu = statusMenu
        presenterEffectsController.setAccessibilityStatusHandler { [weak statusMenu] trusted in
            statusMenu?.setAccessibilityTrusted(trusted)
        }
        timerCoordinator.setLessonDisplayHandler { [weak statusMenu] title in
            statusMenu?.setLessonTimerDisplay(title)
        }
        let overlayController = OverlayController(state: appState, dispatcher: dispatcher)
        self.overlayController = overlayController
        dispatcher.overlayController = overlayController

        let zoomController = ZoomController(state: appState, dispatcher: dispatcher)
        self.zoomController = zoomController
        zoomController.overlayController = overlayController
        dispatcher.zoomController = zoomController

        let hotkeyCenter = HotkeyCenter()
        self.hotkeyCenter = hotkeyCenter

        for (definition, binding) in ShortcutCatalog.globalDefinitions {
            guard let serialized = binding.serializedGlobalShortcut,
                  let shortcut = Shortcut(serialized: serialized)
            else {
                let status = OSStatus(paramErr)
                NSLog(
                    "EduTools: invalid built-in shortcut %@ (OSStatus %d)",
                    definition.id,
                    status
                )
                statusMenu.setHotkeyRegistrationStatus(status)
                continue
            }

            let status = hotkeyCenter.register(
                shortcut: shortcut,
                pressed: { [weak dispatcher] in
                    dispatcher?.keyDown(definition: definition, scope: .global)
                },
                released: { [weak dispatcher] in
                    dispatcher?.keyUp(definition: definition)
                }
            )
            if status != noErr {
                NSLog(
                    "EduTools: failed to register global shortcut %@ (OSStatus %d)",
                    shortcut.serialized,
                    status
                )
            }
            statusMenu.setHotkeyRegistrationStatus(status)
        }

        DispatchQueue.main.async { [weak helpController] in
            helpController?.showFirstRunIfNeeded()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        zoomController?.stopZoom()
        timerCoordinator?.stopBreakMusicForTermination()
        hotkeyCenter?.invalidate()
    }
}

if CommandLine.arguments.contains("--selftest") {
    exit(runSelfTests())
}

let application = NSApplication.shared
let applicationDelegate = AppDelegate()
application.delegate = applicationDelegate
_ = application.setActivationPolicy(.accessory)
application.run()

