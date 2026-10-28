import AppKit
import Carbon
import CoreImage
import CoreMedia
import CoreVideo
import QuartzCore
import ScreenCaptureKit

private final class ZoomPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private final class ZoomCaptureSession: NSObject, SCStreamOutput, SCStreamDelegate {
    private let displayID: CGDirectDisplayID
    private let pixelSize: CGSize
    private let captureQueue = DispatchQueue(
        label: "kr.datasci.edutools.zoom.capture",
        qos: .userInteractive
    )
    private let imageContext = CIContext()
    private let onFrame: (CGImage) -> Void
    private let onFailure: (String) -> Void
    private var stream: SCStream?
    private var isStopping = false

    init(
        displayID: CGDirectDisplayID,
        pixelSize: CGSize,
        onFrame: @escaping (CGImage) -> Void,
        onFailure: @escaping (String) -> Void
    ) {
        self.displayID = displayID
        self.pixelSize = pixelSize
        self.onFrame = onFrame
        self.onFailure = onFailure
    }

    func start() {
        SCShareableContent.getExcludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        ) { [weak self] content, error in
            guard let self else { return }
            captureQueue.async {
                self.configureAndStart(content: content, error: error)
            }
        }
    }

    func stop() {
        captureQueue.async {
            guard !self.isStopping else { return }
            self.isStopping = true
            guard let stream = self.stream else { return }
            stream.stopCapture { error in
                if let error {
                    NSLog("EduTools: failed to stop zoom capture: %@", error.localizedDescription)
                }
            }
            self.stream = nil
        }
    }

    private func configureAndStart(content: SCShareableContent?, error: Error?) {
        guard !isStopping else { return }
        if let error {
            fail("캡처 가능한 화면을 불러오지 못했습니다: \(error.localizedDescription)")
            return
        }
        guard let content else {
            fail("캡처 가능한 화면 목록이 비어 있습니다.")
            return
        }
        guard let display = content.displays.first(where: { $0.displayID == displayID }) else {
            fail("선택한 화면을 ScreenCaptureKit 목록에서 찾지 못했습니다.")
            return
        }
        let processID = ProcessInfo.processInfo.processIdentifier
        guard let ownApplication = content.applications.first(where: {
            $0.processID == processID
        }) else {
            fail("EduTools 창을 캡처에서 제외할 애플리케이션 항목을 찾지 못했습니다.")
            return
        }

        let filter = SCContentFilter(
            display: display,
            excludingApplications: [ownApplication],
            exceptingWindows: []
        )
        let configuration = SCStreamConfiguration()
        configuration.width = max(1, Int(pixelSize.width.rounded()))
        configuration.height = max(1, Int(pixelSize.height.rounded()))
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: 60)
        configuration.queueDepth = 3
        configuration.pixelFormat = kCVPixelFormatType_32BGRA
        configuration.showsCursor = true
        configuration.capturesAudio = false

        let stream = SCStream(filter: filter, configuration: configuration, delegate: self)
        do {
            try stream.addStreamOutput(
                self,
                type: .screen,
                sampleHandlerQueue: captureQueue
            )
        } catch {
            fail("화면 프레임 출력을 연결하지 못했습니다: \(error.localizedDescription)")
            return
        }
        self.stream = stream
        stream.startCapture { [weak self] error in
            guard let self else { return }
            captureQueue.async {
                guard !self.isStopping else { return }
                if let error {
                    self.fail("화면 캡처를 시작하지 못했습니다: \(error.localizedDescription)")
                }
            }
        }
    }

    private func fail(_ message: String) {
        guard !isStopping else { return }
        isStopping = true
        stream = nil
        NSLog("EduTools: zoom capture failure: %@", message)
        DispatchQueue.main.async { [onFailure] in
            onFailure(message)
        }
    }

    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard outputType == .screen, !isStopping else { return }
        guard sampleBuffer.isValid,
              let pixelBuffer = sampleBuffer.imageBuffer
        else {
            NSLog("EduTools: ScreenCaptureKit delivered an invalid zoom frame")
            return
        }
        let image = CIImage(cvPixelBuffer: pixelBuffer)
        guard let frame = imageContext.createCGImage(image, from: image.extent) else {
            NSLog("EduTools: failed to convert a zoom frame to CGImage")
            return
        }
        DispatchQueue.main.async { [onFrame] in
            onFrame(frame)
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        captureQueue.async { [weak self] in
            guard let self, !isStopping else { return }
            fail("화면 캡처가 중단되었습니다: \(error.localizedDescription)")
        }
    }
}

