use crate::log::Logger;
use crate::monitor::{Monitor, MonitorRect};
use crate::render::{px_per_dip, COVER_STROKE_COLOR};
use crate::selection::{
    moved_strokes, normalized_point_bounds, resized_strokes, selected_stroke_ids, selection_bounds,
    selection_hit, SelectionHit,
};
use edutools_core::eraser::{
    EraserAreaShape, EraserEngine, EraserGesture, EraserHitRule, EraserMode, EraserSettings,
    EraserTargetFilter,
};
use edutools_core::ink::{
    ArrowStyle, InkGeometry, InkPoint, InkShapeKind, InkStroke, InkStrokeContent, InkStrokeTool,
    InkTool, Point, Rect, Size,
};
use edutools_core::laser::{LaserPoint, LaserTrail};
use edutools_core::settings::AppSettings;
use edutools_core::shape::{LassoHitRule, Rect as SelectionRect, ShapeRecognizer};
use edutools_core::state::AppState;
use edutools_core::zoom::{
    Point as ZoomPoint, Rect as ZoomRect, Size as ZoomSize, ZoomGeometry, ZoomSessionState,
};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::OnceLock;
use windows::core::Result;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, VK_LBUTTON, VK_RBUTTON, VK_SHIFT,
};
use windows::Win32::UI::Input::Pointer::{
    GetPointerInfo, GetPointerInfoHistory, GetPointerPenInfo, GetPointerPenInfoHistory,
    POINTER_CHANGE_FIRSTBUTTON_DOWN, POINTER_CHANGE_FIRSTBUTTON_UP,
    POINTER_CHANGE_SECONDBUTTON_DOWN, POINTER_CHANGE_SECONDBUTTON_UP, POINTER_FLAG_CANCELED,
    POINTER_FLAG_FIRSTBUTTON, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE,
    POINTER_FLAG_SECONDBUTTON, POINTER_INFO, POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    PEN_FLAG_BARREL, PEN_FLAG_ERASER, PEN_FLAG_INVERTED, PEN_MASK_PRESSURE, POINTER_INPUT_TYPE,
    PT_MOUSE, PT_PEN, PT_POINTER, PT_TOUCH, PT_TOUCHPAD, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN,
    WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
};

