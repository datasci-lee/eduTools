use crate::ink_view::{pointer_id_from_wparam, InkInputConfig, InkInputController, InputOutcome};
use crate::log::Logger;
use crate::monitor::{
    enumerate_monitors, monitor_at_point, monitor_by_key, plan_monitor_sync, Monitor,
};
use crate::render::{GraphicsDevice, OverlayRenderer};
use edutools_core::ink::{
    InkCommand, InkLayerState, InkPoint, InkStroke, InkStrokeContent, InkStrokeTool, Point,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_void, OsString};
use std::rc::Rc;
use windows::core::{w, Error, Result};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, ERROR_SUCCESS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT,
    RECT, WIN32_ERROR, WPARAM,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetWindowLongPtrW, GetWindowRect,
    KillTimer, RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, CREATESTRUCTW, GWLP_USERDATA, GWL_EXSTYLE, HWND_TOPMOST, LWA_ALPHA,
    MA_NOACTIVATE, PA_NOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_MOUSEACTIVATE, WM_NCCREATE, WM_NCDESTROY,
    WM_POINTERACTIVATE, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERLEAVE, WM_POINTERUP,
    WM_POINTERUPDATE, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

pub const SYNC_MESSAGE: u32 = 0x8000 + 2;
const OVERLAY_CLASS: windows::core::PCWSTR = w!("EduToolsOverlay");
const LASER_TIMER_ID: usize = 0xED11;
const LASER_TIMER_INTERVAL_MS: u32 = 16;

type LayerStore = Rc<RefCell<HashMap<String, InkLayerState>>>;

pub struct OverlayManager {
    overlays: HashMap<String, OverlayWindow>,
    layers: LayerStore,
    graphics: GraphicsDevice,
    owner: HWND,
    hinstance: HINSTANCE,
    annotation_enabled: bool,
    input_config: InkInputConfig,
    debug_overlay: bool,
    logger: Logger,
}

