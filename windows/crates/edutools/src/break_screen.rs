//! 쉬는 시간 전체 화면/접힘 화면과 일반 Win32 시각 설정 창.

use crate::lesson_badge::formatted_duration;
use crate::log::Logger;
use crate::monitor::{enumerate_monitors, monitor_at_point, monitor_by_key, Monitor};
use crate::render::{
    create_swap_chain, create_swap_target, pixel_format, read_bitmap_bgra, GraphicsDevice,
};
use crate::z_bands::{raise_band_order, Band, BandRegistration};
use edutools_core::settings::AppSettings;
use edutools_core::timer::{
    break_progress_presentation, break_resume_text, break_timer_layout_metrics,
    break_timer_start_decision, BreakProgressColor, BreakTimerEngine, BreakTimerPhase,
    BreakTimerStartDecision, BreakTimerStartInput,
};
use std::f64::consts::PI;
use std::ffi::c_void;
use std::mem::size_of;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::{w, Error, Interface, Result, PCWSTR};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT,
    SYSTEMTIME, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT, D2D_RECT_F,
    D2D_SIZE_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap1, ID2D1DeviceContext, ID2D1GeometrySink, ID2D1Image, ID2D1PathGeometry,
    ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle, D2D1_ARC_SEGMENT,
    D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE,
    D2D1_LINE_JOIN_ROUND, D2D1_PRIMITIVE_BLEND_COPY, D2D1_PRIMITIVE_BLEND_SOURCE_OVER,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE,
};
use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
use windows::Win32::Graphics::Dxgi::IDXGISwapChain1;
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, GetSysColorBrush, SetBkMode, SetTextColor,
    CLEARTYPE_QUALITY, COLOR_WINDOW, DEFAULT_CHARSET, HBRUSH, HDC, HFONT, LOGFONTW, TRANSPARENT,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, DATETIMEPICK_CLASSW, DTM_GETSYSTEMTIME, DTM_SETFORMATW,
    DTM_SETSYSTEMTIME, DTS_TIMEFORMAT, DTS_UPDOWN, GDT_VALID, ICC_DATE_CLASSES,
    INITCOMMONCONTROLSEX, WM_MOUSELEAVE,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
    VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_SPACE, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, GetClientRect, GetCursorPos, GetWindowLongPtrW, IsChild, IsWindowVisible,
    KillTimer, LoadCursorW, PostMessageW, RegisterClassW, SendMessageW, SetForegroundWindow,
    SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, TrackPopupMenuEx,
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CREATESTRUCTW, GWLP_USERDATA, HTCAPTION, HTCLIENT, IDC_ARROW,
    MF_GRAYED, MF_STRING, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOW, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DPICHANGED,
    WM_ERASEBKGND, WM_GETDLGCODE, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOVE,
    WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_NOTIFY, WM_SETFONT, WM_SIZE, WM_TIMER, WNDCLASSW,
    WS_CAPTION, WS_CHILD, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

pub const WM_APP_BREAK_ACTION: u32 = 0x8000 + 160;
pub const WM_APP_BREAK_TICK: u32 = 0x8000 + 161;

const SCREEN_CLASS: PCWSTR = w!("EduToolsBreakScreen");
const SETUP_CLASS: PCWSTR = w!("EduToolsBreakSetup");
const RUN_TIMER_ID: usize = 0xED08;
const RUN_TIMER_INTERVAL_MS: u32 = 250;

const ACTION_START_SETUP: usize = 1;
const ACTION_CHANGE_END: usize = 2;
const ACTION_TOGGLE_COLLAPSED: usize = 3;
const ACTION_END_EARLY: usize = 4;
const ACTION_TOGGLE_MUSIC: usize = 5;
const ACTION_PLAYLIST_BASE: usize = 100;

const CONTROL_QUICK_5: i32 = 201;
const CONTROL_QUICK_10: i32 = 202;
const CONTROL_QUICK_15: i32 = 203;
const CONTROL_PICKER: i32 = 204;
const CONTROL_WARNING: i32 = 205;
const CONTROL_CANCEL: i32 = 206;
const CONTROL_START: i32 = 207;

