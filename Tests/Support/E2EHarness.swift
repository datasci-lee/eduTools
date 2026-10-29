import AppKit
import ApplicationServices
import CoreGraphics
import Darwin
import Foundation
import ImageIO

public enum E2ETiming {
  public static let conditionTimeout: TimeInterval = 8
  public static let launchTimeout: TimeInterval = 12
  public static let gracefulTerminationTimeout: TimeInterval = 4
  public static let forcedTerminationTimeout: TimeInterval = 2
  public static let pollInterval: TimeInterval = 0.05
  public static let countdownSeconds = 10
}

public enum E2EPixelTolerance {
  public static let channelDifference: UInt8 = 18
  public static let changedPixelFraction = 0.0002
}

public enum E2EHarnessError: Error, CustomStringConvertible {
  case appBundleMissing(URL)
  case appExecutableMissing(URL)
  case appDidNotLaunch(URL)
  case appDidNotTerminate
  case executableUnavailable(String)
  case eventCreationFailed(String)
  case screenshotFailed(Int32)
  case screenshotUnreadable(URL)
  case screenshotSizeMismatch
  case accessibilityActionFailed(String, AXError)

  public var description: String {
    switch self {
    case .appBundleMissing(let url):
      return "앱 번들이 없습니다: \(url.path)"
    case .appExecutableMissing(let url):
      return "앱 실행 파일을 찾지 못했습니다: \(url.path)"
    case .appDidNotLaunch(let url):
      return "제한 시간 안에 앱이 실행되지 않았습니다: \(url.path)"
    case .appDidNotTerminate:
      return "정상 종료와 강제 종료 요청 뒤에도 앱 프로세스가 남았습니다"
    case .executableUnavailable(let name):
      return "PATH에서 실행 파일을 찾지 못했습니다: \(name)"
    case .eventCreationFailed(let kind):
      return "CGEvent를 만들지 못했습니다: \(kind)"
    case .screenshotFailed(let status):
      return "screencapture가 실패했습니다(종료코드 \(status))"
    case .screenshotUnreadable(let url):
      return "스크린샷을 읽지 못했습니다: \(url.path)"
    case .screenshotSizeMismatch:
      return "비교할 스크린샷의 픽셀 크기가 다릅니다"
    case .accessibilityActionFailed(let action, let error):
      return "AX 동작 \(action)이 실패했습니다: \(error.rawValue)"
    }
  }
}

public struct E2EKeystroke {
  public let keyCode: CGKeyCode
  public let flags: CGEventFlags

  public init(keyCode: CGKeyCode, flags: CGEventFlags = []) {
    self.keyCode = keyCode
    self.flags = flags
  }
}

public struct E2EWindowInfo: Equatable {
  public let number: CGWindowID
  public let frame: CGRect
  public let layer: Int
  public let name: String?
}

public struct E2ETermination {
  public let reason: Process.TerminationReason
  public let status: Int32
  public let requiredForce: Bool
}

@discardableResult
public func waitUntil(
  timeout: TimeInterval = E2ETiming.conditionTimeout,
  condition: () -> Bool
) -> Bool {
  let deadline = ProcessInfo.processInfo.systemUptime + timeout
  repeat {
    if condition() { return true }
    let remaining = deadline - ProcessInfo.processInfo.systemUptime
    if remaining <= 0 { break }
    RunLoop.current.run(
      until: Date(timeIntervalSinceNow: min(E2ETiming.pollInterval, remaining))
    )
  } while true
  return condition()
}

