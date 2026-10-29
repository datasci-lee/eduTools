import CoreGraphics
import Foundation

enum ZoomFollowMode: String, CaseIterable {
    case always
    case edge
    case fixed

    var displayName: String {
        switch self {
        case .always: return "항상"
        case .edge: return "가장자리"
        case .fixed: return "고정"
        }
    }

    var next: ZoomFollowMode {
        switch self {
        case .always: return .edge
        case .edge: return .fixed
        case .fixed: return .always
        }
    }
}

enum ZoomDefaults {
    static let minimumMagnification = CGFloat(SettingsRanges.zoomMagnification.lowerBound)
    static var maximumMagnification: CGFloat { CGFloat(AppSettings.default.zoom.maximumMagnification) }
    static var magnificationStep: CGFloat { CGFloat(AppSettings.default.zoom.magnificationStep) }
    static var startingMagnification: CGFloat { CGFloat(AppSettings.default.zoom.startingMagnification) }
    static var defaultFollowMode: ZoomFollowMode {
        ZoomFollowMode(rawValue: AppSettings.default.zoom.followMode) ?? .edge
    }
    static let followEdgeInsetRatio = CGFloat(0.1)
    static var followSmoothness: CGFloat { CGFloat(AppSettings.default.zoom.followSmoothness) }
    static let previewAnimationDuration = TimeInterval(0.15)
}

struct ZoomSessionState: Equatable {
    let displayUUID: String
    let screenSize: CGSize
    var magnification: CGFloat
    let startingMagnification: CGFloat
    var center: CGPoint
    var followMode: ZoomFollowMode
    let followSmoothness: CGFloat
    let previewDimming: CGFloat
    var isFrozen: Bool
    var isPreviewing: Bool

    var sourceRect: CGRect {
        ZoomGeometry.sourceRect(
            center: center,
            screenSize: screenSize,
            magnification: magnification
        )
    }
}

enum ZoomGeometry {
    static func magnificationLabel(_ magnification: CGFloat) -> String {
        let quarterSteps = Int((magnification * 4).rounded())
        if quarterSteps.isMultiple(of: 2) {
            return String(format: "×%.1f", magnification)
        }
        return String(format: "×%.2f", magnification)
    }

    static func clampedMagnification(_ value: CGFloat) -> CGFloat {
        min(max(value, ZoomDefaults.minimumMagnification), ZoomDefaults.maximumMagnification)
    }

    static func steppedMagnification(_ value: CGFloat) -> CGFloat {
        let step = ZoomDefaults.magnificationStep
        let rounded = (value / step).rounded() * step
        return clampedMagnification(rounded)
    }

    static func adjustedMagnification(_ value: CGFloat, by steps: Int) -> CGFloat {
        steppedMagnification(value + CGFloat(steps) * ZoomDefaults.magnificationStep)
    }

    static func viewportSize(screenSize: CGSize, magnification: CGFloat) -> CGSize {
        let clamped = clampedMagnification(magnification)
        return CGSize(
            width: screenSize.width / clamped,
            height: screenSize.height / clamped
        )
    }

    static func clampedCenter(
        _ center: CGPoint,
        screenSize: CGSize,
        magnification: CGFloat
    ) -> CGPoint {
        let viewport = viewportSize(screenSize: screenSize, magnification: magnification)
        let halfWidth = viewport.width / 2
        let halfHeight = viewport.height / 2
        return CGPoint(
            x: min(max(center.x, halfWidth), screenSize.width - halfWidth),
            y: min(max(center.y, halfHeight), screenSize.height - halfHeight)
        )
    }

    static func sourceRect(
        center: CGPoint,
        screenSize: CGSize,
        magnification: CGFloat
    ) -> CGRect {
        let viewport = viewportSize(screenSize: screenSize, magnification: magnification)
        let clamped = clampedCenter(
            center,
            screenSize: screenSize,
            magnification: magnification
        )
        return CGRect(
            x: clamped.x - viewport.width / 2,
            y: clamped.y - viewport.height / 2,
            width: viewport.width,
            height: viewport.height
        )
    }

    static func followedCenter(
        current: CGPoint,
        pointer: CGPoint,
        screenSize: CGSize,
        magnification: CGFloat,
        mode: ZoomFollowMode,
        smoothness: CGFloat
    ) -> CGPoint {
        let current = clampedCenter(
            current,
            screenSize: screenSize,
            magnification: magnification
        )
        let target: CGPoint
        switch mode {
        case .always:
            target = pointer
        case .fixed:
            target = current
        case .edge:
            let viewport = sourceRect(
                center: current,
                screenSize: screenSize,
                magnification: magnification
            )
            let inner = viewport.insetBy(
                dx: viewport.width * ZoomDefaults.followEdgeInsetRatio,
                dy: viewport.height * ZoomDefaults.followEdgeInsetRatio
            )
            var pushed = current
            if pointer.x < inner.minX {
                pushed.x += pointer.x - inner.minX
            } else if pointer.x > inner.maxX {
                pushed.x += pointer.x - inner.maxX
            }
            if pointer.y < inner.minY {
                pushed.y += pointer.y - inner.minY
            } else if pointer.y > inner.maxY {
                pushed.y += pointer.y - inner.maxY
            }
            target = pushed
        }

        let smoothnessRange = SettingsRanges.zoomFollowSmoothness
        let amount = min(
            max(smoothness, CGFloat(smoothnessRange.lowerBound)),
            CGFloat(smoothnessRange.upperBound)
        )
        let interpolated = CGPoint(
            x: current.x + (target.x - current.x) * amount,
            y: current.y + (target.y - current.y) * amount
        )
        return clampedCenter(
            interpolated,
            screenSize: screenSize,
            magnification: magnification
        )
    }

    static func originalPoint(
        fromZoomed point: CGPoint,
        in zoomedSize: CGSize,
        sourceRect: CGRect
    ) -> CGPoint {
        guard zoomedSize.width > 0, zoomedSize.height > 0 else {
            return sourceRect.origin
        }
        return CGPoint(
            x: sourceRect.minX + point.x / zoomedSize.width * sourceRect.width,
            y: sourceRect.minY + point.y / zoomedSize.height * sourceRect.height
        )
    }

    static func presentationTransform(
        sourceRect: CGRect,
        destinationSize: CGSize
    ) -> CGAffineTransform {
        guard sourceRect.width > 0, sourceRect.height > 0 else { return .identity }
        let scaleX = destinationSize.width / sourceRect.width
        let scaleY = destinationSize.height / sourceRect.height
        return CGAffineTransform(
            a: scaleX,
            b: 0,
            c: 0,
            d: scaleY,
            tx: -sourceRect.minX * scaleX,
            ty: -sourceRect.minY * scaleY
        )
    }

    static func pointerPresentationPoint(
        _ sourcePoint: CGPoint,
        for state: ZoomSessionState
    ) -> CGPoint {
        guard !state.isFrozen, !state.isPreviewing else { return sourcePoint }
        return sourcePoint.applying(
            presentationTransform(
                sourceRect: state.sourceRect,
                destinationSize: state.screenSize
            )
        )
    }

    static func pointerScale(for state: ZoomSessionState) -> CGFloat {
        state.isPreviewing ? 1 : state.magnification
    }
}
