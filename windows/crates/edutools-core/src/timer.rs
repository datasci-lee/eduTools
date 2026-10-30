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
