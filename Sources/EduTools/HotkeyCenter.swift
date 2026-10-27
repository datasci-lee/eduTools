import Carbon
import Foundation

private func hotKeyEventHandler(
    _ nextHandler: EventHandlerCallRef?,
    _ event: EventRef?,
    _ userData: UnsafeMutableRawPointer?
) -> OSStatus {
    guard let event, let userData else {
        return OSStatus(eventNotHandledErr)
    }

    var hotKeyID = EventHotKeyID()
    let status = GetEventParameter(
        event,
        EventParamName(kEventParamDirectObject),
        EventParamType(typeEventHotKeyID),
        nil,
        MemoryLayout<EventHotKeyID>.size,
        nil,
        &hotKeyID
    )
    guard status == noErr else { return status }

    let center = Unmanaged<HotkeyCenter>.fromOpaque(userData).takeUnretainedValue()
    return center.dispatch(identifier: hotKeyID.id)
}

final class HotkeyCenter {
    private let signature: OSType = 0x4544_544C
    private var eventHandlerReference: EventHandlerRef?
    private var eventHandlerStatus: OSStatus = noErr
    private var hotKeyReferences: [UInt32: EventHotKeyRef] = [:]
    private var actions: [UInt32: () -> Void] = [:]
    private var nextIdentifier: UInt32 = 1

    init() {
        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        eventHandlerStatus = InstallEventHandler(
            GetApplicationEventTarget(),
            hotKeyEventHandler,
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &eventHandlerReference
        )
    }

    @discardableResult
    func register(shortcut: Shortcut, action: @escaping () -> Void) -> OSStatus {
        guard eventHandlerStatus == noErr else { return eventHandlerStatus }

        let identifier = nextIdentifier
        nextIdentifier &+= 1
        let hotKeyID = EventHotKeyID(signature: signature, id: identifier)
        var reference: EventHotKeyRef?

        actions[identifier] = action
        let status = RegisterEventHotKey(
            shortcut.keyCode,
            shortcut.carbonModifiers,
            hotKeyID,
            GetApplicationEventTarget(),
            OptionBits(kEventHotKeyNoOptions),
            &reference
        )

        guard status == noErr, let reference else {
            actions.removeValue(forKey: identifier)
            return status == noErr ? OSStatus(paramErr) : status
        }

        hotKeyReferences[identifier] = reference
        return noErr
    }

    fileprivate func dispatch(identifier: UInt32) -> OSStatus {
        guard let action = actions[identifier] else {
            return OSStatus(eventNotHandledErr)
        }
        action()
        return noErr
    }

    func invalidate() {
        for reference in hotKeyReferences.values {
            let status = UnregisterEventHotKey(reference)
            if status != noErr {
                NSLog("EduTools: failed to unregister hotkey (OSStatus %d)", status)
            }
        }
        hotKeyReferences.removeAll()
        actions.removeAll()

        if let reference = eventHandlerReference {
            let status = RemoveEventHandler(reference)
            if status != noErr {
                NSLog("EduTools: failed to remove hotkey event handler (OSStatus %d)", status)
            }
            eventHandlerReference = nil
        }
    }

    deinit {
        invalidate()
    }
}

