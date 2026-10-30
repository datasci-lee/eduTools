//! Injected-clock lesson/break timers and badge snapping geometry.

use std::error::Error;
use std::fmt;

// Swift: Date represented as seconds on an injected clock
pub type Timestamp = f64;

// Swift: TimeInterval
pub type TimeInterval = f64;

// Swift: LessonTimerMode
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LessonTimerMode {
    Countdown { duration: TimeInterval },
    CountUp,
}

// Swift: LessonTimerPhase
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LessonTimerPhase {
    Stopped,
    Running,
    Paused,
    Finished,
}

// Swift: LessonTimerEngine
#[derive(Clone, Debug, PartialEq)]
pub struct LessonTimerEngine {
    mode: LessonTimerMode,
    phase: LessonTimerPhase,
    end_date: Option<Timestamp>,
    count_up_started_at: Option<Timestamp>,
    paused_remaining: Option<TimeInterval>,
    accumulated_elapsed: TimeInterval,
}

impl Default for LessonTimerEngine {
    // Swift: LessonTimerEngine.init() with AppSettings.default.timer.lessonDefaultMinutes
    fn default() -> Self {
        Self::new(Self::DEFAULT_DURATION)
    }
}

impl LessonTimerEngine {
    pub const DEFAULT_DURATION: TimeInterval = 50.0 * 60.0;

    // Swift: LessonTimerEngine.init()
    pub fn new(default_duration: TimeInterval) -> Self {
        assert!(
            default_duration > 0.0,
            "a countdown duration must be positive"
        );
        Self {
            mode: LessonTimerMode::Countdown {
                duration: default_duration,
            },
            phase: LessonTimerPhase::Stopped,
            end_date: None,
            count_up_started_at: None,
            paused_remaining: None,
            accumulated_elapsed: 0.0,
        }
    }

    // Swift: LessonTimerEngine.mode
    pub const fn mode(&self) -> LessonTimerMode {
        self.mode
    }

    // Swift: LessonTimerEngine.phase
    pub const fn phase(&self) -> LessonTimerPhase {
        self.phase
    }

    // Swift: LessonTimerEngine.endDate
    pub const fn end_date(&self) -> Option<Timestamp> {
        self.end_date
    }

