import AppKit
import CoreGraphics
import ImageIO
import Testing

@testable import EduTools

@Suite("판서 PNG 내보내기")
struct ExportIntegrationTests {
  @Test("작은 판서는 크기·알파·지정 좌표 색을 보존한 PNG 비트맵이 된다")
  func smallInkRendersToExpectedPNGBitmap() throws {
    let defaults = try IsolatedDefaults()
    let state = IntegrationFixtures.state(defaults: defaults.value)
    let dispatcher = AppCommandDispatcher(state: state)
    let displayUUID = "export-display"
    let stroke = InkStroke(
      tool: .shape,
      color: NSColor(deviceRed: 1, green: 0, blue: 0, alpha: 1),
      width: 2,
      usesPressure: false,
      points: [
        InkPoint(location: CGPoint(x: 0.25, y: 0.25), pressure: 1, timestamp: 0),
        InkPoint(location: CGPoint(x: 0.75, y: 0.75), pressure: 1, timestamp: 1),
      ],
      content: .shape(.rectangle)
    )
    state.addStroke(stroke, to: displayUUID)
    let view = InkView(
      frame: CGRect(x: 0, y: 0, width: 32, height: 32),
      state: state,
      dispatcher: dispatcher,
      displayUUID: displayUUID,
      requestKeyFocus: {}
    )

    let rendered = try #require(view.renderedImage(pixelSize: CGSize(width: 32, height: 32)))
    let pngData = try #require(
      NSBitmapImageRep(cgImage: rendered).representation(using: .png, properties: [:])
    )
    let source = try #require(CGImageSourceCreateWithData(pngData as CFData, nil))
    let decoded = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))

    #expect(decoded.width == 32)
    #expect(decoded.height == 32)
    #expect(decoded.alphaInfo != .none)
    #expect(decoded.alphaInfo != .noneSkipFirst)
    #expect(decoded.alphaInfo != .noneSkipLast)

    let pixels = try RGBA8Bitmap(image: decoded)
    let border = pixels[x: 8, y: 16]
    let center = pixels[x: 16, y: 16]
    let corner = pixels[x: 0, y: 0]
    #expect(border.red >= 200)
    #expect(border.green <= 40)
    #expect(border.blue <= 40)
    #expect(border.alpha >= 180)
    #expect(center.alpha == 0)
    #expect(corner.alpha == 0)
  }
}

private struct RGBA8Bitmap {
  struct Pixel {
    let red: UInt8
    let green: UInt8
    let blue: UInt8
    let alpha: UInt8
  }

  let width: Int
  let height: Int
  private let bytes: [UInt8]

  init(image: CGImage) throws {
    let pixelWidth = image.width
    let pixelHeight = image.height
    var storage = [UInt8](repeating: 0, count: pixelWidth * pixelHeight * 4)
    let rendered = storage.withUnsafeMutableBytes { buffer -> Bool in
      guard let baseAddress = buffer.baseAddress,
            let context = CGContext(
              data: baseAddress,
              width: pixelWidth,
              height: pixelHeight,
              bitsPerComponent: 8,
              bytesPerRow: pixelWidth * 4,
              space: CGColorSpaceCreateDeviceRGB(),
              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )
      else {
        return false
      }
      context.draw(
        image,
        in: CGRect(x: 0, y: 0, width: pixelWidth, height: pixelHeight)
      )
      return true
    }
    guard rendered else { throw CocoaError(.coderInvalidValue) }
    width = pixelWidth
    height = pixelHeight
    bytes = storage
  }

  subscript(x x: Int, y y: Int) -> Pixel {
    let offset = (y * width + x) * 4
    return Pixel(
      red: bytes[offset],
      green: bytes[offset + 1],
      blue: bytes[offset + 2],
      alpha: bytes[offset + 3]
    )
  }
}