public func requireE2EEnvironment() {
  var failures: [String] = []
  if !hasWindowServerConnection() {
    failures.append(
      "WindowServer 연결 없음 — 로그인된 macOS 그래픽 세션의 터미널에서 실행하세요."
    )
  }
  if !AXIsProcessTrusted() {
    failures.append(
      "손쉬운 사용 권한 없음 — 시스템 설정 > 개인정보 보호 및 보안 > 손쉬운 사용에서 실행 터미널을 허용하세요."
    )
  }
  if !CGPreflightScreenCaptureAccess() {
    failures.append(
      "화면 기록 권한 없음 — 시스템 설정 > 개인정보 보호 및 보안 > 화면 및 시스템 오디오 녹음에서 실행 터미널을 허용하세요."
    )
  }

  let runningEduTools = NSWorkspace.shared.runningApplications.filter { application in
    application.bundleIdentifier == "kr.datasci.edutools"
      || application.localizedName == "EduTools"
      || application.executableURL?.deletingPathExtension().lastPathComponent == "EduTools"
  }
  if !runningEduTools.isEmpty {
    let processIDs = runningEduTools.map { String($0.processIdentifier) }.joined(separator: ", ")
    failures.append(
      "사용자 EduTools가 이미 실행 중입니다(PID \(processIDs)) — 앱을 종료한 뒤 격리 E2E를 다시 실행하세요."
    )
  }

  guard failures.isEmpty else {
    writeStandardError("UNRUNNABLE: EduTools E2E 사전 점검 실패\n")
    for failure in failures {
      writeStandardError("- \(failure)\n")
    }
    exit(3)
  }

  // Keep the shared GUI preflight as the WindowServer source of truth after the detailed check.
  requireWindowServer()
}

public func warnAndPerformE2ECountdown() {
  writeStandardError("이 시험은 마우스·키보드를 제어합니다. 10초 안에 Ctrl-C 로 취소\n")
  guard ProcessInfo.processInfo.environment["EDUTOOLS_E2E_NO_COUNTDOWN"] != "1" else {
    return
  }
  for _ in 0..<E2ETiming.countdownSeconds {
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 1))
  }
}

public final class E2EHarness {
  public let testName: String
  public let appURL: URL
  public let dataDirectory: TempDir
  public let defaultsSuiteName: String
  public let defaults: UserDefaults

  public private(set) var process: Process?
  public private(set) var lastScreenshotURL: URL?

  private let fileManager: FileManager
  private let repositoryRoot: URL
  private var cleanedUp = false

  public init(
    testName: String,
    fileManager: FileManager = .default,
    environment: [String: String] = ProcessInfo.processInfo.environment,
    arguments: [String] = CommandLine.arguments
  ) throws {
    self.testName = testName
    self.fileManager = fileManager
    repositoryRoot = try Self.findRepositoryRoot(fileManager: fileManager)
    appURL = Self.resolveAppURL(
      repositoryRoot: repositoryRoot,
      environment: environment,
      arguments: arguments
    )
    dataDirectory = try TempDir(fileManager: fileManager)
    defaultsSuiteName = "EduTools.E2E.\(UUID().uuidString)"
    guard let defaults = UserDefaults(suiteName: defaultsSuiteName) else {
      throw E2EHarnessError.appDidNotLaunch(appURL)
    }
    self.defaults = defaults
  }

  deinit {
    cleanup()
  }

  public var processIdentifier: pid_t? {
    guard let process, process.isRunning else { return nil }
    return process.processIdentifier
  }

  public func launchApp() throws {
    guard fileManager.fileExists(atPath: appURL.path) else {
      throw E2EHarnessError.appBundleMissing(appURL)
    }
    guard let executableURL = Bundle(url: appURL)?.executableURL,
      fileManager.isExecutableFile(atPath: executableURL.path)
    else {
      throw E2EHarnessError.appExecutableMissing(appURL)
    }
    guard process?.isRunning != true else { return }

    let launchedProcess = Process()
    launchedProcess.executableURL = executableURL
    var environment = ProcessInfo.processInfo.environment
    environment["EDUTOOLS_DATA_DIR"] = dataDirectory.url.path
    environment["EDUTOOLS_DEFAULTS_SUITE"] = defaultsSuiteName
    launchedProcess.environment = environment
    try launchedProcess.run()
    process = launchedProcess

    let launched = waitUntil(timeout: E2ETiming.launchTimeout) { [weak self] in
      guard let self, launchedProcess.isRunning else { return false }
      return !self.windows().isEmpty
    }
    guard launched else {
      cleanupProcess(launchedProcess)
      throw E2EHarnessError.appDidNotLaunch(appURL)
    }
  }

