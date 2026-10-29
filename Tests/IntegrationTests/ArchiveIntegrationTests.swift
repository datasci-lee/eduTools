import AppKit
import Darwin
import Foundation
import Testing
import TestSupport

@testable import EduTools

@Suite("연결 해제 판서 보관", .serialized)
struct ArchiveIntegrationTests {
  @Test("EDUTOOLS_DATA_DIR에서 보관한 판서는 내용과 undo 상태를 복원한다")
  func storesAndRestoresDisconnectedDisplayInIsolatedRoot() throws {
    let tempDir = try TempDir()
    #expect(setenv("EDUTOOLS_DATA_DIR", tempDir.url.path, 1) == 0)
    defer { unsetenv("EDUTOOLS_DATA_DIR") }
    let defaults = try IsolatedDefaults()
    let settingsStore = SettingsStore(defaults: defaults.value)
    let state = AppState(defaults: defaults.value, settingsStore: settingsStore)
    let displayUUID = "archive-display"
    let first = IntegrationFixtures.stroke(
      tool: .pen,
      color: NSColor(deviceRed: 0, green: 0.5333333333, blue: 1, alpha: 1),
      location: CGPoint(x: 0.25, y: 0.75),
      width: 3
    )
    let second = IntegrationFixtures.stroke(
      tool: .highlighter,
      color: NSColor(deviceRed: 1, green: 0.8, blue: 0, alpha: 1),
      location: CGPoint(x: 0.8, y: 0.2),
      width: 18
    )
    state.addStroke(first, to: displayUUID)
    state.addStroke(second, to: displayUUID)
    #expect(state.undo(on: displayUUID))
    let controller = ArchiveController(state: state, settingsStore: settingsStore)

    #expect(controller.storeDisconnectedDisplay(displayUUID))
    #expect(state.screenStrokes(for: displayUUID).isEmpty)
    let archiveURL = tempDir.url
      .appendingPathComponent("Disconnected", isDirectory: true)
      .appendingPathComponent(displayUUID)
      .appendingPathExtension("json")
    #expect(FileManager.default.fileExists(atPath: archiveURL.path))

    controller.restoreDisplaysArchivedForFailedPreset([displayUUID])
    #expect(state.screenStrokes(for: displayUUID) == [first])
    #expect(state.redo(on: displayUUID))
    #expect(state.screenStrokes(for: displayUUID) == [first, second])
    #expect(!FileManager.default.fileExists(atPath: archiveURL.path))
  }

  @Test("읽기 전용 저장 루트에서 보관 실패 시 원본 판서를 지우지 않는다")
  func readOnlyRootCancelsRemovalWithoutDataLoss() throws {
    let tempDir = try TempDir()
    let readOnlyRoot = tempDir.url.appendingPathComponent("read-only", isDirectory: true)
    try FileManager.default.createDirectory(at: readOnlyRoot, withIntermediateDirectories: false)
    try FileManager.default.setAttributes(
      [.posixPermissions: 0o500],
      ofItemAtPath: readOnlyRoot.path
    )
    defer {
      try? FileManager.default.setAttributes(
        [.posixPermissions: 0o700],
        ofItemAtPath: readOnlyRoot.path
      )
    }
    #expect(setenv("EDUTOOLS_DATA_DIR", readOnlyRoot.path, 1) == 0)
    defer { unsetenv("EDUTOOLS_DATA_DIR") }

    let defaults = try IsolatedDefaults()
    let settingsStore = SettingsStore(defaults: defaults.value)
    let state = AppState(defaults: defaults.value, settingsStore: settingsStore)
    let stroke = IntegrationFixtures.stroke(
      tool: .pen,
      color: .systemRed,
      location: CGPoint(x: 0.5, y: 0.5),
      width: 4
    )
    state.addStroke(stroke, to: "protected-display")
    let controller = ArchiveController(state: state, settingsStore: settingsStore)

    #expect(!controller.storeDisconnectedDisplay("protected-display"))
    #expect(state.screenStrokes(for: "protected-display") == [stroke])
    #expect(
      !FileManager.default.fileExists(
        atPath: readOnlyRoot.appendingPathComponent("Disconnected").path
      )
    )
  }
}
