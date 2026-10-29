import Foundation
import Testing

@testable import EduTools

@Suite("버전별 설정 골든")
struct SettingsGoldenIntegrationTests {
  private var fixtureRoot: URL {
    URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent()
      .deletingLastPathComponent()
      .appendingPathComponent("Fixtures/settings", isDirectory: true)
  }

  @Test("모든 v* 설정 파일은 검증·저장 왕복 뒤 같은 값을 보존한다")
  func everyVersionedFixtureRoundTripsThroughSettingsStore() throws {
    if ProcessInfo.processInfo.environment["UPDATE_GOLDEN"] == "1" {
      let data = try Self.goldenCandidateData()
      let outputURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("EduTools-settings-v0.1.0.json")
      try data.write(to: outputURL, options: .atomic)
      print("golden candidate: \(outputURL.path)")
      return
    }

    let fileManager = FileManager.default
    let versionDirectories = try fileManager.contentsOfDirectory(
      at: fixtureRoot,
      includingPropertiesForKeys: [.isDirectoryKey],
      options: [.skipsHiddenFiles]
    ).filter { url in
      url.lastPathComponent.hasPrefix("v")
        && (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
    }.sorted { $0.lastPathComponent < $1.lastPathComponent }
    #expect(!versionDirectories.isEmpty)

    let fixtureURLs = try versionDirectories.flatMap { directory in
      try fileManager.contentsOfDirectory(
        at: directory,
        includingPropertiesForKeys: [.isRegularFileKey],
        options: [.skipsHiddenFiles]
      ).filter { $0.pathExtension.lowercased() == "json" }
    }.sorted { $0.path < $1.path }
    #expect(!fixtureURLs.isEmpty)

    for fixtureURL in fixtureURLs {
      let fixtureData = try Data(contentsOf: fixtureURL)
      let expected = try JSONDecoder().decode(AppSettings.self, from: fixtureData)
      try SettingsValidator.validate(expected)

      let defaults = try IsolatedDefaults()
      defaults.value.set(fixtureData, forKey: SettingsStore.storageKey)
      let store = SettingsStore(defaults: defaults.value)
      #expect(store.settings == expected, "\(fixtureURL.lastPathComponent) 복호화 값")

      var changed = expected
      changed.timer.lessonDefaultMinutes += 1
      try store.apply(changed)
      try store.apply(expected)

      let persistedData = try #require(
        defaults.value.data(forKey: SettingsStore.storageKey),
        "\(fixtureURL.lastPathComponent) 재저장 데이터"
      )
      let persisted = try JSONDecoder().decode(AppSettings.self, from: persistedData)
      #expect(persisted == expected, "\(fixtureURL.lastPathComponent) 재저장 왕복 값")
    }
  }

  private static func goldenCandidateData() throws -> Data {
    let defaults = try IsolatedDefaults()
    let store = SettingsStore(defaults: defaults.value)
    var candidate = store.settings
    candidate.general.autoArchiveEnabled = true
    candidate.zoom.startingMagnification = 2.5
    candidate.timer.lessonDefaultMinutes = 42
    try store.apply(candidate)
    let storedData = try #require(defaults.value.data(forKey: SettingsStore.storageKey))
    let stored = try JSONDecoder().decode(AppSettings.self, from: storedData)
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
    return try encoder.encode(stored)
  }
}
