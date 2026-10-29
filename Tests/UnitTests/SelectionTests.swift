import AppKit
import CoreGraphics
import Testing

@testable import EduTools
@testable import TestSupport

@Suite("선택·가리개", .serialized)
struct SelectionTests {
  @Test("오목한 올가미는 다각형 안쪽과 오목한 바깥쪽을 구분한다")
  func concaveLassoRespectsBoundary() {
    let lasso = [
      CGPoint(x: 0, y: 0),
      CGPoint(x: 100, y: 0),
      CGPoint(x: 100, y: 100),
      CGPoint(x: 50, y: 45),
      CGPoint(x: 0, y: 100),
    ]

    #expect(LassoGeometry.contains(CGPoint(x: 20, y: 30), polygon: lasso))
    #expect(!LassoGeometry.contains(CGPoint(x: 50, y: 70), polygon: lasso))
  }

  @Test("선택 이동은 모든 정규화 좌표에 같은 오프셋을 더한다")
  func movementOffsetsEveryNormalizedCoordinate() throws {
    let stroke = selectionStroke()
    let moved = SelectionTransform.moved([stroke], by: CGPoint(x: 0.1, y: -0.05))
    let points = try #require(moved.first?.points)

    #expect(points.count == 2)
    #expect(pointsAreApproximatelyEqual(points[0].location, CGPoint(x: 0.3, y: 0.15)))
    #expect(pointsAreApproximatelyEqual(points[1].location, CGPoint(x: 0.5, y: 0.35)))
  }

  @Test("겹친 가리개 클릭은 가장 위의 일치 가리개 하나만 제거한다")
  func overlappingCoverClickRemovesOnlyTopmost() {
    let selectedStroke = selectionStroke()
    let lowerCover = makeTestStroke(
      tool: .cover,
      color: .systemGray,
      width: 1,
      locations: [CGPoint(x: 0.1, y: 0.1), CGPoint(x: 0.6, y: 0.6)],
      content: .cover
    )
    let upperCover = makeTestStroke(
      tool: .cover,
      color: .systemGray,
      width: 1,
      locations: [CGPoint(x: 0.3, y: 0.3), CGPoint(x: 0.8, y: 0.8)],
      content: .cover
    )

    let remaining = ScreenCoverEngine.removingTopmostCover(
      at: CGPoint(x: 0.4, y: 0.4),
      from: [selectedStroke, lowerCover, upperCover]
    )

    #expect(remaining.map(\.identifier) == [selectedStroke.identifier, lowerCover.identifier])
  }

  @Test("선택 크기 변경은 원본 경계를 새 경계로 사상한다")
  func resizingMapsCoordinatesIntoNewBounds() throws {
    let stroke = selectionStroke()
    let resized = SelectionTransform.resized(
      [stroke],
      from: CGRect(x: 0.2, y: 0.2, width: 0.2, height: 0.2),
      to: CGRect(x: 0.1, y: 0.1, width: 0.6, height: 0.4)
    )
    let points = try #require(resized.first?.points)

    #expect(points.count == 2)
    #expect(pointsAreApproximatelyEqual(points[0].location, CGPoint(x: 0.1, y: 0.1)))
    #expect(pointsAreApproximatelyEqual(points[1].location, CGPoint(x: 0.7, y: 0.5)))
  }

  private func selectionStroke() -> InkStroke {
    makeTestStroke(
      tool: .shape,
      color: .systemBlue,
      locations: [CGPoint(x: 0.2, y: 0.2), CGPoint(x: 0.4, y: 0.4)],
      content: .shape(.rectangle)
    )
  }
}