macro_rules! d2d_point {
    ($x:expr, $y:expr $(,)?) => {{
        let mut point = D2D1_ELLIPSE::default().point;
        point.X = $x;
        point.Y = $y;
        point
    }};
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakStartOutcome {
    Started,
    Changed,
    Restarted,
    Warned,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BreakTickOutcome {
    pub did_finish: bool,
    pub now: f64,
    pub running_end_date: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ButtonId {
    Music,
    Playlist,
    Change,
    Collapse,
    EndEarly,
}

impl ButtonId {
    const ORDER: [Self; 5] = [
        Self::Music,
        Self::Playlist,
        Self::Change,
        Self::Collapse,
        Self::EndEarly,
    ];
}

#[derive(Clone, Copy, Debug, Default)]
struct FloatRect {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl FloatRect {
    fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }

    fn d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct BreakLayout {
    remaining: FloatRect,
    resume: FloatRect,
    ring: FloatRect,
    current: FloatRect,
    music: FloatRect,
    playlist: FloatRect,
    change: FloatRect,
    collapse: FloatRect,
    end_early: FloatRect,
}

impl BreakLayout {
    fn calculate(width: u32, height: u32, dpi: u32, collapsed: bool) -> Self {
        let scale = dpi.max(96) as f32 / 96.0;
        let w = width as f32;
        let h = height as f32;
        let gap = if collapsed { 10.0 } else { 20.0 } * scale;
        let remaining_h = if collapsed { 76.0 } else { 150.0 } * scale;
        let resume_h = if collapsed { 28.0 } else { 42.0 } * scale;
        let ring_h = if collapsed { 0.0 } else { 260.0 * scale };
        let current_h = if collapsed { 0.0 } else { 34.0 * scale };
        let row_h = if collapsed { 34.0 } else { 42.0 } * scale;
        let active_gaps = if collapsed { 3.0 } else { 5.0 };
        let total = remaining_h + resume_h + ring_h + current_h + row_h * 2.0 + gap * active_gaps;
        let mut y = ((h - total) / 2.0).max(8.0 * scale);
        let remaining = FloatRect::from_xywh(0.0, y, w, remaining_h);
        y += remaining_h + gap;
        let resume = FloatRect::from_xywh(0.0, y, w, resume_h);
        y += resume_h + gap;
        let ring = if collapsed {
            FloatRect::default()
        } else {
            let rect = FloatRect::from_xywh((w - ring_h) / 2.0, y, ring_h, ring_h);
            y += ring_h + gap;
            rect
        };
        let current = if collapsed {
            FloatRect::default()
        } else {
            let rect = FloatRect::from_xywh(0.0, y, w, current_h);
            y += current_h + gap;
            rect
        };
        let music_w = 128.0 * scale;
        let playlist_w = 190.0 * scale;
        let music_gap = 12.0 * scale;
        let music_start = (w - music_w - playlist_w - music_gap) / 2.0;
        let music = FloatRect::from_xywh(music_start, y, music_w, row_h);
        let playlist = FloatRect::from_xywh(music.right + music_gap, y, playlist_w, row_h);
        y += row_h + gap;
        let button_w = if collapsed { 116.0 } else { 150.0 } * scale;
        let button_gap = 12.0 * scale;
        let row_width = button_w * 3.0 + button_gap * 2.0;
        let row_start = (w - row_width) / 2.0;
        let change = FloatRect::from_xywh(row_start, y, button_w, row_h);
        let collapse = FloatRect::from_xywh(change.right + button_gap, y, button_w, row_h);
        let end_early = FloatRect::from_xywh(collapse.right + button_gap, y, button_w, row_h);
        Self {
            remaining,
            resume,
            ring,
            current,
            music,
            playlist,
            change,
            collapse,
            end_early,
        }
    }

    fn button(self, id: ButtonId) -> FloatRect {
        match id {
            ButtonId::Music => self.music,
            ButtonId::Playlist => self.playlist,
            ButtonId::Change => self.change,
            ButtonId::Collapse => self.collapse,
            ButtonId::EndEarly => self.end_early,
        }
    }

    fn hit(self, x: f32, y: f32) -> Option<ButtonId> {
        ButtonId::ORDER
            .into_iter()
            .find(|id| self.button(*id).contains(x, y))
    }
}

#[derive(Clone)]
struct BreakSettings {
    background: [f64; 4],
    message: String,
    music_enabled: bool,
    playlists: Vec<String>,
    selected_playlist: usize,
}

impl BreakSettings {
    fn from_app(settings: &AppSettings) -> Self {
        Self {
            background: [
                settings.timer.break_background_color.red,
                settings.timer.break_background_color.green,
                settings.timer.break_background_color.blue,
                0.96,
            ],
            message: settings.timer.break_message.clone(),
            music_enabled: settings.timer.music_enabled,
            playlists: settings.timer.playlist_urls.clone(),
            selected_playlist: usize::try_from(settings.timer.default_playlist_index).unwrap_or(0),
        }
    }
}

struct BreakWindowState {
    owner: HWND,
    logger: Logger,
    screen: HWND,
    setup: HWND,
    setup_title: HWND,
    picker: HWND,
    warning: HWND,
    setup_font: HFONT,
    setup_title_font: HFONT,
    engine: BreakTimerEngine,
    settings: BreakSettings,
    renderer: Option<BreakRenderer>,
    target_monitor: Option<String>,
    collapsed: bool,
    dpi: u32,
    layout: BreakLayout,
    hovered: Option<ButtonId>,
    pressed: Option<ButtonId>,
    focused: Option<ButtonId>,
    quick_end_date: Option<f64>,
    timer_running: bool,
}

impl BreakWindowState {
    fn render_now(&mut self) -> Result<()> {
        let now = wall_clock_timestamp();
        let remaining = self.engine.remaining(now);
        let end_date = self.engine.end_date_value();
        let phase = self.engine.phase();
        let time = end_date.map(|end| local_time_string_at(end, now));
        let presentation = BreakPresentation {
            remaining: if phase == BreakTimerPhase::Finished {
                "수업 재개".to_owned()
            } else {
                formatted_duration(remaining)
            },
            resume: break_resume_text(&self.settings.message, time.as_deref()),
            current: format!("현재 시각 {}", current_local_time_string()),
            progress: break_progress_presentation(remaining, self.engine.initial_duration(), phase),
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(());
        };
        self.layout =
            BreakLayout::calculate(renderer.width, renderer.height, self.dpi, self.collapsed);
        renderer.render(
            &presentation,
            &self.settings,
            self.layout,
            self.collapsed,
            self.hovered,
            self.pressed,
            self.focused,
        )
    }

    fn sync_timer(&mut self) -> Result<()> {
        let needed = self.engine.phase() == BreakTimerPhase::Running && !self.screen.is_invalid();
        match (self.timer_running, needed) {
            (false, true) => {
                let timer = unsafe {
                    SetTimer(Some(self.screen), RUN_TIMER_ID, RUN_TIMER_INTERVAL_MS, None)
                };
                if timer == 0 {
                    return Err(Error::new(
                        E_FAIL,
                        "break screen refresh timer start failed",
                    ));
                }
                self.timer_running = true;
            }
            (true, false) => {
                unsafe { KillTimer(Some(self.screen), RUN_TIMER_ID) }?;
                self.timer_running = false;
            }
            _ => {}
        }
        Ok(())
    }

    fn target_monitor(&self) -> Result<Monitor> {
        let monitors = enumerate_monitors()?;
        if let Some(key) = self.target_monitor.as_deref() {
            if let Some(monitor) = monitor_by_key(&monitors, key) {
                return Ok(monitor.clone());
            }
            return monitors
                .iter()
                .find(|monitor| monitor.is_primary)
                .cloned()
                .or_else(|| monitors.first().cloned())
                .ok_or_else(|| Error::new(E_FAIL, "break screen requires at least one monitor"));
        }
        let mut cursor = POINT::default();
        let pointed = unsafe { GetCursorPos(&mut cursor) }
            .ok()
            .and_then(|_| monitor_at_point(&monitors, cursor).cloned());
        pointed
            .or_else(|| monitors.iter().find(|monitor| monitor.is_primary).cloned())
            .or_else(|| monitors.first().cloned())
            .ok_or_else(|| Error::new(E_FAIL, "break screen requires at least one monitor"))
    }

    fn apply_presentation_frame(&mut self) -> Result<()> {
        let monitor = self.target_monitor()?;
        self.target_monitor = Some(monitor.session_key.clone());
        self.dpi = monitor.dpi.0.max(96);
        let metrics = break_timer_layout_metrics(self.collapsed, self.dpi);
        let (x, y, width, height) = if self.collapsed {
            let width = metrics.compact_width as i32;
            let height = metrics.compact_height as i32;
            (
                monitor.work_rect.left + (monitor.work_rect.width() - width) / 2,
                monitor.work_rect.top + (monitor.work_rect.height() - height) / 2,
                width,
                height,
            )
        } else {
            (
                monitor.rect.left,
                monitor.rect.top,
                monitor.rect.width(),
                monitor.rect.height(),
            )
        };
        unsafe { SetWindowPos(self.screen, None, x, y, width, height, SWP_NOZORDER) }?;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(width, height, self.dpi)?;
        }
        self.render_now()
    }

    fn show_screen(&mut self) -> Result<()> {
        self.apply_presentation_frame()?;
        let _ = unsafe { ShowWindow(self.screen, SW_SHOW) };
        raise_band_order()?;
        let _ = unsafe { SetForegroundWindow(self.screen) };
        let _ = unsafe { SetFocus(Some(self.screen)) };
        self.sync_timer()
    }

    fn hide_windows(&mut self) {
        let _ = unsafe { ShowWindow(self.screen, SW_HIDE) };
        let _ = unsafe { ShowWindow(self.setup, SW_HIDE) };
        if self.timer_running {
            if let Err(error) = unsafe { KillTimer(Some(self.screen), RUN_TIMER_ID) } {
                self.logger
                    .event(&format!("쉬는 시간 갱신 타이머 중단 실패: {error}"));
            }
            self.timer_running = false;
        }
    }

    fn show_warning(&mut self, reason: &str) {
        set_window_text(self.warning, reason);
        let _ = unsafe { ShowWindow(self.warning, SW_SHOW) };
        self.logger
            .event(&format!("쉬는 시간 시작 경고 표시: {reason}"));
    }

    fn hide_warning(&mut self) {
        let _ = unsafe { ShowWindow(self.warning, SW_HIDE) };
    }

    fn post_action(&self, action: usize) {
        if let Err(error) = unsafe {
            PostMessageW(
                Some(self.owner),
                WM_APP_BREAK_ACTION,
                WPARAM(action),
                LPARAM(0),
            )
        } {
            self.logger.event(&format!(
                "쉬는 시간 화면 동작 전달 실패: action={action}: {error}"
            ));
        }
    }
}

pub struct BreakTimerManager {
    state: Box<BreakWindowState>,
    _screen_band: BandRegistration,
    _setup_band: BandRegistration,
    _graphics: GraphicsDevice,
}

impl BreakTimerManager {
    pub fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        settings: &AppSettings,
        logger: Logger,
    ) -> Result<Self> {
        register_classes(hinstance)?;
        let controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_DATE_CLASSES,
        };
        if !unsafe { InitCommonControlsEx(&controls) }.as_bool() {
            return Err(Error::from_thread());
        }
        let mut state = Box::new(BreakWindowState {
            owner,
            logger,
            screen: HWND::default(),
            setup: HWND::default(),
            setup_title: HWND::default(),
            picker: HWND::default(),
            warning: HWND::default(),
            setup_font: HFONT::default(),
            setup_title_font: HFONT::default(),
            engine: BreakTimerEngine::new(),
            settings: BreakSettings::from_app(settings),
            renderer: None,
            target_monitor: None,
            collapsed: false,
            dpi: 96,
            layout: BreakLayout::default(),
            hovered: None,
            pressed: None,
            focused: Some(ButtonId::Music),
            quick_end_date: None,
            timer_running: false,
        });
        let state_ptr = state.as_mut() as *mut BreakWindowState;
        let screen = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP,
                SCREEN_CLASS,
                w!("EduTools Break Screen"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                Some(owner),
                None,
                Some(hinstance),
                Some(state_ptr.cast()),
            )
        }?;
        state.screen = screen;
        let (setup_width, setup_height) = setup_outer_size(96)?;
        // DateTimePicker is a child HWND, so this is a normal top-level window rather than DComp.
        let setup = match unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                SETUP_CLASS,
                w!("쉬는 시간 타이머"),
                WS_POPUP | WS_CAPTION | WS_SYSMENU,
                0,
                0,
                setup_width,
                setup_height,
                Some(owner),
                None,
                Some(hinstance),
                Some(state_ptr.cast()),
            )
        } {
            Ok(hwnd) => hwnd,
            Err(error) => {
                let _ = unsafe { DestroyWindow(screen) };
                return Err(error);
            }
        };
        state.setup = setup;
        if let Err(error) = create_setup_controls(&mut state, hinstance) {
            let _ = unsafe { DestroyWindow(setup) };
            let _ = unsafe { DestroyWindow(screen) };
            return Err(error);
        }
        let screen_band = BandRegistration::new(screen, Band::Guidance);
        let setup_band = BandRegistration::new(setup, Band::Dialog);
        let graphics = match GraphicsDevice::new() {
            Ok(graphics) => graphics,
            Err(error) => {
                let _ = unsafe { DestroyWindow(setup) };
                let _ = unsafe { DestroyWindow(screen) };
                return Err(error);
            }
        };
        state.renderer = Some(match BreakRenderer::new(&graphics, screen, 420, 230, 96) {
            Ok(renderer) => renderer,
            Err(error) => {
                let _ = unsafe { DestroyWindow(setup) };
                let _ = unsafe { DestroyWindow(screen) };
                return Err(error);
            }
        });
        Ok(Self {
            state,
            _screen_band: screen_band,
            _setup_band: setup_band,
            _graphics: graphics,
        })
    }

    pub fn open_setup(&mut self, settings: &AppSettings) -> Result<()> {
        self.apply_settings(settings)?;
        let now = wall_clock_timestamp();
        let proposed = if self.state.engine.phase() == BreakTimerPhase::Running {
            self.state.engine.end_date_value().unwrap_or_else(|| {
                BreakTimerEngine::end_date_adding_minutes(settings.timer.break_default_minutes, now)
            })
        } else {
            BreakTimerEngine::end_date_adding_minutes(settings.timer.break_default_minutes, now)
        };
        set_picker_timestamp(self.state.picker, proposed, now);
        self.state.quick_end_date = Some(proposed);
        self.state.hide_warning();
        self.position_setup_at_pointer()?;
        let _ = unsafe { ShowWindow(self.state.setup, SW_SHOW) };
        raise_band_order()?;
        let _ = unsafe { SetForegroundWindow(self.state.setup) };
        let _ = unsafe { SetFocus(Some(self.state.picker)) };
        Ok(())
    }

    pub fn start_quick(
        &mut self,
        minutes: i64,
        settings: &AppSettings,
        source: &str,
    ) -> Result<BreakStartOutcome> {
        self.apply_settings(settings)?;
        let now = wall_clock_timestamp();
        let was_running = self.state.engine.phase() == BreakTimerPhase::Running;
        let end_date = BreakTimerEngine::end_date_adding_minutes(minutes, now);
        if let Err(error) = self.state.engine.start(end_date, now) {
            self.state.logger.event(&format!(
                "쉬는 시간 시작 거부: source={source} reason={error}"
            ));
            return Err(Error::new(E_FAIL, error.to_string()));
        }
        self.state.target_monitor = None;
        self.state.collapsed = false;
        self.state.show_screen()?;
        self.state.logger.event(&format!(
            "쉬는 시간 시작: monitor={} end={} source={source}",
            self.state.target_monitor.as_deref().unwrap_or("unknown"),
            local_time_string_at(end_date, now)
        ));
        Ok(if was_running {
            BreakStartOutcome::Restarted
        } else {
            BreakStartOutcome::Started
        })
    }

    pub fn start_from_setup(&mut self, settings: &AppSettings) -> Result<BreakStartOutcome> {
        self.apply_settings(settings)?;
        let now = wall_clock_timestamp();
        let running = self.state.engine.phase() == BreakTimerPhase::Running;
        let input = if let Some(end_date) = self.state.quick_end_date {
            BreakTimerStartInput::Quick { end_date }
        } else {
            let local = picker_system_time(self.state.picker)?;
            let current = unsafe { GetLocalTime() };
            let seconds_since_midnight = f64::from(current.wHour) * 3_600.0
                + f64::from(current.wMinute) * 60.0
                + f64::from(current.wSecond)
                + f64::from(current.wMilliseconds) / 1_000.0;
            BreakTimerStartInput::TimeOfDay {
                hour: u8::try_from(local.wHour).unwrap_or(24),
                minute: u8::try_from(local.wMinute).unwrap_or(60),
                start_of_day: now - seconds_since_midnight,
            }
        };
        let decision = break_timer_start_decision(input, now, running);
        let (end_date, outcome) = match decision {
            BreakTimerStartDecision::WarnPast => {
                self.state.show_warning("오늘 시각이 지났습니다");
                return Ok(BreakStartOutcome::Warned);
            }
            BreakTimerStartDecision::Start { end_date } => (end_date, BreakStartOutcome::Started),
            BreakTimerStartDecision::Change { end_date } => (end_date, BreakStartOutcome::Changed),
        };
        if let Err(error) = self.state.engine.start(end_date, now) {
            self.state.show_warning("오늘 시각이 지났습니다");
            self.state
                .logger
                .event(&format!("쉬는 시간 시작 거부: source=설정 reason={error}"));
            return Ok(BreakStartOutcome::Warned);
        }
        let _ = unsafe { ShowWindow(self.state.setup, SW_HIDE) };
        if !running {
            self.state.target_monitor = None;
        }
        self.state.collapsed = false;
        self.state.show_screen()?;
        if outcome == BreakStartOutcome::Changed {
            self.state.logger.event(&format!(
                "쉬는 시간 종료 시각 변경: end={} source=설정",
                local_time_string_at(end_date, now)
            ));
        } else {
            self.state.logger.event(&format!(
                "쉬는 시간 시작: monitor={} end={} source=설정",
                self.state.target_monitor.as_deref().unwrap_or("unknown"),
                local_time_string_at(end_date, now)
            ));
        }
        Ok(outcome)
    }

    pub fn tick(&mut self, settings: &AppSettings) -> Result<BreakTickOutcome> {
        self.apply_settings(settings)?;
        let now = wall_clock_timestamp();
        let did_finish = self.state.engine.synchronize(now);
        self.state.render_now()?;
        self.state.sync_timer()?;
        if did_finish {
            self.state.logger.event("쉬는 시간 완료");
        }
        Ok(BreakTickOutcome {
            did_finish,
            now,
            running_end_date: (self.state.engine.phase() == BreakTimerPhase::Running)
                .then(|| self.state.engine.end_date_value())
                .flatten(),
        })
    }

    pub fn toggle_collapsed(&mut self) -> Result<()> {
        if self.state.collapsed {
            let mut rect = RECT::default();
            if unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowRect(self.state.screen, &mut rect)
            }
            .is_ok()
            {
                let monitors = enumerate_monitors()?;
                let center = POINT {
                    x: rect.left + (rect.right - rect.left) / 2,
                    y: rect.top + (rect.bottom - rect.top) / 2,
                };
                if let Some(monitor) = monitor_at_point(&monitors, center) {
                    self.state.target_monitor = Some(monitor.session_key.clone());
                }
            }
        }
        self.state.collapsed = !self.state.collapsed;
        self.state.hovered = None;
        self.state.pressed = None;
        self.state.apply_presentation_frame()?;
        raise_band_order()?;
        self.state.logger.event(if self.state.collapsed {
            "쉬는 시간 화면 접힘"
        } else {
            "쉬는 시간 화면 펼침"
        });
        Ok(())
    }

    pub fn end_early(&mut self) {
        self.state.engine.end_early();
        self.state.hide_windows();
        self.state.logger.event("쉬는 시간 조기 종료");
    }

    pub fn cancel_setup(&mut self) {
        let _ = unsafe { ShowWindow(self.state.setup, SW_HIDE) };
    }

    pub fn apply_settings(&mut self, settings: &AppSettings) -> Result<()> {
        self.state.settings = BreakSettings::from_app(settings);
        if self.state.focused == Some(ButtonId::Playlist)
            && self.state.settings.playlists.is_empty()
        {
            self.state.focused = Some(ButtonId::Music);
        }
        if unsafe { IsWindowVisible(self.state.screen) }.as_bool() {
            self.state.render_now()?;
        }
        Ok(())
    }

    pub fn display_configuration_changed(&mut self, reason: &str) -> Result<()> {
        if !unsafe { IsWindowVisible(self.state.screen) }.as_bool() {
            return Ok(());
        }
        self.state.apply_presentation_frame()?;
        self.state
            .logger
            .event(&format!("쉬는 시간 화면 구성 변경 재배치: {reason}"));
        Ok(())
    }

    pub fn synchronize_after_system_event(
        &mut self,
        settings: &AppSettings,
        reason: &str,
    ) -> Result<BreakTickOutcome> {
        self.state
            .logger
            .event(&format!("쉬는 시간 잠자기/시간 변경 복귀 동기화: {reason}"));
        self.tick(settings)
    }

    pub fn raise_to_front(&mut self) -> Result<()> {
        if self.state.engine.phase() == BreakTimerPhase::Inactive {
            return Ok(());
        }
        raise_band_order()?;
        let _ = unsafe { SetForegroundWindow(self.state.screen) };
        Ok(())
    }

    pub fn running_timing(&self) -> Option<(f64, f64)> {
        (self.state.engine.phase() == BreakTimerPhase::Running)
            .then(|| {
                self.state
                    .engine
                    .end_date_value()
                    .map(|end_date| (wall_clock_timestamp(), end_date))
            })
            .flatten()
    }

    pub fn is_running(&self) -> bool {
        self.state.engine.phase() == BreakTimerPhase::Running
    }

    pub fn playlist_index_from_action(action: usize) -> Option<i64> {
        action
            .checked_sub(ACTION_PLAYLIST_BASE)
            .and_then(|index| i64::try_from(index).ok())
    }

    pub const fn action_start_setup(action: usize) -> bool {
        action == ACTION_START_SETUP
    }

    pub const fn action_change_end(action: usize) -> bool {
        action == ACTION_CHANGE_END
    }

    pub const fn action_toggle_collapsed(action: usize) -> bool {
        action == ACTION_TOGGLE_COLLAPSED
    }

    pub const fn action_end_early(action: usize) -> bool {
        action == ACTION_END_EARLY
    }

    pub const fn action_toggle_music(action: usize) -> bool {
        action == ACTION_TOGGLE_MUSIC
    }

    pub fn pretranslate_message(&mut self, hwnd: HWND, message: u32, wparam: WPARAM) -> bool {
        if !unsafe { IsWindowVisible(self.state.setup) }.as_bool()
            || (hwnd != self.state.setup && !unsafe { IsChild(self.state.setup, hwnd) }.as_bool())
            || message != WM_KEYDOWN
        {
            return false;
        }
        match wparam.0 as u32 {
            value if value == VK_RETURN.0 as u32 => {
                self.state.post_action(ACTION_START_SETUP);
                true
            }
            value if value == VK_ESCAPE.0 as u32 => {
                self.cancel_setup();
                true
            }
            _ => false,
        }
    }

    pub fn selftest_failures(&mut self) -> Vec<String> {
        let Some(renderer) = self.state.renderer.as_mut() else {
            return vec!["break screen: renderer 없음".to_owned()];
        };
        renderer
            .selftest_pixel_failures(&self.state.settings)
            .unwrap_or_else(|error| vec![format!("break screen: 픽셀 readback 실패: {error}")])
    }

    fn position_setup_at_pointer(&mut self) -> Result<()> {
        let monitors = enumerate_monitors()?;
        let mut cursor = POINT::default();
        let monitor = unsafe { GetCursorPos(&mut cursor) }
            .ok()
            .and_then(|_| monitor_at_point(&monitors, cursor))
            .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
            .or_else(|| monitors.first())
            .ok_or_else(|| Error::new(E_FAIL, "break setup requires at least one monitor"))?;
        let dpi = monitor.dpi.0.max(96);
        let (width, height) = setup_outer_size(dpi)?;
        let x = monitor.work_rect.left + (monitor.work_rect.width() - width) / 2;
        let y = monitor.work_rect.top + (monitor.work_rect.height() - height) / 2;
        unsafe { SetWindowPos(self.state.setup, None, x, y, width, height, SWP_NOZORDER) }?;
        self.state.dpi = dpi;
        layout_setup_controls(&self.state, dpi)?;
        update_setup_fonts(&mut self.state, dpi)
    }
}