impl OverlayManager {
    pub fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        input_config: InkInputConfig,
        logger: Logger,
    ) -> Result<Self> {
        register_window_class(hinstance)?;
        let graphics = GraphicsDevice::new()?;
        let debug_overlay = std::env::var_os("EDUTOOLS_DEBUG_OVERLAY") == Some(OsString::from("1"));
        if debug_overlay {
            logger
                .event("EDUTOOLS_DEBUG_OVERLAY=1: 개발 확인용 틴트/테두리 활성화 (제품 기능 아님)");
        }
        let mut manager = Self {
            overlays: HashMap::new(),
            layers: Rc::new(RefCell::new(HashMap::new())),
            graphics,
            owner,
            hinstance,
            annotation_enabled: false,
            input_config,
            debug_overlay,
            logger,
        };
        manager.synchronize("시작")?;
        Ok(manager)
    }

    pub fn synchronize(&mut self, reason: &str) -> Result<()> {
        // EnumDisplayMonitors can include invisible pseudo monitors from mirroring drivers. W10's
        // CCD active-source comparison will filter those; this round intentionally keeps them.
        let monitors = enumerate_monitors()?;
        let current = self
            .overlays
            .values()
            .map(|overlay| overlay.monitor().snapshot())
            .collect::<Vec<_>>();
        let discovered = monitors.iter().map(Monitor::snapshot).collect::<Vec<_>>();
        let plan = plan_monitor_sync(&current, &discovered);

        for key in &plan.removed {
            if let Some(mut overlay) = self.overlays.remove(key) {
                overlay.finish_active("모니터 제거")?;
            }
        }

        for monitor in &monitors {
            debug_assert!(monitor_by_key(&monitors, &monitor.session_key).is_some());
            if let Some(overlay) = self.overlays.get_mut(&monitor.session_key) {
                if overlay.monitor() != monitor {
                    overlay.update_monitor(
                        monitor.clone(),
                        self.annotation_enabled,
                        self.debug_overlay,
                    )?;
                }
                continue;
            }
            self.layers
                .borrow_mut()
                .entry(monitor.session_key.clone())
                .or_default();
            let overlay = OverlayWindow::new(
                self.owner,
                self.hinstance,
                monitor.clone(),
                &self.graphics,
                self.layers.clone(),
                self.annotation_enabled,
                self.input_config.clone(),
                self.debug_overlay,
                self.logger.clone(),
            )?;
            self.overlays.insert(monitor.session_key.clone(), overlay);
        }

        self.logger.event(&format!(
            "오버레이 모니터 동기화 ({reason}): 추가={} 제거={} 이동={} 유지={} 총={}, 보관 레이어={}",
            plan.added.len(),
            plan.removed.len(),
            plan.moved.len(),
            plan.unchanged.len(),
            self.overlays.len(),
            self.layers.borrow().len()
        ));
        Ok(())
    }

    pub fn set_annotation_mode(
        &mut self,
        enabled: bool,
        input_config: InkInputConfig,
    ) -> Result<()> {
        self.annotation_enabled = enabled;
        self.input_config = input_config.clone();
        for overlay in self.overlays.values_mut() {
            overlay.update_input_context(enabled, input_config.clone())?;
            overlay.set_click_through(!enabled)?;
            overlay.draw(enabled && self.debug_overlay)?;
            let _ = unsafe { ShowWindow(overlay.hwnd(), SW_SHOWNOACTIVATE) };
        }
        Ok(())
    }

    pub fn update_input_context(&mut self, input_config: InkInputConfig) -> Result<()> {
        self.input_config = input_config.clone();
        for overlay in self.overlays.values_mut() {
            overlay.update_input_context(self.annotation_enabled, input_config.clone())?;
        }
        Ok(())
    }

    pub fn monitor_key_at_cursor(&self) -> Result<Option<String>> {
        let mut point = windows::Win32::Foundation::POINT::default();
        // SAFETY: point is writable storage and this call does not retain it.
        unsafe { GetCursorPos(&mut point) }?;
        let monitors = self
            .overlays
            .values()
            .map(|overlay| overlay.monitor().clone())
            .collect::<Vec<_>>();
        Ok(monitor_at_point(&monitors, point).map(|monitor| monitor.session_key.clone()))
    }

    pub fn undo(&mut self, monitor_key: &str) -> Result<bool> {
        let changed = self
            .layers
            .borrow_mut()
            .get_mut(monitor_key)
            .is_some_and(InkLayerState::undo);
        if changed {
            self.rebuild_monitor(monitor_key)?;
        }
        Ok(changed)
    }

    pub fn redo(&mut self, monitor_key: &str) -> Result<bool> {
        let changed = self
            .layers
            .borrow_mut()
            .get_mut(monitor_key)
            .is_some_and(InkLayerState::redo);
        if changed {
            self.rebuild_monitor(monitor_key)?;
        }
        Ok(changed)
    }

    pub fn clear_all(&mut self) -> Result<usize> {
        let changed_keys = {
            let mut layers = self.layers.borrow_mut();
            layers
                .iter_mut()
                .filter_map(|(key, layer)| {
                    if layer.strokes().is_empty() {
                        return None;
                    }
                    let before = layer.strokes().to_vec();
                    layer.execute(InkCommand::ReplaceStrokes {
                        before,
                        after: Vec::new(),
                    });
                    Some(key.clone())
                })
                .collect::<Vec<_>>()
        };
        for key in &changed_keys {
            self.rebuild_monitor(key)?;
        }
        Ok(changed_keys.len())
    }

    pub fn set_pass_through(&mut self, enabled: bool) -> Result<()> {
        for overlay in self.overlays.values_mut() {
            overlay.set_click_through(enabled || !self.annotation_enabled)?;
        }
        Ok(())
    }

    pub fn finish_all_active(&mut self, reason: &str) -> Result<()> {
        for overlay in self.overlays.values_mut() {
            overlay.finish_active(reason)?;
        }
        Ok(())
    }

    fn rebuild_monitor(&mut self, monitor_key: &str) -> Result<()> {
        if let Some(overlay) = self.overlays.get_mut(monitor_key) {
            overlay.rebuild_completed()?;
            overlay.draw(self.annotation_enabled && self.debug_overlay)?;
        }
        Ok(())
    }

    pub fn selftest_failures(&mut self) -> Vec<String> {
        let mut failures = Vec::new();
        if self.overlays.is_empty() {
            failures.push("overlay: 열거된 모니터/생성된 창이 없음".to_owned());
            return failures;
        }
        let required =
            (WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE).0
                as isize;
        for overlay in self.overlays.values_mut() {
            let key = overlay.monitor().session_key.clone();
            let style = unsafe { GetWindowLongPtrW(overlay.hwnd(), GWL_EXSTYLE) };
            if style & required != required {
                failures.push(format!("overlay {key}: 필수 확장 스타일 누락 ({style:#x})"));
            }

            let mut actual = RECT::default();
            let window_size = match unsafe { GetWindowRect(overlay.hwnd(), &mut actual) } {
                Ok(()) => {
                    if actual != overlay.monitor().win32_rect() {
                        failures.push(format!(
                            "overlay {key}: GetWindowRect {:?} != rcMonitor {:?}",
                            actual,
                            overlay.monitor().win32_rect()
                        ));
                    }
                    Some((actual.right - actual.left, actual.bottom - actual.top))
                }
                Err(error) => {
                    failures.push(format!("overlay {key}: GetWindowRect 실패: {error}"));
                    None
                }
            };
            if let Some((window_width, window_height)) = window_size {
                match overlay.swap_chain_size() {
                    Ok((swap_width, swap_height))
                        if i64::from(swap_width) == i64::from(window_width)
                            && i64::from(swap_height) == i64::from(window_height) => {}
                    Ok((swap_width, swap_height)) => failures.push(format!(
                        "overlay {key}: 스왑 체인 버퍼 {swap_width}x{swap_height} != 창 {window_width}x{window_height}"
                    )),
                    Err(error) => failures.push(format!(
                        "overlay {key}: 스왑 체인 버퍼 크기 조회 실패: {error}"
                    )),
                }
            }

            match overlay.selftest_render_path() {
                Ok(pixel_failures) => failures.extend(
                    pixel_failures
                        .into_iter()
                        .map(|failure| format!("overlay {key}: {failure}")),
                ),
                Err(error) => {
                    failures.push(format!("overlay {key}: 가짜 획/픽셀 렌더 실패: {error}"))
                }
            }
            match overlay.set_click_through(true) {
                Ok(()) => {
                    let enabled = unsafe { GetWindowLongPtrW(overlay.hwnd(), GWL_EXSTYLE) };
                    if enabled & WS_EX_TRANSPARENT.0 as isize == 0 {
                        failures.push(format!("overlay {key}: click-through 켜기 실패"));
                    }
                }
                Err(error) => failures.push(format!(
                    "overlay {key}: set_click_through(true) 실패: {error}"
                )),
            }
            match overlay.set_click_through(false) {
                Ok(()) => {
                    let disabled = unsafe { GetWindowLongPtrW(overlay.hwnd(), GWL_EXSTYLE) };
                    if disabled & WS_EX_TRANSPARENT.0 as isize != 0 {
                        failures.push(format!("overlay {key}: click-through 끄기 실패"));
                    }
                }
                Err(error) => failures.push(format!(
                    "overlay {key}: set_click_through(false) 실패: {error}"
                )),
            }
            if let Err(error) = overlay.set_click_through(true) {
                failures.push(format!("overlay {key}: click-through 복원 실패: {error}"));
            }
        }
        failures
    }
}

fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
    let class = WNDCLASSW {
        lpfnWndProc: Some(overlay_window_proc),
        hInstance: hinstance,
        lpszClassName: OVERLAY_CLASS,
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        let error = unsafe { windows::Win32::Foundation::GetLastError() };
        if error != ERROR_CLASS_ALREADY_EXISTS {
            return Err(Error::from_thread());
        }
    }
    Ok(())
}

struct OverlayWindowState {
    owner: HWND,
    monitor: Monitor,
    logger: Logger,
    layers: LayerStore,
    input: InkInputController,
    renderer: Option<OverlayRenderer>,
    debug_visible: bool,
    laser_timer_running: bool,
}

impl OverlayWindowState {
    fn apply_input_outcome(&mut self, outcome: InputOutcome) -> Result<()> {
        let minimum_ratio = self.input.pressure_minimum_width_ratio();
        let highlighter_opacity = self.input.highlighter_opacity();
        for stroke in outcome.finished {
            self.layers
                .borrow_mut()
                .entry(self.monitor.session_key.clone())
                .or_default()
                .execute(InkCommand::AddStroke(stroke.clone()));
            self.renderer
                .as_ref()
                .expect("renderer initialized after window creation")
                .append_completed(&stroke, minimum_ratio, highlighter_opacity)?;
        }
        if let Some(replacement) = outcome.replacement {
            self.logger.event(&format!(
                "지우개 replaceStrokes 적용: monitor={} erased={} Undo=1",
                self.monitor.session_key, replacement.erased_strokes
            ));
            self.layers
                .borrow_mut()
                .entry(self.monitor.session_key.clone())
                .or_default()
                .execute(InkCommand::ReplaceStrokes {
                    before: replacement.before,
                    after: replacement.after,
                });
            let strokes = self.current_strokes();
            self.renderer
                .as_ref()
                .expect("renderer initialized after window creation")
                .rebuild_completed(&strokes, minimum_ratio, highlighter_opacity)?;
        }
        if outcome.redraw {
            self.draw()?;
        }
        Ok(())
    }