  @discardableResult
  public func stopApp() throws -> E2ETermination {
    guard let process else {
      return E2ETermination(reason: .exit, status: 0, requiredForce: false)
    }
    guard process.isRunning else {
      process.waitUntilExit()
      self.process = nil
      return E2ETermination(
        reason: process.terminationReason,
        status: process.terminationStatus,
        requiredForce: false
      )
    }

    let runningApplication = NSRunningApplication(
      processIdentifier: process.processIdentifier
    )
    if runningApplication?.terminate() != true {
      writeStandardError(
        "EduTools E2E: 정상 종료 요청을 전달하지 못해 종료 여부를 계속 확인합니다.\n"
      )
    }
    var requiredForce = false
    if !waitUntil(
      timeout: E2ETiming.gracefulTerminationTimeout,
      condition: {
        !process.isRunning
      })
    {
      requiredForce = true
      runningApplication?.forceTerminate()
      if !waitUntil(
        timeout: E2ETiming.forcedTerminationTimeout,
        condition: {
          !process.isRunning
        })
      {
        throw E2EHarnessError.appDidNotTerminate
      }
    }

    process.waitUntilExit()
    let result = E2ETermination(
      reason: process.terminationReason,
      status: process.terminationStatus,
      requiredForce: requiredForce
    )
    self.process = nil
    return result
  }

  public func cleanup() {
    guard !cleanedUp else { return }
    cleanedUp = true
    if let process, process.isRunning {
      do {
        _ = try stopApp()
      } catch {
        writeStandardError("EduTools E2E process cleanup failed: \(error)\n")
      }
    }
    defaults.removePersistentDomain(forName: defaultsSuiteName)
    if !defaults.synchronize() {
      writeStandardError(
        "EduTools E2E: UserDefaults suite 정리 동기화에 실패했습니다: \(defaultsSuiteName)\n"
      )
    }
    if fileManager.fileExists(atPath: dataDirectory.url.path) {
      do {
        try fileManager.removeItem(at: dataDirectory.url)
      } catch {
        writeStandardError(
          "EduTools E2E data directory cleanup failed at '\(dataDirectory.url.path)': \(error)\n"
        )
      }
    }
  }

  public func post(_ keystroke: E2EKeystroke) throws {
    guard
      let down = CGEvent(
        keyboardEventSource: nil,
        virtualKey: keystroke.keyCode,
        keyDown: true
      ),
      let up = CGEvent(
        keyboardEventSource: nil,
        virtualKey: keystroke.keyCode,
        keyDown: false
      )
    else {
      throw E2EHarnessError.eventCreationFailed("keyboard")
    }
    down.flags = keystroke.flags
    up.flags = keystroke.flags
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
  }

  public func postMouseClick(at point: CGPoint, button: CGMouseButton = .left) throws {
    let downType: CGEventType = button == .right ? .rightMouseDown : .leftMouseDown
    let upType: CGEventType = button == .right ? .rightMouseUp : .leftMouseUp
    try postMouse(type: downType, at: point, button: button)
    try postMouse(type: upType, at: point, button: button)
  }

  public func postMouseDrag(
    through points: [CGPoint],
    button: CGMouseButton = .left
  ) throws {
    guard let first = points.first, let last = points.last, points.count >= 2 else {
      throw E2EHarnessError.eventCreationFailed("mouse drag path")
    }
    let downType: CGEventType = button == .right ? .rightMouseDown : .leftMouseDown
    let dragType: CGEventType = button == .right ? .rightMouseDragged : .leftMouseDragged
    let upType: CGEventType = button == .right ? .rightMouseUp : .leftMouseUp
    try postMouse(type: .mouseMoved, at: first, button: button)
    try postMouse(type: downType, at: first, button: button)
    for point in points.dropFirst().dropLast() {
      try postMouse(type: dragType, at: point, button: button)
    }
    try postMouse(type: dragType, at: last, button: button)
    try postMouse(type: upType, at: last, button: button)
  }

