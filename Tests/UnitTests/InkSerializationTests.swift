import AppKit
import CoreGraphics
import Foundation
import Testing

@testable import EduTools
@testable import TestSupport

@Suite("판서 직렬화", .serialized)
struct InkSerializationTests {
  @Test("화면 판서와 실행 취소 이력은 JSON 왕복 뒤에도 유지된다")
  func displayInkAndHistorySurviveJSONRoundTrip() throws {
    let displayUUID = "archive-display"
    let firstStroke = makeTestStroke(
      tool: .pen,
      color: NSColor(deviceRed: 0.2, green: 0.4, blue: 0.8, alpha: 1),
      usesPressure: true,
      points: [
        InkPoint(location: CGPoint(x: 0.25, y: 0.75), pressure: 0.6, timestamp: 1),
        InkPoint(location: CGPoint(x: 0.5, y: 0.5), pressure: 0.9, timestamp: 2),
      ]
    )
    let secondStroke = makeTestStroke(
      tool: .highlighter,
      color: NSColor(deviceRed: 0.9, green: 0.7, blue: 0.1, alpha: 1),
      width: 18,
      points: [InkPoint(location: CGPoint(x: 0.8, y: 0.2), pressure: 1, timestamp: 3)]
    )
    let layer = InkLayerState()
    layer.execute(.addDisplayStroke(displayUUID: displayUUID, stroke: firstStroke))
    layer.execute(.addDisplayStroke(displayUUID: displayUUID, stroke: secondStroke))
    _ = layer.undo()

    let data = try JSONEncoder().encode(layer.state(for: displayUUID))
    let decoded = try JSONDecoder().decode(InkLayerState.self, from: data)

    #expect(decoded.strokes(for: displayUUID) == [firstStroke])
    #expect(decoded.redo())
    #expect(decoded.strokes(for: displayUUID) == [firstStroke, secondStroke])
    #expect(decoded.undo())
    #expect(decoded.strokes(for: displayUUID) == [firstStroke])
  }

  @Test("다른 해상도로 복원해도 정규화된 획 좌표가 유지된다")
  func differentResolutionPreservesNormalizedCoordinates() throws {
    let displayUUID = "resolution-display"
    let stroke = makeTestStroke(locations: [CGPoint(x: 0.25, y: 0.75)])
    let layer = InkLayerState()
    layer.execute(.addDisplayStroke(displayUUID: displayUUID, stroke: stroke))
    let data = try JSONEncoder().encode(layer.state(for: displayUUID))
    let decoded = try JSONDecoder().decode(InkLayerState.self, from: data)
    let normalizedPoint = try #require(
      decoded.strokes(for: displayUUID).first?.points.first?.location
    )
    let originalResolution = CGSize(width: 1_000, height: 500)
    let restoredResolution = CGSize(width: 2_000, height: 1_000)
    let originalPoint = InkGeometry.screenPoint(from: normalizedPoint, in: originalResolution)
    let restoredPoint = InkGeometry.screenPoint(from: normalizedPoint, in: restoredResolution)

    #expect(originalPoint == CGPoint(x: 250, y: 375))
    #expect(restoredPoint == CGPoint(x: 500, y: 750))
    #expect(
      originalPoint.x / originalResolution.width == restoredPoint.x / restoredResolution.width)
    #expect(
      originalPoint.y / originalResolution.height == restoredPoint.y / restoredResolution.height)
  }

  @Test("시드 고정 판서 상태는 JSON 왕복 전후 불변이다")
  func seededInkStateRoundTripIsInvariant() throws {
    let seed: UInt64 = 0xED06_2003
    var random = SeededRandom(seed: seed)

    for sample in 0..<16 {
      let displayUUID = "seeded-display-\(sample)"
      let layer = InkLayerState()
      let strokeCount = Int.random(in: 1...12, using: &random)
      for strokeIndex in 0..<strokeCount {
        let pointCount = Int.random(in: 1...10, using: &random)
        let locations = (0..<pointCount).map { _ in
          CGPoint(
            x: Double.random(in: 0...1, using: &random),
            y: Double.random(in: 0...1, using: &random)
          )
        }
        let color = NSColor(
          deviceRed: Double.random(in: 0...1, using: &random),
          green: Double.random(in: 0...1, using: &random),
          blue: Double.random(in: 0...1, using: &random),
          alpha: Double.random(in: 0.2...1, using: &random)
        )
        let identifier = try #require(
          UUID(uuidString: String(format: "00000000-0000-0000-%04X-%012X", sample, strokeIndex))
        )
        layer.execute(
          .addDisplayStroke(
            displayUUID: displayUUID,
            stroke: makeTestStroke(
              identifier: identifier,
              color: color,
              width: CGFloat.random(in: 1...30, using: &random),
              locations: locations,
              pressure: CGFloat.random(in: 0...1, using: &random)
            )
          )
        )
      }
      let undoCount = Int.random(in: 0...strokeCount, using: &random)
      for _ in 0..<undoCount {
        _ = layer.undo()
      }
      let before = layer.strokes(for: displayUUID)
      let beforeCanRedo = layer.canRedo

      let data = try JSONEncoder().encode(layer.state(for: displayUUID))
      let decoded = try JSONDecoder().decode(InkLayerState.self, from: data)

      #expect(
        decoded.strokes(for: displayUUID) == before,
        "seed=\(seed), sample=\(sample), current strokes"
      )
      #expect(
        decoded.canRedo == beforeCanRedo,
        "seed=\(seed), sample=\(sample), redo availability"
      )
    }
  }
}