private final class ZoomView: NSView {
    var pointerMoved: ((CGPoint) -> Void)?
    var scrollInput: ((NSEvent) -> Bool)?
    var magnifyInput: ((NSEvent) -> Bool)?

    private let dispatcher: AppCommandDispatcher
    private let displayUUID: String
    private let imageLayer = CALayer()
    private let dimLayer = CALayer()
    private let dimMaskLayer = CAShapeLayer()
    private let previewBorderLayer = CAShapeLayer()
    private let previewLabelLayer = CATextLayer()
    private let messageLayer = CATextLayer()
    private let inkView: InkView
    private let colorControls: CIFilter?
    private var trackingAreaReference: NSTrackingArea?
    private var renderedState: ZoomSessionState?
    private var messagePresentation = 0
    private var pressedShortcutDefinitions: [UInt16: ShortcutDefinition] = [:]

    override var acceptsFirstResponder: Bool { true }

    var preferredFirstResponder: NSResponder {
        if renderedState?.isFrozen == true, renderedState?.isPreviewing == false {
            return inkView
        }
        return self
    }

    init(
        frame frameRect: NSRect,
        state: AppState,
        dispatcher: AppCommandDispatcher,
        displayUUID: String,
        requestKeyFocus: @escaping () -> Void
    ) {
        self.dispatcher = dispatcher
        self.displayUUID = displayUUID
        inkView = InkView(
            frame: NSRect(origin: .zero, size: frameRect.size),
            state: state,
            dispatcher: dispatcher,
            displayUUID: displayUUID,
            requestKeyFocus: requestKeyFocus
        )
        colorControls = CIFilter(name: "CIColorControls")
        super.init(frame: frameRect)
        configureLayers()
        configureInkView()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        imageLayer.frame = bounds
        dimLayer.frame = bounds
        dimMaskLayer.frame = bounds
        messageLayer.frame = CGRect(
            x: max(16, (bounds.width - 300) / 2),
            y: 30,
            width: min(max(bounds.width - 32, 0), 300),
            height: 38
        )
        CATransaction.commit()
        inkView.frame = bounds
        if let renderedState {
            applyGeometry(renderedState, animated: false)
        }
    }

