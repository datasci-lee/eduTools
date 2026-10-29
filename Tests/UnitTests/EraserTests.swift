import AppKit
import CoreGraphics
import Testing

@testable import EduTools
@testable import TestSupport

@Suite("지우개", .serialized)
struct EraserTests {
  @Test("획 중심의 부분 지우기는 양쪽 끝 조각 두 개를 남긴다")
  func partialEraseThroughCenterLeavesTwoFragments() throws {
    let canvasSize = CGSize(width: 1_000, height: 100)
    let stroke = makeTestStroke(
      width: 2,
      locations: [CGPoint(x: 0.1, y: 0.5), CGPoint(x: 0.9, y: 0.5)]
    )

    let result = EraserEngine.erase(
      strokes: [stroke],
      gesture: .sweep(points: [CGPoint(x: 500, y: 50)]),
      settings: partialEraser(diameter: 40),
      canvasSize: canvasSize
    )

    #expect(result.count == 2)
    let first = try #require(result.first)
    let last = try #require(result.last)
    #expect(first.points.allSatisfy { $0.location.x < 0.5 })
    #expect(last.points.allSatisfy { $0.location.x > 0.5 })
  }

  @Test("형광펜 한정 획 지우기는 닿은 펜 획을 보존한다")
  func highlighterOnlyErasePreservesPen() {
    let pen = makeTestStroke(
      tool: .pen,
      color: .systemBlue,
      locations: [CGPoint(x: 0.5, y: 0.5)]
    )
    let highlighter = makeTestStroke(
      tool: .highlighter,
      color: .systemYellow,
      width: 18,
      locations: [CGPoint(x: 0.5, y: 0.5)]
    )

    let result = EraserEngine.erase(
      strokes: [pen, highlighter],
      gesture: .sweep(points: [CGPoint(x: 500, y: 50)]),
      settings: EraserSettings(
        diameter: 24,
        mode: .stroke,
        hitRule: .touching,
        targetFilter: .highlighterOnly
      ),
      canvasSize: CGSize(width: 1_000, height: 100)
    )

    #expect(result.map(\.identifier) == [pen.identifier])
  }

  @Test("영역 지우기는 닿은 획과 완전히 포함된 획을 구분한다")
  func areaEraseDistinguishesTouchingFromContained() {
    let inside = makeTestStroke(
      width: 2,
      locations: [CGPoint(x: 0.3, y: 0.5), CGPoint(x: 0.4, y: 0.5)]
    )
    let crossing = makeTestStroke(
      color: .systemBlue,
      width: 2,
      locations: [CGPoint(x: 0.1, y: 0.5), CGPoint(x: 0.5, y: 0.5)]
    )
    let gesture = EraserGesture.area(CGRect(x: 20, y: 20, width: 30, height: 60))
    let canvasSize = CGSize(width: 100, height: 100)

    let touchingResult = EraserEngine.erase(
      strokes: [inside, crossing],
      gesture: gesture,
      settings: areaEraser(hitRule: .touching),
      canvasSize: canvasSize
    )
    let containedResult = EraserEngine.erase(
      strokes: [inside, crossing],
      gesture: gesture,
      settings: areaEraser(hitRule: .contained),
      canvasSize: canvasSize
    )

    #expect(touchingResult.isEmpty)
    #expect(containedResult.map(\.identifier) == [crossing.identifier])
  }

  @Test("시드 고정 부분 지우기는 전체 점 수를 늘리지 않는다")
  func seededPartialErasePointCountIsMonotonic() {
    let seed: UInt64 = 0xED06_2002
    var random = SeededRandom(seed: seed)
    let canvasSize = CGSize(width: 100, height: 100)

    for sample in 0..<24 {
      let pointCount = Int.random(in: 12...72, using: &random)
      let startX = Int.random(in: 5...(95 - pointCount), using: &random)
      let y = Int.random(in: 20...80, using: &random)
      let locations = (0..<pointCount).map { offset in
        CGPoint(x: CGFloat(startX + offset) / 100, y: CGFloat(y) / 100)
      }
      let stroke = makeTestStroke(width: 2, locations: locations)
      let eraserX = Int.random(in: startX..<(startX + pointCount), using: &random)

      let result = EraserEngine.erase(
        strokes: [stroke],
        gesture: .sweep(points: [CGPoint(x: eraserX, y: y)]),
        settings: partialEraser(diameter: 10),
        canvasSize: canvasSize
      )
      let remainingPointCount = result.reduce(0) { $0 + $1.points.count }

      #expect(
        remainingPointCount <= stroke.points.count,
        "seed=\(seed), sample=\(sample), before=\(stroke.points.count), after=\(remainingPointCount)"
      )
    }
  }

  private func partialEraser(diameter: CGFloat) -> EraserSettings {
    EraserSettings(
      diameter: diameter,
      mode: .partial,
      hitRule: .touching,
      targetFilter: .all
    )
  }

  private func areaEraser(hitRule: EraserHitRule) -> EraserSettings {
    EraserSettings(
      diameter: 24,
      mode: .area,
      hitRule: hitRule,
      targetFilter: .all
    )
  }
}
