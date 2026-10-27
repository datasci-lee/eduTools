import AppKit
import CoreGraphics

func displayID(for screen: NSScreen) -> CGDirectDisplayID? {
    let screenNumberKey = NSDeviceDescriptionKey("NSScreenNumber")
    return (screen.deviceDescription[screenNumberKey] as? NSNumber)?.uint32Value
}

func displayUUID(for screen: NSScreen) -> String? {
    guard let displayID = displayID(for: screen) else { return nil }
    let uuid = CGDisplayCreateUUIDFromDisplayID(displayID).takeRetainedValue()
    return CFUUIDCreateString(nil, uuid) as String
}

func screenUnderMouse() -> NSScreen? {
    let mouseLocation = NSEvent.mouseLocation
    return NSScreen.screens.first { $0.frame.contains(mouseLocation) } ?? NSScreen.screens.first
}