impl Drop for BreakTimerManager {
    fn drop(&mut self) {
        self.state.hide_windows();
        if !self.state.setup.is_invalid() {
            let _ = unsafe { DestroyWindow(self.state.setup) };
        }
        if !self.state.screen.is_invalid() {
            let _ = unsafe { DestroyWindow(self.state.screen) };
        }
        for font in [self.state.setup_font, self.state.setup_title_font] {
            if !font.is_invalid() {
                let _ = unsafe { DeleteObject(font.into()) };
            }
        }
    }
}

struct BreakPresentation {
    remaining: String,
    resume: String,
    current: String,
    progress: edutools_core::timer::BreakProgressPresentation,
}

struct BreakRenderer {
    swap_chain: IDXGISwapChain1,
    context: ID2D1DeviceContext,
    factory: windows::Win32::Graphics::Direct2D::ID2D1Factory,
    round_stroke: ID2D1StrokeStyle,
    swap_target: Option<ID2D1Bitmap1>,
    dwrite: IDWriteFactory,
    _dcomp_target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    width: u32,
    height: u32,
    dpi: u32,
}

impl BreakRenderer {
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
        let factory = unsafe { context.GetFactory() }?;
        let round_stroke = unsafe {
            factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 10.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )
        }?;
        let swap_target = Some(create_swap_target(&context, &swap_chain)?);
        set_context_target(
            &context,
            swap_target.as_ref().expect("break screen target exists"),
        )?;
        let (dcomp_target, visual) = graphics.attach_composition_swap_chain(hwnd, &swap_chain)?;
        let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        Ok(Self {
            swap_chain,
            context,
            factory,
            round_stroke,
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
                .expect("break screen target recreated"),
        )?;
        self.width = width;
        self.height = height;
        self.dpi = dpi;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        presentation: &BreakPresentation,
        settings: &BreakSettings,
        layout: BreakLayout,
        collapsed: bool,
        hovered: Option<ButtonId>,
        pressed: Option<ButtonId>,
        focused: Option<ButtonId>,
    ) -> Result<()> {
        let target = self
            .swap_target
            .as_ref()
            .expect("break screen target exists")
            .clone();
        self.render_to(
            &target,
            presentation,
            settings,
            layout,
            collapsed,
            hovered,
            pressed,
            focused,
            true,
        )?;
        unsafe { self.swap_chain.Present(1, Default::default()).ok() }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_to(
        &self,
        target_bitmap: &ID2D1Bitmap1,
        presentation: &BreakPresentation,
        settings: &BreakSettings,
        layout: BreakLayout,
        collapsed: bool,
        hovered: Option<ButtonId>,
        pressed: Option<ButtonId>,
        focused: Option<ButtonId>,
        draw_background: bool,
    ) -> Result<()> {
        set_context_target(&self.context, target_bitmap)?;
        unsafe {
            self.context.BeginDraw();
            self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
            self.context.Clear(Some(&D2D1_COLOR_F::default()));
            self.context
                .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
        }
        let draw_result = (|| -> Result<()> {
            let scale = self.dpi.max(96) as f32 / 96.0;
            if draw_background {
                let fill = brush(&self.context, settings.background)?;
                let target: ID2D1RenderTarget = self.context.cast()?;
                if collapsed {
                    let radius = 18.0 * scale;
                    let rounded = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: 1.0,
                            top: 1.0,
                            right: self.width as f32 - 1.0,
                            bottom: self.height as f32 - 1.0,
                        },
                        radiusX: radius,
                        radiusY: radius,
                    };
                    let outline = brush(&self.context, [0.05, 0.05, 0.06, 0.95])?;
                    unsafe {
                        target.FillRoundedRectangle(&rounded, &fill);
                        target.DrawRoundedRectangle(&rounded, &outline, 2.0 * scale, None);
                    }
                } else {
                    unsafe {
                        target.FillRectangle(
                            &D2D_RECT_F {
                                left: 0.0,
                                top: 0.0,
                                right: self.width as f32,
                                bottom: self.height as f32,
                            },
                            &fill,
                        )
                    };
                }
            }
            let metrics = break_timer_layout_metrics(collapsed, self.dpi);
            let remaining_color = match presentation.progress.color {
                BreakProgressColor::NearEnd => [1.0, 0.58, 0.0, 1.0],
                BreakProgressColor::Finished => [0.95, 0.18, 0.18, 1.0],
                BreakProgressColor::Accent => [0.96, 0.97, 0.99, 1.0],
            };
            self.draw_text(
                &presentation.remaining,
                layout.remaining,
                metrics.remaining_font as f32,
                DWRITE_FONT_WEIGHT_BOLD,
                remaining_color,
                w!("Consolas"),
            )?;
            self.draw_text(
                &presentation.resume,
                layout.resume,
                metrics.resume_font as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                [0.78, 0.80, 0.84, 1.0],
                w!("Segoe UI"),
            )?;
            if !collapsed {
                self.draw_progress_ring(layout.ring, presentation.progress)?;
                self.draw_text(
                    &presentation.current,
                    layout.current,
                    24.0 * scale,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    [0.72, 0.74, 0.78, 1.0],
                    w!("Segoe UI"),
                )?;
            }
            let playlist_label = if settings.playlists.is_empty() {
                "등록된 재생목록 없음".to_owned()
            } else {
                format!("재생목록 {}", settings.selected_playlist.saturating_add(1))
            };
            let labels = [
                (
                    ButtonId::Music,
                    if settings.music_enabled {
                        "음악 끄기"
                    } else {
                        "음악 켜기"
                    },
                    true,
                ),
                (
                    ButtonId::Playlist,
                    playlist_label.as_str(),
                    !settings.playlists.is_empty(),
                ),
                (ButtonId::Change, "종료 시각 변경", true),
                (
                    ButtonId::Collapse,
                    if collapsed {
                        "전체 화면으로 펼치기"
                    } else {
                        "작은 창으로 접기"
                    },
                    true,
                ),
                (ButtonId::EndEarly, "조기 종료", true),
            ];
            for (id, label, enabled) in labels {
                self.draw_button(
                    layout.button(id),
                    label,
                    enabled,
                    hovered == Some(id),
                    pressed == Some(id),
                    focused == Some(id),
                    scale,
                )?;
            }
            Ok(())
        })();
        let end_result = unsafe { self.context.EndDraw(None, None) };
        draw_result?;
        end_result
    }

    fn draw_progress_ring(
        &self,
        rect: FloatRect,
        progress: edutools_core::timer::BreakProgressPresentation,
    ) -> Result<()> {
        let center = [
            (rect.left + rect.right) / 2.0,
            (rect.top + rect.bottom) / 2.0,
        ];
        let scale = self.dpi.max(96) as f32 / 96.0;
        let stroke = 14.0 * scale;
        let radius = ((rect.right - rect.left).min(rect.bottom - rect.top) - stroke * 2.0) / 2.0;
        let target: ID2D1RenderTarget = self.context.cast()?;
        let background = brush(&self.context, [0.56, 0.57, 0.60, 0.35])?;
        let mut ellipse = D2D1_ELLIPSE::default();
        ellipse.point.X = center[0];
        ellipse.point.Y = center[1];
        ellipse.radiusX = radius;
        ellipse.radiusY = radius;
        unsafe { target.DrawEllipse(&ellipse, &background, stroke, Some(&self.round_stroke)) };
        if progress.fraction <= 0.0 {
            return Ok(());
        }
        let tint = match progress.color {
            BreakProgressColor::Accent => [0.22, 0.74, 0.97, 1.0],
            BreakProgressColor::NearEnd => [1.0, 0.58, 0.0, 1.0],
            BreakProgressColor::Finished => [0.95, 0.18, 0.18, 1.0],
        };
        let foreground = brush(&self.context, tint)?;
        if progress.fraction >= 0.9999 {
            unsafe { target.DrawEllipse(&ellipse, &foreground, stroke, Some(&self.round_stroke)) };
            return Ok(());
        }
        let end_angle = 2.0 * PI * progress.fraction;
        let path = self.arc_geometry(center, f64::from(radius), end_angle)?;
        unsafe {
            target.DrawGeometry(&path, &foreground, stroke, Some(&self.round_stroke));
        }
        Ok(())
    }

    fn arc_geometry(
        &self,
        center: [f32; 2],
        radius: f64,
        end_angle: f64,
    ) -> Result<ID2D1PathGeometry> {
        let start = d2d_point!(center[0], center[1] - radius as f32);
        let end = d2d_point!(
            center[0] + (end_angle.sin() * radius) as f32,
            center[1] - (end_angle.cos() * radius) as f32,
        );
        let path = unsafe { self.factory.CreatePathGeometry() }?;
        let sink: ID2D1GeometrySink = unsafe { path.Open() }?;
        unsafe {
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: radius as f32,
                    height: radius as f32,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if end_angle > PI {
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

    #[allow(clippy::too_many_arguments)]
    fn draw_button(
        &self,
        rect: FloatRect,
        label: &str,
        enabled: bool,
        hovered: bool,
        pressed: bool,
        focused: bool,
        scale: f32,
    ) -> Result<()> {
        let fill = if !enabled {
            [0.18, 0.19, 0.22, 0.55]
        } else if pressed {
            [0.12, 0.52, 0.75, 0.95]
        } else if hovered {
            [0.24, 0.27, 0.32, 0.95]
        } else {
            [0.18, 0.20, 0.24, 0.92]
        };
        let rounded = D2D1_ROUNDED_RECT {
            rect: rect.d2d(),
            radiusX: 8.0 * scale,
            radiusY: 8.0 * scale,
        };
        let target: ID2D1RenderTarget = self.context.cast()?;
        let fill_brush = brush(&self.context, fill)?;
        let border = brush(&self.context, [0.75, 0.78, 0.84, 0.28])?;
        unsafe {
            target.FillRoundedRectangle(&rounded, &fill_brush);
            target.DrawRoundedRectangle(&rounded, &border, scale, None);
        }
        if focused {
            let focus = brush(&self.context, [0.22, 0.74, 0.97, 0.95])?;
            unsafe { target.DrawRoundedRectangle(&rounded, &focus, 2.0 * scale, None) };
        }
        self.draw_text(
            label,
            rect,
            14.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            if enabled {
                [0.96, 0.97, 0.99, 1.0]
            } else {
                [0.58, 0.60, 0.64, 1.0]
            },
            w!("Segoe UI"),
        )
    }

    fn draw_text(
        &self,
        value: &str,
        rect: FloatRect,
        size: f32,
        weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
        tint: [f64; 4],
        family: PCWSTR,
    ) -> Result<()> {
        let format = text_format(&self.dwrite, family, size, weight)?;
        let text = value.encode_utf16().collect::<Vec<_>>();
        let text_brush = brush(&self.context, tint)?;
        let target: ID2D1RenderTarget = self.context.cast()?;
        unsafe {
            target.DrawText(
                &text,
                &format,
                &rect.d2d(),
                &text_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    fn selftest_pixel_failures(&self, settings: &BreakSettings) -> Result<Vec<String>> {
        let width = 800;
        let height = 800;
        let target = create_target_bitmap(&self.context, width, height)?;
        let presentation = BreakPresentation {
            remaining: "05:00".to_owned(),
            resume: "수업 재개 시각 10:30".to_owned(),
            current: "현재 시각 10:25".to_owned(),
            progress: break_progress_presentation(150.0, 600.0, BreakTimerPhase::Running),
        };
        let layout = BreakLayout::calculate(width, height, 96, false);
        self.render_to(
            &target,
            &presentation,
            settings,
            layout,
            false,
            None,
            None,
            None,
            true,
        )?;
        let (pixels, pitch) = read_bitmap_bgra(&self.context, &target)?;
        let pixel_at = |x: u32, y: u32| -> [u8; 4] {
            let offset = y as usize * pitch as usize + x as usize * 4;
            [
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            ]
        };
        let mut failures = Vec::new();
        let background = pixel_at(4, 4);
        if !(240..=250).contains(&background[3]) {
            failures.push(format!(
                "break screen: 배경 alpha={} (기대 약 245)",
                background[3]
            ));
        }
        let center_x = ((layout.ring.left + layout.ring.right) / 2.0) as u32;
        let center_y = ((layout.ring.top + layout.ring.bottom) / 2.0) as u32;
        let radius = ((layout.ring.right - layout.ring.left) / 2.0 - 14.0) as u32;
        let foreground = pixel_at(center_x, center_y.saturating_sub(radius));
        let background_ring = pixel_at(center_x.saturating_sub(radius), center_y);
        if foreground == background_ring {
            failures.push("break screen: 25% 호의 전경/배경 링 픽셀이 같음".to_owned());
        }
        let compact_target = create_target_bitmap(&self.context, width, height)?;
        let compact_layout = BreakLayout::calculate(420, 230, 96, true);
        self.render_to(
            &compact_target,
            &presentation,
            settings,
            compact_layout,
            true,
            None,
            None,
            None,
            false,
        )?;
        let (compact_pixels, compact_pitch) = read_bitmap_bgra(&self.context, &compact_target)?;
        let ring_offset = center_y as usize * compact_pitch as usize + center_x as usize * 4 + 3;
        if compact_pixels[ring_offset] != 0 {
            failures.push(format!(
                "break screen: 접힘 링 영역 alpha={} (기대 0)",
                compact_pixels[ring_offset]
            ));
        }
        if let Some(swap_target) = self.swap_target.as_ref() {
            set_context_target(&self.context, swap_target)?;
        }
        Ok(failures)
    }
}

unsafe extern "system" fn screen_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut BreakWindowState };
    if message == WM_NCCREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        if !create.is_null() {
            state_ptr = unsafe { (*create).lpCreateParams.cast::<BreakWindowState>() };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize) };
        }
    }
    if !state_ptr.is_null() {
        let state = unsafe { &mut *state_ptr };
        match message {
            WM_ERASEBKGND => return LRESULT(1),
            WM_SIZE => {
                if let Some(renderer) = state.renderer.as_mut() {
                    let mut rect = RECT::default();
                    if unsafe { GetClientRect(hwnd, &mut rect) }.is_ok() {
                        let width = rect.right - rect.left;
                        let height = rect.bottom - rect.top;
                        if width > 0 && height > 0 {
                            if let Err(error) = renderer.resize(width, height, state.dpi) {
                                state
                                    .logger
                                    .event(&format!("쉬는 시간 화면 리사이즈 실패: {error}"));
                            }
                        }
                    }
                }
                let _ = state.render_now();
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == RUN_TIMER_ID => {
                if let Err(error) = unsafe {
                    PostMessageW(Some(state.owner), WM_APP_BREAK_TICK, WPARAM(0), LPARAM(0))
                } {
                    state
                        .logger
                        .event(&format!("쉬는 시간 tick 전달 실패: {error}"));
                }
                return LRESULT(0);
            }
            WM_MOVE if state.collapsed => {
                let mut rect = RECT::default();
                if unsafe {
                    windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect)
                }
                .is_ok()
                {
                    if let Ok(monitors) = enumerate_monitors() {
                        let center = POINT {
                            x: rect.left + (rect.right - rect.left) / 2,
                            y: rect.top + (rect.bottom - rect.top) / 2,
                        };
                        if let Some(monitor) = monitor_at_point(&monitors, center) {
                            state.target_monitor = Some(monitor.session_key.clone());
                        }
                    }
                }
                return LRESULT(0);
            }
            WM_MOUSEMOVE => {
                let (x, y) = lparam_point(lparam);
                let hovered = state.layout.hit(x, y);
                if hovered != state.hovered {
                    state.hovered = hovered;
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = unsafe { TrackMouseEvent(&mut track) };
                    let _ = state.render_now();
                }
                return LRESULT(0);
            }
            WM_MOUSELEAVE => {
                state.hovered = None;
                state.pressed = None;
                let _ = state.render_now();
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                let (x, y) = lparam_point(lparam);
                state.pressed = state
                    .layout
                    .hit(x, y)
                    .filter(|button| button_enabled(state, *button));
                if let Some(button) = state.pressed {
                    state.focused = Some(button);
                    unsafe { SetCapture(hwnd) };
                    let _ = unsafe { SetFocus(Some(hwnd)) };
                    let _ = state.render_now();
                }
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let (x, y) = lparam_point(lparam);
                let pressed = state.pressed.take();
                let _ = unsafe { ReleaseCapture() };
                if pressed.is_some() && pressed == state.layout.hit(x, y) {
                    activate_button(state, pressed.expect("checked Some"), hwnd);
                }
                let _ = state.render_now();
                return LRESULT(0);
            }
            WM_NCHITTEST if state.collapsed => {
                let screen_x = (lparam.0 as i32 as i16) as i32;
                let screen_y = ((lparam.0 >> 16) as i16) as i32;
                let mut origin = POINT { x: 0, y: 0 };
                let _ = unsafe { windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin) };
                let x = (screen_x - origin.x) as f32;
                let y = (screen_y - origin.y) as f32;
                return if state.layout.hit(x, y).is_some() {
                    LRESULT(HTCLIENT as isize)
                } else {
                    LRESULT(HTCAPTION as isize)
                };
            }
            WM_GETDLGCODE => return LRESULT(0x0004),
            WM_KEYDOWN => {
                match wparam.0 as u32 {
                    value if value == VK_TAB.0 as u32 => {
                        let current = state
                            .focused
                            .and_then(|focused| {
                                ButtonId::ORDER.iter().position(|id| *id == focused)
                            })
                            .unwrap_or(0);
                        let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                        let step = if shift { ButtonId::ORDER.len() - 1 } else { 1 };
                        state.focused = (1..=ButtonId::ORDER.len())
                            .map(|offset| {
                                ButtonId::ORDER[(current + offset * step) % ButtonId::ORDER.len()]
                            })
                            .find(|button| button_enabled(state, *button));
                        let _ = state.render_now();
                    }
                    value if value == VK_RETURN.0 as u32 || value == VK_SPACE.0 as u32 => {
                        if let Some(focused) = state.focused {
                            activate_button(state, focused, hwnd);
                        }
                    }
                    value if value == VK_ESCAPE.0 as u32 => {
                        // Swift의 borderless panel에도 Escape 동작이 없으므로 접거나 닫지 않는다.
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                state.dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
                if let Err(error) = state.apply_presentation_frame() {
                    state
                        .logger
                        .event(&format!("쉬는 시간 화면 DPI 재배치 실패: {error}"));
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn setup_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut BreakWindowState };
    if message == WM_NCCREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        if !create.is_null() {
            state_ptr = unsafe { (*create).lpCreateParams.cast::<BreakWindowState>() };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize) };
        }
    }
    if !state_ptr.is_null() {
        let state = unsafe { &mut *state_ptr };
        match message {
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as i32;
                match id {
                    CONTROL_QUICK_5 | CONTROL_QUICK_10 | CONTROL_QUICK_15 => {
                        let minutes = match id {
                            CONTROL_QUICK_5 => 5,
                            CONTROL_QUICK_10 => 10,
                            _ => 15,
                        };
                        let now = wall_clock_timestamp();
                        let end = BreakTimerEngine::end_date_adding_minutes(minutes, now);
                        set_picker_timestamp(state.picker, end, now);
                        state.quick_end_date = Some(end);
                        state.hide_warning();
                    }
                    CONTROL_CANCEL => {
                        let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                    }
                    CONTROL_START => state.post_action(ACTION_START_SETUP),
                    _ => {}
                }
                return LRESULT(0);
            }
            WM_NOTIFY => {
                state.quick_end_date = None;
                state.hide_warning();
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                return LRESULT(0);
            }
            WM_CTLCOLORSTATIC => {
                if HWND(lparam.0 as *mut c_void) == state.warning {
                    let hdc = HDC(wparam.0 as *mut c_void);
                    unsafe {
                        SetTextColor(hdc, COLORREF(0x000000e5));
                        SetBkMode(hdc, TRANSPARENT);
                    }
                    return LRESULT(unsafe { GetSysColorBrush(COLOR_WINDOW) }.0 as isize);
                }
            }
            WM_DPICHANGED => {
                state.dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
                let rect = lparam.0 as *const RECT;
                if !rect.is_null() {
                    let rect = unsafe { *rect };
                    let _ = unsafe {
                        SetWindowPos(
                            hwnd,
                            None,
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        )
                    };
                }
                let _ = layout_setup_controls(state, state.dpi);
                if let Err(error) = update_setup_fonts(state, state.dpi) {
                    state
                        .logger
                        .event(&format!("쉬는 시간 설정 창 DPI 글꼴 실패: {error}"));
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn activate_button(state: &mut BreakWindowState, button: ButtonId, hwnd: HWND) {
    match button {
        ButtonId::Music => state.post_action(ACTION_TOGGLE_MUSIC),
        ButtonId::Playlist => show_playlist_menu(state, hwnd),
        ButtonId::Change => state.post_action(ACTION_CHANGE_END),
        ButtonId::Collapse => state.post_action(ACTION_TOGGLE_COLLAPSED),
        ButtonId::EndEarly => state.post_action(ACTION_END_EARLY),
    }
}

fn button_enabled(state: &BreakWindowState, button: ButtonId) -> bool {
    button != ButtonId::Playlist || !state.settings.playlists.is_empty()
}

fn show_playlist_menu(state: &BreakWindowState, hwnd: HWND) {
    if state.settings.playlists.is_empty() {
        return;
    }
    let menu = match unsafe { CreatePopupMenu() } {
        Ok(menu) => menu,
        Err(error) => {
            state
                .logger
                .event(&format!("쉬는 시간 재생목록 메뉴 생성 실패: {error}"));
            return;
        }
    };
    for (index, _) in state.settings.playlists.iter().enumerate() {
        let label = wide(&format!("재생목록 {}", index + 1));
        let flags = if index > u16::MAX as usize {
            MF_GRAYED
        } else {
            MF_STRING
        };
        let _ = unsafe { AppendMenuW(menu, flags, index + 1, PCWSTR(label.as_ptr())) };
    }
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
        let command = unsafe {
            TrackPopupMenuEx(
                menu,
                (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
                cursor.x,
                cursor.y,
                hwnd,
                None,
            )
        };
        if command.0 > 0 {
            state.post_action(ACTION_PLAYLIST_BASE + command.0 as usize - 1);
        }
    }
    let _ = unsafe { DestroyMenu(menu) };
}

fn create_setup_controls(state: &mut BreakWindowState, hinstance: HINSTANCE) -> Result<()> {
    let controls = [
        (w!("STATIC"), w!("수업 재개 시각"), 0, 200),
        (
            w!("BUTTON"),
            w!("+5분"),
            BS_PUSHBUTTON as u32,
            CONTROL_QUICK_5,
        ),
        (
            w!("BUTTON"),
            w!("+10분"),
            BS_PUSHBUTTON as u32,
            CONTROL_QUICK_10,
        ),
        (
            w!("BUTTON"),
            w!("+15분"),
            BS_PUSHBUTTON as u32,
            CONTROL_QUICK_15,
        ),
        (
            w!("STATIC"),
            w!("오늘 시각이 지났습니다"),
            1,
            CONTROL_WARNING,
        ),
        (
            w!("BUTTON"),
            w!("취소"),
            BS_PUSHBUTTON as u32,
            CONTROL_CANCEL,
        ),
        (
            w!("BUTTON"),
            w!("시작"),
            BS_DEFPUSHBUTTON as u32,
            CONTROL_START,
        ),
    ];
    for (class, text, kind, id) in controls {
        let mut style = WS_CHILD | WS_VISIBLE | WINDOW_STYLE(kind);
        if id != 200 && id != CONTROL_WARNING {
            style |= WS_TABSTOP;
        }
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                class,
                text,
                style,
                0,
                0,
                1,
                1,
                Some(state.setup),
                Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                    id as usize as *mut c_void,
                )),
                Some(hinstance),
                None,
            )
        }?;
        if id == CONTROL_WARNING {
            state.warning = hwnd;
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        } else if id == 200 {
            state.setup_title = hwnd;
        }
    }
    state.picker = unsafe {
        CreateWindowExW(
            Default::default(),
            DATETIMEPICK_CLASSW,
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(DTS_TIMEFORMAT | DTS_UPDOWN),
            0,
            0,
            1,
            1,
            Some(state.setup),
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                CONTROL_PICKER as usize as *mut c_void,
            )),
            Some(hinstance),
            None,
        )
    }?;
    let format = wide("HH:mm");
    unsafe {
        SendMessageW(
            state.picker,
            DTM_SETFORMATW,
            None,
            Some(LPARAM(format.as_ptr() as isize)),
        )
    };
    layout_setup_controls(state, 96)?;
    update_setup_fonts(state, 96)
}