    override func updateTrackingAreas() {
        if let trackingAreaReference {
            removeTrackingArea(trackingAreaReference)
        }
        let trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea)
        trackingAreaReference = trackingArea
        super.updateTrackingAreas()
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard bounds.contains(point) else { return nil }
        if renderedState?.isFrozen == true, renderedState?.isPreviewing == false {
            let inkPoint = inkView.convert(point, from: self)
            return inkView.hitTest(inkPoint) ?? inkView
        }
        return self
    }

    override func mouseMoved(with event: NSEvent) {
        pointerMoved?(convert(event.locationInWindow, from: nil))
    }

    override func mouseDragged(with event: NSEvent) {
        pointerMoved?(convert(event.locationInWindow, from: nil))
    }

    override func scrollWheel(with event: NSEvent) {
        if scrollInput?(event) == true { return }
        super.scrollWheel(with: event)
    }

    override func magnify(with event: NSEvent) {
        if magnifyInput?(event) == true { return }
        super.magnify(with: event)
    }

    override func keyDown(with event: NSEvent) {
        if handleKeyDown(event) { return }
        super.keyDown(with: event)
    }

    override func keyUp(with event: NSEvent) {
        if handleKeyUp(event) { return }
        super.keyUp(with: event)
    }

    func setFrameImage(_ image: CGImage) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        imageLayer.contents = image
        dimLayer.contents = image
        CATransaction.commit()
    }

    func render(_ state: ZoomSessionState) {
        let previewChanged = renderedState?.isPreviewing != state.isPreviewing
        renderedState = state
        inkView.isHidden = !state.isFrozen
        applyGeometry(state, animated: previewChanged)
    }

    func showMessage(_ message: String) {
        messagePresentation += 1
        let presentation = messagePresentation
        messageLayer.string = message
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        messageLayer.isHidden = false
        messageLayer.opacity = 1
        CATransaction.commit()

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.1) { [weak self] in
            guard let self, messagePresentation == presentation else { return }
            CATransaction.begin()
            CATransaction.setAnimationDuration(0.2)
            CATransaction.setCompletionBlock { [weak self] in
                guard self?.messagePresentation == presentation else { return }
                self?.messageLayer.isHidden = true
            }
            messageLayer.opacity = 0
            CATransaction.commit()
        }
    }

    private func configureLayers() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        layer?.masksToBounds = true

        for imageLayer in [imageLayer, dimLayer] {
            imageLayer.contentsGravity = .resize
            imageLayer.magnificationFilter = .linear
            imageLayer.minificationFilter = .linear
            layer?.addSublayer(imageLayer)
        }
        if let colorControls {
            colorControls.setValue(0, forKey: kCIInputSaturationKey)
            dimLayer.filters = [colorControls]
        } else {
            NSLog("EduTools: CIColorControls was unavailable for zoom preview dimming")
        }
        dimMaskLayer.fillRule = .evenOdd
        dimMaskLayer.fillColor = NSColor.white.cgColor
        dimLayer.mask = dimMaskLayer
        dimLayer.isHidden = true

        previewBorderLayer.fillColor = NSColor.clear.cgColor
        previewBorderLayer.strokeColor = NSColor.controlAccentColor.cgColor
        previewBorderLayer.lineWidth = 2
        previewBorderLayer.isHidden = true
        layer?.addSublayer(previewBorderLayer)

        for textLayer in [previewLabelLayer, messageLayer] {
            textLayer.alignmentMode = .center
            textLayer.fontSize = 16
            textLayer.foregroundColor = NSColor.white.cgColor
            textLayer.backgroundColor = NSColor.black.withAlphaComponent(0.78).cgColor
            textLayer.cornerRadius = 8
            textLayer.contentsScale = window?.backingScaleFactor
                ?? NSScreen.main?.backingScaleFactor
                ?? 1
            layer?.addSublayer(textLayer)
        }
        previewLabelLayer.isHidden = true
        messageLayer.isHidden = true
    }

    private func configureInkView() {
        inkView.autoresizingMask = [.width, .height]
        inkView.isHidden = true
        inkView.keyDownInterceptor = { [weak self] event in
            self?.handleKeyDown(event) ?? false
        }
        inkView.keyUpInterceptor = { [weak self] event in
            self?.handleKeyUp(event) ?? false
        }
        inkView.scrollWheelInterceptor = { [weak self] event in
            self?.scrollInput?(event) ?? false
        }
        inkView.magnifyInterceptor = { [weak self] event in
            self?.magnifyInput?(event) ?? false
        }
        addSubview(inkView)
    }

    private func handleKeyDown(_ event: NSEvent) -> Bool {
        guard let definition = ShortcutCatalog.matching(event, scope: .zoom) else {
            return false
        }
        if !event.isARepeat {
            pressedShortcutDefinitions[event.keyCode] = definition
        }
        dispatcher.keyDown(
            definition: definition,
            scope: .zoom,
            displayUUID: displayUUID,
            isRepeat: event.isARepeat
        )
        return true
    }

    private func handleKeyUp(_ event: NSEvent) -> Bool {
        guard let definition = pressedShortcutDefinitions.removeValue(forKey: event.keyCode) else {
            return false
        }
        dispatcher.keyUp(definition: definition)
        return true
    }

    private func applyGeometry(_ state: ZoomSessionState, animated: Bool) {
        let sourceRect = state.sourceRect
        let contentsRect: CGRect
        if state.isPreviewing {
            contentsRect = CGRect(x: 0, y: 0, width: 1, height: 1)
        } else {
            contentsRect = CGRect(
                x: sourceRect.minX / state.screenSize.width,
                y: sourceRect.minY / state.screenSize.height,
                width: sourceRect.width / state.screenSize.width,
                height: sourceRect.height / state.screenSize.height
            )
        }

        if let colorControls {
            colorControls.setValue(
                -0.42 * state.previewDimming,
                forKey: kCIInputBrightnessKey
            )
        }
        let maskPath = CGMutablePath()
        maskPath.addRect(bounds)
        maskPath.addRect(sourceRect)

        CATransaction.begin()
        if animated {
            CATransaction.setAnimationDuration(ZoomDefaults.previewAnimationDuration)
            CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .easeInEaseOut))
        } else {
            CATransaction.setDisableActions(true)
        }
        imageLayer.contentsRect = contentsRect
        dimLayer.contentsRect = CGRect(x: 0, y: 0, width: 1, height: 1)
        dimMaskLayer.path = maskPath
        previewBorderLayer.path = CGPath(rect: sourceRect.insetBy(dx: 1, dy: 1), transform: nil)
        dimLayer.isHidden = !state.isPreviewing
        previewBorderLayer.isHidden = !state.isPreviewing
        previewLabelLayer.isHidden = !state.isPreviewing
        previewLabelLayer.string = ZoomGeometry.magnificationLabel(state.magnification)
        previewLabelLayer.frame = previewLabelFrame(for: sourceRect)
        CATransaction.commit()

        inkView.setPresentationSourceRect(
            state.isPreviewing ? nil : sourceRect,
            animated: animated
        )
    }

    private func previewLabelFrame(for sourceRect: CGRect) -> CGRect {
        let width = CGFloat(76)
        let height = CGFloat(30)
        let proposedX = sourceRect.maxX - width
        let proposedY = sourceRect.maxY + 6
        return CGRect(
            x: min(max(proposedX, 8), max(8, bounds.width - width - 8)),
            y: min(max(proposedY, 8), max(8, bounds.height - height - 8)),
            width: width,
            height: height
        )
    }
}

