import AppKit
import Foundation
import TestSupport

@testable import EduTools

enum IntegrationTestFoundationError: Error {
  case defaultsSuiteUnavailable(String)
}

final class IsolatedDefaults {
  let suiteName: String
  let value: UserDefaults

  init() throws {
    suiteName = "EduTools.IntegrationTests.\(UUID().uuidString)"
    guard let defaults = UserDefaults(suiteName: suiteName) else {
      throw IntegrationTestFoundationError.defaultsSuiteUnavailable(suiteName)
    }
    value = defaults
  }

  deinit {
    value.removePersistentDomain(forName: suiteName)
  }
}

enum IntegrationFixtures {
  static func state(defaults: UserDefaults) -> AppState {
    let store = SettingsStore(defaults: defaults)
    return AppState(defaults: defaults, settingsStore: store)
  }

  static func stroke(
    tool: InkStrokeTool,
    color: NSColor,
    location: CGPoint,
    width: CGFloat
  ) -> InkStroke {
    InkStroke(
      tool: tool,
      color: color,
      width: width,
      usesPressure: false,
      points: [InkPoint(location: location, pressure: 1, timestamp: 0)]
    )
  }
}
