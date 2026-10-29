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

func fileComponent(forDisplayUUID displayUUID: String) -> String {
    let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_"))
    return displayUUID.unicodeScalars.map {
        allowed.contains($0) ? String($0) : "_"
    }.joined()
}

/// Screen geometry comes from `NSScreen`, never from a window frame: a window can lag behind a
/// display change (or be mispositioned), the screen list cannot.
func attachedScreen(forDisplayUUID identifier: String) -> NSScreen? {
    NSScreen.screens.first { displayUUID(for: $0) == identifier }
}

func displayUUID(at point: NSPoint) -> String? {
    NSScreen.screens.first { $0.frame.contains(point) }.flatMap { displayUUID(for: $0) }
}

func screenUnderMouse() -> NSScreen? {
    let mouseLocation = NSEvent.mouseLocation
    return NSScreen.screens.first { $0.frame.contains(mouseLocation) } ?? NSScreen.screens.first
}
