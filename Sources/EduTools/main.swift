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

        let registrations: [(serialized: String, action: () -> Void)] = [
            ("AppMod+P", { [weak appState] in appState?.toggleAnnotationMode() }),
            ("AppMod+L", { [weak appState] in appState?.activateLaserShortcut() }),
        ]
        for registration in registrations {
            guard let shortcut = Shortcut(serialized: registration.serialized) else {
                let status = OSStatus(paramErr)
                NSLog(
                    "EduTools: invalid built-in shortcut %@ (OSStatus %d)",
                    registration.serialized,
                    status
                )
                statusMenu.setHotkeyRegistrationStatus(status)
                continue
            }

            let status = hotkeyCenter.register(shortcut: shortcut, action: registration.action)
            if status != noErr {
                NSLog(
                    "EduTools: failed to register global shortcut %@ (OSStatus %d)",
                    shortcut.serialized,
                    status
                )
            }
            statusMenu.setHotkeyRegistrationStatus(status)
        }
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

