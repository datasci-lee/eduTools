use crate::archive::{disconnect_action, ArchiveController, DisconnectAction};
use crate::export::BgraImage;
use crate::ink_view::{
    pointer_id_from_wparam, stroke_shape_label, InkInputConfig, InkInputController,
    InkInputMapping, InputOutcome, StrokeReplacementKind, TextEntrySeed,
};
use crate::log::Logger;
use crate::monitor::{
    current_display_configuration, enumerate_monitors, hash_prefix8, monitor_at_point,
    monitor_by_key, plan_monitor_sync, Monitor,
};
use crate::render::{BoardRenderState, GraphicsDevice, OverlayRenderer};
use crate::text_entry::{TextEntryCommit, TextEntryRequest, WM_APP_TEXT_ENTRY_OPEN};
use crate::z_bands::{raise_band_order, Band, BandRegistration};
use edutools_core::board::{BoardAppearance, BoardBackgroundPattern, BoardKind, BoardPagePosition};
use edutools_core::dispatch::BoardPageDirection;
use edutools_core::ink::{
    text_stroke_size, InkCommand, InkLayerState, InkPoint, InkStroke, InkStrokeContent,
    InkStrokeTool, InkTextContent, Point, Size,
};
use edutools_core::settings::{AppSettings, SettingsColor};
use edutools_core::state::{AppState, InkRouteTarget};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{c_void, OsString};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use windows::core::{w, Error, Result};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, ERROR_SUCCESS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT,
    RECT, WIN32_ERROR, WPARAM,
};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetWindowLongPtrW, GetWindowRect,
    KillTimer, RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, CREATESTRUCTW, GWLP_USERDATA, GWL_EXSTYLE, LWA_ALPHA, MA_NOACTIVATE,
    PA_NOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_MOUSEACTIVATE, WM_NCCREATE, WM_NCDESTROY,
    WM_POINTERACTIVATE, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERLEAVE, WM_POINTERUP,
    WM_POINTERUPDATE, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

pub const SYNC_MESSAGE: u32 = 0x8000 + 2;
const OVERLAY_CLASS: windows::core::PCWSTR = w!("EduToolsOverlay");
const LASER_TIMER_ID: usize = 0xED11;
const LASER_TIMER_INTERVAL_MS: u32 = 16;
const RECOGNITION_TIMER_ID: usize = 0xED12;
const RECOGNITION_TIMER_INTERVAL_MS: u32 = 500;
static TEXT_STROKE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

type LayerStore = Rc<RefCell<HashMap<String, InkLayerState>>>;
type SharedAppState = Rc<RefCell<AppState>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardMoveResult {
    pub changed: bool,
    pub from: usize,
    pub to: usize,
    pub total: usize,
}

pub struct OverlayManager {
    overlays: HashMap<String, OverlayWindow>,
    layers: LayerStore,
    app_state: SharedAppState,
    graphics: GraphicsDevice,
    owner: HWND,
    hinstance: HINSTANCE,
    annotation_enabled: bool,
    input_config: InkInputConfig,
    debug_overlay: bool,
    pending_preset_archive_keys: HashSet<String>,
    archive: ArchiveController,
    logger: Logger,
}

