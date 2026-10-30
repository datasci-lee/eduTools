//! Windows 원형 팔레트 창, 포인터 상태기계, D2D 렌더러.

#[cfg(windows)]
mod windows_impl {
    use crate::app_icon::{AppIconRenderer, IconTint};
    use crate::log::Logger;
    use crate::monitor::{enumerate_monitors, monitor_at_point, Monitor};
    use crate::palette::{app_data_file, load_positions, write_file};
    use crate::render::{
        create_swap_chain, create_swap_target, pixel_format, px_per_dip, read_bitmap_bgra,
        GraphicsDevice,
    };
    use crate::z_bands::{raise_band_order, Band, BandRegistration};
    use edutools_core::board::BoardKind;
    use edutools_core::ink::{InkTool, Point, Rect};
    use edutools_core::radial::{
        group_definitions, hit_target, primary_intent, radial_drag_threshold_reached,
        radial_palette_should_show, render_state, secondary_intent, RadialColor,
        RadialPaletteArcStyle, RadialPaletteGroupId, RadialPaletteHitTarget, RadialPaletteIntent,
        RadialPalettePreview, RadialPaletteRenderCell, RadialPaletteRenderState, RadialPaletteRing,
        RadialPaletteSectorGeometry, RadialPaletteSectorPath, RadialPaletteState, CENTER_RADIUS,
        COLLAPSED_DIAMETER, ENTRY_INNER_RADIUS, ENTRY_OUTER_RADIUS, EXPANDED_DIAMETER,
        GROUP_INNER_RADIUS, GROUP_OUTER_RADIUS, OPTION_INNER_RADIUS, OPTION_OUTER_RADIUS,
        PROPERTY_GAP, SECTOR_GAP,
    };
    use edutools_core::screen_position::{
        restored_origin, saved_ratio, PositionRatio, ScreenFrame, ScreenPositionDocument,
        WindowOrigin, WindowSize,
    };
    use edutools_core::settings::{AppSettings, PaletteStyle};
    use edutools_core::shortcut::ShortcutBoard;
    use edutools_core::state::AppState;
    use std::f64::consts::PI;
    use std::ffi::c_void;
    use std::path::PathBuf;
    use windows::core::{w, Error, Interface, Result};
    use windows::Win32::Foundation::{
        ERROR_CLASS_ALREADY_EXISTS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
    };
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
        D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_F, D2D_SIZE_U,
    };
    use windows::Win32::Graphics::Direct2D::{
        ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory, ID2D1GeometrySink, ID2D1Image,
        ID2D1PathGeometry, ID2D1RenderTarget, ID2D1StrokeStyle, D2D1_ARC_SEGMENT,
        D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL, D2D1_BITMAP_OPTIONS_TARGET,
        D2D1_BITMAP_PROPERTIES1, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
        D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_LINE_JOIN_ROUND, D2D1_PRIMITIVE_BLEND_COPY,
        D2D1_PRIMITIVE_BLEND_SOURCE_OVER, D2D1_STROKE_STYLE_PROPERTIES,
        D2D1_SWEEP_DIRECTION_CLOCKWISE, D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
    };
    use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, DWRITE_FACTORY_TYPE_SHARED,
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_MEDIUM,
        DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
    use windows::Win32::Graphics::Dxgi::IDXGISwapChain1;
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::Pointer::{
        GetPointerInfo, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE, POINTER_FLAG_SECONDBUTTON,
        POINTER_INFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, PostMessageW,
        RegisterClassW, SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW, GWLP_USERDATA,
        GWL_EXSTYLE, GWL_STYLE, HTCLIENT, MA_NOACTIVATE, PA_NOACTIVATE, SWP_NOACTIVATE,
        SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, WM_DPICHANGED, WM_ERASEBKGND, WM_MOUSEACTIVATE,
        WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_POINTERACTIVATE, WM_POINTERCAPTURECHANGED,
        WM_POINTERDOWN, WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE, WM_SETTINGCHANGE,
        WM_SYSCOLORCHANGE, WM_THEMECHANGED, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    macro_rules! d2d_point {
        ($x:expr, $y:expr) => {{
            let mut point = D2D1_ELLIPSE::default().point;
            point.X = $x as f32;
            point.Y = $y as f32;
            point
        }};
    }

    pub const WM_APP_RADIAL_ACTION: u32 = 0x8000 + 8;
    pub const WM_APP_RADIAL_REDRAW: u32 = 0x8000 + 9;
    pub const WM_APP_RADIAL_SAVE_POSITION: u32 = 0x8000 + 10;
    pub const WM_APP_RADIAL_COLLAPSE: u32 = 0x8000 + 11;

    const WINDOW_CLASS: windows::core::PCWSTR = w!("EduToolsRadialPalette");
    const DEFAULT_POSITION: PositionRatio = PositionRatio { x: 0.5, y: 0.15 };
    const STORAGE_PREFIX: &str = "radial::";
    const GROUP_ICON_SIZE: f64 = 13.0;
    const ENTRY_ICON_SIZE: f64 = 14.0;
    const CENTER_ICON_SIZE: f64 = 18.0;
    const OPTION_TRACK_RADIUS: f64 = OPTION_INNER_RADIUS + 6.0;
    const OPTION_NODE_RADIUS: f64 = (OPTION_INNER_RADIUS + OPTION_OUTER_RADIUS) / 2.0;
    const ACTIVE_ARC_WIDTH: f64 = 3.0;
    const INACTIVE_ARC_WIDTH: f64 = 3.0;

    struct PendingAction {
        target: String,
        intent: RadialPaletteIntent,
        secondary: bool,
    }

    struct PointerPress {
        pointer_id: u32,
        target: String,
        center: bool,
        screen_start: POINT,
        resting_center_start: POINT,
        dragging: bool,
        secondary: bool,
    }

    struct RadialPaletteWindowState {
        owner: HWND,
        logger: Logger,
        dpi: u32,
        radial: RadialPaletteState,
        hover: Option<String>,
        press: Option<PointerPress>,
        pending_action: Option<PendingAction>,
        resting_center: POINT,
        last_render: Option<RadialPaletteRenderState>,
        renderer: Option<RadialPaletteRenderer>,
    }

    struct RadialPaletteWindow {
        hwnd: HWND,
        state: Box<RadialPaletteWindowState>,
        _band: BandRegistration,
    }

    pub struct RadialPaletteManager {
        window: RadialPaletteWindow,
        _graphics: GraphicsDevice,
        positions: ScreenPositionDocument,
        positions_path: Option<PathBuf>,
        visible: bool,
    }

    impl RadialPaletteManager {
        pub fn new(
            owner: HWND,
            hinstance: HINSTANCE,
            state: &AppState,
            settings: &AppSettings,
            logger: Logger,
        ) -> Result<Self> {
            register_window_class(hinstance)?;
            let positions_path = app_data_file("positions.json");
            let positions = load_positions(positions_path.as_deref(), &logger);
            let monitors = enumerate_monitors()?;
            let monitor =
                target_monitor(&monitors, positions.last_display_key()).ok_or_else(|| {
                    Error::new(E_FAIL, "radial palette requires at least one monitor")
                })?;
            let dpi = monitor.dpi.0.max(96);
            let diameter = pixels(COLLAPSED_DIAMETER, dpi);
            let origin = restored_palette_origin(&positions, monitor, diameter);
            let resting_center = POINT {
                x: origin.x.round() as i32 + diameter / 2,
                y: origin.y.round() as i32 + diameter / 2,
            };
            logger.event(&format!(
                "원형 팔레트 위치 복원: monitor={} ratio={:?} origin=({}, {})",
                monitor.session_key,
                positions.ratio_for(Some(&storage_key(&monitor.session_key))),
                origin.x.round() as i32,
                origin.y.round() as i32
            ));
            let mut radial = RadialPaletteState::from_settings(settings);
            refresh_radial_state(&mut radial, state, settings, true);
            let mut window_state = Box::new(RadialPaletteWindowState {
                owner,
                logger: logger.clone(),
                dpi,
                radial,
                hover: None,
                press: None,
                pending_action: None,
                resting_center,
                last_render: None,
                renderer: None,
            });
            let state_ptr = window_state.as_mut() as *mut RadialPaletteWindowState;
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP,
                    WINDOW_CLASS,
                    w!("EduTools Radial Palette"),
                    WS_POPUP,
                    origin.x.round() as i32,
                    origin.y.round() as i32,
                    diameter,
                    diameter,
                    None,
                    None,
                    Some(hinstance),
                    Some(state_ptr.cast::<c_void>()),
                )
            }?;
            let band = BandRegistration::new(hwnd, Band::Tool);
            let graphics = match GraphicsDevice::new() {
                Ok(graphics) => graphics,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            let renderer = match RadialPaletteRenderer::new(
                &graphics, hwnd, diameter, diameter, dpi, logger,
            ) {
                Ok(renderer) => renderer,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            window_state.renderer = Some(renderer);
            Ok(Self {
                window: RadialPaletteWindow {
                    hwnd,
                    state: window_state,
                    _band: band,
                },
                _graphics: graphics,
                positions,
                positions_path,
                visible: false,
            })
        }

        pub fn hwnd(&self) -> HWND {
            self.window.hwnd
        }

        pub fn is_expanded(&self) -> bool {
            self.window.state.radial.is_expanded
        }

        pub fn refresh(
            &mut self,
            state: &AppState,
            settings: &AppSettings,
            reason: &str,
        ) -> Result<()> {
            refresh_radial_state(&mut self.window.state.radial, state, settings, false);
            let visibility_reason = if !state.is_annotation_mode_enabled() {
                "판서 꺼짐"
            } else if !state.is_palette_visible() {
                "palette.toggle 숨김"
            } else if settings.tools.palette_style != PaletteStyle::Radial {
                "스타일 Linear"
            } else {
                reason
            };
            let visible = radial_palette_should_show(
                state.is_annotation_mode_enabled(),
                state.is_palette_visible(),
                settings.tools.palette_style,
            );
            if !visible && self.is_expanded() {
                self.set_expanded(false, visibility_reason)?;
            }
            self.redraw()?;
            if visible {
                let _ = unsafe { ShowWindow(self.window.hwnd, SW_SHOWNOACTIVATE) };
                if !self.visible {
                    raise_band_order()?;
                    self.window
                        .state
                        .logger
                        .event(&format!("원형 팔레트 표시: {visibility_reason}"));
                }
            } else {
                let _ = unsafe { ShowWindow(self.window.hwnd, SW_HIDE) };
                if self.visible {
                    self.window
                        .state
                        .logger
                        .event(&format!("원형 팔레트 숨김: {visibility_reason}"));
                }
            }
            self.visible = visible;
            Ok(())
        }

        pub fn redraw(&mut self) -> Result<()> {
            let render = render_state(
                &self.window.state.radial,
                false,
                self.window.state.hover.as_deref(),
                None,
            );
            if self.window.state.last_render.as_ref() == Some(&render) {
                return Ok(());
            }
            self.window
                .state
                .renderer
                .as_mut()
                .expect("radial palette renderer exists")
                .render(&render)?;
            self.window.state.last_render = Some(render);
            Ok(())
        }

        pub fn toggle_expansion(&mut self, reason: &str) -> Result<()> {
            let expanded = !self.is_expanded();
            self.set_expanded(expanded, reason)
        }

        pub fn collapse(&mut self, reason: &str) -> Result<bool> {
            if !self.is_expanded() {
                return Ok(false);
            }
            self.set_expanded(false, reason)?;
            Ok(true)
        }

        pub fn select_group(&mut self, group: RadialPaletteGroupId) -> Result<()> {
            self.window.state.radial.selected_group = group;
            self.window.state.radial.selected_entry_identifier = None;
            self.redraw()
        }

        pub fn take_pending_action(&mut self) -> Option<(String, RadialPaletteIntent, bool)> {
            self.window
                .state
                .pending_action
                .take()
                .map(|pending| (pending.target, pending.intent, pending.secondary))
        }

        pub fn save_position(&mut self) -> Result<()> {
            let monitors = enumerate_monitors()?;
            let center = self.window.state.resting_center;
            let monitor = monitor_at_point(&monitors, center)
                .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "radial palette position has no monitor"))?;
            let collapsed = pixels(COLLAPSED_DIAMETER, monitor.dpi.0.max(96));
            let origin = WindowOrigin {
                x: f64::from(center.x - collapsed / 2),
                y: f64::from(center.y - collapsed / 2),
            };
            let ratio = saved_ratio(
                origin,
                screen_frame(monitor),
                WindowSize {
                    width: f64::from(collapsed),
                    height: f64::from(collapsed),
                },
            );
            self.positions =
                load_positions(self.positions_path.as_deref(), &self.window.state.logger);
            self.positions.save_with_display_key(
                ratio,
                storage_key(&monitor.session_key),
                monitor.session_key.clone(),
            );
            self.persist_positions()?;
            self.window.state.logger.event(&format!(
                "원형 팔레트 끌기 종료: monitor={} ratio=({:.4}, {:.4})",
                monitor.session_key, ratio.x, ratio.y
            ));
            Ok(())
        }

        pub fn monitor_key(&self) -> Result<Option<String>> {
            let monitors = enumerate_monitors()?;
            Ok(
                monitor_at_point(&monitors, self.window.state.resting_center)
                    .map(|monitor| monitor.session_key.clone()),
            )
        }

        pub fn display_configuration_changed(
            &mut self,
            state: &AppState,
            settings: &AppSettings,
        ) -> Result<()> {
            self.restore_position()?;
            self.refresh(state, settings, "모니터 구성 변경")
        }

        pub fn selftest_failures(
            &mut self,
            state: &AppState,
            settings: &AppSettings,
        ) -> Vec<String> {
            let mut failures = Vec::new();
            let required_ex =
                (WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP).0
                    as isize;
            let actual_ex = unsafe { GetWindowLongPtrW(self.window.hwnd, GWL_EXSTYLE) };
            if actual_ex & required_ex != required_ex {
                failures.push(format!(
                    "radial palette: 필수 확장 스타일 누락 ({actual_ex:#x})"
                ));
            }
            let actual_style = unsafe { GetWindowLongPtrW(self.window.hwnd, GWL_STYLE) };
            if actual_style & WS_POPUP.0 as isize == 0 {
                failures.push(format!("radial palette: WS_POPUP 누락 ({actual_style:#x})"));
            }
            refresh_radial_state(&mut self.window.state.radial, state, settings, true);
            let renderer = self
                .window
                .state
                .renderer
                .as_mut()
                .expect("radial palette renderer exists");
            match renderer.selftest_pixel_failures(&self.window.state.radial) {
                Ok(pixel_failures) => failures.extend(pixel_failures),
                Err(error) => failures.push(format!("radial palette: 픽셀 readback 실패: {error}")),
            }
            failures
        }

        fn set_expanded(&mut self, expanded: bool, reason: &str) -> Result<()> {
            if self.window.state.radial.is_expanded == expanded {
                return Ok(());
            }
            if expanded {
                synchronize_path(&mut self.window.state.radial);
            }
            self.window.state.radial.is_expanded = expanded;
            self.window.state.hover = None;
            self.window.state.press = None;
            apply_window_frame(self.window.hwnd, &mut self.window.state)?;
            self.redraw()?;
            self.window.state.logger.event(&format!(
                "원형 팔레트 {}: {reason}",
                if expanded { "펼침" } else { "접힘" }
            ));
            Ok(())
        }

        fn restore_position(&mut self) -> Result<()> {
            self.positions =
                load_positions(self.positions_path.as_deref(), &self.window.state.logger);
            let monitors = enumerate_monitors()?;
            let monitor = target_monitor(&monitors, self.positions.last_display_key())
                .ok_or_else(|| Error::new(E_FAIL, "cannot restore radial palette"))?;
            let dpi = monitor.dpi.0.max(96);
            self.window.state.dpi = dpi;
            let collapsed = pixels(COLLAPSED_DIAMETER, dpi);
            let origin = restored_palette_origin(&self.positions, monitor, collapsed);
            self.window.state.resting_center = POINT {
                x: origin.x.round() as i32 + collapsed / 2,
                y: origin.y.round() as i32 + collapsed / 2,
            };
            apply_window_frame(self.window.hwnd, &mut self.window.state)?;
            self.window.state.logger.event(&format!(
                "원형 팔레트 위치 복원/클램프: monitor={} origin=({}, {})",
                monitor.session_key,
                origin.x.round() as i32,
                origin.y.round() as i32
            ));
            Ok(())
        }

        fn persist_positions(&self) -> Result<()> {
            let Some(path) = self.positions_path.as_deref() else {
                return Err(Error::new(
                    E_FAIL,
                    "APPDATA is unavailable for radial positions.json",
                ));
            };
            let json = self
                .positions
                .to_json()
                .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
            write_file(path, json.as_bytes(), "positions.json")
        }
    }

    impl Drop for RadialPaletteWindow {
        fn drop(&mut self) {
            self.state.renderer.take();
            if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
                self.state
                    .logger
                    .event(&format!("원형 팔레트 창 제거 실패: {error}"));
            }
        }
    }

    fn refresh_radial_state(
        radial: &mut RadialPaletteState,
        state: &AppState,
        settings: &AppSettings,
        force_path_sync: bool,
    ) {
        let expanded = radial.is_expanded;
        let selected_group = radial.selected_group;
        let selected_entry = radial.selected_entry_identifier.clone();
        let old_tool = radial.current_tool;
        let old_eraser_mode = radial.eraser_mode;
        let mut refreshed = RadialPaletteState::from_settings(settings);
        refreshed.is_expanded = expanded;
        refreshed.selected_group = selected_group;
        refreshed.selected_entry_identifier = selected_entry;
        refreshed.current_tool = state.current_tool();
        refreshed.selected_color_index = usize::try_from(state.selected_color_index(settings))
            .unwrap_or_default()
            .min(refreshed.color_presets.len().saturating_sub(1));
        refreshed.current_favorite_snapshot =
            state.current_favorite_snapshot(settings).unwrap_or(None);
        refreshed.is_pass_through_enabled = state.is_pass_through_mode_enabled();
        refreshed.is_zoom_enabled = state.zoom_session().is_some();
        refreshed.presented_board = state.presented_board().map(|board| match board.kind {
            BoardKind::White => ShortcutBoard::White,
            BoardKind::Black => ShortcutBoard::Black,
        });
        let path_changed =
            old_tool != refreshed.current_tool || old_eraser_mode != refreshed.eraser_mode;
        *radial = refreshed;
        if force_path_sync || path_changed {
            synchronize_path(radial);
        }
    }

    fn synchronize_path(state: &mut RadialPaletteState) {
        let current_tool = state.current_tool;
        let eraser_mode = state.eraser_mode;
        for definition in group_definitions(state) {
            if let Some(entry) = definition.entries.iter().find(|entry| {
                entry.tool == Some(current_tool)
                    && (current_tool != InkTool::Eraser || entry.eraser_mode == Some(eraser_mode))
            }) {
                state.selected_group = definition.id;
                state.selected_entry_identifier = Some(entry.identifier.clone());
                return;
            }
        }
    }

    fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
        let class = WNDCLASSW {
            lpfnWndProc: Some(radial_window_proc),
            hInstance: hinstance,
            lpszClassName: WINDOW_CLASS,
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

    unsafe extern "system" fn radial_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            let state = create.lpCreateParams as *mut RadialPaletteWindowState;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
        }
        let state =
            unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RadialPaletteWindowState };
        if !state.is_null() {
            let state = unsafe { &mut *state };
            match message {
                WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {
                    let pointer_id = (wparam.0 as u32) & 0xffff;
                    if let Err(error) = handle_pointer(hwnd, state, message, pointer_id) {
                        state.logger.event(&format!(
                            "원형 팔레트 포인터 처리 실패: pointerId={pointer_id} message={message:#x}: {error}"
                        ));
                    }
                    return LRESULT(0);
                }
                WM_POINTERLEAVE => {
                    if state.press.is_none() && state.hover.take().is_some() {
                        post_owner(state, WM_APP_RADIAL_REDRAW);
                    }
                    return LRESULT(0);
                }
                WM_POINTERCAPTURECHANGED => {
                    if state.press.take().is_some() {
                        state
                            .logger
                            .event("원형 팔레트 포인터 캡처 해제: 눌림/끌기 초기화");
                    }
                    post_owner(state, WM_APP_RADIAL_REDRAW);
                    return LRESULT(0);
                }
                WM_DPICHANGED => {
                    state.dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
                    if let Err(error) = apply_window_frame(hwnd, state) {
                        state
                            .logger
                            .event(&format!("원형 팔레트 WM_DPICHANGED 실패: {error}"));
                    }
                    post_owner(state, WM_APP_RADIAL_REDRAW);
                    return LRESULT(0);
                }
                WM_NCHITTEST => return LRESULT(HTCLIENT as isize),
                WM_POINTERACTIVATE => return LRESULT(PA_NOACTIVATE as isize),
                WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
                WM_SETTINGCHANGE => {
                    if let Err(error) = apply_window_frame(hwnd, state) {
                        state
                            .logger
                            .event(&format!("원형 팔레트 작업 영역 변경 반영 실패: {error}"));
                    }
                    post_owner(state, WM_APP_RADIAL_REDRAW);
                    return LRESULT(0);
                }
                WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
                    post_owner(state, WM_APP_RADIAL_REDRAW);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => return LRESULT(1),
                WM_NCDESTROY => unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                },
                _ => {}
            }
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn handle_pointer(
        hwnd: HWND,
        state: &mut RadialPaletteWindowState,
        message: u32,
        pointer_id: u32,
    ) -> Result<()> {
        let (info, client) = pointer_snapshot(hwnd, pointer_id)?;
        let diameter = current_diameter(&state.radial);
        let scale = px_per_dip(state.dpi);
        let dip = Point {
            x: f64::from(client.x) / scale,
            y: diameter - f64::from(client.y) / scale,
        };
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: diameter,
            height: diameter,
        };
        let hit = hit_target(&state.radial, dip, bounds);
        let hit_identifier = hit.as_ref().map(|target| target.identifier().to_owned());

        if message == WM_POINTERDOWN {
            if state.press.is_some() {
                return Ok(());
            }
            let secondary =
                info.pointerFlags & POINTER_FLAG_SECONDBUTTON == POINTER_FLAG_SECONDBUTTON;
            let Some(target) = hit else {
                if state.radial.is_expanded {
                    post_owner(state, WM_APP_RADIAL_COLLAPSE);
                }
                return Ok(());
            };
            let center = matches!(target, RadialPaletteHitTarget::Center);
            state.press = Some(PointerPress {
                pointer_id,
                target: target.identifier().to_owned(),
                center,
                screen_start: info.ptPixelLocation,
                resting_center_start: state.resting_center,
                dragging: false,
                secondary,
            });
            if state.hover.as_deref() != Some(target.identifier()) {
                state.hover = Some(target.identifier().to_owned());
                post_owner(state, WM_APP_RADIAL_REDRAW);
            }
            return Ok(());
        }

        if message == WM_POINTERUPDATE {
            let in_range = info.pointerFlags & POINTER_FLAG_INRANGE == POINTER_FLAG_INRANGE;
            let in_contact = info.pointerFlags & POINTER_FLAG_INCONTACT == POINTER_FLAG_INCONTACT;
            if !in_contact && in_range && state.hover != hit_identifier {
                state.hover = hit_identifier;
                post_owner(state, WM_APP_RADIAL_REDRAW);
            }
            if let Some(press) = state.press.as_mut() {
                if press.pointer_id == pointer_id && press.center && !press.secondary {
                    let dx = info.ptPixelLocation.x - press.screen_start.x;
                    let dy = info.ptPixelLocation.y - press.screen_start.y;
                    if press.dragging
                        || radial_drag_threshold_reached(
                            f64::from(dx),
                            f64::from(dy),
                            px_per_dip(state.dpi),
                        )
                    {
                        if !press.dragging {
                            press.dragging = true;
                            state.logger.event("원형 팔레트 끌기 시작");
                        }
                        let proposed = POINT {
                            x: press.resting_center_start.x + dx,
                            y: press.resting_center_start.y + dy,
                        };
                        move_palette_center(hwnd, state, proposed, info.ptPixelLocation)?;
                    }
                }
            }
            return Ok(());
        }

        let Some(press) = state.press.take() else {
            return Ok(());
        };
        if press.pointer_id != pointer_id {
            state.press = Some(press);
            return Ok(());
        }
        if press.dragging {
            post_owner(state, WM_APP_RADIAL_SAVE_POSITION);
        } else if hit_identifier.as_deref() == Some(press.target.as_str()) {
            let intent = if press.secondary {
                hit.as_ref().and_then(secondary_intent)
            } else {
                hit.as_ref()
                    .and_then(|target| primary_intent(target, &state.radial))
            };
            if let Some(intent) = intent {
                state.pending_action = Some(PendingAction {
                    target: press.target,
                    intent,
                    secondary: press.secondary,
                });
                post_owner(state, WM_APP_RADIAL_ACTION);
            }
        }
        if state.hover != hit_identifier {
            state.hover = hit_identifier;
            post_owner(state, WM_APP_RADIAL_REDRAW);
        }
        Ok(())
    }

    fn pointer_snapshot(hwnd: HWND, pointer_id: u32) -> Result<(POINTER_INFO, POINT)> {
        let mut info = POINTER_INFO::default();
        unsafe { GetPointerInfo(pointer_id, &mut info) }?;
        let mut client = info.ptPixelLocation;
        if !unsafe { ScreenToClient(hwnd, &mut client) }.as_bool() {
            return Err(Error::from_thread());
        }
        Ok((info, client))
    }

    fn move_palette_center(
        hwnd: HWND,
        state: &mut RadialPaletteWindowState,
        proposed: POINT,
        pointer: POINT,
    ) -> Result<()> {
        let monitors = enumerate_monitors()?;
        let monitor = monitor_at_point(&monitors, pointer)
            .or_else(|| monitor_at_point(&monitors, proposed))
            .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
            .or_else(|| monitors.first())
            .ok_or_else(|| Error::new(E_FAIL, "radial drag has no monitor"))?;
        let collapsed = pixels(COLLAPSED_DIAMETER, monitor.dpi.0.max(96));
        let clamped = clamped_center(proposed, collapsed, monitor);
        if clamped != proposed {
            state.logger.event(&format!(
                "원형 팔레트 위치 클램프: proposed=({}, {}) clamped=({}, {})",
                proposed.x, proposed.y, clamped.x, clamped.y
            ));
        }
        state.resting_center = clamped;
        apply_window_frame_on(hwnd, state, monitor)
    }

    fn apply_window_frame(hwnd: HWND, state: &mut RadialPaletteWindowState) -> Result<()> {
        let monitors = enumerate_monitors()?;
        let monitor = monitor_at_point(&monitors, state.resting_center)
            .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
            .or_else(|| monitors.first())
            .ok_or_else(|| Error::new(E_FAIL, "radial palette has no monitor"))?;
        apply_window_frame_on(hwnd, state, monitor)
    }

    fn apply_window_frame_on(
        hwnd: HWND,
        state: &mut RadialPaletteWindowState,
        monitor: &Monitor,
    ) -> Result<()> {
        let dpi = monitor.dpi.0.max(96);
        state.dpi = dpi;
        let diameter = pixels(current_diameter(&state.radial), dpi);
        let displayed = clamped_center(state.resting_center, diameter, monitor);
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                displayed.x - diameter / 2,
                displayed.y - diameter / 2,
                diameter,
                diameter,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }?;
        state
            .renderer
            .as_mut()
            .expect("radial palette renderer exists")
            .resize(diameter, diameter, dpi)?;
        state.last_render = None;
        Ok(())
    }

    fn clamped_center(center: POINT, diameter: i32, monitor: &Monitor) -> POINT {
        let leading_radius = diameter / 2;
        let trailing_radius = diameter - leading_radius;
        let work = monitor.work_rect;
        let x = if work.width() >= diameter {
            center
                .x
                .clamp(work.left + leading_radius, work.right - trailing_radius)
        } else {
            work.left + work.width() / 2
        };
        let y = if work.height() >= diameter {
            center
                .y
                .clamp(work.top + leading_radius, work.bottom - trailing_radius)
        } else {
            work.top + work.height() / 2
        };
        POINT { x, y }
    }

    fn post_owner(state: &RadialPaletteWindowState, message: u32) {
        if let Err(error) =
            unsafe { PostMessageW(Some(state.owner), message, WPARAM(0), LPARAM(0)) }
        {
            state.logger.event(&format!(
                "원형 팔레트 owner 메시지 전송 실패: {message:#x}: {error}"
            ));
        }
    }

    struct RadialPaletteRenderer {
        swap_chain: IDXGISwapChain1,
        context: ID2D1DeviceContext,
        factory: ID2D1Factory,
        round_stroke: ID2D1StrokeStyle,
        swap_target: Option<ID2D1Bitmap1>,
        icons: AppIconRenderer,
        dwrite: IDWriteFactory,
        _dcomp_target: IDCompositionTarget,
        _visual: IDCompositionVisual,
        width: u32,
        height: u32,
        dpi: u32,
    }

    impl RadialPaletteRenderer {
        fn new(
            graphics: &GraphicsDevice,
            hwnd: HWND,
            width: i32,
            height: i32,
            dpi: u32,
            logger: Logger,
        ) -> Result<Self> {
            let width = positive_dimension(width)?;
            let height = positive_dimension(height)?;
            let swap_chain = create_swap_chain(graphics, width, height)?;
            let context = graphics.create_d2d_context()?;
            unsafe { context.SetDpi(96.0, 96.0) };
            let factory = unsafe { context.GetFactory() }?;
            let stroke_properties = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let round_stroke = unsafe { factory.CreateStrokeStyle(&stroke_properties, None) }?;
            let swap_target = Some(create_swap_target(&context, &swap_chain)?);
            set_context_target(
                &context,
                swap_target
                    .as_ref()
                    .expect("radial palette swap target exists"),
            )?;
            let (dcomp_target, visual) =
                graphics.attach_composition_swap_chain(hwnd, &swap_chain)?;
            let icons = AppIconRenderer::new(context.clone(), logger)?;
            let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
            Ok(Self {
                swap_chain,
                context,
                factory,
                round_stroke,
                swap_target,
                icons,
                dwrite,
                _dcomp_target: dcomp_target,
                _visual: visual,
                width,
                height,
                dpi,
            })
        }

        fn resize(&mut self, width: i32, height: i32, dpi: u32) -> Result<()> {
            let width = positive_dimension(width)?;
            let height = positive_dimension(height)?;
            if self.width == width && self.height == height && self.dpi == dpi {
                return Ok(());
            }
            unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
            self.swap_target = None;
            if self.width != width || self.height != height {
                unsafe {
                    self.swap_chain.ResizeBuffers(
                        0,
                        width,
                        height,
                        DXGI_FORMAT_UNKNOWN,
                        Default::default(),
                    )
                }?;
            }
            self.swap_target = Some(create_swap_target(&self.context, &self.swap_chain)?);
            set_context_target(
                &self.context,
                self.swap_target
                    .as_ref()
                    .expect("radial palette swap target recreated"),
            )?;
            self.width = width;
            self.height = height;
            self.dpi = dpi;
            Ok(())
        }

        fn render(&mut self, state: &RadialPaletteRenderState) -> Result<()> {
            let target = self
                .swap_target
                .as_ref()
                .expect("radial palette swap target exists")
                .clone();
            self.render_to(&target, state)?;
            unsafe { self.swap_chain.Present(1, Default::default()).ok() }
        }

        fn selftest_pixel_failures(&mut self, radial: &RadialPaletteState) -> Result<Vec<String>> {
            let saved_width = self.width;
            let saved_height = self.height;
            let saved_dpi = self.dpi;
            let scale = px_per_dip(saved_dpi);
            let mut failures = Vec::new();

            let collapsed_size = pixels(COLLAPSED_DIAMETER, saved_dpi);
            let collapsed_bitmap =
                create_target_bitmap(&self.context, collapsed_size as u32, collapsed_size as u32)?;
            let mut collapsed = radial.clone();
            collapsed.is_expanded = false;
            self.width = collapsed_size as u32;
            self.height = collapsed_size as u32;
            self.render_to(
                &collapsed_bitmap,
                &render_state(&collapsed, false, None, None),
            )?;
            let (collapsed_pixels, pitch) = read_bitmap_bgra(&self.context, &collapsed_bitmap)?;
            let alpha_at = |x: u32, y: u32| -> u8 {
                collapsed_pixels[y as usize * pitch as usize + x as usize * 4 + 3]
            };
            let middle = collapsed_size as u32 / 2;
            if alpha_at(middle, middle) == 0 {
                failures.push("radial palette: 접힘 중심 alpha가 0".to_owned());
            }
            if alpha_at(0, 0) != 0 {
                failures.push("radial palette: 접힘 창 모서리 alpha가 0이 아님".to_owned());
            }

            let expanded_size = pixels(EXPANDED_DIAMETER, saved_dpi);
            let expanded_bitmap =
                create_target_bitmap(&self.context, expanded_size as u32, expanded_size as u32)?;
            let mut expanded = radial.clone();
            expanded.is_expanded = true;
            self.width = expanded_size as u32;
            self.height = expanded_size as u32;
            self.render_to(
                &expanded_bitmap,
                &render_state(&expanded, false, None, None),
            )?;
            let (pixels, pitch) = read_bitmap_bgra(&self.context, &expanded_bitmap)?;
            let alpha_at =
                |x: u32, y: u32| -> u8 { pixels[y as usize * pitch as usize + x as usize * 4 + 3] };
            let middle = expanded_size as u32 / 2;
            let gap_x = middle + (61.0 * scale).round() as u32;
            let ring_x = middle + (43.0 * scale).round() as u32;
            if alpha_at(gap_x, middle) != 0 {
                failures.push(format!(
                    "radial palette: 링 사이 틈 alpha={} (기대 0)",
                    alpha_at(gap_x, middle)
                ));
            }
            if alpha_at(ring_x, middle) == 0 {
                failures.push("radial palette: 그룹 링 alpha가 0".to_owned());
            }

            self.width = saved_width;
            self.height = saved_height;
            self.dpi = saved_dpi;
            let target = self
                .swap_target
                .as_ref()
                .expect("radial palette swap target exists");
            set_context_target(&self.context, target)?;
            Ok(failures)
        }

        fn render_to(
            &mut self,
            target: &ID2D1Bitmap1,
            state: &RadialPaletteRenderState,
        ) -> Result<()> {
            set_context_target(&self.context, target)?;
            unsafe {
                self.context.BeginDraw();
                self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                self.context.Clear(Some(&D2D1_COLOR_F::default()));
                self.context
                    .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            }
            let draw_result = self.draw_contents(state);
            let end_result = unsafe { self.context.EndDraw(None, None) };
            draw_result?;
            self.icons.handle_render_result(end_result)
        }

        fn draw_contents(&mut self, state: &RadialPaletteRenderState) -> Result<()> {
            if state.is_expanded {
                self.draw_group_ring(state)?;
                self.draw_entry_ring(state)?;
                self.draw_option_ring(state)?;
            }
            self.draw_center(state)
        }

        fn draw_group_ring(&mut self, state: &RadialPaletteRenderState) -> Result<()> {
            let scale = self.scale();
            let center = self.center();
            let target: ID2D1RenderTarget = self.context.cast()?;
            let surface = self.brush(RadialPaletteArcStyle::surface_color(state.glass_opacity))?;
            let radius = (GROUP_INNER_RADIUS + GROUP_OUTER_RADIUS) / 2.0 * scale;
            let thickness = (GROUP_OUTER_RADIUS - GROUP_INNER_RADIUS) * scale;
            unsafe {
                target.DrawEllipse(
                    &ellipse(center, radius),
                    &surface,
                    thickness as f32,
                    None::<&ID2D1StrokeStyle>,
                )
            };
            let border = self.brush(RadialPaletteArcStyle::OUTER_BORDER)?;
            unsafe {
                target.DrawEllipse(
                    &ellipse(center, GROUP_OUTER_RADIUS * scale),
                    &border,
                    (0.8 * scale) as f32,
                    None::<&ID2D1StrokeStyle>,
                )
            };

            for cell in state
                .cells
                .iter()
                .filter(|cell| cell.ring == RadialPaletteRing::Group)
            {
                if cell.is_hovered && cell.is_enabled {
                    let geometry = self.sector_geometry(cell.geometry)?;
                    let brush = self.brush(RadialPaletteArcStyle::HOVER_FILL)?;
                    unsafe {
                        target.FillGeometry(
                            &geometry,
                            &brush,
                            None::<&windows::Win32::Graphics::Direct2D::ID2D1Brush>,
                        )
                    };
                }
                if cell.is_selected {
                    self.draw_active_arc(
                        cell.geometry,
                        GROUP_OUTER_RADIUS - ACTIVE_ARC_WIDTH / 2.0,
                        cell.is_enabled,
                    )?;
                }
                self.draw_cell_icon(cell, radius / scale, GROUP_ICON_SIZE)?;
            }
            Ok(())
        }

        fn draw_entry_ring(&mut self, state: &RadialPaletteRenderState) -> Result<()> {
            let mut cells = state
                .cells
                .iter()
                .filter(|cell| cell.ring == RadialPaletteRing::Entry)
                .collect::<Vec<_>>();
            cells.sort_by(|left, right| {
                left.geometry
                    .start_angle
                    .total_cmp(&right.geometry.start_angle)
            });
            let (Some(first), Some(last)) = (cells.first(), cells.last()) else {
                return Ok(());
            };
            let orbit = (ENTRY_INNER_RADIUS + ENTRY_OUTER_RADIUS) / 2.0;
            let track = RadialPaletteSectorGeometry {
                inner_radius: orbit,
                outer_radius: orbit,
                start_angle: first.geometry.start_angle,
                end_angle: last.geometry.end_angle,
            };
            let border = self.brush(RadialPaletteArcStyle::OUTER_BORDER)?;
            self.draw_arc(track, orbit, &border, 34.0, Some(&self.round_stroke))?;
            let mut surface_color = RadialPaletteArcStyle::surface_color(state.glass_opacity);
            surface_color[3] *= 0.92;
            let surface = self.brush(surface_color)?;
            self.draw_arc(track, orbit, &surface, 32.0, Some(&self.round_stroke))?;
            let target: ID2D1RenderTarget = self.context.cast()?;
            for cell in cells {
                let node = self.cell_center(cell, orbit);
                let diameter = self.node_diameter(cell, orbit, 24.0);
                if cell.is_selected || (cell.is_hovered && cell.is_enabled) {
                    let fill = if cell.is_selected {
                        RadialPaletteArcStyle::SELECTED_NODE_FILL
                    } else {
                        RadialPaletteArcStyle::HOVER_FILL
                    };
                    let brush = self.brush(fill)?;
                    unsafe {
                        target.FillEllipse(&ellipse(node, diameter / 2.0 * self.scale()), &brush)
                    };
                }
                self.draw_cell_icon(cell, orbit, ENTRY_ICON_SIZE)?;
            }
            Ok(())
        }

        fn draw_option_ring(&mut self, state: &RadialPaletteRenderState) -> Result<()> {
            let mut cells = state
                .cells
                .iter()
                .filter(|cell| cell.ring == RadialPaletteRing::Option)
                .collect::<Vec<_>>();
            cells.sort_by(|left, right| {
                left.geometry
                    .start_angle
                    .total_cmp(&right.geometry.start_angle)
            });
            if cells.is_empty() {
                return Ok(());
            }
            let mut segments = Vec::new();
            let mut start = cells[0].geometry.start_angle;
            let mut end = cells[0].geometry.end_angle;
            let threshold = SECTOR_GAP + PROPERTY_GAP / 2.0;
            for cell in cells.iter().skip(1) {
                if cell.geometry.start_angle - end > threshold {
                    segments.push((start, end));
                    start = cell.geometry.start_angle;
                }
                end = cell.geometry.end_angle;
            }
            segments.push((start, end));
            for (start_angle, end_angle) in segments {
                let geometry = RadialPaletteSectorGeometry {
                    inner_radius: OPTION_TRACK_RADIUS,
                    outer_radius: OPTION_TRACK_RADIUS,
                    start_angle,
                    end_angle,
                };
                let shadow = self.brush(RadialPaletteArcStyle::INACTIVE_TRACK_SHADOW)?;
                self.draw_arc(
                    geometry,
                    OPTION_TRACK_RADIUS,
                    &shadow,
                    INACTIVE_ARC_WIDTH + 2.0,
                    Some(&self.round_stroke),
                )?;
                let inactive = self.brush(RadialPaletteArcStyle::INACTIVE_TRACK)?;
                self.draw_arc(
                    geometry,
                    OPTION_TRACK_RADIUS,
                    &inactive,
                    INACTIVE_ARC_WIDTH,
                    Some(&self.round_stroke),
                )?;
            }
            for cell in cells {
                if cell.is_selected || cell.is_hovered {
                    self.draw_active_arc(cell.geometry, OPTION_TRACK_RADIUS, cell.is_enabled)?;
                }
                self.draw_option_node(cell, state.glass_opacity)?;
            }
            Ok(())
        }

        fn draw_option_node(
            &mut self,
            cell: &RadialPaletteRenderCell,
            glass_opacity: f64,
        ) -> Result<()> {
            let target: ID2D1RenderTarget = self.context.cast()?;
            let scale = self.scale();
            let center = self.cell_center(cell, OPTION_NODE_RADIUS);
            let content = content_color(cell);
            if let Some(fill) = cell.color {
                let brush = self.brush(fill)?;
                unsafe { target.FillEllipse(&ellipse(center, 10.0 * scale), &brush) };
                let outline = self.brush(if cell.is_selected {
                    RadialPaletteArcStyle::ACTIVE_ICON
                } else {
                    RadialPaletteArcStyle::OUTER_BORDER
                })?;
                unsafe {
                    target.DrawEllipse(
                        &ellipse(center, 10.0 * scale),
                        &outline,
                        (if cell.is_selected { 2.0 } else { 1.0 } * scale) as f32,
                        None::<&ID2D1StrokeStyle>,
                    )
                };
                return Ok(());
            }

            let surface = self.brush(RadialPaletteArcStyle::surface_color(glass_opacity))?;
            unsafe { target.FillEllipse(&ellipse(center, 13.0 * scale), &surface) };
            if cell.is_hovered {
                let hover = self.brush(RadialPaletteArcStyle::HOVER_FILL)?;
                unsafe { target.FillEllipse(&ellipse(center, 13.0 * scale), &hover) };
            }
            let border = self.brush(RadialPaletteArcStyle::OUTER_BORDER)?;
            unsafe {
                target.DrawEllipse(
                    &ellipse(center, 13.0 * scale),
                    &border,
                    (0.8 * scale) as f32,
                    None::<&ID2D1StrokeStyle>,
                )
            };
            if let Some(preview) = cell.preview {
                self.draw_preview(preview, &cell.short_label, center, content)?;
            } else if let Some(icon_key) = cell.icon_key.as_deref() {
                self.draw_icon(icon_key, center, 10.0, content)?;
                self.draw_label(
                    &cell.short_label,
                    rect_around(
                        [center[0], center[1] + 7.0 * scale as f32],
                        22.0 * scale,
                        8.0 * scale,
                    ),
                    7.0,
                    content,
                    true,
                )?;
            } else if !cell.short_label.is_empty() {
                self.draw_label(
                    &cell.short_label,
                    rect_around(center, 22.0 * scale, 12.0 * scale),
                    8.0,
                    content,
                    true,
                )?;
            }
            Ok(())
        }

        fn draw_preview(
            &mut self,
            preview: RadialPalettePreview,
            label: &str,
            center: [f32; 2],
            content: RadialColor,
        ) -> Result<()> {
            let target: ID2D1RenderTarget = self.context.cast()?;
            let scale = self.scale();
            match preview {
                RadialPalettePreview::StrokeWidth { relative, color } => {
                    let width = (1.0 + relative * 7.0) * scale;
                    let brush = self.brush(color)?;
                    unsafe {
                        target.DrawLine(
                            d2d_point!(
                                center[0] - 8.0 * scale as f32,
                                center[1] - 3.0 * scale as f32
                            ),
                            d2d_point!(
                                center[0] + 8.0 * scale as f32,
                                center[1] - 3.0 * scale as f32
                            ),
                            &brush,
                            width as f32,
                            Some(&self.round_stroke),
                        )
                    };
                }
                RadialPalettePreview::TextSize { relative, color } => {
                    self.draw_label(
                        "가",
                        rect_around(
                            [center[0], center[1] - 3.0 * scale as f32],
                            18.0 * scale,
                            16.0 * scale,
                        ),
                        8.0 + relative * 6.0,
                        color,
                        true,
                    )?;
                }
                RadialPalettePreview::EraserDiameter { relative } => {
                    let radius = (2.5 + relative * 5.0) * scale;
                    let brush = self.brush(content)?;
                    unsafe {
                        target.DrawEllipse(
                            &ellipse([center[0], center[1] - 3.0 * scale as f32], radius),
                            &brush,
                            (1.2 * scale) as f32,
                            None::<&ID2D1StrokeStyle>,
                        )
                    };
                }
            }
            self.draw_label(
                label,
                rect_around(
                    [center[0], center[1] + 8.0 * scale as f32],
                    22.0 * scale,
                    8.0 * scale,
                ),
                7.5,
                content,
                true,
            )
        }

        fn draw_center(&mut self, state: &RadialPaletteRenderState) -> Result<()> {
            let target: ID2D1RenderTarget = self.context.cast()?;
            let center = self.center();
            let scale = self.scale();
            let radius = CENTER_RADIUS * scale;
            let surface = self.brush(RadialPaletteArcStyle::surface_color(state.glass_opacity))?;
            unsafe { target.FillEllipse(&ellipse(center, radius), &surface) };
            if state.center_is_pass_through_enabled {
                let mut selected = RadialPaletteArcStyle::ACTIVE;
                selected[3] *= RadialPaletteArcStyle::SELECTED_TINT_OPACITY;
                let brush = self.brush(selected)?;
                unsafe { target.FillEllipse(&ellipse(center, radius), &brush) };
            }
            if state.center_is_hovered {
                let hover = self.brush(RadialPaletteArcStyle::HOVER_FILL)?;
                unsafe { target.FillEllipse(&ellipse(center, radius), &hover) };
            }
            let border = self.brush(RadialPaletteArcStyle::OUTER_BORDER)?;
            unsafe {
                target.DrawEllipse(
                    &ellipse(center, radius - 0.4 * scale),
                    &border,
                    (0.8 * scale) as f32,
                    None::<&ID2D1StrokeStyle>,
                )
            };
            let stroke = self.brush(state.center_color)?;
            unsafe {
                target.DrawEllipse(
                    &ellipse(center, 18.0 * scale),
                    &stroke,
                    (state.center_stroke_width * scale) as f32,
                    None::<&ID2D1StrokeStyle>,
                )
            };
            if let Some(icon_key) = state.center_icon_key.as_deref() {
                let tint = if state.center_is_pass_through_enabled {
                    RadialPaletteArcStyle::ACTIVE_ICON
                } else {
                    RadialPaletteArcStyle::INACTIVE_ICON
                };
                self.draw_icon(icon_key, center, CENTER_ICON_SIZE, tint)?;
            }
            Ok(())
        }

        fn draw_cell_icon(
            &mut self,
            cell: &RadialPaletteRenderCell,
            orbit: f64,
            size: f64,
        ) -> Result<()> {
            let Some(icon_key) = cell.icon_key.as_deref() else {
                return Ok(());
            };
            self.draw_icon(
                icon_key,
                self.cell_center(cell, orbit),
                size,
                content_color(cell),
            )
        }

        fn draw_icon(
            &mut self,
            key: &str,
            center: [f32; 2],
            size_dip: f64,
            tint: RadialColor,
        ) -> Result<()> {
            let size = size_dip * self.scale();
            self.icons.draw(
                key,
                rect_around(center, size, size),
                IconTint::rgba(
                    tint[0] as f32,
                    tint[1] as f32,
                    tint[2] as f32,
                    tint[3] as f32,
                ),
            )
        }

        fn draw_active_arc(
            &self,
            geometry: RadialPaletteSectorGeometry,
            radius: f64,
            enabled: bool,
        ) -> Result<()> {
            let mut glow_color = RadialPaletteArcStyle::ACTIVE_GLOW;
            if !enabled {
                glow_color[3] *= RadialPaletteArcStyle::DISABLED_CONTENT_OPACITY;
            }
            let glow = self.brush(glow_color)?;
            self.draw_arc(
                geometry,
                radius,
                &glow,
                ACTIVE_ARC_WIDTH + 5.0,
                Some(&self.round_stroke),
            )?;
            let mut active_color = RadialPaletteArcStyle::ACTIVE;
            if !enabled {
                active_color[3] *= RadialPaletteArcStyle::DISABLED_CONTENT_OPACITY;
            }
            let active = self.brush(active_color)?;
            self.draw_arc(
                geometry,
                radius,
                &active,
                ACTIVE_ARC_WIDTH,
                Some(&self.round_stroke),
            )
        }

        fn draw_arc(
            &self,
            geometry: RadialPaletteSectorGeometry,
            radius: f64,
            brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
            width: f64,
            stroke: Option<&ID2D1StrokeStyle>,
        ) -> Result<()> {
            let path = self.arc_geometry(radius, geometry.start_angle, geometry.end_angle)?;
            let target: ID2D1RenderTarget = self.context.cast()?;
            unsafe { target.DrawGeometry(&path, brush, (width * self.scale()) as f32, stroke) };
            Ok(())
        }

        fn sector_geometry(
            &self,
            geometry: RadialPaletteSectorGeometry,
        ) -> Result<ID2D1PathGeometry> {
            let center = self.center();
            let metrics = RadialPaletteSectorPath::from_geometry(
                geometry,
                self.scale(),
                [f64::from(center[0]), f64::from(center[1])],
            );
            let path = unsafe { self.factory.CreatePathGeometry() }?;
            let sink: ID2D1GeometrySink = unsafe { path.Open() }?;
            let span = metrics.end_angle - metrics.start_angle;
            unsafe {
                sink.BeginFigure(
                    d2d_point!(metrics.outer_start[0], metrics.outer_start[1]),
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: d2d_point!(metrics.outer_end[0], metrics.outer_end[1]),
                    size: D2D_SIZE_F {
                        width: metrics.outer_radius_px as f32,
                        height: metrics.outer_radius_px as f32,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: if span.abs() > PI {
                        D2D1_ARC_SIZE_LARGE
                    } else {
                        D2D1_ARC_SIZE_SMALL
                    },
                });
                sink.AddLine(d2d_point!(metrics.inner_end[0], metrics.inner_end[1]));
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: d2d_point!(metrics.inner_start[0], metrics.inner_start[1]),
                    size: D2D_SIZE_F {
                        width: metrics.inner_radius_px as f32,
                        height: metrics.inner_radius_px as f32,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE,
                    arcSize: if span.abs() > PI {
                        D2D1_ARC_SIZE_LARGE
                    } else {
                        D2D1_ARC_SIZE_SMALL
                    },
                });
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close()?;
            }
            Ok(path)
        }

        fn arc_geometry(
            &self,
            radius: f64,
            start_angle: f64,
            end_angle: f64,
        ) -> Result<ID2D1PathGeometry> {
            let scale = self.scale();
            let center = self.center();
            let point = |angle: f64| {
                [
                    center[0] + (angle.sin() * radius * scale) as f32,
                    center[1] - (angle.cos() * radius * scale) as f32,
                ]
            };
            let start = point(start_angle);
            let end = point(end_angle);
            let path = unsafe { self.factory.CreatePathGeometry() }?;
            let sink: ID2D1GeometrySink = unsafe { path.Open() }?;
            unsafe {
                sink.BeginFigure(d2d_point!(start[0], start[1]), D2D1_FIGURE_BEGIN_HOLLOW);
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: d2d_point!(end[0], end[1]),
                    size: D2D_SIZE_F {
                        width: (radius * scale) as f32,
                        height: (radius * scale) as f32,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: if (end_angle - start_angle).abs() > PI {
                        D2D1_ARC_SIZE_LARGE
                    } else {
                        D2D1_ARC_SIZE_SMALL
                    },
                });
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                sink.Close()?;
            }
            Ok(path)
        }

        fn draw_label(
            &self,
            label: &str,
            rect: D2D_RECT_F,
            font_size: f64,
            tint: RadialColor,
            medium: bool,
        ) -> Result<()> {
            if label.is_empty() {
                return Ok(());
            }
            let format = unsafe {
                self.dwrite.CreateTextFormat(
                    w!("Segoe UI"),
                    None::<&IDWriteFontCollection>,
                    if medium {
                        DWRITE_FONT_WEIGHT_MEDIUM
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    (font_size * self.scale()) as f32,
                    w!("ko-KR"),
                )
            }?;
            unsafe {
                format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            }
            let brush = self.brush(tint)?;
            let target: ID2D1RenderTarget = self.context.cast()?;
            let text = label.encode_utf16().collect::<Vec<_>>();
            unsafe {
                target.DrawText(
                    &text,
                    &format,
                    &rect,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                )
            };
            Ok(())
        }

        fn brush(
            &self,
            value: RadialColor,
        ) -> Result<windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush> {
            unsafe { self.context.CreateSolidColorBrush(&color(value), None) }
        }

        fn cell_center(&self, cell: &RadialPaletteRenderCell, radius: f64) -> [f32; 2] {
            let angle = (cell.geometry.start_angle + cell.geometry.end_angle) / 2.0;
            let scale = self.scale();
            let center = self.center();
            [
                center[0] + (angle.sin() * radius * scale) as f32,
                center[1] - (angle.cos() * radius * scale) as f32,
            ]
        }

        fn node_diameter(&self, cell: &RadialPaletteRenderCell, radius: f64, maximum: f64) -> f64 {
            let arc = (cell.geometry.end_angle - cell.geometry.start_angle) * radius;
            (arc - 4.0).clamp(0.0, maximum)
        }

        fn center(&self) -> [f32; 2] {
            [self.width as f32 / 2.0, self.height as f32 / 2.0]
        }

        fn scale(&self) -> f64 {
            px_per_dip(self.dpi)
        }
    }

    fn content_color(cell: &RadialPaletteRenderCell) -> RadialColor {
        let mut color = if cell.is_selected && cell.color.is_none() {
            RadialPaletteArcStyle::ACTIVE_ICON
        } else {
            RadialPaletteArcStyle::INACTIVE_ICON
        };
        if !cell.is_enabled {
            color[3] *= RadialPaletteArcStyle::DISABLED_CONTENT_OPACITY;
        }
        color
    }

    fn current_diameter(state: &RadialPaletteState) -> f64 {
        if state.is_expanded {
            EXPANDED_DIAMETER
        } else {
            COLLAPSED_DIAMETER
        }
    }

    fn pixels(dip: f64, dpi: u32) -> i32 {
        (dip * px_per_dip(dpi)).round().max(1.0) as i32
    }

    fn target_monitor<'a>(monitors: &'a [Monitor], saved: Option<&str>) -> Option<&'a Monitor> {
        saved
            .and_then(|key| monitors.iter().find(|monitor| monitor.session_key == key))
            .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
            .or_else(|| monitors.first())
    }

    fn storage_key(display_key: &str) -> String {
        format!("{STORAGE_PREFIX}{display_key}")
    }

    fn restored_palette_origin(
        positions: &ScreenPositionDocument,
        monitor: &Monitor,
        diameter: i32,
    ) -> WindowOrigin {
        restored_origin(
            positions.ratio_for(Some(&storage_key(&monitor.session_key))),
            DEFAULT_POSITION,
            screen_frame(monitor),
            WindowSize {
                width: f64::from(diameter),
                height: f64::from(diameter),
            },
        )
    }

    fn screen_frame(monitor: &Monitor) -> ScreenFrame {
        ScreenFrame {
            x: f64::from(monitor.work_rect.left),
            y: f64::from(monitor.work_rect.top),
            width: f64::from(monitor.work_rect.width()),
            height: f64::from(monitor.work_rect.height()),
        }
    }

    fn positive_dimension(value: i32) -> Result<u32> {
        u32::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| Error::new(E_FAIL, "radial palette dimension must be positive"))
    }

    fn create_target_bitmap(
        context: &ID2D1DeviceContext,
        width: u32,
        height: u32,
    ) -> Result<ID2D1Bitmap1> {
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: pixel_format().format,
                alphaMode: pixel_format().alphaMode,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            ..Default::default()
        };
        unsafe { context.CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &properties) }
    }

    fn set_context_target(context: &ID2D1DeviceContext, bitmap: &ID2D1Bitmap1) -> Result<()> {
        let image: ID2D1Image = bitmap.cast()?;
        unsafe { context.SetTarget(&image) };
        Ok(())
    }

    fn ellipse(center: [f32; 2], radius: f64) -> D2D1_ELLIPSE {
        D2D1_ELLIPSE {
            point: d2d_point!(center[0], center[1]),
            radiusX: radius as f32,
            radiusY: radius as f32,
        }
    }

    fn rect_around(center: [f32; 2], width: f64, height: f64) -> D2D_RECT_F {
        D2D_RECT_F {
            left: center[0] - width as f32 / 2.0,
            top: center[1] - height as f32 / 2.0,
            right: center[0] + width as f32 / 2.0,
            bottom: center[1] + height as f32 / 2.0,
        }
    }

    fn color(value: RadialColor) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: value[0] as f32,
            g: value[1] as f32,
            b: value[2] as f32,
            a: value[3] as f32,
        }
    }
}

#[cfg(windows)]
pub use windows_impl::*;