fn layout_setup_controls(state: &BreakWindowState, dpi: u32) -> Result<()> {
    let scale = f64::from(dpi.max(96)) / 96.0;
    let px = |value: f64| (value * scale).round() as i32;
    let positions = [
        (200, 24.0, 18.0, 382.0, 28.0),
        (CONTROL_QUICK_5, 92.0, 58.0, 76.0, 30.0),
        (CONTROL_QUICK_10, 177.0, 58.0, 76.0, 30.0),
        (CONTROL_QUICK_15, 262.0, 58.0, 76.0, 30.0),
        (CONTROL_PICKER, 120.0, 100.0, 190.0, 30.0),
        (CONTROL_WARNING, 80.0, 136.0, 270.0, 22.0),
        (CONTROL_CANCEL, 238.0, 176.0, 80.0, 32.0),
        (CONTROL_START, 326.0, 176.0, 80.0, 32.0),
    ];
    for (id, x, y, width, height) in positions {
        let hwnd = if id == CONTROL_PICKER {
            state.picker
        } else if id == CONTROL_WARNING {
            state.warning
        } else {
            unsafe { windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(state.setup), id) }?
        };
        if !hwnd.is_invalid() {
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    px(x),
                    px(y),
                    px(width),
                    px(height),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            }?;
        }
    }
    Ok(())
}

