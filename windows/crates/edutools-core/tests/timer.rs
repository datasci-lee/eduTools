use edutools_core::timer::{BreakTimerEngine, LessonTimerEngine, Point, Rect, Size, TimerGeometry};

const NOW: f64 = 1_000_000.0;

// Swift: TimerTests.countdownUsesInjectedClock()
#[test]
fn countdown_uses_injected_clock() {
    let mut engine = LessonTimerEngine::default();
    engine.start_countdown(600.0, NOW);

    assert_eq!(engine.remaining(NOW + 125.0), 475.0);
}

// Swift: TimerTests.pauseFreezesAndResumeContinues()
#[test]
fn pause_freezes_and_resume_continues() {
    let mut engine = LessonTimerEngine::default();
    engine.start_countdown(600.0, NOW);
    engine.pause(NOW + 100.0);

    assert_eq!(engine.remaining(NOW + 400.0), 500.0);

    engine.resume(NOW + 400.0);

    assert_eq!(engine.remaining(NOW + 450.0), 450.0);
}

// Swift: TimerTests.tenMinuteBreakUsesInjectedClock()
#[test]
fn ten_minute_break_uses_injected_clock() {
    assert_eq!(
        BreakTimerEngine::end_date_adding_minutes(10, NOW),
        NOW + 600.0
    );
}

// Swift: TimerTests.pastDirectBreakTimeIsRejected()
#[test]
fn past_direct_break_time_is_rejected() {
    let start_of_day = 0.0;
    let after_direct_time = 10.0 * 60.0 * 60.0 + 51.0 * 60.0;

    assert_eq!(
        BreakTimerEngine::end_date_for_today(10, 50, after_direct_time, start_of_day),
        None
    );
}

// Swift: TimerTests.badgeSnapsInsideSixteenPixelsOnly()
#[test]
fn badge_snaps_inside_sixteen_pixels_only() {
    let frame = Rect::new(0.0, 0.0, 100.0, 100.0);
    let size = Size::new(10.0, 10.0);
    let inside = TimerGeometry::snapped_origin(Point::new(16.0, 40.0), size, frame, false);
    let outside = TimerGeometry::snapped_origin(Point::new(17.0, 40.0), size, frame, false);

    assert_eq!(inside.x, 0.0);
    assert_eq!(outside.x, 17.0);
}
