import Foundation
import TestSupport
import Testing

@testable import EduTools

enum AttrIconVectorError: Error {
  case unsupportedVersion(Int)
}

struct AttrIconWidthVectorFile: Decodable, Sendable {
  let version: Int
  let cases: [AttrIconWidthVectorCase]
}

struct AttrIconWidthVectorCase: Decodable, Sendable {
  let name: String
  let input: Input
  let expected: Int

  struct Input: Decodable, Sendable {
    let value: Double
    let lowerBound: Double
    let upperBound: Double
  }
}

struct AttrIconConditionVectorFile: Decodable, Sendable {
  let version: Int
  let cases: [AttrIconConditionVectorCase]
}

struct AttrIconConditionVectorCase: Decodable, Sendable {
  let name: String
  let input: Input
  let expected: Expected

  struct Input: Decodable, Sendable {
    let expression: String
    let attributes: [String: String]
  }

  struct Expected: Decodable, Sendable {
    let matches: Bool?
    let unknownAttribute: String?
  }
}

struct AttrIconContrastVectorFile: Decodable, Sendable {
  let version: Int
  let cases: [AttrIconContrastVectorCase]
}

struct AttrIconContrastVectorCase: Decodable, Sendable {
  let name: String
  let input: Input
  let expected: Expected

  struct Input: Decodable, Sendable {
    let foreground: Color
    let background: Color
  }

  struct Color: Decodable, Sendable {
    let red: Double
    let green: Double
    let blue: Double

    var attrIconColor: AttrIconColor {
      AttrIconColor(red: red, green: green, blue: blue)
    }
  }

  struct Expected: Decodable, Sendable {
    let ratio: Double
    let tolerance: Double
    let needsHalo: Bool
  }
}

private func attrIconWidthCases() throws -> [AttrIconWidthVectorCase] {
  let fixture = try Fixtures.decode(
    AttrIconWidthVectorFile.self,
    from: "vectors/attr-icon-width-levels.json"
  )
  guard fixture.version == 1 else {
    throw AttrIconVectorError.unsupportedVersion(fixture.version)
  }
  return fixture.cases
}

private func attrIconConditionCases() throws -> [AttrIconConditionVectorCase] {
  let fixture = try Fixtures.decode(
    AttrIconConditionVectorFile.self,
    from: "vectors/attr-icon-conditions.json"
  )
  guard fixture.version == 1 else {
    throw AttrIconVectorError.unsupportedVersion(fixture.version)
  }
  return fixture.cases
}

private func attrIconContrastCases() throws -> [AttrIconContrastVectorCase] {
  let fixture = try Fixtures.decode(
    AttrIconContrastVectorFile.self,
    from: "vectors/attr-icon-contrast.json"
  )
  guard fixture.version == 1 else {
    throw AttrIconVectorError.unsupportedVersion(fixture.version)
  }
  return fixture.cases
}

@Suite("속성 아이콘")
struct AttrIconTests {
  @Test(
    "두께 단계는 범위를 고정하고 사분위 경계를 보존한다",
    arguments: try attrIconWidthCases()
  )
  func widthLevelsFollowVector(_ testCase: AttrIconWidthVectorCase) {
    let actual = AttrIconPure.widthLevel(
      value: testCase.input.value,
      lowerBound: testCase.input.lowerBound,
      upperBound: testCase.input.upperBound
    )

    #expect(actual == testCase.expected, "case=\(testCase.name)")
  }

  @Test(
    "속성 조건은 불리언·값을 비교하고 알 수 없는 속성을 거부한다",
    arguments: try attrIconConditionCases()
  )
  func conditionsFollowVector(_ testCase: AttrIconConditionVectorCase) throws {
    if let expectedMatch = testCase.expected.matches {
      #expect(testCase.expected.unknownAttribute == nil, "case=\(testCase.name)")
      let actual = try AttrIconPure.conditionMatches(
        testCase.input.expression,
        attributes: testCase.input.attributes
      )
      #expect(actual == expectedMatch, "case=\(testCase.name)")
      return
    }

    let attribute = try #require(testCase.expected.unknownAttribute)
    #expect(throws: AttrIconPure.ConditionError.unknownAttribute(attribute)) {
      try AttrIconPure.conditionMatches(
        testCase.input.expression,
        attributes: testCase.input.attributes
      )
    }
  }

  @Test(
    "명암비는 전경·배경에 대칭이고 임계값 아래에서 halo가 필요하다",
    arguments: try attrIconContrastCases()
  )
  func contrastFollowsVector(_ testCase: AttrIconContrastVectorCase) {
    let foreground = testCase.input.foreground.attrIconColor
    let background = testCase.input.background.attrIconColor
    let ratio = AttrIconPure.contrastRatio(foreground, background)

    #expect(
      abs(ratio - testCase.expected.ratio) < testCase.expected.tolerance,
      "case=\(testCase.name)"
    )
    #expect(
      AttrIconPure.needsHalo(foreground: foreground, background: background)
        == testCase.expected.needsHalo,
      "case=\(testCase.name)"
    )
  }
}
