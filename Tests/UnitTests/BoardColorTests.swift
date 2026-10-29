import AppKit
import Testing

@testable import EduTools
@testable import TestSupport

@Suite("보드 표시 색", .serialized)
struct BoardColorTests {
  @Test("저장된 검은 획은 블랙보드에서 흰색으로 표시된다")
  func blackStrokeDisplaysWhiteOnBlackboard() throws {
    let stroke = makeTestStroke(color: .black)
    let displayedColor = try #require(
      BoardAppearance.displayColor(for: stroke.color, on: .black)
        .usingColorSpace(.deviceRGB)
    )

    #expect(BoardAppearance.isBlack(stroke.color))
    #expect(displayedColor.redComponent >= 0.999)
    #expect(displayedColor.greenComponent >= 0.999)
    #expect(displayedColor.blueComponent >= 0.999)
  }
}
