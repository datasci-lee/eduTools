import Foundation
import Testing

@testable import EduTools

struct SettingsTokenCase: Sendable, CustomTestStringConvertible {
  let fieldName: String
  let tokens: [String]
  let setValue: @Sendable (inout AppSettings, String) -> Void

  var testDescription: String { fieldName }
}

private let settingsTokenCases: [SettingsTokenCase] = [
  SettingsTokenCase(
    fieldName: "general.displayDisconnectPolicy",
    tokens: ["archive", "moveToPrimary", "delete"],
    setValue: { $0.general.displayDisconnectPolicy = $1 }
  ),
  SettingsTokenCase(
    fieldName: "eraser.mode",
    tokens: ["partial", "stroke", "area"],
    setValue: { $0.eraser.mode = $1 }
  ),
  SettingsTokenCase(
    fieldName: "eraser.hitRule",
    tokens: ["touching", "contained"],
    setValue: { $0.eraser.hitRule = $1 }
  ),
  SettingsTokenCase(
    fieldName: "eraser.targetFilter",
    tokens: ["all", "penOnly", "highlighterOnly"],
    setValue: { $0.eraser.targetFilter = $1 }
  ),
  SettingsTokenCase(
    fieldName: "eraser.areaShape",
    tokens: ["rectangle", "lasso"],
    setValue: { $0.eraser.areaShape = $1 }
  ),
  SettingsTokenCase(
    fieldName: "board.backgroundPattern",
    tokens: ["none", "grid", "lines"],
    setValue: { $0.board.backgroundPattern = $1 }
  ),
  SettingsTokenCase(
    fieldName: "timer.lessonBadgeSize",
    tokens: ["small", "medium", "large"],
    setValue: { $0.timer.lessonBadgeSize = $1 }
  ),
  SettingsTokenCase(
    fieldName: "timer.lessonDisplayMode",
    tokens: ["remaining", "elapsed"],
    setValue: { $0.timer.lessonDisplayMode = $1 }
  ),
]

@Suite("설정 문자열 토큰")
struct SettingsTokenTests {
  @Test(
    "정의된 토큰만 검증을 통과한다",
    arguments: settingsTokenCases
  )
  func validatorAcceptsOnlyDefinedTokens(_ tokenCase: SettingsTokenCase) {
    for token in tokenCase.tokens {
      var settings = AppSettings.default
      tokenCase.setValue(&settings, token)
      do {
        try SettingsValidator.validate(settings)
      } catch {
        Issue.record("\(tokenCase.fieldName)의 \(token) 토큰이 거부됨: \(error)")
      }
    }

    var settings = AppSettings.default
    tokenCase.setValue(&settings, "bogus")
    do {
      try SettingsValidator.validate(settings)
      Issue.record("\(tokenCase.fieldName)의 bogus 토큰이 허용됨")
    } catch let error as SettingsValidationError {
      guard case .invalidValue = error else {
        Issue.record("\(tokenCase.fieldName)이 잘못된 검증 오류를 반환함: \(error)")
        return
      }
    } catch {
      Issue.record("\(tokenCase.fieldName)이 잘못된 오류 형식을 반환함: \(error)")
    }
  }

  @Test("런타임 enum raw value는 저장 토큰 집합과 같다")
  func enumRawValuesMatchStoredTokenSets() {
    #expect(
      Set(DisplayDisconnectPolicy.allCases.map(\.rawValue))
        == Set(["archive", "moveToPrimary", "delete"])
    )
    #expect(
      Set(EraserMode.allCases.map(\.rawValue))
        == Set(["partial", "stroke", "area"])
    )
    #expect(
      Set(EraserHitRule.allCases.map(\.rawValue))
        == Set(["touching", "contained"])
    )
    #expect(
      Set(EraserTargetFilter.allCases.map(\.rawValue))
        == Set(["all", "penOnly", "highlighterOnly"])
    )
    #expect(
      Set(EraserAreaShape.allCases.map(\.rawValue))
        == Set(["rectangle", "lasso"])
    )
    #expect(
      Set(BoardBackgroundPattern.allCases.map(\.rawValue))
        == Set(["none", "grid", "lines"])
    )
    #expect(
      Set(LessonTimerController.SizePreset.allCases.map(\.rawValue))
        == Set(["small", "medium", "large"])
    )
    #expect(
      Set(LessonTimerController.DisplayMode.allCases.map(\.rawValue))
        == Set(["remaining", "elapsed"])
    )
  }

  @Test("AppState 변경은 설정 토큰을 그대로 저장한다")
  func appStateWritesSettingsTokens() throws {
    let (suiteName, defaults, store, state) = try makeState()
    defer { defaults.removePersistentDomain(forName: suiteName) }

    state.setDisplayDisconnectPolicy(.moveToPrimary)
    #expect(store.settings.general.displayDisconnectPolicy == "moveToPrimary")
    state.setEraserMode(.area)
    #expect(store.settings.eraser.mode == "area")
    state.setEraserHitRule(.contained)
    #expect(store.settings.eraser.hitRule == "contained")
    state.setEraserTargetFilter(.highlighterOnly)
    #expect(store.settings.eraser.targetFilter == "highlighterOnly")
    state.setEraserAreaShape(.lasso)
    #expect(store.settings.eraser.areaShape == "lasso")
    state.setBoardBackgroundPattern(.lines)
    #expect(store.settings.board.backgroundPattern == "lines")
  }

  @Test("AppState는 알 수 없는 설정 토큰을 기존 기본값으로 적용한다")
  func appStateFallsBackForUnknownTokens() throws {
    let (suiteName, defaults, store, state) = try makeState()
    defer { defaults.removePersistentDomain(forName: suiteName) }
    var settings = store.settings
    settings.general.displayDisconnectPolicy = "bogus"
    settings.eraser.mode = "bogus"
    settings.eraser.hitRule = "bogus"
    settings.eraser.targetFilter = "bogus"
    settings.eraser.areaShape = "bogus"
    settings.board.backgroundPattern = "bogus"

    store.settings = settings

    #expect(state.displayDisconnectPolicy == .archive)
    #expect(state.eraserMode == .partial)
    #expect(state.eraserHitRule == .touching)
    #expect(state.eraserTargetFilter == .all)
    #expect(state.eraserAreaShape == .rectangle)
    #expect(state.boardBackgroundPattern == .none)
  }

  private func makeState() throws -> (String, UserDefaults, SettingsStore, AppState) {
    let suiteName = "EduTools.SettingsTokenTests.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suiteName))
    let store = SettingsStore(defaults: defaults)
    let state = AppState(defaults: defaults, settingsStore: store)
    return (suiteName, defaults, store, state)
  }
}
