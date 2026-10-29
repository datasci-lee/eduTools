import CoreGraphics
import Foundation
import Testing

@testable import EduTools

struct RestoredOriginExpectation: Sendable {
  let name: String
  let ratio: CGPoint?
  let defaultRatio: CGPoint
  let visibleFrame: CGRect
  let size: CGSize
  let expected: CGPoint
}

@Suite("화면별 창 위치")
struct ScreenPositionTests {
  @Test(
    "저장 비율과 기본 비율을 화면 안 원점으로 복원한다",
    arguments: [
      RestoredOriginExpectation(
        name: "일반 비율",
        ratio: CGPoint(x: 0.5, y: 0.85),
        defaultRatio: CGPoint(x: 0.1, y: 0.2),
        visibleFrame: CGRect(x: 0, y: 0, width: 1_000, height: 800),
        size: CGSize(width: 100, height: 50),
        expected: CGPoint(x: 450, y: 637.5)
      ),
      RestoredOriginExpectation(
        name: "아래 경계",
        ratio: CGPoint(x: 0, y: 0),
        defaultRatio: CGPoint(x: 0.5, y: 0.5),
        visibleFrame: CGRect(x: 100, y: 200, width: 1_000, height: 800),
        size: CGSize(width: 100, height: 50),
        expected: CGPoint(x: 100, y: 200)
      ),
      RestoredOriginExpectation(
        name: "위 경계",
        ratio: CGPoint(x: 1, y: 1),
        defaultRatio: CGPoint(x: 0.5, y: 0.5),
        visibleFrame: CGRect(x: 100, y: 200, width: 1_000, height: 800),
        size: CGSize(width: 100, height: 50),
        expected: CGPoint(x: 1_000, y: 950)
      ),
      RestoredOriginExpectation(
        name: "범위 밖 저장값",
        ratio: CGPoint(x: -0.2, y: 1.7),
        defaultRatio: CGPoint(x: 0.5, y: 0.5),
        visibleFrame: CGRect(x: 0, y: 0, width: 1_000, height: 800),
        size: CGSize(width: 100, height: 50),
        expected: CGPoint(x: 0, y: 750)
      ),
      RestoredOriginExpectation(
        name: "창이 가용 영역보다 큼",
        ratio: CGPoint(x: 0.25, y: 0.75),
        defaultRatio: CGPoint(x: 0.5, y: 0.5),
        visibleFrame: CGRect(x: 100, y: 200, width: 1_000, height: 800),
        size: CGSize(width: 1_200, height: 900),
        expected: CGPoint(x: 100, y: 200)
      ),
      RestoredOriginExpectation(
        name: "저장 없음",
        ratio: nil,
        defaultRatio: CGPoint(x: 0.95, y: 0.88),
        visibleFrame: CGRect(x: 0, y: 0, width: 1_000, height: 800),
        size: CGSize(width: 100, height: 50),
        expected: CGPoint(x: 855, y: 660)
      ),
    ]
  )
  func restoresOrigin(_ expectation: RestoredOriginExpectation) {
    let actual = restoredOrigin(
      ratio: expectation.ratio,
      default: expectation.defaultRatio,
      visibleFrame: expectation.visibleFrame,
      size: expectation.size
    )

    #expect(
      pointsAreEqual(actual, expectation.expected),
      Comment(rawValue: expectation.name)
    )
  }

  @Test("복원한 원점을 다시 저장하면 같은 비율이다")
  func restoredOriginRoundTripsThroughSavedRatio() {
    let ratio = CGPoint(x: 0.37, y: 0.62)
    let visibleFrame = CGRect(x: -320, y: 40, width: 1_280, height: 720)
    let size = CGSize(width: 180, height: 90)
    let origin = restoredOrigin(
      ratio: ratio,
      default: CGPoint(x: 0.5, y: 0.85),
      visibleFrame: visibleFrame,
      size: size
    )

    let actual = savedRatio(origin: origin, visibleFrame: visibleFrame, size: size)

    #expect(pointsAreEqual(actual, ratio))
  }

  @Test("창이 가용 영역 이상이어도 1pt 분모 하한으로 유한한 비율을 저장한다")
  func savedRatioUsesOnePointDenominatorFloor() {
    let visibleFrame = CGRect(x: 100, y: 200, width: 1_000, height: 800)
    let origin = CGPoint(x: 100.5, y: 200.5)

    let equalSizeRatio = savedRatio(
      origin: origin,
      visibleFrame: visibleFrame,
      size: visibleFrame.size
    )
    let largerSizeRatio = savedRatio(
      origin: CGPoint(x: 102, y: 198),
      visibleFrame: visibleFrame,
      size: CGSize(width: 1_200, height: 900)
    )

    #expect(equalSizeRatio == CGPoint(x: 0.5, y: 0.5))
    #expect(largerSizeRatio == CGPoint(x: 1, y: 0))
    #expect(equalSizeRatio.x.isFinite)
    #expect(equalSizeRatio.y.isFinite)
  }

  @Test("화면 위치 저장소는 값과 UUID를 왕복하고 없거나 손상된 항목을 거부한다")
  func screenPositionStoreRoundTripsAndRejectsInvalidEntries() throws {
    let suiteName = "EduTools.ScreenPositionTests.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suiteName))
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let positionsKey = "positions"
    let displayUUIDKey = "displayUUID"
    let store = ScreenPositionStore(
      defaults: defaults,
      positionsKey: positionsKey,
      displayUUIDKey: displayUUIDKey
    )

    #expect(store.ratio(for: "missing") == nil)

    let expected = CGPoint(x: 0.25, y: 0.75)
    store.save(ratio: expected, displayUUID: "display-a")

    #expect(store.ratio(for: "display-a") == expected)
    #expect(defaults.string(forKey: displayUUIDKey) == "display-a")

    var positions = defaults.dictionary(forKey: positionsKey) ?? [:]
    positions["broken"] = ["x": "not-a-number", "y": 0.4]
    defaults.set(positions, forKey: positionsKey)

    #expect(store.ratio(for: "broken") == nil)
  }

  private func pointsAreEqual(
    _ first: CGPoint,
    _ second: CGPoint,
    tolerance: CGFloat = 0.000_001
  ) -> Bool {
    hypot(first.x - second.x, first.y - second.y) < tolerance
  }
}
