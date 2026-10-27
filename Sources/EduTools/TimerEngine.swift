import CoreGraphics
import Foundation

enum LessonTimerMode: Equatable {
    case countdown(duration: TimeInterval)
    case countUp
}

enum LessonTimerPhase: Equatable {
    case stopped
    case running
    case paused
    case finished
}

final class LessonTimerEngine {
    static let defaultDuration: TimeInterval = 50 * 60

    private(set) var mode = LessonTimerMode.countdown(duration: defaultDuration)
    private(set) var phase = LessonTimerPhase.stopped
    private(set) var endDate: Date?
    private var countUpStartedAt: Date?
    private var pausedRemaining: TimeInterval?
    private var accumulatedElapsed: TimeInterval = 0

    var isActive: Bool {
        phase == .running || phase == .paused
    }

    func startCountdown(duration: TimeInterval, now: Date) {
        precondition(duration > 0, "A countdown duration must be positive")
        mode = .countdown(duration: duration)
        phase = .running
        endDate = now.addingTimeInterval(duration)
        countUpStartedAt = nil
        pausedRemaining = nil
        accumulatedElapsed = 0
    }

    func startCountUp(now: Date) {
        mode = .countUp
        phase = .running
        endDate = nil
        countUpStartedAt = now
        pausedRemaining = nil
        accumulatedElapsed = 0
    }

    func toggleDefault(now: Date) {
        switch phase {
        case .stopped, .finished:
            startCountdown(duration: Self.defaultDuration, now: now)
        case .running:
            pause(now: now)
        case .paused:
            resume(now: now)
        }
    }

    func pause(now: Date) {
        guard phase == .running else { return }
        switch mode {
        case .countdown:
            pausedRemaining = remaining(at: now)
        case .countUp:
            accumulatedElapsed = elapsed(at: now)
            countUpStartedAt = nil
        }
        phase = .paused
    }

    func resume(now: Date) {
        guard phase == .paused else { return }
        switch mode {
        case .countdown:
            let remaining = max(0, pausedRemaining ?? 0)
            endDate = now.addingTimeInterval(remaining)
            pausedRemaining = nil
            phase = remaining > 0 ? .running : .finished
        case .countUp:
            countUpStartedAt = now
            phase = .running
        }
    }

    func reset(now: Date) {
        switch mode {
        case let .countdown(duration):
            startCountdown(duration: duration, now: now)
        case .countUp:
            startCountUp(now: now)
        }
    }

    func remaining(at now: Date) -> TimeInterval {
        guard case let .countdown(duration) = mode else { return 0 }
        switch phase {
        case .stopped:
            return duration
        case .paused:
            return max(0, pausedRemaining ?? 0)
        case .running:
            guard let endDate else { return 0 }
            return max(0, endDate.timeIntervalSince(now))
        case .finished:
            return 0
        }
    }

    func elapsed(at now: Date) -> TimeInterval {
        switch mode {
        case let .countdown(duration):
            return max(0, min(duration, duration - remaining(at: now)))
        case .countUp:
            guard phase == .running, let countUpStartedAt else {
                return max(0, accumulatedElapsed)
            }
            return max(0, accumulatedElapsed + now.timeIntervalSince(countUpStartedAt))
        }
    }

    @discardableResult
    func synchronize(now: Date) -> Bool {
        guard phase == .running,
              case .countdown = mode,
              let endDate,
              endDate <= now
        else {
            return false
        }
        phase = .finished
        pausedRemaining = nil
        return true
    }
}

enum BreakTimerStartError: Error {
    case endDateIsNotInFuture
}

enum BreakTimerPhase: Equatable {
    case inactive
    case running
    case finished
}

final class BreakTimerEngine {
    private(set) var phase = BreakTimerPhase.inactive
    private(set) var endDate: Date?
    private(set) var initialDuration: TimeInterval = 0

    static func endDate(addingMinutes minutes: Int, to now: Date) -> Date {
        now.addingTimeInterval(TimeInterval(minutes * 60))
    }

    static func endDate(
        hour: Int,
        minute: Int,
        now: Date,
        calendar: Calendar = .current
    ) -> Date? {
        guard let candidate = calendar.date(
            bySettingHour: hour,
            minute: minute,
            second: 0,
            of: now
        ), candidate > now else {
            return nil
        }
        return candidate
    }

    func start(until endDate: Date, now: Date) throws {
        guard endDate > now else { throw BreakTimerStartError.endDateIsNotInFuture }
        self.endDate = endDate
        initialDuration = endDate.timeIntervalSince(now)
        phase = .running
    }

    func remaining(at now: Date) -> TimeInterval {
        guard let endDate else { return 0 }
        return max(0, endDate.timeIntervalSince(now))
    }

    @discardableResult
    func synchronize(now: Date) -> Bool {
        guard phase == .running, let endDate, endDate <= now else { return false }
        phase = .finished
        return true
    }

    func endEarly() {
        phase = .inactive
        endDate = nil
        initialDuration = 0
    }
}

enum TimerGeometry {
    static let snapThreshold: CGFloat = 16

    static func snappedOrigin(
        proposed: CGPoint,
        windowSize: CGSize,
        visibleFrame: CGRect,
        snappingDisabled: Bool
    ) -> CGPoint {
        let maximumX = max(visibleFrame.minX, visibleFrame.maxX - windowSize.width)
        let maximumY = max(visibleFrame.minY, visibleFrame.maxY - windowSize.height)
        var result = CGPoint(
            x: min(max(proposed.x, visibleFrame.minX), maximumX),
            y: min(max(proposed.y, visibleFrame.minY), maximumY)
        )

        guard !snappingDisabled else { return result }
        if abs(result.x - visibleFrame.minX) <= snapThreshold {
            result.x = visibleFrame.minX
        } else if abs(result.x - maximumX) <= snapThreshold {
            result.x = maximumX
        }
        if abs(result.y - visibleFrame.minY) <= snapThreshold {
            result.y = visibleFrame.minY
        } else if abs(result.y - maximumY) <= snapThreshold {
            result.y = maximumY
        }
        return result
    }
}
