import Testing

@testable import EduTools

enum SettingsRangeBoundary: Sendable {
  case decimal(Double)
  case integer(Int)

  var immediatelyBelow: SettingsRangeBoundary {
    switch self {
    case let .decimal(value): return .decimal(value.nextDown)
    case let .integer(value): return .integer(value - 1)
    }
  }

  var immediatelyAbove: SettingsRangeBoundary {
    switch self {
    case let .decimal(value): return .decimal(value.nextUp)
    case let .integer(value): return .integer(value + 1)
    }
  }

  var decimalValue: Double {
    guard case let .decimal(value) = self else {
      preconditionFailure("Expected a decimal settings boundary")
    }
    return value
  }

  var integerValue: Int {
    guard case let .integer(value) = self else {
      preconditionFailure("Expected an integer settings boundary")
    }
    return value
  }
}

struct SettingsRangeCase: Sendable, CustomTestStringConvertible {
  let name: String
  let minimum: SettingsRangeBoundary
  let maximum: SettingsRangeBoundary
  let setValue: @Sendable (inout AppSettings, SettingsRangeBoundary) -> Void

  var testDescription: String { name }
}

private let settingsRangeCases: [SettingsRangeCase] = [
  SettingsRangeCase(
    name: "형광펜 투명도",
    minimum: .decimal(0.05),
    maximum: .decimal(1),
    setValue: { $0.tools.highlighterOpacity = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "필압 최소 굵기",
    minimum: .decimal(0.05),
    maximum: .decimal(1),
    setValue: { $0.tools.pressureMinimumWidthRatio = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "레이저 크기",
    minimum: .decimal(2),
    maximum: .decimal(50),
    setValue: { $0.tools.laserSize = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "레이저 꼬리",
    minimum: .decimal(0.1),
    maximum: .decimal(10),
    setValue: { $0.tools.laserTrailDuration = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "지우개 지름",
    minimum: .decimal(8),
    maximum: .decimal(200),
    setValue: { $0.eraser.diameter = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "지우개 지름 프리셋",
    minimum: .decimal(8),
    maximum: .decimal(200),
    setValue: { $0.eraser.presetDiameters[0] = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "줌 시작 배율",
    minimum: .decimal(1.25),
    maximum: .decimal(8),
    setValue: { $0.zoom.startingMagnification = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "줌 최대 배율",
    minimum: .decimal(1.25),
    maximum: .decimal(8),
    setValue: {
      $0.zoom.startingMagnification = 1.25
      $0.zoom.maximumMagnification = $1.decimalValue
    }
  ),
  SettingsRangeCase(
    name: "줌 따라가기 부드러움",
    minimum: .decimal(0),
    maximum: .decimal(1),
    setValue: { $0.zoom.followSmoothness = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "줌 미리보기 회색 농도",
    minimum: .decimal(0),
    maximum: .decimal(1),
    setValue: { $0.zoom.previewDimming = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "수업 시간 프리셋",
    minimum: .integer(1),
    maximum: .integer(240),
    setValue: { $0.timer.lessonPresets[0] = $1.integerValue }
  ),
  SettingsRangeCase(
    name: "기본 수업 시간",
    minimum: .integer(1),
    maximum: .integer(240),
    setValue: { $0.timer.lessonDefaultMinutes = $1.integerValue }
  ),
  SettingsRangeCase(
    name: "기본 쉬는 시간",
    minimum: .integer(1),
    maximum: .integer(180),
    setValue: { $0.timer.breakDefaultMinutes = $1.integerValue }
  ),
  SettingsRangeCase(
    name: "페이드 시간",
    minimum: .decimal(1),
    maximum: .decimal(600),
    setValue: { $0.timer.fadeDuration = $1.decimalValue }
  ),
  SettingsRangeCase(
    name: "수업 배지 투명도",
    minimum: .decimal(0.4),
    maximum: .decimal(1),
    setValue: { $0.timer.lessonBadgeOpacity = $1.decimalValue }
  ),
]

@Suite("설정 값 범위")
struct SettingsRangeTests {
  @Test(
    "경계값은 허용하고 바로 바깥 값은 거부한다",
    arguments: settingsRangeCases
  )
  func validatesEachRangeBoundary(_ rangeCase: SettingsRangeCase) {
    expectValid(rangeCase.minimum, for: rangeCase)
    expectValid(rangeCase.maximum, for: rangeCase)
    expectInvalidValue(rangeCase.minimum.immediatelyBelow, for: rangeCase)
    expectInvalidValue(rangeCase.maximum.immediatelyAbove, for: rangeCase)
  }

  private func expectValid(
    _ value: SettingsRangeBoundary,
    for rangeCase: SettingsRangeCase
  ) {
    var settings = AppSettings.default
    rangeCase.setValue(&settings, value)

    do {
      try SettingsValidator.validate(settings)
    } catch {
      Issue.record("\(rangeCase.name) 경계값이 거부됨: \(error)")
    }
  }

  private func expectInvalidValue(
    _ value: SettingsRangeBoundary,
    for rangeCase: SettingsRangeCase
  ) {
    var settings = AppSettings.default
    rangeCase.setValue(&settings, value)

    do {
      try SettingsValidator.validate(settings)
      Issue.record("\(rangeCase.name) 범위 밖 값이 허용됨")
    } catch let error as SettingsValidationError {
      guard case .invalidValue = error else {
        Issue.record("\(rangeCase.name)이 잘못된 검증 오류를 반환함: \(error)")
        return
      }
    } catch {
      Issue.record("\(rangeCase.name)이 잘못된 오류 형식을 반환함: \(error)")
    }
  }
}
