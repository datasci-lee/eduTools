import AppKit
import CoreGraphics
import Foundation
import ScreenCaptureKit

final class ExportController {
    private let state: AppState
    private weak var overlayController: OverlayController?
    private let fileManager: FileManager

    init(
        state: AppState,
        overlayController: OverlayController,
        fileManager: FileManager = .default
    ) {
        self.state = state
        self.overlayController = overlayController
        self.fileManager = fileManager
    }

    func saveCurrentContent() {
        guard let displayUUID = targetDisplayUUID() else {
            reportFailure(title: "저장할 화면을 찾지 못했습니다", message: "연결된 화면을 확인해 주세요.")
            return
        }
        if state.boardKind(on: displayUUID) != nil {
            saveBoardPDF()
        } else {
            savePNG(on: displayUUID)
        }
    }

    func savePNG() {
        guard let displayUUID = targetDisplayUUID() else {
            reportFailure(title: "저장할 화면을 찾지 못했습니다", message: "연결된 화면을 확인해 주세요.")
            return
        }
        savePNG(on: displayUUID)
    }

    func saveBoardPDF() {
        guard let presentation = state.presentedBoard,
              let overlayController,
              let screen = overlayController.screen(for: presentation.displayUUID)
        else {
            reportFailure(title: "열린 보드가 없습니다", message: "화이트보드나 블랙보드를 먼저 열어 주세요.")
            return
        }

        overlayController.finishActiveInteraction(on: presentation.displayUUID)
        let scale = screen.backingScaleFactor
        let pixelSize = CGSize(
            width: screen.frame.width * scale,
            height: screen.frame.height * scale
        )
        let images = state.boardPages(for: presentation.kind).compactMap { page in
            overlayController.renderedInkImage(
                for: presentation.displayUUID,
                strokes: page,
                boardKind: presentation.kind,
                pixelSize: pixelSize
            )
        }
        guard images.count == state.boardPages(for: presentation.kind).count,
              let data = Self.pdfData(images: images, pageSize: screen.frame.size)
        else {
            reportFailure(title: "보드 PDF를 만들지 못했습니다", message: "보드 페이지 렌더링에 실패했습니다.")
            return
        }

        do {
            let directory = try exportDirectory()
            let url = directory
                .appendingPathComponent(exportBaseName(for: presentation.displayUUID))
                .appendingPathExtension("pdf")
            try data.write(to: url, options: .atomic)
            NSLog("EduTools: saved board PDF to %@", url.path)
        } catch {
            reportFailure(title: "보드 PDF를 저장하지 못했습니다", message: error.localizedDescription)
        }
    }

    func copyCurrentImage() {
        guard let displayUUID = targetDisplayUUID() else {
            reportFailure(title: "복사할 화면을 찾지 못했습니다", message: "연결된 화면을 확인해 주세요.")
            return
        }
        captureComposite(on: displayUUID) { [weak self] image in
            guard let self, let image else { return }
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            let copied = pasteboard.writeObjects([
                NSImage(cgImage: image, size: .zero)
            ])
            if !copied {
                self.reportFailure(
                    title: "클립보드에 복사하지 못했습니다",
                    message: "시스템 클립보드가 이미 사용 중인지 확인해 주세요."
                )
            }
        }
    }

    private func savePNG(on displayUUID: String) {
        captureComposite(on: displayUUID) { [weak self] image in
            guard let self, let image else { return }
            guard let data = NSBitmapImageRep(cgImage: image).representation(
                using: .png,
                properties: [:]
            ) else {
                self.reportFailure(
                    title: "PNG를 만들지 못했습니다",
                    message: "화면 이미지를 PNG 데이터로 변환하지 못했습니다."
                )
                return
            }
            do {
                let directory = try self.exportDirectory()
                let url = directory
                    .appendingPathComponent(self.exportBaseName(for: displayUUID))
                    .appendingPathExtension("png")
                try data.write(to: url, options: .atomic)
                NSLog("EduTools: saved PNG to %@", url.path)
            } catch {
                self.reportFailure(title: "PNG를 저장하지 못했습니다", message: error.localizedDescription)
            }
        }
    }

