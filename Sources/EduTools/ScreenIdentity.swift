import AppKit
import CoreGraphics

func displayUUID(for screen: NSScreen) -> String? {
    let screenNumberKey = NSDeviceDescriptionKey("NSScreenNumber")
    guard let number = screen.deviceDescription[screenNumberKey] as? NSNumber else {
        return nil
    }

    let uuid = CGDisplayCreateUUIDFromDisplayID(number.uint32Value).takeRetainedValue()
    return CFUUIDCreateString(nil, uuid) as String
}

func screenUnderMouse() -> NSScreen? {
    let mouseLocation = NSEvent.mouseLocation
    return NSScreen.screens.first { $0.frame.contains(mouseLocation) } ?? NSScreen.screens.first
}