impl OverlayManager {
    pub fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        input_config: InkInputConfig,
        app_state: SharedAppState,
        settings: &AppSettings,
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
            app_state,
            graphics,
            owner,
            hinstance,
            annotation_enabled: false,
            input_config,
            debug_overlay,
            pending_preset_archive_keys: HashSet::new(),
            archive: ArchiveController::new(owner, settings, logger.clone()),
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
        let primary_session_key = monitors
            .iter()
            .find(|monitor| monitor.is_primary)
            .map(|monitor| monitor.session_key.clone());
        for key in &plan.removed {
            if let Some(overlay) = self.overlays.get_mut(key) {
                overlay.finish_active("모니터 제거")?;
            }
        }
        for key in &plan.removed {
            let archive_key = self
                .overlays
                .get(key)
                .map(|overlay| overlay.monitor().archive_key.clone());
            if archive_key
                .as_ref()
                .is_some_and(|value| self.pending_preset_archive_keys.remove(value))
            {
                self.logger.event(&format!(
                    "화면 프리셋 제거 직전 보관 훅 적용: monitorHash={}",
                    hash_prefix8(archive_key.as_deref().expect("checked archive key"))
                ));
            } else {
                self.display_was_removed(key, primary_session_key.as_deref());
            }
            self.overlays.remove(key);
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
                self.app_state.clone(),
                self.annotation_enabled,
                self.input_config.clone(),
                self.debug_overlay,
                self.logger.clone(),
            )?;
            self.overlays.insert(monitor.session_key.clone(), overlay);
        }

        for monitor in &monitors {
            self.restore_disconnected_display_if_needed(
                &monitor.session_key,
                &monitor.archive_key,
            )?;
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
        raise_band_order()?;
        Ok(())
    }

    pub fn archive_disconnected_displays_for_preset(
        &mut self,
        display_ids: &HashSet<String>,
    ) -> Option<HashSet<String>> {
        let configuration = match current_display_configuration() {
            Ok(configuration) => configuration,
            Err(error) => {
                self.archive
                    .report_failure("프리셋 보관 대상 확인", "display configuration unavailable");
                self.logger
                    .event(&format!("프리셋 보관 대상 확인 실패: {error}"));
                return None;
            }
        };
        let mut archived = HashSet::new();
        let mut requested = display_ids.iter().collect::<Vec<_>>();
        requested.sort();
        for display_id in requested {
            let Some(archive_key) = configuration.archive_key_for_uuid(display_id) else {
                self.archive
                    .report_failure("프리셋 보관 대상 연결", "display identity was unavailable");
                self.restore_displays_archived_for_failed_preset(&archived);
                return None;
            };
            let Some((session_key, resolution)) = self
                .overlays
                .values()
                .find(|overlay| overlay.monitor().archive_key == archive_key)
                .map(|overlay| {
                    (
                        overlay.monitor().session_key.clone(),
                        monitor_size(overlay.monitor()),
                    )
                })
            else {
                self.archive
                    .report_failure("프리셋 보관 대상 연결", "display overlay was unavailable");
                self.restore_displays_archived_for_failed_preset(&archived);
                return None;
            };
            if let Some(overlay) = self.overlays.get_mut(&session_key) {
                if let Err(error) = overlay.finish_active("화면 프리셋 적용 전 보관") {
                    self.archive.report_failure(
                        "프리셋 보관 입력 종료",
                        "active interaction could not be finished",
                    );
                    self.logger
                        .event(&format!("프리셋 보관 입력 종료 실패: {error}"));
                    self.restore_displays_archived_for_failed_preset(&archived);
                    return None;
                }
            }
            let layer = self
                .layers
                .borrow()
                .get(&session_key)
                .cloned()
                .unwrap_or_default();
            match self
                .archive
                .store_disconnected(archive_key, &layer, resolution, "preset")
            {
                Ok(false) => continue,
                Ok(true) => {
                    self.archive.display_was_removed(archive_key);
                    if let Some(current) = self.layers.borrow_mut().get_mut(&session_key) {
                        AppState::discard_screen_ink(
                            &mut self.app_state.borrow_mut(),
                            &session_key,
                            current,
                        );
                    }
                    archived.insert(archive_key.to_owned());
                    self.pending_preset_archive_keys
                        .insert(archive_key.to_owned());
                    if let Err(error) = self.rebuild_monitor(&session_key) {
                        self.logger
                            .event(&format!("프리셋 보관 후 화면 갱신 실패: {error}"));
                    }
                }
                Err(error) => {
                    self.archive.report_failure("프리셋 화면 보관", &error);
                    self.restore_displays_archived_for_failed_preset(&archived);
                    return None;
                }
            }
        }
        self.logger.event(&format!(
            "archive_disconnected_displays_for_preset: requested={} archived={}",
            display_ids.len(),
            archived.len()
        ));
        Some(archived)
    }

    pub fn restore_displays_archived_for_failed_preset(&mut self, archive_keys: &HashSet<String>) {
        let mut keys = archive_keys.iter().collect::<Vec<_>>();
        keys.sort();
        for archive_key in keys {
            let Some(session_key) = self
                .overlays
                .values()
                .find(|overlay| overlay.monitor().archive_key == *archive_key)
                .map(|overlay| overlay.monitor().session_key.clone())
            else {
                continue;
            };
            match self.archive.restore_stored(archive_key) {
                Ok(Some(archived)) => {
                    let restored = {
                        let mut layers = self.layers.borrow_mut();
                        let layer = layers.entry(session_key.clone()).or_default();
                        self.app_state.borrow_mut().restore_screen_ink(
                            &session_key,
                            layer,
                            &archived,
                        )
                    };
                    if restored {
                        if let Err(error) = self.rebuild_monitor(&session_key) {
                            self.logger
                                .event(&format!("프리셋 실패 복원 화면 갱신 실패: {error}"));
                        }
                    }
                    self.pending_preset_archive_keys.remove(archive_key);
                }
                Ok(None) => {}
                Err(error) => self.archive.report_failure("프리셋 실패 화면 복원", &error),
            }
        }
        self.logger.event(&format!(
            "restore_displays_archived_for_failed_preset: requested={}",
            archive_keys.len()
        ));
    }

    pub fn set_display_preset_restoration_deferred(
        &mut self,
        deferred: bool,
        resume_pending: bool,
    ) {
        let pending = self
            .archive
            .set_preset_restoration_deferred(deferred, resume_pending);
        for archive_key in pending {
            let Some(session_key) = self
                .overlays
                .values()
                .find(|overlay| overlay.monitor().archive_key == archive_key)
                .map(|overlay| overlay.monitor().session_key.clone())
            else {
                continue;
            };
            if let Err(error) =
                self.restore_disconnected_display_if_needed(&session_key, &archive_key)
            {
                self.logger
                    .event(&format!("보류된 화면 복원 갱신 실패: {error}"));
            }
        }
    }

    pub fn update_archive_settings(&mut self, settings: &AppSettings) {
        self.archive.update_settings(settings);
    }

    fn display_was_removed(&mut self, session_key: &str, primary_session_key: Option<&str>) {
        let Some((archive_key, resolution)) = self.overlays.get(session_key).map(|overlay| {
            (
                overlay.monitor().archive_key.clone(),
                monitor_size(overlay.monitor()),
            )
        }) else {
            return;
        };
        self.archive.display_was_removed(&archive_key);
        let has_state = self
            .layers
            .borrow()
            .get(session_key)
            .is_some_and(AppState::has_restorable_screen_state);
        let policy = self.archive.policy();
        let mut moved = false;
        if policy == edutools_core::state::DisplayDisconnectPolicy::MoveToPrimary {
            if let Some(primary_key) = primary_session_key.filter(|key| *key != session_key) {
                let mut layers = self.layers.borrow_mut();
                let mut source = layers.remove(session_key).unwrap_or_default();
                let destination = layers.entry(primary_key.to_owned()).or_default();
                moved = self.app_state.borrow_mut().move_screen_ink(
                    session_key,
                    &mut source,
                    primary_key,
                    destination,
                );
                layers.insert(session_key.to_owned(), source);
                drop(layers);
                if moved {
                    if let Err(error) = self.rebuild_monitor(primary_key) {
                        self.logger
                            .event(&format!("화면 이동 후 주 화면 갱신 실패: {error}"));
                    }
                }
            }
        }
        let action = disconnect_action(policy, has_state, primary_session_key.is_some(), moved);
        match action {
            DisconnectAction::None | DisconnectAction::MoveToPrimary => {}
            DisconnectAction::Delete => {
                if let Some(layer) = self.layers.borrow_mut().get_mut(session_key) {
                    self.app_state
                        .borrow_mut()
                        .discard_screen_ink(session_key, layer);
                }
            }
            DisconnectAction::Archive => {
                let layer = self
                    .layers
                    .borrow()
                    .get(session_key)
                    .cloned()
                    .unwrap_or_default();
                match self.archive.store_disconnected(
                    &archive_key,
                    &layer,
                    resolution,
                    "disconnect-policy",
                ) {
                    Ok(true) => {
                        if let Some(current) = self.layers.borrow_mut().get_mut(session_key) {
                            self.app_state
                                .borrow_mut()
                                .discard_screen_ink(session_key, current);
                        }
                    }
                    Ok(false) => {}
                    Err(error) => self.archive.report_failure("연결 해제 화면 보관", &error),
                }
            }
        }
        self.logger.event(&format!(
            "화면 사라짐 정책 적용: policy={} action={action:?} monitorHash={}",
            policy.as_str(),
            hash_prefix8(&archive_key)
        ));
    }

    fn restore_disconnected_display_if_needed(
        &mut self,
        session_key: &str,
        archive_key: &str,
    ) -> Result<()> {
        let archived = match self.archive.restore_if_needed(archive_key) {
            Ok(archived) => archived,
            Err(error) => {
                self.archive.report_failure("연결 화면 복원", &error);
                return Ok(());
            }
        };
        let Some(archived) = archived else {
            return Ok(());
        };
        let restored = {
            let mut layers = self.layers.borrow_mut();
            let layer = layers.entry(session_key.to_owned()).or_default();
            self.app_state
                .borrow_mut()
                .restore_screen_ink(session_key, layer, &archived)
        };
        if restored {
            self.rebuild_monitor(session_key)?;
        }
        Ok(())
    }

    pub fn archive_all_for_sleep(&mut self) -> Result<usize> {
        self.finish_all_active("PBT_APMSUSPEND")?;
        let targets = self
            .overlays
            .values()
            .map(|overlay| {
                (
                    overlay.monitor().session_key.clone(),
                    overlay.monitor().archive_key.clone(),
                    monitor_size(overlay.monitor()),
                )
            })
            .collect::<Vec<_>>();
        let mut archived_count = 0;
        let mut failed = false;
        let mut changed_keys = Vec::new();
        for (session_key, archive_key, resolution) in targets {
            let layer = self
                .layers
                .borrow()
                .get(&session_key)
                .cloned()
                .unwrap_or_default();
            match self.archive.store_disconnected(
                &archive_key,
                &layer,
                resolution,
                "PBT_APMSUSPEND",
            ) {
                Ok(true) => {
                    self.archive.display_was_removed(&archive_key);
                    if let Some(current) = self.layers.borrow_mut().get_mut(&session_key) {
                        self.app_state
                            .borrow_mut()
                            .discard_screen_ink(&session_key, current);
                    }
                    archived_count += 1;
                    changed_keys.push(session_key);
                }
                Ok(false) => {}
                Err(_) => failed = true,
            }
        }
        if failed {
            self.archive
                .report_failure("잠자기 전 화면 보관", "one or more displays failed");
            return Err(Error::new(
                E_FAIL,
                "sleep archive failed for one or more displays",
            ));
        }
        for session_key in changed_keys {
            self.rebuild_monitor(&session_key)?;
        }
        self.logger.event(&format!(
            "잠자기 전 화면 보관 완료: archived={archived_count}"
        ));
        Ok(archived_count)
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

    pub fn contains_monitor(&self, monitor_key: &str) -> bool {
        self.overlays.contains_key(monitor_key)
    }

    pub fn monitor(&self, monitor_key: &str) -> Option<Monitor> {
        self.overlays
            .get(monitor_key)
            .map(|overlay| overlay.monitor().clone())
    }

    pub fn graphics_device(&self) -> GraphicsDevice {
        self.graphics.clone()
    }

    pub fn finish_export_interaction(&mut self, monitor_key: &str) -> Result<()> {
        self.overlays
            .get_mut(monitor_key)
            .ok_or_else(|| {
                Error::new(
                    E_FAIL,
                    format!("내보내기 대상 오버레이가 없음: {monitor_key}"),
                )
            })?
            .finish_active("내보내기")
    }

    pub fn export_ink_bitmap(&mut self, monitor_key: &str) -> Result<BgraImage> {
        let overlay = self.overlays.get_mut(monitor_key).ok_or_else(|| {
            Error::new(
                E_FAIL,
                format!("내보내기 대상 오버레이가 없음: {monitor_key}"),
            )
        })?;
        overlay.finish_active("내보내기")?;
        let strokes = overlay.strokes();
        overlay
            .state
            .renderer
            .as_ref()
            .expect("renderer lives until OverlayWindow::drop")
            .export_bitmap(
                &strokes,
                None,
                overlay.state.input.pressure_minimum_width_ratio(),
                overlay.state.input.highlighter_opacity(),
            )
    }

    pub fn export_board_page_bitmap(
        &mut self,
        monitor_key: &str,
        strokes: &[InkStroke],
        kind: BoardKind,
        page_position: BoardPagePosition,
        settings: &AppSettings,
    ) -> Result<BgraImage> {
        let overlay = self.overlays.get_mut(monitor_key).ok_or_else(|| {
            Error::new(
                E_FAIL,
                format!("보드 내보내기 대상 오버레이가 없음: {monitor_key}"),
            )
        })?;
        overlay.finish_active("보드 PDF 내보내기")?;
        let background = BoardAppearance::settings_background_color(kind, &settings.board);
        let pattern_color = BoardAppearance::pattern_color(kind);
        let mut background_color = color_components(&background);
        background_color[3] = 1.0;
        let board = BoardRenderState {
            kind,
            background_color,
            pattern: BoardBackgroundPattern::parse(&settings.board.background_pattern)
                .expect("validated board background pattern"),
            pattern_color: color_components(&pattern_color),
            page_position,
        };
        overlay
            .state
            .renderer
            .as_ref()
            .expect("renderer lives until OverlayWindow::drop")
            .export_bitmap(
                strokes,
                Some(&board),
                overlay.state.input.pressure_minimum_width_ratio(),
                overlay.state.input.highlighter_opacity(),
            )
    }

    pub fn zoom_ink_frame(&self, monitor_key: &str) -> Option<ID2D1Bitmap1> {
        self.overlays
            .get(monitor_key)
            .and_then(|overlay| overlay.state.renderer.as_ref())
            .and_then(OverlayRenderer::frame_bitmap)
    }

    pub fn zoom_gesture_in_progress(&self, monitor_key: &str) -> bool {
        self.overlays
            .get(monitor_key)
            .is_some_and(|overlay| overlay.state.input.gesture_in_progress())
    }

    pub fn handle_zoom_input(
        &mut self,
        message: u32,
        pointer_id: u32,
        session: &edutools_core::zoom::ZoomSessionState,
    ) -> Result<()> {
        if !self.annotation_enabled || session.is_previewing {
            return Ok(());
        }
        if message == WM_POINTERDOWN {
            self.app_state
                .borrow_mut()
                .set_active_display_if_annotating(session.display_id.clone());
        }
        let Some(overlay) = self.overlays.get_mut(&session.display_id) else {
            return Err(Error::new(
                E_FAIL,
                format!("줌 판서 대상 오버레이가 없음: {}", session.display_id),
            ));
        };
        let mapping = InkInputMapping::for_zoom(session);
        if message == WM_POINTERDOWN {
            self.logger.event(&format!(
                "줌 판서 입력 시작: monitor={} pointerId={pointer_id} mapping={}",
                session.display_id,
                mapping.label()
            ));
        }
        overlay.handle_pointer_message(message, pointer_id, mapping)?;
        if matches!(
            message,
            WM_POINTERUP | WM_POINTERCAPTURECHANGED | WM_POINTERLEAVE
        ) {
            self.logger.event(&format!(
                "줌 판서 입력 종료: monitor={} pointerId={pointer_id} mapping={} message={message:#x}",
                session.display_id,
                mapping.label()
            ));
        }
        Ok(())
    }

    pub fn add_text_stroke(&mut self, commit: TextEntryCommit) -> Result<Size> {
        let monitor_key = commit.request.monitor_key.clone();
        let overlay = self.overlays.get_mut(&monitor_key).ok_or_else(|| {
            Error::new(
                E_FAIL,
                format!("텍스트 대상 모니터가 사라짐: {monitor_key}"),
            )
        })?;
        overlay.add_text_stroke(commit)
    }

    pub fn refresh_board(&mut self, settings: &AppSettings) -> Result<()> {
        let pattern = BoardBackgroundPattern::parse(&settings.board.background_pattern)
            .expect("validated board background pattern");
        for (monitor_key, overlay) in &mut self.overlays {
            let board = {
                let state = self.app_state.borrow();
                state.board_kind_on(monitor_key).map(|kind| {
                    let background =
                        BoardAppearance::settings_background_color(kind, &settings.board);
                    let pattern_color = BoardAppearance::pattern_color(kind);
                    let mut background_color = color_components(&background);
                    background_color[3] = 1.0;
                    BoardRenderState {
                        kind,
                        background_color,
                        pattern,
                        pattern_color: color_components(&pattern_color),
                        page_position: state
                            .board_page_position(monitor_key)
                            .expect("board kind and page position are paired"),
                    }
                })
            };
            overlay.set_board(board)?;
        }
        Ok(())
    }

    pub fn redraw_monitor(&mut self, monitor_key: &str) -> Result<()> {
        self.rebuild_monitor(monitor_key)
    }

    pub fn move_board_page(
        &mut self,
        direction: BoardPageDirection,
        monitor_key: &str,
        settings: &AppSettings,
    ) -> Result<BoardMoveResult> {
        if let Some(overlay) = self.overlays.get_mut(monitor_key) {
            overlay.finish_active("보드 페이지 이동")?;
        }
        let (changed, from, to, total) = {
            let mut state = self.app_state.borrow_mut();
            let before = state.board_page_position(monitor_key);
            let changed = match direction {
                BoardPageDirection::Previous => state.move_to_previous_board_page(monitor_key),
                BoardPageDirection::Next => state.move_to_next_board_page(monitor_key),
            };
            let after = state.board_page_position(monitor_key);
            let from = before.map_or(0, |position| position.current);
            let to = after.map_or(from, |position| position.current);
            let total = after.or(before).map_or(0, |position| position.total);
            (changed, from, to, total)
        };
        if changed {
            self.refresh_board(settings)?;
        }
        Ok(BoardMoveResult {
            changed,
            from,
            to,
            total,
        })
    }

    pub fn undo(&mut self, monitor_key: &str) -> Result<bool> {
        let changed = {
            let mut layers = self.layers.borrow_mut();
            let layer = layers.entry(monitor_key.to_owned()).or_default();
            let mut state = self.app_state.borrow_mut();
            let route = state.ink_route(monitor_key);
            self.logger.event(&format!(
                "스트로크 라우팅: operation=undo monitor={monitor_key} target={}",
                route.label()
            ));
            state.undo_ink(monitor_key, layer)
        };
        if changed {
            self.rebuild_monitor(monitor_key)?;
        }
        Ok(changed)
    }

    pub fn redo(&mut self, monitor_key: &str) -> Result<bool> {
        let changed = {
            let mut layers = self.layers.borrow_mut();
            let layer = layers.entry(monitor_key.to_owned()).or_default();
            let mut state = self.app_state.borrow_mut();
            let route = state.ink_route(monitor_key);
            self.logger.event(&format!(
                "스트로크 라우팅: operation=redo monitor={monitor_key} target={}",
                route.label()
            ));
            state.redo_ink(monitor_key, layer)
        };
        if changed {
            self.rebuild_monitor(monitor_key)?;
        }
        Ok(changed)
    }

    pub fn clear_all(&mut self) -> Result<usize> {
        let board_display = self
            .app_state
            .borrow()
            .presented_board()
            .map(|board| board.display_id.clone());
        let (target_keys, clear_all_selections) = if let Some(key) = board_display {
            (vec![key], false)
        } else {
            (self.overlays.keys().cloned().collect::<Vec<_>>(), true)
        };
        self.clear_display_targets(&target_keys, "전체 지우기", clear_all_selections)
    }

    pub fn clear_display(&mut self, key: &str) -> Result<usize> {
        self.clear_display_targets(&[key.to_owned()], "현재 모니터만 지우기", false)
    }

    fn clear_display_targets(
        &mut self,
        target_keys: &[String],
        operation: &str,
        clear_all_selections: bool,
    ) -> Result<usize> {
        if self.archive.auto_archive_enabled() {
            let visible_keys = {
                let layers = self.layers.borrow();
                let app_state = self.app_state.borrow();
                target_keys
                    .iter()
                    .filter(|key| {
                        layers
                            .get(key.as_str())
                            .is_some_and(|layer| !app_state.strokes_for(key, layer).is_empty())
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            };
            for key in visible_keys {
                let Some((archive_key, resolution)) = self.overlays.get(&key).map(|overlay| {
                    (
                        overlay.monitor().archive_key.clone(),
                        monitor_size(overlay.monitor()),
                    )
                }) else {
                    self.archive.report_failure(
                        "자동 보관 대상 연결",
                        "visible display overlay was unavailable",
                    );
                    return Err(Error::new(E_FAIL, "automatic archive target unavailable"));
                };
                let archived_state = {
                    let layers = self.layers.borrow();
                    let layer = layers.get(&key).cloned().unwrap_or_default();
                    let app_state = self.app_state.borrow();
                    if app_state
                        .presented_board()
                        .is_some_and(|board| board.display_id == key)
                    {
                        let mut visible = InkLayerState::new();
                        for stroke in app_state.strokes_for(&key, &layer) {
                            visible.execute(InkCommand::AddStroke(stroke.clone()));
                        }
                        visible
                    } else {
                        layer
                    }
                };
                if let Err(error) =
                    self.archive
                        .archive_visible(&archive_key, &archived_state, resolution)
                {
                    self.archive
                        .report_failure(&format!("{operation} 전 자동 보관"), &error);
                    return Err(Error::new(
                        E_FAIL,
                        "automatic archive failed; clear was cancelled",
                    ));
                }
            }
        }
        let changed_keys = {
            let mut layers = self.layers.borrow_mut();
            let mut app_state = self.app_state.borrow_mut();
            let mut clear_keys = layers
                .keys()
                .filter(|key| target_keys.iter().any(|target| target == *key))
                .cloned()
                .collect::<Vec<_>>();
            clear_keys.extend(
                target_keys
                    .iter()
                    .filter(|key| {
                        !layers.contains_key(*key)
                            && matches!(app_state.ink_route(key), InkRouteTarget::Board(_))
                    })
                    .cloned(),
            );
            let mut changed = Vec::new();
            for key in clear_keys {
                let route = app_state.ink_route(&key);
                let board_target = matches!(route, InkRouteTarget::Board(_));
                if board_target && !layers.contains_key(&key) {
                    layers.entry(key.clone()).or_default();
                }
                let Some(layer) = layers.get_mut(&key) else {
                    continue;
                };
                if !board_target && app_state.strokes_for(&key, layer).is_empty() {
                    continue;
                }
                self.logger.event(&format!(
                    "{operation} 대상: monitor={key} target={}",
                    route.label()
                ));
                if app_state.clear_ink(&key, layer) {
                    changed.push(key);
                }
            }
            if clear_all_selections {
                app_state.clear_all_selections();
            }
            changed
        };
        for key in &changed_keys {
            self.rebuild_monitor(key)?;
        }
        Ok(changed_keys.len())
    }

    pub fn clear_selection(&mut self, monitor_key: &str) -> Result<bool> {
        let changed = self.app_state.borrow_mut().clear_selection(monitor_key);
        if changed {
            self.logger
                .event(&format!("선택 해제: monitor={monitor_key}"));
            self.rebuild_monitor(monitor_key)?;
        }
        Ok(changed)
    }

    pub fn delete_selection(&mut self, monitor_key: &str) -> Result<bool> {
        let changed = {
            let mut layers = self.layers.borrow_mut();
            layers.get_mut(monitor_key).is_some_and(|layer| {
                self.app_state
                    .borrow_mut()
                    .delete_selection(monitor_key, layer)
            })
        };
        if changed {
            self.logger
                .event(&format!("선택 삭제: monitor={monitor_key} Undo=1"));
            self.rebuild_monitor(monitor_key)?;
        }
        Ok(changed)
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

fn monitor_size(monitor: &Monitor) -> Size {
    Size {
        width: f64::from(monitor.rect.width()),
        height: f64::from(monitor.rect.height()),
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
    app_state: SharedAppState,
    input: InkInputController,
    renderer: Option<OverlayRenderer>,
    board: Option<BoardRenderState>,
    debug_visible: bool,
    laser_timer_running: bool,
    recognition_timer_running: bool,
}

impl OverlayWindowState {
    fn apply_input_outcome(&mut self, outcome: InputOutcome) -> Result<()> {
        let text_entry = outcome.text_entry;
        let minimum_ratio = self.input.pressure_minimum_width_ratio();
        let highlighter_opacity = self.input.highlighter_opacity();
        for stroke in outcome.finished {
            self.execute_ink_command(InkCommand::AddStroke(stroke.clone()));
            self.renderer
                .as_ref()
                .expect("renderer initialized after window creation")
                .append_completed(&stroke, minimum_ratio, highlighter_opacity)?;
        }
        if let Some(recognized) = outcome.recognized {
            let before = self.current_strokes();
            let after = before
                .iter()
                .map(|stroke| {
                    if stroke.identifier == recognized.identifier {
                        recognized.clone()
                    } else {
                        stroke.clone()
                    }
                })
                .collect::<Vec<_>>();
            if before != after {
                self.logger.event(&format!(
                    "도형 인식 후보 적용: monitor={} kind={} Undo=2(Add+Replace)",
                    self.monitor.session_key,
                    stroke_shape_label(&recognized)
                ));
                self.execute_ink_command(InkCommand::ReplaceStrokes { before, after });
                let strokes = self.current_strokes();
                self.renderer
                    .as_ref()
                    .expect("renderer initialized after window creation")
                    .rebuild_completed(&strokes, minimum_ratio, highlighter_opacity)?;
            }
        }
        if let Some(replacement) = outcome.replacement {
            match replacement.kind {
                StrokeReplacementKind::Eraser { erased_strokes } => self.logger.event(&format!(
                    "지우개 replaceStrokes 적용: monitor={} erased={} Undo=1",
                    self.monitor.session_key, erased_strokes
                )),
                StrokeReplacementKind::SelectionMove => self.logger.event(&format!(
                    "선택 이동 replaceStrokes 적용: monitor={} Undo=1",
                    self.monitor.session_key
                )),
                StrokeReplacementKind::SelectionResize => self.logger.event(&format!(
                    "선택 크기 조절 replaceStrokes 적용: monitor={} Undo=1",
                    self.monitor.session_key
                )),
            }
            self.execute_ink_command(InkCommand::ReplaceStrokes {
                before: replacement.before,
                after: replacement.after,
            });
            let strokes = self.current_strokes();
            self.renderer
                .as_ref()
                .expect("renderer initialized after window creation")
                .rebuild_completed(&strokes, minimum_ratio, highlighter_opacity)?;
        }
        if let Some(selected_ids) = outcome.selected_ids {
            let strokes = self.current_strokes();
            let selected_count = selected_ids.len();
            let changed = self.app_state.borrow_mut().set_selected_stroke_ids(
                &self.monitor.session_key,
                selected_ids,
                &strokes,
            );
            if changed && selected_count == 0 {
                self.logger
                    .event(&format!("선택 해제: monitor={}", self.monitor.session_key));
            }
        }
        if outcome.redraw {
            self.draw()?;
        }
        if let Some(seed) = text_entry {
            self.post_text_entry_request(seed)?;
        }
        Ok(())
    }

    fn post_text_entry_request(&self, seed: TextEntrySeed) -> Result<()> {
        if self.owner.is_invalid() {
            return Err(Error::new(E_FAIL, "텍스트 편집 요청을 받을 앱 창이 없음"));
        }
        let request = Box::new(TextEntryRequest {
            monitor_key: self.monitor.session_key.clone(),
            monitor_left: self.monitor.rect.left,
            monitor_top: self.monitor.rect.top,
            monitor_size: Size {
                width: f64::from(self.monitor.rect.width()),
                height: f64::from(self.monitor.rect.height()),
            },
            origin: seed.origin,
            display_origin: seed.display_origin,
            dpi: self.monitor.dpi.0,
            color: seed.color,
            font_size: seed.font_size,
        });
        let raw = Box::into_raw(request);
        match unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(self.owner),
                WM_APP_TEXT_ENTRY_OPEN,
                WPARAM(0),
                LPARAM(raw as isize),
            )
        } {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = unsafe { Box::from_raw(raw) };
                Err(error)
            }
        }
    }

    fn draw(&self) -> Result<()> {
        let active_stroke = self.input.active_stroke();
        let eraser = self.input.eraser_render_state();
        let laser = self.input.laser_render_state();
        let strokes = self.current_strokes();
        let selected_ids = self
            .app_state
            .borrow()
            .selected_stroke_ids(&self.monitor.session_key);
        let selection = self.input.selection_render_state(
            &strokes,
            &selected_ids,
            edutools_core::ink::Size {
                width: f64::from(self.monitor.rect.width()),
                height: f64::from(self.monitor.rect.height()),
            },
        );
        self.renderer
            .as_ref()
            .expect("renderer initialized after window creation")
            .draw_frame(
                active_stroke.as_ref(),
                eraser.as_ref(),
                laser.as_ref(),
                selection.as_ref(),
                self.board.as_ref(),
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
        let layers = self.layers.borrow();
        let layer = layers
            .get(&self.monitor.session_key)
            .expect("overlay monitor layer is initialized");
        self.app_state
            .borrow()
            .strokes_for(&self.monitor.session_key, layer)
            .to_vec()
    }

    fn execute_ink_command(&mut self, command: InkCommand) {
        let display_id = self.monitor.session_key.clone();
        let mut layers = self.layers.borrow_mut();
        let layer = layers.entry(display_id.clone()).or_default();
        let route = self
            .app_state
            .borrow_mut()
            .execute_ink_command(&display_id, layer, command);
        self.logger.event(&format!(
            "스트로크 라우팅: operation=execute monitor={display_id} target={}",
            route.label()
        ));
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

    fn sync_recognition_timer(&mut self, hwnd: HWND) -> Result<()> {
        if self.input.take_recognition_timer_reset() {
            let timer = unsafe {
                SetTimer(
                    Some(hwnd),
                    RECOGNITION_TIMER_ID,
                    RECOGNITION_TIMER_INTERVAL_MS,
                    None,
                )
            };
            if timer == 0 {
                return Err(Error::new(E_FAIL, "도형 인식 타이머 시작 실패"));
            }
            self.recognition_timer_running = true;
        } else if self.recognition_timer_running && !self.input.recognition_timer_needed() {
            unsafe { KillTimer(Some(hwnd), RECOGNITION_TIMER_ID) }?;
            self.recognition_timer_running = false;
        }
        Ok(())
    }

    fn recognition_timer_fired(&mut self, hwnd: HWND) -> Result<()> {
        if self.recognition_timer_running {
            unsafe { KillTimer(Some(hwnd), RECOGNITION_TIMER_ID) }?;
            self.recognition_timer_running = false;
        }
        self.input.recognize_active();
        Ok(())
    }

    fn handle_pointer_message(
        &mut self,
        hwnd: HWND,
        message: u32,
        pointer_id: u32,
        mapping: InkInputMapping,
    ) -> Result<()> {
        if message == WM_POINTERDOWN {
            self.app_state
                .borrow_mut()
                .set_active_display_if_annotating(self.monitor.session_key.clone());
        }
        let strokes = self.current_strokes();
        let selected_ids = self
            .app_state
            .borrow()
            .selected_stroke_ids(&self.monitor.session_key);
        self.input
            .handle_message(
                message,
                pointer_id,
                &self.monitor,
                &strokes,
                &selected_ids,
                mapping,
            )
            .and_then(|outcome| self.apply_input_outcome(outcome))
            .and_then(|()| self.sync_laser_timer(hwnd))
            .and_then(|()| self.sync_recognition_timer(hwnd))
    }
}

struct OverlayWindow {
    hwnd: HWND,
    state: Box<OverlayWindowState>,
    _band: BandRegistration,
}

impl OverlayWindow {
    #[allow(clippy::too_many_arguments)]
    fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        monitor: Monitor,
        graphics: &GraphicsDevice,
        layers: LayerStore,
        app_state: SharedAppState,
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
            app_state,
            renderer: None,
            board: None,
            debug_visible: annotation_enabled && debug_overlay,
            laser_timer_running: false,
            recognition_timer_running: false,
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
        let band = BandRegistration::new(hwnd, Band::Overlay);
        if let Err(error) = unsafe {
            SetWindowPos(
                hwnd,
                None,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
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
        let mut window = Self {
            hwnd,
            state,
            _band: band,
        };
        window.rebuild_completed()?;
        window.set_click_through(!annotation_enabled)?;
        window.draw(annotation_enabled && debug_overlay)?;
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        Ok(window)
    }

    fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn handle_pointer_message(
        &mut self,
        message: u32,
        pointer_id: u32,
        mapping: InkInputMapping,
    ) -> Result<()> {
        self.state
            .handle_pointer_message(self.hwnd, message, pointer_id, mapping)
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
        self.state.sync_laser_timer(self.hwnd)?;
        self.state.sync_recognition_timer(self.hwnd)
    }

    fn finish_active(&mut self, reason: &str) -> Result<()> {
        self.state.finish_active(reason)?;
        self.state.sync_laser_timer(self.hwnd)?;
        self.state.sync_recognition_timer(self.hwnd)
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
        self.state.current_strokes()
    }

    fn set_board(&mut self, board: Option<BoardRenderState>) -> Result<()> {
        self.state.board = board;
        self.rebuild_completed()?;
        self.state.draw()
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

    fn add_text_stroke(&mut self, commit: TextEntryCommit) -> Result<Size> {
        let text_size = text_stroke_size(commit.measured, commit.request.font_size);
        let scale = crate::render::px_per_dip(commit.request.dpi);
        let canvas = commit.request.monitor_size;
        let start = commit.request.origin;
        let end = Point {
            x: start.x + text_size.width * scale,
            y: start.y + text_size.height * scale,
        };
        let sequence = TEXT_STROKE_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
        let stroke = InkStroke::new(
            format!("win-text-{sequence}"),
            InkStrokeTool::Text,
            commit.request.color,
            1.0,
            false,
            vec![
                InkPoint::new(
                    edutools_core::ink::InkGeometry::normalized_point(start, canvas),
                    1.0,
                    sequence as f64,
                ),
                InkPoint::new(
                    edutools_core::ink::InkGeometry::normalized_point(end, canvas),
                    1.0,
                    sequence as f64,
                ),
            ],
            InkStrokeContent::Text(InkTextContent {
                string: commit.string,
                font_size: commit.request.font_size,
            }),
        );
        self.state
            .execute_ink_command(InkCommand::AddStroke(stroke.clone()));
        self.state
            .renderer
            .as_ref()
            .expect("renderer initialized after window creation")
            .append_completed(
                &stroke,
                self.state.input.pressure_minimum_width_ratio(),
                self.state.input.highlighter_opacity(),
            )?;
        self.state.draw()?;
        Ok(text_size)
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
        renderer.draw_frame(
            None,
            None,
            None,
            None,
            None,
            ratio,
            highlighter_opacity,
            false,
        )?;
        renderer.rebuild_completed(&self.strokes(), ratio, highlighter_opacity)?;
        self.state.draw()?;
        Ok(pixel_failures)
    }
}

fn color_components(color: &SettingsColor) -> [f64; 4] {
    [color.red, color.green, color.blue, color.alpha]
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
        if self.state.recognition_timer_running {
            if let Err(error) = unsafe { KillTimer(Some(self.hwnd), RECOGNITION_TIMER_ID) } {
                self.state
                    .logger
                    .event(&format!("도형 인식 타이머 해제 실패: {error}"));
            }
            self.state.recognition_timer_running = false;
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
                let result = state.handle_pointer_message(
                    hwnd,
                    message,
                    pointer_id,
                    InkInputMapping::Identity,
                );
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
                    .and_then(|()| state.sync_laser_timer(hwnd))
                    .and_then(|()| state.sync_recognition_timer(hwnd));
                if let Err(error) = result {
                    state.logger.event(&format!(
                        "레이저 줄이기 타이머 실패: monitor={}: {error}",
                        state.monitor.session_key
                    ));
                }
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == RECOGNITION_TIMER_ID => {
                if let Err(error) = state.recognition_timer_fired(hwnd) {
                    state.logger.event(&format!(
                        "도형 인식 타이머 실패: monitor={}: {error}",
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