    private func captureComposite(
        on displayUUID: String,
        completion: @escaping (CGImage?) -> Void
    ) {
        guard let overlayController,
              let screen = overlayController.screen(for: displayUUID),
              let displayID = displayID(for: screen)
        else {
            reportFailure(title: "화면 이미지를 만들지 못했습니다", message: "대상 화면 정보를 찾지 못했습니다.")
            completion(nil)
            return
        }
        overlayController.finishActiveInteraction(on: displayUUID)

        let boardKind = state.boardKind(on: displayUUID)
        if let boardKind {
            let scale = screen.backingScaleFactor
            completion(
                overlayController.renderedInkImage(
                    for: displayUUID,
                    boardKind: boardKind,
                    pixelSize: CGSize(
                        width: screen.frame.width * scale,
                        height: screen.frame.height * scale
                    )
                )
            )
            return
        }

        SCShareableContent.getExcludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        ) { [weak self] content, error in
            DispatchQueue.main.async {
                guard let self, let overlayController = self.overlayController else {
                    completion(nil)
                    return
                }
                guard error == nil,
                      let content,
                      let display = content.displays.first(where: { $0.displayID == displayID })
                else {
                    self.useInkOnlyFallback(
                        displayUUID: displayUUID,
                        pixelSize: self.pixelSize(for: screen),
                        reason: error?.localizedDescription ?? "ScreenCaptureKit에서 화면을 찾지 못했습니다.",
                        completion: completion
                    )
                    return
                }
                guard let ownApplication = content.applications.first(where: {
                    $0.processID == ProcessInfo.processInfo.processIdentifier
                }) else {
                    self.useInkOnlyFallback(
                        displayUUID: displayUUID,
                        pixelSize: CGSize(width: display.width, height: display.height),
                        reason: "EduTools 창을 화면 캡처에서 제외하지 못했습니다.",
                        completion: completion
                    )
                    return
                }

                let pixelSize = CGSize(width: display.width, height: display.height)
                guard let inkImage = overlayController.renderedInkImage(
                    for: displayUUID,
                    pixelSize: pixelSize
                ) else {
                    self.reportFailure(
                        title: "판서 이미지를 만들지 못했습니다",
                        message: "판서층 렌더링에 실패했습니다."
                    )
                    completion(nil)
                    return
                }

                let filter = SCContentFilter(
                    display: display,
                    excludingApplications: [ownApplication],
                    exceptingWindows: []
                )
                let configuration = SCStreamConfiguration()
                configuration.width = display.width
                configuration.height = display.height
                configuration.showsCursor = true
                configuration.capturesAudio = false
                SCScreenshotManager.captureImage(
                    contentFilter: filter,
                    configuration: configuration
                ) { background, captureError in
                    DispatchQueue.main.async {
                        guard let background, captureError == nil else {
                            self.useInkOnlyFallback(
                                displayUUID: displayUUID,
                                pixelSize: pixelSize,
                                existingInkImage: inkImage,
                                reason: captureError?.localizedDescription
                                    ?? "화면 기록 권한으로 배경을 캡처하지 못했습니다.",
                                completion: completion
                            )
                            return
                        }
                        guard let composite = Self.composite(
                            background: background,
                            foreground: inkImage
                        ) else {
                            self.reportFailure(
                                title: "화면과 판서를 합치지 못했습니다",
                                message: "이미지 합성 컨텍스트를 만들지 못했습니다."
                            )
                            completion(nil)
                            return
                        }
                        completion(composite)
                    }
                }
            }
        }
    }

    private func useInkOnlyFallback(
        displayUUID: String,
        pixelSize: CGSize,
        existingInkImage: CGImage? = nil,
        reason: String,
        completion: @escaping (CGImage?) -> Void
    ) {
        NSLog("EduTools: screen background capture failed; exporting ink only: %@", reason)
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "배경 없이 판서만 저장합니다"
        alert.informativeText = "화면 기록 권한을 확인해 주세요.\n\(reason)"
        alert.addButton(withTitle: "확인")
        alert.runAboveOverlay()
        completion(
            existingInkImage
                ?? overlayController?.renderedInkImage(
                    for: displayUUID,
                    pixelSize: pixelSize
                )
        )
    }

    private func targetDisplayUUID() -> String? {
        overlayController?.displayUUIDUnderMouse() ?? state.activeDisplayUUID
    }

    private func pixelSize(for screen: NSScreen) -> CGSize {
        CGSize(
            width: screen.frame.width * screen.backingScaleFactor,
            height: screen.frame.height * screen.backingScaleFactor
        )
    }

    private func exportDirectory() throws -> URL {
        let pictures = try fileManager.url(
            for: .picturesDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let directory = pictures.appendingPathComponent("EduTools", isDirectory: true)
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    private func exportBaseName(for displayUUID: String) -> String {
        let safeDisplay = fileComponent(forDisplayUUID: displayUUID)
        return "\(Self.exportDateFormatter.string(from: Date()))-\(safeDisplay)"
    }

    private func reportFailure(title: String, message: String) {
        NSLog("EduTools: %@: %@", title, message)
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "확인")
        alert.runAboveOverlay()
    }

    private static func composite(background: CGImage, foreground: CGImage) -> CGImage? {
        let width = background.width
        let height = background.height
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }
        let target = CGRect(x: 0, y: 0, width: width, height: height)
        context.draw(background, in: target)
        context.draw(foreground, in: target)
        return context.makeImage()
    }

    private static func pdfData(images: [CGImage], pageSize: CGSize) -> Data? {
        guard !images.isEmpty, pageSize.width > 0, pageSize.height > 0 else { return nil }
        let mutableData = NSMutableData()
        guard let consumer = CGDataConsumer(data: mutableData as CFMutableData) else { return nil }
        var mediaBox = CGRect(origin: .zero, size: pageSize)
        guard let context = CGContext(consumer: consumer, mediaBox: &mediaBox, nil) else {
            return nil
        }
        for image in images {
            context.beginPDFPage(nil)
            context.draw(image, in: mediaBox)
            context.endPDFPage()
        }
        context.closePDF()
        return mutableData as Data
    }

    private static let exportDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        return formatter
    }()
}
