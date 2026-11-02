//! OS-independent zoom geometry and frozen-zoom coordinate mapping.

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

// Swift: ZoomFollowMode
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomFollowMode {
    Always,
    Edge,
    Fixed,
}

impl ZoomFollowMode {
    // Swift: ZoomFollowMode.next
    pub const fn next(self) -> Self {
        match self {
            Self::Always => Self::Edge,
            Self::Edge => Self::Fixed,
            Self::Fixed => Self::Always,
        }
    }
}

// Swift: ZoomDefaults
pub struct ZoomDefaults;

impl ZoomDefaults {
    pub const MINIMUM_MAGNIFICATION: f64 = 1.25;
    pub const MAXIMUM_MAGNIFICATION: f64 = 8.0;
    pub const MAGNIFICATION_STEP: f64 = 0.25;
    pub const STARTING_MAGNIFICATION: f64 = 2.0;
    pub const FOLLOW_EDGE_INSET_RATIO: f64 = 0.1;
    pub const FOLLOW_SMOOTHNESS: f64 = 0.22;
    pub const MINIMUM_FOLLOW_SMOOTHNESS: f64 = 0.0;
    pub const MAXIMUM_FOLLOW_SMOOTHNESS: f64 = 1.0;
}

// Swift: ZoomSessionState
#[derive(Clone, Debug, PartialEq)]
pub struct ZoomSessionState {
    pub display_id: String,
    pub screen_size: Size,
    pub magnification: f64,
    pub starting_magnification: f64,
    pub center: Point,
    pub follow_mode: ZoomFollowMode,
    pub follow_smoothness: f64,
    pub preview_dimming: f64,
    pub is_frozen: bool,
    pub is_previewing: bool,
}

impl ZoomSessionState {
    // Swift: ZoomSessionState.sourceRect
    pub fn source_rect(&self) -> Rect {
        ZoomGeometry::source_rect(self.center, self.screen_size, self.magnification)
    }
}

// Swift: CGAffineTransform
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineTransform {
    pub scale_x: f64,
    pub scale_y: f64,
    pub translate_x: f64,
    pub translate_y: f64,
}

impl AffineTransform {
    // Swift: CGPoint.applying(_:)
    pub fn apply(self, point: Point) -> Point {
        Point::new(
            point.x * self.scale_x + self.translate_x,
            point.y * self.scale_y + self.translate_y,
        )
    }
}