  public func windows() -> [E2EWindowInfo] {
    guard let processIdentifier else { return [] }
    guard
      let rawWindows = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
      ) as? [[String: Any]]
    else {
      return []
    }
    return rawWindows.compactMap { entry in
      guard (entry[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == processIdentifier,
        let number = (entry[kCGWindowNumber as String] as? NSNumber)?.uint32Value,
        let bounds = entry[kCGWindowBounds as String] as? [String: Any],
        let frame = CGRect(dictionaryRepresentation: bounds as CFDictionary)
      else {
        return nil
      }
      return E2EWindowInfo(
        number: number,
        frame: frame,
        layer: (entry[kCGWindowLayer as String] as? NSNumber)?.intValue ?? 0,
        name: entry[kCGWindowName as String] as? String
      )
    }
  }

  public func largeWindows(on displayBounds: CGRect) -> [E2EWindowInfo] {
    let minimumArea = displayBounds.width * displayBounds.height * 0.8
    return windows().filter { window in
      let intersection = window.frame.intersection(displayBounds)
      return !intersection.isNull
        && intersection.width * intersection.height >= minimumArea
    }
  }

  public func accessibilityApplication() throws -> E2EAccessibilityApplication {
    guard let processIdentifier else {
      throw E2EHarnessError.appDidNotLaunch(appURL)
    }
    return E2EAccessibilityApplication(processIdentifier: processIdentifier)
  }

  @discardableResult
  public func captureScreenshot(
    rect: CGRect,
    name: String = "last"
  ) throws -> URL {
    let target = dataDirectory.url.appendingPathComponent("\(sanitized(name)).png")
    if fileManager.fileExists(atPath: target.path) {
      try fileManager.removeItem(at: target)
    }
    let executable = try Self.resolveExecutable(named: "screencapture")
    let integral = rect.standardized.integral
    let region = [
      Int(integral.origin.x),
      Int(integral.origin.y),
      Int(integral.width),
      Int(integral.height),
    ].map(String.init).joined(separator: ",")
    let capture = Process()
    capture.executableURL = executable
    capture.arguments = ["-x", "-R\(region)", target.path]
    try capture.run()
    capture.waitUntilExit()
    guard capture.terminationReason == .exit, capture.terminationStatus == 0 else {
      throw E2EHarnessError.screenshotFailed(capture.terminationStatus)
    }
    lastScreenshotURL = target
    return target
  }

  @discardableResult
  public func preserveFailureArtifact() throws -> URL {
    let artifactDirectory =
      repositoryRoot
      .appendingPathComponent("Tests/E2ETests/__artifacts__", isDirectory: true)
    try fileManager.createDirectory(
      at: artifactDirectory,
      withIntermediateDirectories: true
    )
    let timestamp = ISO8601DateFormatter().string(from: Date())
      .replacingOccurrences(of: ":", with: "-")
    let destination = artifactDirectory.appendingPathComponent(
      "\(sanitized(testName))-\(timestamp).png"
    )
    let source: URL
    if let lastScreenshotURL, fileManager.fileExists(atPath: lastScreenshotURL.path) {
      source = lastScreenshotURL
    } else {
      source = try captureScreenshot(rect: Self.primaryDisplayBounds(), name: "failure")
    }
    try fileManager.copyItem(at: source, to: destination)
    writeStandardError("EduTools E2E failure artifact: \(destination.path)\n")
    return destination
  }

  public func crashReports(since startDate: Date) throws -> [URL] {
    let library = try fileManager.url(
      for: .libraryDirectory,
      in: .userDomainMask,
      appropriateFor: nil,
      create: false
    )
    let directory = library.appendingPathComponent("Logs/DiagnosticReports", isDirectory: true)
    guard fileManager.fileExists(atPath: directory.path) else { return [] }
    let keys: Set<URLResourceKey> = [.contentModificationDateKey, .isRegularFileKey]
    return try fileManager.contentsOfDirectory(
      at: directory,
      includingPropertiesForKeys: Array(keys),
      options: [.skipsHiddenFiles]
    ).filter { url throws in
      guard url.lastPathComponent.hasPrefix("EduTools") else { return false }
      let values = try url.resourceValues(forKeys: keys)
      return values.isRegularFile == true
        && (values.contentModificationDate ?? .distantPast) >= startDate
    }
  }

  public static func primaryDisplayBounds() -> CGRect {
    CGDisplayBounds(CGMainDisplayID())
  }

  public static func strokePath(in displayBounds: CGRect) -> [CGPoint] {
    let start = CGPoint(
      x: displayBounds.minX + displayBounds.width * 0.36,
      y: displayBounds.minY + displayBounds.height * 0.48
    )
    let end = CGPoint(
      x: displayBounds.minX + displayBounds.width * 0.64,
      y: displayBounds.minY + displayBounds.height * 0.55
    )
    return (0...6).map { index in
      let progress = CGFloat(index) / 6
      return CGPoint(
        x: start.x + (end.x - start.x) * progress,
        y: start.y + (end.y - start.y) * progress
      )
    }
  }

  public static func probeRect(for points: [CGPoint], in displayBounds: CGRect) -> CGRect {
    guard let first = points.first else { return displayBounds }
    let bounds = points.dropFirst().reduce(CGRect(origin: first, size: .zero)) { result, point in
      result.union(CGRect(origin: point, size: .zero))
    }
    let inset = max(displayBounds.width, displayBounds.height) * 0.02
    return bounds.insetBy(dx: -inset, dy: -inset).intersection(displayBounds)
  }

  private func postMouse(
    type: CGEventType,
    at point: CGPoint,
    button: CGMouseButton
  ) throws {
    guard
      let event = CGEvent(
        mouseEventSource: nil,
        mouseType: type,
        mouseCursorPosition: point,
        mouseButton: button
      )
    else {
      throw E2EHarnessError.eventCreationFailed("mouse")
    }
    event.post(tap: .cghidEventTap)
  }

  private func cleanupProcess(_ process: Process) {
    guard process.isRunning else { return }
    NSRunningApplication(processIdentifier: process.processIdentifier)?.forceTerminate()
    _ = waitUntil(timeout: E2ETiming.forcedTerminationTimeout) { !process.isRunning }
  }

  private func sanitized(_ value: String) -> String {
    let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_"))
    return value.unicodeScalars.map { allowed.contains($0) ? String($0) : "-" }.joined()
  }

  private static func resolveAppURL(
    repositoryRoot: URL,
    environment: [String: String],
    arguments: [String]
  ) -> URL {
    let argumentValue: String? = arguments.indices.compactMap { index in
      guard arguments[index] == "--app", arguments.indices.contains(index + 1) else { return nil }
      return arguments[index + 1]
    }.first
    let path =
      argumentValue
      ?? environment["EDUTOOLS_E2E_APP"]
      ?? "dist/EduTools.app"
    let expanded = NSString(string: path).expandingTildeInPath
    if expanded.hasPrefix("/") {
      return URL(fileURLWithPath: expanded, isDirectory: true).standardizedFileURL
    }
    return repositoryRoot.appendingPathComponent(expanded, isDirectory: true).standardizedFileURL
  }

  private static func findRepositoryRoot(fileManager: FileManager) throws -> URL {
    var candidate = URL(fileURLWithPath: fileManager.currentDirectoryPath, isDirectory: true)
      .standardizedFileURL
    while candidate.path != candidate.deletingLastPathComponent().path {
      if fileManager.fileExists(
        atPath: candidate.appendingPathComponent("Package.swift").path
      ) {
        return candidate
      }
      candidate.deleteLastPathComponent()
    }
    throw E2EHarnessError.appBundleMissing(candidate)
  }

  private static func resolveExecutable(named name: String) throws -> URL {
    let pathEntries =
      ProcessInfo.processInfo.environment["PATH"]?
      .split(separator: ":")
      .map(String.init) ?? []
    for directory in pathEntries {
      let candidate = URL(fileURLWithPath: directory, isDirectory: true)
        .appendingPathComponent(name)
      if FileManager.default.isExecutableFile(atPath: candidate.path) {
        return candidate
      }
    }
    throw E2EHarnessError.executableUnavailable(name)
  }
}

public struct E2EPixelImage {
  public struct Pixel: Equatable {
    public let red: UInt8
    public let green: UInt8
    public let blue: UInt8
    public let alpha: UInt8

