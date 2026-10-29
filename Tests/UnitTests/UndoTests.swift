import AppKit
import CoreGraphics
import Testing

@testable import EduTools
@testable import TestSupport

@Suite("판서 실행 취소", .serialized)
struct UndoTests {
  @Test("101개 획을 기록하면 최근 100단계를 실행 취소할 수 있다")
  func oneHundredOneStrokesPermitOneHundredUndos() {
    let layer = InkLayerState()
    for index in 0..<101 {
      layer.execute(
        .addStroke(
          makeTestStroke(
            locations: [CGPoint(x: CGFloat(index) / 101, y: 0.5)]
          )
        )
      )
    }

    let completedOneHundredUndos = (0..<100).allSatisfy { _ in layer.undo() }

    #expect(completedOneHundredUndos)
    #expect(layer.strokes.count == 1)
  }

  @Test("실행 취소 뒤 새 획을 추가하면 다시 실행 이력이 비워진다")
  func newStrokeClearsRedoHistory() {
    let layer = InkLayerState()
    layer.execute(.addStroke(makeTestStroke(color: .systemRed)))
    _ = layer.undo()
    layer.execute(.addStroke(makeTestStroke(color: .systemBlue)))

    #expect(!layer.canRedo)
    #expect(!layer.redo())
  }
}
