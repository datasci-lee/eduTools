import CoreGraphics
import Foundation
import Testing

@testable import EduTools
@testable import TestSupport

@Suite("판서 기하", .serialized)
struct InkGeometryTests {
  @Test("필압 최소 굵기 비율은 펜 굵기의 하한을 보정한다")
  func pressureMinimumWidthRatioAdjustsEffectiveWidth() {
    let pressureStroke = makeTestStroke(width: 10, usesPressure: true)
    let fixedStroke = makeTestStroke(width: 10, usesPressure: false)

    #expect(abs(pressureStroke.effectiveWidth(pressure: 0, minimumRatio: 0.2) - 2) < 0.000_001)
    #expect(abs(pressureStroke.effectiveWidth(pressure: 1, minimumRatio: 0.2) - 10) < 0.000_001)
    #expect(abs(pressureStroke.effectiveWidth(pressure: 0.5, minimumRatio: 0.2) - 6) < 0.000_001)
    #expect(abs(pressureStroke.effectiveWidth(pressure: 0.5, minimumRatio: 1) - 10) < 0.000_001)
    #expect(abs(fixedStroke.effectiveWidth(pressure: 0, minimumRatio: 0.2) - 10) < 0.000_001)
  }

  @Test("Catmull-Rom 평활 곡선은 모든 원본 점을 통과한다")
  func catmullRomPassesThroughSourcePoints() {
    let sourcePoints = makeTestStroke(
      locations: [
        CGPoint(x: 0.1, y: 0.2),
        CGPoint(x: 0.35, y: 0.8),
        CGPoint(x: 0.7, y: 0.3),
        CGPoint(x: 0.9, y: 0.6),
      ]
    ).points
    let smoothedPoints = InkGeometry.smoothedPoints(sourcePoints)

    #expect(
      sourcePoints.allSatisfy { source in
        smoothedPoints.contains {
          pointsAreApproximatelyEqual($0.location, source.location)
        }
      }
    )
  }

  @Test("펜 Shift 스냅은 0도·45도·90도를 선택한다")
  func penShiftSnapsToThreeDirections() {
    let start = CGPoint(x: 100, y: 100)
    let proposals = [
      CGPoint(x: 220, y: 112),
      CGPoint(x: 205, y: 198),
      CGPoint(x: 112, y: 220),
    ]
    let expectedAngles = [CGFloat(0), CGFloat.pi / 4, CGFloat.pi / 2]

    let snapsToExpectedAngles = zip(proposals, expectedAngles).allSatisfy {
      proposal, expectedAngle in
      let endpoint = InkGeometry.snappedPenEndpoint(from: start, to: proposal)
      let actualAngle = atan2(endpoint.y - start.y, endpoint.x - start.x)
      return abs(actualAngle - expectedAngle) < 0.000_001
    }

    #expect(snapsToExpectedAngles)
  }

  @Test("형광펜 Shift 스냅은 시작점과 같은 높이의 수평선이다")
  func highlighterShiftStaysHorizontal() {
    let endpoint = InkGeometry.snappedHighlighterEndpoint(
      from: CGPoint(x: 90, y: 140),
      to: CGPoint(x: 260, y: 310)
    )

    #expect(endpoint == CGPoint(x: 260, y: 140))
  }

  @Test("시드 고정 평활 입력에서도 모든 보간 구간이 원본 점을 보존한다")
  func seededSmoothingPreservesSourcePoints() {
    let seed: UInt64 = 0xED06_2001
    var random = SeededRandom(seed: seed)

    for sample in 0..<24 {
      let pointCount = Int.random(in: 2...12, using: &random)
      let locations = (0..<pointCount).map { _ in
        CGPoint(
          x: Double.random(in: 0...1, using: &random),
          y: Double.random(in: 0...1, using: &random)
        )
      }
      let sourcePoints = makeTestStroke(locations: locations).points
      let subdivisions = Int.random(in: 1...12, using: &random)
      let smoothedPoints = InkGeometry.smoothedPoints(
        sourcePoints,
        subdivisions: subdivisions
      )
      let preservedEverySourcePoint = sourcePoints.allSatisfy { source in
        smoothedPoints.contains {
          pointsAreApproximatelyEqual($0.location, source.location)
        }
      }

      #expect(
        preservedEverySourcePoint,
        "seed=\(seed), sample=\(sample), subdivisions=\(subdivisions)"
      )
    }
  }
}
