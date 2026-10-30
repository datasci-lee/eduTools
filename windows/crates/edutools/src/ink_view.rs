use crate::log::Logger;
use crate::monitor::{Monitor, MonitorRect};
use edutools_core::ink::{
    InkGeometry, InkPoint, InkStroke, InkStrokeContent, InkStrokeTool, InkTool, Point, Size,
};
use edutools_core::settings::AppSettings;
use edutools_core::state::AppState;
use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use windows::core::Result;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::Pointer::{
    GetPointerInfoHistory, GetPointerPenInfoHistory, GetPointerType, POINTER_FLAG_CANCELED,
    POINTER_INFO, POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    PEN_MASK_PRESSURE, POINTER_INPUT_TYPE, PT_MOUSE, PT_PEN, PT_POINTER, PT_TOUCH, PT_TOUCHPAD,
    WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
};

static STROKE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerDevice {
    Pointer,
    Touch,
    Pen,
    Mouse,
    Touchpad,
    Unknown(i32),
}

impl PointerDevice {
    fn from_win32(value: POINTER_INPUT_TYPE) -> Self {
        match value {
            PT_POINTER => Self::Pointer,
            PT_TOUCH => Self::Touch,
            PT_PEN => Self::Pen,
            PT_MOUSE => Self::Mouse,
            PT_TOUCHPAD => Self::Touchpad,
            other => Self::Unknown(other.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InkInputConfig {
    pub current_tool: InkTool,
    pub current_color_index: Option<i64>,
    pub current_color: Option<[f64; 4]>,
    pub current_width: Option<f64>,
    pub highlighter_opacity: f64,
    pub pressure_minimum_width_ratio: f64,
}

impl InkInputConfig {
    pub fn from_core(state: &AppState, settings: &AppSettings) -> Self {
        let memory = state
            .current_tool()
            .memory_slot()
            .map(|slot| slot.setting(&settings.tools.per_tool_memory));
        let current_color_index = memory.map(|memory| memory.color_index);
        let current_color = current_color_index
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| settings.tools.colors.get(index))
            .map(|color| [color.red, color.green, color.blue, 1.0]);
        Self {
            current_tool: state.current_tool(),
            current_color_index,
            current_color,
            current_width: memory.map(|memory| memory.size),
            highlighter_opacity: settings.tools.highlighter_opacity,
            pressure_minimum_width_ratio: settings.tools.pressure_minimum_width_ratio,
        }
    }
}

#[derive(Default)]
pub struct InputOutcome {
    pub finished: Vec<InkStroke>,
    pub redraw: bool,
}

struct ActiveStroke {
    pointer_id: u32,
    device: PointerDevice,
    stroke: InkStroke,
}

pub struct InkInputController {
    monitor_key: String,
    annotation_enabled: bool,
    config: InkInputConfig,
    active: Option<ActiveStroke>,
    logger: Logger,
}

impl InkInputController {
    pub fn new(
        monitor_key: String,
        annotation_enabled: bool,
        config: InkInputConfig,
        logger: Logger,
    ) -> Self {
        Self {
            monitor_key,
            annotation_enabled,
            config,
            active: None,
            logger,
        }
    }

    pub fn update_context(&mut self, annotation_enabled: bool, config: InkInputConfig) {
        self.annotation_enabled = annotation_enabled;
        self.config = config;
    }

    pub fn update_monitor_key(&mut self, monitor_key: String) {
        self.monitor_key = monitor_key;
    }

    pub fn pressure_minimum_width_ratio(&self) -> f64 {
        self.config.pressure_minimum_width_ratio
    }

    pub fn highlighter_opacity(&self) -> f64 {
        self.config.highlighter_opacity
    }

    pub fn active_stroke(&self) -> Option<&InkStroke> {
        self.active.as_ref().map(|active| &active.stroke)
    }

    pub fn finish_active(&mut self, reason: &str) -> InputOutcome {
        let mut outcome = InputOutcome::default();
        if let Some(stroke) = self.take_active(reason) {
            outcome.finished.push(stroke);
            outcome.redraw = true;
        }
        outcome
    }

    pub fn handle_message(
        &mut self,
        message: u32,
        pointer_id: u32,
        monitor: &Monitor,
    ) -> Result<InputOutcome> {
        if !self.annotation_enabled {
            return Ok(InputOutcome::default());
        }
        match message {
            WM_POINTERDOWN => self.handle_down(pointer_id, monitor),
            WM_POINTERUPDATE | WM_POINTERUP => {
                self.handle_update_or_up(message, pointer_id, monitor)
            }
            WM_POINTERCAPTURECHANGED => Ok(self.finish_active("WM_POINTERCAPTURECHANGED")),
            WM_POINTERLEAVE => Ok(InputOutcome::default()),
            _ => Ok(InputOutcome::default()),
        }
    }

    fn handle_down(&mut self, pointer_id: u32, monitor: &Monitor) -> Result<InputOutcome> {
        let mut outcome = InputOutcome::default();
        if let Some(active) = self.active.as_ref() {
            if active.pointer_id != pointer_id {
                self.logger.event(&format!(
                    "무시한 입력: 다른 포인터 down, owner={} incoming={pointer_id}",
                    active.pointer_id
                ));
                return Ok(outcome);
            }
            self.logger.event(&format!(
                "무시한 입력 복구: 누락 up, pointerId={pointer_id}; 기존 획 확정 후 새 획 시작"
            ));
            if let Some(stroke) = self.take_active("누락 up 복구") {
                outcome.finished.push(stroke);
                outcome.redraw = true;
            }
        }

        let (stroke_tool, uses_pressure) = match self.config.current_tool {
            InkTool::Pen => (InkStrokeTool::Pen, true),
            InkTool::Highlighter => (InkStrokeTool::Highlighter, false),
            other => {
                self.logger
                    .event(&format!("무시한 입력: 미구현 도구: {other:?}"));
                return Ok(outcome);
            }
        };
        let Some(color) = self.config.current_color else {
            self.logger.event(&format!(
                "무시한 입력: 도구 색 인덱스 범위 밖 ({:?})",
                self.config.current_color_index
            ));
            return Ok(outcome);
        };
        let Some(width) = self.config.current_width else {
            self.logger.event("무시한 입력: 도구 굵기 설정 없음");
            return Ok(outcome);
        };

        let device = pointer_device(pointer_id)?;
        let history = pointer_history(pointer_id, device, monitor.rect)?;
        if history.points.is_empty() {
            self.logger.event(&format!(
                "무시한 입력: 포인터 이력 점 없음, pointerId={pointer_id}"
            ));
            return Ok(outcome);
        }
        let uses_pressure = uses_pressure && device == PointerDevice::Pen;
        let identifier = format!(
            "win-{}-{pointer_id}-{}",
            history.points[0].timestamp,
            STROKE_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed)
        );
        let stroke = InkStroke::new(
            identifier,
            stroke_tool,
            color,
            width,
            uses_pressure,
            history.points,
            InkStrokeContent::Freehand,
        );
        self.logger.event(&format!(
            "획 시작: monitor={} pointerId={pointer_id} device={device:?} tool={:?} colorIndex={} width={}",
            self.monitor_key,
            self.config.current_tool,
            self.config.current_color_index.unwrap_or(-1),
            width
        ));
        self.active = Some(ActiveStroke {
            pointer_id,
            device,
            stroke,
        });
        outcome.redraw = true;
        if history.canceled {
            if let Some(stroke) = self.take_active("POINTER_FLAG_CANCELED") {
                outcome.finished.push(stroke);
            }
        }
        Ok(outcome)
    }

    fn handle_update_or_up(
        &mut self,
        message: u32,
        pointer_id: u32,
        monitor: &Monitor,
    ) -> Result<InputOutcome> {
        let mut outcome = InputOutcome::default();
        let Some(active) = self.active.as_ref() else {
            return Ok(outcome);
        };
        if active.pointer_id != pointer_id {
            self.logger.event(&format!(
                "무시한 입력: 다른 포인터, owner={} incoming={pointer_id}",
                active.pointer_id
            ));
            return Ok(outcome);
        }

        let device = pointer_device(pointer_id)?;
        let history = pointer_history(pointer_id, device, monitor.rect)?;
        if let Some(active) = self.active.as_mut() {
            debug_assert_eq!(active.device, device);
            merge_points(&mut active.stroke.points, history.points);
        }
        outcome.redraw = true;
        if message == WM_POINTERUP || history.canceled {
            let reason = if history.canceled {
                "POINTER_FLAG_CANCELED"
            } else {
                "WM_POINTERUP"
            };
            if let Some(stroke) = self.take_active(reason) {
                outcome.finished.push(stroke);
            }
        }
        Ok(outcome)
    }

    fn take_active(&mut self, reason: &str) -> Option<InkStroke> {
        let active = self.active.take()?;
        let (minimum, maximum) = active
            .stroke
            .points
            .iter()
            .map(|point| point.pressure)
            .fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
            );
        self.logger.event(&format!(
            "획 확정: monitor={} pointerId={} reason={reason} points={} uses_pressure={} pressure={minimum:.3}..{maximum:.3}",
            self.monitor_key,
            active.pointer_id,
            active.stroke.points.len(),
            active.stroke.uses_pressure
        ));
        if active.stroke.tool == InkStrokeTool::Highlighter {
            self.logger.event(&format!(
                "형광펜 획 확정: monitor={} points={} width={} opacity={}",
                self.monitor_key,
                active.stroke.points.len(),
                active.stroke.width,
                self.config.highlighter_opacity
            ));
        }
        Some(active.stroke)
    }
}

struct PointerHistory {
    points: Vec<InkPoint>,
    canceled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HistorySample {
    point: POINT,
    pressure: f64,
    timestamp_ms: u32,
    performance_count: u64,
    canceled: bool,
}

fn pointer_device(pointer_id: u32) -> Result<PointerDevice> {
    let mut pointer_type = POINTER_INPUT_TYPE::default();
    // SAFETY: pointer_type is writable and pointer_id comes from this WM_POINTER event.
    unsafe { GetPointerType(pointer_id, &mut pointer_type) }?;
    Ok(PointerDevice::from_win32(pointer_type))
}

fn pointer_history(
    pointer_id: u32,
    device: PointerDevice,
    monitor: MonitorRect,
) -> Result<PointerHistory> {
    let mut samples = if device == PointerDevice::Pen {
        pen_history(pointer_id)?
    } else {
        info_history(pointer_id)?
    };
    samples.sort_by(history_order);
    let canceled = samples.last().is_some_and(|sample| sample.canceled);
    let points = samples
        .into_iter()
        .map(|sample| {
            InkPoint::new(
                normalized_monitor_point(sample.point, monitor),
                sample.pressure,
                f64::from(sample.timestamp_ms) / 1_000.0,
            )
        })
        .collect();
    Ok(PointerHistory { points, canceled })
}

fn pen_history(pointer_id: u32) -> Result<Vec<HistorySample>> {
    let mut count = 0;
    // SAFETY: a null output requests the required history count for this event pointer.
    unsafe { GetPointerPenInfoHistory(pointer_id, &mut count, None) }?;
    let mut history = vec![POINTER_PEN_INFO::default(); count as usize];
    // SAFETY: history has count writable entries and the API updates count to the number written.
    unsafe { GetPointerPenInfoHistory(pointer_id, &mut count, Some(history.as_mut_ptr())) }?;
    history.truncate(count as usize);
    Ok(history
        .into_iter()
        .map(|pen| HistorySample {
            point: pen.pointerInfo.ptPixelLocationRaw,
            pressure: if pen.penMask & PEN_MASK_PRESSURE != 0 {
                normalized_pressure(pen.pressure)
            } else {
                1.0
            },
            timestamp_ms: pen.pointerInfo.dwTime,
            performance_count: pen.pointerInfo.PerformanceCount,
            canceled: pen.pointerInfo.pointerFlags & POINTER_FLAG_CANCELED == POINTER_FLAG_CANCELED,
        })
        .collect())
}

fn info_history(pointer_id: u32) -> Result<Vec<HistorySample>> {
    let mut count = 0;
    // SAFETY: a null output requests the required history count for this event pointer.
    unsafe { GetPointerInfoHistory(pointer_id, &mut count, None) }?;
    let mut history = vec![POINTER_INFO::default(); count as usize];
    // SAFETY: history has count writable entries and the API updates count to the number written.
    unsafe { GetPointerInfoHistory(pointer_id, &mut count, Some(history.as_mut_ptr())) }?;
    history.truncate(count as usize);
    Ok(history
        .into_iter()
        .map(|info| HistorySample {
            point: info.ptPixelLocationRaw,
            pressure: 1.0,
            timestamp_ms: info.dwTime,
            performance_count: info.PerformanceCount,
            canceled: info.pointerFlags & POINTER_FLAG_CANCELED == POINTER_FLAG_CANCELED,
        })
        .collect())
}

fn history_order(first: &HistorySample, second: &HistorySample) -> Ordering {
    first
        .timestamp_ms
        .cmp(&second.timestamp_ms)
        .then(first.performance_count.cmp(&second.performance_count))
}

fn merge_points(existing: &mut Vec<InkPoint>, incoming: Vec<InkPoint>) {
    existing.extend(incoming);
    existing.sort_by(|first, second| {
        first
            .timestamp
            .partial_cmp(&second.timestamp)
            .unwrap_or(Ordering::Equal)
    });
    let mut deduplicated = Vec::with_capacity(existing.len());
    for point in existing.drain(..) {
        if !deduplicated.iter().any(|candidate: &InkPoint| {
            candidate.timestamp == point.timestamp
                && candidate.location == point.location
                && candidate.pressure == point.pressure
        }) {
            deduplicated.push(point);
        }
    }
    *existing = deduplicated;
}

fn normalized_pressure(pressure: u32) -> f64 {
    (f64::from(pressure) / 1_024.0).clamp(0.0, 1.0)
}

fn normalized_monitor_point(point: POINT, monitor: MonitorRect) -> Point {
    InkGeometry::normalized_point(
        Point {
            x: f64::from(point.x - monitor.left),
            y: f64::from(point.y - monitor.top),
        },
        Size {
            width: f64::from(monitor.width()),
            height: f64::from(monitor.height()),
        },
    )
}

pub fn pointer_id_from_wparam(wparam: usize) -> u32 {
    (wparam as u32) & 0xffff
}

#[cfg(test)]
mod tests {
    use super::{
        history_order, normalized_monitor_point, normalized_pressure, HistorySample, MonitorRect,
    };
    use edutools_core::ink::{InkGeometry, Size};
    use windows::Win32::Foundation::POINT;

