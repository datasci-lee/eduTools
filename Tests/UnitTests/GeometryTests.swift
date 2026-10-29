import CoreGraphics
import Foundation
import Testing

@testable import EduTools

@Suite("기하")
struct GeometryTests {
  @Test("정규화 좌표와 화면 좌표는 왕복된다")
  func normalizedAndScreenCoordinatesRoundTrip() {
    let screenSize = CGSize(width: 1_920, height: 1_080)
    let screenPoint = CGPoint(x: 384, y: 810)
    let normalizedPoint = InkGeometry.normalizedPoint(from: screenPoint, in: screenSize)
    let roundTrippedPoint = InkGeometry.screenPoint(from: normalizedPoint, in: screenSize)

    #expect(
      hypot(roundTrippedPoint.x - screenPoint.x, roundTrippedPoint.y - screenPoint.y) < 0.000_001)
  }
}