fn update_setup_fonts(state: &mut BreakWindowState, dpi: u32) -> Result<()> {
    let create = |size_dip: u32, weight: i32| -> Result<HFONT> {
        let pixel_height = size_dip.saturating_mul(dpi.max(96)).saturating_add(48) / 96;
        let mut logical_font = LOGFONTW {
            lfHeight: -(pixel_height as i32),
            lfWeight: weight,
            lfCharSet: DEFAULT_CHARSET,
            lfQuality: CLEARTYPE_QUALITY,
            ..Default::default()
        };
        for (destination, value) in logical_font
            .lfFaceName
            .iter_mut()
            .zip("Segoe UI Variable".encode_utf16())
        {
            *destination = value;
        }
        let font = unsafe { CreateFontIndirectW(&logical_font) };
        if font.is_invalid() {
            Err(Error::from_thread())
        } else {
            Ok(font)
        }
    };
    let normal = create(14, 400)?;
    let title = match create(20, 600) {
        Ok(title) => title,
        Err(error) => {
            let _ = unsafe { DeleteObject(normal.into()) };
            return Err(error);
        }
    };
    for hwnd in [
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(state.setup), CONTROL_QUICK_5)
        }?,
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(state.setup), CONTROL_QUICK_10)
        }?,
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(state.setup), CONTROL_QUICK_15)
        }?,
        state.picker,
        state.warning,
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(state.setup), CONTROL_CANCEL)
        }?,
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(state.setup), CONTROL_START)
        }?,
    ] {
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM(normal.0 as usize)),
                Some(LPARAM(1)),
            )
        };
    }
    unsafe {
        SendMessageW(
            state.setup_title,
            WM_SETFONT,
            Some(WPARAM(title.0 as usize)),
            Some(LPARAM(1)),
        )
    };
    let old_normal = std::mem::replace(&mut state.setup_font, normal);
    let old_title = std::mem::replace(&mut state.setup_title_font, title);
    for font in [old_normal, old_title] {
        if !font.is_invalid() {
            let _ = unsafe { DeleteObject(font.into()) };
        }
    }
    Ok(())
}

