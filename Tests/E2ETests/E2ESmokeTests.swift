import AppKit
import CoreGraphics
import Foundation
import TestSupport
import Testing

@testable import EduTools

private enum E2ESmokeFailure: Error, CustomStringConvertible {
  case unmet(String)

  var description: String {
    switch self {
    case .unmet(let message): return message
    }
  }
}

@MainActor
@Suite("실제 앱 E2E 스모크", .serialized)
struct E2ESmokeTests {
  @Test("실제 번들은 격리 환경에서 실행되고 정상 종료하며 크래시 리포트를 남기지 않는다")
  func launchAndTerminateWithoutCrashReport() throws {
    try runScenario("launch-terminate") { harness in
      let launchDate = Date()
      try harness.launchApp()
      try dismissFirstRunUI(harness)

      let termination = try harness.stopApp()

      try require(
        termination.reason == .exit && termination.status == 0,
        "EduTools가 정상 종료 상태를 반환하지 않았습니다"
      )
      try require(
        !termination.requiredForce,
        "EduTools 정상 종료에 실패해 강제 종료가 필요했습니다"
      )
      let reports = try harness.crashReports(since: launchDate)
      try require(
        reports.isEmpty,
        "실행 시각 이후 EduTools 크래시 리포트가 생겼습니다: \(reports.map(\.lastPathComponent))"
      )
    }
  }