    // Swift: LessonTimerEngine.isActive
    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            LessonTimerPhase::Running | LessonTimerPhase::Paused
        )
    }

    // Swift: LessonTimerEngine.startCountdown(duration:now:)
    pub fn start_countdown(&mut self, duration: TimeInterval, now: Timestamp) {
        assert!(duration > 0.0, "a countdown duration must be positive");
        self.mode = LessonTimerMode::Countdown { duration };
        self.phase = LessonTimerPhase::Running;
        self.end_date = Some(now + duration);
        self.count_up_started_at = None;
        self.paused_remaining = None;
        self.accumulated_elapsed = 0.0;
    }

    // Swift: LessonTimerEngine.startCountUp(now:)
    pub fn start_count_up(&mut self, now: Timestamp) {
        self.mode = LessonTimerMode::CountUp;
        self.phase = LessonTimerPhase::Running;
        self.end_date = None;
        self.count_up_started_at = Some(now);
        self.paused_remaining = None;
        self.accumulated_elapsed = 0.0;
    }

    // Swift: LessonTimerEngine.toggleDefault(now:defaultDuration:)
    pub fn toggle_default(&mut self, now: Timestamp, default_duration: TimeInterval) {
        match self.phase {
            LessonTimerPhase::Stopped | LessonTimerPhase::Finished => {
                self.start_countdown(default_duration, now);
            }
            LessonTimerPhase::Running => self.pause(now),
            LessonTimerPhase::Paused => self.resume(now),
        }
    }

    // Swift: LessonTimerEngine.pause(now:)
    pub fn pause(&mut self, now: Timestamp) {
        if self.phase != LessonTimerPhase::Running {
            return;
        }
        match self.mode {
            LessonTimerMode::Countdown { .. } => {
                self.paused_remaining = Some(self.remaining(now));
            }
            LessonTimerMode::CountUp => {
                self.accumulated_elapsed = self.elapsed(now);
                self.count_up_started_at = None;
            }
        }
        self.phase = LessonTimerPhase::Paused;
    }

    // Swift: LessonTimerEngine.resume(now:)
    pub fn resume(&mut self, now: Timestamp) {
        if self.phase != LessonTimerPhase::Paused {
            return;
        }
        match self.mode {
            LessonTimerMode::Countdown { .. } => {
                let remaining = self.paused_remaining.unwrap_or(0.0).max(0.0);
                self.end_date = Some(now + remaining);
                self.paused_remaining = None;
                self.phase = if remaining > 0.0 {
                    LessonTimerPhase::Running
                } else {
                    LessonTimerPhase::Finished
                };
            }
            LessonTimerMode::CountUp => {
                self.count_up_started_at = Some(now);
                self.phase = LessonTimerPhase::Running;
            }
        }
    }

    // Swift: LessonTimerEngine.reset(now:)
    pub fn reset(&mut self, now: Timestamp) {
        match self.mode {
            LessonTimerMode::Countdown { duration } => self.start_countdown(duration, now),
            LessonTimerMode::CountUp => self.start_count_up(now),
        }
    }

    // Swift: LessonTimerEngine.remaining(at:)
    pub fn remaining(&self, now: Timestamp) -> TimeInterval {
        let LessonTimerMode::Countdown { duration } = self.mode else {
            return 0.0;
        };
        match self.phase {
            LessonTimerPhase::Stopped => duration,
            LessonTimerPhase::Paused => self.paused_remaining.unwrap_or(0.0).max(0.0),
            LessonTimerPhase::Running => self
                .end_date
                .map(|end_date| (end_date - now).max(0.0))
                .unwrap_or(0.0),
            LessonTimerPhase::Finished => 0.0,
        }
    }

    // Swift: LessonTimerEngine.elapsed(at:)
    pub fn elapsed(&self, now: Timestamp) -> TimeInterval {
        match self.mode {
            LessonTimerMode::Countdown { duration } => {
                (duration - self.remaining(now)).clamp(0.0, duration)
            }
            LessonTimerMode::CountUp => {
                if self.phase == LessonTimerPhase::Running {
                    self.count_up_started_at
                        .map(|started_at| (self.accumulated_elapsed + now - started_at).max(0.0))
                        .unwrap_or(self.accumulated_elapsed.max(0.0))
                } else {
                    self.accumulated_elapsed.max(0.0)
                }
            }
        }
    }

    // Swift: LessonTimerEngine.synchronize(now:)
    pub fn synchronize(&mut self, now: Timestamp) -> bool {
        if self.phase != LessonTimerPhase::Running
            || !matches!(self.mode, LessonTimerMode::Countdown { .. })
            || !self.end_date.is_some_and(|end_date| end_date <= now)
        {
            return false;
        }
        self.phase = LessonTimerPhase::Finished;
        self.paused_remaining = None;
        true
    }
}

// Swift: BreakTimerStartError
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakTimerStartError {
    EndDateIsNotInFuture,
}

impl fmt::Display for BreakTimerStartError {
    // Swift: BreakTimerStartError localized error presentation
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("break timer end date is not in the future")
    }
}

impl Error for BreakTimerStartError {}

// Swift: BreakTimerPhase
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakTimerPhase {
    Inactive,
    Running,
    Finished,
}

// Swift: BreakTimerEngine
#[derive(Clone, Debug, PartialEq)]
pub struct BreakTimerEngine {
    phase: BreakTimerPhase,
    end_date: Option<Timestamp>,
    initial_duration: TimeInterval,
}

impl Default for BreakTimerEngine {
    // Swift: BreakTimerEngine.init()
    fn default() -> Self {
        Self::new()
    }
}