final class ZoomController {
    private let state: AppState
    private let dispatcher: AppCommandDispatcher
    weak var overlayController: OverlayController?

    private var panel: ZoomPanel?
    private var zoomView: ZoomView?
    private var captureSession: ZoomCaptureSession?
    private var zoomObserverIdentifier: UUID?
    private var screenObserver: NSObjectProtocol?
    private var targetScreenFrame: CGRect?
    private var targetDisplayUUID: String?
    private var previousApplication: NSRunningApplication?
    private var latestFrame: CGImage?
    private var previewOriginalFrame: CGImage?
    private var scrollAccumulator = CGFloat.zero
    private var magnifyAccumulator = CGFloat.zero

    init(state: AppState, dispatcher: AppCommandDispatcher) {
        self.state = state
        self.dispatcher = dispatcher
        zoomObserverIdentifier = state.addZoomObserver { [weak self] session in
            guard let session else { return }
            self?.zoomView?.render(session)
        }
        screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.screenConfigurationDidChange()
        }
    }

    func toggleZoom() {
        if state.isZoomEnabled {
            stopZoom()
        } else {
            startZoom()
        }
    }

    func startZoom() {
        guard !state.isZoomEnabled else { return }
        guard CGPreflightScreenCaptureAccess() else {
            let wasGranted = CGRequestScreenCaptureAccess()
            NSLog(
                "EduTools: zoom did not start because screen recording permission was missing (request result: %@)",
                wasGranted ? "granted" : "denied"
            )
            showAlert(
                title: "화면 기록 권한이 필요합니다",
                message: "시스템 설정에서 EduTools의 화면 및 시스템 오디오 녹음 권한을 허용한 뒤 줌 단축키를 다시 누르세요."
            )
            return
        }
        guard let screen = screenUnderMouse(),
              let displayUUID = displayUUID(for: screen),
              let displayID = displayID(for: screen)
        else {
            NSLog("EduTools: zoom could not resolve the screen under the pointer")
            showAlert(title: "줌을 시작할 수 없습니다", message: "마우스가 있는 화면을 찾지 못했습니다.")
            return
        }

        let mouseLocation = NSEvent.mouseLocation
        let localCenter = CGPoint(
            x: mouseLocation.x - screen.frame.minX,
            y: mouseLocation.y - screen.frame.minY
        )
        state.beginZoom(
            on: displayUUID,
            screenSize: screen.frame.size,
            center: localCenter
        )
        targetScreenFrame = screen.frame
        targetDisplayUUID = displayUUID

        let frontmostApplication = NSWorkspace.shared.frontmostApplication
        if frontmostApplication?.processIdentifier != ProcessInfo.processInfo.processIdentifier {
            previousApplication = frontmostApplication
        }

        let panel = makePanel(for: screen, displayUUID: displayUUID)
        self.panel = panel
        panel.orderFrontRegardless()
        NSApp.activate()
        focusZoomPanel()

        let scale = screen.backingScaleFactor
        let captureSession = ZoomCaptureSession(
            displayID: displayID,
            pixelSize: CGSize(
                width: screen.frame.width * scale,
                height: screen.frame.height * scale
            ),
            onFrame: { [weak self] frame in
                self?.receive(frame: frame)
            },
            onFailure: { [weak self] message in
                guard let self, self.state.isZoomEnabled else { return }
                stopZoom()
                showAlert(title: "화면 줌 캡처 오류", message: message)
            }
        )
        self.captureSession = captureSession
        captureSession.start()
    }

    func stopZoom() {
        guard state.isZoomEnabled || panel != nil else { return }
        let stoppedDisplayUUID = targetDisplayUUID
        captureSession?.stop()
        captureSession = nil
        zoomView = nil
        panel?.close()
        panel = nil
        targetScreenFrame = nil
        targetDisplayUUID = nil
        latestFrame = nil
        previewOriginalFrame = nil
        scrollAccumulator = 0
        magnifyAccumulator = 0
        state.endZoom()

        if state.isAnnotationModeEnabled, let stoppedDisplayUUID {
            overlayController?.focusPanelForBoard(on: stoppedDisplayUUID)
        } else if let previousApplication, !previousApplication.isTerminated {
            let activated = previousApplication.activate(from: .current, options: [])
            if !activated {
                NSLog(
                    "EduTools: failed to return focus after zoom to process %d",
                    previousApplication.processIdentifier
                )
            }
        }
        previousApplication = nil
    }

    func adjustMagnification(by steps: Int) {
        guard steps != 0, let magnification = state.adjustZoomMagnification(by: steps) else {
            return
        }
        zoomView?.showMessage(magnificationLabel(magnification))
    }

    func resetMagnification() {
        guard let magnification = state.resetZoomMagnification() else { return }
        zoomView?.showMessage(magnificationLabel(magnification))
    }

    func cycleFollowMode() {
        guard let mode = state.cycleZoomFollowMode() else { return }
        zoomView?.showMessage("따라가기: \(mode.displayName)")
    }

    func toggleFreeze() {
        if state.zoomSession?.isFrozen == false, latestFrame == nil {
            NSLog("EduTools: freeze zoom ignored until the first capture frame arrives")
            zoomView?.showMessage("첫 화면을 캡처하는 중입니다")
            return
        }
        guard let isFrozen = state.toggleZoomFrozen() else { return }
        if isFrozen {
            state.setAnnotationModeEnabled(true)
            zoomView?.showMessage("정지 줌 · 판서 켜짐")
        } else {
            zoomView?.showMessage("라이브 줌")
        }
        DispatchQueue.main.async { [weak self] in
            self?.focusZoomPanel()
        }
    }

    func beginPreview() {
        guard let session = state.zoomSession, !session.isPreviewing else { return }
        previewOriginalFrame = latestFrame
        state.beginZoomPreview()
        focusZoomPanel()
    }

    func finishPreview() {
        guard state.zoomSession?.isPreviewing == true else { return }
        state.finishZoomPreview(cancelled: false)
        previewOriginalFrame = nil
        focusZoomPanel()
    }

    func stopOrCancelPreview() {
        guard state.zoomSession?.isPreviewing == true else {
            stopZoom()
            return
        }
        state.finishZoomPreview(cancelled: true)
        if state.zoomSession?.isFrozen == true, let previewOriginalFrame {
            latestFrame = previewOriginalFrame
            zoomView?.setFrameImage(previewOriginalFrame)
        }
        self.previewOriginalFrame = nil
        focusZoomPanel()
    }

    private func makePanel(for screen: NSScreen, displayUUID: String) -> ZoomPanel {
        let panel = ZoomPanel(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        panel.backgroundColor = .black
        panel.isOpaque = true
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.isMovableByWindowBackground = false
        panel.isReleasedWhenClosed = false
        panel.animationBehavior = .none
        panel.level = NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 1)
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.acceptsMouseMovedEvents = true
        panel.sharingType = .none

        let view = ZoomView(
            frame: NSRect(origin: .zero, size: screen.frame.size),
            state: state,
            dispatcher: dispatcher,
            displayUUID: displayUUID,
            requestKeyFocus: { [weak self] in
                self?.focusZoomPanel()
            }
        )
        view.autoresizingMask = [.width, .height]
        view.pointerMoved = { [weak self] point in
            self?.pointerMoved(to: point)
        }
        view.scrollInput = { [weak self] event in
            self?.handleScroll(event) ?? false
        }
        view.magnifyInput = { [weak self] event in
            self?.handleMagnify(event) ?? false
        }
        panel.contentView = view
        zoomView = view
        if let session = state.zoomSession {
            view.render(session)
            view.showMessage(magnificationLabel(session.magnification))
        }
        return panel
    }

    private func receive(frame: CGImage) {
        guard let session = state.zoomSession else { return }
        if session.isFrozen && !session.isPreviewing { return }
        latestFrame = frame
        zoomView?.setFrameImage(frame)

        guard !session.isPreviewing,
              let screenFrame = targetScreenFrame
        else {
            return
        }
        let mouseLocation = NSEvent.mouseLocation
        state.advanceZoomFollow(
            toward: CGPoint(
                x: mouseLocation.x - screenFrame.minX,
                y: mouseLocation.y - screenFrame.minY
            )
        )
    }

    private func pointerMoved(to point: CGPoint) {
        guard let session = state.zoomSession else { return }
        if session.isPreviewing {
            state.setZoomCenter(point)
        } else if !session.isFrozen {
            state.advanceZoomFollow(toward: point)
        }
    }

    private func handleScroll(_ event: NSEvent) -> Bool {
        guard let session = state.zoomSession,
              session.isPreviewing || event.modifierFlags.contains(.command)
        else {
            return false
        }
        if event.phase == .began {
            scrollAccumulator = 0
        }
        scrollAccumulator += event.scrollingDeltaY
        let threshold = event.hasPreciseScrollingDeltas ? CGFloat(10) : CGFloat(1)
        let steps = Int(scrollAccumulator / threshold)
        if steps != 0 {
            scrollAccumulator -= CGFloat(steps) * threshold
            adjustMagnification(by: steps)
        }
        if event.phase == .ended || event.phase == .cancelled {
            scrollAccumulator = 0
        }
        return true
    }

    private func handleMagnify(_ event: NSEvent) -> Bool {
        guard state.isZoomEnabled else { return false }
        if event.phase == .began {
            magnifyAccumulator = 0
        }
        magnifyAccumulator += event.magnification
        let threshold = CGFloat(0.08)
        let steps = Int(magnifyAccumulator / threshold)
        if steps != 0 {
            magnifyAccumulator -= CGFloat(steps) * threshold
            adjustMagnification(by: steps)
        }
        if event.phase == .ended || event.phase == .cancelled {
            magnifyAccumulator = 0
        }
        return true
    }

    private func focusZoomPanel() {
        guard let panel, let zoomView else { return }
        panel.makeKeyAndOrderFront(nil)
        panel.makeFirstResponder(zoomView.preferredFirstResponder)
    }

    private func screenConfigurationDidChange() {
        guard let targetDisplayUUID else { return }
        guard let currentScreen = NSScreen.screens.first(where: {
            displayUUID(for: $0) == targetDisplayUUID
        }) else {
            NSLog("EduTools: zoom display disconnected; stopping its SCStream")
            stopZoom()
            showAlert(
                title: "화면 줌 종료",
                message: "줌 중이던 화면의 연결이 해제되어 캡처를 정리했습니다."
            )
            return
        }
        guard currentScreen.frame == targetScreenFrame else {
            NSLog("EduTools: zoom display geometry changed; stopping its SCStream")
            stopZoom()
            showAlert(
                title: "화면 줌 종료",
                message: "줌 중이던 화면의 해상도나 배치가 바뀌어 캡처를 정리했습니다. 다시 줌을 시작하세요."
            )
            return
        }
    }

    private func magnificationLabel(_ magnification: CGFloat) -> String {
        ZoomGeometry.magnificationLabel(magnification)
    }

    private func showAlert(title: String, message: String) {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "확인")
        alert.runModal()
    }

    deinit {
        captureSession?.stop()
        if let zoomObserverIdentifier {
            state.removeZoomObserver(zoomObserverIdentifier)
        }
        if let screenObserver {
            NotificationCenter.default.removeObserver(screenObserver)
        }
        panel?.close()
    }
}
