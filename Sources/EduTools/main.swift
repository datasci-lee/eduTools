import AppKit
import Carbon
import Darwin

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let settingsStore: SettingsStore
    private let appState: AppState
    private var statusMenu: StatusMenu?
    private var overlayController: OverlayController?
    private var paletteController: PaletteController?
    private var helpController: HelpController?
    private var timerCoordinator: TimerCoordinator?
    private var zoomController: ZoomController?
    private var presenterEffectsController: PresenterEffectsController?
    private var archiveController: ArchiveController?
    private var exportController: ExportController?
    private var dispatcher: AppCommandDispatcher?
    private var hotkeyCenter: HotkeyCenter?
    private var settingsController: SettingsController?
    private var settingsObserverIdentifier: UUID?
    private var lastShortcutSettings: ShortcutGroupSettings?

    override init() {
        let store = SettingsStore()
        settingsStore = store
        appState = AppState(settingsStore: store)
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        _ = NSApp.setActivationPolicy(.accessory)
        ShortcutCatalog.configure(settingsStore: settingsStore)

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

        let timerCoordinator = TimerCoordinator(settingsStore: settingsStore)
        self.timerCoordinator = timerCoordinator
        dispatcher.timerCoordinator = timerCoordinator

        let settingsController = SettingsController(settingsStore: settingsStore)
        self.settingsController = settingsController
        let statusMenu = StatusMenu(
            state: appState,
            dispatcher: dispatcher,
            settingsController: settingsController
        )
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

        let archiveController = ArchiveController(
            state: appState,
            overlayController: overlayController,
            settingsStore: settingsStore
        )
        self.archiveController = archiveController
        overlayController.archiveController = archiveController
        for displayUUID in overlayController.connectedDisplayUUIDs {
            archiveController.restoreDisconnectedDisplayIfNeeded(displayUUID)
        }

        let exportController = ExportController(
            state: appState,
            overlayController: overlayController
        )
        self.exportController = exportController
        dispatcher.exportController = exportController

        let zoomController = ZoomController(state: appState, dispatcher: dispatcher)
        self.zoomController = zoomController
        zoomController.overlayController = overlayController
        dispatcher.zoomController = zoomController

        registerGlobalShortcuts(dispatcher: dispatcher, statusMenu: statusMenu)
        lastShortcutSettings = settingsStore.settings.shortcuts
        settingsObserverIdentifier = settingsStore.addObserver { [weak self, weak dispatcher, weak statusMenu] settings in
            guard let self, settings.shortcuts != self.lastShortcutSettings,
                  let dispatcher, let statusMenu
            else {
                return
            }
            self.lastShortcutSettings = settings.shortcuts
            self.registerGlobalShortcuts(dispatcher: dispatcher, statusMenu: statusMenu)
        }

        DispatchQueue.main.async { [weak helpController, weak settingsController] in
            helpController?.showFirstRunIfNeeded()
            settingsController?.showFirstRunPermissionsIfNeeded()
            settingsController?.showVoiceOverWarningIfNeeded()
        }
    }

    private func registerGlobalShortcuts(
        dispatcher: AppCommandDispatcher,
        statusMenu: StatusMenu
    ) {
        hotkeyCenter?.invalidate()
        let hotkeyCenter = HotkeyCenter()
        self.hotkeyCenter = hotkeyCenter
        statusMenu.resetHotkeyRegistrationStatus()
        var warnings = ShortcutConflictDetector.warnings(for: settingsStore.settings.shortcuts)

        for (definition, binding) in ShortcutCatalog.globalDefinitions {
            guard let serialized = binding.serializedGlobalShortcut,
                  let shortcut = Shortcut(
                    serialized: serialized,
                    appModifier: settingsStore.settings.shortcuts.appModifier
                  )
            else {
                let status = OSStatus(paramErr)
                NSLog(
                    "EduTools: invalid built-in shortcut %@ (OSStatus %d)",
                    definition.id,
                    status
                )
                statusMenu.setHotkeyRegistrationStatus(status)
                warnings[ShortcutCatalog.overrideKey(
                    definitionID: definition.id,
                    scope: .global
                )] = "등록할 수 없는 조합입니다. 대안 단축키를 선택하세요."
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
                warnings[ShortcutCatalog.overrideKey(
                    definitionID: definition.id,
                    scope: .global
                )] = "다른 앱이 사용 중이거나 등록이 거부되었습니다(OSStatus \(status)). 대안을 선택하세요."
            }
            statusMenu.setHotkeyRegistrationStatus(status)
        }
        settingsStore.setRuntimeShortcutWarnings(warnings)
        settingsController?.refreshShortcutPresentation()
    }

    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls where url.pathExtension.lowercased() == "pansetting" {
            settingsController?.openImportPreview(for: url)
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        zoomController?.stopZoom()
        timerCoordinator?.stopBreakMusicForTermination()
        hotkeyCenter?.invalidate()
        if let settingsObserverIdentifier {
            settingsStore.removeObserver(settingsObserverIdentifier)
        }
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

