import AppKit
import Carbon
import Darwin

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let appState = AppState()
    private var statusMenu: StatusMenu?
    private var overlayController: OverlayController?
    private var hotkeyCenter: HotkeyCenter?

    func applicationDidFinishLaunching(_ notification: Notification) {
        _ = NSApp.setActivationPolicy(.accessory)

        let statusMenu = StatusMenu(state: appState)
        self.statusMenu = statusMenu
        overlayController = OverlayController(state: appState)

        let hotkeyCenter = HotkeyCenter()
        self.hotkeyCenter = hotkeyCenter

        guard let shortcut = Shortcut(serialized: "AppMod+P") else {
            let status = OSStatus(paramErr)
            NSLog("EduTools: invalid built-in shortcut AppMod+P (OSStatus %d)", status)
            statusMenu.setHotkeyRegistrationStatus(status)
            return
        }

        let status = hotkeyCenter.register(shortcut: shortcut) { [weak appState] in
            appState?.toggleAnnotationMode()
        }
        if status != noErr {
            NSLog(
                "EduTools: failed to register global shortcut %@ (OSStatus %d)",
                shortcut.serialized,
                status
            )
        }
        statusMenu.setHotkeyRegistrationStatus(status)
    }

    func applicationWillTerminate(_ notification: Notification) {
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

