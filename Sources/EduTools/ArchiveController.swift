import AppKit
import Foundation

final class ArchiveController {
    private let state: AppState
    private let settingsStore: SettingsStore
    private weak var overlayController: OverlayController?
    private let fileManager: FileManager
    private var workspaceObservers: [NSObjectProtocol] = []
    private var offeredDisplayUUIDs: Set<String> = []
    private var disconnectedDataByDisplayUUID: [String: Data] = [:]

    init(
        state: AppState,
        overlayController: OverlayController,
        settingsStore: SettingsStore,
        fileManager: FileManager = .default
    ) {
        self.state = state
        self.settingsStore = settingsStore
        self.overlayController = overlayController
        self.fileManager = fileManager
        state.archiveBeforeRemoval = { [weak self] displayUUIDs in
            guard let self else {
                NSLog("EduTools: automatic archive controller is unavailable; removal was cancelled")
                return false
            }
            return self.archiveVisibleContent(on: displayUUIDs)
        }
        observeWorkspaceSleep()
    }

    func displayWasRemoved(_ displayUUID: String, primaryDisplayUUID: String?) {
        offeredDisplayUUIDs.remove(displayUUID)
        guard state.hasRestorableScreenState(for: displayUUID) else { return }
        switch state.displayDisconnectPolicy {
        case .archive:
            _ = storeDisconnectedDisplay(displayUUID)
        case .moveToPrimary:
            guard let primaryDisplayUUID else {
                NSLog(
                    "EduTools: no primary display was available; disconnected ink will be archived"
                )
                _ = storeDisconnectedDisplay(displayUUID)
                return
            }
            if !state.moveScreenInk(from: displayUUID, to: primaryDisplayUUID),
               state.hasRestorableScreenState(for: displayUUID)
            {
                NSLog(
                    "EduTools: disconnected ink could not be moved; it will be archived instead"
                )
                _ = storeDisconnectedDisplay(displayUUID)
            }
        case .delete:
            state.discardScreenInk(on: displayUUID)
        }
    }

    func restoreDisconnectedDisplayIfNeeded(_ displayUUID: String) {
        guard !offeredDisplayUUIDs.contains(displayUUID),
              let url = disconnectedArchiveURL(for: displayUUID)
        else {
            return
        }
        let inMemoryData = disconnectedDataByDisplayUUID[displayUUID]
        guard inMemoryData != nil || fileManager.fileExists(atPath: url.path) else { return }
        offeredDisplayUUIDs.insert(displayUUID)

        let shouldRestore: Bool
        if state.automaticallyRestoresDisconnectedDisplays {
            shouldRestore = true
        } else {
            let alert = NSAlert()
            alert.alertStyle = .informational
            alert.messageText = "이전 판서를 복원할까요?"
            alert.informativeText = "다시 연결된 화면의 판서와 실행 취소 기록을 복원합니다."
            alert.addButton(withTitle: "복원")
            alert.addButton(withTitle: "나중에")
            shouldRestore = alert.runAboveOverlay() == .alertFirstButtonReturn
        }
        guard shouldRestore else { return }

        do {
            let data = try inMemoryData ?? Data(contentsOf: url)
            try state.restoreScreenInk(from: data, to: displayUUID)
            disconnectedDataByDisplayUUID.removeValue(forKey: displayUUID)
            do {
                try fileManager.removeItem(at: url)
            } catch {
                NSLog(
                    "EduTools: restored disconnected ink but could not remove %@: %@",
                    url.path,
                    error.localizedDescription
                )
            }
        } catch {
            reportFailure(
                title: "이전 판서를 복원하지 못했습니다",
                message: error.localizedDescription
            )
        }
    }