impl BreakTimerEngine {
    const SECONDS_PER_MINUTE: TimeInterval = 60.0;
    const HOURS_PER_DAY: u8 = 24;
    const MINUTES_PER_HOUR: u8 = 60;

    // Swift: BreakTimerEngine.init()
    pub const fn new() -> Self {
        Self {
            phase: BreakTimerPhase::Inactive,
            end_date: None,
            initial_duration: 0.0,
        }
    }

    // Swift: BreakTimerEngine.phase
    pub const fn phase(&self) -> BreakTimerPhase {
        self.phase
    }

    // Swift: BreakTimerEngine.endDate
    pub const fn end_date_value(&self) -> Option<Timestamp> {
        self.end_date
    }

    // Swift: BreakTimerEngine.initialDuration
    pub const fn initial_duration(&self) -> TimeInterval {
        self.initial_duration
    }

    // Swift: BreakTimerEngine.endDate(addingMinutes:to:)
    pub fn end_date_adding_minutes(minutes: i64, now: Timestamp) -> Timestamp {
        now + minutes as f64 * Self::SECONDS_PER_MINUTE
    }

    // Swift: BreakTimerEngine.endDate(hour:minute:now:calendar:)
    pub fn end_date_for_today(
        hour: u8,
        minute: u8,
        now: Timestamp,
        start_of_day: Timestamp,
    ) -> Option<Timestamp> {
        if hour >= Self::HOURS_PER_DAY || minute >= Self::MINUTES_PER_HOUR {
            return None;
        }
        let candidate = start_of_day
            + (f64::from(hour) * f64::from(Self::MINUTES_PER_HOUR) + f64::from(minute))
                * Self::SECONDS_PER_MINUTE;
        (candidate > now).then_some(candidate)
    }

    // Swift: BreakTimerEngine.start(until:now:)
    pub fn start(
        &mut self,
        end_date: Timestamp,
        now: Timestamp,
    ) -> Result<(), BreakTimerStartError> {
        if end_date <= now {
            return Err(BreakTimerStartError::EndDateIsNotInFuture);
        }
        self.end_date = Some(end_date);
        self.initial_duration = end_date - now;
        self.phase = BreakTimerPhase::Running;
        Ok(())
    }

    // Swift: BreakTimerEngine.remaining(at:)
    pub fn remaining(&self, now: Timestamp) -> TimeInterval {
        self.end_date
            .map(|end_date| (end_date - now).max(0.0))
            .unwrap_or(0.0)
    }

    // Swift: BreakTimerEngine.synchronize(now:)
    pub fn synchronize(&mut self, now: Timestamp) -> bool {
        if self.phase != BreakTimerPhase::Running
            || !self.end_date.is_some_and(|end_date| end_date <= now)
        {
            return false;
        }
        self.phase = BreakTimerPhase::Finished;
        true
    }

    // Swift: BreakTimerEngine.endEarly()
    pub fn end_early(&mut self) {
        self.phase = BreakTimerPhase::Inactive;
        self.end_date = None;
        self.initial_duration = 0.0;
    }
}