    #[test]
    fn pointer_conversion_pressure_and_history_table() {
        let monitor = MonitorRect {
            left: -1_920,
            top: 120,
            right: 0,
            bottom: 1_200,
        };
        let cases = [
            (POINT { x: -1_920, y: 120 }, (0.0, 0.0)),
            (POINT { x: -960, y: 660 }, (0.5, 0.5)),
            (POINT { x: 0, y: 1_200 }, (1.0, 1.0)),
        ];
        for (screen, expected) in cases {
            let normalized = normalized_monitor_point(screen, monitor);
            assert_eq!((normalized.x, normalized.y), expected);
            let round_trip = InkGeometry::screen_point(
                normalized,
                Size {
                    width: f64::from(monitor.width()),
                    height: f64::from(monitor.height()),
                },
            );
            assert!((round_trip.x - f64::from(screen.x - monitor.left)).abs() <= 1.0);
            assert!((round_trip.y - f64::from(screen.y - monitor.top)).abs() <= 1.0);
        }
        assert_eq!(normalized_pressure(0), 0.0);
        assert_eq!(normalized_pressure(512), 0.5);
        assert_eq!(normalized_pressure(1_024), 1.0);

        let mut history = [
            sample(30, 300),
            sample(10, 200),
            sample(10, 100),
            sample(20, 400),
        ];
        history.sort_by(history_order);
        assert_eq!(
            history.map(|sample| (sample.timestamp_ms, sample.performance_count)),
            [(10, 100), (10, 200), (20, 400), (30, 300)]
        );
    }

    fn sample(timestamp_ms: u32, performance_count: u64) -> HistorySample {
        HistorySample {
            point: POINT::default(),
            pressure: 1.0,
            timestamp_ms,
            performance_count,
            canceled: false,
        }
    }
}
