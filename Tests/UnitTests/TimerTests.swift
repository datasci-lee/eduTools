import CoreGraphics
import Foundation
import Testing

@testable import EduTools

@Suite("수업·휴식 타이머")
struct TimerTests {
  private let now = Date(timeIntervalSinceReferenceDate: 1_000_000)

  @Test("수업 타이머 남은 시간은 종료 시각에서 주입된 현재 시각을 뺀 값이다")
  func countdownUsesInjectedClock() {
    let engine = LessonTimerEngine()
    engine.startCountdown(duration: 600, now: now)

    #expect(engine.remaining(at: now.addingTimeInterval(125)) == 475)
  }

  @Test("일시정지한 수업 타이머는 고정되고 재개한 시점부터 이어진다")
  func pauseFreezesAndResumeContinues() {
    let engine = LessonTimerEngine()
    engine.startCountdown(duration: 600, now: now)
    engine.pause(now: now.addingTimeInterval(100))

    #expect(engine.remaining(at: now.addingTimeInterval(400)) == 500)

    engine.resume(now: now.addingTimeInterval(400))

    #expect(engine.remaining(at: now.addingTimeInterval(450)) == 450)
  }

  @Test("휴식 타이머 +10분은 주입된 현재 시각에서 종료 시각을 계산한다")
  func tenMinuteBreakUsesInjectedClock() {
    #expect(
      BreakTimerEngine.endDate(addingMinutes: 10, to: now)
        == now.addingTimeInterval(600)
    )
  }

  @Test("오늘의 직접 입력 시각이 이미 지났으면 거부한다")
  func pastDirectBreakTimeIsRejected() throws {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = try #require(TimeZone(secondsFromGMT: 0))
    let afterDirectTime = try #require(
      calendar.date(
        from: DateComponents(year: 2026, month: 10, day: 28, hour: 10, minute: 51)
      )
    )

    #expect(
      BreakTimerEngine.endDate(
        hour: 10,
        minute: 50,
        now: afterDirectTime,
        calendar: calendar
      ) == nil
    )
  }

  @Test("수업 배지는 가장자리 16px 안쪽만 스냅한다")
  func badgeSnapsInsideSixteenPixelsOnly() {
    let frame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let size = CGSize(width: 10, height: 10)
    let inside = TimerGeometry.snappedOrigin(
      proposed: CGPoint(x: 16, y: 40),
      windowSize: size,
      visibleFrame: frame,
      snappingDisabled: false
    )
    let outside = TimerGeometry.snappedOrigin(
      proposed: CGPoint(x: 17, y: 40),
      windowSize: size,
      visibleFrame: frame,
      snappingDisabled: false
    )

    #expect(inside.x == 0)
    #expect(outside.x == 17)
  }
}