fn register_classes(hinstance: HINSTANCE) -> Result<()> {
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }?;
    for (name, procedure, background) in [
        (
            SCREEN_CLASS,
            screen_window_proc as unsafe extern "system" fn(_, _, _, _) -> _,
            HBRUSH::default(),
        ),
        (
            SETUP_CLASS,
            setup_window_proc as unsafe extern "system" fn(_, _, _, _) -> _,
            unsafe { GetSysColorBrush(COLOR_WINDOW) },
        ),
    ] {
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: hinstance,
            lpszClassName: name,
            hCursor: cursor,
            hbrBackground: background,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            let error = Error::from_thread();
            if error.code() != ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
                return Err(error);
            }
        }
    }
    Ok(())
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

fn brush(context: &ID2D1DeviceContext, value: [f64; 4]) -> Result<ID2D1SolidColorBrush> {
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
        .ok_or_else(|| Error::new(E_FAIL, "break screen dimension must be positive"))
}

fn setup_outer_size(dpi: u32) -> Result<(i32, i32)> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 430,
        bottom: 250,
    };
    unsafe {
        AdjustWindowRectEx(
            &mut rect,
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            false,
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        )
    }?;
    let scale = f64::from(dpi.max(96)) / 96.0;
    Ok((
        (f64::from(rect.right - rect.left) * scale).round() as i32,
        (f64::from(rect.bottom - rect.top) * scale).round() as i32,
    ))
}

