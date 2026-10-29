import Foundation
import Testing

@testable import EduTools

@Suite("설정")
struct SettingsTests {
  @Test("기본 설정은 JSON 왕복 후에도 값이 바뀌지 않는다")
  func jsonRoundTripPreservesValues() throws {
    let data = try JSONEncoder().encode(AppSettings.default)
    let decoded = try JSONDecoder().decode(AppSettings.self, from: data)

    #expect(decoded == AppSettings.default)
  }

  @Test("최대 줌 배율은 1.25에서 8 범위 밖의 값을 거부한다")
  func validationRejectsOutOfRangeZoom() {
    var settings = AppSettings.default
    settings.zoom.maximumMagnification = 9

    #expect(throws: SettingsValidationError.self) {
      try SettingsValidator.validate(settings)
    }
  }

  @Test("설정 디코딩은 알 수 없는 키를 무시하고 누락된 호환 값을 기본값으로 채운다")
  func decodingIgnoresUnknownKeysAndDefaultsMissingValues() throws {
    let data = try JSONEncoder().encode(AppSettings.default)
    let jsonObject = try JSONSerialization.jsonObject(with: data)
    var object = try #require(jsonObject as? [String: Any])
    object["futureSetting"] = ["enabled": true]
    object.removeValue(forKey: "display")

    var zoom = try #require(object["zoom"] as? [String: Any])
    zoom.removeValue(forKey: "previewDimming")
    object["zoom"] = zoom

    var tools = try #require(object["tools"] as? [String: Any])
    let memory = AppSettings.default.tools.perToolMemory
    tools["penWidth"] = memory.pen.size
    tools["highlighterWidth"] = memory.highlighter.size
    tools["textFontSize"] = memory.text.size
    tools.removeValue(forKey: "perToolMemory")
    tools.removeValue(forKey: "paletteGlassOpacity")
    tools.removeValue(forKey: "pressureMinimumWidthRatio")
    object["tools"] = tools

    let compatibleData = try JSONSerialization.data(withJSONObject: object)
    let decoded = try JSONDecoder().decode(AppSettings.self, from: compatibleData)

    #expect(decoded.tools.pressureMinimumWidthRatio == AppSettings.default.tools.pressureMinimumWidthRatio)
    #expect(decoded == AppSettings.default)
  }

  @Test("기본 내보내기는 화면 프리셋을 포함하고 컴퓨터별 스위치와 경로를 제외한다")
  func exportIncludesPresetsAndExcludesComputerSpecificValues() throws {
    var settings = AppSettings.default
    settings.general.archiveFolderPath = "/Volumes/Classroom/Archive"
    settings.display = DisplaySettings(
      autoSwitchEnabled: true,
      presets: [
        DisplayPreset(
          name: "교실",
          main: PresetScreen(
            monitors: [
              PresetMonitor(
                identity: DisplayIdentity(
                  uuid: "display-test-uuid",
                  vendor: 1,
                  model: 2,
                  serial: 3,
                  name: "교실 모니터",
                  kind: .external
                ),
                autoMatch: true
              )
            ]
          )
        )
      ]
    )

    let document = try SettingsTransfer.exportDocument(settings: settings)
    let exportedData = try SettingsTransfer.encode(document)
    let exportedObject = try #require(
      try JSONSerialization.jsonObject(with: exportedData) as? [String: Any]
    )
    let payloadObject = try #require(exportedObject["payload"] as? [String: Any])
    let displayObject = try #require(payloadObject["display"] as? [String: Any])
    let imported = try SettingsTransfer.appliedSettings(
      current: .default,
      document: document,
      selectedBundles: Set(document.includedBundles)
    )

    #expect(!document.includesComputerSpecific)
    #expect(document.payload.general?.archiveFolderPath == nil)
    #expect(document.payload.display?.presets == settings.display.presets)
    #expect(document.includedBundles.contains(.display))
    #expect(displayObject["autoSwitchEnabled"] == nil)
    #expect(imported.display.presets == settings.display.presets)
    #expect(!imported.display.autoSwitchEnabled)
  }

  @Test(
    "설정 묶음은 같은 이름의 payload 하나만 내보내고 그 묶음만 적용한다",
    arguments: SettingsBundle.allCases
  )
  func eachBundleExportsAndAppliesOnlyItsPayload(_ bundle: SettingsBundle) throws {
    let payloadKeyByBundle: [SettingsBundle: String] = [
      .general: "general",
      .tools: "tools",
      .eraser: "eraser",
      .zoom: "zoom",
      .board: "board",
      .timer: "timer",
      .display: "display",
      .shortcuts: "shortcuts",
      .help: "help",
    ]
    let presetID = try #require(UUID(uuidString: "11111111-2222-3333-4444-555555555555"))
    var changed = AppSettings.default
    changed.general.launchAtLogin = true
    changed.tools.paletteStyle = .linear
    changed.eraser.mode = "stroke"
    changed.zoom.startingMagnification = 2.5
    changed.board.backgroundPattern = "grid"
    changed.timer.lessonDefaultMinutes = 45
    changed.display.presets = [
      DisplayPreset(
        id: presetID,
        name: "교실",
        main: PresetScreen(
          monitors: [
            PresetMonitor(
              identity: DisplayIdentity(
                uuid: "settings-bundle-display",
                vendor: 1,
                model: 2,
                serial: 3,
                name: "교실 모니터",
                kind: .external
              )
            ),
          ]
        )
      ),
    ]
    changed.shortcuts.appModifier = .controlOptionShift
    changed.help.showsHelpInSharing = true

    let document = try SettingsTransfer.exportDocument(
      settings: changed,
      bundles: [bundle],
      includeComputerSpecific: true
    )
    let payloadObject = try #require(
      try JSONSerialization.jsonObject(with: JSONEncoder().encode(document.payload))
        as? [String: Any]
    )
    let expectedPayloadKey = try #require(payloadKeyByBundle[bundle])
    let imported = try SettingsTransfer.appliedSettings(
      current: .default,
      document: document,
      selectedBundles: [bundle]
    )
    var expected = AppSettings.default
    switch bundle {
    case .general: expected.general = changed.general
    case .tools: expected.tools = changed.tools
    case .eraser: expected.eraser = changed.eraser
    case .zoom: expected.zoom = changed.zoom
    case .board: expected.board = changed.board
    case .timer: expected.timer = changed.timer
    case .display: expected.display.presets = changed.display.presets
    case .shortcuts: expected.shortcuts = changed.shortcuts
    case .help: expected.help = changed.help
    }

    #expect(document.includedBundles == [bundle])
    #expect(Set(payloadObject.keys) == [expectedPayloadKey])
    #expect(imported == expected)
  }

  @Test("피하도록 정한 전역 단축키는 충돌로 보고한다")
  func avoidedGlobalShortcutIsReportedAsConflict() {
    #expect(ShortcutConflictDetector.hasAvoidedConflict("AppMod+D"))
  }
}