    public func differs(from other: Pixel, tolerance: UInt8) -> Bool {
      func difference(_ lhs: UInt8, _ rhs: UInt8) -> UInt8 {
        lhs >= rhs ? lhs - rhs : rhs - lhs
      }
      return difference(red, other.red) > tolerance
        || difference(green, other.green) > tolerance
        || difference(blue, other.blue) > tolerance
        || difference(alpha, other.alpha) > tolerance
    }
  }

  public let width: Int
  public let height: Int
  private let pixels: [UInt8]

  public init(url: URL) throws {
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else {
      throw E2EHarnessError.screenshotUnreadable(url)
    }
    width = image.width
    height = image.height
    var storage = [UInt8](repeating: 0, count: width * height * 4)
    guard let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
      let context = CGContext(
        data: &storage,
        width: width,
        height: height,
        bitsPerComponent: 8,
        bytesPerRow: width * 4,
        space: colorSpace,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
      )
    else {
      throw E2EHarnessError.screenshotUnreadable(url)
    }
    context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
    pixels = storage
  }

  public var pixelCount: Int { width * height }

  public func pixel(x: Int, y: Int) -> Pixel? {
    guard (0..<width).contains(x), (0..<height).contains(y) else { return nil }
    let offset = (y * width + x) * 4
    return Pixel(
      red: pixels[offset],
      green: pixels[offset + 1],
      blue: pixels[offset + 2],
      alpha: pixels[offset + 3]
    )
  }