fn wall_clock_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

fn current_local_time_string() -> String {
    let local = unsafe { GetLocalTime() };
    format!("{:02}:{:02}", local.wHour, local.wMinute)
}

fn local_time_string_at(timestamp: f64, now: f64) -> String {
    let local = unsafe { GetLocalTime() };
    let seconds = f64::from(local.wHour) * 3_600.0
        + f64::from(local.wMinute) * 60.0
        + f64::from(local.wSecond)
        + timestamp
        - now;
    let normalized = seconds.rem_euclid(86_400.0).floor() as u32;
    format!("{:02}:{:02}", normalized / 3_600, (normalized / 60) % 60)
}

fn set_picker_timestamp(picker: HWND, timestamp: f64, now: f64) {
    let mut local = unsafe { GetLocalTime() };
    let current_seconds = f64::from(local.wHour) * 3_600.0
        + f64::from(local.wMinute) * 60.0
        + f64::from(local.wSecond);
    let proposed = (current_seconds + timestamp - now).rem_euclid(86_400.0);
    local.wHour = (proposed / 3_600.0).floor() as u16;
    local.wMinute = ((proposed / 60.0).floor() as u16) % 60;
    local.wSecond = 0;
    unsafe {
        SendMessageW(
            picker,
            DTM_SETSYSTEMTIME,
            Some(WPARAM(GDT_VALID.0 as usize)),
            Some(LPARAM((&local as *const SYSTEMTIME) as isize)),
        )
    };
}

fn picker_system_time(picker: HWND) -> Result<SYSTEMTIME> {
    let mut value = SYSTEMTIME::default();
    let result = unsafe {
        SendMessageW(
            picker,
            DTM_GETSYSTEMTIME,
            None,
            Some(LPARAM((&mut value as *mut SYSTEMTIME) as isize)),
        )
    };
    if result.0 as u32 == GDT_VALID.0 {
        Ok(value)
    } else {
        Err(Error::new(E_FAIL, "DateTimePicker returned no valid time"))
    }
}

fn set_window_text(hwnd: HWND, value: &str) {
    let value = wide(value);
    let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(value.as_ptr())) };
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn lparam_point(lparam: LPARAM) -> (f32, f32) {
    (
        (lparam.0 as i32 as i16) as f32,
        ((lparam.0 >> 16) as i16) as f32,
    )
}
