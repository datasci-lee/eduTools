import AppKit
import Testing

@testable import EduTools

enum FaceToneInk: Sendable {
  case none
  case rgb(Double, Double, Double)

  var color: NSColor? {
    switch self {
    case .none:
      return nil
    case let .rgb(red, green, blue):
      return NSColor(
        deviceRed: CGFloat(red),
        green: CGFloat(green),
        blue: CGFloat(blue),
        alpha: 1
      )
    }
  }
}

struct FaceToneExpectation: Sendable {
  let name: String
  let ink: FaceToneInk
  let expected: RadialPaletteFaceTone
}

@Suite("원형 팔레트 면 색조")
struct RadialPaletteFaceToneTests {
  @Test(
    "밝은 면과 잉크의 기존 halo 명암비 기준으로 전체 면 색조를 고른다",
    arguments: [
      FaceToneExpectation(name: "색 없음", ink: .none, expected: .light),
      // Against the 0.93 light-surface estimate: white 1.17:1, below 1.8.
      FaceToneExpectation(name: "흰색", ink: .rgb(1, 1, 1), expected: .dark),
      // Black 17.96:1 and blue 6.16:1 are above 1.8.
      FaceToneExpectation(name: "검정", ink: .rgb(0, 0, 0), expected: .light),
      FaceToneExpectation(name: "파랑", ink: .rgb(0, 0.2, 1), expected: .light),
      // Light yellow (1, 1, 0.4) is about 1.10:1, below 1.8.
      FaceToneExpectation(name: "연한 노랑", ink: .rgb(1, 1, 0.4), expected: .dark),
    ]
  )
  func resolvesFaceTone(_ expectation: FaceToneExpectation) {
    let actual = RadialPaletteFaceTone.resolve(inkColor: expectation.ink.color)

    #expect(actual == expectation.expected, Comment(rawValue: expectation.name))
  }
}