    fn draw(&self) -> Result<()> {
        let eraser = self.input.eraser_render_state();
        let laser = self.input.laser_render_state();
        self.renderer
            .as_ref()
            .expect("renderer initialized after window creation")
            .draw_frame(
                self.input.active_stroke(),
                eraser.as_ref(),
                laser.as_ref(),
                self.input.pressure_minimum_width_ratio(),
                self.input.highlighter_opacity(),
                self.debug_visible,
            )
    }

    fn finish_active(&mut self, reason: &str) -> Result<()> {
        let outcome = self.input.finish_active(reason);
        self.apply_input_outcome(outcome)
    }

    fn current_strokes(&self) -> Vec<InkStroke> {
        self.layers
            .borrow()
            .get(&self.monitor.session_key)
            .map(|layer| layer.strokes().to_vec())
            .unwrap_or_default()
    }

    fn sync_laser_timer(&mut self, hwnd: HWND) -> Result<()> {
        let needed = self.input.laser_timer_needed();
        match (self.laser_timer_running, needed) {
            (false, true) => {
                let timer =
                    unsafe { SetTimer(Some(hwnd), LASER_TIMER_ID, LASER_TIMER_INTERVAL_MS, None) };
                if timer == 0 {
                    return Err(Error::new(E_FAIL, "레이저 줄이기 타이머 시작 실패"));
                }
                self.laser_timer_running = true;
            }
            (true, false) => {
                unsafe { KillTimer(Some(hwnd), LASER_TIMER_ID) }?;
                self.laser_timer_running = false;
            }
            _ => {}
        }
        Ok(())
    }
}

struct OverlayWindow {
    hwnd: HWND,
    state: Box<OverlayWindowState>,
}