// Swift: ZoomGeometry
pub struct ZoomGeometry;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomInkInputMode {
    Identity,
    ThroughPresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomGestureControl {
    FollowMovement,
    Magnification,
    PreviewKey,
    FreezeKey,
    FollowKey,
}

// Swift: InkView.localLocation(for:), ZoomController.isInkGestureActive()
pub const fn zoom_gesture_control_locked(
    gesture_in_progress: bool,
    _control: ZoomGestureControl,
) -> bool {
    gesture_in_progress
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomControlInput {
    IncreaseKey,
    DecreaseKey,
    ResetKey,
    Wheel(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomControlEffect {
    Adjust(i32),
    Reset,
}

// Swift: ZoomController.adjustMagnification/resetMagnification/handleScroll
pub const fn zoom_control_effect(input: ZoomControlInput) -> Option<ZoomControlEffect> {
    match input {
        ZoomControlInput::IncreaseKey => Some(ZoomControlEffect::Adjust(1)),
        ZoomControlInput::DecreaseKey => Some(ZoomControlEffect::Adjust(-1)),
        ZoomControlInput::ResetKey => Some(ZoomControlEffect::Reset),
        ZoomControlInput::Wheel(0) => None,
        ZoomControlInput::Wheel(steps) => Some(ZoomControlEffect::Adjust(steps)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomStopAction {
    CancelPreview,
    StopZoom,
}

// Swift: ZoomController.stopOrCancelPreview()
pub const fn stop_or_cancel_preview_action(is_previewing: bool) -> ZoomStopAction {
    if is_previewing {
        ZoomStopAction::CancelPreview
    } else {
        ZoomStopAction::StopZoom
    }
}

impl ZoomGeometry {
    // Swift: ZoomGeometry.magnificationLabel(_:)
    pub fn magnification_label(magnification: f64) -> String {
        let quarter_steps = (magnification * 4.0).round() as i64;
        if quarter_steps % 2 == 0 {
            format!("×{magnification:.1}")
        } else {
            format!("×{magnification:.2}")
        }
    }

    // Swift: ZoomGeometry.clampedMagnification(_:)
    pub fn clamped_magnification(value: f64) -> f64 {
        value.clamp(
            ZoomDefaults::MINIMUM_MAGNIFICATION,
            ZoomDefaults::MAXIMUM_MAGNIFICATION,
        )
    }

    // Swift: ZoomGeometry.steppedMagnification(_:)
    pub fn stepped_magnification(value: f64) -> f64 {
        let rounded =
            (value / ZoomDefaults::MAGNIFICATION_STEP).round() * ZoomDefaults::MAGNIFICATION_STEP;
        Self::clamped_magnification(rounded)
    }

    // Swift: ZoomGeometry.adjustedMagnification(_:by:)
    pub fn adjusted_magnification(value: f64, steps: i32) -> f64 {
        Self::stepped_magnification(value + f64::from(steps) * ZoomDefaults::MAGNIFICATION_STEP)
    }

    // Swift: ZoomGeometry.viewportSize(screenSize:magnification:)
    pub fn viewport_size(screen_size: Size, magnification: f64) -> Size {
        let clamped = Self::clamped_magnification(magnification);
        Size::new(screen_size.width / clamped, screen_size.height / clamped)
    }

    // Swift: ZoomGeometry.clampedCenter(_:screenSize:magnification:)
    pub fn clamped_center(center: Point, screen_size: Size, magnification: f64) -> Point {
        let viewport = Self::viewport_size(screen_size, magnification);
        let half_width = viewport.width / 2.0;
        let half_height = viewport.height / 2.0;
        Point::new(
            center.x.clamp(half_width, screen_size.width - half_width),
            center
                .y
                .clamp(half_height, screen_size.height - half_height),
        )
    }

    // Swift: ZoomGeometry.sourceRect(center:screenSize:magnification:)
    pub fn source_rect(center: Point, screen_size: Size, magnification: f64) -> Rect {
        let viewport = Self::viewport_size(screen_size, magnification);
        let clamped = Self::clamped_center(center, screen_size, magnification);
        Rect::new(
            clamped.x - viewport.width / 2.0,
            clamped.y - viewport.height / 2.0,
            viewport.width,
            viewport.height,
        )
    }

    // Swift: ZoomGeometry.followedCenter(current:pointer:screenSize:magnification:mode:smoothness:)
    pub fn followed_center(
        current: Point,
        pointer: Point,
        screen_size: Size,
        magnification: f64,
        mode: ZoomFollowMode,
        smoothness: f64,
    ) -> Point {
        let current = Self::clamped_center(current, screen_size, magnification);
        let target = match mode {
            ZoomFollowMode::Always => pointer,
            ZoomFollowMode::Fixed => current,
            ZoomFollowMode::Edge => {
                let viewport = Self::source_rect(current, screen_size, magnification);
                let inset_x = viewport.width * ZoomDefaults::FOLLOW_EDGE_INSET_RATIO;
                let inset_y = viewport.height * ZoomDefaults::FOLLOW_EDGE_INSET_RATIO;
                let minimum_x = viewport.x + inset_x;
                let maximum_x = viewport.x + viewport.width - inset_x;
                let minimum_y = viewport.y + inset_y;
                let maximum_y = viewport.y + viewport.height - inset_y;
                let mut pushed = current;
                if pointer.x < minimum_x {
                    pushed.x += pointer.x - minimum_x;
                } else if pointer.x > maximum_x {
                    pushed.x += pointer.x - maximum_x;
                }
                if pointer.y < minimum_y {
                    pushed.y += pointer.y - minimum_y;
                } else if pointer.y > maximum_y {
                    pushed.y += pointer.y - maximum_y;
                }
                pushed
            }
        };

        let amount = smoothness.clamp(
            ZoomDefaults::MINIMUM_FOLLOW_SMOOTHNESS,
            ZoomDefaults::MAXIMUM_FOLLOW_SMOOTHNESS,
        );
        Self::clamped_center(
            Point::new(
                current.x + (target.x - current.x) * amount,
                current.y + (target.y - current.y) * amount,
            ),
            screen_size,
            magnification,
        )
    }

    // Swift: ZoomGeometry.originalPoint(fromZoomed:in:sourceRect:)
    pub fn original_point(point: Point, zoomed_size: Size, source_rect: Rect) -> Point {
        if zoomed_size.width <= 0.0 || zoomed_size.height <= 0.0 {
            return Point::new(source_rect.x, source_rect.y);
        }
        Point::new(
            source_rect.x + point.x / zoomed_size.width * source_rect.width,
            source_rect.y + point.y / zoomed_size.height * source_rect.height,
        )
    }

    // Swift: InkView.localLocation(for:)
    pub fn ink_input_point(
        screen_point: Point,
        monitor_rect: Rect,
        zoomed_size: Size,
        source_rect: Rect,
        mode: ZoomInkInputMode,
    ) -> Point {
        let view_point = Point::new(
            screen_point.x - monitor_rect.x,
            screen_point.y - monitor_rect.y,
        );
        match mode {
            ZoomInkInputMode::Identity => view_point,
            ZoomInkInputMode::ThroughPresentation => {
                Self::original_point(view_point, zoomed_size, source_rect)
            }
        }
    }

    // Swift: ZoomController.warpPointer(toViewPoint:)
    pub fn warp_screen_point(view_point: Point, monitor_rect: Rect) -> Option<Point> {
        let inside = view_point.x >= 0.0
            && view_point.y >= 0.0
            && view_point.x < monitor_rect.width
            && view_point.y < monitor_rect.height;
        inside.then(|| Point::new(monitor_rect.x + view_point.x, monitor_rect.y + view_point.y))
    }

    // Swift: ZoomGeometry.presentationTransform(sourceRect:destinationSize:)
    pub fn presentation_transform(source_rect: Rect, destination_size: Size) -> AffineTransform {
        if source_rect.width <= 0.0 || source_rect.height <= 0.0 {
            return AffineTransform {
                scale_x: 1.0,
                scale_y: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
            };
        }
        let scale_x = destination_size.width / source_rect.width;
        let scale_y = destination_size.height / source_rect.height;
        AffineTransform {
            scale_x,
            scale_y,
            translate_x: -source_rect.x * scale_x,
            translate_y: -source_rect.y * scale_y,
        }
    }

    // Swift: ZoomGeometry.pointerPresentationPoint(_:for:)
    pub fn pointer_presentation_point(source_point: Point, state: &ZoomSessionState) -> Point {
        if state.is_frozen || state.is_previewing {
            return source_point;
        }
        Self::presentation_transform(state.source_rect(), state.screen_size).apply(source_point)
    }

    // Swift: ZoomGeometry.pointerScale(for:)
    pub fn pointer_scale(state: &ZoomSessionState) -> f64 {
        if state.is_previewing {
            1.0
        } else {
            state.magnification
        }
    }

    /// Windows 전용(macOS 에는 터치 화면이 없다): 두 손가락이 확대 화면에서 `view_delta`
    /// 만큼 움직이면 손가락 아래 내용이 함께 끌려오도록 중심을 반대 방향으로 옮긴다.
    /// 확대 화면 1px 은 원본 1/배율 px 이다.
    pub fn panned_center(
        center: Point,
        view_delta: Point,
        screen_size: Size,
        magnification: f64,
    ) -> Point {
        let current = Self::clamped_center(center, screen_size, magnification);
        let scale = Self::clamped_magnification(magnification);
        Self::clamped_center(
            Point::new(
                current.x - view_delta.x / scale,
                current.y - view_delta.y / scale,
            ),
            screen_size,
            magnification,
        )
    }
}

/// 줌 창 터치 입력의 처리 방향.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoomTouchPanStep {
    /// 화면 이동과 무관 — 평소처럼 판서로 넘긴다.
    Forward,
    /// 두 번째 손가락이 닿아 화면 이동이 시작됐다. 첫 손가락이 시작한 판서는 버린다.
    Began,
    /// 두 손가락 가운데점이 확대 화면 좌표로 이만큼 움직였다.
    Pan(Point),
    /// 화면 이동 중인 입력 — 판서로 넘기지 않는다.
    Consumed,
    /// 화면 이동 뒤 마지막 손가락이 떨어졌다.
    Ended,
}

/// Windows 전용 두 손가락 화면 이동 추적기. 한 번 화면 이동이 시작되면 모든 손가락이
/// 떨어질 때까지 터치 입력을 판서로 넘기지 않는다(한 손가락만 떼도 남은 손가락이
/// 획을 긋지 않게).
#[derive(Clone, Debug, Default)]
pub struct ZoomTouchPan {
    contacts: Vec<(u32, Point)>,
    panning: bool,
}

impl ZoomTouchPan {
    pub fn is_panning(&self) -> bool {
        self.panning
    }

    pub fn tracks(&self, pointer_id: u32) -> bool {
        self.contacts.iter().any(|(id, _)| *id == pointer_id)
    }

    pub fn contact_down(&mut self, pointer_id: u32, point: Point) -> ZoomTouchPanStep {
        if let Some(contact) = self.contacts.iter_mut().find(|(id, _)| *id == pointer_id) {
            contact.1 = point;
        } else {
            self.contacts.push((pointer_id, point));
        }
        if self.panning {
            ZoomTouchPanStep::Consumed
        } else if self.contacts.len() >= 2 {
            self.panning = true;
            ZoomTouchPanStep::Began
        } else {
            ZoomTouchPanStep::Forward
        }
    }

    pub fn contact_moved(&mut self, pointer_id: u32, point: Point) -> ZoomTouchPanStep {
        let before = self.midpoint();
        let Some(contact) = self.contacts.iter_mut().find(|(id, _)| *id == pointer_id) else {
            return if self.panning {
                ZoomTouchPanStep::Consumed
            } else {
                ZoomTouchPanStep::Forward
            };
        };
        contact.1 = point;
        if !self.panning {
            return ZoomTouchPanStep::Forward;
        }
        match (before, self.midpoint()) {
            (Some(before), Some(after)) if before != after => {
                ZoomTouchPanStep::Pan(Point::new(after.x - before.x, after.y - before.y))
            }
            _ => ZoomTouchPanStep::Consumed,
        }
    }

    pub fn contact_up(&mut self, pointer_id: u32) -> ZoomTouchPanStep {
        self.contacts.retain(|(id, _)| *id != pointer_id);
        if !self.panning {
            ZoomTouchPanStep::Forward
        } else if self.contacts.is_empty() {
            self.panning = false;
            ZoomTouchPanStep::Ended
        } else {
            ZoomTouchPanStep::Consumed
        }
    }

    /// 먼저 닿은 두 손가락의 가운데점. 셋째 손가락부터는 이동량에 쓰지 않는다.
    fn midpoint(&self) -> Option<Point> {
        match self.contacts.as_slice() {
            [(_, a), (_, b), ..] => Some(Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        stop_or_cancel_preview_action, zoom_control_effect, zoom_gesture_control_locked, Point,
        Rect, Size, ZoomControlEffect, ZoomControlInput, ZoomGeometry, ZoomGestureControl,
        ZoomInkInputMode, ZoomStopAction, ZoomTouchPan, ZoomTouchPanStep,
    };

    #[test]
    fn panned_center_drags_content_with_fingers_and_clamps() {
        let screen = Size::new(1_920.0, 1_080.0);
        // 2배: 손가락이 오른쪽 아래로 (200,100) 움직이면 원본에서 (100,50) 만큼 왼쪽 위를 본다.
        assert_eq!(
            ZoomGeometry::panned_center(
                Point::new(960.0, 540.0),
                Point::new(200.0, 100.0),
                screen,
                2.0
            ),
            Point::new(860.0, 490.0)
        );
        // 가장자리를 넘으면 원본 화면 안으로 고정된다(2배 뷰포트 960x540 → 중심 최소 480,270).
        assert_eq!(
            ZoomGeometry::panned_center(
                Point::new(500.0, 300.0),
                Point::new(1_000.0, 1_000.0),
                screen,
                2.0
            ),
            Point::new(480.0, 270.0)
        );
    }

    #[test]
    fn touch_pan_single_finger_is_forwarded_to_ink() {
        let mut pan = ZoomTouchPan::default();
        assert_eq!(
            pan.contact_down(1, Point::new(10.0, 10.0)),
            ZoomTouchPanStep::Forward
        );
        assert_eq!(
            pan.contact_moved(1, Point::new(30.0, 10.0)),
            ZoomTouchPanStep::Forward
        );
        assert_eq!(pan.contact_up(1), ZoomTouchPanStep::Forward);
        assert!(!pan.is_panning());
    }

    #[test]
    fn touch_pan_two_fingers_pan_by_midpoint_until_all_lift() {
        let mut pan = ZoomTouchPan::default();
        assert_eq!(
            pan.contact_down(1, Point::new(100.0, 100.0)),
            ZoomTouchPanStep::Forward
        );
        assert_eq!(
            pan.contact_down(2, Point::new(200.0, 100.0)),
            ZoomTouchPanStep::Began
        );
        // 한 손가락만 20 움직이면 가운데점은 10 움직인다.
        assert_eq!(
            pan.contact_moved(1, Point::new(120.0, 100.0)),
            ZoomTouchPanStep::Pan(Point::new(10.0, 0.0))
        );
        assert_eq!(
            pan.contact_moved(2, Point::new(220.0, 140.0)),
            ZoomTouchPanStep::Pan(Point::new(10.0, 20.0))
        );
        // 제자리 갱신은 이동 없음.
        assert_eq!(
            pan.contact_moved(2, Point::new(220.0, 140.0)),
            ZoomTouchPanStep::Consumed
        );
        // 한 손가락을 떼도 남은 손가락은 판서로 가지 않는다.
        assert_eq!(pan.contact_up(1), ZoomTouchPanStep::Consumed);
        assert_eq!(
            pan.contact_moved(2, Point::new(260.0, 140.0)),
            ZoomTouchPanStep::Consumed
        );
        assert_eq!(
            pan.contact_down(3, Point::new(50.0, 50.0)),
            ZoomTouchPanStep::Consumed
        );
        assert_eq!(pan.contact_up(2), ZoomTouchPanStep::Consumed);
        assert_eq!(pan.contact_up(3), ZoomTouchPanStep::Ended);
        assert!(!pan.is_panning());
        // 끝난 뒤 새 한 손가락은 다시 판서.
        assert_eq!(
            pan.contact_down(4, Point::new(10.0, 10.0)),
            ZoomTouchPanStep::Forward
        );
    }

    #[test]
    fn zoom_ink_input_mapping_table() {
        let monitor = Rect::new(-1_920.0, -120.0, 1_920.0, 1_080.0);
        let zoomed_size = Size::new(1_920.0, 1_080.0);
        let source = Rect::new(480.0, 270.0, 960.0, 540.0);
        let cases = [
            (
                ZoomInkInputMode::Identity,
                Point::new(-1_440.0, 150.0),
                Point::new(480.0, 270.0),
            ),
            (
                ZoomInkInputMode::ThroughPresentation,
                Point::new(-960.0, 420.0),
                Point::new(960.0, 540.0),
            ),
        ];
        for (mode, screen, expected) in cases {
            assert_eq!(
                ZoomGeometry::ink_input_point(screen, monitor, zoomed_size, source, mode),
                expected,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn zoom_gesture_lock_table() {
        let controls = [
            ZoomGestureControl::FollowMovement,
            ZoomGestureControl::Magnification,
            ZoomGestureControl::PreviewKey,
            ZoomGestureControl::FreezeKey,
            ZoomGestureControl::FollowKey,
        ];
        for gesture in [false, true] {
            for control in controls {
                assert_eq!(
                    zoom_gesture_control_locked(gesture, control),
                    gesture,
                    "gesture={gesture} control={control:?}"
                );
            }
        }
    }

    #[test]
    fn warp_target_table() {
        let monitor = Rect::new(-1_920.0, -120.0, 1_920.0, 1_080.0);
        let cases = [
            (Point::new(0.0, 0.0), Some(Point::new(-1_920.0, -120.0))),
            (Point::new(1_919.0, 1_079.0), Some(Point::new(-1.0, 959.0))),
            (Point::new(-1.0, 10.0), None),
            (Point::new(1_920.0, 10.0), None),
        ];
        for (view, expected) in cases {
            assert_eq!(ZoomGeometry::warp_screen_point(view, monitor), expected);
        }
    }

    #[test]
    fn zoom_key_and_wheel_effect_table() {
        let cases = [
            (
                ZoomControlInput::IncreaseKey,
                Some(ZoomControlEffect::Adjust(1)),
            ),
            (
                ZoomControlInput::DecreaseKey,
                Some(ZoomControlEffect::Adjust(-1)),
            ),
            (ZoomControlInput::ResetKey, Some(ZoomControlEffect::Reset)),
            (
                ZoomControlInput::Wheel(3),
                Some(ZoomControlEffect::Adjust(3)),
            ),
            (
                ZoomControlInput::Wheel(-2),
                Some(ZoomControlEffect::Adjust(-2)),
            ),
            (ZoomControlInput::Wheel(0), None),
        ];
        for (input, expected) in cases {
            assert_eq!(zoom_control_effect(input), expected, "{input:?}");
        }
    }

    #[test]
    fn preview_freeze_and_escape_priority_table() {
        let cases = [
            (false, false, ZoomStopAction::StopZoom),
            (true, false, ZoomStopAction::CancelPreview),
            (true, true, ZoomStopAction::CancelPreview),
            (false, true, ZoomStopAction::StopZoom),
        ];
        for (previewing, _frozen, expected) in cases {
            assert_eq!(stop_or_cancel_preview_action(previewing), expected);
        }

        let first = stop_or_cancel_preview_action(true);
        let second = stop_or_cancel_preview_action(false);
        assert_eq!(first, ZoomStopAction::CancelPreview);
        assert_eq!(second, ZoomStopAction::StopZoom);
    }
}