static STROKE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static QPC_FREQUENCY: OnceLock<i64> = OnceLock::new();

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
    pub arrow_style: ArrowStyle,
    pub shape_recognition_enabled: bool,
    pub current_color_index: Option<i64>,
    pub current_color: Option<[f64; 4]>,
    pub current_width: Option<f64>,
    pub highlighter_opacity: f64,
    pub pressure_minimum_width_ratio: f64,
    pub eraser_settings: Option<EraserSettings>,
    pub laser_color: [f64; 4],
    pub laser_size: f64,
    pub laser_trail_duration: f64,
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
            arrow_style: state.arrow_style(),
            shape_recognition_enabled: settings.tools.shape_recognition_enabled,
            current_color_index,
            current_color,
            current_width: memory.map(|memory| memory.size),
            highlighter_opacity: settings.tools.highlighter_opacity,
            pressure_minimum_width_ratio: settings.tools.pressure_minimum_width_ratio,
            eraser_settings: eraser_settings_from_core(settings),
            laser_color: [
                settings.tools.laser_color.red,
                settings.tools.laser_color.green,
                settings.tools.laser_color.blue,
                1.0,
            ],
            laser_size: settings.tools.laser_size,
            laser_trail_duration: settings.tools.laser_trail_duration,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StrokeReplacement {
    pub before: Vec<InkStroke>,
    pub after: Vec<InkStroke>,
    pub kind: StrokeReplacementKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrokeReplacementKind {
    Eraser { erased_strokes: usize },
    SelectionMove,
    SelectionResize,
}

#[derive(Default)]
pub struct InputOutcome {
    pub finished: Vec<InkStroke>,
    pub recognized: Option<InkStroke>,
    pub replacement: Option<StrokeReplacement>,
    pub selected_ids: Option<BTreeSet<String>>,
    pub text_entry: Option<TextEntrySeed>,
    pub redraw: bool,
}

#[derive(Clone, Debug)]
pub struct TextEntrySeed {
    pub origin: Point,
    pub display_origin: Point,
    pub color: [f64; 4],
    pub font_size: f64,
}

#[derive(Clone, Copy, Debug)]
pub enum InkInputMapping {
    Identity,
    ThroughPresentation {
        zoomed_size: ZoomSize,
        source_rect: ZoomRect,
    },
}

impl InkInputMapping {
    pub fn for_zoom(session: &ZoomSessionState) -> Self {
        if session.is_frozen {
            Self::ThroughPresentation {
                zoomed_size: session.screen_size,
                source_rect: session.source_rect(),
            }
        } else {
            Self::Identity
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Identity => "live/identity",
            Self::ThroughPresentation { .. } => "frozen/original_point",
        }
    }

    fn map_local(self, point: Point) -> Point {
        match self {
            Self::Identity => point,
            Self::ThroughPresentation {
                zoomed_size,
                source_rect,
            } => {
                let mapped = ZoomGeometry::original_point(
                    ZoomPoint::new(point.x, point.y),
                    zoomed_size,
                    source_rect,
                );
                Point {
                    x: mapped.x,
                    y: mapped.y,
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct EraserRenderState {
    pub settings: EraserSettings,
    pub points: Vec<Point>,
    pub cursor: Option<Point>,
    pub preview_strokes: Option<Vec<InkStroke>>,
    pub highlighted_strokes: Vec<InkStroke>,
}

#[derive(Clone, Debug)]
pub struct LaserRenderState {
    pub point: Option<Point>,
    pub trails: Vec<Vec<Point>>,
    pub color: [f64; 4],
    pub size_dip: f64,
}

#[derive(Clone, Debug, Default)]
pub struct SelectionRenderState {
    pub lasso: Vec<Point>,
    pub bounds: Option<SelectionRect>,
    pub preview_strokes: Option<Vec<InkStroke>>,
}

struct ActiveStroke {
    stroke: InkStroke,
    is_straight: bool,
    canvas_size: Size,
    recognized: Option<InkStroke>,
}

struct ActiveEraser {
    settings: EraserSettings,
    points: Vec<Point>,
    before: Vec<InkStroke>,
    canvas_size: Size,
    preview_strokes: Option<Vec<InkStroke>>,
    highlighted_strokes: Vec<InkStroke>,
}

enum ActiveSelection {
    Lasso {
        points: Vec<Point>,
        before: Vec<InkStroke>,
        canvas_size: Size,
    },
    Move {
        start: Point,
        before: Vec<InkStroke>,
        selected_ids: BTreeSet<String>,
        canvas_size: Size,
        preview: Option<Vec<InkStroke>>,
    },
    Resize {
        anchor: Point,
        source_bounds: SelectionRect,
        before: Vec<InkStroke>,
        selected_ids: BTreeSet<String>,
        canvas_size: Size,
        preview: Option<Vec<InkStroke>>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnerButton {
    MousePrimary,
    MouseSecondary,
    PointerContact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InputOwner {
    pointer_id: u32,
    device: PointerDevice,
    button: OwnerButton,
}

impl InputOwner {
    fn label(self) -> &'static str {
        match self.button {
            OwnerButton::MousePrimary => "mouse-primary",
            OwnerButton::MouseSecondary => "mouse-secondary",
            OwnerButton::PointerContact if self.device == PointerDevice::Pen => "pen",
            OwnerButton::PointerContact if self.device == PointerDevice::Touch => "touch",
            OwnerButton::PointerContact => "pointer",
        }
    }
}

/// 전역 레이저(마우스 훅 입력)용 가상 pointerId. 실제 WM_POINTER id와 겹치지 않는다.
const GLOBAL_LASER_POINTER_ID: u32 = u32::MAX;

#[derive(Default)]
struct LaserInputState {
    point: Option<Point>,
    trails: Vec<LaserTrail>,
    trailing: bool,
    active_pointer_id: Option<u32>,
    last_pointer_id: Option<u32>,
}

impl LaserInputState {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn has_visual(&self) -> bool {
        self.point.is_some() || self.trails.iter().any(|trail| !trail.trail.is_empty())
    }

    fn begin_or_append(&mut self, pointer_id: u32, point: Point, now: f64) -> bool {
        let is_new_pointer = is_new_laser_pointer(self.last_pointer_id, pointer_id);
        if !self.trailing || self.active_pointer_id != Some(pointer_id) {
            self.trails.push(LaserTrail::default());
        }
        self.trailing = true;
        self.active_pointer_id = Some(pointer_id);
        self.last_pointer_id = Some(pointer_id);
        self.point = Some(point);
        self.trails
            .last_mut()
            .expect("laser trail was just created")
            .push(LaserPoint {
                location: point,
                time: now,
            });
        is_new_pointer
    }

    fn show_point(&mut self, pointer_id: u32, point: Point) -> bool {
        let became_visible = self.point.is_none();
        self.point = Some(point);
        self.last_pointer_id = Some(pointer_id);
        became_visible
    }

    fn break_trail(&mut self, hide_point: bool) -> bool {
        let changed = self.trailing || (hide_point && self.point.is_some());
        self.trailing = false;
        self.active_pointer_id = None;
        if hide_point {
            self.point = None;
        }
        changed
    }

    fn retract(&mut self, now: f64, duration: f64) {
        for trail in &mut self.trails {
            trail.retract(now, duration);
        }
        let active_index = self.trailing.then(|| self.trails.len().saturating_sub(1));
        self.trails = self
            .trails
            .drain(..)
            .enumerate()
            .filter_map(|(index, trail)| {
                (!trail.trail.is_empty() || Some(index) == active_index).then_some(trail)
            })
            .collect();
    }

    fn timer_needed(&self) -> bool {
        self.trailing || self.trails.iter().any(|trail| !trail.trail.is_empty())
    }
}

pub struct InkInputController {
    monitor_key: String,
    annotation_enabled: bool,
    config: InkInputConfig,
    dpi: u32,
    owner: Option<InputOwner>,
    active_stroke: Option<ActiveStroke>,
    recognition_timer_waiting: bool,
    recognition_timer_reset_requested: bool,
    active_eraser: Option<ActiveEraser>,
    active_selection: Option<ActiveSelection>,
    eraser_cursor: Option<Point>,
    laser: LaserInputState,
    /// 팜 리젝션: 마지막으로 펜(호버 포함) 입력을 받은 시각.
    last_pen_seen: Option<f64>,
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
            dpi: 96,
            owner: None,
            active_stroke: None,
            recognition_timer_waiting: false,
            recognition_timer_reset_requested: false,
            active_eraser: None,
            active_selection: None,
            eraser_cursor: None,
            laser: LaserInputState::default(),
            last_pen_seen: None,
            logger,
        }
    }

    pub fn update_context(&mut self, annotation_enabled: bool, config: InkInputConfig) {
        let tool_changed = self.config.current_tool != config.current_tool;
        let recognition_disabled =
            self.config.shape_recognition_enabled && !config.shape_recognition_enabled;
        let clear_laser = !annotation_enabled
            || (self.config.current_tool == InkTool::Laser
                && config.current_tool != InkTool::Laser);
        self.annotation_enabled = annotation_enabled;
        self.config = config;
        if !annotation_enabled || tool_changed || recognition_disabled {
            let reason = if !annotation_enabled {
                "판서 끔"
            } else if tool_changed {
                "도구 변경"
            } else {
                "도형 인식 끔"
            };
            self.cancel_recognition(reason);
            if self.active_selection.take().is_some() {
                self.owner = None;
                self.logger.event(&format!(
                    "선택 상호작용 취소: monitor={} reason={reason}",
                    self.monitor_key
                ));
            }
        }
        if clear_laser && self.laser.has_visual() {
            self.logger
                .event("레이저 상태: 도구 전환/판서 끔으로 clear");
            self.laser.clear();
        }
        if !annotation_enabled
            || (self.config.current_tool != InkTool::Eraser && self.active_eraser.is_none())
        {
            self.eraser_cursor = None;
        }
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

    pub fn active_stroke(&self) -> Option<InkStroke> {
        self.active_stroke.as_ref().map(effective_stroke)
    }

    pub fn gesture_in_progress(&self) -> bool {
        self.owner.is_some()
            || self.active_stroke.is_some()
            || self.active_eraser.is_some()
            || self.active_selection.is_some()
    }

    pub fn recognition_timer_needed(&self) -> bool {
        self.recognition_timer_waiting
    }

    pub fn take_recognition_timer_reset(&mut self) -> bool {
        std::mem::take(&mut self.recognition_timer_reset_requested)
    }

    pub fn recognize_active(&mut self) {
        self.recognition_timer_waiting = false;
        self.recognition_timer_reset_requested = false;
        if !self.recognition_is_eligible() {
            return;
        }
        let Some(active) = self.active_stroke.as_mut() else {
            return;
        };
        active.recognized = ShapeRecognizer::corrected_stroke(&active.stroke, active.canvas_size);
        if let Some(candidate) = active.recognized.as_ref() {
            self.logger.event(&format!(
                "도형 인식 후보 생성: monitor={} kind={}",
                self.monitor_key,
                stroke_shape_label(candidate)
            ));
        }
    }

    pub fn eraser_render_state(&self) -> Option<EraserRenderState> {
        let active = self.active_eraser.as_ref();
        let settings = active
            .map(|active| active.settings)
            .or_else(|| self.scaled_eraser_settings())?;
        if active.is_none() && self.eraser_cursor.is_none() {
            return None;
        }
        Some(EraserRenderState {
            settings,
            points: active
                .map(|active| active.points.clone())
                .unwrap_or_default(),
            cursor: self.eraser_cursor,
            preview_strokes: active.and_then(|active| active.preview_strokes.clone()),
            highlighted_strokes: active
                .map(|active| active.highlighted_strokes.clone())
                .unwrap_or_default(),
        })
    }

    pub fn laser_render_state(&self) -> Option<LaserRenderState> {
        let trails = self
            .laser
            .trails
            .iter()
            .filter(|trail| !trail.trail.is_empty())
            .map(|trail| trail.trail.iter().map(|point| point.location).collect())
            .collect::<Vec<Vec<Point>>>();
        if self.laser.point.is_none() && trails.is_empty() {
            return None;
        }
        Some(LaserRenderState {
            point: self.laser.point,
            trails,
            color: self.config.laser_color,
            size_dip: self.config.laser_size,
        })
    }

    pub fn selection_render_state(
        &self,
        strokes: &[InkStroke],
        selected_ids: &BTreeSet<String>,
        canvas_size: Size,
    ) -> Option<SelectionRenderState> {
        let (lasso, preview_strokes) = match self.active_selection.as_ref() {
            Some(ActiveSelection::Lasso { points, .. }) => (points.clone(), None),
            Some(ActiveSelection::Move { preview, .. })
            | Some(ActiveSelection::Resize { preview, .. }) => (Vec::new(), preview.clone()),
            None => (Vec::new(), None),
        };
        let bounds_source = preview_strokes.as_deref().unwrap_or(strokes);
        let bounds = selection_bounds(bounds_source, selected_ids, canvas_size);
        if lasso.is_empty() && bounds.is_none() && preview_strokes.is_none() {
            return None;
        }
        Some(SelectionRenderState {
            lasso,
            bounds,
            preview_strokes,
        })
    }

    pub fn laser_timer_needed(&self) -> bool {
        self.laser.timer_needed()
    }

    /// 판서 모드 밖 전역 레이저: 오버레이는 클릭 통과 상태이므로 WM_POINTER 대신
    /// 전역 마우스 훅의 커서 위치(모니터 로컬 물리 픽셀)로 점과 꼬리를 그린다.
    /// 설계서대로 클릭 없이 이동만으로 꼬리가 생긴다.
    pub fn global_laser_move(&mut self, local_point: Point) -> Result<InputOutcome> {
        let now = monotonic_seconds()?;
        if self
            .laser
            .begin_or_append(GLOBAL_LASER_POINTER_ID, local_point, now)
        {
            self.logger
                .event("레이저 상태: 전역 레이저 커서로 전환, 이전 꼬리와 분리");
        }
        Ok(InputOutcome {
            redraw: true,
            ..InputOutcome::default()
        })
    }

    /// 커서가 이 모니터를 벗어났거나 전역 레이저가 꺼졌을 때 점을 숨기고 꼬리를 끊는다.
    /// 이미 그려진 꼬리는 레이저 타이머가 설정 시간에 맞춰 줄인다.
    pub fn global_laser_leave(&mut self) -> InputOutcome {
        InputOutcome {
            redraw: self.laser.active_pointer_id == Some(GLOBAL_LASER_POINTER_ID)
                && self.laser.break_trail(true),
            ..InputOutcome::default()
        }
    }

    pub fn retract_laser(&mut self) -> Result<InputOutcome> {
        let before = self
            .laser
            .trails
            .iter()
            .map(|trail| trail.trail.len())
            .sum::<usize>();
        self.laser
            .retract(monotonic_seconds()?, self.config.laser_trail_duration);
        let after = self
            .laser
            .trails
            .iter()
            .map(|trail| trail.trail.len())
            .sum::<usize>();
        Ok(InputOutcome {
            redraw: before != after,
            ..InputOutcome::default()
        })
    }

    pub fn finish_active(&mut self, reason: &str) -> InputOutcome {
        let mut outcome = self.finish_owned(reason, true);
        if self.laser.has_visual() {
            self.logger
                .event(&format!("레이저 상태: {reason}으로 clear"));
            self.laser.clear();
            outcome.redraw = true;
        }
        self.eraser_cursor = None;
        outcome
    }

    pub fn handle_message(
        &mut self,
        message: u32,
        pointer_id: u32,
        monitor: &Monitor,
        strokes: &[InkStroke],
        selected_ids: &BTreeSet<String>,
        mapping: InkInputMapping,
    ) -> Result<InputOutcome> {
        if !self.annotation_enabled {
            return Ok(InputOutcome::default());
        }
        self.dpi = monitor.dpi.0;
        match message {
            WM_POINTERCAPTURECHANGED => {
                let mut outcome = self.finish_owned("WM_POINTERCAPTURECHANGED", true);
                if self.laser.break_trail(true) {
                    self.logger
                        .event("레이저 상태: capture changed, 점 숨김/꼬리 끊김");
                    outcome.redraw = true;
                }
                return Ok(outcome);
            }
            WM_POINTERLEAVE => {
                // 장치 종류와 플래그는 leave가 아니라 실제 WM_POINTER 입력에서만 읽는다.
                let cursor_hidden = self.eraser_cursor.take().is_some();
                let laser_owned = self.owner.is_some()
                    && self.active_stroke.is_none()
                    && self.active_eraser.is_none()
                    && self.laser.trailing;
                let laser_changed = self.laser.break_trail(true);
                if laser_owned {
                    self.owner = None;
                }
                if laser_changed {
                    self.logger
                        .event("레이저 상태: 범위 이탈, 점 숨김/꼬리 끊김");
                }
                return Ok(InputOutcome {
                    redraw: cursor_hidden || laser_changed,
                    ..InputOutcome::default()
                });
            }
            WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {}
            _ => return Ok(InputOutcome::default()),
        }

        let snapshot = PointerSnapshot::read(pointer_id, monitor.rect, mapping)?;
        let mut palm_outcome = InputOutcome::default();
        if snapshot.device == PointerDevice::Pen {
            self.last_pen_seen = Some(monotonic_seconds()?);
            palm_outcome = self.discard_touch_owner();
        } else if snapshot.device == PointerDevice::Touch
            && self
                .owner
                .is_none_or(|owner| owner.pointer_id != snapshot.pointer_id)
            && palm_rejects_touch(self.last_pen_seen, monotonic_seconds()?)
        {
            if message == WM_POINTERDOWN {
                self.logger.event(&format!(
                    "팜 리젝션: 펜 사용 중 터치 무시 monitor={} pointerId={}",
                    self.monitor_key, snapshot.pointer_id
                ));
            }
            return Ok(palm_outcome);
        }
        let mut outcome = if message == WM_POINTERDOWN {
            self.handle_down(snapshot, monitor, strokes, selected_ids, mapping)
        } else {
            self.handle_update_or_up(message, snapshot, monitor, mapping)
        }?;
        merge_outcome(&mut outcome, palm_outcome);
        Ok(outcome)
    }

    /// 펜이 감지되면 그 전에 손바닥(터치)이 시작한 입력을 버린다. 획은 확정하지 않고 폐기하고,
    /// 지우개·선택처럼 이미 화면을 바꾼 입력은 정상 종료한다. 이렇게 해야 손바닥이 쥔 입력
    /// 소유권 때문에 펜 입력이 무시되지 않는다.
    fn discard_touch_owner(&mut self) -> InputOutcome {
        let Some(owner) = self
            .owner
            .filter(|owner| owner.device == PointerDevice::Touch)
        else {
            return InputOutcome::default();
        };
        if self.active_stroke.take().is_some() {
            self.owner = None;
            self.recognition_timer_waiting = false;
            self.recognition_timer_reset_requested = false;
            self.logger.event(&format!(
                "팜 리젝션: 펜 감지로 터치 획 폐기 monitor={} pointerId={}",
                self.monitor_key, owner.pointer_id
            ));
            return InputOutcome {
                redraw: true,
                ..InputOutcome::default()
            };
        }
        self.finish_owned("팜 리젝션: 펜 감지", true)
    }

    fn handle_down(
        &mut self,
        snapshot: PointerSnapshot,
        monitor: &Monitor,
        strokes: &[InkStroke],
        selected_ids: &BTreeSet<String>,
        mapping: InkInputMapping,
    ) -> Result<InputOutcome> {
        let mut outcome = InputOutcome::default();
        if let Some(owner) = self.owner {
            let owner_pressed = self.owner_is_pressed(owner);
            match ownership_on_down(true, owner_pressed) {
                OwnershipTransition::Ignore => {
                    self.logger.event(&format!(
                        "무시한 입력: owner={} pointerId={} incoming={:?}/{}",
                        owner.label(),
                        owner.pointer_id,
                        snapshot.device,
                        snapshot.pointer_id
                    ));
                    return Ok(outcome);
                }
                OwnershipTransition::RecoverAndAccept => {
                    self.logger.event(&format!(
                        "누락 up 복구: owner={} pointerId={}; 진행 제스처 확정 후 새 입력 수락",
                        owner.label(),
                        owner.pointer_id
                    ));
                    merge_outcome(&mut outcome, self.finish_owned("누락 up 복구", true));
                }
                OwnershipTransition::Accept => unreachable!("owner exists"),
            }
        }

        let owner = snapshot.owner();
        if snapshot_uses_eraser(self.config.current_tool, &snapshot) {
            let Some(settings) = self.scaled_eraser_settings() else {
                self.logger.event("무시한 입력: 유효하지 않은 지우개 설정");
                return Ok(outcome);
            };
            let points =
                pointer_screen_points(snapshot.pointer_id, snapshot.device, monitor.rect, mapping)?;
            let canvas_size = Size {
                width: f64::from(monitor.rect.width()),
                height: f64::from(monitor.rect.height()),
            };
            let mut effective_strokes = outcome
                .replacement
                .as_ref()
                .map(|replacement| replacement.after.clone())
                .unwrap_or_else(|| strokes.to_vec());
            effective_strokes.extend(outcome.finished.iter().cloned());
            self.owner = Some(owner);
            self.active_eraser = Some(ActiveEraser {
                settings,
                points: if points.is_empty() {
                    vec![snapshot.local_point]
                } else {
                    points
                },
                before: effective_strokes,
                canvas_size,
                preview_strokes: None,
                highlighted_strokes: Vec::new(),
            });
            self.eraser_cursor = Some(snapshot.local_point);
            self.laser.break_trail(true);
            self.refresh_eraser_preview();
            self.logger.event(&format!(
                "지우개 제스처 시작: monitor={} mode={:?} owner={} diameter_px={:.2}",
                self.monitor_key,
                settings.mode,
                owner.label(),
                settings.diameter
            ));
            // 지우개 모드 HUD는 후속 팔레트 라운드 전까지 이 로그 한 줄로 대체한다.
            self.logger
                .event(&format!("지우개 모드 HUD 대체 로그: {:?}", settings.mode));
            outcome.redraw = true;
            if snapshot.canceled {
                merge_outcome(
                    &mut outcome,
                    self.finish_owned("POINTER_FLAG_CANCELED", true),
                );
            }
            return Ok(outcome);
        }

        if self.config.current_tool == InkTool::Text {
            if !snapshot_starts_text_entry(&snapshot) {
                self.logger.event(&format!(
                    "무시한 텍스트 입력: monitor={} device={:?} 접촉/좌클릭 아님",
                    self.monitor_key, snapshot.device
                ));
                return Ok(outcome);
            }
            let Some(color) = self.config.current_color else {
                self.logger.event(&format!(
                    "무시한 텍스트 입력: 도구 색 인덱스 범위 밖 ({:?})",
                    self.config.current_color_index
                ));
                return Ok(outcome);
            };
            let Some(font_size) = self.config.current_width else {
                self.logger.event("무시한 텍스트 입력: 글꼴 크기 설정 없음");
                return Ok(outcome);
            };
            outcome.text_entry = Some(TextEntrySeed {
                origin: snapshot.local_point,
                display_origin: snapshot.presentation_point,
                color,
                font_size,
            });
            return Ok(outcome);
        }

        if self.config.current_tool == InkTool::Select {
            let canvas_size = Size {
                width: f64::from(monitor.rect.width()),
                height: f64::from(monitor.rect.height()),
            };
            let mut effective_strokes = outcome
                .replacement
                .as_ref()
                .map(|replacement| replacement.after.clone())
                .unwrap_or_else(|| strokes.to_vec());
            effective_strokes.extend(outcome.finished.iter().cloned());
            let effective_selection = outcome.selected_ids.as_ref().unwrap_or(selected_ids);
            self.owner = Some(owner);
            if self.begin_selection(
                snapshot.local_point,
                effective_strokes,
                effective_selection,
                canvas_size,
            ) {
                outcome.selected_ids = Some(BTreeSet::new());
            }
            outcome.redraw = true;
            if snapshot.canceled {
                merge_outcome(
                    &mut outcome,
                    self.finish_owned("POINTER_FLAG_CANCELED", true),
                );
            }
            return Ok(outcome);
        }

        if self.config.current_tool == InkTool::Laser
            && laser_action(
                PointerPhase::Down,
                snapshot.device,
                snapshot.in_range,
                snapshot.in_contact,
                snapshot.first_button,
                false,
            ) == LaserAction::Trail
        {
            self.owner = Some(owner);
            let new_pointer = self.laser.begin_or_append(
                snapshot.pointer_id,
                snapshot.local_point,
                monotonic_seconds()?,
            );
            if new_pointer {
                self.logger.event(&format!(
                    "레이저 상태: 새 pointerId={}, 이전 꼬리와 분리",
                    snapshot.pointer_id
                ));
            }
            self.logger.event(&format!(
                "레이저 상태: 꼬리 시작 pointerId={} owner={}",
                snapshot.pointer_id,
                owner.label()
            ));
            outcome.redraw = true;
            return Ok(outcome);
        }

        let (stroke_tool, uses_pressure, content) = match self.config.current_tool {
            InkTool::Pen => (InkStrokeTool::Pen, true, InkStrokeContent::Freehand),
            InkTool::Highlighter => (
                InkStrokeTool::Highlighter,
                false,
                InkStrokeContent::Freehand,
            ),
            InkTool::Arrow => (
                InkStrokeTool::Shape,
                false,
                InkStrokeContent::Shape(InkShapeKind::Arrow(self.config.arrow_style)),
            ),
            InkTool::Rectangle => (
                InkStrokeTool::Shape,
                false,
                InkStrokeContent::Shape(InkShapeKind::Rectangle),
            ),
            InkTool::Ellipse => (
                InkStrokeTool::Shape,
                false,
                InkStrokeContent::Shape(InkShapeKind::Ellipse),
            ),
            InkTool::Cover => (InkStrokeTool::Cover, false, InkStrokeContent::Cover),
            other => {
                self.logger
                    .event(&format!("무시한 입력: 미구현 도구: {other:?}"));
                return Ok(outcome);
            }
        };
        let (color, width) = if stroke_tool == InkStrokeTool::Cover {
            (COVER_STROKE_COLOR, 1.0)
        } else {
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
            (color, width)
        };

        let history = pointer_history(snapshot.pointer_id, snapshot.device, monitor.rect, mapping)?;
        if history.points.is_empty() {
            self.logger.event(&format!(
                "무시한 입력: 포인터 이력 점 없음, pointerId={}",
                snapshot.pointer_id
            ));
            return Ok(outcome);
        }
        let uses_pressure = uses_pressure && snapshot.device == PointerDevice::Pen;
        let is_straight = shift_is_pressed()
            && matches!(
                stroke_tool,
                InkStrokeTool::Pen | InkStrokeTool::Highlighter | InkStrokeTool::Shape
            );
        let identifier = format!(
            "win-{}-{}-{}",
            history.points[0].timestamp,
            snapshot.pointer_id,
            STROKE_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed)
        );
        let points = if matches!(&content, InkStrokeContent::Freehand) {
            history.points
        } else {
            let first = history.points[0].clone();
            let last = history
                .points
                .last()
                .cloned()
                .unwrap_or_else(|| first.clone());
            vec![first, last]
        };
        let stroke = InkStroke::new(
            identifier,
            stroke_tool,
            color,
            width,
            uses_pressure,
            points,
            content,
        );
        match &stroke.content {
            InkStrokeContent::Shape(kind) => self.logger.event(&format!(
                "도형 제스처 시작: monitor={} kind={} constrained={is_straight}",
                self.monitor_key,
                shape_kind_label(*kind)
            )),
            InkStrokeContent::Cover => self.logger.event(&format!(
                "가리개 제스처 시작: monitor={}",
                self.monitor_key
            )),
            _ => self.logger.event(&format!(
                "획 시작: monitor={} pointerId={} device={:?} tool={:?} colorIndex={} width={} straight={is_straight}",
                self.monitor_key,
                snapshot.pointer_id,
                snapshot.device,
                self.config.current_tool,
                self.config.current_color_index.unwrap_or(-1),
                width
            )),
        }
        self.owner = Some(owner);
        self.active_stroke = Some(ActiveStroke {
            stroke,
            is_straight,
            canvas_size: Size {
                width: f64::from(monitor.rect.width()),
                height: f64::from(monitor.rect.height()),
            },
            recognized: None,
        });
        self.schedule_recognition_if_needed();
        outcome.redraw = true;
        if history.canceled || snapshot.canceled {
            merge_outcome(
                &mut outcome,
                self.finish_owned("POINTER_FLAG_CANCELED", true),
            );
        }
        Ok(outcome)
    }

    fn handle_update_or_up(
        &mut self,
        message: u32,
        snapshot: PointerSnapshot,
        monitor: &Monitor,
        mapping: InkInputMapping,
    ) -> Result<InputOutcome> {
        let mut outcome = InputOutcome::default();
        let Some(owner) = self.owner else {
            return self.handle_hover(message, snapshot);
        };
        if owner.pointer_id != snapshot.pointer_id {
            self.logger.event(&format!(
                "무시한 입력: 다른 포인터, owner={} incoming={}",
                owner.pointer_id, snapshot.pointer_id
            ));
            return Ok(outcome);
        }
        if message == WM_POINTERUP && !snapshot.release_matches(owner) {
            self.logger.event(&format!(
                "무시한 입력: 소유하지 않은 버튼 up, owner={} pointerId={}",
                owner.label(),
                owner.pointer_id
            ));
            return Ok(outcome);
        }

        if self.active_selection.is_some() {
            self.update_selection(snapshot, monitor, mapping)?;
            outcome.redraw = true;
        } else if self.active_eraser.is_some() {
            let points =
                pointer_screen_points(snapshot.pointer_id, snapshot.device, monitor.rect, mapping)?;
            if let Some(active) = self.active_eraser.as_mut() {
                append_unique_points(&mut active.points, points);
            }
            self.eraser_cursor = Some(snapshot.local_point);
            self.refresh_eraser_preview();
            outcome.redraw = true;
        } else if let Some(active) = self.active_stroke.as_mut() {
            let history =
                pointer_history(snapshot.pointer_id, snapshot.device, monitor.rect, mapping)?;
            let points_added = match &active.stroke.content {
                InkStrokeContent::Shape(_) | InkStrokeContent::Cover => history
                    .points
                    .last()
                    .is_some_and(|point| set_stroke_endpoint(&mut active.stroke, point.clone())),
                _ => merge_points(&mut active.stroke.points, history.points),
            };
            if points_added {
                self.schedule_recognition_if_needed();
            }
            outcome.redraw = true;
        } else if self.laser.trailing {
            if snapshot_uses_eraser(self.config.current_tool, &snapshot) {
                if self.laser.break_trail(true) {
                    self.logger
                        .event("레이저 상태: 지우개 끝/측면 버튼으로 점 숨김/꼬리 끊김");
                }
                self.eraser_cursor = Some(snapshot.local_point);
                outcome.redraw = true;
            } else if message != WM_POINTERUP && snapshot.contact_for_laser() {
                let new_pointer = self.laser.begin_or_append(
                    snapshot.pointer_id,
                    snapshot.local_point,
                    monotonic_seconds()?,
                );
                debug_assert!(!new_pointer);
                outcome.redraw = true;
            } else {
                self.laser.point = snapshot.in_range.then_some(snapshot.local_point);
            }
        }

        if message == WM_POINTERUP || snapshot.canceled {
            let reason = if snapshot.canceled {
                "POINTER_FLAG_CANCELED"
            } else {
                "WM_POINTERUP"
            };
            merge_outcome(&mut outcome, self.finish_owned(reason, snapshot.canceled));
        }
        Ok(outcome)
    }

    fn begin_selection(
        &mut self,
        point: Point,
        before: Vec<InkStroke>,
        selected_ids: &BTreeSet<String>,
        canvas_size: Size,
    ) -> bool {
        let bounds = selection_bounds(&before, selected_ids, canvas_size);
        match selection_hit(point, bounds) {
            SelectionHit::Resize { anchor } => {
                let Some(source_bounds) = normalized_point_bounds(&before, selected_ids) else {
                    self.active_selection = Some(ActiveSelection::Lasso {
                        points: vec![point],
                        before,
                        canvas_size,
                    });
                    self.logger
                        .event(&format!("올가미 시작: monitor={}", self.monitor_key));
                    return true;
                };
                self.active_selection = Some(ActiveSelection::Resize {
                    anchor: InkGeometry::normalized_point(
                        Point {
                            x: anchor.x,
                            y: anchor.y,
                        },
                        canvas_size,
                    ),
                    source_bounds,
                    before,
                    selected_ids: selected_ids.clone(),
                    canvas_size,
                    preview: None,
                });
                self.logger.event(&format!(
                    "선택 크기 조절 시작: monitor={}",
                    self.monitor_key
                ));
                false
            }
            SelectionHit::Move => {
                self.active_selection = Some(ActiveSelection::Move {
                    start: point,
                    before,
                    selected_ids: selected_ids.clone(),
                    canvas_size,
                    preview: None,
                });
                self.logger
                    .event(&format!("선택 이동 시작: monitor={}", self.monitor_key));
                false
            }
            SelectionHit::Lasso => {
                self.active_selection = Some(ActiveSelection::Lasso {
                    points: vec![point],
                    before,
                    canvas_size,
                });
                self.logger
                    .event(&format!("올가미 시작: monitor={}", self.monitor_key));
                true
            }
        }
    }

    fn update_selection(
        &mut self,
        snapshot: PointerSnapshot,
        monitor: &Monitor,
        mapping: InkInputMapping,
    ) -> Result<()> {
        let lasso_points = matches!(self.active_selection, Some(ActiveSelection::Lasso { .. }))
            .then(|| {
                pointer_screen_points(snapshot.pointer_id, snapshot.device, monitor.rect, mapping)
            })
            .transpose()?;
        let Some(selection) = self.active_selection.as_mut() else {
            return Ok(());
        };
        match selection {
            ActiveSelection::Lasso { points, .. } => {
                append_unique_points(
                    points,
                    lasso_points.unwrap_or_else(|| vec![snapshot.local_point]),
                );
            }
            ActiveSelection::Move {
                start,
                before,
                selected_ids,
                canvas_size,
                preview,
            } => {
                let delta = Point {
                    x: (snapshot.local_point.x - start.x) / canvas_size.width.max(1.0),
                    y: (snapshot.local_point.y - start.y) / canvas_size.height.max(1.0),
                };
                *preview = Some(moved_strokes(before, selected_ids, delta));
            }
            ActiveSelection::Resize {
                anchor,
                source_bounds,
                before,
                selected_ids,
                canvas_size,
                preview,
            } => {
                let current = InkGeometry::normalized_point(snapshot.local_point, *canvas_size);
                let destination = SelectionRect::new(
                    anchor.x.min(current.x),
                    anchor.y.min(current.y),
                    (current.x - anchor.x).abs().max(0.001),
                    (current.y - anchor.y).abs().max(0.001),
                );
                *preview = Some(resized_strokes(
                    before,
                    selected_ids,
                    *source_bounds,
                    destination,
                ));
            }
        }
        Ok(())
    }

    fn handle_hover(&mut self, message: u32, snapshot: PointerSnapshot) -> Result<InputOutcome> {
        let mut redraw = false;
        if snapshot_uses_eraser(self.config.current_tool, &snapshot) {
            redraw |= self.eraser_cursor != Some(snapshot.local_point);
            self.eraser_cursor = Some(snapshot.local_point);
            if self.laser.break_trail(true) {
                self.logger
                    .event("레이저 상태: 지우개 사용 가능, 점 숨김/꼬리 끊김");
                redraw = true;
            }
        } else {
            redraw |= self.eraser_cursor.take().is_some();
            if self.config.current_tool == InkTool::Laser {
                match laser_action(
                    if message == WM_POINTERUP {
                        PointerPhase::Up
                    } else {
                        PointerPhase::Update
                    },
                    snapshot.device,
                    snapshot.in_range,
                    snapshot.in_contact,
                    snapshot.first_button,
                    false,
                ) {
                    LaserAction::Hover | LaserAction::Release if snapshot.in_range => {
                        let new_pointer =
                            is_new_laser_pointer(self.laser.last_pointer_id, snapshot.pointer_id);
                        let became_visible = self
                            .laser
                            .show_point(snapshot.pointer_id, snapshot.local_point);
                        if new_pointer {
                            self.logger.event(&format!(
                                "레이저 상태: 새 pointerId={}, 이전 꼬리와 분리",
                                snapshot.pointer_id
                            ));
                        } else if became_visible {
                            self.logger.event(&format!(
                                "레이저 상태: 점 표시 pointerId={}",
                                snapshot.pointer_id
                            ));
                        }
                        redraw = true;
                    }
                    _ => {}
                }
            } else if self.laser.break_trail(true) {
                redraw = true;
            }
        }
        Ok(InputOutcome {
            redraw,
            ..InputOutcome::default()
        })
    }

    fn owner_is_pressed(&self, owner: InputOwner) -> bool {
        match owner.button {
            OwnerButton::MousePrimary => {
                (unsafe { GetAsyncKeyState(VK_LBUTTON.0.into()) } as u16 & 0x8000) != 0
            }
            OwnerButton::MouseSecondary => {
                (unsafe { GetAsyncKeyState(VK_RBUTTON.0.into()) } as u16 & 0x8000) != 0
            }
            OwnerButton::PointerContact => {
                if owner.device == PointerDevice::Pen {
                    let mut pen = POINTER_PEN_INFO::default();
                    // Only queried while handling a new WM_POINTERDOWN to recover a lost up.
                    return match unsafe { GetPointerPenInfo(owner.pointer_id, &mut pen) } {
                        Ok(()) => {
                            pen.pointerInfo
                                .pointerFlags
                                .contains(POINTER_FLAG_INCONTACT)
                                || pen.penFlags & PEN_FLAG_BARREL != 0
                        }
                        Err(error) => {
                            self.logger.event(&format!(
                                "누락 up 복구 펜 상태 조회 실패: pointerId={} error={error}; released로 처리",
                                owner.pointer_id
                            ));
                            false
                        }
                    };
                }
                let mut info = POINTER_INFO::default();
                // Only queried while handling a new WM_POINTERDOWN to recover a lost up.
                match unsafe { GetPointerInfo(owner.pointer_id, &mut info) } {
                    Ok(()) => info.pointerFlags.contains(POINTER_FLAG_INCONTACT),
                    Err(error) => {
                        self.logger.event(&format!(
                            "누락 up 복구 상태 조회 실패: pointerId={} error={error}; released로 처리",
                            owner.pointer_id
                        ));
                        false
                    }
                }
            }
        }
    }

    fn finish_owned(&mut self, reason: &str, hide_laser_point: bool) -> InputOutcome {
        let owner = self.owner.take();
        let mut outcome = InputOutcome::default();
        let had_selection = self.active_selection.is_some();
        merge_outcome(&mut outcome, self.take_active_selection(reason));
        outcome.redraw |= had_selection;
        let had_stroke = self.active_stroke.is_some();
        if let Some((stroke, recognized)) = self.take_active_stroke(reason, owner) {
            outcome.finished.push(stroke);
            outcome.recognized = recognized;
        }
        outcome.redraw |= had_stroke;
        let had_eraser = self.active_eraser.is_some();
        if let Some(replacement) = self.take_active_eraser(reason, owner) {
            debug_assert!(outcome.replacement.is_none());
            outcome.replacement = Some(replacement);
        }
        outcome.redraw |= had_eraser;
        if owner.is_some() && self.laser.trailing && self.laser.break_trail(hide_laser_point) {
            self.logger.event(&format!(
                "레이저 상태: {reason}, 꼬리 끊김{}",
                if hide_laser_point { "/점 숨김" } else { "" }
            ));
            outcome.redraw = true;
        }
        outcome
    }

    fn take_active_stroke(
        &mut self,
        reason: &str,
        owner: Option<InputOwner>,
    ) -> Option<(InkStroke, Option<InkStroke>)> {
        let active = self.active_stroke.take()?;
        self.recognition_timer_waiting = false;
        self.recognition_timer_reset_requested = false;
        let stroke = effective_stroke(&active);
        let recognized = if reason == "WM_POINTERUP" {
            active.recognized
        } else {
            if let Some(candidate) = active.recognized.as_ref() {
                self.logger.event(&format!(
                    "도형 인식 후보 폐기: monitor={} kind={} reason={reason}",
                    self.monitor_key,
                    stroke_shape_label(candidate)
                ));
            }
            None
        };
        if matches!(&stroke.content, InkStrokeContent::Cover) {
            let first = stroke.points.first()?.location;
            let last = stroke.points.last()?.location;
            let size = cover_size(first, last, active.canvas_size);
            if !cover_meets_minimum_size(first, last, active.canvas_size) {
                self.logger.event(&format!(
                    "가리개 버림: monitor={} size={:.2}x{:.2}px minimum=4x4px",
                    self.monitor_key, size.width, size.height
                ));
                return None;
            }
            self.logger.event(&format!(
                "가리개 확정: monitor={} size={:.2}x{:.2}px",
                self.monitor_key, size.width, size.height
            ));
            return Some((stroke, None));
        }
        if let InkStrokeContent::Shape(kind) = &stroke.content {
            self.logger.event(&format!(
                "도형 확정: monitor={} kind={} constrained={}",
                self.monitor_key,
                shape_kind_label(*kind),
                active.is_straight
            ));
            return Some((stroke, None));
        }
        let (minimum, maximum) = stroke.points.iter().map(|point| point.pressure).fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
        );
        self.logger.event(&format!(
            "획 확정: monitor={} pointerId={} reason={reason} points={} uses_pressure={} pressure={minimum:.3}..{maximum:.3}",
            self.monitor_key,
            owner.map_or(0, |owner| owner.pointer_id),
            stroke.points.len(),
            stroke.uses_pressure
        ));
        if active.is_straight {
            self.logger.event(&format!(
                "Shift 직선 적용: monitor={} tool={:?}",
                self.monitor_key, stroke.tool
            ));
        }
        if stroke.tool == InkStrokeTool::Highlighter {
            self.logger.event(&format!(
                "형광펜 획 확정: monitor={} points={} width={} opacity={}",
                self.monitor_key,
                stroke.points.len(),
                stroke.width,
                self.config.highlighter_opacity
            ));
        }
        Some((stroke, recognized))
    }

    fn take_active_eraser(
        &mut self,
        reason: &str,
        owner: Option<InputOwner>,
    ) -> Option<StrokeReplacement> {
        let active = self.active_eraser.take()?;
        let gesture = eraser_gesture(&active)?;
        let after = EraserEngine::erase(
            &active.before,
            &gesture,
            &active.settings,
            active.canvas_size,
            self.config.pressure_minimum_width_ratio,
        );
        let erased_strokes = removed_original_count(&active.before, &after);
        let undo_units = usize::from(active.before != after);
        self.logger.event(&format!(
            "지우개 제스처 종료: monitor={} mode={:?} owner={} reason={reason} erased={} Undo={undo_units}",
            self.monitor_key,
            active.settings.mode,
            owner.map_or("unknown", InputOwner::label),
            erased_strokes
        ));
        self.eraser_cursor = owner
            .filter(|_| reason == "WM_POINTERUP")
            .and_then(|_| active.points.last().copied());
        (active.before != after).then_some(StrokeReplacement {
            before: active.before,
            after,
            kind: StrokeReplacementKind::Eraser { erased_strokes },
        })
    }

    fn take_active_selection(&mut self, reason: &str) -> InputOutcome {
        let Some(active) = self.active_selection.take() else {
            return InputOutcome::default();
        };
        let should_commit = matches!(reason, "WM_POINTERUP" | "누락 up 복구");
        if !should_commit {
            self.logger.event(&format!(
                "선택 상호작용 취소: monitor={} reason={reason}",
                self.monitor_key
            ));
            return InputOutcome {
                redraw: true,
                ..InputOutcome::default()
            };
        }

        match active {
            ActiveSelection::Lasso {
                points,
                before,
                canvas_size,
            } => {
                let selected = if points.len() < 3 {
                    BTreeSet::new()
                } else {
                    selected_stroke_ids(&before, &points, self.selection_hit_rule(), canvas_size)
                };
                self.logger.event(&format!(
                    "올가미 확정: monitor={} selected={}",
                    self.monitor_key,
                    selected.len()
                ));
                InputOutcome {
                    selected_ids: Some(selected),
                    redraw: true,
                    ..InputOutcome::default()
                }
            }
            ActiveSelection::Move {
                before, preview, ..
            } => self.finish_selection_transform(
                before,
                preview,
                StrokeReplacementKind::SelectionMove,
                "이동",
            ),
            ActiveSelection::Resize {
                before, preview, ..
            } => self.finish_selection_transform(
                before,
                preview,
                StrokeReplacementKind::SelectionResize,
                "크기 조절",
            ),
        }
    }

    fn finish_selection_transform(
        &self,
        before: Vec<InkStroke>,
        preview: Option<Vec<InkStroke>>,
        kind: StrokeReplacementKind,
        label: &str,
    ) -> InputOutcome {
        let replacement = preview
            .filter(|after| *after != before)
            .map(|after| StrokeReplacement {
                before,
                after,
                kind,
            });
        self.logger.event(&format!(
            "선택 {label} 확정: monitor={} Undo={}",
            self.monitor_key,
            usize::from(replacement.is_some())
        ));
        InputOutcome {
            replacement,
            redraw: true,
            ..InputOutcome::default()
        }
    }

    fn selection_hit_rule(&self) -> LassoHitRule {
        match self
            .config
            .eraser_settings
            .map(|settings| settings.hit_rule)
        {
            Some(EraserHitRule::Contained) => LassoHitRule::Contained,
            Some(EraserHitRule::Touching) | None => LassoHitRule::Touching,
        }
    }

    fn refresh_eraser_preview(&mut self) {
        let Some(active) = self.active_eraser.as_mut() else {
            return;
        };
        let Some(gesture) = eraser_gesture(active) else {
            return;
        };
        let after = EraserEngine::erase(
            &active.before,
            &gesture,
            &active.settings,
            active.canvas_size,
            self.config.pressure_minimum_width_ratio,
        );
        match active.settings.mode {
            EraserMode::Partial => {
                active.preview_strokes = Some(after);
                active.highlighted_strokes.clear();
            }
            EraserMode::Stroke => {
                active.highlighted_strokes = active
                    .before
                    .iter()
                    .filter(|stroke| {
                        !after
                            .iter()
                            .any(|candidate| candidate.identifier == stroke.identifier)
                    })
                    .cloned()
                    .collect();
                active.preview_strokes = None;
            }
            EraserMode::Area => {
                active.preview_strokes = None;
                active.highlighted_strokes.clear();
            }
        }
    }

    fn recognition_is_eligible(&self) -> bool {
        self.annotation_enabled
            && self.config.current_tool == InkTool::Pen
            && self.config.shape_recognition_enabled
            && self.active_stroke.as_ref().is_some_and(|active| {
                active.stroke.tool == InkStrokeTool::Pen
                    && matches!(&active.stroke.content, InkStrokeContent::Freehand)
                    && !active.is_straight
                    && active.stroke.points.len() >= 2
            })
    }

    fn schedule_recognition_if_needed(&mut self) {
        if !self.recognition_is_eligible() {
            self.recognition_timer_waiting = false;
            self.recognition_timer_reset_requested = false;
            return;
        }
        if let Some(candidate) = self
            .active_stroke
            .as_mut()
            .and_then(|active| active.recognized.take())
        {
            self.logger.event(&format!(
                "도형 인식 후보 폐기: monitor={} kind={} reason=새 점",
                self.monitor_key,
                stroke_shape_label(&candidate)
            ));
        }
        self.recognition_timer_waiting = true;
        self.recognition_timer_reset_requested = true;
    }

    fn cancel_recognition(&mut self, reason: &str) {
        if let Some(candidate) = self
            .active_stroke
            .as_mut()
            .and_then(|active| active.recognized.take())
        {
            self.logger.event(&format!(
                "도형 인식 후보 폐기: monitor={} kind={} reason={reason}",
                self.monitor_key,
                stroke_shape_label(&candidate)
            ));
        }
        self.recognition_timer_waiting = false;
        self.recognition_timer_reset_requested = false;
    }

    fn scaled_eraser_settings(&self) -> Option<EraserSettings> {
        self.config.eraser_settings.map(|mut settings| {
            settings.diameter *= px_per_dip(self.dpi);
            settings
        })
    }
}

fn effective_stroke(active: &ActiveStroke) -> InkStroke {
    let mut stroke = active.stroke.clone();
    if !active.is_straight {
        return stroke;
    }
    let (Some(first), Some(last)) = (
        stroke.points.first().cloned(),
        stroke.points.last().cloned(),
    ) else {
        return stroke;
    };
    let start = InkGeometry::screen_point(first.location, active.canvas_size);
    let proposed = InkGeometry::screen_point(last.location, active.canvas_size);
    let endpoint = match &stroke.content {
        InkStrokeContent::Shape(kind) => shape_endpoint(*kind, true, start, proposed),
        InkStrokeContent::Freehand if stroke.tool == InkStrokeTool::Pen => {
            InkGeometry::snapped_pen_endpoint(start, proposed)
        }
        InkStrokeContent::Freehand if stroke.tool == InkStrokeTool::Highlighter => {
            InkGeometry::snapped_highlighter_endpoint(start, proposed)
        }
        _ => proposed,
    };
    stroke.points = vec![
        first,
        InkPoint::new(
            InkGeometry::normalized_point(endpoint, active.canvas_size),
            last.pressure,
            last.timestamp,
        ),
    ];
    stroke
}

fn shape_endpoint(
    kind: InkShapeKind,
    is_constrained: bool,
    start: Point,
    proposed: Point,
) -> Point {
    if !is_constrained {
        return proposed;
    }
    match kind {
        InkShapeKind::Line | InkShapeKind::Arrow(_) => {
            InkGeometry::snapped_pen_endpoint(start, proposed)
        }
        InkShapeKind::Rectangle | InkShapeKind::Ellipse => {
            InkGeometry::snapped_square_endpoint(start, proposed)
        }
    }
}

fn cover_size(start: Point, end: Point, canvas_size: Size) -> Size {
    let start = InkGeometry::screen_point(start, canvas_size);
    let end = InkGeometry::screen_point(end, canvas_size);
    Size {
        width: (end.x - start.x).abs(),
        height: (end.y - start.y).abs(),
    }
}

fn cover_meets_minimum_size(start: Point, end: Point, canvas_size: Size) -> bool {
    let size = cover_size(start, end, canvas_size);
    size.width >= 4.0 && size.height >= 4.0
}

fn shape_kind_label(kind: InkShapeKind) -> &'static str {
    match kind {
        InkShapeKind::Line => "line",
        InkShapeKind::Arrow(ArrowStyle::Single) => "arrow-single",
        InkShapeKind::Arrow(ArrowStyle::Double) => "arrow-double",
        InkShapeKind::Rectangle => "rectangle",
        InkShapeKind::Ellipse => "ellipse",
    }
}

pub(crate) fn stroke_shape_label(stroke: &InkStroke) -> &'static str {
    match &stroke.content {
        InkStrokeContent::Shape(kind) => shape_kind_label(*kind),
        _ => "none",
    }
}

fn shift_is_pressed() -> bool {
    (unsafe { GetKeyState(VK_SHIFT.0 as i32) } as u16 & 0x8000) != 0
}

fn snapshot_starts_text_entry(snapshot: &PointerSnapshot) -> bool {
    match snapshot.device {
        PointerDevice::Mouse => snapshot.first_button && !snapshot.second_button,
        PointerDevice::Pen | PointerDevice::Touch => snapshot.in_contact,
        PointerDevice::Pointer | PointerDevice::Touchpad | PointerDevice::Unknown(_) => false,
    }
}

#[derive(Clone, Copy, Debug)]
struct PointerSnapshot {
    pointer_id: u32,
    device: PointerDevice,
    local_point: Point,
    presentation_point: Point,
    in_range: bool,
    in_contact: bool,
    first_button: bool,
    second_button: bool,
    first_button_change: bool,
    second_button_change: bool,
    barrel: bool,
    eraser: bool,
    inverted: bool,
    canceled: bool,
}

impl PointerSnapshot {
    fn read(pointer_id: u32, monitor: MonitorRect, mapping: InkInputMapping) -> Result<Self> {
        let mut info = POINTER_INFO::default();
        // Device type and pen flags are read from this WM_POINTER event only.
        unsafe { GetPointerInfo(pointer_id, &mut info) }?;
        let device = PointerDevice::from_win32(info.pointerType);
        let pen_flags = if device == PointerDevice::Pen {
            let mut pen = POINTER_PEN_INFO::default();
            unsafe { GetPointerPenInfo(pointer_id, &mut pen) }?;
            info = pen.pointerInfo;
            pen.penFlags
        } else {
            0
        };
        let presentation_point = screen_point(info.ptPixelLocationRaw, monitor);
        Ok(Self {
            pointer_id,
            device,
            local_point: mapping.map_local(presentation_point),
            presentation_point,
            in_range: info.pointerFlags.contains(POINTER_FLAG_INRANGE),
            in_contact: info.pointerFlags.contains(POINTER_FLAG_INCONTACT),
            first_button: info.pointerFlags.contains(POINTER_FLAG_FIRSTBUTTON),
            second_button: info.pointerFlags.contains(POINTER_FLAG_SECONDBUTTON),
            first_button_change: info.ButtonChangeType == POINTER_CHANGE_FIRSTBUTTON_DOWN
                || info.ButtonChangeType == POINTER_CHANGE_FIRSTBUTTON_UP,
            second_button_change: info.ButtonChangeType == POINTER_CHANGE_SECONDBUTTON_DOWN
                || info.ButtonChangeType == POINTER_CHANGE_SECONDBUTTON_UP,
            barrel: pen_flags & PEN_FLAG_BARREL != 0,
            eraser: pen_flags & PEN_FLAG_ERASER != 0,
            inverted: pen_flags & PEN_FLAG_INVERTED != 0,
            canceled: info.pointerFlags.contains(POINTER_FLAG_CANCELED),
        })
    }

    fn owner(self) -> InputOwner {
        let button = if self.device == PointerDevice::Mouse
            && (self.second_button || self.second_button_change)
        {
            OwnerButton::MouseSecondary
        } else if self.device == PointerDevice::Mouse {
            OwnerButton::MousePrimary
        } else {
            OwnerButton::PointerContact
        };
        InputOwner {
            pointer_id: self.pointer_id,
            device: self.device,
            button,
        }
    }

    fn release_matches(self, owner: InputOwner) -> bool {
        match owner.button {
            OwnerButton::MousePrimary => {
                self.first_button_change || (!self.first_button && !self.second_button_change)
            }
            OwnerButton::MouseSecondary => {
                self.second_button_change || (!self.second_button && !self.first_button_change)
            }
            OwnerButton::PointerContact => true,
        }
    }

    fn contact_for_laser(self) -> bool {
        match self.device {
            PointerDevice::Mouse => self.first_button,
            PointerDevice::Pen | PointerDevice::Touch => self.in_contact,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnershipTransition {
    Accept,
    Ignore,
    RecoverAndAccept,
}

fn ownership_on_down(has_owner: bool, owner_pressed: bool) -> OwnershipTransition {
    match (has_owner, owner_pressed) {
        (false, _) => OwnershipTransition::Accept,
        (true, true) => OwnershipTransition::Ignore,
        (true, false) => OwnershipTransition::RecoverAndAccept,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
enum PointerPhase {
    Down,
    Update,
    Up,
    Leave,
    CaptureChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaserAction {
    None,
    Hover,
    Trail,
    Release,
    Break,
}

fn laser_action(
    phase: PointerPhase,
    device: PointerDevice,
    in_range: bool,
    in_contact: bool,
    first_button: bool,
    uses_eraser: bool,
) -> LaserAction {
    if matches!(phase, PointerPhase::Leave | PointerPhase::CaptureChanged) {
        return LaserAction::Break;
    }
    if uses_eraser {
        return LaserAction::None;
    }
    if phase == PointerPhase::Up {
        return LaserAction::Release;
    }
    if !in_range {
        return LaserAction::Break;
    }
    let pressed = match device {
        PointerDevice::Mouse => first_button,
        PointerDevice::Pen | PointerDevice::Touch => in_contact,
        _ => false,
    };
    if pressed {
        LaserAction::Trail
    } else if phase == PointerPhase::Update {
        LaserAction::Hover
    } else {
        LaserAction::None
    }
}

/// 펜을 마지막으로 본 뒤 이 시간(초) 안의 터치는 손바닥으로 보고 무시한다.
/// 펜이 호버 범위에 있는 동안은 WM_POINTERUPDATE가 계속 와서 창이 갱신된다.
const PALM_REJECTION_WINDOW_SECONDS: f64 = 1.0;

fn palm_rejects_touch(last_pen_seen: Option<f64>, now: f64) -> bool {
    last_pen_seen.is_some_and(|seen| now - seen <= PALM_REJECTION_WINDOW_SECONDS)
}

fn is_new_laser_pointer(previous: Option<u32>, incoming: u32) -> bool {
    previous.is_some_and(|previous| previous != incoming)
}

fn uses_eraser(
    current_tool: InkTool,
    device: PointerDevice,
    mouse_secondary: bool,
    barrel: bool,
    eraser: bool,
    inverted: bool,
) -> bool {
    current_tool == InkTool::Eraser
        || (device == PointerDevice::Mouse && mouse_secondary)
        || (device == PointerDevice::Pen && (barrel || eraser || inverted))
}

fn snapshot_uses_eraser(current_tool: InkTool, snapshot: &PointerSnapshot) -> bool {
    uses_eraser(
        current_tool,
        snapshot.device,
        snapshot.second_button || snapshot.second_button_change,
        snapshot.barrel,
        snapshot.eraser,
        snapshot.inverted,
    )
}

fn eraser_settings_from_core(settings: &AppSettings) -> Option<EraserSettings> {
    Some(EraserSettings {
        diameter: settings.eraser.diameter,
        mode: match settings.eraser.mode.as_str() {
            "partial" => EraserMode::Partial,
            "stroke" => EraserMode::Stroke,
            "area" => EraserMode::Area,
            _ => return None,
        },
        hit_rule: match settings.eraser.hit_rule.as_str() {
            "touching" => EraserHitRule::Touching,
            "contained" => EraserHitRule::Contained,
            _ => return None,
        },
        target_filter: match settings.eraser.target_filter.as_str() {
            "all" => EraserTargetFilter::All,
            "penOnly" => EraserTargetFilter::PenOnly,
            "highlighterOnly" => EraserTargetFilter::HighlighterOnly,
            _ => return None,
        },
        area_shape: match settings.eraser.area_shape.as_str() {
            "rectangle" => EraserAreaShape::Rectangle,
            "lasso" => EraserAreaShape::Lasso,
            _ => return None,
        },
    })
}

fn eraser_gesture(active: &ActiveEraser) -> Option<EraserGesture> {
    let first = *active.points.first()?;
    Some(match active.settings.mode {
        EraserMode::Partial | EraserMode::Stroke => EraserGesture::Sweep(active.points.clone()),
        EraserMode::Area if active.settings.area_shape == EraserAreaShape::Rectangle => {
            let last = active.points.last().copied().unwrap_or(first);
            EraserGesture::Area(Rect {
                x: first.x,
                y: first.y,
                width: last.x - first.x,
                height: last.y - first.y,
            })
        }
        EraserMode::Area => EraserGesture::Lasso(active.points.clone()),
    })
}

fn removed_original_count(before: &[InkStroke], after: &[InkStroke]) -> usize {
    before
        .iter()
        .filter(|stroke| {
            !after
                .iter()
                .any(|candidate| candidate.identifier == stroke.identifier)
        })
        .count()
}

fn merge_outcome(target: &mut InputOutcome, mut incoming: InputOutcome) {
    target.finished.append(&mut incoming.finished);
    if incoming.recognized.is_some() {
        debug_assert!(target.recognized.is_none());
        target.recognized = incoming.recognized;
    }
    if incoming.replacement.is_some() {
        debug_assert!(target.replacement.is_none());
        target.replacement = incoming.replacement;
    }
    if incoming.selected_ids.is_some() {
        target.selected_ids = incoming.selected_ids;
    }
    target.redraw |= incoming.redraw;
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

fn pointer_history(
    pointer_id: u32,
    device: PointerDevice,
    monitor: MonitorRect,
    mapping: InkInputMapping,
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
                mapped_monitor_point(sample.point, monitor, mapping),
                sample.pressure,
                f64::from(sample.timestamp_ms) / 1_000.0,
            )
        })
        .collect();
    Ok(PointerHistory { points, canceled })
}

fn pointer_screen_points(
    pointer_id: u32,
    device: PointerDevice,
    monitor: MonitorRect,
    mapping: InkInputMapping,
) -> Result<Vec<Point>> {
    let size = Size {
        width: f64::from(monitor.width()),
        height: f64::from(monitor.height()),
    };
    Ok(pointer_history(pointer_id, device, monitor, mapping)?
        .points
        .into_iter()
        .map(|point| InkGeometry::screen_point(point.location, size))
        .collect())
}

fn pen_history(pointer_id: u32) -> Result<Vec<HistorySample>> {
    let mut count = 0;
    unsafe { GetPointerPenInfoHistory(pointer_id, &mut count, None) }?;
    let mut history = vec![POINTER_PEN_INFO::default(); count as usize];
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
            canceled: pen.pointerInfo.pointerFlags.contains(POINTER_FLAG_CANCELED),
        })
        .collect())
}

fn info_history(pointer_id: u32) -> Result<Vec<HistorySample>> {
    let mut count = 0;
    unsafe { GetPointerInfoHistory(pointer_id, &mut count, None) }?;
    let mut history = vec![POINTER_INFO::default(); count as usize];
    unsafe { GetPointerInfoHistory(pointer_id, &mut count, Some(history.as_mut_ptr())) }?;
    history.truncate(count as usize);
    Ok(history
        .into_iter()
        .map(|info| HistorySample {
            point: info.ptPixelLocationRaw,
            pressure: 1.0,
            timestamp_ms: info.dwTime,
            performance_count: info.PerformanceCount,
            canceled: info.pointerFlags.contains(POINTER_FLAG_CANCELED),
        })
        .collect())
}

fn history_order(first: &HistorySample, second: &HistorySample) -> Ordering {
    first
        .timestamp_ms
        .cmp(&second.timestamp_ms)
        .then(first.performance_count.cmp(&second.performance_count))
}

fn merge_points(existing: &mut Vec<InkPoint>, incoming: Vec<InkPoint>) -> bool {
    let before = existing.len();
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
            // Swift appendActivePoint: a point at the previous location is not a new point
            // (keeps the pen-up at a resting position from discarding the recognition candidate).
            if deduplicated.len() >= 2
                && deduplicated
                    .last()
                    .is_some_and(|last: &InkPoint| last.location == point.location)
            {
                continue;
            }
            deduplicated.push(point);
        }
    }
    *existing = deduplicated;
    existing.len() > before
}

fn set_stroke_endpoint(stroke: &mut InkStroke, point: InkPoint) -> bool {
    let Some(endpoint) = stroke.points.last_mut() else {
        stroke.points.push(point);
        return true;
    };
    if *endpoint == point {
        return false;
    }
    *endpoint = point;
    true
}

fn append_unique_points(existing: &mut Vec<Point>, incoming: Vec<Point>) {
    for point in incoming {
        if existing.last() != Some(&point) {
            existing.push(point);
        }
    }
}

fn normalized_pressure(pressure: u32) -> f64 {
    (f64::from(pressure) / 1_024.0).clamp(0.0, 1.0)
}

fn screen_point(point: POINT, monitor: MonitorRect) -> Point {
    Point {
        x: f64::from(point.x - monitor.left).clamp(0.0, f64::from(monitor.width())),
        y: f64::from(point.y - monitor.top).clamp(0.0, f64::from(monitor.height())),
    }
}

#[cfg(test)]
fn normalized_monitor_point(point: POINT, monitor: MonitorRect) -> Point {
    mapped_monitor_point(point, monitor, InkInputMapping::Identity)
}

fn mapped_monitor_point(point: POINT, monitor: MonitorRect, mapping: InkInputMapping) -> Point {
    InkGeometry::normalized_point(
        mapping.map_local(screen_point(point, monitor)),
        Size {
            width: f64::from(monitor.width()),
            height: f64::from(monitor.height()),
        },
    )
}

pub fn monotonic_seconds() -> Result<f64> {
    let frequency = if let Some(frequency) = QPC_FREQUENCY.get() {
        *frequency
    } else {
        let mut frequency = 0;
        unsafe { QueryPerformanceFrequency(&mut frequency) }?;
        let _ = QPC_FREQUENCY.set(frequency);
        frequency
    };
    let mut counter = 0;
    unsafe { QueryPerformanceCounter(&mut counter) }?;
    Ok(counter as f64 / frequency as f64)
}

pub fn pointer_id_from_wparam(wparam: usize) -> u32 {
    (wparam as u32) & 0xffff
}

#[cfg(test)]
mod tests {
    use super::palm_rejects_touch;
    use super::{
        cover_meets_minimum_size, history_order, laser_action, normalized_monitor_point,
        normalized_pressure, ownership_on_down, shape_endpoint, uses_eraser, HistorySample,
        LaserAction, MonitorRect, OwnershipTransition, PointerDevice, PointerPhase,
    };
    use edutools_core::ink::{ArrowStyle, InkGeometry, InkShapeKind, InkTool, Point, Size};
    use windows::Win32::Foundation::POINT;

    #[test]
    fn uses_eraser_decision_table() {
        let cases = [
            (
                "mouse-left",
                InkTool::Pen,
                PointerDevice::Mouse,
                false,
                false,
                false,
                false,
                false,
            ),
            (
                "mouse-right",
                InkTool::Pen,
                PointerDevice::Mouse,
                true,
                false,
                false,
                false,
                true,
            ),
            (
                "pen-contact",
                InkTool::Pen,
                PointerDevice::Pen,
                false,
                false,
                false,
                false,
                false,
            ),
            (
                "pen-barrel",
                InkTool::Pen,
                PointerDevice::Pen,
                false,
                true,
                false,
                false,
                true,
            ),
            (
                "pen-eraser",
                InkTool::Laser,
                PointerDevice::Pen,
                false,
                false,
                true,
                false,
                true,
            ),
            (
                "pen-inverted",
                InkTool::Laser,
                PointerDevice::Pen,
                false,
                false,
                false,
                true,
                true,
            ),
            (
                "eraser-tool",
                InkTool::Eraser,
                PointerDevice::Mouse,
                false,
                false,
                false,
                false,
                true,
            ),
        ];
        for (name, tool, device, secondary, barrel, eraser, inverted, expected) in cases {
            assert_eq!(
                uses_eraser(tool, device, secondary, barrel, eraser, inverted),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn ownership_transition_table() {
        let cases = [
            ("free-to-owner", false, false, OwnershipTransition::Accept),
            (
                "owner-ignores-other",
                true,
                true,
                OwnershipTransition::Ignore,
            ),
            (
                "missing-up-recovers",
                true,
                false,
                OwnershipTransition::RecoverAndAccept,
            ),
        ];
        for (name, has_owner, pressed, expected) in cases {
            assert_eq!(ownership_on_down(has_owner, pressed), expected, "{name}");
        }
    }

    #[test]
    fn palm_rejection_window_ignores_touch_only_shortly_after_pen() {
        assert!(!palm_rejects_touch(None, 10.0));
        assert!(palm_rejects_touch(Some(10.0), 10.0));
        assert!(palm_rejects_touch(Some(10.0), 11.0));
        assert!(!palm_rejects_touch(Some(10.0), 11.01));
    }

    #[test]
    fn laser_input_mapping_table() {
        let cases = [
            (
                "pen-hover",
                PointerPhase::Update,
                PointerDevice::Pen,
                true,
                false,
                false,
                false,
                LaserAction::Hover,
            ),
            (
                "pen-contact",
                PointerPhase::Down,
                PointerDevice::Pen,
                true,
                true,
                false,
                false,
                LaserAction::Trail,
            ),
            (
                "touch-contact",
                PointerPhase::Update,
                PointerDevice::Touch,
                true,
                true,
                false,
                false,
                LaserAction::Trail,
            ),
            (
                "mouse-left",
                PointerPhase::Down,
                PointerDevice::Mouse,
                true,
                true,
                true,
                false,
                LaserAction::Trail,
            ),
            (
                "mouse-right-not-laser",
                PointerPhase::Down,
                PointerDevice::Mouse,
                true,
                true,
                false,
                true,
                LaserAction::None,
            ),
            (
                "up",
                PointerPhase::Up,
                PointerDevice::Pen,
                true,
                false,
                false,
                false,
                LaserAction::Release,
            ),
            (
                "leave",
                PointerPhase::Leave,
                PointerDevice::Pen,
                false,
                false,
                false,
                false,
                LaserAction::Break,
            ),
            (
                "capture",
                PointerPhase::CaptureChanged,
                PointerDevice::Pen,
                true,
                true,
                false,
                false,
                LaserAction::Break,
            ),
        ];
        for (name, phase, device, range, contact, first, eraser, expected) in cases {
            assert_eq!(
                laser_action(phase, device, range, contact, first, eraser),
                expected,
                "{name}"
            );
        }
        for (name, previous, incoming, expected) in [
            ("first-pointer", None, 7, false),
            ("same-pointer", Some(7), 7, false),
            ("new-pointer", Some(7), 8, true),
        ] {
            assert_eq!(
                super::is_new_laser_pointer(previous, incoming),
                expected,
                "{name}"
            );
        }
    }

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

    #[test]
    fn shape_constraint_and_cover_minimum_table() {
        let start = Point { x: 10.0, y: 20.0 };
        let proposed = Point { x: 42.0, y: 35.0 };
        for (name, kind, constrained, expected) in [
            (
                "arrow-constrained-like-pen",
                InkShapeKind::Arrow(ArrowStyle::Single),
                true,
                InkGeometry::snapped_pen_endpoint(start, proposed),
            ),
            (
                "rectangle-constrained-square",
                InkShapeKind::Rectangle,
                true,
                InkGeometry::snapped_square_endpoint(start, proposed),
            ),
            (
                "ellipse-unconstrained",
                InkShapeKind::Ellipse,
                false,
                proposed,
            ),
        ] {
            assert_eq!(
                shape_endpoint(kind, constrained, start, proposed),
                expected,
                "{name}"
            );
        }

        let canvas = Size {
            width: 1_000.0,
            height: 800.0,
        };
        for (name, start, end, expected) in [
            (
                "both-at-minimum",
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.004, y: 0.005 },
                true,
            ),
            (
                "width-under-minimum",
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.003, y: 0.005 },
                false,
            ),
            (
                "height-under-minimum",
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.004, y: 0.004 },
                false,
            ),
        ] {
            assert_eq!(
                cover_meets_minimum_size(start, end, canvas),
                expected,
                "{name}"
            );
        }
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