impl OverlayWindow {
    #[allow(clippy::too_many_arguments)]
    fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        monitor: Monitor,
        graphics: &GraphicsDevice,
        layers: LayerStore,
        annotation_enabled: bool,
        input_config: InkInputConfig,
        debug_overlay: bool,
        logger: Logger,
    ) -> Result<Self> {
        let rect = monitor.rect;
        let mut state = Box::new(OverlayWindowState {
            owner,
            input: InkInputController::new(
                monitor.session_key.clone(),
                annotation_enabled,
                input_config,
                logger.clone(),
            ),
            monitor,
            logger,
            layers,
            renderer: None,
            debug_visible: annotation_enabled && debug_overlay,
            laser_timer_running: false,
        });
        let state_ptr = state.as_mut() as *mut OverlayWindowState;
        let ex_style =
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP;
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                OVERLAY_CLASS,
                w!("EduTools Overlay"),
                WS_POPUP,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                None,
                None,
                Some(hinstance),
                Some(state_ptr.cast::<c_void>()),
            )
        }?;
        if let Err(error) = unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        } {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }

        let renderer = match OverlayRenderer::new(
            graphics,
            hwnd,
            rect.width(),
            rect.height(),
            state.monitor.dpi.0,
            state.monitor.session_key.clone(),
            state.logger.clone(),
        ) {
            Ok(renderer) => renderer,
            Err(error) => {
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
        };
        state.renderer = Some(renderer);
        let mut window = Self { hwnd, state };
        window.rebuild_completed()?;
        window.set_click_through(!annotation_enabled)?;
        window.draw(annotation_enabled && debug_overlay)?;
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        Ok(window)
    }

    fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn monitor(&self) -> &Monitor {
        &self.state.monitor
    }

    fn update_monitor(
        &mut self,
        monitor: Monitor,
        annotation_enabled: bool,
        debug_overlay: bool,
    ) -> Result<()> {
        let old = self.state.monitor.clone();
        self.state.monitor = monitor;
        self.state
            .input
            .update_monitor_key(self.state.monitor.session_key.clone());
        self.state.debug_visible = annotation_enabled && debug_overlay;
        let rect = self.state.monitor.rect;
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }?;
        if old.rect != rect || old.dpi != self.state.monitor.dpi {
            let strokes = self.strokes();
            self.state
                .renderer
                .as_mut()
                .expect("renderer lives until OverlayWindow::drop")
                .resize(
                    rect.width(),
                    rect.height(),
                    self.state.monitor.dpi.0,
                    &strokes,
                    self.state.input.pressure_minimum_width_ratio(),
                    self.state.input.highlighter_opacity(),
                )?;
        }
        self.state.draw()
    }

    fn update_input_context(
        &mut self,
        annotation_enabled: bool,
        input_config: InkInputConfig,
    ) -> Result<()> {
        self.state
            .input
            .update_context(annotation_enabled, input_config);
        self.state.draw()?;
        self.state.sync_laser_timer(self.hwnd)
    }

    fn finish_active(&mut self, reason: &str) -> Result<()> {
        self.state.finish_active(reason)?;
        self.state.sync_laser_timer(self.hwnd)
    }

    fn set_click_through(&mut self, enabled: bool) -> Result<()> {
        let current = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) };
        let toggle = (WS_EX_TRANSPARENT | WS_EX_LAYERED).0 as isize;
        let updated = if enabled {
            current | toggle
        } else {
            current & !toggle
        };
        if updated != current {
            unsafe { windows::Win32::Foundation::SetLastError(WIN32_ERROR(0)) };
            let previous = unsafe { SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, updated) };
            let error = unsafe { windows::Win32::Foundation::GetLastError() };
            if previous == 0 && error != ERROR_SUCCESS {
                return Err(Error::from_thread());
            }
        }
        if enabled {
            unsafe { SetLayeredWindowAttributes(self.hwnd, COLORREF(0), 255, LWA_ALPHA) }?;
        }
        Ok(())
    }

    fn draw(&mut self, debug_visible: bool) -> Result<()> {
        self.state.debug_visible = debug_visible;
        self.state.draw()
    }

    fn swap_chain_size(&self) -> Result<(u32, u32)> {
        self.state
            .renderer
            .as_ref()
            .expect("renderer lives until OverlayWindow::drop")
            .swap_chain_size()
    }

    fn strokes(&self) -> Vec<InkStroke> {
        self.state
            .layers
            .borrow()
            .get(&self.state.monitor.session_key)
            .map(|layer| layer.strokes().to_vec())
            .unwrap_or_default()
    }

    fn rebuild_completed(&self) -> Result<()> {
        self.state
            .renderer
            .as_ref()
            .expect("renderer lives until OverlayWindow::drop")
            .rebuild_completed(
                &self.strokes(),
                self.state.input.pressure_minimum_width_ratio(),
                self.state.input.highlighter_opacity(),
            )
    }

    fn selftest_render_path(&mut self) -> Result<Vec<String>> {
        let mut fake_layer = InkLayerState::new();
        let fake = InkStroke::new(
            "selftest-five-point-stroke",
            InkStrokeTool::Pen,
            [0.2, 0.6, 1.0, 1.0],
            8.0,
            true,
            (0..5)
                .map(|index| {
                    InkPoint::new(
                        Point {
                            x: 0.1 + f64::from(index) * 0.08,
                            y: 0.2 + f64::from(index % 2) * 0.08,
                        },
                        0.2 + f64::from(index) * 0.2,
                        f64::from(index),
                    )
                })
                .collect(),
            InkStrokeContent::Freehand,
        );
        fake_layer.execute(InkCommand::AddStroke(fake));
        let ratio = self.state.input.pressure_minimum_width_ratio();
        let renderer = self
            .state
            .renderer
            .as_ref()
            .expect("renderer lives until OverlayWindow::drop");
        let highlighter_opacity = self.state.input.highlighter_opacity();
        renderer.selftest_icons()?;
        let pixel_failures = renderer.selftest_pixel_failures(ratio, highlighter_opacity)?;
        renderer.rebuild_completed(fake_layer.strokes(), ratio, highlighter_opacity)?;
        renderer.draw_frame(None, None, None, ratio, highlighter_opacity, false)?;
        renderer.rebuild_completed(&self.strokes(), ratio, highlighter_opacity)?;
        self.state.draw()?;
        Ok(pixel_failures)
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        if self.state.laser_timer_running {
            if let Err(error) = unsafe { KillTimer(Some(self.hwnd), LASER_TIMER_ID) } {
                self.state
                    .logger
                    .event(&format!("레이저 타이머 해제 실패: {error}"));
            }
            self.state.laser_timer_running = false;
        }
        self.state.renderer.take();
        if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
            self.state
                .logger
                .event(&format!("오버레이 창 제거 실패: {error}"));
        }
    }
}