  @Test("기본 전역 단축키는 판서 모드를 켜고 화면 크기의 오버레이를 활성화한다")
  func annotationHotkeyShowsOverlay() throws {
    try runScenario("annotation-overlay") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      let displayBounds = E2EHarness.primaryDisplayBounds()

      try harness.post(try shortcut(for: .toggleAnnotationMode, scope: .global))

      try require(
        waitUntil {
          harness.largeWindows(on: displayBounds).isEmpty == false
            && NSWorkspace.shared.frontmostApplication?.processIdentifier
              == harness.processIdentifier
        },
        "판서 단축키 뒤 활성 오버레이 창을 확인하지 못했습니다"
      )
    }
  }

  @Test("마우스 획은 화면 픽셀을 바꾸고 제품 Undo 단축키는 원래 픽셀로 되돌린다")
  func strokeThenUndoRestoresPixels() throws {
    try runScenario("stroke-undo") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      try enableAnnotation(harness)
      let displayBounds = E2EHarness.primaryDisplayBounds()
      let path = E2EHarness.strokePath(in: displayBounds)
      let probeRect = E2EHarness.probeRect(for: path, in: displayBounds)
      let baselineURL = try harness.captureScreenshot(rect: probeRect, name: "baseline")
      let baseline = try E2EPixelImage(url: baselineURL)
      let threshold = changedPixelThreshold(for: baseline)

      try harness.postMouseDrag(through: path)
      let inkPixels = try waitForPixelDifference(
        harness: harness,
        baseline: baseline,
        rect: probeRect,
        name: "ink",
        condition: { $0 > threshold }
      )
      try require(inkPixels > threshold, "마우스 획이 화면 픽셀을 바꾸지 않았습니다")

      try harness.post(try shortcut(for: .undo, scope: .annotation))
      let remainingPixels = try waitForPixelDifference(
        harness: harness,
        baseline: baseline,
        rect: probeRect,
        name: "undo",
        condition: { $0 <= threshold }
      )
      try require(
        remainingPixels <= threshold,
        "Undo 뒤에도 획 픽셀이 남았습니다(변경 픽셀 \(remainingPixels))"
      )
    }
  }

  @Test("우클릭 드래그는 현재 화면의 획 픽셀을 줄인다")
  func secondaryDragErasesStroke() throws {
    try runScenario("secondary-erase") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      try enableAnnotation(harness)
      let displayBounds = E2EHarness.primaryDisplayBounds()
      let path = E2EHarness.strokePath(in: displayBounds)
      let probeRect = E2EHarness.probeRect(for: path, in: displayBounds)
      let baselineURL = try harness.captureScreenshot(rect: probeRect, name: "baseline")
      let baseline = try E2EPixelImage(url: baselineURL)
      let threshold = changedPixelThreshold(for: baseline)

      try harness.postMouseDrag(through: path)
      let inkPixels = try waitForPixelDifference(
        harness: harness,
        baseline: baseline,
        rect: probeRect,
        name: "before-erase",
        condition: { $0 > threshold }
      )
      try harness.postMouseDrag(through: Array(path.reversed()), button: .right)
      let erasedPixels = try waitForPixelDifference(
        harness: harness,
        baseline: baseline,
        rect: probeRect,
        name: "after-erase",
        condition: { $0 < inkPixels }
      )

      try require(
        erasedPixels < inkPixels,
        "우클릭 지우기 뒤 변경 픽셀이 줄지 않았습니다(전 \(inkPixels), 후 \(erasedPixels))"
      )
    }
  }

  @Test("M 팔레트에서 도구를 고르고 저장한 즐겨찾기는 격리 설정 suite에 남는다")
  func paletteSelectionPersistsFavorite() throws {
    try runScenario("palette-favorite") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      try enableAnnotation(harness)
      let displayBounds = E2EHarness.primaryDisplayBounds()
      let baselinePaletteArea =
        harness.windows().filter { window in
          window.frame.width < displayBounds.width * 0.5
            && window.frame.height < displayBounds.height * 0.5
            && abs(window.frame.width - window.frame.height)
              < min(window.frame.width, window.frame.height) * 0.1
        }.map { $0.frame.width * $0.frame.height }.max() ?? 0

      try harness.post(try shortcut(for: .toggleRadialPalette, scope: .annotation))
      try require(
        waitUntil {
          harness.windows().contains { window in
            let area = window.frame.width * window.frame.height
            return window.frame.width < displayBounds.width * 0.5
              && window.frame.height < displayBounds.height * 0.5
              && abs(window.frame.width - window.frame.height)
                < min(window.frame.width, window.frame.height) * 0.1
              && area > baselinePaletteArea * 4
          }
        },
        "M 입력 뒤 확장된 원형 팔레트 창을 찾지 못했습니다"
      )

      // 현재 펜 → 다음 도구(형광펜) 실행 → 그룹으로 나가기 → 즐겨찾기 → 저장 칸.
      for input in ["Right", "Return", "Down", "Left", "Up", "Return"] {
        try harness.post(try portableShortcut(input))
      }

      try require(
        waitUntil {
          guard let settings = persistedSettings(in: harness) else { return false }
          return settings.tools.favorites.count == 1
            && settings.tools.favorites.first?.inkTool == .highlighter
        },
        "팔레트에서 저장한 형광펜 즐겨찾기가 격리 설정에 기록되지 않았습니다"
      )
    }
  }

  @Test("변경한 전역 단축키는 재실행 뒤에도 유지되고 실제 핫키로 판서 모드를 켠다")
  func reassignedShortcutPersistsAndWorks() throws {
    try runScenario("shortcut-persistence") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      _ = try harness.stopApp()

      let originalSettings = AppSettings.default.shortcuts
      let annotation = try binding(
        for: .toggleAnnotationMode,
        scope: .global,
        settings: originalSettings
      )
      let help = try binding(for: .showHelp, scope: .global, settings: originalSettings)
      let annotationDefinition = try definition(
        for: .toggleAnnotationMode,
        settings: originalSettings
      )
      let helpDefinition = try definition(for: .showHelp, settings: originalSettings)
      let store = SettingsStore(defaults: harness.defaults)
      var reassigned = store.settings
      reassigned.shortcuts.overrides[
        ShortcutCatalog.overrideKey(
          definitionID: annotationDefinition.id,
          scope: .global
        )
      ] = help.portableString
      reassigned.shortcuts.overrides[
        ShortcutCatalog.overrideKey(
          definitionID: helpDefinition.id,
          scope: .global
        )
      ] = annotation.portableString
      try store.apply(reassigned)
      try require(harness.defaults.synchronize(), "단축키 설정 suite 동기화에 실패했습니다")

      let reloaded = SettingsStore(defaults: harness.defaults).settings
      try require(
        reloaded.shortcuts.overrides == reassigned.shortcuts.overrides,
        "재실행 전 단축키 override를 다시 읽지 못했습니다"
      )
      try harness.launchApp()
      try harness.post(
        try shortcut(
          for: .toggleAnnotationMode,
          scope: .global,
          settings: reloaded.shortcuts
        )
      )

      try require(
        waitUntil {
          NSWorkspace.shared.frontmostApplication?.processIdentifier
            == harness.processIdentifier
        },
        "재지정한 전역 단축키가 판서 모드를 활성화하지 않았습니다"
      )
    }
  }

  @Test("제품 줌 단축키는 화면 캡처 창을 시작하고 같은 단축키로 종료한다")
  func zoomHotkeyStartsAndStopsCaptureWindow() throws {
    try runScenario("zoom-start-stop") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      let displayBounds = E2EHarness.primaryDisplayBounds()
      let baselineCount = harness.largeWindows(on: displayBounds).count
      let zoomShortcut = try shortcut(for: .toggleZoom, scope: .global)

      try harness.post(zoomShortcut)
      try require(
        waitUntil {
          harness.largeWindows(on: displayBounds).count > baselineCount
        },
        "줌 단축키 뒤 화면 크기의 줌 창이 추가되지 않았습니다"
      )

      try harness.post(zoomShortcut)
      try require(
        waitUntil {
          harness.largeWindows(on: displayBounds).count == baselineCount
        },
        "두 번째 줌 단축키 뒤 줌 창이 종료되지 않았습니다"
      )
    }
  }

  @Test("메뉴바의 설정 항목은 실제 EduTools 설정 창을 연다")
  func statusMenuOpensSettingsWindow() throws {
    try runScenario("settings-window") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      let accessibility = try harness.accessibilityApplication()

      try accessibility.openStatusMenu()
      try require(
        waitUntil { accessibility.menuItemExists(title: "설정…") },
        "메뉴바에서 설정 항목을 찾지 못했습니다"
      )
      try accessibility.pressMenuItem(title: "설정…")
      try require(
        waitUntil { accessibility.windowExists(title: "EduTools 설정") },
        "설정 메뉴가 EduTools 설정 창을 열지 않았습니다"
      )
    }
  }

  @Test("메뉴바 화면 구성은 적용 없이 현재 프리셋 목록과 설정 진입점을 펼친다")
  func statusMenuShowsDisplayConfigurationListWithoutApplying() throws {
    try runScenario("display-configuration-menu") { harness in
      try harness.launchApp()
      try dismissFirstRunUI(harness)
      let accessibility = try harness.accessibilityApplication()

      try accessibility.openStatusMenu()
      try require(
        waitUntil { accessibility.menuItemExists(title: "화면 구성") },
        "메뉴바에서 화면 구성 항목을 찾지 못했습니다"
      )
      try accessibility.pressMenuItem(title: "화면 구성")
      try require(
        waitUntil { accessibility.menuItemExists(title: "화면 구성 설정…") },
        "화면 구성 하위 목록이 표시되지 않았습니다"
      )
    }
  }
}