  public func changedPixelCount(
    from other: E2EPixelImage,
    tolerance: UInt8
  ) throws -> Int {
    guard width == other.width, height == other.height else {
      throw E2EHarnessError.screenshotSizeMismatch
    }
    var count = 0
    for index in 0..<pixelCount {
      let offset = index * 4
      let lhs = Pixel(
        red: pixels[offset],
        green: pixels[offset + 1],
        blue: pixels[offset + 2],
        alpha: pixels[offset + 3]
      )
      let rhs = Pixel(
        red: other.pixels[offset],
        green: other.pixels[offset + 1],
        blue: other.pixels[offset + 2],
        alpha: other.pixels[offset + 3]
      )
      if lhs.differs(from: rhs, tolerance: tolerance) {
        count += 1
      }
    }
    return count
  }
}

public final class E2EAccessibilityApplication {
  private let root: AXUIElement

  public init(processIdentifier: pid_t) {
    root = AXUIElementCreateApplication(processIdentifier)
  }

  public func windowExists(title: String) -> Bool {
    findElement(role: kAXWindowRole as String, title: title) != nil
  }

  @discardableResult
  public func closeWindow(title: String) throws -> Bool {
    guard let window = findElement(role: kAXWindowRole as String, title: title) else {
      return false
    }
    guard let closeButton = elementAttribute(kAXCloseButtonAttribute as String, from: window) else {
      throw E2EHarnessError.accessibilityActionFailed("window close button", .noValue)
    }
    try perform(kAXPressAction as String, on: closeButton)
    return true
  }

