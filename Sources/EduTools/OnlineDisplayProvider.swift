import ColorSync
import CoreGraphics
import Foundation
import IOKit

enum OnlineDisplayProviderError: LocalizedError {
    case listFailed(CGError)

    var errorDescription: String? {
        switch self {
        case let .listFailed(error):
            return "온라인 모니터 목록을 읽지 못했습니다. CoreGraphics 오류 \(error.rawValue)"
        }
    }
}

enum OnlineDisplayProvider {
    static func current() throws -> [OnlineDisplay] {
        var displayCount: UInt32 = 0
        let countResult = CGGetOnlineDisplayList(0, nil, &displayCount)
        guard countResult == .success else {
            throw OnlineDisplayProviderError.listFailed(countResult)
        }
        guard displayCount > 0 else { return [] }

        var displayIDs = [CGDirectDisplayID](repeating: 0, count: Int(displayCount))
        var actualCount = displayCount
        let listResult = displayIDs.withUnsafeMutableBufferPointer { buffer in
            CGGetOnlineDisplayList(displayCount, buffer.baseAddress, &actualCount)
        }
        guard listResult == .success else {
            throw OnlineDisplayProviderError.listFailed(listResult)
        }

        return displayIDs.prefix(Int(actualCount)).map { displayID in
            let displayUUID = CGDisplayCreateUUIDFromDisplayID(displayID).takeRetainedValue()
            let uuid = CFUUIDCreateString(nil, displayUUID) as String
            let kind = displayKind(displayID)
            let model = CGDisplayModelNumber(displayID)
            let bounds = CGDisplayBounds(displayID)
            let mirroredDisplay = CGDisplayMirrorsDisplay(displayID)
            return OnlineDisplay(
                id: displayID,
                uuid: uuid,
                vendor: CGDisplayVendorNumber(displayID),
                model: model,
                serial: CGDisplaySerialNumber(displayID),
                name: localizedName(
                    displayID: displayID,
                    displayUUID: displayUUID,
                    model: model,
                    kind: kind
                ),
                kind: kind,
                size: DisplaySize(width: bounds.width, height: bounds.height),
                origin: DisplayPoint(x: bounds.origin.x, y: bounds.origin.y),
                currentMirrorMasterID: mirroredDisplay == kCGNullDirectDisplay
                    ? nil
                    : mirroredDisplay
            )
        }
    }

    private static func displayKind(_ displayID: CGDirectDisplayID) -> DisplayKind {
        if CGDisplayIsBuiltin(displayID) != 0 {
            return .builtIn
        }
        if let mode = CGDisplayCopyDisplayMode(displayID),
           mode.ioFlags & UInt32(kDisplayModeValidForAirPlayFlag) != 0
        {
            return .virtual
        }
        return .external
    }

    private static func localizedName(
        displayID: CGDirectDisplayID,
        displayUUID: CFUUID,
        model: UInt32,
        kind: DisplayKind
    ) -> String {
        if let displayClass = kColorSyncDisplayDeviceClass?.takeUnretainedValue(),
           let descriptionKey = kColorSyncDeviceDescription?.takeUnretainedValue(),
           let unmanagedInfo = ColorSyncDeviceCopyDeviceInfo(displayClass, displayUUID)
        {
            let info = unmanagedInfo.takeRetainedValue() as NSDictionary
            if let name = info[descriptionKey] as? String,
               !name.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines).isEmpty
            {
                return name
            }
        }
        NSLog(
            "EduTools: display name unavailable; using a generic label; display=%u model=%u",
            displayID,
            model
        )
        switch kind {
        case .builtIn:
            return "내장 디스플레이"
        case .external:
            return String(format: "외장 디스플레이 %08X", model)
        case .virtual:
            return String(format: "가상 디스플레이 %08X", model)
        }
    }
}

// [확인 필요] Sidecar·AirPlay의 UUID가 재연결 때 유지되는지는 실기 검증 전이다.
// 모델의 3단계 가상 화면 매칭(vendor+model+name)이 UUID 변경을 흡수한다.
