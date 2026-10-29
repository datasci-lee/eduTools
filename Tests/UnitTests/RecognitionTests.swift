import CoreGraphics
import Testing

@testable import EduTools

@Suite("도형 인식", .serialized)
struct RecognitionTests {
  @Test("거의 곧은 손그림 획은 직선으로 인식된다")
  func nearlyStraightStrokeIsRecognized() {
    let recognized = ShapeRecognizer.recognize(
      points: [
        CGPoint(x: 0, y: 0),
        CGPoint(x: 25, y: 1),
        CGPoint(x: 50, y: -1),
        CGPoint(x: 75, y: 1),
        CGPoint(x: 100, y: 0),
      ]
    )

    #expect(recognized == .line)
  }

  @Test("닫힌 네 변 손그림 획은 사각형으로 인식된다")
  func closedFourSidedStrokeIsRectangle() {
    let recognized = ShapeRecognizer.recognize(
      points: [
        CGPoint(x: 0, y: 0),
        CGPoint(x: 50, y: 1),
        CGPoint(x: 100, y: 0),
        CGPoint(x: 99, y: 40),
        CGPoint(x: 100, y: 80),
        CGPoint(x: 50, y: 79),
        CGPoint(x: 0, y: 80),
        CGPoint(x: 1, y: 40),
        CGPoint(x: 0, y: 0),
      ]
    )

    #expect(recognized == .rectangle)
  }

  @Test("닫힌 둥근 손그림 획은 타원으로 인식된다")
  func closedRoundStrokeIsEllipse() {
    let points = (0...24).map { index -> CGPoint in
      let angle = CGFloat(index) / 24 * 2 * .pi
      return CGPoint(x: 50 + cos(angle) * 40, y: 50 + sin(angle) * 40)
    }

    #expect(ShapeRecognizer.recognize(points: points) == .ellipse)
  }

  @Test("열린 지그재그는 도형으로 인식되지 않는다")
  func openZigzagIsRejected() {
    let recognized = ShapeRecognizer.recognize(
      points: [
        CGPoint(x: 0, y: 0),
        CGPoint(x: 80, y: 80),
        CGPoint(x: 10, y: 70),
        CGPoint(x: 90, y: 10),
        CGPoint(x: 40, y: 60),
      ]
    )

    #expect(recognized == nil)
  }
}
