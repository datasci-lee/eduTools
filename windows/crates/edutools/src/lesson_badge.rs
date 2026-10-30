//! 수업 타이머 배지의 순수 표시 규칙과 Windows 창 구현.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SizePreset {
    Small,
    Medium,
    Large,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl SizePreset {
    pub const fn from_setting(value: &str) -> Self {
        match value.as_bytes() {
            b"small" => Self::Small,
            b"large" => Self::Large,
            _ => Self::Medium,
        }
    }

    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub const fn dip_size(self) -> (u32, u32) {
        match self {
            Self::Small => (140, 64),
            Self::Medium => (180, 80),
            Self::Large => (240, 104),
        }
    }

    pub fn pixel_size(self, dpi: u32) -> (i32, i32) {
        let dpi = dpi.max(96);
        let (width, height) = self.dip_size();
        (
            ((f64::from(width) * f64::from(dpi) / 96.0).round()) as i32,
            ((f64::from(height) * f64::from(dpi) / 96.0).round()) as i32,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayMode {
    Remaining,
    Elapsed,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl DisplayMode {
    pub fn from_setting(value: &str) -> Self {
        if value == "elapsed" {
            Self::Elapsed
        } else {
            Self::Remaining
        }
    }

    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::Remaining => "remaining",
            Self::Elapsed => "elapsed",
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::Remaining => Self::Elapsed,
            Self::Elapsed => Self::Remaining,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum BadgePhase {
    Stopped,
    Running,
    Paused,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadgeMode {
    Countdown,
    CountUp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeColor {
    Default,
    Warning,
    Finished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadgePresentation {
    pub time: String,
    pub label: &'static str,
    pub color: TimeColor,
}

// Swift: LessonTimerController.formattedDuration(_:)
pub fn formatted_duration(interval: f64) -> String {
    let total_seconds = interval.max(0.0).ceil() as u64;
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}

// Swift: LessonTimerController.refresh(now:)
pub fn badge_presentation(
    phase: BadgePhase,
    mode: BadgeMode,
    display_mode: DisplayMode,
    remaining: f64,
    elapsed: f64,
) -> BadgePresentation {
    let shows_remaining = display_mode == DisplayMode::Remaining && mode == BadgeMode::Countdown;
    let value = if shows_remaining { remaining } else { elapsed };
    let label = if phase == BadgePhase::Finished {
        "수업 종료"
    } else {
        match (shows_remaining, phase == BadgePhase::Paused) {
            (true, true) => "남은 시간 · 일시정지",
            (true, false) => "남은 시간",
            (false, true) => "경과 시간 · 일시정지",
            (false, false) => "경과 시간",
        }
    };
    let color = if phase == BadgePhase::Finished {
        TimeColor::Finished
    } else if mode == BadgeMode::Countdown && remaining <= 5.0 * 60.0 {
        TimeColor::Warning
    } else {
        TimeColor::Default
    };
    BadgePresentation {
        time: formatted_duration(value),
        label,
        color,
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        badge_presentation, BadgeMode, BadgePhase, BadgePresentation, DisplayMode, SizePreset,
        TimeColor,
    };
    use crate::log::Logger;
    use crate::monitor::{enumerate_monitors, monitor_at_point, monitor_by_key, Monitor};
    use crate::palette::{app_data_file, load_positions, write_file};
    use crate::render::{
        create_swap_chain, create_swap_target, pixel_format, px_per_dip, read_bitmap_bgra,
        GraphicsDevice,
    };
    use crate::z_bands::{raise_band_order, Band, BandRegistration};
    use edutools_core::screen_position::{
        restored_origin, saved_ratio, PositionRatio, ScreenFrame, ScreenPositionDocument,
        WindowOrigin, WindowSize,
    };
    use edutools_core::settings::{AppSettings, SettingsRanges};
    use edutools_core::timer::{
        LessonTimerEngine, LessonTimerMode, LessonTimerPhase, Point, Rect, Size, TimerCoordinator,
        TimerGeometry,
    };
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows::core::{w, Error, Interface, Result, PCWSTR};
    use windows::Win32::Foundation::{
        ERROR_CLASS_ALREADY_EXISTS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
    };
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
    };
    use windows::Win32::Graphics::Direct2D::{
        ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Image, ID2D1RenderTarget,
        D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DRAW_TEXT_OPTIONS_CLIP,
        D2D1_PRIMITIVE_BLEND_COPY, D2D1_PRIMITIVE_BLEND_SOURCE_OVER, D2D1_ROUNDED_RECT,
    };
    use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_MEASURING_MODE_NATURAL,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_WORD_WRAPPING_NO_WRAP,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
    use windows::Win32::Graphics::Dxgi::IDXGISwapChain1;
    use windows::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_MENU};
    use windows::Win32::UI::Input::Pointer::{
        GetPointerInfo, POINTER_FLAG_SECONDBUTTON, POINTER_INFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
        GetCursorPos, GetWindowLongPtrW, GetWindowRect, KillTimer, PostMessageW, RegisterClassW,
        SetTimer, SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos, ShowWindow,
        TrackPopupMenuEx, CREATESTRUCTW, GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE, MA_NOACTIVATE,
        MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MENU_ITEM_FLAGS, MF_CHECKED,
        MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, PA_NOACTIVATE, SWP_NOACTIVATE, SWP_NOSIZE,
        SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        WDA_EXCLUDEFROMCAPTURE, WM_CONTEXTMENU, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY,
        WM_POINTERACTIVATE, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE, WM_POWERBROADCAST,
        WM_RBUTTONUP, WM_TIMECHANGE, WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE,
        WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    pub const WM_APP_LESSON_ACTION: u32 = 0x8000 + 100;
    pub const WM_APP_LESSON_COMPLETED: u32 = 0x8000 + 101;

    const WINDOW_CLASS: windows::core::PCWSTR = w!("EduToolsLessonBadge");
    const DEFAULT_POSITION: PositionRatio = PositionRatio { x: 0.95, y: 0.88 };
    const RUN_TIMER_ID: usize = 0xED05;
    const BLINK_TIMER_ID: usize = 0xED06;
    const RUN_TIMER_INTERVAL_MS: u32 = 1_000;
    const BLINK_TIMER_INTERVAL_MS: u32 = 350;
    const PBT_APMRESUMEAUTOMATIC_VALUE: usize = 0x12;

    const MENU_PRESET_BASE: u16 = 1_000;
    const MENU_COUNT_UP: u16 = 1_100;
    const MENU_PAUSE: u16 = 1_101;
    const MENU_RESET: u16 = 1_102;
    const MENU_SIZE_SMALL: u16 = 1_110;
    const MENU_SIZE_MEDIUM: u16 = 1_111;
    const MENU_SIZE_LARGE: u16 = 1_112;
    const MENU_OPACITY_100: u16 = 1_120;
    const MENU_OPACITY_80: u16 = 1_121;
    const MENU_OPACITY_60: u16 = 1_122;
    const MENU_OPACITY_40: u16 = 1_123;
    const MENU_DISPLAY_MODE: u16 = 1_130;
    const MENU_RESET_POSITION: u16 = 1_131;
    const MENU_AUTO_BREAK: u16 = 1_132;

    #[derive(Clone, Debug, PartialEq)]
    pub enum SettingsMutation {
        SetSize(SizePreset),
        SetOpacity(f64),
        SetDisplayMode(DisplayMode),
        SetAutoBreak(bool),
    }

    impl SettingsMutation {
        pub fn apply(&self, settings: &mut AppSettings) {
            match self {
                Self::SetSize(size) => {
                    settings.timer.lesson_badge_size = size.setting_value().to_owned();
                }
                Self::SetOpacity(opacity) => {
                    settings.timer.lesson_badge_opacity = opacity.clamp(
                        *SettingsRanges::LESSON_BADGE_OPACITY.start(),
                        *SettingsRanges::LESSON_BADGE_OPACITY.end(),
                    );
                }
                Self::SetDisplayMode(mode) => {
                    settings.timer.lesson_display_mode = mode.setting_value().to_owned();
                }
                Self::SetAutoBreak(enabled) => settings.timer.auto_start_break = *enabled,
            }
        }
    }

    #[derive(Clone)]
    struct BadgeSettings {
        presets: Vec<i64>,
        default_minutes: i64,
        break_minutes: i64,
        sound: String,
        auto_break: bool,
        size: SizePreset,
        opacity: f64,
        display_mode: DisplayMode,
    }

    impl BadgeSettings {
        fn from_app(settings: &AppSettings) -> Self {
            Self {
                presets: settings.timer.lesson_presets.clone(),
                default_minutes: settings.timer.lesson_default_minutes,
                break_minutes: settings.timer.break_default_minutes,
                sound: settings.timer.notification_sound.clone(),
                auto_break: settings.timer.auto_start_break,
                size: SizePreset::from_setting(&settings.timer.lesson_badge_size),
                opacity: settings.timer.lesson_badge_opacity,
                display_mode: DisplayMode::from_setting(&settings.timer.lesson_display_mode),
            }
        }
    }

    struct DragState {
        pointer_id: Option<u32>,
        cursor_start: POINT,
        window_start: POINT,
        moved: bool,
    }

    struct LessonBadgeWindowState {
        owner: HWND,
        logger: Logger,
        dpi: u32,
        settings: BadgeSettings,
        engine: LessonTimerEngine,
        renderer: Option<LessonBadgeRenderer>,
        positions: ScreenPositionDocument,
        positions_path: Option<PathBuf>,
        current_monitor_key: String,
        drag: Option<DragState>,
        context_pointer: Option<u32>,
        visible: bool,
        run_timer_running: bool,
        blink_timer_running: bool,
        blink_count: u8,
        blink_on: bool,
    }

    struct LessonBadgeWindow {
        hwnd: HWND,
        state: Box<LessonBadgeWindowState>,
        _band: BandRegistration,
    }

    pub struct LessonBadgeManager {
        window: LessonBadgeWindow,
        _graphics: GraphicsDevice,
    }

    impl LessonBadgeManager {
        pub fn new(
            owner: HWND,
            hinstance: HINSTANCE,
            settings: &AppSettings,
            logger: Logger,
        ) -> Result<Self> {
            register_window_class(hinstance)?;
            let positions_path = app_data_file("positions.json");
            let positions = load_positions(positions_path.as_deref(), &logger);
            let monitors = enumerate_monitors()?;
            let saved_key = positions.last_display_key().map(str::to_owned);
            let monitor = saved_key
                .as_deref()
                .and_then(|key| monitor_by_key(&monitors, key))
                .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "lesson badge requires at least one monitor"))?;
            let badge_settings = BadgeSettings::from_app(settings);
            let dpi = monitor.dpi.0.max(96);
            let (width, height) = badge_settings.size.pixel_size(dpi);
            let restored_ratio = saved_key
                .as_deref()
                .and_then(|key| positions.ratio_for(Some(&storage_key(key))))
                .or_else(|| positions.ratio_for(Some(&storage_key(&monitor.session_key))));
            let origin = restored_origin(
                restored_ratio,
                DEFAULT_POSITION,
                screen_frame(monitor),
                WindowSize {
                    width: f64::from(width),
                    height: f64::from(height),
                },
            );
            let default_duration = badge_settings.default_minutes as f64 * 60.0;
            let mut state = Box::new(LessonBadgeWindowState {
                owner,
                logger: logger.clone(),
                dpi,
                settings: badge_settings,
                engine: LessonTimerEngine::new(default_duration),
                renderer: None,
                positions,
                positions_path,
                current_monitor_key: monitor.session_key.clone(),
                drag: None,
                context_pointer: None,
                visible: false,
                run_timer_running: false,
                blink_timer_running: false,
                blink_count: 0,
                blink_on: false,
            });
            let state_ptr = state.as_mut() as *mut LessonBadgeWindowState;
            let ex_style =
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP;
            let hwnd = unsafe {
                CreateWindowExW(
                    ex_style,
                    WINDOW_CLASS,
                    w!("EduTools Lesson Timer"),
                    WS_POPUP,
                    origin.x.round() as i32,
                    origin.y.round() as i32,
                    width,
                    height,
                    None,
                    None,
                    Some(hinstance),
                    Some(state_ptr.cast::<c_void>()),
                )
            }?;
            if let Err(error) = unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) } {
                logger.event(&format!(
                    "수업 배지 WDA_EXCLUDEFROMCAPTURE 설정 실패: {error}"
                ));
            }
            let band = BandRegistration::new(hwnd, Band::Tool);
            let graphics = match GraphicsDevice::new() {
                Ok(graphics) => graphics,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            let renderer = match LessonBadgeRenderer::new(&graphics, hwnd, width, height, dpi) {
                Ok(renderer) => renderer,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            state.renderer = Some(renderer);
            state.render_now(wall_clock_timestamp())?;
            logger.event(&format!(
                "수업 배지 위치 복원: monitor={} ratio=({:.3},{:.3}) origin=({}, {}) clamp={}",
                monitor.session_key,
                restored_ratio.unwrap_or(DEFAULT_POSITION).x,
                restored_ratio.unwrap_or(DEFAULT_POSITION).y,
                origin.x.round() as i32,
                origin.y.round() as i32,
                saved_key.is_some_and(|key| key != monitor.session_key)
            ));
            Ok(Self {
                window: LessonBadgeWindow {
                    hwnd,
                    state,
                    _band: band,
                },
                _graphics: graphics,
            })
        }

        pub fn toggle_from_shortcut(&mut self, settings: &AppSettings) -> Result<()> {
            self.apply_settings(settings)?;
            let now = wall_clock_timestamp();
            match self.window.state.engine.phase() {
                LessonTimerPhase::Stopped | LessonTimerPhase::Finished => {
                    self.window.state.engine.toggle_default(
                        now,
                        self.window.state.settings.default_minutes as f64 * 60.0,
                    );
                    self.window.state.logger.event(&format!(
                        "수업 타이머 시작: 종류=countdown 분={}",
                        self.window.state.settings.default_minutes
                    ));
                }
                LessonTimerPhase::Running => {
                    self.window.state.engine.toggle_default(now, 1.0);
                    self.window
                        .state
                        .logger
                        .event("수업 타이머 일시정지: 전역 단축키");
                }
                LessonTimerPhase::Paused => {
                    self.window.state.engine.toggle_default(now, 1.0);
                    self.window
                        .state
                        .logger
                        .event("수업 타이머 재개: 전역 단축키");
                }
            }
            self.window.state.show(self.window.hwnd, "전역 단축키")?;
            self.window.state.refresh(self.window.hwnd, "전역 단축키")
        }

        pub fn handle_menu_action(
            &mut self,
            command: u16,
            settings: &AppSettings,
        ) -> Result<Option<SettingsMutation>> {
            self.apply_settings(settings)?;
            let now = wall_clock_timestamp();
            if (MENU_PRESET_BASE..MENU_PRESET_BASE + 100).contains(&command) {
                let index = usize::from(command - MENU_PRESET_BASE);
                let Some(minutes) = self.window.state.settings.presets.get(index).copied() else {
                    return Err(Error::new(
                        E_FAIL,
                        "lesson preset menu index is out of range",
                    ));
                };
                self.window
                    .state
                    .engine
                    .start_countdown(minutes as f64 * 60.0, now);
                self.window
                    .state
                    .logger
                    .event(&format!("수업 타이머 시작: 종류=countdown 분={minutes}"));
                self.window.state.show(self.window.hwnd, "프리셋 메뉴")?;
                self.window.state.refresh(self.window.hwnd, "프리셋 메뉴")?;
                return Ok(None);
            }
            let mutation = match command {
                MENU_COUNT_UP => {
                    self.window.state.engine.start_count_up(now);
                    self.window
                        .state
                        .logger
                        .event("수업 타이머 시작: 종류=countup 분=0");
                    self.window.state.show(self.window.hwnd, "카운트업 메뉴")?;
                    self.window
                        .state
                        .refresh(self.window.hwnd, "카운트업 메뉴")?;
                    None
                }
                MENU_PAUSE => {
                    if self.window.state.engine.phase() == LessonTimerPhase::Paused {
                        self.window.state.engine.resume(now);
                        self.window.state.logger.event("수업 타이머 재개: 메뉴");
                    } else {
                        self.window.state.engine.pause(now);
                        self.window.state.logger.event("수업 타이머 일시정지: 메뉴");
                    }
                    self.window
                        .state
                        .refresh(self.window.hwnd, "일시정지 메뉴")?;
                    None
                }
                MENU_RESET => {
                    self.window.state.engine.reset(now);
                    self.window.state.logger.event("수업 타이머 리셋");
                    self.window.state.show(self.window.hwnd, "리셋 메뉴")?;
                    self.window.state.refresh(self.window.hwnd, "리셋 메뉴")?;
                    None
                }
                MENU_SIZE_SMALL => Some(SettingsMutation::SetSize(SizePreset::Small)),
                MENU_SIZE_MEDIUM => Some(SettingsMutation::SetSize(SizePreset::Medium)),
                MENU_SIZE_LARGE => Some(SettingsMutation::SetSize(SizePreset::Large)),
                MENU_OPACITY_100 => Some(SettingsMutation::SetOpacity(1.0)),
                MENU_OPACITY_80 => Some(SettingsMutation::SetOpacity(0.8)),
                MENU_OPACITY_60 => Some(SettingsMutation::SetOpacity(0.6)),
                MENU_OPACITY_40 => Some(SettingsMutation::SetOpacity(0.4)),
                MENU_DISPLAY_MODE => Some(SettingsMutation::SetDisplayMode(
                    self.window.state.settings.display_mode.toggled(),
                )),
                MENU_RESET_POSITION => {
                    self.window.state.reset_position(self.window.hwnd)?;
                    None
                }
                MENU_AUTO_BREAK => Some(SettingsMutation::SetAutoBreak(
                    !self.window.state.settings.auto_break,
                )),
                _ => {
                    self.window
                        .state
                        .logger
                        .event(&format!("수업 배지 알 수 없는 메뉴 명령: {command}"));
                    None
                }
            };
            Ok(mutation)
        }

        pub fn apply_settings(&mut self, settings: &AppSettings) -> Result<()> {
            let next = BadgeSettings::from_app(settings);
            let size_changed = next.size != self.window.state.settings.size;
            self.window.state.settings = next;
            if size_changed {
                self.window.state.resize_around_center(self.window.hwnd)?;
            }
            self.window.state.render_now(wall_clock_timestamp())
        }

        pub fn display_configuration_changed(&mut self, reason: &str) -> Result<()> {
            self.window.state.restore_position(self.window.hwnd, reason)
        }

        pub fn synchronize_after_system_event(&mut self, reason: &str) -> Result<()> {
            self.window.state.logger.event(&format!(
                "수업 타이머 잠자기/시간 변경 복귀 동기화: {reason}"
            ));
            self.window.state.refresh(self.window.hwnd, reason)
        }

        pub fn selftest_failures(&mut self) -> Vec<String> {
            let mut failures = Vec::new();
            let required_ex =
                (WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP).0
                    as isize;
            let actual_ex = unsafe { GetWindowLongPtrW(self.window.hwnd, GWL_EXSTYLE) };
            if actual_ex & required_ex != required_ex {
                failures.push(format!(
                    "lesson badge: 필수 확장 스타일 누락 ({actual_ex:#x})"
                ));
            }
            let actual_style = unsafe { GetWindowLongPtrW(self.window.hwnd, GWL_STYLE) };
            if actual_style & WS_POPUP.0 as isize == 0 {
                failures.push(format!("lesson badge: WS_POPUP 누락 ({actual_style:#x})"));
            }
            match self
                .window
                .state
                .renderer
                .as_mut()
                .expect("lesson badge renderer exists")
                .selftest_pixel_failures(self.window.state.settings.opacity)
            {
                Ok(pixel_failures) => failures.extend(pixel_failures),
                Err(error) => failures.push(format!("lesson badge: 픽셀 readback 실패: {error}")),
            }
            failures
        }

        pub fn hide(&mut self, reason: &str) {
            self.window.state.hide(self.window.hwnd, reason);
        }
    }

    impl LessonBadgeWindowState {
        fn show(&mut self, hwnd: HWND, reason: &str) -> Result<()> {
            if !self.visible {
                let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
                raise_band_order()?;
                self.visible = true;
                self.logger
                    .event(&format!("수업 배지 표시: reason={reason}"));
            }
            self.sync_run_timer(hwnd)
        }

        fn hide(&mut self, hwnd: HWND, reason: &str) {
            if self.visible {
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                self.visible = false;
                self.logger
                    .event(&format!("수업 배지 숨김: reason={reason}"));
            }
            if self.run_timer_running {
                if let Err(error) = unsafe { KillTimer(Some(hwnd), RUN_TIMER_ID) } {
                    self.logger
                        .event(&format!("수업 배지 갱신 타이머 중단 실패: {error}"));
                }
                self.run_timer_running = false;
            }
        }

        fn refresh(&mut self, hwnd: HWND, reason: &str) -> Result<()> {
            let now = wall_clock_timestamp();
            let did_finish = self.engine.synchronize(now);
            self.render_now(now)?;
            self.sync_run_timer(hwnd)?;
            if did_finish {
                self.handle_completion(hwnd, reason)?;
            }
            Ok(())
        }

        fn render_now(&mut self, now: f64) -> Result<()> {
            let presentation =
                presentation_for_engine(&self.engine, self.settings.display_mode, now);
            self.renderer
                .as_mut()
                .expect("lesson badge renderer exists after creation")
                .render(&presentation, self.settings.opacity, self.blink_on)
        }

        fn sync_run_timer(&mut self, hwnd: HWND) -> Result<()> {
            let needed = self.visible && self.engine.phase() == LessonTimerPhase::Running;
            match (self.run_timer_running, needed) {
                (false, true) => {
                    let timer =
                        unsafe { SetTimer(Some(hwnd), RUN_TIMER_ID, RUN_TIMER_INTERVAL_MS, None) };
                    if timer == 0 {
                        return Err(Error::new(E_FAIL, "lesson badge timer start failed"));
                    }
                    self.run_timer_running = true;
                }
                (true, false) => {
                    unsafe { KillTimer(Some(hwnd), RUN_TIMER_ID) }?;
                    self.run_timer_running = false;
                }
                _ => {}
            }
            Ok(())
        }

        fn handle_completion(&mut self, hwnd: HWND, reason: &str) -> Result<()> {
            play_completion_sound(&self.settings.sound, &self.logger);
            self.start_blinking(hwnd)?;
            let requested_minutes = TimerCoordinator::pending_auto_break_request(
                self.settings.auto_break,
                self.settings.break_minutes,
            );
            self.logger.event(&format!(
                "수업 타이머 완료: reason={reason} 자동 쉬는 시간 요청={}",
                requested_minutes.is_some()
            ));
            if let Err(error) = unsafe {
                PostMessageW(
                    Some(self.owner),
                    WM_APP_LESSON_COMPLETED,
                    WPARAM(requested_minutes.unwrap_or(0) as usize),
                    LPARAM(0),
                )
            } {
                self.logger.event(&format!(
                    "수업 타이머 완료 요청 전달 실패: minutes={requested_minutes:?}: {error}"
                ));
            }
            Ok(())
        }

        fn start_blinking(&mut self, hwnd: HWND) -> Result<()> {
            if self.blink_timer_running {
                unsafe { KillTimer(Some(hwnd), BLINK_TIMER_ID) }?;
            }
            self.blink_count = 0;
            self.blink_on = false;
            let timer =
                unsafe { SetTimer(Some(hwnd), BLINK_TIMER_ID, BLINK_TIMER_INTERVAL_MS, None) };
            if timer == 0 {
                return Err(Error::new(E_FAIL, "lesson badge blink timer start failed"));
            }
            self.blink_timer_running = true;
            Ok(())
        }

        fn blink_tick(&mut self, hwnd: HWND) -> Result<()> {
            self.blink_count = self.blink_count.saturating_add(1);
            self.blink_on = self.blink_count.is_multiple_of(2);
            if self.blink_count >= 8 {
                unsafe { KillTimer(Some(hwnd), BLINK_TIMER_ID) }?;
                self.blink_timer_running = false;
                self.blink_on = false;
            }
            self.render_now(wall_clock_timestamp())
        }

        fn start_drag(&mut self, hwnd: HWND, pointer_id: Option<u32>) -> Result<()> {
            if self.drag.is_some() {
                return Ok(());
            }
            let mut cursor = POINT::default();
            unsafe { GetCursorPos(&mut cursor) }?;
            let mut rect = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut rect) }?;
            self.drag = Some(DragState {
                pointer_id,
                cursor_start: cursor,
                window_start: POINT {
                    x: rect.left,
                    y: rect.top,
                },
                moved: false,
            });
            Ok(())
        }

        fn continue_drag(&mut self, hwnd: HWND, pointer_id: Option<u32>) -> Result<()> {
            let Some(drag) = self.drag.as_mut() else {
                return Ok(());
            };
            if drag.pointer_id.is_some() && drag.pointer_id != pointer_id {
                return Ok(());
            }
            let mut cursor = POINT::default();
            unsafe { GetCursorPos(&mut cursor) }?;
            let dx = cursor.x - drag.cursor_start.x;
            let dy = cursor.y - drag.cursor_start.y;
            drag.moved |= dx != 0 || dy != 0;
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    drag.window_start.x + dx,
                    drag.window_start.y + dy,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                )
            }
        }

        fn end_drag(&mut self, hwnd: HWND, pointer_id: Option<u32>) -> Result<()> {
            let Some(drag) = self.drag.take() else {
                return Ok(());
            };
            if drag.pointer_id.is_some() && drag.pointer_id != pointer_id {
                self.drag = Some(drag);
                return Ok(());
            }
            let alt = unsafe { GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000 != 0 };
            let ratio = self.snap_clamp_and_save(hwnd, alt)?;
            self.logger.event(&format!(
                "수업 배지 드래그 종료: snap={} ratio=({:.3},{:.3}) moved={}",
                !alt, ratio.x, ratio.y, drag.moved
            ));
            Ok(())
        }

        fn snap_clamp_and_save(
            &mut self,
            hwnd: HWND,
            snapping_disabled: bool,
        ) -> Result<PositionRatio> {
            let mut rect = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut rect) }?;
            let monitors = enumerate_monitors()?;
            let center = POINT {
                x: rect.left + (rect.right - rect.left) / 2,
                y: rect.top + (rect.bottom - rect.top) / 2,
            };
            let monitor = monitor_at_point(&monitors, center)
                .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "lesson badge drag has no monitor"))?;
            let work = screen_frame(monitor);
            let size = WindowSize {
                width: f64::from(rect.right - rect.left),
                height: f64::from(rect.bottom - rect.top),
            };
            let origin = TimerGeometry::snapped_origin(
                Point::new(f64::from(rect.left), f64::from(rect.top)),
                Size::new(size.width, size.height),
                Rect::new(work.x, work.y, work.width, work.height),
                snapping_disabled,
            );
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    origin.x.round() as i32,
                    origin.y.round() as i32,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                )
            }?;
            let ratio = saved_ratio(
                WindowOrigin {
                    x: origin.x,
                    y: origin.y,
                },
                work,
                size,
            );
            self.positions = load_positions(self.positions_path.as_deref(), &self.logger);
            self.positions.save_with_display_key(
                ratio,
                storage_key(&monitor.session_key),
                monitor.session_key.clone(),
            );
            self.current_monitor_key = monitor.session_key.clone();
            self.persist_positions()?;
            Ok(ratio)
        }

        fn reset_position(&mut self, hwnd: HWND) -> Result<()> {
            let monitors = enumerate_monitors()?;
            let monitor = monitors
                .iter()
                .find(|monitor| monitor.is_primary)
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "lesson badge reset has no monitor"))?;
            self.place_on_monitor(hwnd, monitor, Some(DEFAULT_POSITION), "위치 초기화")?;
            let ratio = self.snap_clamp_and_save(hwnd, true)?;
            self.logger.event(&format!(
                "수업 배지 위치 초기화: monitor={} ratio=({:.3},{:.3})",
                monitor.session_key, ratio.x, ratio.y
            ));
            Ok(())
        }

        fn restore_position(&mut self, hwnd: HWND, reason: &str) -> Result<()> {
            self.positions = load_positions(self.positions_path.as_deref(), &self.logger);
            let monitors = enumerate_monitors()?;
            let saved_key = self.positions.last_display_key().map(str::to_owned);
            let monitor = saved_key
                .as_deref()
                .and_then(|key| monitor_by_key(&monitors, key))
                .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "lesson badge restore has no monitor"))?;
            let ratio = saved_key
                .as_deref()
                .and_then(|key| self.positions.ratio_for(Some(&storage_key(key))))
                .or_else(|| {
                    self.positions
                        .ratio_for(Some(&storage_key(&monitor.session_key)))
                });
            let clamped = saved_key
                .as_deref()
                .is_some_and(|key| key != monitor.session_key);
            self.place_on_monitor(hwnd, monitor, ratio, reason)?;
            let applied_ratio = if clamped {
                self.snap_clamp_and_save(hwnd, true)?
            } else {
                ratio.unwrap_or(DEFAULT_POSITION)
            };
            self.logger.event(&format!(
                "수업 배지 위치 복원/클램프: reason={reason} monitor={} ratio=({:.3},{:.3}) clamp={clamped}",
                monitor.session_key,
                applied_ratio.x,
                applied_ratio.y
            ));
            Ok(())
        }

        fn place_on_monitor(
            &mut self,
            hwnd: HWND,
            monitor: &Monitor,
            ratio: Option<PositionRatio>,
            _reason: &str,
        ) -> Result<()> {
            let dpi = monitor.dpi.0.max(96);
            let (width, height) = self.settings.size.pixel_size(dpi);
            let origin = restored_origin(
                ratio,
                DEFAULT_POSITION,
                screen_frame(monitor),
                WindowSize {
                    width: f64::from(width),
                    height: f64::from(height),
                },
            );
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    origin.x.round() as i32,
                    origin.y.round() as i32,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            }?;
            self.dpi = dpi;
            self.current_monitor_key = monitor.session_key.clone();
            self.renderer
                .as_mut()
                .expect("lesson badge renderer exists")
                .resize(width, height, dpi)?;
            self.render_now(wall_clock_timestamp())
        }

        fn dpi_changed(&mut self, hwnd: HWND, suggested: RECT) -> Result<()> {
            let queried = unsafe { GetDpiForWindow(hwnd) }.max(96);
            let (width, height) = self.settings.size.pixel_size(queried);
            let monitors = enumerate_monitors()?;
            let point = POINT {
                x: suggested.left + width / 2,
                y: suggested.top + height / 2,
            };
            let monitor = monitor_at_point(&monitors, point)
                .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "lesson badge DPI change has no monitor"))?;
            let work = screen_frame(monitor);
            let origin = TimerGeometry::snapped_origin(
                Point::new(f64::from(suggested.left), f64::from(suggested.top)),
                Size::new(f64::from(width), f64::from(height)),
                Rect::new(work.x, work.y, work.width, work.height),
                true,
            );
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    origin.x.round() as i32,
                    origin.y.round() as i32,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            }?;
            self.dpi = queried;
            self.current_monitor_key = monitor.session_key.clone();
            self.renderer
                .as_mut()
                .expect("lesson badge renderer exists")
                .resize(width, height, queried)?;
            self.render_now(wall_clock_timestamp())?;
            let ratio = self.snap_clamp_and_save(hwnd, true)?;
            self.logger.event(&format!(
                "수업 배지 WM_DPICHANGED 복원/클램프: dpi={queried} monitor={} ratio=({:.3},{:.3})",
                monitor.session_key, ratio.x, ratio.y
            ));
            Ok(())
        }

        fn resize_around_center(&mut self, hwnd: HWND) -> Result<()> {
            let mut rect = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut rect) }?;
            let (width, height) = self.settings.size.pixel_size(self.dpi);
            let center_x = rect.left + (rect.right - rect.left) / 2;
            let center_y = rect.top + (rect.bottom - rect.top) / 2;
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    center_x - width / 2,
                    center_y - height / 2,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            }?;
            self.renderer
                .as_mut()
                .expect("lesson badge renderer exists")
                .resize(width, height, self.dpi)?;
            let _ = self.snap_clamp_and_save(hwnd, true)?;
            Ok(())
        }

        fn persist_positions(&self) -> Result<()> {
            let path = self.positions_path.as_deref().ok_or_else(|| {
                Error::new(E_FAIL, "APPDATA is unavailable for lesson positions.json")
            })?;
            let json = self
                .positions
                .to_json()
                .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
            write_file(path, json.as_bytes(), "lesson positions.json")
        }
    }

    impl Drop for LessonBadgeWindow {
        fn drop(&mut self) {
            self.state.hide(self.hwnd, "앱 종료");
            if self.state.blink_timer_running {
                let _ = unsafe { KillTimer(Some(self.hwnd), BLINK_TIMER_ID) };
                self.state.blink_timer_running = false;
            }
            self.state.renderer.take();
            if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
                self.state
                    .logger
                    .event(&format!("수업 배지 창 제거 실패: {error}"));
            }
        }
    }

    fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
        let class = WNDCLASSW {
            lpfnWndProc: Some(lesson_badge_window_proc),
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

    unsafe extern "system" fn lesson_badge_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            let state = create.lpCreateParams as *mut LessonBadgeWindowState;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
        }
        let state =
            unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LessonBadgeWindowState };
        if !state.is_null() {
            let state = unsafe { &mut *state };
            let result = match message {
                WM_POINTERDOWN => pointer_down(hwnd, state, (wparam.0 as u32) & 0xffff),
                WM_POINTERUPDATE => state.continue_drag(hwnd, Some((wparam.0 as u32) & 0xffff)),
                WM_POINTERUP => pointer_up(hwnd, state, (wparam.0 as u32) & 0xffff),
                WM_LBUTTONDOWN => state.start_drag(hwnd, None),
                WM_MOUSEMOVE
                    if state
                        .drag
                        .as_ref()
                        .is_some_and(|drag| drag.pointer_id.is_none()) =>
                {
                    state.continue_drag(hwnd, None)
                }
                WM_LBUTTONUP => state.end_drag(hwnd, None),
                WM_CONTEXTMENU => show_context_menu(hwnd, state, lparam),
                WM_RBUTTONUP => show_context_menu(hwnd, state, LPARAM(-1)),
                WM_TIMER if wparam.0 == RUN_TIMER_ID => state.refresh(hwnd, "1초 타이머"),
                WM_TIMER if wparam.0 == BLINK_TIMER_ID => state.blink_tick(hwnd),
                WM_DPICHANGED => {
                    let suggested = unsafe { *(lparam.0 as *const RECT) };
                    state.dpi_changed(hwnd, suggested)
                }
                WM_DISPLAYCHANGE => state.restore_position(hwnd, "WM_DISPLAYCHANGE"),
                WM_TIMECHANGE => {
                    state
                        .logger
                        .event("수업 타이머 시간 변경 복귀 동기화: WM_TIMECHANGE");
                    state.refresh(hwnd, "WM_TIMECHANGE")
                }
                WM_POWERBROADCAST if wparam.0 == PBT_APMRESUMEAUTOMATIC_VALUE => {
                    state
                        .logger
                        .event("수업 타이머 잠자기 복귀 동기화: PBT_APMRESUMEAUTOMATIC");
                    state.refresh(hwnd, "PBT_APMRESUMEAUTOMATIC")
                }
                WM_POINTERACTIVATE => return LRESULT(PA_NOACTIVATE as isize),
                WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
                WM_ERASEBKGND => return LRESULT(1),
                WM_NCDESTROY => {
                    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
                    return LRESULT(0);
                }
                _ => return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
            };
            if let Err(error) = result {
                state.logger.event(&format!(
                    "수업 배지 창 메시지 처리 실패: message={message:#x}: {error}"
                ));
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn pointer_down(hwnd: HWND, state: &mut LessonBadgeWindowState, pointer_id: u32) -> Result<()> {
        let mut info = POINTER_INFO::default();
        unsafe { GetPointerInfo(pointer_id, &mut info) }?;
        if info.pointerFlags & POINTER_FLAG_SECONDBUTTON == POINTER_FLAG_SECONDBUTTON {
            state.context_pointer = Some(pointer_id);
            return Ok(());
        }
        state.start_drag(hwnd, Some(pointer_id))
    }

    fn pointer_up(hwnd: HWND, state: &mut LessonBadgeWindowState, pointer_id: u32) -> Result<()> {
        if state.context_pointer == Some(pointer_id) {
            state.context_pointer = None;
            show_context_menu(hwnd, state, LPARAM(-1))
        } else {
            state.end_drag(hwnd, Some(pointer_id))
        }
    }

    fn show_context_menu(hwnd: HWND, state: &LessonBadgeWindowState, lparam: LPARAM) -> Result<()> {
        let menu = unsafe { CreatePopupMenu() }?;
        let populated = populate_context_menu(menu, state);
        if let Err(error) = populated {
            let _ = unsafe { DestroyMenu(menu) };
            return Err(error);
        }
        let mut point = POINT {
            x: lparam.0 as i16 as i32,
            y: (lparam.0 >> 16) as i16 as i32,
        };
        if lparam.0 == -1 {
            unsafe { GetCursorPos(&mut point) }?;
        }
        let selected = unsafe {
            TrackPopupMenuEx(
                menu,
                TPM_RIGHTBUTTON.0 | TPM_RETURNCMD.0,
                point.x,
                point.y,
                hwnd,
                None,
            )
        };
        if let Err(error) = unsafe { DestroyMenu(menu) } {
            state
                .logger
                .event(&format!("수업 배지 메뉴 해제 실패: {error}"));
        }
        let command = u16::try_from(selected.0).unwrap_or(0);
        if command != 0 {
            unsafe {
                PostMessageW(
                    Some(state.owner),
                    WM_APP_LESSON_ACTION,
                    WPARAM(usize::from(command)),
                    LPARAM(0),
                )
            }?;
        }
        Ok(())
    }

    fn populate_context_menu(
        menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
        state: &LessonBadgeWindowState,
    ) -> Result<()> {
        let presets = unsafe { CreatePopupMenu() }?;
        for (index, minutes) in state.settings.presets.iter().enumerate() {
            let Some(command) = u16::try_from(index)
                .ok()
                .and_then(|index| MENU_PRESET_BASE.checked_add(index))
            else {
                continue;
            };
            append_text(presets, MF_STRING, command, &format!("{minutes}분"))?;
        }
        append_submenu(menu, presets, "프리셋")?;
        append_text(menu, MF_STRING, MENU_COUNT_UP, "카운트업")?;
        append_separator(menu)?;

        let pause_flags = if state.engine.is_active() {
            MF_STRING
        } else {
            MENU_ITEM_FLAGS(MF_STRING.0 | MF_GRAYED.0)
        };
        append_text(
            menu,
            pause_flags,
            MENU_PAUSE,
            if state.engine.phase() == LessonTimerPhase::Paused {
                "재시작"
            } else {
                "일시정지"
            },
        )?;
        append_text(menu, MF_STRING, MENU_RESET, "리셋")?;
        append_separator(menu)?;

        let sizes = unsafe { CreatePopupMenu() }?;
        for (command, preset, title) in [
            (MENU_SIZE_SMALL, SizePreset::Small, "소"),
            (MENU_SIZE_MEDIUM, SizePreset::Medium, "중"),
            (MENU_SIZE_LARGE, SizePreset::Large, "대"),
        ] {
            append_text(
                sizes,
                checked_flags(state.settings.size == preset),
                command,
                title,
            )?;
        }
        append_submenu(menu, sizes, "크기")?;

        let opacities = unsafe { CreatePopupMenu() }?;
        for (command, opacity, title) in [
            (MENU_OPACITY_100, 1.0, "100%"),
            (MENU_OPACITY_80, 0.8, "80%"),
            (MENU_OPACITY_60, 0.6, "60%"),
            (MENU_OPACITY_40, 0.4, "40%"),
        ] {
            append_text(
                opacities,
                checked_flags((state.settings.opacity - opacity).abs() < 0.01),
                command,
                title,
            )?;
        }
        append_submenu(menu, opacities, "투명도")?;
        append_text(
            menu,
            MF_STRING,
            MENU_DISPLAY_MODE,
            if state.settings.display_mode == DisplayMode::Remaining {
                "경과 시간 표시"
            } else {
                "남은 시간 표시"
            },
        )?;
        append_text(menu, MF_STRING, MENU_RESET_POSITION, "위치 초기화")?;
        append_separator(menu)?;
        append_text(
            menu,
            checked_flags(state.settings.auto_break),
            MENU_AUTO_BREAK,
            &format!(
                "종료 후 {}분 쉬는 시간 자동 시작",
                state.settings.break_minutes
            ),
        )
    }

    fn append_text(
        menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
        flags: MENU_ITEM_FLAGS,
        command: u16,
        text: &str,
    ) -> Result<()> {
        let wide = wide(text);
        unsafe { AppendMenuW(menu, flags, usize::from(command), PCWSTR(wide.as_ptr())) }
    }

    fn append_submenu(
        menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
        submenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
        text: &str,
    ) -> Result<()> {
        let wide = wide(text);
        unsafe {
            AppendMenuW(
                menu,
                MENU_ITEM_FLAGS(MF_STRING.0 | MF_POPUP.0),
                submenu.0 as usize,
                PCWSTR(wide.as_ptr()),
            )
        }
    }

    fn append_separator(menu: windows::Win32::UI::WindowsAndMessaging::HMENU) -> Result<()> {
        unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) }
    }

    fn checked_flags(checked: bool) -> MENU_ITEM_FLAGS {
        MENU_ITEM_FLAGS(MF_STRING.0 | if checked { MF_CHECKED.0 } else { 0 })
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct LessonBadgeRenderer {
        swap_chain: IDXGISwapChain1,
        context: ID2D1DeviceContext,
        swap_target: Option<ID2D1Bitmap1>,
        dwrite: IDWriteFactory,
        _dcomp_target: IDCompositionTarget,
        _visual: IDCompositionVisual,
        width: u32,
        height: u32,
        dpi: u32,
    }

    impl LessonBadgeRenderer {
        fn new(
            graphics: &GraphicsDevice,
            hwnd: HWND,
            width: i32,
            height: i32,
            dpi: u32,
        ) -> Result<Self> {
            let width = positive_dimension(width)?;
            let height = positive_dimension(height)?;
            let swap_chain = create_swap_chain(graphics, width, height)?;
            let context = graphics.create_d2d_context()?;
            unsafe { context.SetDpi(96.0, 96.0) };
            let swap_target = Some(create_swap_target(&context, &swap_chain)?);
            set_context_target(
                &context,
                swap_target.as_ref().expect("lesson badge target exists"),
            )?;
            let (dcomp_target, visual) =
                graphics.attach_composition_swap_chain(hwnd, &swap_chain)?;
            let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
            Ok(Self {
                swap_chain,
                context,
                swap_target,
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
                    .expect("lesson badge target recreated"),
            )?;
            self.width = width;
            self.height = height;
            self.dpi = dpi;
            Ok(())
        }

        fn render(
            &mut self,
            presentation: &BadgePresentation,
            opacity: f64,
            blink_on: bool,
        ) -> Result<()> {
            let target = self
                .swap_target
                .as_ref()
                .expect("lesson badge target exists")
                .clone();
            self.render_to(&target, presentation, opacity, blink_on)?;
            unsafe { self.swap_chain.Present(1, Default::default()).ok() }
        }

        fn selftest_pixel_failures(&mut self, opacity: f64) -> Result<Vec<String>> {
            let target = create_target_bitmap(&self.context, self.width, self.height)?;
            let presentation = BadgePresentation {
                time: "05:00".to_owned(),
                label: "남은 시간",
                color: TimeColor::Default,
            };
            self.render_to(&target, &presentation, opacity, false)?;
            let (pixels, pitch) = read_bitmap_bgra(&self.context, &target)?;
            let pixel_at = |x: u32, y: u32| -> [u8; 4] {
                let start = y as usize * pitch as usize + x as usize * 4;
                [
                    pixels[start],
                    pixels[start + 1],
                    pixels[start + 2],
                    pixels[start + 3],
                ]
            };
            let mut failures = Vec::new();
            if pixel_at(self.width / 2, self.height / 2)[3] == 0 {
                failures.push("lesson badge: 면 영역 alpha가 0".to_owned());
            }
            for (x, y) in [
                (0, 0),
                (self.width - 1, 0),
                (0, self.height - 1),
                (self.width - 1, self.height - 1),
            ] {
                if pixel_at(x, y)[3] != 0 {
                    failures.push(format!(
                        "lesson badge: 둥근 면 밖 ({x},{y}) alpha={} (기대 0)",
                        pixel_at(x, y)[3]
                    ));
                }
            }
            let text_bottom = self.height.saturating_mul(3) / 4;
            let bright = (0..text_bottom).any(|y| {
                (0..self.width).any(|x| {
                    let pixel = pixel_at(x, y);
                    pixel[0] > 150 && pixel[1] > 150 && pixel[2] > 150 && pixel[3] > 0
                })
            });
            if !bright {
                failures.push("lesson badge: 시간 글자 영역에 밝은 픽셀이 없음".to_owned());
            }
            let swap_target = self
                .swap_target
                .as_ref()
                .expect("lesson badge target exists");
            set_context_target(&self.context, swap_target)?;
            Ok(failures)
        }

        fn render_to(
            &mut self,
            target: &ID2D1Bitmap1,
            presentation: &BadgePresentation,
            opacity: f64,
            blink_on: bool,
        ) -> Result<()> {
            set_context_target(&self.context, target)?;
            unsafe {
                self.context.BeginDraw();
                self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                self.context.Clear(Some(&D2D1_COLOR_F::default()));
            }
            let draw_result = self.draw_contents(presentation, opacity, blink_on);
            let end_result = unsafe { self.context.EndDraw(None, None) };
            draw_result?;
            end_result
        }

        fn draw_contents(
            &self,
            presentation: &BadgePresentation,
            opacity: f64,
            blink_on: bool,
        ) -> Result<()> {
            let opacity = opacity.clamp(0.0, 1.0);
            let panel = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: self.width as f32,
                    bottom: self.height as f32,
                },
                radiusX: (14.0 * px_per_dip(self.dpi)) as f32,
                radiusY: (14.0 * px_per_dip(self.dpi)) as f32,
            };
            let surface = brush(
                &self.context,
                [20.0 / 255.0, 23.0 / 255.0, 31.0 / 255.0, opacity],
            )?;
            unsafe { self.context.FillRoundedRectangle(&panel, &surface) };
            unsafe {
                self.context
                    .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER)
            };
            let border = brush(&self.context, [1.0, 1.0, 1.0, 0.08 * opacity])?;
            unsafe {
                self.context
                    .DrawRoundedRectangle(&panel, &border, 1.0, None)
            };
            if blink_on {
                let blink = brush(&self.context, [1.0, 0.18, 0.16, 0.45 * opacity])?;
                unsafe { self.context.FillRoundedRectangle(&panel, &blink) };
            }

            let scale = px_per_dip(self.dpi) as f32;
            let height_dip = self.height as f64 / px_per_dip(self.dpi);
            let time_size = height_dip.mul_add(0.43, 0.0).clamp(24.0, 64.0) as f32 * scale;
            let time_format = text_format(
                &self.dwrite,
                w!("Consolas"),
                time_size,
                DWRITE_FONT_WEIGHT_BOLD,
            )?;
            let label_format = text_format(
                &self.dwrite,
                w!("Segoe UI"),
                10.0 * scale,
                DWRITE_FONT_WEIGHT_MEDIUM,
            )?;
            let time_color = match presentation.color {
                TimeColor::Default => [0.96, 0.97, 1.0, opacity],
                TimeColor::Warning => [1.0, 0.62, 0.08, opacity],
                TimeColor::Finished => [1.0, 0.27, 0.23, opacity],
            };
            let label_color = [0.68, 0.72, 0.80, 0.92 * opacity];
            let time_rect = D2D_RECT_F {
                left: 6.0 * scale,
                top: 2.0 * scale,
                right: self.width as f32 - 6.0 * scale,
                bottom: self.height as f32 * 0.72,
            };
            let label_rect = D2D_RECT_F {
                left: 4.0 * scale,
                top: self.height as f32 * 0.66,
                right: self.width as f32 - 4.0 * scale,
                bottom: self.height as f32 - 3.0 * scale,
            };
            self.draw_text(&presentation.time, &time_format, time_rect, time_color)?;
            self.draw_text(presentation.label, &label_format, label_rect, label_color)
        }

        fn draw_text(
            &self,
            value: &str,
            format: &IDWriteTextFormat,
            rect: D2D_RECT_F,
            tint: [f64; 4],
        ) -> Result<()> {
            let text = value.encode_utf16().collect::<Vec<_>>();
            let text_brush = brush(&self.context, tint)?;
            let target: ID2D1RenderTarget = self.context.cast()?;
            unsafe {
                target.DrawText(
                    &text,
                    format,
                    &rect,
                    &text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                )
            };
            Ok(())
        }
    }

    fn text_format(
        factory: &IDWriteFactory,
        family: PCWSTR,
        size: f32,
        weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
    ) -> Result<IDWriteTextFormat> {
        let format = unsafe {
            factory.CreateTextFormat(
                family,
                None::<&IDWriteFontCollection>,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("ko-KR"),
            )
        }?;
        unsafe {
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        }
        Ok(format)
    }

    fn brush(
        context: &ID2D1DeviceContext,
        value: [f64; 4],
    ) -> Result<windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush> {
        unsafe {
            context.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: value[0] as f32,
                    g: value[1] as f32,
                    b: value[2] as f32,
                    a: value[3] as f32,
                },
                None,
            )
        }
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

    fn positive_dimension(value: i32) -> Result<u32> {
        u32::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| Error::new(E_FAIL, "lesson badge dimension must be positive"))
    }

    fn presentation_for_engine(
        engine: &LessonTimerEngine,
        display_mode: DisplayMode,
        now: f64,
    ) -> BadgePresentation {
        badge_presentation(
            match engine.phase() {
                LessonTimerPhase::Stopped => BadgePhase::Stopped,
                LessonTimerPhase::Running => BadgePhase::Running,
                LessonTimerPhase::Paused => BadgePhase::Paused,
                LessonTimerPhase::Finished => BadgePhase::Finished,
            },
            match engine.mode() {
                LessonTimerMode::Countdown { .. } => BadgeMode::Countdown,
                LessonTimerMode::CountUp => BadgeMode::CountUp,
            },
            display_mode,
            engine.remaining(now),
            engine.elapsed(now),
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

    fn storage_key(display_key: &str) -> String {
        format!("lessonTimer::{display_key}")
    }

    fn wall_clock_timestamp() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |duration| duration.as_secs_f64())
    }

    pub(crate) fn play_completion_sound(name: &str, logger: &Logger) {
        let style = match name {
            "Glass" | "Hero" | "Ping" | "Pop" | "Purr" | "Tink" => Some(MB_ICONINFORMATION),
            "Basso" | "Funk" | "Sosumi" => Some(MB_ICONERROR),
            "Blow" | "Bottle" | "Frog" | "Morse" | "Submarine" => Some(MB_ICONWARNING),
            _ => None,
        };
        let style = style.unwrap_or_else(|| {
            logger.event(&format!(
                "수업 타이머 완료음 폴백: 알 수 없는 이름={name}; 기본 비프 사용"
            ));
            MB_OK
        });
        if let Err(error) = unsafe { MessageBeep(style) } {
            logger.event(&format!(
                "수업 타이머 완료음 재생 실패: 이름={name}; 기본 비프 재시도: {error}"
            ));
            let _ = unsafe { MessageBeep(MB_OK) };
        }
    }
}

