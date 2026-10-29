import CoreGraphics
import Testing

@testable import EduTools

@Suite("줌")
struct ZoomTests {
  @Test("배율은 0.25씩 변하고 1.25에서 8 사이로 제한된다")
  func magnificationStepsAndClamps() {
    #expect(ZoomGeometry.adjustedMagnification(2, by: 1) == 2.25)
    #expect(ZoomGeometry.adjustedMagnification(1.25, by: -1) == 1.25)
    #expect(ZoomGeometry.adjustedMagnification(8, by: 1) == 8)
  }

  @Test("가장자리 따라가기는 안쪽에서는 멈추고 경계를 넘으면 이동한다")
  func edgeFollowingMovesOnlyBeyondInnerEdge() {
    let screenSize = CGSize(width: 1_000, height: 800)
    let center = CGPoint(x: 500, y: 400)
    let centerPointerResult = ZoomGeometry.followedCenter(
      current: center,
      pointer: CGPoint(x: 500, y: 400),
      screenSize: screenSize,
      magnification: 2,
      mode: .edge,
      smoothness: 1
    )
    let edgePointerResult = ZoomGeometry.followedCenter(
      current: center,
      pointer: CGPoint(x: 740, y: 400),
      screenSize: screenSize,
      magnification: 2,
      mode: .edge,
      smoothness: 1
    )

    #expect(centerPointerResult == center)
    #expect(edgePointerResult == CGPoint(x: 540, y: 400))
  }

  @Test("줌 중심은 뷰포트를 화면 네 가장자리 안에 둔다")
  func centerClampKeepsViewportOnScreen() {
    #expect(
      ZoomGeometry.clampedCenter(
        CGPoint(x: 1, y: 799),
        screenSize: CGSize(width: 1_000, height: 800),
        magnification: 2
      ) == CGPoint(x: 250, y: 600)
    )
  }

  @Test("정지 줌 입력은 원래 화면의 정규화 좌표로 돌아간다")
  func frozenInputMapsBackToOriginalCoordinate() {
    let screenSize = CGSize(width: 1_000, height: 800)
    let originalPoint = ZoomGeometry.originalPoint(
      fromZoomed: CGPoint(x: 250, y: 600),
      in: screenSize,
      sourceRect: CGRect(x: 250, y: 200, width: 500, height: 400)
    )
    let normalizedPoint = InkGeometry.normalizedPoint(from: originalPoint, in: screenSize)

    #expect(normalizedPoint == CGPoint(x: 0.375, y: 0.625))
  }
}