@MainActor
private func runScenario(
  _ name: String,
  body: (E2EHarness) throws -> Void
) throws {
  if ProcessInfo.processInfo.environment["EDUTOOLS_E2E_PREFLIGHTED"] != "1" {
    requireE2EEnvironment()
    warnAndPerformE2ECountdown()
  }
  let harness = try E2EHarness(testName: name)
  defer { harness.cleanup() }
  do {
    try body(harness)
  } catch {
    do {
      _ = try harness.preserveFailureArtifact()
    } catch let artifactError {
      FileHandle.standardError.write(
        Data("EduTools E2E artifact capture failed: \(artifactError)\n".utf8)
      )
    }
    throw error
  }
}

@MainActor
private func dismissFirstRunUI(_ harness: E2EHarness) throws {
  let accessibility = try harness.accessibilityApplication()
  if waitUntil(condition: { accessibility.windowExists(title: "EduTools 권한 안내") }) {
    _ = try accessibility.closeWindow(title: "EduTools 권한 안내")
    try require(
      waitUntil { !accessibility.windowExists(title: "EduTools 권한 안내") },
      "첫 실행 권한 안내 창을 닫지 못했습니다"
    )
  }

  let displayBounds = E2EHarness.primaryDisplayBounds()
  if harness.largeWindows(on: displayBounds).count > 1 {
    try harness.post(try shortcut(for: .showHelp, scope: .global))
    try require(
      waitUntil {
        harness.largeWindows(on: displayBounds).count <= 1
      },
      "첫 실행 도움말을 닫지 못했습니다"
    )
  }
}