/// Swift `startFromSetup`가 빠른 선택과 시각 입력에 공통 적용하는 시작 판정.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BreakTimerStartInput {
    Quick {
        end_date: Timestamp,
    },
    TimeOfDay {
        hour: u8,
        minute: u8,
        start_of_day: Timestamp,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BreakTimerStartDecision {
    Start { end_date: Timestamp },
    Change { end_date: Timestamp },
    WarnPast,
}

pub fn break_timer_start_decision(
    input: BreakTimerStartInput,
    now: Timestamp,
    is_running: bool,
) -> BreakTimerStartDecision {
    let candidate = match input {
        BreakTimerStartInput::Quick { end_date } => (end_date > now).then_some(end_date),
        BreakTimerStartInput::TimeOfDay {
            hour,
            minute,
            start_of_day,
        } => BreakTimerEngine::end_date_for_today(hour, minute, now, start_of_day),
    };
    match (candidate, is_running) {
        (Some(end_date), false) => BreakTimerStartDecision::Start { end_date },
        (Some(end_date), true) => BreakTimerStartDecision::Change { end_date },
        (None, _) => BreakTimerStartDecision::WarnPast,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakProgressColor {
    Accent,
    NearEnd,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakProgressPresentation {
    pub fraction: f64,
    pub start_degrees: f64,
    pub sweep_degrees: f64,
    pub color: BreakProgressColor,
}

pub fn break_progress_presentation(
    remaining: TimeInterval,
    initial_duration: TimeInterval,
    phase: BreakTimerPhase,
) -> BreakProgressPresentation {
    let fraction = if initial_duration > 0.0 {
        (remaining / initial_duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let color = if phase == BreakTimerPhase::Finished || remaining <= 0.0 {
        BreakProgressColor::Finished
    } else if remaining <= 60.0 {
        BreakProgressColor::NearEnd
    } else {
        BreakProgressColor::Accent
    };
    BreakProgressPresentation {
        fraction,
        start_degrees: -90.0,
        sweep_degrees: 360.0 * fraction,
        color,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BreakTimerLayoutMetrics {
    pub compact_width: u32,
    pub compact_height: u32,
    pub corner_radius: u32,
    pub remaining_font: u32,
    pub resume_font: u32,
    pub ring_diameter: u32,
    pub ring_stroke: u32,
    pub vertical_gap: u32,
    pub ring_visible: bool,
    pub current_time_visible: bool,
}

pub fn break_timer_layout_metrics(collapsed: bool, dpi: u32) -> BreakTimerLayoutMetrics {
    let scale = f64::from(dpi.max(96)) / 96.0;
    let px = |dip: f64| (dip * scale).round() as u32;
    BreakTimerLayoutMetrics {
        compact_width: px(420.0),
        compact_height: px(230.0),
        corner_radius: px(if collapsed { 18.0 } else { 0.0 }),
        remaining_font: px(if collapsed { 64.0 } else { 128.0 }),
        resume_font: px(if collapsed { 20.0 } else { 32.0 }),
        ring_diameter: px(260.0),
        ring_stroke: px(14.0),
        vertical_gap: px(20.0),
        ring_visible: !collapsed,
        current_time_visible: !collapsed,
    }
}

pub fn break_resume_text(message: &str, time: Option<&str>) -> String {
    let message = message.trim();
    match (message.is_empty(), time) {
        (true, Some(time)) => time.to_owned(),
        (false, Some(time)) => format!("{message} {time}"),
        (_, None) => message.to_owned(),
    }
}

// Swift: CGPoint
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    // Swift: CGPoint.init(x:y:)
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// Swift: CGSize
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    // Swift: CGSize.init(width:height:)
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

// Swift: CGRect
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    // Swift: CGRect.init(x:y:width:height:)
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

// Swift: TimerGeometry
pub struct TimerGeometry;

impl TimerGeometry {
    pub const SNAP_THRESHOLD: f64 = 16.0;

    // Swift: TimerGeometry.snappedOrigin(proposed:windowSize:visibleFrame:snappingDisabled:)
    pub fn snapped_origin(
        proposed: Point,
        window_size: Size,
        visible_frame: Rect,
        snapping_disabled: bool,
    ) -> Point {
        let maximum_x = visible_frame
            .x
            .max(visible_frame.x + visible_frame.width - window_size.width);
        let maximum_y = visible_frame
            .y
            .max(visible_frame.y + visible_frame.height - window_size.height);
        let mut result = Point::new(
            proposed.x.clamp(visible_frame.x, maximum_x),
            proposed.y.clamp(visible_frame.y, maximum_y),
        );
        if snapping_disabled {
            return result;
        }
        if (result.x - visible_frame.x).abs() <= Self::SNAP_THRESHOLD {
            result.x = visible_frame.x;
        } else if (result.x - maximum_x).abs() <= Self::SNAP_THRESHOLD {
            result.x = maximum_x;
        }
        if (result.y - visible_frame.y).abs() <= Self::SNAP_THRESHOLD {
            result.y = visible_frame.y;
        } else if (result.y - maximum_y).abs() <= Self::SNAP_THRESHOLD {
            result.y = maximum_y;
        }
        result
    }
}

// Swift: TimerCoordinator
#[derive(Clone, Debug, PartialEq)]
pub struct TimerCoordinator {
    pub lesson: LessonTimerEngine,
    pub break_timer: BreakTimerEngine,
    auto_break_minutes: Option<i64>,
}

impl TimerCoordinator {
    // Swift: TimerCoordinator.init(defaults:settingsStore:)
    pub fn new(default_duration: TimeInterval, auto_break_minutes: Option<i64>) -> Self {
        Self {
            lesson: LessonTimerEngine::new(default_duration),
            break_timer: BreakTimerEngine::new(),
            auto_break_minutes,
        }
    }

    /// 118d가 소비할 앱 내 pending 요청을 만드는 순수 규칙.
    pub const fn pending_auto_break_request(
        auto_start_break: bool,
        break_default_minutes: i64,
    ) -> Option<i64> {
        if auto_start_break {
            Some(break_default_minutes)
        } else {
            None
        }
    }

    // Swift: TimerCoordinator wiring of LessonTimerController.onAutoBreakRequested
    pub fn synchronize(&mut self, now: Timestamp) -> Result<bool, BreakTimerStartError> {
        if !self.lesson.synchronize(now) {
            return Ok(false);
        }
        if let Some(minutes) = self.auto_break_minutes {
            let end_date = BreakTimerEngine::end_date_adding_minutes(minutes, now);
            self.break_timer.start(end_date, now)?;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod lesson_badge_tests {
    use super::{
        break_progress_presentation, break_resume_text, break_timer_layout_metrics,
        break_timer_start_decision, BreakProgressColor, BreakTimerPhase, BreakTimerStartDecision,
        BreakTimerStartInput, Point, Rect, Size, TimerGeometry,
    };

    #[test]
    fn break_start_rule_table_matches_setup_inputs() {
        let cases = [
            (
                "quick future starts",
                BreakTimerStartInput::Quick { end_date: 1_600.0 },
                false,
                BreakTimerStartDecision::Start { end_date: 1_600.0 },
            ),
            (
                "quick future changes running timer",
                BreakTimerStartInput::Quick { end_date: 1_600.0 },
                true,
                BreakTimerStartDecision::Change { end_date: 1_600.0 },
            ),
            (
                "quick past warns while inactive",
                BreakTimerStartInput::Quick { end_date: 900.0 },
                false,
                BreakTimerStartDecision::WarnPast,
            ),
            (
                "quick past warns while running",
                BreakTimerStartInput::Quick { end_date: 900.0 },
                true,
                BreakTimerStartDecision::WarnPast,
            ),
            (
                "time future starts",
                BreakTimerStartInput::TimeOfDay {
                    hour: 1,
                    minute: 0,
                    start_of_day: 0.0,
                },
                false,
                BreakTimerStartDecision::Start { end_date: 3_600.0 },
            ),
            (
                "time future changes running timer",
                BreakTimerStartInput::TimeOfDay {
                    hour: 1,
                    minute: 0,
                    start_of_day: 0.0,
                },
                true,
                BreakTimerStartDecision::Change { end_date: 3_600.0 },
            ),
            (
                "time past warns while inactive",
                BreakTimerStartInput::TimeOfDay {
                    hour: 0,
                    minute: 10,
                    start_of_day: 0.0,
                },
                false,
                BreakTimerStartDecision::WarnPast,
            ),
            (
                "time past warns while running",
                BreakTimerStartInput::TimeOfDay {
                    hour: 0,
                    minute: 10,
                    start_of_day: 0.0,
                },
                true,
                BreakTimerStartDecision::WarnPast,
            ),
        ];
        for (name, input, running, expected) in cases {
            assert_eq!(
                break_timer_start_decision(input, 1_000.0, running),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn break_progress_angle_and_color_table_covers_boundaries() {
        let cases = [
            (
                "half accent",
                300.0,
                600.0,
                BreakTimerPhase::Running,
                (0.5, 180.0, BreakProgressColor::Accent),
            ),
            (
                "sixty seconds is near end",
                60.0,
                600.0,
                BreakTimerPhase::Running,
                (0.1, 36.0, BreakProgressColor::NearEnd),
            ),
            (
                "finished is red",
                0.0,
                600.0,
                BreakTimerPhase::Finished,
                (0.0, 0.0, BreakProgressColor::Finished),
            ),
        ];
        for (name, remaining, initial, phase, expected) in cases {
            let actual = break_progress_presentation(remaining, initial, phase);
            assert_eq!(actual.start_degrees, -90.0, "{name}");
            assert!((actual.fraction - expected.0).abs() < 1e-9, "{name}");
            assert!((actual.sweep_degrees - expected.1).abs() < 1e-9, "{name}");
            assert_eq!(actual.color, expected.2, "{name}");
        }
    }

    #[test]
    fn break_layout_dimension_table_scales_literal_dips() {
        let cases = [
            (96, (420, 230, 18, 64, 20, 260, 14, 20)),
            (144, (630, 345, 27, 96, 30, 390, 21, 30)),
            (192, (840, 460, 36, 128, 40, 520, 28, 40)),
        ];
        for (dpi, expected) in cases {
            let compact = break_timer_layout_metrics(true, dpi);
            assert_eq!(
                (
                    compact.compact_width,
                    compact.compact_height,
                    compact.corner_radius,
                    compact.remaining_font,
                    compact.resume_font,
                    compact.ring_diameter,
                    compact.ring_stroke,
                    compact.vertical_gap,
                ),
                expected,
                "compact dpi={dpi}"
            );
            assert!(!compact.ring_visible && !compact.current_time_visible);
            let full = break_timer_layout_metrics(false, dpi);
            assert!(full.ring_visible && full.current_time_visible);
            assert_eq!(
                (
                    full.corner_radius,
                    full.remaining_font,
                    full.resume_font,
                    full.ring_diameter,
                    full.ring_stroke,
                    full.vertical_gap,
                ),
                match dpi {
                    96 => (0, 128, 32, 260, 14, 20),
                    144 => (0, 192, 48, 390, 21, 30),
                    192 => (0, 256, 64, 520, 28, 40),
                    _ => unreachable!(),
                },
                "full dpi={dpi}"
            );
        }
    }

    #[test]
    fn break_resume_text_table_handles_optional_message() {
        let cases = [
            ("수업 재개 시각", Some("10:30"), "수업 재개 시각 10:30"),
            ("  ", Some("10:30"), "10:30"),
            ("메시지", None, "메시지"),
        ];
        for (message, time, expected) in cases {
            assert_eq!(break_resume_text(message, time), expected);
        }
    }

    #[test]
    fn badge_snap_and_alt_decision_table_uses_physical_pixels() {
        let visible = Rect::new(100.0, 200.0, 1_000.0, 700.0);
        let size = Size::new(180.0, 80.0);
        let cases = [
            (
                "within 16 physical pixels snaps",
                Point::new(115.0, 805.0),
                false,
                Point::new(100.0, 820.0),
            ),
            (
                "outside 16 physical pixels does not snap",
                Point::new(117.0, 802.0),
                false,
                Point::new(117.0, 802.0),
            ),
            (
                "Alt disables snap but keeps work-area clamp",
                Point::new(115.0, 805.0),
                true,
                Point::new(115.0, 805.0),
            ),
            (
                "Alt still clamps outside work area",
                Point::new(50.0, 900.0),
                true,
                Point::new(100.0, 820.0),
            ),
        ];
        for (name, proposed, alt_pressed, expected) in cases {
            assert_eq!(
                TimerGeometry::snapped_origin(proposed, size, visible, alt_pressed),
                expected,
                "{name}"
            );
        }
    }
}
