//! Windows 도움말 오버레이와 OS 비의존 레이아웃/scope 선택.

const CARD_MINIMUM_WIDTH: f64 = 320.0;
const CARD_SPACING: f64 = 10.0;
const CONTENT_HORIZONTAL_INSET: f64 = 18.0;
const CONTENT_TOP_INSET: f64 = 18.0;
const CONTENT_BOTTOM_INSET: f64 = 18.0;
const HEADER_CARDS_SPACING: f64 = 14.0;
const VISIBLE_HORIZONTAL_MARGIN: f64 = 40.0;
const VISIBLE_VERTICAL_MARGIN: f64 = 30.0;
const TITLE_HEIGHT: f64 = 29.0;
const SEARCH_HEIGHT: f64 = 32.0;
const HEADER_SPACING: f64 = 8.0;
const CARD_HORIZONTAL_INSET: f64 = 10.0;
const CARD_VERTICAL_INSET: f64 = 9.0;
const CARD_TITLE_HEIGHT: f64 = 18.0;
const ITEM_HEIGHT: f64 = 30.0;
const ITEM_SPACING: f64 = 2.0;
const COLUMN_CANDIDATES: [usize; 3] = [4, 2, 1];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RectF {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl RectF {
    const fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    fn width(self) -> f64 {
        self.right - self.left
    }

    fn height(self) -> f64 {
        self.bottom - self.top
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn offset_y(self, delta: f64) -> Self {
        Self::new(self.left, self.top + delta, self.right, self.bottom + delta)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CardLayout {
    group_index: usize,
    rect: RectF,
    title_rect: RectF,
    item_rects: Vec<RectF>,
}

#[derive(Clone, Debug, PartialEq)]
struct HelpLayout {
    columns: usize,
    panel: RectF,
    title: RectF,
    search: Option<RectF>,
    cards_viewport: RectF,
    cards: Vec<CardLayout>,
    maximum_scroll: f64,
}

fn content_width(columns: usize) -> f64 {
    columns as f64 * CARD_MINIMUM_WIDTH
        + columns.saturating_sub(1) as f64 * CARD_SPACING
        + 2.0 * CONTENT_HORIZONTAL_INSET
}

fn card_height(item_count: usize) -> f64 {
    let rows = item_count.max(1);
    2.0 * CARD_VERTICAL_INSET
        + CARD_TITLE_HEIGHT
        + ITEM_SPACING
        + rows as f64 * ITEM_HEIGHT
        + rows.saturating_sub(1) as f64 * ITEM_SPACING
}

// Swift: HelpController.layoutMetrics(for:), apply(_:), makeCard(group:definitions:)
fn calculate_layout(work: RectF, group_item_counts: &[usize], pinned: bool) -> HelpLayout {
    let maximum_content_width = (work.width() - 2.0 * VISIBLE_HORIZONTAL_MARGIN).max(1.0);
    let columns = COLUMN_CANDIDATES
        .into_iter()
        .find(|columns| content_width(*columns) <= maximum_content_width)
        .unwrap_or(1);
    let panel_width = content_width(columns).min(maximum_content_width);

    let row_heights = group_item_counts
        .chunks(columns)
        .map(|row| row.iter().copied().map(card_height).fold(0.0, f64::max))
        .collect::<Vec<_>>();
    let cards_height =
        row_heights.iter().sum::<f64>() + row_heights.len().saturating_sub(1) as f64 * CARD_SPACING;
    let header_height = TITLE_HEIGHT
        + if pinned {
            HEADER_SPACING + SEARCH_HEIGHT
        } else {
            0.0
        };
    let preferred_height = CONTENT_TOP_INSET
        + header_height
        + HEADER_CARDS_SPACING
        + cards_height
        + CONTENT_BOTTOM_INSET;
    let maximum_height = (work.height() - 2.0 * VISIBLE_VERTICAL_MARGIN).max(1.0);
    let panel_height = preferred_height.min(maximum_height);
    let panel = RectF::new(
        work.left + (work.width() - panel_width) / 2.0,
        work.top + (work.height() - panel_height) / 2.0,
        work.left + (work.width() + panel_width) / 2.0,
        work.top + (work.height() + panel_height) / 2.0,
    );
    let title = RectF::new(
        panel.left + 22.0,
        panel.top + CONTENT_TOP_INSET,
        panel.right - 22.0,
        panel.top + CONTENT_TOP_INSET + TITLE_HEIGHT,
    );
    let search = pinned.then(|| {
        RectF::new(
            title.left,
            title.bottom + HEADER_SPACING,
            (title.left + 300.0).min(title.right),
            title.bottom + HEADER_SPACING + SEARCH_HEIGHT,
        )
    });
    let header_bottom = search.map_or(title.bottom, |rect| rect.bottom);
    let cards_viewport = RectF::new(
        panel.left + CONTENT_HORIZONTAL_INSET,
        header_bottom + HEADER_CARDS_SPACING,
        panel.right - CONTENT_HORIZONTAL_INSET,
        panel.bottom - CONTENT_BOTTOM_INSET,
    );

    let available_card_width =
        (cards_viewport.width() - columns.saturating_sub(1) as f64 * CARD_SPACING) / columns as f64;
    let mut cards = Vec::with_capacity(group_item_counts.len());
    let mut y = cards_viewport.top;
    for (row_index, row) in group_item_counts.chunks(columns).enumerate() {
        for (column, item_count) in row.iter().copied().enumerate() {
            let left = cards_viewport.left + column as f64 * (available_card_width + CARD_SPACING);
            let rect = RectF::new(
                left,
                y,
                left + available_card_width,
                y + card_height(item_count),
            );
            let title_rect = RectF::new(
                rect.left + CARD_HORIZONTAL_INSET,
                rect.top + CARD_VERTICAL_INSET,
                rect.right - CARD_HORIZONTAL_INSET,
                rect.top + CARD_VERTICAL_INSET + CARD_TITLE_HEIGHT,
            );
            let mut item_rects = Vec::with_capacity(item_count.max(1));
            let item_top = title_rect.bottom + ITEM_SPACING;
            for item_index in 0..item_count.max(1) {
                let top = item_top + item_index as f64 * (ITEM_HEIGHT + ITEM_SPACING);
                item_rects.push(RectF::new(
                    rect.left + CARD_HORIZONTAL_INSET,
                    top,
                    rect.right - CARD_HORIZONTAL_INSET,
                    top + ITEM_HEIGHT,
                ));
            }
            cards.push(CardLayout {
                group_index: row_index * columns + column,
                rect,
                title_rect,
                item_rects,
            });
        }
        y += row_heights[row_index] + CARD_SPACING;
    }
    let content_bottom = cards.last().map_or(cards_viewport.top, |card| {
        cards
            .iter()
            .map(|candidate| candidate.rect.bottom)
            .fold(card.rect.bottom, f64::max)
    });
    let maximum_scroll = (content_bottom - cards_viewport.bottom).max(0.0);

    HelpLayout {
        columns,
        panel,
        title,
        search,
        cards_viewport,
        cards,
        maximum_scroll,
    }
}

// Swift: HelpController.performDefinition(_:)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionScope {
    Global,
    Annotation,
    Zoom,
}

fn select_execution_scope(
    zoom_enabled: bool,
    annotation_enabled: bool,
    has_zoom_binding: bool,
    has_annotation_binding: bool,
    has_global_binding: bool,
) -> Option<ExecutionScope> {
    if zoom_enabled && has_zoom_binding {
        Some(ExecutionScope::Zoom)
    } else if annotation_enabled && has_annotation_binding {
        Some(ExecutionScope::Annotation)
    } else if has_global_binding {
        Some(ExecutionScope::Global)
    } else if has_annotation_binding {
        Some(ExecutionScope::Annotation)
    } else if has_zoom_binding {
        Some(ExecutionScope::Zoom)
    } else {
        None
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{calculate_layout, select_execution_scope, ExecutionScope, HelpLayout, RectF};
    use crate::app_icon::high_contrast;
    use crate::help_state::{HelpState, HelpTransition};
    use crate::kbhook::set_text_input_active;
    use crate::log::Logger;
    use crate::monitor::{enumerate_monitors, monitor_at_point};
    use crate::render::{
        create_swap_chain, create_swap_target, pixel_format, px_per_dip, read_bitmap_bgra,
        GraphicsDevice,
    };
    use crate::z_bands::{raise_band_order, Band, BandRegistration};
    use edutools_core::ink::{ArrowStyle, InkTool};
    use edutools_core::radial::RadialPaletteArcStyle;
    use edutools_core::settings::AppSettings;
    use edutools_core::shortcut::{
        AppMode, ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutGroup,
        ShortcutScope, ShortcutTool,
    };
    use edutools_core::state::AppState;
    use std::ffi::c_void;
    use std::fs;
    use std::path::PathBuf;
    use windows::core::{w, Error, Interface, Result, PCWSTR};
    use windows::Win32::Foundation::{
        COLORREF, ERROR_CLASS_ALREADY_EXISTS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT,
        WPARAM,
    };
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
    };
    use windows::Win32::Graphics::Direct2D::{
        ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Image, ID2D1RenderTarget,
        D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
        D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_PRIMITIVE_BLEND_COPY, D2D1_PRIMITIVE_BLEND_SOURCE_OVER,
        D2D1_ROUNDED_RECT,
    };
    use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
        DWRITE_WORD_WRAPPING_NO_WRAP,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
    use windows::Win32::Graphics::Dxgi::IDXGISwapChain1;
    use windows::Win32::Graphics::Gdi::{
        ClientToScreen, CreateFontIndirectW, CreateSolidBrush, DeleteObject, GetSysColor,
        GetSysColorBrush, InvalidateRect, SetBkColor, SetTextColor, CLEARTYPE_QUALITY,
        COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT,
        DEFAULT_CHARSET, HBRUSH, HDC, HFONT, LOGFONTW,
    };
    use windows::Win32::System::SystemInformation::GetTickCount64;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetFocus, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetForegroundWindow,
        GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsWindow, KillTimer, MoveWindow,
        PostMessageW, RegisterClassW, SendMessageW, SetForegroundWindow, SetTimer,
        SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, CREATESTRUCTW, EN_CHANGE,
        EN_KILLFOCUS, EN_SETFOCUS, ES_AUTOHSCROLL, GWLP_USERDATA, GWL_EXSTYLE, HTCLIENT,
        HTTRANSPARENT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, WINDOW_STYLE, WM_COMMAND, WM_CTLCOLOREDIT,
        WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_MOVE,
        WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_SETFONT, WM_SETTINGCHANGE, WM_SIZE,
        WM_SYSCOLORCHANGE, WM_THEMECHANGED, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_TABSTOP,
    };

    pub const WM_APP_HELP_ACTION: u32 = 0x8000 + 60;
    pub const WM_APP_HELP_REFRESH: u32 = 0x8000 + 61;
    pub const WM_APP_HELP_CLOSE: u32 = 0x8000 + 62;
    pub const HELP_RELEASE_TIMER_ID: usize = 0xE117;
    const HELP_RELEASE_POLL_MS: u32 = 30;
    const HELP_RELEASE_MAX_MS: u64 = 10_000;
    const HELP_CLASS: PCWSTR = w!("EduToolsHelp");
    const EM_SETSEL_MESSAGE: u32 = 177;
    const EM_SETMARGINS_MESSAGE: u32 = 0x00d3;
    const EM_SETCUEBANNER_MESSAGE: u32 = 0x1501;
    const EC_LEFTMARGIN: usize = 0x0001;
    const EC_RIGHTMARGIN: usize = 0x0002;
    const WM_MOUSELEAVE_MESSAGE: u32 = 675;
    const SEARCH_FONT_SIZE_DIP: u32 = 14;
    const SEARCH_HORIZONTAL_MARGIN_DIP: u32 = 8;
    const SEARCH_BACKGROUND: COLORREF = COLORREF(0x0038_2c28);
    const SEARCH_TEXT: COLORREF = COLORREF(0x00ff_ffff);

    #[derive(Clone, Debug, Default)]
    struct HelpContent {
        title: String,
        groups: Vec<HelpGroup>,
        opacity: f64,
    }

    #[derive(Clone, Debug)]
    struct HelpGroup {
        name: &'static str,
        items: Vec<HelpItem>,
    }

    #[derive(Clone, Debug)]
    struct HelpItem {
        definition: ShortcutDefinition,
        name: String,
        keys: String,
        current: bool,
    }

    struct HelpWindowState {
        owner: HWND,
        logger: Logger,
        dpi: u32,
        work: RectF,
        layout: HelpLayout,
        content: HelpContent,
        hover_action: Option<usize>,
        scroll: f64,
        pinned: bool,
        search: HWND,
        search_font: HFONT,
        search_background: HBRUSH,
        renderer: Option<HelpRenderer>,
    }

    impl Drop for HelpWindowState {
        fn drop(&mut self) {
            if !self.search_font.is_invalid() {
                let _ = unsafe { DeleteObject(self.search_font.into()) };
                self.search_font = HFONT::default();
            }
            if !self.search_background.is_invalid() {
                let _ = unsafe { DeleteObject(self.search_background.into()) };
                self.search_background = HBRUSH::default();
            }
        }
    }

    struct HelpWindow {
        hwnd: HWND,
        state: Box<HelpWindowState>,
        _band: BandRegistration,
    }

    impl Drop for HelpWindow {
        fn drop(&mut self) {
            set_text_input_active(false);
            let _ = unsafe { DestroyWindow(self.state.search) };
            let _ = unsafe { DestroyWindow(self.hwnd) };
        }
    }

    struct GlobalReleasePoll {
        vk: u32,
        started_ms: u64,
    }

    pub struct HelpManager {
        window: HelpWindow,
        _graphics: GraphicsDevice,
        state: HelpState,
        previous_foreground: Option<HWND>,
        activated: bool,
        global_poll: Option<GlobalReleasePoll>,
        first_run_path: Option<PathBuf>,
        first_run_checked: bool,
        logger: Logger,
    }

    impl HelpManager {
        pub fn new(owner: HWND, hinstance: HINSTANCE, logger: Logger) -> Result<Self> {
            register_window_class(hinstance)?;
            let empty_layout =
                calculate_layout(RectF::new(0.0, 0.0, 1.0, 1.0), &[0, 0, 0, 0], false);
            let mut window_state = Box::new(HelpWindowState {
                owner,
                logger: logger.clone(),
                dpi: 96,
                work: RectF::new(0.0, 0.0, 1.0, 1.0),
                layout: empty_layout,
                content: HelpContent::default(),
                hover_action: None,
                scroll: 0.0,
                pinned: false,
                search: HWND::default(),
                search_font: HFONT::default(),
                search_background: HBRUSH::default(),
                renderer: None,
            });
            let state_ptr = window_state.as_mut() as *mut HelpWindowState;
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP,
                    HELP_CLASS,
                    w!("EduTools Help"),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    Some(owner),
                    None,
                    Some(hinstance),
                    Some(state_ptr.cast::<c_void>()),
                )
            }?;
            let search = match unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    w!("EDIT"),
                    w!(""),
                    WS_POPUP | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    0,
                    0,
                    1,
                    1,
                    Some(hwnd),
                    None,
                    Some(hinstance),
                    None,
                )
            } {
                Ok(search) => search,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            window_state.search = search;
            window_state.search_background = unsafe { CreateSolidBrush(SEARCH_BACKGROUND) };
            if window_state.search_background.is_invalid() {
                let error = Error::from_thread();
                let _ = unsafe { DestroyWindow(search) };
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
            if let Err(error) = update_search_font(&mut window_state) {
                let _ = unsafe { DestroyWindow(search) };
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
            unsafe {
                SendMessageW(
                    search,
                    EM_SETCUEBANNER_MESSAGE,
                    Some(WPARAM(0)),
                    Some(LPARAM(w!("이름 또는 키로 찾기").as_ptr() as isize)),
                )
            };

            let band = BandRegistration::new(hwnd, Band::Guidance);
            let graphics = match GraphicsDevice::new() {
                Ok(graphics) => graphics,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(search) };
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            let renderer = match HelpRenderer::new(&graphics, hwnd, 1, 1, 96) {
                Ok(renderer) => renderer,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(search) };
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            window_state.renderer = Some(renderer);
            Ok(Self {
                window: HelpWindow {
                    hwnd,
                    state: window_state,
                    _band: band,
                },
                _graphics: graphics,
                state: HelpState::default(),
                previous_foreground: None,
                activated: false,
                global_poll: None,
                first_run_path: std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .map(|path| path.join("EduTools").join("help-first-run-v1")),
                first_run_checked: false,
                logger,
            })
        }

        pub fn is_visible(&self) -> bool {
            self.state.is_visible()
        }

        pub fn is_pinned(&self) -> bool {
            self.state.is_pinned()
        }

        pub fn owns_hwnd(&self, hwnd: HWND) -> bool {
            hwnd == self.window.hwnd || hwnd == self.window.state.search
        }

        pub fn show_first_run_if_needed(&mut self, app_state: &AppState, settings: &AppSettings) {
            if self.first_run_checked {
                return;
            }
            self.first_run_checked = true;
            if let Some(path) = self.first_run_path.as_ref() {
                match fs::metadata(path) {
                    Ok(_) => return,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => self.logger.event(&format!(
                        "도움말 첫 실행 표식 확인 실패; 표시를 계속함: {}: {error}",
                        path.display()
                    )),
                }
            }
            if let Some(path) = self.first_run_path.as_ref() {
                let result = path
                    .parent()
                    .ok_or_else(|| "첫 실행 표식 경로에 부모가 없음".to_owned())
                    .and_then(|parent| {
                        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                        fs::write(path, b"shown\n").map_err(|error| error.to_string())
                    });
                if let Err(error) = result {
                    self.logger.event(&format!(
                        "도움말 첫 실행 표식 저장 실패 (이번 프로세스에서는 1회만 표시): {error}"
                    ));
                }
            } else {
                self.logger.event(
                    "도움말 첫 실행 표식 저장 생략: APPDATA 없음 (이번 프로세스에서는 1회만 표시)",
                );
            }
            self.show_pinned(app_state, settings, "첫 실행");
        }

        pub fn show_pinned(&mut self, app_state: &AppState, settings: &AppSettings, source: &str) {
            if !self.state.is_visible() {
                self.capture_previous_foreground(app_state);
            }
            let transition = self.state.toggle_pinned();
            self.apply_transition(transition, app_state, settings, source);
        }

        pub fn shortcut_did_press(
            &mut self,
            now: f64,
            app_state: &AppState,
            settings: &AppSettings,
            source: &str,
        ) {
            if !self.state.is_visible() {
                self.capture_previous_foreground(app_state);
            }
            let transition = self.state.shortcut_did_press(now);
            self.apply_transition(transition, app_state, settings, source);
        }

        pub fn shortcut_did_release(
            &mut self,
            now: f64,
            app_state: &AppState,
            settings: &AppSettings,
            source: &str,
        ) {
            let transition = self.state.shortcut_did_release(now);
            self.apply_transition(transition, app_state, settings, source);
        }

        pub fn start_global_release_poll(&mut self, vk: u32, now_ms: u64) -> bool {
            self.global_poll = Some(GlobalReleasePoll {
                vk,
                started_ms: now_ms,
            });
            if unsafe {
                SetTimer(
                    Some(self.window.state.owner),
                    HELP_RELEASE_TIMER_ID,
                    HELP_RELEASE_POLL_MS,
                    None,
                )
            } == 0
            {
                self.logger
                    .event("도움말 전역 키 해제 폴링 타이머 시작 실패; 즉시 강제 해제");
                self.global_poll = None;
                return false;
            }
            true
        }

        pub fn poll_global_release(
            &mut self,
            now_ms: u64,
            app_state: &AppState,
            settings: &AppSettings,
        ) -> bool {
            let Some(poll) = self.global_poll.as_ref() else {
                return false;
            };
            let down = unsafe { GetAsyncKeyState(poll.vk as i32) as u16 & 0x8000 != 0 };
            let forced = now_ms.saturating_sub(poll.started_ms) >= HELP_RELEASE_MAX_MS;
            if down && !forced {
                return true;
            }
            let vk = poll.vk;
            self.global_poll = None;
            let _ = unsafe { KillTimer(Some(self.window.state.owner), HELP_RELEASE_TIMER_ID) };
            self.logger.event(&format!(
                "도움말 전역 키 폴링 해제: vk={vk:#x}{}",
                if forced {
                    " (상한 시간 강제 해제)"
                } else {
                    ""
                }
            ));
            self.shortcut_did_release(
                now_ms as f64 / 1_000.0,
                app_state,
                settings,
                if forced {
                    "전역 키 강제 해제"
                } else {
                    "전역 키 해제"
                },
            );
            true
        }

        pub fn close(&mut self, app_state: &AppState, reason: &str) {
            let transition = self.state.close();
            if transition == HelpTransition::Close {
                self.hide_window(app_state, reason);
            }
        }

        pub fn refresh(&mut self, app_state: &AppState, settings: &AppSettings, reason: &str) {
            if !self.state.is_visible() {
                return;
            }
            if let Err(error) = self.refresh_content(app_state, settings) {
                self.logger
                    .event(&format!("도움말 내용 갱신 실패 ({reason}): {error}"));
            }
        }

        pub fn display_configuration_changed(
            &mut self,
            app_state: &AppState,
            settings: &AppSettings,
        ) {
            if !self.state.is_visible() {
                return;
            }
            if let Err(error) = self.move_to_mouse_monitor() {
                self.logger
                    .event(&format!("도움말 모니터 변경 반영 실패: {error}"));
                return;
            }
            self.refresh(app_state, settings, "모니터 변경");
        }

        pub fn action(
            &self,
            value: usize,
            app_state: &AppState,
        ) -> Option<(ShortcutDefinition, ShortcutScope)> {
            let item = self
                .window
                .state
                .content
                .groups
                .iter()
                .flat_map(|group| group.items.iter())
                .nth(value.checked_sub(1)?)?;
            let definition = item.definition.clone();
            let scope = match select_execution_scope(
                app_state.zoom_session().is_some(),
                app_state.is_annotation_mode_enabled(),
                definition.binding(ShortcutScope::Zoom).is_some(),
                definition.binding(ShortcutScope::Annotation).is_some(),
                definition.binding(ShortcutScope::Global).is_some(),
            )? {
                ExecutionScope::Global => ShortcutScope::Global,
                ExecutionScope::Annotation => ShortcutScope::Annotation,
                ExecutionScope::Zoom => ShortcutScope::Zoom,
            };
            Some((definition, scope))
        }

        pub fn selftest_failures(
            &mut self,
            app_state: &AppState,
            settings: &AppSettings,
        ) -> Vec<String> {
            let mut failures = Vec::new();
            if let Err(error) = self.move_to_mouse_monitor() {
                failures.push(format!("help: 모니터 배치 실패: {error}"));
                return failures;
            }
            self.window.state.pinned = true;
            if let Err(error) = self.refresh_content(app_state, settings) {
                failures.push(format!("help: selftest 내용 생성 실패: {error}"));
                return failures;
            }
            let state = &mut self.window.state;
            match state
                .renderer
                .as_mut()
                .expect("help renderer exists")
                .selftest_pixel_failures(&state.layout, &state.content)
            {
                Ok(pixel_failures) => failures.extend(pixel_failures),
                Err(error) => failures.push(format!("help: 픽셀 readback 실패: {error}")),
            }
            failures
        }

        fn apply_transition(
            &mut self,
            transition: HelpTransition,
            app_state: &AppState,
            settings: &AppSettings,
            source: &str,
        ) {
            match transition {
                HelpTransition::ShowHeld => {
                    if let Err(error) = self.show_window(app_state, settings, false) {
                        self.logger
                            .event(&format!("도움말 held 표시 실패 ({source}): {error}"));
                        let _ = self.state.close();
                    }
                }
                HelpTransition::ShowPinned => {
                    if let Err(error) = self.show_window(app_state, settings, true) {
                        self.logger
                            .event(&format!("도움말 pinned 표시 실패 ({source}): {error}"));
                        let _ = self.state.close();
                    }
                }
                HelpTransition::Close => self.hide_window(app_state, source),
                HelpTransition::None | HelpTransition::IgnoreRelease => {}
            }
        }

        fn show_window(
            &mut self,
            app_state: &AppState,
            settings: &AppSettings,
            pinned: bool,
        ) -> Result<()> {
            self.window.state.pinned = pinned;
            self.window.state.scroll = 0.0;
            self.window.state.hover_action = None;
            self.move_to_mouse_monitor()?;
            self.refresh_content(app_state, settings)?;
            set_activation_style(self.window.hwnd, pinned)?;
            if pinned {
                let _ = unsafe { ShowWindow(self.window.hwnd, SW_SHOW) };
                let _ = unsafe { ShowWindow(self.window.state.search, SW_SHOW) };
                if !unsafe { SetForegroundWindow(self.window.hwnd) }.as_bool() {
                    self.logger.event("도움말 pinned 창 전경 전환 실패");
                }
                let _ = unsafe { SetFocus(Some(self.window.state.search)) };
                if unsafe { GetFocus() } != self.window.state.search {
                    self.logger.event("도움말 검색창 포커스 설정 실패");
                }
                unsafe {
                    SendMessageW(
                        self.window.state.search,
                        EM_SETSEL_MESSAGE,
                        Some(WPARAM(0)),
                        Some(LPARAM(-1)),
                    )
                };
                set_text_input_active(true);
                self.activated = true;
            } else {
                set_text_input_active(false);
                let _ = unsafe { ShowWindow(self.window.hwnd, SW_SHOWNOACTIVATE) };
            }
            raise_band_order()?;
            self.logger.event(&format!(
                "도움말 표시: mode={} monitor={} columns={}",
                if pinned { "pinned" } else { "held" },
                self.target_monitor_key().as_deref().unwrap_or("unknown"),
                self.window.state.layout.columns
            ));
            Ok(())
        }

        fn hide_window(&mut self, app_state: &AppState, reason: &str) {
            let _ = unsafe { ShowWindow(self.window.state.search, SW_HIDE) };
            let _ = unsafe { ShowWindow(self.window.hwnd, SW_HIDE) };
            let _ = unsafe { SetWindowTextW(self.window.state.search, w!("")) };
            set_text_input_active(false);
            self.window.state.scroll = 0.0;
            self.window.state.hover_action = None;
            self.logger.event(&format!("도움말 닫기: {reason}"));
            if self.activated
                && !app_state.is_annotation_mode_enabled()
                && app_state.zoom_session().is_none()
            {
                if let Some(previous) = self.previous_foreground.take() {
                    if unsafe { IsWindow(Some(previous)) }.as_bool()
                        && !unsafe { SetForegroundWindow(previous) }.as_bool()
                    {
                        self.logger.event("도움말 닫기 후 포커스 복귀 실패");
                    }
                }
            }
            self.previous_foreground = None;
            self.activated = false;
        }

        fn capture_previous_foreground(&mut self, app_state: &AppState) {
            self.previous_foreground = None;
            self.activated = false;
            if app_state.is_annotation_mode_enabled() || app_state.zoom_session().is_some() {
                return;
            }
            let foreground = unsafe { GetForegroundWindow() };
            if foreground != self.window.hwnd && foreground != self.window.state.owner {
                self.previous_foreground = Some(foreground);
            }
        }

        fn move_to_mouse_monitor(&mut self) -> Result<()> {
            let mut cursor = POINT::default();
            unsafe { GetCursorPos(&mut cursor) }?;
            let monitors = enumerate_monitors()?;
            let monitor = monitor_at_point(&monitors, cursor)
                .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "help requires at least one monitor"))?;
            let width = monitor.rect.width();
            let height = monitor.rect.height();
            let dpi = monitor.dpi.0.max(96);
            unsafe {
                SetWindowPos(
                    self.window.hwnd,
                    None,
                    monitor.rect.left,
                    monitor.rect.top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            }?;
            self.window
                .state
                .renderer
                .as_mut()
                .expect("help renderer exists")
                .resize(width, height, dpi)?;
            let scale = px_per_dip(dpi);
            self.window.state.dpi = dpi;
            update_search_font(&mut self.window.state)?;
            self.window.state.work = RectF::new(
                f64::from(monitor.work_rect.left - monitor.rect.left) / scale,
                f64::from(monitor.work_rect.top - monitor.rect.top) / scale,
                f64::from(monitor.work_rect.right - monitor.rect.left) / scale,
                f64::from(monitor.work_rect.bottom - monitor.rect.top) / scale,
            );
            Ok(())
        }

        fn target_monitor_key(&self) -> Option<String> {
            let monitors = enumerate_monitors().ok()?;
            let mut cursor = POINT::default();
            unsafe { GetCursorPos(&mut cursor) }.ok()?;
            monitor_at_point(&monitors, cursor).map(|monitor| monitor.session_key.clone())
        }

        fn refresh_content(&mut self, app_state: &AppState, settings: &AppSettings) -> Result<()> {
            let query = if self.window.state.pinned {
                search_text(self.window.state.search)
            } else {
                String::new()
            };
            self.window.state.content =
                build_content(app_state, settings, &query, self.window.state.pinned);
            self.window.state.hover_action = None;
            let counts = self
                .window
                .state
                .content
                .groups
                .iter()
                .map(|group| group.items.len())
                .collect::<Vec<_>>();
            self.window.state.layout =
                calculate_layout(self.window.state.work, &counts, self.window.state.pinned);
            self.window.state.scroll = self
                .window
                .state
                .scroll
                .min(self.window.state.layout.maximum_scroll);
            position_search(self.window.hwnd, &self.window.state)?;
            render_window_state(&mut self.window.state)?;
            self.logger.event(&format!(
                "도움말 검색: queryLength={}",
                query.chars().count()
            ));
            Ok(())
        }
    }

    impl Drop for HelpManager {
        fn drop(&mut self) {
            let _ = unsafe { KillTimer(Some(self.window.state.owner), HELP_RELEASE_TIMER_ID) };
            set_text_input_active(false);
        }
    }

    pub fn monotonic_seconds() -> f64 {
        monotonic_millis() as f64 / 1_000.0
    }

    pub fn monotonic_millis() -> u64 {
        unsafe { GetTickCount64() }
    }

    fn build_content(
        state: &AppState,
        settings: &AppSettings,
        query: &str,
        pinned: bool,
    ) -> HelpContent {
        let (title, active_mode, groups) = if state.zoom_session().is_some() {
            (
                "줌 모드 단축키",
                AppMode::Zoom,
                [
                    ShortcutGroup::ScreenBoardZoom,
                    ShortcutGroup::Tools,
                    ShortcutGroup::Editing,
                    ShortcutGroup::TimerAndOther,
                ],
            )
        } else if state.is_annotation_mode_enabled() {
            (
                "판서 모드 단축키",
                AppMode::Annotation,
                [
                    ShortcutGroup::Tools,
                    ShortcutGroup::Editing,
                    ShortcutGroup::ScreenBoardZoom,
                    ShortcutGroup::TimerAndOther,
                ],
            )
        } else {
            (
                "전역 단축키",
                AppMode::Global,
                [
                    ShortcutGroup::ScreenBoardZoom,
                    ShortcutGroup::TimerAndOther,
                    ShortcutGroup::Tools,
                    ShortcutGroup::Editing,
                ],
            )
        };
        let definitions = ShortcutCatalog::resolved_definitions(&settings.shortcuts);
        let mut definitions =
            ShortcutCatalog::filtered(&definitions, if pinned { query } else { "" });
        definitions.sort_by_key(|definition| !definition.modes.contains(&active_mode));
        let groups = groups
            .into_iter()
            .map(|group| HelpGroup {
                name: group_name(group),
                items: definitions
                    .iter()
                    .filter(|definition| definition.group == group)
                    .map(|definition| HelpItem {
                        definition: definition.clone(),
                        name: contextual_name(definition, state, settings),
                        keys: definition.display_keys(),
                        current: is_current(definition, state, settings),
                    })
                    .collect(),
            })
            .collect();
        HelpContent {
            title: title.to_owned(),
            groups,
            opacity: settings.tools.palette_glass_opacity,
        }
    }

    fn group_name(group: ShortcutGroup) -> &'static str {
        match group {
            ShortcutGroup::Tools => "도구",
            ShortcutGroup::Editing => "편집",
            ShortcutGroup::ScreenBoardZoom => "화면·보드·줌",
            ShortcutGroup::TimerAndOther => "타이머·기타",
        }
    }

    fn contextual_name(
        definition: &ShortcutDefinition,
        state: &AppState,
        settings: &AppSettings,
    ) -> String {
        match definition.command {
            ShortcutCommand::SelectOrCycleEraser => format!(
                "{} · {}",
                definition.name,
                match settings.eraser.mode.as_str() {
                    "partial" => "부분",
                    "stroke" => "획",
                    "area" => "영역",
                    _ => "?",
                }
            ),
            ShortcutCommand::ToggleEraserAreaShape => format!(
                "{} · {}",
                definition.name,
                if settings.eraser.area_shape == "lasso" {
                    "올가미"
                } else {
                    "사각형"
                }
            ),
            ShortcutCommand::ToggleShapeRecognition => format!(
                "{} · {}",
                definition.name,
                if settings.tools.shape_recognition_enabled {
                    "켬"
                } else {
                    "끔"
                }
            ),
            ShortcutCommand::SelectTool(ShortcutTool::Arrow) => format!(
                "화살표 · {}",
                match state.arrow_style() {
                    ArrowStyle::Single => "한쪽",
                    ArrowStyle::Double => "양쪽",
                }
            ),
            _ => definition.name.clone(),
        }
    }

    fn is_current(
        definition: &ShortcutDefinition,
        state: &AppState,
        settings: &AppSettings,
    ) -> bool {
        match definition.command {
            ShortcutCommand::SelectTool(tool) => shortcut_tool(tool) == state.current_tool(),
            ShortcutCommand::UseLaser => state.current_tool() == InkTool::Laser,
            ShortcutCommand::SelectOrCycleEraser => state.current_tool() == InkTool::Eraser,
            ShortcutCommand::ToggleShapeRecognition => settings.tools.shape_recognition_enabled,
            ShortcutCommand::ToggleEraserAreaShape => {
                state.current_tool() == InkTool::Eraser && settings.eraser.mode == "area"
            }
            _ => false,
        }
    }

    fn shortcut_tool(tool: ShortcutTool) -> InkTool {
        match tool {
            ShortcutTool::Pen => InkTool::Pen,
            ShortcutTool::Highlighter => InkTool::Highlighter,
            ShortcutTool::Arrow => InkTool::Arrow,
            ShortcutTool::Rectangle => InkTool::Rectangle,
            ShortcutTool::Ellipse => InkTool::Ellipse,
            ShortcutTool::Text => InkTool::Text,
            ShortcutTool::Select => InkTool::Select,
            ShortcutTool::Cover => InkTool::Cover,
        }
    }

    fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
        let class = WNDCLASSW {
            lpfnWndProc: Some(help_window_proc),
            hInstance: hinstance,
            lpszClassName: HELP_CLASS,
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

    fn set_activation_style(hwnd: HWND, pinned: bool) -> Result<()> {
        let current = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        let held_bits = (WS_EX_NOACTIVATE | WS_EX_TRANSPARENT).0 as isize;
        let updated = if pinned {
            current & !held_bits
        } else {
            current | held_bits
        };
        if current != updated {
            unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated) };
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                )
            }?;
        }
        Ok(())
    }

    unsafe extern "system" fn help_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            let state = create.lpCreateParams as *mut HelpWindowState;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
        }
        let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpWindowState };
        if !state.is_null() {
            let state = unsafe { &mut *state };
            match message {
                WM_NCHITTEST => {
                    return LRESULT(if state.pinned {
                        HTCLIENT as isize
                    } else {
                        HTTRANSPARENT as isize
                    });
                }
                WM_LBUTTONDOWN => {
                    let (x, y) = client_point(lparam, state.dpi);
                    if !state.layout.panel.contains(x, y) {
                        post_owner(state, WM_APP_HELP_CLOSE, 1);
                    } else if let Some(action) = action_at(state, x, y) {
                        post_owner(state, WM_APP_HELP_ACTION, action);
                    }
                    return LRESULT(0);
                }
                WM_MOUSEMOVE => {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        ..Default::default()
                    };
                    let _ = unsafe { TrackMouseEvent(&mut track) };
                    let (x, y) = client_point(lparam, state.dpi);
                    let hover = action_at(state, x, y);
                    if state.hover_action != hover {
                        state.hover_action = hover;
                        if let Err(error) = render_window_state(state) {
                            state
                                .logger
                                .event(&format!("도움말 hover 다시 그리기 실패: {error}"));
                        }
                    }
                    return LRESULT(0);
                }
                WM_MOUSELEAVE_MESSAGE => {
                    if state.hover_action.take().is_some() {
                        if let Err(error) = render_window_state(state) {
                            state
                                .logger
                                .event(&format!("도움말 hover 해제 다시 그리기 실패: {error}"));
                        }
                    }
                    return LRESULT(0);
                }
                WM_MOUSEWHEEL => {
                    let delta = ((wparam.0 >> 16) as u16) as i16;
                    let previous = state.scroll;
                    state.scroll = (state.scroll - f64::from(delta) / 120.0 * 48.0)
                        .clamp(0.0, state.layout.maximum_scroll);
                    if state.scroll != previous {
                        state.hover_action = None;
                        if let Err(error) = position_search(hwnd, state) {
                            state
                                .logger
                                .event(&format!("도움말 검색창 스크롤 배치 실패: {error}"));
                        }
                        if let Err(error) = render_window_state(state) {
                            state
                                .logger
                                .event(&format!("도움말 스크롤 다시 그리기 실패: {error}"));
                        }
                    }
                    return LRESULT(0);
                }
                WM_COMMAND => {
                    if HWND(lparam.0 as *mut c_void) == state.search {
                        let notification = ((wparam.0 >> 16) & 0xffff) as u32;
                        if notification == EN_CHANGE {
                            post_owner(state, WM_APP_HELP_REFRESH, 0);
                        } else if notification == EN_SETFOCUS {
                            set_text_input_active(true);
                        } else if notification == EN_KILLFOCUS {
                            set_text_input_active(false);
                        }
                    }
                    return LRESULT(0);
                }
                WM_CTLCOLOREDIT => {
                    if HWND(lparam.0 as *mut c_void) == state.search {
                        let hdc = HDC(wparam.0 as *mut c_void);
                        let (text, background, brush) = if high_contrast().unwrap_or(false) {
                            (
                                COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) }),
                                COLORREF(unsafe { GetSysColor(COLOR_WINDOW) }),
                                unsafe { GetSysColorBrush(COLOR_WINDOW) },
                            )
                        } else {
                            (SEARCH_TEXT, SEARCH_BACKGROUND, state.search_background)
                        };
                        unsafe {
                            SetTextColor(hdc, text);
                            SetBkColor(hdc, background);
                        }
                        return LRESULT(brush.0 as isize);
                    }
                }
                WM_DPICHANGED => {
                    state.dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
                    if let Err(error) = update_search_font(state) {
                        state
                            .logger
                            .event(&format!("도움말 검색창 DPI 글꼴 갱신 실패: {error}"));
                    }
                    if let Err(error) = position_search(hwnd, state) {
                        state
                            .logger
                            .event(&format!("도움말 검색창 DPI 배치 실패: {error}"));
                    }
                    post_owner(state, WM_APP_HELP_REFRESH, 0);
                    return LRESULT(0);
                }
                WM_MOVE | WM_SIZE => {
                    if let Err(error) = position_search(hwnd, state) {
                        state
                            .logger
                            .event(&format!("도움말 검색창 창 배치 실패: {error}"));
                    }
                    return LRESULT(0);
                }
                WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
                    let _ = unsafe { InvalidateRect(Some(state.search), None, true) };
                    post_owner(state, WM_APP_HELP_REFRESH, 0);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => return LRESULT(1),
                WM_NCDESTROY => {
                    set_text_input_active(false);
                    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
                }
                _ => {}
            }
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn update_search_font(state: &mut HelpWindowState) -> Result<()> {
        let pixel_height = dip_to_px(SEARCH_FONT_SIZE_DIP, state.dpi).max(1);
        let mut logical_font = LOGFONTW {
            lfHeight: -(pixel_height as i32),
            lfWeight: 400,
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
            return Err(Error::from_thread());
        }

        unsafe {
            SendMessageW(
                state.search,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            )
        };
        let old_font = std::mem::replace(&mut state.search_font, font);
        if !old_font.is_invalid() {
            let _ = unsafe { DeleteObject(old_font.into()) };
        }

        let margin = dip_to_px(SEARCH_HORIZONTAL_MARGIN_DIP, state.dpi).min(u32::from(u16::MAX));
        let packed_margins = usize::from(margin as u16) | (usize::from(margin as u16) << 16);
        unsafe {
            SendMessageW(
                state.search,
                EM_SETMARGINS_MESSAGE,
                Some(WPARAM(EC_LEFTMARGIN | EC_RIGHTMARGIN)),
                Some(LPARAM(packed_margins as isize)),
            )
        };
        Ok(())
    }

    fn dip_to_px(value: u32, dpi: u32) -> u32 {
        value.saturating_mul(dpi).saturating_add(48) / 96
    }

    fn client_point(lparam: LPARAM, dpi: u32) -> (f64, f64) {
        let x = (lparam.0 as u16) as i16;
        let y = ((lparam.0 >> 16) as u16) as i16;
        let scale = px_per_dip(dpi);
        (f64::from(x) / scale, f64::from(y) / scale)
    }

    fn action_at(state: &HelpWindowState, x: f64, y: f64) -> Option<usize> {
        if !state.layout.cards_viewport.contains(x, y) {
            return None;
        }
        let mut action = 1usize;
        for card in &state.layout.cards {
            let group = state.content.groups.get(card.group_index)?;
            for (item_index, _) in group.items.iter().enumerate() {
                let rect = card.item_rects.get(item_index)?.offset_y(-state.scroll);
                if rect.contains(x, y) {
                    return Some(action);
                }
                action += 1;
            }
        }
        None
    }

    fn position_search(hwnd: HWND, state: &HelpWindowState) -> Result<()> {
        if state.search == HWND::default() {
            return Ok(());
        }
        let Some(rect) = state.layout.search else {
            let _ = unsafe { ShowWindow(state.search, SW_HIDE) };
            return Ok(());
        };
        let scale = px_per_dip(state.dpi);
        let mut origin = POINT {
            x: (rect.left * scale).round() as i32,
            y: (rect.top * scale).round() as i32,
        };
        if !unsafe { ClientToScreen(hwnd, &mut origin) }.as_bool() {
            return Err(Error::from_thread());
        }
        let width = (rect.width() * scale).round().max(1.0) as i32;
        let height = (rect.height() * scale).round().max(1.0) as i32;
        unsafe { MoveWindow(state.search, origin.x, origin.y, width, height, true) }?;
        Ok(())
    }

    fn search_text(hwnd: HWND) -> String {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return String::new();
        }
        let mut text = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, &mut text) };
        String::from_utf16_lossy(&text[..copied.max(0) as usize])
    }

    fn post_owner(state: &HelpWindowState, message: u32, value: usize) {
        if let Err(error) =
            unsafe { PostMessageW(Some(state.owner), message, WPARAM(value), LPARAM(0)) }
        {
            state.logger.event(&format!(
                "도움말 owner 메시지 전송 실패: {message:#x}: {error}"
            ));
        }
    }

    fn render_window_state(state: &mut HelpWindowState) -> Result<()> {
        state
            .renderer
            .as_mut()
            .expect("help renderer exists")
            .render(
                &state.layout,
                &state.content,
                state.hover_action,
                state.scroll,
            )
    }

    struct HelpRenderer {
        swap_chain: IDXGISwapChain1,
        context: ID2D1DeviceContext,
        swap_target: Option<ID2D1Bitmap1>,
        dwrite: IDWriteFactory,
        formats: TextFormats,
        _dcomp_target: IDCompositionTarget,
        _visual: IDCompositionVisual,
        width: u32,
        height: u32,
        dpi: u32,
    }

    struct TextFormats {
        title: IDWriteTextFormat,
        group: IDWriteTextFormat,
        item: IDWriteTextFormat,
        key: IDWriteTextFormat,
        empty: IDWriteTextFormat,
    }

    #[derive(Clone, Copy)]
    struct HelpTheme {
        surface: [f64; 4],
        border: [f64; 4],
        card: [f64; 4],
        text: [f64; 4],
        secondary: [f64; 4],
        hover: [f64; 4],
        selected: [f64; 4],
        selected_text: [f64; 4],
    }

    impl HelpRenderer {
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
                swap_target.as_ref().expect("help swap target exists"),
            )?;
            let (dcomp_target, visual) =
                graphics.attach_composition_swap_chain(hwnd, &swap_chain)?;
            let dwrite: IDWriteFactory =
                unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
            let formats = TextFormats::new(&dwrite, dpi)?;
            Ok(Self {
                swap_chain,
                context,
                swap_target,
                dwrite,
                formats,
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
                    .expect("help swap target recreated"),
            )?;
            if self.dpi != dpi {
                self.formats = TextFormats::new(&self.dwrite, dpi)?;
            }
            self.width = width;
            self.height = height;
            self.dpi = dpi;
            Ok(())
        }

        fn render(
            &mut self,
            layout: &HelpLayout,
            content: &HelpContent,
            hover_action: Option<usize>,
            scroll: f64,
        ) -> Result<()> {
            let target = self
                .swap_target
                .as_ref()
                .expect("help swap target exists")
                .clone();
            self.render_to(&target, layout, content, hover_action, scroll)?;
            unsafe { self.swap_chain.Present(1, Default::default()).ok() }
        }

        fn selftest_pixel_failures(
            &mut self,
            layout: &HelpLayout,
            content: &HelpContent,
        ) -> Result<Vec<String>> {
            let bitmap = create_target_bitmap(&self.context, self.width, self.height)?;
            self.render_to(&bitmap, layout, content, None, 0.0)?;
            let (pixels, pitch) = read_bitmap_bgra(&self.context, &bitmap)?;
            let pixel = |x: u32, y: u32| -> [u8; 4] {
                let offset = y as usize * pitch as usize + x as usize * 4;
                [
                    pixels[offset],
                    pixels[offset + 1],
                    pixels[offset + 2],
                    pixels[offset + 3],
                ]
            };
            let scale = px_per_dip(self.dpi);
            let mut failures = Vec::new();
            let panel_x = ((layout.panel.left + layout.panel.right) / 2.0 * scale)
                .round()
                .clamp(0.0, f64::from(self.width - 1)) as u32;
            let panel_y = ((layout.panel.top + layout.panel.bottom) / 2.0 * scale)
                .round()
                .clamp(0.0, f64::from(self.height - 1)) as u32;
            if pixel(panel_x, panel_y)[3] == 0 {
                failures.push("help: 패널 영역 alpha가 0".to_owned());
            }
            if pixel(0, 0)[3] != 0 {
                failures.push(format!(
                    "help: 패널 밖 alpha가 0이 아님 ({})",
                    pixel(0, 0)[3]
                ));
            }
            if let Some(card) = layout.cards.first() {
                let rect = pixel_rect(card.title_rect, scale, self.width, self.height);
                let base = pixel(rect.0, rect.1);
                let mut has_text_pixel = false;
                'rows: for y in rect.1..rect.3 {
                    for x in rect.0..rect.2 {
                        let candidate = pixel(x, y);
                        let difference = (i16::from(candidate[0]) - i16::from(base[0])).abs()
                            + (i16::from(candidate[1]) - i16::from(base[1])).abs()
                            + (i16::from(candidate[2]) - i16::from(base[2])).abs();
                        if candidate[3] != 0 && difference > 45 {
                            has_text_pixel = true;
                            break 'rows;
                        }
                    }
                }
                if !has_text_pixel {
                    failures.push("help: 카드 제목 영역에 글자 픽셀이 없음".to_owned());
                }
            } else {
                failures.push("help: 카드 레이아웃이 비어 있음".to_owned());
            }
            let target = self.swap_target.as_ref().expect("help swap target exists");
            set_context_target(&self.context, target)?;
            Ok(failures)
        }

        fn render_to(
            &mut self,
            target: &ID2D1Bitmap1,
            layout: &HelpLayout,
            content: &HelpContent,
            hover_action: Option<usize>,
            scroll: f64,
        ) -> Result<()> {
            set_context_target(&self.context, target)?;
            unsafe {
                self.context.BeginDraw();
                self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                self.context.Clear(Some(&D2D1_COLOR_F::default()));
            }
            let draw_result = self.draw_contents(layout, content, hover_action, scroll);
            let end_result = unsafe { self.context.EndDraw(None, None) };
            draw_result?;
            end_result
        }

        fn draw_contents(
            &self,
            layout: &HelpLayout,
            content: &HelpContent,
            hover_action: Option<usize>,
            scroll: f64,
        ) -> Result<()> {
            let theme = help_theme(content.opacity)?;
            let scale = px_per_dip(self.dpi);
            let surface = unsafe {
                self.context
                    .CreateSolidColorBrush(&color(theme.surface), None)
            }?;
            let panel = rounded_rect(layout.panel, 18.0, scale);
            unsafe { self.context.FillRoundedRectangle(&panel, &surface) };
            unsafe {
                self.context
                    .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER)
            };
            let border = unsafe {
                self.context
                    .CreateSolidColorBrush(&color(theme.border), None)
            }?;
            unsafe {
                self.context
                    .DrawRoundedRectangle(&panel, &border, 1.0, None)
            };
            self.draw_text(
                &content.title,
                &self.formats.title,
                layout.title,
                theme.text,
                scale,
            )?;

            let clip = d2d_rect(layout.cards_viewport, scale);
            unsafe {
                self.context
                    .PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE)
            };
            let mut action = 1usize;
            for card in &layout.cards {
                let Some(group) = content.groups.get(card.group_index) else {
                    continue;
                };
                let card_rect = card.rect.offset_y(-scroll);
                if card_rect.bottom <= layout.cards_viewport.top
                    || card_rect.top >= layout.cards_viewport.bottom
                {
                    action += group.items.len();
                    continue;
                }
                let card_brush =
                    unsafe { self.context.CreateSolidColorBrush(&color(theme.card), None) }?;
                unsafe {
                    self.context
                        .FillRoundedRectangle(&rounded_rect(card_rect, 10.0, scale), &card_brush)
                };
                self.draw_text(
                    group.name,
                    &self.formats.group,
                    card.title_rect.offset_y(-scroll),
                    theme.text,
                    scale,
                )?;
                if group.items.is_empty() {
                    if let Some(rect) = card.item_rects.first() {
                        self.draw_text(
                            "일치하는 항목 없음",
                            &self.formats.empty,
                            rect.offset_y(-scroll),
                            theme.secondary,
                            scale,
                        )?;
                    }
                    continue;
                }
                for (index, item) in group.items.iter().enumerate() {
                    let Some(base_rect) = card.item_rects.get(index) else {
                        continue;
                    };
                    let rect = base_rect.offset_y(-scroll);
                    if item.current || hover_action == Some(action) {
                        let fill = if item.current {
                            theme.selected
                        } else {
                            theme.hover
                        };
                        let brush =
                            unsafe { self.context.CreateSolidColorBrush(&color(fill), None) }?;
                        unsafe {
                            self.context
                                .FillRoundedRectangle(&rounded_rect(rect, 7.0, scale), &brush)
                        };
                    }
                    let name_rect = RectF::new(
                        rect.left + 8.0,
                        rect.top,
                        rect.left + rect.width() * 0.68,
                        rect.bottom,
                    );
                    let key_rect = RectF::new(
                        rect.left + rect.width() * 0.60,
                        rect.top,
                        rect.right - 8.0,
                        rect.bottom,
                    );
                    self.draw_text(
                        &item.name,
                        &self.formats.item,
                        name_rect,
                        if item.current {
                            theme.selected_text
                        } else {
                            theme.text
                        },
                        scale,
                    )?;
                    self.draw_text(
                        &item.keys,
                        &self.formats.key,
                        key_rect,
                        theme.secondary,
                        scale,
                    )?;
                    action += 1;
                }
            }
            unsafe { self.context.PopAxisAlignedClip() };
            Ok(())
        }

        fn draw_text(
            &self,
            value: &str,
            format: &IDWriteTextFormat,
            rect: RectF,
            tint: [f64; 4],
            scale: f64,
        ) -> Result<()> {
            let brush = unsafe { self.context.CreateSolidColorBrush(&color(tint), None) }?;
            let target: ID2D1RenderTarget = self.context.cast()?;
            let text = value.encode_utf16().collect::<Vec<_>>();
            unsafe {
                target.DrawText(
                    &text,
                    format,
                    &d2d_rect(rect, scale),
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                )
            };
            Ok(())
        }
    }

    impl TextFormats {
        fn new(factory: &IDWriteFactory, dpi: u32) -> Result<Self> {
            let scale = px_per_dip(dpi) as f32;
            Ok(Self {
                title: text_format(
                    factory,
                    24.0 * scale,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                )?,
                group: text_format(
                    factory,
                    14.0 * scale,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                )?,
                item: text_format(
                    factory,
                    12.0 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                )?,
                key: text_format(
                    factory,
                    12.0 * scale,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                )?,
                empty: text_format(
                    factory,
                    12.0 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                )?,
            })
        }
    }

    fn text_format(
        factory: &IDWriteFactory,
        size: f32,
        weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
        alignment: windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT,
    ) -> Result<IDWriteTextFormat> {
        let format = unsafe {
            factory.CreateTextFormat(
                w!("Segoe UI Variable"),
                None::<&IDWriteFontCollection>,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("ko-KR"),
            )
        }?;
        unsafe {
            format.SetTextAlignment(alignment)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        }
        Ok(format)
    }

    fn help_theme(opacity: f64) -> Result<HelpTheme> {
        if high_contrast()? {
            return Ok(HelpTheme {
                surface: system_color(COLOR_WINDOW),
                border: system_color(COLOR_WINDOWTEXT),
                card: system_color(COLOR_WINDOW),
                text: system_color(COLOR_WINDOWTEXT),
                secondary: system_color(COLOR_GRAYTEXT),
                hover: system_color(COLOR_HIGHLIGHT),
                selected: system_color(COLOR_HIGHLIGHT),
                selected_text: system_color(COLOR_HIGHLIGHTTEXT),
            });
        }
        Ok(HelpTheme {
            surface: RadialPaletteArcStyle::surface_color(opacity),
            border: RadialPaletteArcStyle::OUTER_BORDER,
            card: [0.12, 0.14, 0.19, 0.88],
            text: [1.0, 1.0, 1.0, 0.96],
            secondary: RadialPaletteArcStyle::INACTIVE_ICON,
            hover: RadialPaletteArcStyle::HOVER_FILL,
            selected: RadialPaletteArcStyle::SELECTED_NODE_FILL,
            selected_text: RadialPaletteArcStyle::ACTIVE_ICON,
        })
    }

    fn system_color(index: windows::Win32::Graphics::Gdi::SYS_COLOR_INDEX) -> [f64; 4] {
        let value = unsafe { GetSysColor(index) };
        [
            f64::from(value & 0xff) / 255.0,
            f64::from((value >> 8) & 0xff) / 255.0,
            f64::from((value >> 16) & 0xff) / 255.0,
            1.0,
        ]
    }

    fn positive_dimension(value: i32) -> Result<u32> {
        u32::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| Error::new(E_FAIL, "help dimension must be positive"))
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

    fn rounded_rect(rect: RectF, radius: f64, scale: f64) -> D2D1_ROUNDED_RECT {
        D2D1_ROUNDED_RECT {
            rect: d2d_rect(rect, scale),
            radiusX: (radius * scale) as f32,
            radiusY: (radius * scale) as f32,
        }
    }

    fn d2d_rect(rect: RectF, scale: f64) -> D2D_RECT_F {
        D2D_RECT_F {
            left: (rect.left * scale) as f32,
            top: (rect.top * scale) as f32,
            right: (rect.right * scale) as f32,
            bottom: (rect.bottom * scale) as f32,
        }
    }

    fn color(value: [f64; 4]) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: value[0] as f32,
            g: value[1] as f32,
            b: value[2] as f32,
            a: value[3] as f32,
        }
    }

    fn pixel_rect(rect: RectF, scale: f64, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let left = (rect.left * scale)
            .floor()
            .clamp(0.0, f64::from(width.saturating_sub(1))) as u32;
        let top = (rect.top * scale)
            .floor()
            .clamp(0.0, f64::from(height.saturating_sub(1))) as u32;
        let right = (rect.right * scale)
            .ceil()
            .clamp(f64::from(left + 1), f64::from(width)) as u32;
        let bottom = (rect.bottom * scale)
            .ceil()
            .clamp(f64::from(top + 1), f64::from(height)) as u32;
        (left, top, right, bottom)
    }
}