  public func menuItemExists(title: String) -> Bool {
    findElement(role: kAXMenuItemRole as String, title: title) != nil
  }

  public func openStatusMenu() throws {
    guard
      let statusItem = allElements().first(where: { element in
        stringAttribute(kAXRoleAttribute as String, from: element) == kAXMenuBarItemRole as String
          && supports(kAXPressAction as String, element: element)
      })
    else {
      throw E2EHarnessError.accessibilityActionFailed("status menu lookup", .noValue)
    }
    try perform(kAXPressAction as String, on: statusItem)
  }

  public func pressMenuItem(title: String) throws {
    guard let item = findElement(role: kAXMenuItemRole as String, title: title) else {
      throw E2EHarnessError.accessibilityActionFailed("menu item \(title)", .noValue)
    }
    try perform(kAXPressAction as String, on: item)
  }

  private func findElement(role: String, title: String) -> AXUIElement? {
    allElements().first { element in
      stringAttribute(kAXRoleAttribute as String, from: element) == role
        && labels(for: element).contains(title)
    }
  }

  private func labels(for element: AXUIElement) -> [String] {
    [
      kAXTitleAttribute as String,
      kAXDescriptionAttribute as String,
      kAXHelpAttribute as String,
      kAXValueAttribute as String,
    ].compactMap { stringAttribute($0, from: element) }
  }

  private func allElements() -> [AXUIElement] {
    var result: [AXUIElement] = []
    var queue: [AXUIElement] = [root]
    var visited: Set<CFHashCode> = []
    while !queue.isEmpty {
      let element = queue.removeFirst()
      let identifier = CFHash(element)
      guard visited.insert(identifier).inserted else { continue }
      result.append(element)
      for attribute in [
        kAXChildrenAttribute as String,
        kAXWindowsAttribute as String,
        kAXMenuBarAttribute as String,
      ] {
        queue.append(contentsOf: elementArrayAttribute(attribute, from: element))
      }
    }
    return result
  }

  private func elementArrayAttribute(
    _ attribute: String,
    from element: AXUIElement
  ) -> [AXUIElement] {
    guard let value = copyAttribute(attribute, from: element) else { return [] }
    if let array = value as? [AXUIElement] { return array }
    if CFGetTypeID(value) == AXUIElementGetTypeID() {
      return [unsafeBitCast(value, to: AXUIElement.self)]
    }
    return []
  }

  private func stringAttribute(_ attribute: String, from element: AXUIElement) -> String? {
    copyAttribute(attribute, from: element) as? String
  }

  private func elementAttribute(_ attribute: String, from element: AXUIElement) -> AXUIElement? {
    guard let value = copyAttribute(attribute, from: element),
      CFGetTypeID(value) == AXUIElementGetTypeID()
    else {
      return nil
    }
    return unsafeBitCast(value, to: AXUIElement.self)
  }

  private func copyAttribute(_ attribute: String, from element: AXUIElement) -> CFTypeRef? {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
    return status == .success ? value : nil
  }

  private func supports(_ action: String, element: AXUIElement) -> Bool {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success else { return false }
    return (names as? [String])?.contains(action) == true
  }

  private func perform(_ action: String, on element: AXUIElement) throws {
    let status = AXUIElementPerformAction(element, action as CFString)
    guard status == .success else {
      throw E2EHarnessError.accessibilityActionFailed(action, status)
    }
  }
}

private func writeStandardError(_ message: String) {
  FileHandle.standardError.write(Data(message.utf8))
}

#if E2E_ENVIRONMENT_CHECK_MAIN
  @main
  private enum E2EEnvironmentCheck {
    static func main() {
      requireE2EEnvironment()
      warnAndPerformE2ECountdown()
    }
  }
#endif
