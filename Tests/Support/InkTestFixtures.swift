import AppKit
import CoreGraphics
import Foundation

@testable import EduTools

func makeTestStroke(
  identifier: UUID = UUID(),
  tool: InkStrokeTool = .pen,
  color: NSColor = .systemRed,
  width: CGFloat = 3,
  usesPressure: Bool = false,
  locations: [CGPoint] = [],
  pressure: CGFloat = 1,
  points: [InkPoint]? = nil,
  content: InkStrokeContent = .freehand
) -> InkStroke {
  InkStroke(
    identifier: identifier,
    tool: tool,
    color: color,
    width: width,
    usesPressure: usesPressure,
    points: points
      ?? locations.enumerated().map { index, location in
        InkPoint(location: location, pressure: pressure, timestamp: TimeInterval(index))
      },
    content: content
  )
}

func pointsAreApproximatelyEqual(
  _ first: CGPoint,
  _ second: CGPoint,
  tolerance: CGFloat = 0.000_001
) -> Bool {
  hypot(first.x - second.x, first.y - second.y) < tolerance
}