#[cfg(windows)]
pub use windows_impl::*;

#[cfg(test)]
mod tests {
    use super::{calculate_layout, content_width, select_execution_scope, ExecutionScope, RectF};

    #[test]
    fn layout_column_boundary_table_uses_swift_widths() {
        let cases = [(1_426.0, 4), (1_425.0, 2), (766.0, 2), (765.0, 1)];
        assert_eq!(content_width(4), 1_346.0);
        assert_eq!(content_width(2), 686.0);
        for (work_width, expected_columns) in cases {
            let layout = calculate_layout(
                RectF::new(0.0, 0.0, work_width, 1_080.0),
                &[2, 2, 2, 2],
                true,
            );
            assert_eq!(layout.columns, expected_columns, "work width {work_width}");
        }
    }

    #[test]
    fn execution_scope_selection_table_matches_swift_priority() {
        let cases = [
            (true, true, true, true, true, Some(ExecutionScope::Zoom)),
            (
                false,
                true,
                true,
                true,
                true,
                Some(ExecutionScope::Annotation),
            ),
            (false, false, true, true, true, Some(ExecutionScope::Global)),
            (
                false,
                false,
                true,
                true,
                false,
                Some(ExecutionScope::Annotation),
            ),
            (false, false, true, false, false, Some(ExecutionScope::Zoom)),
            (false, false, false, false, false, None),
        ];
        for (zoom, annotation, has_zoom, has_annotation, has_global, expected) in cases {
            assert_eq!(
                select_execution_scope(zoom, annotation, has_zoom, has_annotation, has_global,),
                expected
            );
        }
    }
}
