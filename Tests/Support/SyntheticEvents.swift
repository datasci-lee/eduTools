import AppKit
import Foundation

public enum SyntheticEventError: Error {
  case creationFailed(NSEvent.EventType)
}

public enum SyntheticEvents {
  public static func mouseEvent(
    type: NSEvent.EventType,
    location: CGPoint,
    in window: NSWindow,
    modifierFlags: NSEvent.ModifierFlags = [],
    timestamp: TimeInterval = 0,
    eventNumber: Int = 1,
    clickCount: Int = 1,
    pressure: Float = 1
  ) throws -> NSEvent {
    guard
      let event = NSEvent.mouseEvent(
        with: type,
        location: location,
        modifierFlags: modifierFlags,
        timestamp: timestamp,
        windowNumber: window.windowNumber,
        context: nil,
        eventNumber: eventNumber,
        clickCount: clickCount,
        pressure: pressure
      )
    else {
      throw SyntheticEventError.creationFailed(type)
    }
    return event
  }

  public static func enterExitEvent(
    type: NSEvent.EventType,
    location: CGPoint,
    in window: NSWindow,
    timestamp: TimeInterval = 0,
    eventNumber: Int = 1,
    trackingNumber: Int = 1
  ) throws -> NSEvent {
    guard
      let event = NSEvent.enterExitEvent(
        with: type,
        location: location,
        modifierFlags: [],
        timestamp: timestamp,
        windowNumber: window.windowNumber,
        context: nil,
        eventNumber: eventNumber,
        trackingNumber: trackingNumber,
        userData: nil
      )
    else {
      throw SyntheticEventError.creationFailed(type)
    }
    return event
  }

  public static func keyEvent(
    type: NSEvent.EventType,
    keyCode: UInt16,
    characters: String,
    in window: NSWindow,
    modifierFlags: NSEvent.ModifierFlags = [],
    timestamp: TimeInterval = 0,
    isRepeat: Bool = false
  ) throws -> NSEvent {
    guard
      let event = NSEvent.keyEvent(
        with: type,
        location: .zero,
        modifierFlags: modifierFlags,
        timestamp: timestamp,
        windowNumber: window.windowNumber,
        context: nil,
        characters: characters,
        charactersIgnoringModifiers: characters,
        isARepeat: isRepeat,
        keyCode: keyCode
      )
    else {
      throw SyntheticEventError.creationFailed(type)
    }
    return event
  }
}

public final class OffscreenWindowHost {
  public let window: NSWindow

  public init(view: NSView, size: CGSize) {
    let screenFrames = NSScreen.screens.map(\.frame)
    let minimumX = screenFrames.map(\.minX).min() ?? 0
    let minimumY = screenFrames.map(\.minY).min() ?? 0
    let frame = CGRect(
      x: minimumX - size.width - 100,
      y: minimumY - size.height - 100,
      width: size.width,
      height: size.height
    )
    window = NSWindow(
      contentRect: frame,
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )
    window.isReleasedWhenClosed = false
    window.alphaValue = 0
    view.frame = CGRect(origin: .zero, size: size)
    window.contentView = view
    // Do not order the transparent offscreen window front: GUI tests must not steal focus.
  }

  deinit {
    window.close()
  }
}