#[cfg(windows)]
pub use windows_impl::*;

#[cfg(test)]
mod tests {
    use super::{
        badge_presentation, formatted_duration, BadgeMode, BadgePhase, BadgePresentation,
        DisplayMode, SizePreset, TimeColor,
    };

    #[test]
    fn formatted_duration_boundary_table() {
        let cases = [
            (0.0, "00:00"),
            (59.0, "00:59"),
            (60.0, "01:00"),
            (3_599.0, "59:59"),
            (3_600.0, "60:00"),
            (3_661.0, "61:01"),
        ];
        for (input, expected) in cases {
            assert_eq!(formatted_duration(input), expected, "input={input}");
        }
    }

    #[test]
    fn state_label_and_color_rule_table() {
        let cases = [
            (
                "running remaining above boundary",
                BadgePhase::Running,
                BadgeMode::Countdown,
                DisplayMode::Remaining,
                301.0,
                99.0,
                BadgePresentation {
                    time: "05:01".to_owned(),
                    label: "남은 시간",
                    color: TimeColor::Default,
                },
            ),
            (
                "running remaining at boundary",
                BadgePhase::Running,
                BadgeMode::Countdown,
                DisplayMode::Remaining,
                300.0,
                100.0,
                BadgePresentation {
                    time: "05:00".to_owned(),
                    label: "남은 시간",
                    color: TimeColor::Warning,
                },
            ),
            (
                "paused elapsed countdown warning",
                BadgePhase::Paused,
                BadgeMode::Countdown,
                DisplayMode::Elapsed,
                299.0,
                101.0,
                BadgePresentation {
                    time: "01:41".to_owned(),
                    label: "경과 시간 · 일시정지",
                    color: TimeColor::Warning,
                },
            ),
            (
                "paused remaining",
                BadgePhase::Paused,
                BadgeMode::Countdown,
                DisplayMode::Remaining,
                601.0,
                99.0,
                BadgePresentation {
                    time: "10:01".to_owned(),
                    label: "남은 시간 · 일시정지",
                    color: TimeColor::Default,
                },
            ),
            (
                "count up ignores remaining warning",
                BadgePhase::Running,
                BadgeMode::CountUp,
                DisplayMode::Remaining,
                0.0,
                61.0,
                BadgePresentation {
                    time: "01:01".to_owned(),
                    label: "경과 시간",
                    color: TimeColor::Default,
                },
            ),
            (
                "finished remaining",
                BadgePhase::Finished,
                BadgeMode::Countdown,
                DisplayMode::Remaining,
                0.0,
                3_000.0,
                BadgePresentation {
                    time: "00:00".to_owned(),
                    label: "수업 종료",
                    color: TimeColor::Finished,
                },
            ),
            (
                "finished elapsed",
                BadgePhase::Finished,
                BadgeMode::Countdown,
                DisplayMode::Elapsed,
                0.0,
                3_000.0,
                BadgePresentation {
                    time: "50:00".to_owned(),
                    label: "수업 종료",
                    color: TimeColor::Finished,
                },
            ),
        ];
        for (name, phase, mode, display, remaining, elapsed, expected) in cases {
            assert_eq!(
                badge_presentation(phase, mode, display, remaining, elapsed),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn size_preset_dip_to_pixel_table() {
        let cases = [
            (SizePreset::Small, 96, (140, 64)),
            (SizePreset::Small, 144, (210, 96)),
            (SizePreset::Small, 192, (280, 128)),
            (SizePreset::Medium, 96, (180, 80)),
            (SizePreset::Medium, 144, (270, 120)),
            (SizePreset::Medium, 192, (360, 160)),
            (SizePreset::Large, 96, (240, 104)),
            (SizePreset::Large, 144, (360, 156)),
            (SizePreset::Large, 192, (480, 208)),
        ];
        for (preset, dpi, expected) in cases {
            assert_eq!(preset.pixel_size(dpi), expected, "{preset:?} dpi={dpi}");
        }
    }
}