    private func archiveVisibleContent(on displayUUIDs: Set<String>) -> Bool {
        guard let overlayController else {
            reportFailure(
                title: "자동 보관에 실패했습니다",
                message: "판서 화면을 찾지 못해 내용 삭제를 취소했습니다."
            )
            return false
        }

        let now = Date()
        guard let directory = automaticArchiveDirectory(for: now) else { return false }
        do {
            try fileManager.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
        } catch {
            reportFailure(title: "자동 보관 폴더를 만들지 못했습니다", message: error.localizedDescription)
            return false
        }

        let time = Self.timeFormatter.string(from: now)
        for displayUUID in displayUUIDs.sorted() {
            overlayController.finishActiveInteraction(on: displayUUID)
            do {
                let data = try state.encodedVisibleContent(on: displayUUID)
                let baseName = "\(time)-\(safePathComponent(displayUUID))"
                let jsonURL = directory.appendingPathComponent(baseName).appendingPathExtension("json")
                try data.write(to: jsonURL, options: .atomic)

                guard let screen = overlayController.screen(for: displayUUID) else {
                    throw CocoaError(.fileWriteUnknown)
                }
                let scale = screen.backingScaleFactor
                let pixelSize = CGSize(
                    width: screen.frame.width * scale,
                    height: screen.frame.height * scale
                )
                let boardKind = state.boardKind(on: displayUUID)
                guard let image = overlayController.renderedInkImage(
                    for: displayUUID,
                    boardKind: boardKind,
                    pixelSize: pixelSize
                ), let png = Self.pngData(for: image)
                else {
                    throw CocoaError(.fileWriteUnknown)
                }
                let pngURL = directory.appendingPathComponent(baseName).appendingPathExtension("png")
                try png.write(to: pngURL, options: .atomic)
            } catch {
                reportFailure(title: "자동 보관에 실패했습니다", message: error.localizedDescription)
                return false
            }
        }
        return true
    }

    @discardableResult
    private func storeDisconnectedDisplay(_ displayUUID: String) -> Bool {
        guard state.hasRestorableScreenState(for: displayUUID) else { return true }
        guard let url = disconnectedArchiveURL(for: displayUUID) else { return false }
        do {
            try fileManager.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let data = try state.encodedScreenInk(for: displayUUID)
            try data.write(to: url, options: .atomic)
            disconnectedDataByDisplayUUID[displayUUID] = data
            state.discardScreenInk(on: displayUUID)
            offeredDisplayUUIDs.remove(displayUUID)
            return true
        } catch {
            reportFailure(
                title: "연결 해제된 화면을 보관하지 못했습니다",
                message: error.localizedDescription
            )
            return false
        }
    }

    private func observeWorkspaceSleep() {
        let center = NSWorkspace.shared.notificationCenter
        workspaceObservers.append(
            center.addObserver(
                forName: NSWorkspace.willSleepNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                guard let self, let overlayController = self.overlayController else { return }
                overlayController.finishAllActiveInteractions()
                for displayUUID in overlayController.connectedDisplayUUIDs {
                    _ = self.storeDisconnectedDisplay(displayUUID)
                }
            }
        )
        workspaceObservers.append(
            center.addObserver(
                forName: NSWorkspace.didWakeNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                self?.overlayController?.refreshAfterWake()
            }
        )
    }

    private func automaticArchiveDirectory(for date: Date) -> URL? {
        let root: URL
        if let path = settingsStore.settings.general.archiveFolderPath {
            root = URL(fileURLWithPath: path, isDirectory: true)
        } else {
            guard let applicationSupportRoot = applicationSupportRoot() else { return nil }
            root = applicationSupportRoot.appendingPathComponent("Archive", isDirectory: true)
        }
        return root
            .appendingPathComponent(Self.dayFormatter.string(from: date), isDirectory: true)
    }

    private func disconnectedArchiveURL(for displayUUID: String) -> URL? {
        applicationSupportRoot()?
            .appendingPathComponent("Disconnected", isDirectory: true)
            .appendingPathComponent(safePathComponent(displayUUID))
            .appendingPathExtension("json")
    }

    private func applicationSupportRoot() -> URL? {
        do {
            return try fileManager.url(
                for: .applicationSupportDirectory,
                in: .userDomainMask,
                appropriateFor: nil,
                create: true
            ).appendingPathComponent("EduTools", isDirectory: true)
        } catch {
            reportFailure(
                title: "EduTools 보관 폴더를 찾지 못했습니다",
                message: error.localizedDescription
            )
            return nil
        }
    }

    private func safePathComponent(_ value: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_"))
        return value.unicodeScalars.map { allowed.contains($0) ? String($0) : "_" }.joined()
    }

    private func reportFailure(title: String, message: String) {
        NSLog("EduTools: %@: %@", title, message)
        let showAlert = {
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = title
            alert.informativeText = message
            alert.addButton(withTitle: "확인")
            alert.runAboveOverlay()
        }
        if Thread.isMainThread {
            showAlert()
        } else {
            DispatchQueue.main.async(execute: showAlert)
        }
    }

    private static func pngData(for image: CGImage) -> Data? {
        NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])
    }

    private static let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "HHmmss"
        return formatter
    }()

    deinit {
        state.archiveBeforeRemoval = nil
        let center = NSWorkspace.shared.notificationCenter
        for observer in workspaceObservers {
            center.removeObserver(observer)
        }
    }
}