unsafe extern "system" fn overlay_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let state = create.lpCreateParams as *mut OverlayWindowState;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
    }
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayWindowState };
    if !state.is_null() {
        let state = unsafe { &mut *state };
        match message {
            WM_DPICHANGED => {
                let rect = state.monitor.rect;
                let result = unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        rect.left,
                        rect.top,
                        rect.width(),
                        rect.height(),
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                };
                match result {
                    Ok(()) => state.logger.event(&format!(
                        "오버레이 WM_DPICHANGED: key={} rcMonitor 적용, SetWindowPos 호출=1",
                        state.monitor.session_key
                    )),
                    Err(error) => state.logger.event(&format!(
                        "오버레이 WM_DPICHANGED 배치 실패: key={}, {error}",
                        state.monitor.session_key
                    )),
                }
                post_sync(state, "WM_DPICHANGED");
                return LRESULT(0);
            }
            WM_DISPLAYCHANGE => {
                post_sync(state, "WM_DISPLAYCHANGE");
                return LRESULT(0);
            }
            WM_POINTERDOWN
            | WM_POINTERUPDATE
            | WM_POINTERUP
            | WM_POINTERCAPTURECHANGED
            | WM_POINTERLEAVE => {
                let pointer_id = pointer_id_from_wparam(wparam.0);
                let strokes = state.current_strokes();
                let result = state
                    .input
                    .handle_message(message, pointer_id, &state.monitor, &strokes)
                    .and_then(|outcome| state.apply_input_outcome(outcome))
                    .and_then(|()| state.sync_laser_timer(hwnd));
                match result {
                    Ok(()) => {}
                    Err(error) => state.logger.event(&format!(
                        "포인터 입력/렌더 실패: monitor={} pointerId={pointer_id} message={message:#x}: {error}",
                        state.monitor.session_key
                    )),
                }
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == LASER_TIMER_ID => {
                let result = state
                    .input
                    .retract_laser()
                    .and_then(|outcome| state.apply_input_outcome(outcome))
                    .and_then(|()| state.sync_laser_timer(hwnd));
                if let Err(error) = result {
                    state.logger.event(&format!(
                        "레이저 줄이기 타이머 실패: monitor={}: {error}",
                        state.monitor.session_key
                    ));
                }
                return LRESULT(0);
            }
            WM_POINTERACTIVATE => return LRESULT(PA_NOACTIVATE as isize),
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_NCDESTROY => {
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn post_sync(state: &OverlayWindowState, reason: &str) {
    if state.owner.is_invalid() {
        return;
    }
    if let Err(error) = unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(state.owner),
            SYNC_MESSAGE,
            WPARAM(0),
            LPARAM(0),
        )
    } {
        state
            .logger
            .event(&format!("오버레이 {reason} 동기화 요청 실패: {error}"));
    }
}
