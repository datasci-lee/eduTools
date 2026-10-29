import AppKit
import CoreGraphics

func restoredOrigin(
    ratio: CGPoint?,
    default defaultRatio: CGPoint,
    visibleFrame: CGRect,
    size: CGSize
) -> CGPoint {
    let availableWidth = max(0, visibleFrame.width - size.width)
    let availableHeight = max(0, visibleFrame.height - size.height)
    let xRatio = min(max(ratio?.x ?? defaultRatio.x, 0), 1)
    let yRatio = min(max(ratio?.y ?? defaultRatio.y, 0), 1)
    return CGPoint(
        x: visibleFrame.minX + availableWidth * xRatio,
        y: visibleFrame.minY + availableHeight * yRatio
    )
}

func savedRatio(origin: CGPoint, visibleFrame: CGRect, size: CGSize) -> CGPoint {
    let availableWidth = max(1, visibleFrame.width - size.width)
    let availableHeight = max(1, visibleFrame.height - size.height)
    return CGPoint(
        x: min(max((origin.x - visibleFrame.minX) / availableWidth, 0), 1),
        y: min(max((origin.y - visibleFrame.minY) / availableHeight, 0), 1)
    )
}

struct ScreenPositionStore {
    let defaults: UserDefaults
    let positionsKey: String
    let displayUUIDKey: String

    func ratio(for displayUUID: String?) -> CGPoint? {
        guard let displayUUID,
              let positions = defaults.dictionary(forKey: positionsKey),
              let entry = positions[displayUUID] as? [String: Any],
              let x = entry["x"] as? NSNumber,
              let y = entry["y"] as? NSNumber
        else {
            return nil
        }
        return CGPoint(x: x.doubleValue, y: y.doubleValue)
    }

    func save(ratio: CGPoint, displayUUID: String) {
        var positions = defaults.dictionary(forKey: positionsKey) ?? [:]
        positions[displayUUID] = ["x": Double(ratio.x), "y": Double(ratio.y)]
        defaults.set(positions, forKey: positionsKey)
        defaults.set(displayUUID, forKey: displayUUIDKey)
    }
}

func displayID(for screen: NSScreen) -> CGDirectDisplayID? {
    let screenNumberKey = NSDeviceDescriptionKey("NSScreenNumber")
    return (screen.deviceDescription[screenNumberKey] as? NSNumber)?.uint32Value
}

func displayUUID(for screen: NSScreen) -> String? {
    guard let displayID = displayID(for: screen) else { return nil }
    let uuid = CGDisplayCreateUUIDFromDisplayID(displayID).takeRetainedValue()
    return CFUUIDCreateString(nil, uuid) as String
}

func screensByDisplayUUID() -> [String: NSScreen] {
    Dictionary(
        NSScreen.screens.compactMap { screen in
            displayUUID(for: screen).map { ($0, screen) }
        },
        uniquingKeysWith: { first, _ in first }
    )
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