@MainActor
private func enableAnnotation(_ harness: E2EHarness) throws {
  try harness.post(try shortcut(for: .toggleAnnotationMode, scope: .global))
  try require(
    waitUntil {
      NSWorkspace.shared.frontmostApplication?.processIdentifier == harness.processIdentifier
    },
    "판서 모드가 입력을 받을 상태가 되지 않았습니다"
  )
}

private func binding(
  for command: AppCommand,
  scope: ShortcutScope,
  settings: ShortcutGroupSettings = AppSettings.default.shortcuts
) throws -> ShortcutBinding {
  let definition = try definition(for: command, settings: settings)
  guard let binding = definition.binding(in: scope) else {
    throw E2ESmokeFailure.unmet("제품 단축키 표에서 \(command) / \(scope)를 찾지 못했습니다")
  }
  return binding
}

private func definition(
  for command: AppCommand,
  settings: ShortcutGroupSettings
) throws -> ShortcutDefinition {
  guard
    let definition = ShortcutCatalog.resolvedDefinitions(for: settings).first(where: {
      $0.command == command
    })
  else {
    throw E2ESmokeFailure.unmet("제품 단축키 표에서 \(command)를 찾지 못했습니다")
  }
  return definition
}

private func shortcut(
  for command: AppCommand,
  scope: ShortcutScope,
  settings: ShortcutGroupSettings = AppSettings.default.shortcuts
) throws -> E2EKeystroke {
  try e2eKeystroke(from: binding(for: command, scope: scope, settings: settings))
}

private func portableShortcut(_ serialized: String) throws -> E2EKeystroke {
  guard let portable = PortableShortcut(serialized: serialized) else {
    throw E2ESmokeFailure.unmet("제품 단축키 모델이 \(serialized)를 해석하지 못했습니다")
  }
  return E2EKeystroke(
    keyCode: CGKeyCode(portable.key.keyCode),
    flags: cgFlags(from: portable.modifiers)
  )
}

private func e2eKeystroke(from binding: ShortcutBinding) throws -> E2EKeystroke {
  E2EKeystroke(
    keyCode: CGKeyCode(binding.keyCode),
    flags: cgFlags(from: binding.modifiers)
  )
}

private func cgFlags(from modifiers: NSEvent.ModifierFlags) -> CGEventFlags {
  var flags: CGEventFlags = []
  if modifiers.contains(.command) { flags.insert(.maskCommand) }
  if modifiers.contains(.shift) { flags.insert(.maskShift) }
  if modifiers.contains(.control) { flags.insert(.maskControl) }
  if modifiers.contains(.option) { flags.insert(.maskAlternate) }
  return flags
}

private func persistedSettings(in harness: E2EHarness) -> AppSettings? {
  if !harness.defaults.synchronize() {
    FileHandle.standardError.write(
      Data("EduTools E2E: 격리 설정 suite 동기화에 실패했습니다.\n".utf8)
    )
  }
  guard let data = harness.defaults.data(forKey: SettingsStore.storageKey) else { return nil }
  do {
    return try JSONDecoder().decode(AppSettings.self, from: data)
  } catch {
    FileHandle.standardError.write(
      Data("EduTools E2E: 격리 설정을 해석하지 못했습니다: \(error)\n".utf8)
    )
    return nil
  }
}

private func changedPixelThreshold(for image: E2EPixelImage) -> Int {
  max(4, Int(Double(image.pixelCount) * E2EPixelTolerance.changedPixelFraction))
}

private func waitForPixelDifference(
  harness: E2EHarness,
  baseline: E2EPixelImage,
  rect: CGRect,
  name: String,
  condition: (Int) -> Bool
) throws -> Int {
  var latestCount = 0
  var latestError: Error?
  let matched = waitUntil {
    do {
      let url = try harness.captureScreenshot(rect: rect, name: name)
      let image = try E2EPixelImage(url: url)
      latestCount = try image.changedPixelCount(
        from: baseline,
        tolerance: E2EPixelTolerance.channelDifference
      )
      return condition(latestCount)
    } catch {
      latestError = error
      return false
    }
  }
  if !matched, let latestError {
    throw latestError
  }
  return latestCount
}

private func require(_ condition: @autoclosure () -> Bool, _ message: String) throws {
  guard condition() else { throw E2ESmokeFailure.unmet(message) }
}
