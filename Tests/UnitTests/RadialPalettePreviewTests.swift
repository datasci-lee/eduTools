import AppKit
import Testing

@testable import EduTools

struct RadialPaletteRelativeSizeExpectation: Sendable {
  let name: String
  let values: [CGFloat]
  let inputs: [CGFloat]
  let expected: [CGFloat]
  let expectsStrictIncrease: Bool
}

@Suite("원형 팔레트 옵션 미리보기")
struct RadialPalettePreviewTests {
  @Test(
    "프리셋 값은 그룹 범위 안 상대 크기로 정규화되고 경계에서 클램프된다",
    arguments: [
      RadialPaletteRelativeSizeExpectation(
        name: "필기 굵기",
        values: RadialPaletteMetrics.writingWidths,
        inputs: RadialPaletteMetrics.writingWidths,
        expected: [0, 0.181_818_181_818, 0.363_636_363_636, 0.636_363_636_364, 1],
        expectsStrictIncrease: true
      ),
      RadialPaletteRelativeSizeExpectation(
        name: "형광펜 굵기",
        values: RadialPaletteMetrics.highlighterWidths,
        inputs: RadialPaletteMetrics.highlighterWidths,
        expected: [0, 0.25, 0.5, 0.75, 1],
        expectsStrictIncrease: true
      ),
      RadialPaletteRelativeSizeExpectation(
        name: "텍스트 크기",
        values: RadialPaletteMetrics.textSizes,
        inputs: RadialPaletteMetrics.textSizes,
        expected: [0, 0.142_857_142_857, 0.357_142_857_143, 0.571_428_571_429, 1],
        expectsStrictIncrease: true
      ),
      RadialPaletteRelativeSizeExpectation(
        name: "단일 값",
        values: [42],
        inputs: [42],
        expected: [0.5],
        expectsStrictIncrease: false
      ),
      RadialPaletteRelativeSizeExpectation(
        name: "범위 밖 값",
        values: [2, 4, 6],
        inputs: [-20, 20],
        expected: [0, 1],
        expectsStrictIncrease: false
      ),
    ]
  )
  func relativeSizeMatchesPresetRange(_ expectation: RadialPaletteRelativeSizeExpectation) {
    let results = expectation.inputs.map {
      RadialPalettePreview.relativeSize(of: $0, in: expectation.values)
    }

    #expect(results.count == expectation.expected.count, Comment(rawValue: expectation.name))
    for (result, expected) in zip(results, expectation.expected) {
      #expect(
        abs(result - expected) < 0.000_001,
        Comment(rawValue: "\(expectation.name): \(result) != \(expected)")
      )
    }
    if expectation.expectsStrictIncrease {
      #expect(results.first == 0, Comment(rawValue: "\(expectation.name) minimum"))
      #expect(results.last == 1, Comment(rawValue: "\(expectation.name) maximum"))
      #expect(
        zip(results, results.dropFirst()).allSatisfy(<),
        Comment(rawValue: "\(expectation.name) monotonic increase")
      )
    }
  }
}
