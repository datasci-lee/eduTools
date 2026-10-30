//! Win32 설정 창. Swift `SettingsController`의 6개 비-단축키 탭을 선언형 표로 옮긴다.

#[cfg(windows)]
use crate::log::Logger;
#[cfg(windows)]
use crate::z_bands::{raise_band_order, Band, BandRegistration};
use edutools_core::settings::{
    AppSettings, PaletteStyle, RadialPaletteGlassStyle, SettingsColor, SettingsRanges,
    SettingsValidator,
};
use std::collections::BTreeSet;
#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::mem::size_of;
use std::ops::RangeInclusive;
#[cfg(windows)]
use windows::core::{w, Error, Result, PCWSTR, PWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, CreateSolidBrush, DeleteObject, DrawFocusRect, FillRect, FrameRect, GetDC,
    GetMonitorInfoW, GetSysColorBrush, GetTextExtentPoint32W, InvalidateRect, MonitorFromPoint,
    ReleaseDC, SelectObject, COLOR_WINDOW, HFONT, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
#[cfg(windows)]
use windows::Win32::System::Com::CoTaskMemFree;
#[cfg(windows)]
use windows::Win32::UI::Controls::Dialogs::{ChooseColorW, CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW};
#[cfg(windows)]
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, SetScrollInfo, BST_CHECKED, BST_UNCHECKED, DRAWITEMSTRUCT,
    ICC_BAR_CLASSES, ICC_TAB_CLASSES, INITCOMMONCONTROLSEX, NMHDR, NM_RELEASEDCAPTURE, TBM_SETPOS,
    TBM_SETRANGEMAX, TBM_SETRANGEMIN, TB_ENDTRACK, TCIF_TEXT, TCITEMW, TCM_ADJUSTRECT,
    TCM_GETCURSEL, TCM_INSERTITEMW, TCM_SETCURSEL, TCN_SELCHANGE, TRACKBAR_CLASSW, WC_TABCONTROLW,
};
#[cfg(windows)]
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForMonitor, GetDpiForWindow, SystemParametersInfoForDpi,
    MDT_EFFECTIVE_DPI,
};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    SHBrowseForFolderW, SHGetPathFromIDListW, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos, GetParent,
    GetScrollInfo, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsWindow, LoadCursorW,
    MessageBoxW, PostMessageW, RegisterClassW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, BS_AUTOCHECKBOX, BS_OWNERDRAW,
    BS_PUSHBUTTON, CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL,
    CREATESTRUCTW, EN_KILLFOCUS, ES_AUTOHSCROLL, ES_MULTILINE, ES_READONLY, GWLP_USERDATA, HMENU,
    IDC_ARROW, MB_ICONERROR, MB_OK, MINMAXINFO, NONCLIENTMETRICSW, SB_BOTTOM, SB_ENDSCROLL,
    SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP, SB_THUMBPOSITION, SB_THUMBTRACK, SB_TOP,
    SCROLLINFO, SIF_PAGE, SIF_POS, SIF_RANGE, SIF_TRACKPOS, SPI_GETNONCLIENTMETRICS,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_RESTORE, SW_SHOW,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WHEEL_DELTA, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_COMMAND, WM_DESTROY, WM_DPICHANGED, WM_DRAWITEM, WM_GETMINMAXINFO, WM_HSCROLL,
    WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_NOTIFY, WM_SETFONT, WM_SIZE, WM_VSCROLL,
    WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_TABSTOP,
    WS_VISIBLE, WS_VSCROLL,
};

#[cfg(windows)]
pub const WM_APP_SETTINGS_CHANGE: u32 = 0x8000 + 220;
#[cfg(windows)]
const WM_APP_PAGE_SCROLL: u32 = 0x8000 + 239;
#[cfg(windows)]
const WM_APP_PAGE_WHEEL: u32 = 0x8000 + 240;

#[cfg(windows)]
const WINDOW_CLASS: PCWSTR = w!("EduToolsSettingsWindow");
#[cfg(windows)]
const PAGE_CLASS: PCWSTR = w!("EduToolsSettingsPage");
const BASE_DPI: u32 = 96;
const MINIMUM_WIDTH_DIP: i32 = 900;
const MINIMUM_HEIGHT_DIP: i32 = 620;
const INITIAL_WIDTH_DIP: i32 = 980;
const INITIAL_HEIGHT_DIP: i32 = 700;
#[cfg(windows)]
const CONTROL_ID_BASE: i32 = 4_000;
#[cfg(windows)]
const TBM_GETPOS: u32 = 0x0400;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LayoutMetrics {
    minimum_width: i32,
    minimum_height: i32,
    initial_width: i32,
    initial_height: i32,
    outer_margin: i32,
    error_height: i32,
    page_padding: i32,
    row_gap: i32,
    control_height: i32,
    multiline_height: i32,
    help_height: i32,
    label_gutter: i32,
    scrollbar_gutter: i32,
    slider_value_width: i32,
    folder_button_width: i32,
    scroll_line: i32,
}

fn layout_metrics(dpi: u32) -> LayoutMetrics {
    let dpi = normalized_dpi(dpi);
    LayoutMetrics {
        minimum_width: dip(MINIMUM_WIDTH_DIP, dpi),
        minimum_height: dip(MINIMUM_HEIGHT_DIP, dpi),
        initial_width: dip(INITIAL_WIDTH_DIP, dpi),
        initial_height: dip(INITIAL_HEIGHT_DIP, dpi),
        outer_margin: dip(14, dpi),
        error_height: dip(28, dpi),
        page_padding: dip(12, dpi),
        row_gap: dip(10, dpi),
        control_height: dip(25, dpi),
        multiline_height: dip(76, dpi),
        help_height: dip(30, dpi),
        label_gutter: dip(12, dpi),
        scrollbar_gutter: dip(18, dpi),
        slider_value_width: dip(60, dpi),
        folder_button_width: dip(86, dpi),
        scroll_line: dip(24, dpi),
    }
}

fn normalized_dpi(dpi: u32) -> u32 {
    if dpi == 0 {
        BASE_DPI
    } else {
        dpi
    }
}

fn dip(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(normalized_dpi(dpi)) + i64::from(BASE_DPI / 2))
        / i64::from(BASE_DPI)) as i32
}

const UI_PALETTE_OPACITY: RangeInclusive<f64> =
    RadialPaletteGlassStyle::MINIMUM_OPACITY..=RadialPaletteGlassStyle::MAXIMUM_OPACITY;

const COLOR_OPTIONS: &[ComboOption] = &[
    ComboOption::new("색 1", "0"),
    ComboOption::new("색 2", "1"),
    ComboOption::new("색 3", "2"),
    ComboOption::new("색 4", "3"),
    ComboOption::new("색 5", "4"),
    ComboOption::new("색 6", "5"),
];
const PALETTE_OPTIONS: &[ComboOption] = &[
    ComboOption::new("원형", "radial"),
    ComboOption::new("가로", "linear"),
];
const ERASER_MODE_OPTIONS: &[ComboOption] = &[
    ComboOption::new("부분", "partial"),
    ComboOption::new("획", "stroke"),
    ComboOption::new("영역", "area"),
];
const HIT_RULE_OPTIONS: &[ComboOption] = &[
    ComboOption::new("닿는 획", "touching"),
    ComboOption::new("완전히 포함된 획", "contained"),
];
const TARGET_OPTIONS: &[ComboOption] = &[
    ComboOption::new("모든 판서", "all"),
    ComboOption::new("펜만", "penOnly"),
    ComboOption::new("형광펜만", "highlighterOnly"),
];
const AREA_SHAPE_OPTIONS: &[ComboOption] = &[
    ComboOption::new("사각형", "rectangle"),
    ComboOption::new("올가미", "lasso"),
];
const MAGNIFICATION_STEP_OPTIONS: &[ComboOption] = &[
    ComboOption::new("0.25배", "0.25"),
    ComboOption::new("0.5배", "0.5"),
];
const FOLLOW_OPTIONS: &[ComboOption] = &[
    ComboOption::new("항상", "always"),
    ComboOption::new("가장자리", "edge"),
    ComboOption::new("고정", "fixed"),
];
const PATTERN_OPTIONS: &[ComboOption] = &[
    ComboOption::new("없음", "none"),
    ComboOption::new("격자", "grid"),
    ComboOption::new("줄", "lines"),
];
const SOUND_OPTIONS: &[ComboOption] = &[
    ComboOption::new("Glass", "Glass"),
    ComboOption::new("Ping", "Ping"),
    ComboOption::new("Pop", "Pop"),
];
const BROWSER_OPTIONS: &[ComboOption] = &[
    ComboOption::new("Chrome", "chrome"),
    ComboOption::new("YouTube Music 앱", "youtubeMusicApp"),
    ComboOption::new("기본 브라우저", "defaultBrowser"),
];
const BADGE_SIZE_OPTIONS: &[ComboOption] = &[
    ComboOption::new("작게", "small"),
    ComboOption::new("보통", "medium"),
    ComboOption::new("크게", "large"),
];
const DISPLAY_MODE_OPTIONS: &[ComboOption] = &[
    ComboOption::new("남은 시간", "remaining"),
    ComboOption::new("지난 시간", "elapsed"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsTab {
    General,
    Tools,
    Eraser,
    Zoom,
    Board,
    Timer,
    Shortcuts,
    /// W10 화면 탭이 연결되기 전 호출 호환용. 현재는 일반 탭을 보여 준다.
    Display,
}

impl SettingsTab {
    fn index(self) -> usize {
        match self {
            Self::General | Self::Display => 0,
            Self::Tools => 1,
            Self::Eraser => 2,
            Self::Zoom => 3,
            Self::Board => 4,
            Self::Timer => 5,
            Self::Shortcuts => 6,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Value {
    Bool(bool),
    Number(f64),
    Integer(i64),
    Text(String),
    Color(SettingsColor),
    IntList(Vec<i64>),
    DoubleList(Vec<f64>),
    StringList(Vec<String>),
}

#[derive(Clone, Copy)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct ComboOption {
    pub label: &'static str,
    pub value: &'static str,
}

impl ComboOption {
    const fn new(label: &'static str, value: &'static str) -> Self {
        Self { label, value }
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum FieldKind {
    Check,
    Slider {
        range: &'static RangeInclusive<f64>,
        step: f64,
        precision: usize,
    },
    Combo {
        options: &'static [ComboOption],
    },
    Color,
    Text,
    Integer {
        range: &'static RangeInclusive<i64>,
    },
    IntList {
        range: &'static RangeInclusive<i64>,
    },
    DoubleList {
        range: &'static RangeInclusive<f64>,
    },
    StringList,
    Folder,
    ReadOnly,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Field {
    pub id: &'static str,
    pub label: &'static str,
    pub help: Option<&'static str>,
    pub kind: FieldKind,
    pub get: fn(&AppSettings) -> Value,
    pub set: fn(&mut AppSettings, Value),
}

#[derive(Clone, Copy)]
struct TabDefinition {
    title: &'static str,
    fields: &'static [Field],
}

#[derive(Clone, Copy)]
pub struct ExcludedField {
    pub id: &'static str,
    pub reason: &'static str,
}

pub const EXCLUDED_FIELDS: &[ExcludedField] = &[
    ExcludedField {
        id: "general.launchAtLogin",
        reason: "Windows 자동 실행은 122 작업 범위",
    },
    ExcludedField {
        id: "general.displayDisconnectPolicy",
        reason: "현재 Swift 일반 탭에 없고 화면 구성/복원 작업에서 노출",
    },
    ExcludedField {
        id: "general.automaticallyRestoresDisconnectedDisplays",
        reason: "현재 Swift 일반 탭에 없고 화면 구성/복원 작업에서 노출",
    },
    ExcludedField {
        id: "display",
        reason: "화면 프리셋 탭은 W10/121c 범위",
    },
    ExcludedField {
        id: "shortcuts",
        reason: "단축키 탭은 120c 범위",
    },
];

const COMPOSITE_FIELD_COVERAGE: &[(&str, &[&str])] = &[(
    "timer.playlist",
    &["timer.playlistURLs", "timer.defaultPlaylistIndex"],
)];

macro_rules! bool_accessors {
    ($get:ident, $set:ident, $($path:tt)+) => {
        fn $get(settings: &AppSettings) -> Value { Value::Bool(settings.$($path)+) }
        fn $set(settings: &mut AppSettings, value: Value) {
            if let Value::Bool(value) = value { settings.$($path)+ = value; }
        }
    };
}

macro_rules! number_accessors {
    ($get:ident, $set:ident, $($path:tt)+) => {
        fn $get(settings: &AppSettings) -> Value { Value::Number(settings.$($path)+) }
        fn $set(settings: &mut AppSettings, value: Value) {
            if let Value::Number(value) = value { settings.$($path)+ = value; }
        }
    };
}

macro_rules! integer_accessors {
    ($get:ident, $set:ident, $($path:tt)+) => {
        fn $get(settings: &AppSettings) -> Value { Value::Integer(settings.$($path)+) }
        fn $set(settings: &mut AppSettings, value: Value) {
            if let Value::Integer(value) = value { settings.$($path)+ = value; }
        }
    };
}

macro_rules! string_accessors {
    ($get:ident, $set:ident, $($path:tt)+) => {
        fn $get(settings: &AppSettings) -> Value { Value::Text(settings.$($path)+.clone()) }
        fn $set(settings: &mut AppSettings, value: Value) {
            if let Value::Text(value) = value { settings.$($path)+ = value; }
        }
    };
}

macro_rules! color_accessors {
    ($get:ident, $set:ident, $($path:tt)+) => {
        fn $get(settings: &AppSettings) -> Value { Value::Color(settings.$($path)+.clone()) }
        fn $set(settings: &mut AppSettings, value: Value) {
            if let Value::Color(value) = value { settings.$($path)+ = value; }
        }
    };
}

bool_accessors!(
    get_clear_on_end,
    set_clear_on_end,
    general.clears_ink_when_annotation_mode_ends
);
bool_accessors!(
    get_auto_archive,
    set_auto_archive,
    general.auto_archive_enabled
);
bool_accessors!(
    get_help_sharing,
    set_help_sharing,
    help.shows_help_in_sharing
);
bool_accessors!(
    get_shape_recognition,
    set_shape_recognition,
    tools.shape_recognition_enabled
);
bool_accessors!(get_zoom_live, set_zoom_live, zoom.live_by_default);
bool_accessors!(get_music_enabled, set_music_enabled, timer.music_enabled);
bool_accessors!(get_fade_enabled, set_fade_enabled, timer.fade_enabled);
bool_accessors!(get_auto_break, set_auto_break, timer.auto_start_break);

number_accessors!(
    get_palette_opacity,
    set_palette_opacity,
    tools.palette_glass_opacity
);
number_accessors!(
    get_highlighter_opacity,
    set_highlighter_opacity,
    tools.highlighter_opacity
);
number_accessors!(
    get_pressure_ratio,
    set_pressure_ratio,
    tools.pressure_minimum_width_ratio
);
number_accessors!(get_laser_size, set_laser_size, tools.laser_size);
number_accessors!(get_laser_trail, set_laser_trail, tools.laser_trail_duration);
number_accessors!(get_eraser_diameter, set_eraser_diameter, eraser.diameter);
number_accessors!(get_zoom_start, set_zoom_start, zoom.starting_magnification);
number_accessors!(
    get_zoom_maximum,
    set_zoom_maximum,
    zoom.maximum_magnification
);
number_accessors!(
    get_zoom_smoothness,
    set_zoom_smoothness,
    zoom.follow_smoothness
);
number_accessors!(get_zoom_dimming, set_zoom_dimming, zoom.preview_dimming);
number_accessors!(get_fade_duration, set_fade_duration, timer.fade_duration);
number_accessors!(
    get_badge_opacity,
    set_badge_opacity,
    timer.lesson_badge_opacity
);

integer_accessors!(
    get_lesson_default,
    set_lesson_default,
    timer.lesson_default_minutes
);
integer_accessors!(
    get_break_default,
    set_break_default,
    timer.break_default_minutes
);

string_accessors!(get_eraser_mode, set_eraser_mode, eraser.mode);
string_accessors!(get_eraser_hit, set_eraser_hit, eraser.hit_rule);
string_accessors!(get_eraser_target, set_eraser_target, eraser.target_filter);
string_accessors!(get_eraser_shape, set_eraser_shape, eraser.area_shape);
string_accessors!(get_zoom_follow, set_zoom_follow, zoom.follow_mode);
string_accessors!(
    get_board_pattern,
    set_board_pattern,
    board.background_pattern
);
string_accessors!(
    get_notification_sound,
    set_notification_sound,
    timer.notification_sound
);
string_accessors!(get_break_message, set_break_message, timer.break_message);
string_accessors!(get_music_browser, set_music_browser, timer.music_browser);
string_accessors!(get_badge_size, set_badge_size, timer.lesson_badge_size);
string_accessors!(
    get_display_mode,
    set_display_mode,
    timer.lesson_display_mode
);

color_accessors!(get_laser_color, set_laser_color, tools.laser_color);
color_accessors!(
    get_whiteboard_color,
    set_whiteboard_color,
    board.whiteboard_color
);
color_accessors!(
    get_blackboard_color,
    set_blackboard_color,
    board.blackboard_color
);
color_accessors!(
    get_break_color,
    set_break_color,
    timer.break_background_color
);

fn get_archive_folder(settings: &AppSettings) -> Value {
    Value::Text(
        settings
            .general
            .archive_folder_path
            .clone()
            .unwrap_or_default(),
    )
}

fn set_archive_folder(settings: &mut AppSettings, value: Value) {
    if let Value::Text(value) = value {
        settings.general.archive_folder_path = (!value.is_empty()).then_some(value);
    }
}

fn get_palette_style(settings: &AppSettings) -> Value {
    Value::Text(
        match settings.tools.palette_style {
            PaletteStyle::Radial => "radial",
            PaletteStyle::Linear => "linear",
        }
        .to_owned(),
    )
}

fn set_palette_style(settings: &mut AppSettings, value: Value) {
    if let Value::Text(value) = value {
        settings.tools.palette_style = if value == "linear" {
            PaletteStyle::Linear
        } else {
            PaletteStyle::Radial
        };
    }
}

fn get_zoom_step(settings: &AppSettings) -> Value {
    Value::Text(format_slider_value(settings.zoom.magnification_step, 2))
}

fn set_zoom_step(settings: &mut AppSettings, value: Value) {
    if let Value::Text(value) = value {
        if let Ok(value) = value.parse() {
            settings.zoom.magnification_step = value;
        }
    }
}

fn get_lesson_presets(settings: &AppSettings) -> Value {
    Value::IntList(settings.timer.lesson_presets.clone())
}

fn set_lesson_presets(settings: &mut AppSettings, value: Value) {
    if let Value::IntList(value) = value {
        settings.timer.lesson_presets = value;
    }
}

fn get_playlist(settings: &AppSettings) -> Value {
    Value::StringList(settings.timer.playlist_urls.clone())
}

fn set_playlist(settings: &mut AppSettings, value: Value) {
    if let Value::StringList(value) = value {
        settings.timer.playlist_urls = value;
        settings.timer.default_playlist_index = 0;
    }
}

fn get_favorites(settings: &AppSettings) -> Value {
    let text = if settings.tools.favorites.is_empty() {
        "없음".to_owned()
    } else {
        settings
            .tools
            .favorites
            .iter()
            .map(|favorite| {
                let mut detail = format!("{} {:.2}", favorite.tool, favorite.size);
                if let Some(color) = favorite.color_index {
                    detail.push_str(&format!(" / 색 {}", color + 1));
                }
                if let Some(style) = favorite.arrow_style.as_deref() {
                    detail.push_str(&format!(" / {style}"));
                }
                if let Some(mode) = favorite.eraser_mode.as_deref() {
                    detail.push_str(&format!(" / {mode}"));
                }
                detail
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    Value::Text(text)
}

fn ignore_value(_: &mut AppSettings, _: Value) {}

macro_rules! color_slot_accessors {
    ($get:ident, $set:ident, $index:expr) => {
        fn $get(settings: &AppSettings) -> Value {
            Value::Color(settings.tools.colors[$index].clone())
        }
        fn $set(settings: &mut AppSettings, value: Value) {
            if let Value::Color(value) = value {
                settings.tools.colors[$index] = value;
            }
        }
    };
}

color_slot_accessors!(get_color_0, set_color_0, 0);
color_slot_accessors!(get_color_1, set_color_1, 1);
color_slot_accessors!(get_color_2, set_color_2, 2);
color_slot_accessors!(get_color_3, set_color_3, 3);
color_slot_accessors!(get_color_4, set_color_4, 4);
color_slot_accessors!(get_color_5, set_color_5, 5);

macro_rules! memory_accessors {
    ($get_size:ident, $set_size:ident, $get_color:ident, $set_color:ident, $slot:ident) => {
        fn $get_size(settings: &AppSettings) -> Value {
            Value::Number(settings.tools.per_tool_memory.$slot.size)
        }
        fn $set_size(settings: &mut AppSettings, value: Value) {
            if let Value::Number(value) = value {
                settings.tools.per_tool_memory.$slot.size = value;
            }
        }
        fn $get_color(settings: &AppSettings) -> Value {
            Value::Text(settings.tools.per_tool_memory.$slot.color_index.to_string())
        }
        fn $set_color(settings: &mut AppSettings, value: Value) {
            if let Value::Text(value) = value {
                if let Ok(value) = value.parse() {
                    settings.tools.per_tool_memory.$slot.color_index = value;
                }
            }
        }
    };
}

memory_accessors!(
    get_pen_size,
    set_pen_size,
    get_pen_color,
    set_pen_color,
    pen
);
memory_accessors!(
    get_highlighter_size,
    set_highlighter_size,
    get_highlighter_color,
    set_highlighter_color,
    highlighter
);
memory_accessors!(
    get_arrow_size,
    set_arrow_size,
    get_arrow_color,
    set_arrow_color,
    arrow
);
memory_accessors!(
    get_rectangle_size,
    set_rectangle_size,
    get_rectangle_color,
    set_rectangle_color,
    rectangle
);
memory_accessors!(
    get_ellipse_size,
    set_ellipse_size,
    get_ellipse_color,
    set_ellipse_color,
    ellipse
);
memory_accessors!(
    get_text_size,
    set_text_size,
    get_text_color,
    set_text_color,
    text
);

macro_rules! preset_accessors {
    ($get:ident, $set:ident, $index:expr) => {
        fn $get(settings: &AppSettings) -> Value {
            Value::Number(settings.eraser.preset_diameters[$index])
        }
        fn $set(settings: &mut AppSettings, value: Value) {
            if let Value::Number(value) = value {
                settings.eraser.preset_diameters[$index] = value;
            }
        }
    };
}

preset_accessors!(get_eraser_preset_0, set_eraser_preset_0, 0);
preset_accessors!(get_eraser_preset_1, set_eraser_preset_1, 1);
preset_accessors!(get_eraser_preset_2, set_eraser_preset_2, 2);

const GENERAL_FIELDS: &[Field] = &[
    Field {
        id: "general.clearsInkWhenAnnotationModeEnds",
        label: "판서 모드를 끌 때 내용 삭제",
        help: None,
        kind: FieldKind::Check,
        get: get_clear_on_end,
        set: set_clear_on_end,
    },
    Field {
        id: "general.autoArchiveEnabled",
        label: "지우기 전 자동 보관",
        help: None,
        kind: FieldKind::Check,
        get: get_auto_archive,
        set: set_auto_archive,
    },
    Field {
        id: "general.archiveFolderPath",
        label: "자동 보관 폴더",
        help: Some("비워 두면 Windows 기본 폴더를 사용합니다."),
        kind: FieldKind::Folder,
        get: get_archive_folder,
        set: set_archive_folder,
    },
    Field {
        id: "help.showsHelpInSharing",
        label: "도움말을 화면 공유·녹화에 표시",
        help: None,
        kind: FieldKind::Check,
        get: get_help_sharing,
        set: set_help_sharing,
    },
];

const TOOLS_FIELDS: &[Field] = &[
    Field {
        id: "tools.paletteStyle",
        label: "팔레트 모양",
        help: None,
        kind: FieldKind::Combo {
            options: PALETTE_OPTIONS,
        },
        get: get_palette_style,
        set: set_palette_style,
    },
    Field {
        id: "tools.paletteGlassOpacity",
        label: "팔레트 불투명도",
        help: Some("낮을수록 투명합니다."),
        kind: FieldKind::Slider {
            range: &UI_PALETTE_OPACITY,
            step: 0.01,
            precision: 2,
        },
        get: get_palette_opacity,
        set: set_palette_opacity,
    },
    Field {
        id: "tools.colors[0]",
        label: "색상 프리셋 1",
        help: None,
        kind: FieldKind::Color,
        get: get_color_0,
        set: set_color_0,
    },
    Field {
        id: "tools.colors[1]",
        label: "색상 프리셋 2",
        help: None,
        kind: FieldKind::Color,
        get: get_color_1,
        set: set_color_1,
    },
    Field {
        id: "tools.colors[2]",
        label: "색상 프리셋 3",
        help: None,
        kind: FieldKind::Color,
        get: get_color_2,
        set: set_color_2,
    },
    Field {
        id: "tools.colors[3]",
        label: "색상 프리셋 4",
        help: None,
        kind: FieldKind::Color,
        get: get_color_3,
        set: set_color_3,
    },
    Field {
        id: "tools.colors[4]",
        label: "색상 프리셋 5",
        help: None,
        kind: FieldKind::Color,
        get: get_color_4,
        set: set_color_4,
    },
    Field {
        id: "tools.colors[5]",
        label: "색상 프리셋 6",
        help: None,
        kind: FieldKind::Color,
        get: get_color_5,
        set: set_color_5,
    },
    Field {
        id: "tools.perToolMemory.pen.size",
        label: "펜 기본 굵기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::TOOL_SIZE,
            step: 1.0,
            precision: 0,
        },
        get: get_pen_size,
        set: set_pen_size,
    },
    Field {
        id: "tools.perToolMemory.pen.colorIndex",
        label: "펜 색",
        help: None,
        kind: FieldKind::Combo {
            options: COLOR_OPTIONS,
        },
        get: get_pen_color,
        set: set_pen_color,
    },
    Field {
        id: "tools.pressureMinimumWidthRatio",
        label: "필압 최소 굵기",
        help: Some("가장 약하게 눌렀을 때의 굵기 비율. 1이면 필압을 쓰지 않습니다."),
        kind: FieldKind::Slider {
            range: &SettingsRanges::PRESSURE_MINIMUM_WIDTH_RATIO,
            step: 0.05,
            precision: 2,
        },
        get: get_pressure_ratio,
        set: set_pressure_ratio,
    },
    Field {
        id: "tools.perToolMemory.highlighter.size",
        label: "형광펜 기본 굵기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::TOOL_SIZE,
            step: 1.0,
            precision: 0,
        },
        get: get_highlighter_size,
        set: set_highlighter_size,
    },
    Field {
        id: "tools.perToolMemory.highlighter.colorIndex",
        label: "형광펜 색",
        help: None,
        kind: FieldKind::Combo {
            options: COLOR_OPTIONS,
        },
        get: get_highlighter_color,
        set: set_highlighter_color,
    },
    Field {
        id: "tools.highlighterOpacity",
        label: "형광펜 투명도",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::HIGH_LIGHTER_OPACITY,
            step: 0.05,
            precision: 2,
        },
        get: get_highlighter_opacity,
        set: set_highlighter_opacity,
    },
    Field {
        id: "tools.perToolMemory.arrow.size",
        label: "화살표 굵기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::TOOL_SIZE,
            step: 1.0,
            precision: 0,
        },
        get: get_arrow_size,
        set: set_arrow_size,
    },
    Field {
        id: "tools.perToolMemory.arrow.colorIndex",
        label: "화살표 색",
        help: None,
        kind: FieldKind::Combo {
            options: COLOR_OPTIONS,
        },
        get: get_arrow_color,
        set: set_arrow_color,
    },
    Field {
        id: "tools.perToolMemory.rectangle.size",
        label: "사각형 굵기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::TOOL_SIZE,
            step: 1.0,
            precision: 0,
        },
        get: get_rectangle_size,
        set: set_rectangle_size,
    },
    Field {
        id: "tools.perToolMemory.rectangle.colorIndex",
        label: "사각형 색",
        help: None,
        kind: FieldKind::Combo {
            options: COLOR_OPTIONS,
        },
        get: get_rectangle_color,
        set: set_rectangle_color,
    },
    Field {
        id: "tools.perToolMemory.ellipse.size",
        label: "타원 굵기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::TOOL_SIZE,
            step: 1.0,
            precision: 0,
        },
        get: get_ellipse_size,
        set: set_ellipse_size,
    },
    Field {
        id: "tools.perToolMemory.ellipse.colorIndex",
        label: "타원 색",
        help: None,
        kind: FieldKind::Combo {
            options: COLOR_OPTIONS,
        },
        get: get_ellipse_color,
        set: set_ellipse_color,
    },
    Field {
        id: "tools.perToolMemory.text.size",
        label: "텍스트 크기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::TEXT_SIZE,
            step: 1.0,
            precision: 0,
        },
        get: get_text_size,
        set: set_text_size,
    },
    Field {
        id: "tools.perToolMemory.text.colorIndex",
        label: "텍스트 색",
        help: None,
        kind: FieldKind::Combo {
            options: COLOR_OPTIONS,
        },
        get: get_text_color,
        set: set_text_color,
    },
    Field {
        id: "tools.laserColor",
        label: "레이저 색",
        help: None,
        kind: FieldKind::Color,
        get: get_laser_color,
        set: set_laser_color,
    },
    Field {
        id: "tools.laserSize",
        label: "레이저 크기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::LASER_SIZE,
            step: 1.0,
            precision: 0,
        },
        get: get_laser_size,
        set: set_laser_size,
    },
    Field {
        id: "tools.laserTrailDuration",
        label: "레이저 사라지는 시간",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::LASER_TRAIL_DURATION,
            step: 0.1,
            precision: 1,
        },
        get: get_laser_trail,
        set: set_laser_trail,
    },
    Field {
        id: "tools.shapeRecognitionEnabled",
        label: "도형 자동 보정",
        help: None,
        kind: FieldKind::Check,
        get: get_shape_recognition,
        set: set_shape_recognition,
    },
    Field {
        id: "tools.favorites",
        label: "즐겨찾기",
        help: Some("추가·삭제는 원형 팔레트에서 수행합니다."),
        kind: FieldKind::ReadOnly,
        get: get_favorites,
        set: ignore_value,
    },
];

const ERASER_FIELDS: &[Field] = &[
    Field {
        id: "eraser.mode",
        label: "기본 방식",
        help: None,
        kind: FieldKind::Combo {
            options: ERASER_MODE_OPTIONS,
        },
        get: get_eraser_mode,
        set: set_eraser_mode,
    },
    Field {
        id: "eraser.diameter",
        label: "기본 크기",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ERASER_DIAMETER,
            step: 1.0,
            precision: 0,
        },
        get: get_eraser_diameter,
        set: set_eraser_diameter,
    },
    Field {
        id: "eraser.presetDiameters[0]",
        label: "크기 프리셋 1",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ERASER_DIAMETER,
            step: 1.0,
            precision: 0,
        },
        get: get_eraser_preset_0,
        set: set_eraser_preset_0,
    },
    Field {
        id: "eraser.presetDiameters[1]",
        label: "크기 프리셋 2",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ERASER_DIAMETER,
            step: 1.0,
            precision: 0,
        },
        get: get_eraser_preset_1,
        set: set_eraser_preset_1,
    },
    Field {
        id: "eraser.presetDiameters[2]",
        label: "크기 프리셋 3",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ERASER_DIAMETER,
            step: 1.0,
            precision: 0,
        },
        get: get_eraser_preset_2,
        set: set_eraser_preset_2,
    },
    Field {
        id: "eraser.hitRule",
        label: "판정 기준",
        help: None,
        kind: FieldKind::Combo {
            options: HIT_RULE_OPTIONS,
        },
        get: get_eraser_hit,
        set: set_eraser_hit,
    },
    Field {
        id: "eraser.targetFilter",
        label: "대상",
        help: None,
        kind: FieldKind::Combo {
            options: TARGET_OPTIONS,
        },
        get: get_eraser_target,
        set: set_eraser_target,
    },
    Field {
        id: "eraser.areaShape",
        label: "영역 모양",
        help: None,
        kind: FieldKind::Combo {
            options: AREA_SHAPE_OPTIONS,
        },
        get: get_eraser_shape,
        set: set_eraser_shape,
    },
];

const ZOOM_FIELDS: &[Field] = &[
    Field {
        id: "zoom.startingMagnification",
        label: "시작 배율",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ZOOM_MAGNIFICATION,
            step: 0.25,
            precision: 2,
        },
        get: get_zoom_start,
        set: set_zoom_start,
    },
    Field {
        id: "zoom.maximumMagnification",
        label: "최대 배율",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ZOOM_MAGNIFICATION,
            step: 0.25,
            precision: 2,
        },
        get: get_zoom_maximum,
        set: set_zoom_maximum,
    },
    Field {
        id: "zoom.magnificationStep",
        label: "배율 단계",
        help: None,
        kind: FieldKind::Combo {
            options: MAGNIFICATION_STEP_OPTIONS,
        },
        get: get_zoom_step,
        set: set_zoom_step,
    },
    Field {
        id: "zoom.followMode",
        label: "따라가기",
        help: None,
        kind: FieldKind::Combo {
            options: FOLLOW_OPTIONS,
        },
        get: get_zoom_follow,
        set: set_zoom_follow,
    },
    Field {
        id: "zoom.followSmoothness",
        label: "따라가기 부드러움",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ZOOM_FOLLOW_SMOOTHNESS,
            step: 0.05,
            precision: 2,
        },
        get: get_zoom_smoothness,
        set: set_zoom_smoothness,
    },
    Field {
        id: "zoom.previewDimming",
        label: "미리보기 회색 농도",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::ZOOM_PREVIEW_DIMMING,
            step: 0.05,
            precision: 2,
        },
        get: get_zoom_dimming,
        set: set_zoom_dimming,
    },
    Field {
        id: "zoom.liveByDefault",
        label: "라이브 줌을 기본값으로",
        help: None,
        kind: FieldKind::Check,
        get: get_zoom_live,
        set: set_zoom_live,
    },
];

const BOARD_FIELDS: &[Field] = &[
    Field {
        id: "board.whiteboardColor",
        label: "화이트보드 배경",
        help: None,
        kind: FieldKind::Color,
        get: get_whiteboard_color,
        set: set_whiteboard_color,
    },
    Field {
        id: "board.blackboardColor",
        label: "블랙보드 배경",
        help: None,
        kind: FieldKind::Color,
        get: get_blackboard_color,
        set: set_blackboard_color,
    },
    Field {
        id: "board.backgroundPattern",
        label: "배경 무늬",
        help: None,
        kind: FieldKind::Combo {
            options: PATTERN_OPTIONS,
        },
        get: get_board_pattern,
        set: set_board_pattern,
    },
];

const TIMER_FIELDS: &[Field] = &[
    Field {
        id: "timer.lessonPresets",
        label: "수업 프리셋(분)",
        help: Some("쉼표로 구분합니다."),
        kind: FieldKind::IntList {
            range: &SettingsRanges::LESSON_MINUTES,
        },
        get: get_lesson_presets,
        set: set_lesson_presets,
    },
    Field {
        id: "timer.lessonDefaultMinutes",
        label: "기본 수업 시간(분)",
        help: None,
        kind: FieldKind::Integer {
            range: &SettingsRanges::LESSON_MINUTES,
        },
        get: get_lesson_default,
        set: set_lesson_default,
    },
    Field {
        id: "timer.notificationSound",
        label: "알림 사운드",
        help: None,
        kind: FieldKind::Combo {
            options: SOUND_OPTIONS,
        },
        get: get_notification_sound,
        set: set_notification_sound,
    },
    Field {
        id: "timer.autoStartBreak",
        label: "수업 종료 후 쉬는 시간 자동 시작",
        help: None,
        kind: FieldKind::Check,
        get: get_auto_break,
        set: set_auto_break,
    },
    Field {
        id: "timer.lessonBadgeSize",
        label: "수업 배지 크기",
        help: None,
        kind: FieldKind::Combo {
            options: BADGE_SIZE_OPTIONS,
        },
        get: get_badge_size,
        set: set_badge_size,
    },
    Field {
        id: "timer.lessonBadgeOpacity",
        label: "수업 배지 불투명도",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::LESSON_BADGE_OPACITY,
            step: 0.05,
            precision: 2,
        },
        get: get_badge_opacity,
        set: set_badge_opacity,
    },
    Field {
        id: "timer.lessonDisplayMode",
        label: "수업 배지 표시",
        help: None,
        kind: FieldKind::Combo {
            options: DISPLAY_MODE_OPTIONS,
        },
        get: get_display_mode,
        set: set_display_mode,
    },
    Field {
        id: "timer.breakDefaultMinutes",
        label: "기본 쉬는 시간(분)",
        help: None,
        kind: FieldKind::Integer {
            range: &SettingsRanges::BREAK_MINUTES,
        },
        get: get_break_default,
        set: set_break_default,
    },
    Field {
        id: "timer.breakMessage",
        label: "쉬는 시간 화면 문구",
        help: None,
        kind: FieldKind::Text,
        get: get_break_message,
        set: set_break_message,
    },
    Field {
        id: "timer.breakBackgroundColor",
        label: "쉬는 시간 배경색",
        help: None,
        kind: FieldKind::Color,
        get: get_break_color,
        set: set_break_color,
    },
    Field {
        id: "timer.playlist",
        label: "음악 재생목록",
        help: Some(
            "최대 5개. music.youtube.com URL을 줄마다 입력합니다. 첫 항목이 기본 재생목록입니다.",
        ),
        kind: FieldKind::StringList,
        get: get_playlist,
        set: set_playlist,
    },
    Field {
        id: "timer.musicEnabled",
        label: "쉬는 시간 음악 사용",
        help: None,
        kind: FieldKind::Check,
        get: get_music_enabled,
        set: set_music_enabled,
    },
    Field {
        id: "timer.musicBrowser",
        label: "열 브라우저",
        help: None,
        kind: FieldKind::Combo {
            options: BROWSER_OPTIONS,
        },
        get: get_music_browser,
        set: set_music_browser,
    },
    Field {
        id: "timer.fadeEnabled",
        label: "종료 전 페이드아웃",
        help: None,
        kind: FieldKind::Check,
        get: get_fade_enabled,
        set: set_fade_enabled,
    },
    Field {
        id: "timer.fadeDuration",
        label: "페이드 시간(초)",
        help: None,
        kind: FieldKind::Slider {
            range: &SettingsRanges::FADE_DURATION,
            step: 1.0,
            precision: 0,
        },
        get: get_fade_duration,
        set: set_fade_duration,
    },
];

const TABS: &[TabDefinition] = &[
    TabDefinition {
        title: "일반",
        fields: GENERAL_FIELDS,
    },
    TabDefinition {
        title: "도구",
        fields: TOOLS_FIELDS,
    },
    TabDefinition {
        title: "지우개",
        fields: ERASER_FIELDS,
    },
    TabDefinition {
        title: "줌",
        fields: ZOOM_FIELDS,
    },
    TabDefinition {
        title: "보드",
        fields: BOARD_FIELDS,
    },
    TabDefinition {
        title: "타이머",
        fields: TIMER_FIELDS,
    },
    TabDefinition {
        title: "단축키",
        fields: &[],
    },
];

#[derive(Clone, Debug)]
pub struct SettingsChangeRequest {
    pub field_id: &'static str,
    pub value: Value,
}

pub fn apply_change(
    settings: &mut AppSettings,
    request: &SettingsChangeRequest,
) -> std::result::Result<(), String> {
    let field = all_fields()
        .find(|field| field.id == request.field_id)
        .ok_or_else(|| format!("unknown settings field: {}", request.field_id))?;
    validate_value_for_field(field, &request.value)?;
    (field.set)(settings, request.value.clone());
    Ok(())
}

fn validate_value_for_field(field: &Field, value: &Value) -> std::result::Result<(), String> {
    let accepted = match (field.kind, value) {
        (FieldKind::Check, Value::Bool(_)) => true,
        (FieldKind::Slider { range, .. }, Value::Number(value)) => range.contains(value),
        (FieldKind::Combo { options }, Value::Text(value)) => {
            options.iter().any(|option| option.value == value)
        }
        (FieldKind::Color, Value::Color(color)) => {
            [color.red, color.green, color.blue, color.alpha]
                .into_iter()
                .all(|part| (0.0..=1.0).contains(&part))
        }
        (FieldKind::Text | FieldKind::Folder, Value::Text(_)) => true,
        (FieldKind::Integer { range }, Value::Integer(value)) => range.contains(value),
        (FieldKind::IntList { range }, Value::IntList(values)) => {
            !values.is_empty() && values.iter().all(|value| range.contains(value))
        }
        (FieldKind::DoubleList { range }, Value::DoubleList(values)) => {
            !values.is_empty() && values.iter().all(|value| range.contains(value))
        }
        (FieldKind::StringList, Value::StringList(values)) => values.len() <= 5,
        (FieldKind::ReadOnly, _) => false,
        _ => false,
    };
    accepted
        .then_some(())
        .ok_or_else(|| format!("{} value is outside UI range", field.id))
}

fn all_fields() -> impl Iterator<Item = &'static Field> {
    TABS.iter().flat_map(|tab| tab.fields.iter())
}

pub fn tab_field_counts() -> Vec<(&'static str, usize)> {
    TABS.iter()
        .map(|tab| (tab.title, tab.fields.len()))
        .collect()
}

pub fn field_count() -> usize {
    all_fields().count()
}

fn format_slider_value(value: f64, precision: usize) -> String {
    let text = format!("{value:.precision$}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(windows)]
pub struct SettingsWindow {
    state: Box<WindowState>,
    _band: BandRegistration,
}

#[cfg(windows)]
struct WindowState {
    owner: HWND,
    hinstance: HINSTANCE,
    hwnd: HWND,
    tab: HWND,
    error: HWND,
    pages: Vec<PageState>,
    settings: AppSettings,
    font: HFONT,
    dpi: u32,
    label_width: i32,
    logger: Logger,
}

#[cfg(windows)]
struct PageState {
    hwnd: HWND,
    rows: Vec<ControlRow>,
    scroll_position: i32,
    content_height: i32,
    wheel_remainder: i32,
}

#[cfg(windows)]
struct ControlRow {
    field: &'static Field,
    label: HWND,
    control: HWND,
    auxiliary: Option<HWND>,
    help: Option<HWND>,
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct TargetMonitor {
    work_area: RECT,
    dpi: u32,
}

#[cfg(windows)]
fn target_monitor() -> Result<TargetMonitor> {
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) }?;
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Err(Error::from_thread());
    }
    let mut dpi_x = BASE_DPI;
    let mut dpi_y = BASE_DPI;
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }?;
    Ok(TargetMonitor {
        work_area: info.rcWork,
        dpi: normalized_dpi(dpi_x),
    })
}

#[cfg(windows)]
fn initial_window_rect(target: TargetMonitor) -> Result<RECT> {
    let metrics = layout_metrics(target.dpi);
    let mut window = RECT {
        left: 0,
        top: 0,
        right: metrics.initial_width,
        bottom: metrics.initial_height,
    };
    unsafe {
        AdjustWindowRectExForDpi(
            &mut window,
            WS_OVERLAPPEDWINDOW,
            false,
            WINDOW_EX_STYLE(0),
            target.dpi,
        )
    }?;

    let work_width = (target.work_area.right - target.work_area.left).max(1);
    let work_height = (target.work_area.bottom - target.work_area.top).max(1);
    let width = (window.right - window.left).min(work_width).max(1);
    let height = (window.bottom - window.top).min(work_height).max(1);
    let left = target.work_area.left + (work_width - width) / 2;
    let top = target.work_area.top + (work_height - height) / 2;
    Ok(RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    })
}

#[cfg(windows)]
fn window_dpi(hwnd: HWND, fallback: u32) -> u32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    normalized_dpi(if dpi == 0 { fallback } else { dpi })
}

#[cfg(windows)]
impl SettingsWindow {
    pub fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        settings: &AppSettings,
        logger: Logger,
    ) -> Result<Self> {
        register_classes(hinstance)?;
        let common_controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_TAB_CLASSES | ICC_BAR_CLASSES,
        };
        if !unsafe { InitCommonControlsEx(&common_controls) }.as_bool() {
            return Err(Error::from_thread());
        }

        let target = target_monitor()?;
        let mut state = Box::new(WindowState {
            owner,
            hinstance,
            hwnd: HWND::default(),
            tab: HWND::default(),
            error: HWND::default(),
            pages: Vec::new(),
            settings: settings.clone(),
            font: HFONT::default(),
            dpi: target.dpi,
            label_width: 0,
            logger,
        });
        let state_ptr = state.as_mut() as *mut WindowState;
        let rect = initial_window_rect(target)?;
        let owner = (!owner.is_invalid()).then_some(owner);
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WINDOW_CLASS,
                w!("EduTools 설정"),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                owner,
                None,
                Some(hinstance),
                Some(state_ptr.cast()),
            )
        }?;
        state.hwnd = hwnd;
        state.dpi = window_dpi(hwnd, target.dpi);
        if let Err(error) = state.create_controls() {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }
        let band = BandRegistration::new(hwnd, Band::Dialog);
        Ok(Self { state, _band: band })
    }

    pub fn show(&mut self, tab: SettingsTab, settings: &AppSettings) {
        self.state.settings = settings.clone();
        self.state.select_tab(tab.index());
        self.state.refresh_controls(true);
        self.state.clear_error();
        let _ = unsafe { ShowWindow(self.state.hwnd, SW_RESTORE) };
        let _ = unsafe { ShowWindow(self.state.hwnd, SW_SHOW) };
        let _ = unsafe { SetForegroundWindow(self.state.hwnd) };
        let _ = unsafe { SetFocus(Some(self.state.tab)) };
        if tab == SettingsTab::Display {
            self.state
                .logger
                .event("설정 창 탭 이동 요청: 화면 탭은 W10 범위이므로 일반 탭 표시");
        }
        self.state
            .logger
            .event(&format!("설정 창 열림: tab={}", TABS[tab.index()].title));
        if let Err(error) = raise_band_order() {
            self.state
                .logger
                .event(&format!("설정 창 z 대역 정렬 실패: {error}"));
        }
    }

    pub fn refresh_external(&mut self, settings: &AppSettings, source: &str) {
        self.state.settings = settings.clone();
        self.state.refresh_controls(false);
        self.state
            .logger
            .event(&format!("설정 창 외부 변경 반영: source={source}"));
    }

    pub fn accept_change(&mut self, field_id: &str, settings: &AppSettings) {
        self.state.settings = settings.clone();
        self.state.refresh_controls(false);
        self.state.clear_error();
        self.state
            .logger
            .event(&format!("설정 창 필드 변경 성공: id={field_id}"));
    }

    pub fn reject_change(&mut self, field_id: &str, reason: &str, settings: &AppSettings) {
        self.state.settings = settings.clone();
        self.state.refresh_controls(true);
        let message = format!("{field_id}: {reason}");
        self.state.show_error(&message);
        self.state.logger.event(&format!(
            "설정 창 필드 변경 거부: id={field_id} reason={reason}"
        ));
        let message = wide(&message);
        unsafe {
            MessageBoxW(
                Some(self.state.hwnd),
                PCWSTR(message.as_ptr()),
                w!("설정을 적용할 수 없습니다"),
                MB_OK | MB_ICONERROR,
            )
        };
    }

    pub fn hwnd(&self) -> HWND {
        self.state.hwnd
    }

    pub fn field_control_count(&self) -> usize {
        self.state
            .pages
            .iter()
            .flat_map(|page| page.rows.iter())
            .filter(|row| unsafe { IsWindow(Some(row.control)) }.as_bool())
            .count()
    }
}

#[cfg(windows)]
impl Drop for SettingsWindow {
    fn drop(&mut self) {
        if !self.state.hwnd.is_invalid() && unsafe { IsWindow(Some(self.state.hwnd)) }.as_bool() {
            let _ = unsafe { DestroyWindow(self.state.hwnd) };
        }
    }
}

#[cfg(windows)]
impl Drop for WindowState {
    fn drop(&mut self) {
        if !self.font.is_invalid() {
            let _ = unsafe { DeleteObject(self.font.into()) };
        }
    }
}

#[cfg(windows)]
impl WindowState {
    fn create_controls(&mut self) -> Result<()> {
        self.tab = create_child(
            self.hwnd,
            self.hinstance,
            WC_TABCONTROLW,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_CLIPCHILDREN,
            WINDOW_EX_STYLE(0),
            1,
        )?;
        self.error = create_child(
            self.hwnd,
            self.hinstance,
            w!("STATIC"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE,
            WINDOW_EX_STYLE(0),
            2,
        )?;

        for (index, tab) in TABS.iter().enumerate() {
            let mut title = wide(tab.title);
            let item = TCITEMW {
                mask: TCIF_TEXT,
                pszText: PWSTR(title.as_mut_ptr()),
                ..Default::default()
            };
            unsafe {
                SendMessageW(
                    self.tab,
                    TCM_INSERTITEMW,
                    Some(WPARAM(index)),
                    Some(LPARAM((&item as *const TCITEMW) as isize)),
                )
            };
            let page_hwnd = create_child(
                self.hwnd,
                self.hinstance,
                PAGE_CLASS,
                PCWSTR::null(),
                WS_CHILD | WS_CLIPCHILDREN | WS_VSCROLL,
                WINDOW_EX_STYLE(0),
                100 + index as i32,
            )?;
            let rows = tab
                .fields
                .iter()
                .enumerate()
                .map(|(field_index, field)| {
                    self.create_field_row(page_hwnd, field, index, field_index)
                })
                .collect::<Result<Vec<_>>>()?;
            self.pages.push(PageState {
                hwnd: page_hwnd,
                rows,
                scroll_position: 0,
                content_height: 0,
                wheel_remainder: 0,
            });
        }
        self.update_font()?;
        self.select_tab(0);
        self.refresh_controls(true);
        self.layout();
        Ok(())
    }

    fn create_field_row(
        &self,
        page: HWND,
        field: &'static Field,
        tab_index: usize,
        field_index: usize,
    ) -> Result<ControlRow> {
        let control_id = CONTROL_ID_BASE + (tab_index as i32 * 100) + field_index as i32;
        let label_text = wide(field.label);
        let label = create_child(
            page,
            self.hinstance,
            w!("STATIC"),
            PCWSTR(label_text.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            WINDOW_EX_STYLE(0),
            control_id + 1_000,
        )?;
        let (class, style, ex_style) = match field.kind {
            FieldKind::Check => (
                w!("BUTTON"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                WINDOW_EX_STYLE(0),
            ),
            FieldKind::Slider { .. } => (
                TRACKBAR_CLASSW,
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                WINDOW_EX_STYLE(0),
            ),
            FieldKind::Combo { .. } => (
                w!("COMBOBOX"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                WINDOW_EX_STYLE(0),
            ),
            FieldKind::Color => (
                w!("BUTTON"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
                WINDOW_EX_STYLE(0),
            ),
            FieldKind::ReadOnly => (
                w!("EDIT"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_READONLY as u32),
                WS_EX_CLIENTEDGE,
            ),
            FieldKind::StringList => (
                w!("EDIT"),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | WINDOW_STYLE((ES_MULTILINE | ES_AUTOHSCROLL) as u32),
                WS_EX_CLIENTEDGE,
            ),
            _ => (
                w!("EDIT"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                WS_EX_CLIENTEDGE,
            ),
        };
        let control = create_child(
            page,
            self.hinstance,
            class,
            PCWSTR::null(),
            style,
            ex_style,
            control_id,
        )?;
        if let FieldKind::Combo { options } = field.kind {
            for option in options {
                let text = wide(option.label);
                unsafe {
                    SendMessageW(
                        control,
                        CB_ADDSTRING,
                        None,
                        Some(LPARAM(text.as_ptr() as isize)),
                    )
                };
            }
        }
        let auxiliary = match field.kind {
            FieldKind::Slider { .. } => Some(create_child(
                page,
                self.hinstance,
                w!("STATIC"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE,
                WINDOW_EX_STYLE(0),
                control_id + 2_000,
            )?),
            FieldKind::Folder => Some(create_child(
                page,
                self.hinstance,
                w!("BUTTON"),
                w!("선택…"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                WINDOW_EX_STYLE(0),
                control_id + 2_000,
            )?),
            _ => None,
        };
        let help = field
            .help
            .map(|help| {
                let text = wide(help);
                create_child(
                    page,
                    self.hinstance,
                    w!("STATIC"),
                    PCWSTR(text.as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    WINDOW_EX_STYLE(0),
                    control_id + 3_000,
                )
            })
            .transpose()?;
        Ok(ControlRow {
            field,
            label,
            control,
            auxiliary,
            help,
        })
    }

    fn update_font(&mut self) -> Result<()> {
        let new_font = system_message_font(self.dpi)?;
        let old_font = std::mem::replace(&mut self.font, new_font);
        self.for_each_control(|hwnd| {
            unsafe {
                SendMessageW(
                    hwnd,
                    WM_SETFONT,
                    Some(WPARAM(new_font.0 as usize)),
                    Some(LPARAM(1)),
                )
            };
        });
        self.label_width = self.measure_label_width();
        if !old_font.is_invalid() {
            let _ = unsafe { DeleteObject(old_font.into()) };
        }
        Ok(())
    }

    fn measure_label_width(&self) -> i32 {
        let metrics = layout_metrics(self.dpi);
        let fallback = all_fields()
            .map(|field| field.label.encode_utf16().count() as i32 * dip(9, self.dpi))
            .max()
            .unwrap_or(0)
            + metrics.label_gutter;
        let dc = unsafe { GetDC(Some(self.hwnd)) };
        if dc.is_invalid() {
            return fallback;
        }
        let previous = unsafe { SelectObject(dc, self.font.into()) };
        let measured = all_fields()
            .filter_map(|field| {
                let text = field.label.encode_utf16().collect::<Vec<_>>();
                let mut size = SIZE::default();
                unsafe { GetTextExtentPoint32W(dc, &text, &mut size) }
                    .as_bool()
                    .then_some(size.cx)
            })
            .max()
            .map(|width| width + metrics.label_gutter)
            .unwrap_or(fallback);
        let _ = unsafe { SelectObject(dc, previous) };
        unsafe { ReleaseDC(Some(self.hwnd), dc) };
        measured.max(fallback)
    }

    fn for_each_control(&self, mut body: impl FnMut(HWND)) {
        body(self.tab);
        body(self.error);
        for page in &self.pages {
            body(page.hwnd);
            for row in &page.rows {
                body(row.label);
                body(row.control);
                if let Some(hwnd) = row.auxiliary {
                    body(hwnd);
                }
                if let Some(hwnd) = row.help {
                    body(hwnd);
                }
            }
        }
    }

    fn select_tab(&mut self, index: usize) {
        let index = index.min(self.pages.len().saturating_sub(1));
        unsafe {
            SendMessageW(self.tab, TCM_SETCURSEL, Some(WPARAM(index)), None);
        }
        for (candidate, page) in self.pages.iter().enumerate() {
            let _ = unsafe {
                ShowWindow(
                    page.hwnd,
                    if candidate == index { SW_SHOW } else { SW_HIDE },
                )
            };
        }
        self.logger
            .event(&format!("설정 창 탭 전환: tab={}", TABS[index].title));
    }

    fn selected_tab(&self) -> usize {
        let index = unsafe { SendMessageW(self.tab, TCM_GETCURSEL, None, None) }.0;
        usize::try_from(index)
            .unwrap_or(0)
            .min(self.pages.len().saturating_sub(1))
    }

    fn layout(&mut self) {
        if self.hwnd.is_invalid() || self.pages.is_empty() {
            return;
        }
        let mut client = RECT::default();
        if unsafe { GetClientRect(self.hwnd, &mut client) }.is_err() {
            return;
        }
        let metrics = layout_metrics(self.dpi);
        let margin = metrics.outer_margin;
        let error_height = metrics.error_height;
        let width = (client.right - client.left - margin * 2).max(dip(300, self.dpi));
        let height =
            (client.bottom - client.top - margin * 3 - error_height).max(dip(220, self.dpi));
        let _ = unsafe {
            SetWindowPos(
                self.tab,
                None,
                margin,
                margin,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        let mut page_rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        unsafe {
            SendMessageW(
                self.tab,
                TCM_ADJUSTRECT,
                Some(WPARAM(0)),
                Some(LPARAM((&mut page_rect as *mut RECT) as isize)),
            )
        };
        let page_x = margin + page_rect.left;
        let page_y = margin + page_rect.top;
        let page_width = (page_rect.right - page_rect.left).max(1);
        let page_height = (page_rect.bottom - page_rect.top).max(1);
        for page in &mut self.pages {
            let _ = unsafe {
                SetWindowPos(
                    page.hwnd,
                    None,
                    page_x,
                    page_y,
                    page_width,
                    page_height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            layout_page(page, page_width, page_height, self.dpi, self.label_width);
        }
        let _ = unsafe {
            SetWindowPos(
                self.error,
                None,
                margin,
                margin * 2 + height,
                width,
                error_height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }

    fn refresh_controls(&mut self, force: bool) {
        let focus = unsafe { GetFocus() };
        for page in &self.pages {
            for row in &page.rows {
                if !force && (focus == row.control || row.auxiliary == Some(focus)) {
                    continue;
                }
                set_row_value(row, (row.field.get)(&self.settings));
            }
        }
        let radial = self.settings.tools.palette_style == PaletteStyle::Radial;
        if let Some(row) = self.pages.get(1).and_then(|page| {
            page.rows
                .iter()
                .find(|row| row.field.id == "tools.paletteGlassOpacity")
        }) {
            let _ = unsafe { EnableWindow(row.control, radial) };
        }
    }

    fn clear_error(&self) {
        let _ = unsafe { SetWindowTextW(self.error, PCWSTR::null()) };
    }

    fn show_error(&self, message: &str) {
        let message = wide(message);
        let _ = unsafe { SetWindowTextW(self.error, PCWSTR(message.as_ptr())) };
    }

    fn handle_command(&mut self, wparam: WPARAM, lparam: LPARAM) {
        let hwnd = HWND(lparam.0 as *mut c_void);
        let notification = ((wparam.0 >> 16) & 0xffff) as u16;
        let Some((page_index, row_index, auxiliary)) = self.find_row(hwnd) else {
            return;
        };
        let field = self.pages[page_index].rows[row_index].field;
        match field.kind {
            FieldKind::Check if notification == 0 => self.commit_row(page_index, row_index),
            FieldKind::Combo { .. } if notification == CBN_SELCHANGE as u16 => {
                self.commit_row(page_index, row_index)
            }
            FieldKind::Color if notification == 0 => self.choose_row_color(page_index, row_index),
            FieldKind::Folder if auxiliary && notification == 0 => {
                self.choose_row_folder(page_index, row_index)
            }
            FieldKind::Folder
            | FieldKind::Text
            | FieldKind::Integer { .. }
            | FieldKind::IntList { .. }
            | FieldKind::DoubleList { .. }
            | FieldKind::StringList
                if !auxiliary && notification == EN_KILLFOCUS as u16 =>
            {
                self.commit_row(page_index, row_index)
            }
            _ => {}
        }
    }

    fn handle_hscroll(&mut self, wparam: WPARAM, lparam: LPARAM) {
        let hwnd = HWND(lparam.0 as *mut c_void);
        let Some((page_index, row_index, _)) = self.find_row(hwnd) else {
            return;
        };
        let row = &self.pages[page_index].rows[row_index];
        let FieldKind::Slider {
            range,
            step,
            precision,
        } = row.field.kind
        else {
            return;
        };
        let position = unsafe { SendMessageW(hwnd, TBM_GETPOS, None, None) }.0 as i32;
        let value = slider_value(range, step, position);
        if let Some(label) = row.auxiliary {
            set_text(label, &format_slider_value(value, precision));
        }
        if (wparam.0 & 0xffff) as u32 == TB_ENDTRACK {
            self.commit_row(page_index, row_index);
        }
    }

    fn handle_notify(&mut self, lparam: LPARAM) {
        let header = lparam.0 as *const NMHDR;
        if header.is_null() {
            return;
        }
        let header = unsafe { &*header };
        if header.hwndFrom == self.tab && header.code == TCN_SELCHANGE {
            let index = self.selected_tab();
            self.select_tab(index);
        } else if header.code == NM_RELEASEDCAPTURE {
            if let Some((page, row, _)) = self.find_row(header.hwndFrom) {
                if matches!(
                    self.pages[page].rows[row].field.kind,
                    FieldKind::Slider { .. }
                ) {
                    self.commit_row(page, row);
                }
            }
        }
    }

    fn find_row(&self, hwnd: HWND) -> Option<(usize, usize, bool)> {
        self.pages
            .iter()
            .enumerate()
            .find_map(|(page_index, page)| {
                page.rows.iter().enumerate().find_map(|(row_index, row)| {
                    if row.control == hwnd {
                        Some((page_index, row_index, false))
                    } else if row.auxiliary == Some(hwnd) {
                        Some((page_index, row_index, true))
                    } else {
                        None
                    }
                })
            })
    }

    fn commit_row(&mut self, page_index: usize, row_index: usize) {
        let row = &self.pages[page_index].rows[row_index];
        match read_row_value(row) {
            Ok(value) => self.post_change(row.field, value),
            Err(error) => {
                self.show_error(&error);
                self.logger.event(&format!(
                    "설정 창 필드 변경 거부: id={} reason={error}",
                    row.field.id
                ));
                set_row_value(row, (row.field.get)(&self.settings));
            }
        }
    }

    fn choose_row_color(&mut self, page_index: usize, row_index: usize) {
        let row = &self.pages[page_index].rows[row_index];
        let Value::Color(current) = (row.field.get)(&self.settings) else {
            return;
        };
        if let Some(color) = choose_color(self.hwnd, &current) {
            self.post_change(row.field, Value::Color(color));
        }
    }

    fn choose_row_folder(&mut self, page_index: usize, row_index: usize) {
        let row = &self.pages[page_index].rows[row_index];
        if let Some(path) = choose_folder(self.hwnd) {
            set_text(row.control, &path);
            self.post_change(row.field, Value::Text(path));
        }
    }

    fn post_change(&self, field: &'static Field, value: Value) {
        if let Err(reason) = validate_value_for_field(field, &value) {
            self.show_error(&reason);
            self.logger.event(&format!(
                "설정 창 필드 변경 거부: id={} reason={reason}",
                field.id
            ));
            return;
        }
        self.logger
            .event(&format!("설정 창 필드 변경 요청: id={}", field.id));
        let request = Box::new(SettingsChangeRequest {
            field_id: field.id,
            value,
        });
        let pointer = Box::into_raw(request);
        if unsafe {
            PostMessageW(
                Some(self.owner),
                WM_APP_SETTINGS_CHANGE,
                WPARAM(0),
                LPARAM(pointer as isize),
            )
        }
        .is_err()
        {
            let _ = unsafe { Box::from_raw(pointer) };
            self.show_error("설정 변경 요청을 앱에 전달하지 못했습니다.");
        }
    }

    fn handle_page_scroll(&mut self, page_hwnd: HWND, wparam: WPARAM) {
        let Some(page) = self.pages.iter_mut().find(|page| page.hwnd == page_hwnd) else {
            return;
        };
        let mut client = RECT::default();
        if unsafe { GetClientRect(page.hwnd, &mut client) }.is_err() {
            return;
        }
        let page_height = client.bottom - client.top;
        let maximum = (page.content_height - page_height).max(0);
        let code = (wparam.0 & 0xffff) as u32;
        let mut next = page.scroll_position;
        let metrics = layout_metrics(self.dpi);
        match code {
            value if value == SB_TOP.0 as u32 => next = 0,
            value if value == SB_BOTTOM.0 as u32 => next = maximum,
            value if value == SB_LINEUP.0 as u32 => next -= metrics.scroll_line,
            value if value == SB_LINEDOWN.0 as u32 => next += metrics.scroll_line,
            value if value == SB_PAGEUP.0 as u32 => next -= page_height,
            value if value == SB_PAGEDOWN.0 as u32 => next += page_height,
            value if value == SB_THUMBPOSITION.0 as u32 || value == SB_THUMBTRACK.0 as u32 => {
                let mut info = SCROLLINFO {
                    cbSize: size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_TRACKPOS,
                    ..Default::default()
                };
                if unsafe {
                    GetScrollInfo(
                        page.hwnd,
                        windows::Win32::UI::WindowsAndMessaging::SB_VERT,
                        &mut info,
                    )
                }
                .is_ok()
                {
                    next = info.nTrackPos;
                }
            }
            value if value == SB_ENDSCROLL.0 as u32 => return,
            _ => {}
        }
        page.scroll_position = next.clamp(0, maximum);
        layout_page(
            page,
            client.right - client.left,
            page_height,
            self.dpi,
            self.label_width,
        );
    }

    fn handle_page_wheel(&mut self, page_hwnd: HWND, wparam: WPARAM) {
        let Some(page) = self.pages.iter_mut().find(|page| page.hwnd == page_hwnd) else {
            return;
        };
        let delta = ((wparam.0 >> 16) as u16 as i16) as i32;
        if delta == 0 {
            return;
        }
        page.wheel_remainder += delta;
        let steps = page.wheel_remainder / WHEEL_DELTA as i32;
        page.wheel_remainder %= WHEEL_DELTA as i32;
        if steps == 0 {
            return;
        }
        let mut client = RECT::default();
        if unsafe { GetClientRect(page.hwnd, &mut client) }.is_err() {
            return;
        }
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        let maximum = (page.content_height - height).max(0);
        let distance = layout_metrics(self.dpi).scroll_line * 3;
        page.scroll_position = (page.scroll_position - steps * distance).clamp(0, maximum);
        layout_page(page, width, height, self.dpi, self.label_width);
    }

    fn draw_color_button(&self, lparam: LPARAM) -> bool {
        let item = lparam.0 as *const DRAWITEMSTRUCT;
        if item.is_null() {
            return false;
        }
        let item = unsafe { &*item };
        let Some((page, row, _)) = self.find_row(item.hwndItem) else {
            return false;
        };
        let Value::Color(color) = (self.pages[page].rows[row].field.get)(&self.settings) else {
            return false;
        };
        let brush = unsafe { CreateSolidBrush(color_ref(&color)) };
        unsafe { FillRect(item.hDC, &item.rcItem, brush) };
        let border = unsafe { CreateSolidBrush(COLORREF(0x00505050)) };
        unsafe { FrameRect(item.hDC, &item.rcItem, border) };
        let _ = unsafe { DeleteObject(brush.into()) };
        let _ = unsafe { DeleteObject(border.into()) };
        if item.itemState.0 & 0x0010 != 0 {
            let _ = unsafe { DrawFocusRect(item.hDC, &item.rcItem) };
        }
        true
    }
}

#[cfg(windows)]
fn layout_page(page: &mut PageState, width: i32, height: i32, dpi: u32, label_width: i32) {
    let metrics = layout_metrics(dpi);
    let padding = metrics.page_padding;
    let gap = metrics.row_gap;
    let control_height = metrics.control_height;
    let label_width = label_width.max(dip(80, dpi));
    let control_x = padding + label_width + gap;
    let control_width = (width - control_x - padding - metrics.scrollbar_gutter).max(1);
    page.content_height = page.rows.iter().fold(padding * 2, |height, row| {
        let base_height = if matches!(row.field.kind, FieldKind::StringList) {
            metrics.multiline_height
        } else {
            control_height
        };
        let help_height = if row.help.is_some() {
            metrics.help_height
        } else {
            0
        };
        height + base_height + help_height + gap
    });
    let maximum = (page.content_height - height).max(0);
    page.scroll_position = page.scroll_position.clamp(0, maximum);
    let mut y = padding - page.scroll_position;
    for row in &page.rows {
        let base_height = if matches!(row.field.kind, FieldKind::StringList) {
            metrics.multiline_height
        } else {
            control_height
        };
        let help_height = if row.help.is_some() {
            metrics.help_height
        } else {
            0
        };
        let _ = unsafe {
            SetWindowPos(
                row.label,
                None,
                padding,
                y + dip(4, dpi),
                label_width,
                control_height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        match row.field.kind {
            FieldKind::Slider { .. } => {
                let value_width = metrics.slider_value_width.min(control_width);
                let _ = unsafe {
                    SetWindowPos(
                        row.control,
                        None,
                        control_x,
                        y,
                        (control_width - value_width - gap).max(1),
                        control_height,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
                if let Some(aux) = row.auxiliary {
                    let _ = unsafe {
                        SetWindowPos(
                            aux,
                            None,
                            control_x + control_width - value_width,
                            y + dip(4, dpi),
                            value_width,
                            control_height,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        )
                    };
                }
            }
            FieldKind::Folder => {
                let button_width = metrics.folder_button_width.min(control_width);
                let _ = unsafe {
                    SetWindowPos(
                        row.control,
                        None,
                        control_x,
                        y,
                        (control_width - button_width - gap).max(1),
                        control_height,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
                if let Some(aux) = row.auxiliary {
                    let _ = unsafe {
                        SetWindowPos(
                            aux,
                            None,
                            control_x + control_width - button_width,
                            y,
                            button_width,
                            control_height,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        )
                    };
                }
            }
            _ => {
                let _ = unsafe {
                    SetWindowPos(
                        row.control,
                        None,
                        control_x,
                        y,
                        control_width,
                        base_height,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
            }
        }
        if let Some(help) = row.help {
            let _ = unsafe {
                SetWindowPos(
                    help,
                    None,
                    control_x,
                    y + base_height + dip(2, dpi),
                    control_width,
                    help_height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
        }
        y += base_height + help_height + gap;
    }
    let info = SCROLLINFO {
        cbSize: size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: page.content_height.saturating_sub(1),
        nPage: height.max(0) as u32,
        nPos: page.scroll_position,
        ..Default::default()
    };
    unsafe {
        SetScrollInfo(
            page.hwnd,
            windows::Win32::UI::WindowsAndMessaging::SB_VERT,
            &info,
            true,
        )
    };
}

#[cfg(windows)]
fn set_row_value(row: &ControlRow, value: Value) {
    match (row.field.kind, value) {
        (FieldKind::Check, Value::Bool(value)) => unsafe {
            SendMessageW(
                row.control,
                windows::Win32::UI::WindowsAndMessaging::BM_SETCHECK,
                Some(WPARAM(if value {
                    BST_CHECKED.0 as usize
                } else {
                    BST_UNCHECKED.0 as usize
                })),
                None,
            );
        },
        (
            FieldKind::Slider {
                range,
                step,
                precision,
            },
            Value::Number(value),
        ) => {
            let maximum = slider_position(range, step, *range.end());
            unsafe {
                SendMessageW(
                    row.control,
                    TBM_SETRANGEMIN,
                    Some(WPARAM(1)),
                    Some(LPARAM(0)),
                );
                SendMessageW(
                    row.control,
                    TBM_SETRANGEMAX,
                    Some(WPARAM(1)),
                    Some(LPARAM(maximum as isize)),
                );
                SendMessageW(
                    row.control,
                    TBM_SETPOS,
                    Some(WPARAM(1)),
                    Some(LPARAM(slider_position(range, step, value) as isize)),
                );
            }
            if let Some(label) = row.auxiliary {
                set_text(label, &format_slider_value(value, precision));
            }
        }
        (FieldKind::Combo { options }, Value::Text(value)) => {
            let index = options
                .iter()
                .position(|option| option.value == value)
                .unwrap_or(0);
            unsafe {
                SendMessageW(row.control, CB_SETCURSEL, Some(WPARAM(index)), None);
            }
        }
        (FieldKind::Color, Value::Color(_)) => {
            let _ = unsafe { InvalidateRect(Some(row.control), None, true) };
        }
        (FieldKind::Integer { .. }, Value::Integer(value)) => {
            set_text(row.control, &value.to_string())
        }
        (FieldKind::IntList { .. }, Value::IntList(values)) => set_text(
            row.control,
            &values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ),
        (FieldKind::DoubleList { .. }, Value::DoubleList(values)) => set_text(
            row.control,
            &values
                .iter()
                .map(|value| format_slider_value(*value, 2))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        (FieldKind::StringList, Value::StringList(values)) => {
            set_text(row.control, &values.join("\r\n"))
        }
        (_, Value::Text(value)) => set_text(row.control, &value),
        _ => {}
    }
}

#[cfg(windows)]
fn read_row_value(row: &ControlRow) -> std::result::Result<Value, String> {
    match row.field.kind {
        FieldKind::Check => {
            let checked = unsafe {
                SendMessageW(
                    row.control,
                    windows::Win32::UI::WindowsAndMessaging::BM_GETCHECK,
                    None,
                    None,
                )
            }
            .0 as u32
                == BST_CHECKED.0;
            Ok(Value::Bool(checked))
        }
        FieldKind::Slider { range, step, .. } => {
            let position = unsafe { SendMessageW(row.control, TBM_GETPOS, None, None) }.0 as i32;
            Ok(Value::Number(slider_value(range, step, position)))
        }
        FieldKind::Combo { options } => {
            let index = unsafe { SendMessageW(row.control, CB_GETCURSEL, None, None) }.0;
            let index = usize::try_from(index)
                .map_err(|_| format!("{} 선택값이 없습니다", row.field.label))?;
            options
                .get(index)
                .map(|option| Value::Text(option.value.to_owned()))
                .ok_or_else(|| format!("{} 선택값이 올바르지 않습니다", row.field.label))
        }
        FieldKind::Text | FieldKind::Folder => Ok(Value::Text(window_text(row.control))),
        FieldKind::Integer { .. } => window_text(row.control)
            .trim()
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|_| format!("{}에는 정수를 입력하세요", row.field.label)),
        FieldKind::IntList { .. } => {
            parse_list(&window_text(row.control), |part| part.parse::<i64>())
                .map(Value::IntList)
                .map_err(|_| format!("{}에는 쉼표로 구분한 정수를 입력하세요", row.field.label))
        }
        FieldKind::DoubleList { .. } => {
            parse_list(&window_text(row.control), |part| part.parse::<f64>())
                .map(Value::DoubleList)
                .map_err(|_| format!("{}에는 쉼표로 구분한 숫자를 입력하세요", row.field.label))
        }
        FieldKind::StringList => Ok(Value::StringList(
            window_text(row.control)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
        )),
        FieldKind::Color | FieldKind::ReadOnly => {
            Err(format!("{}은 직접 입력할 수 없습니다", row.field.label))
        }
    }
}

#[cfg(windows)]
fn parse_list<T, E>(
    value: &str,
    parse: impl Fn(&str) -> std::result::Result<T, E>,
) -> std::result::Result<Vec<T>, E> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse)
        .collect()
}

#[cfg(windows)]
fn slider_position(range: &RangeInclusive<f64>, step: f64, value: f64) -> i32 {
    ((value.clamp(*range.start(), *range.end()) - *range.start()) / step).round() as i32
}

#[cfg(windows)]
fn slider_value(range: &RangeInclusive<f64>, step: f64, position: i32) -> f64 {
    (*range.start() + f64::from(position) * step).clamp(*range.start(), *range.end())
}

#[cfg(windows)]
fn choose_color(owner: HWND, current: &SettingsColor) -> Option<SettingsColor> {
    let mut custom = [COLORREF(0); 16];
    let mut chooser = CHOOSECOLORW {
        lStructSize: size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: owner,
        rgbResult: color_ref(current),
        lpCustColors: custom.as_mut_ptr(),
        Flags: CC_FULLOPEN | CC_RGBINIT,
        ..Default::default()
    };
    unsafe { ChooseColorW(&mut chooser) }
        .as_bool()
        .then(|| SettingsColor {
            red: f64::from(chooser.rgbResult.0 & 0xff) / 255.0,
            green: f64::from((chooser.rgbResult.0 >> 8) & 0xff) / 255.0,
            blue: f64::from((chooser.rgbResult.0 >> 16) & 0xff) / 255.0,
            alpha: current.alpha,
        })
}

#[cfg(windows)]
fn choose_folder(owner: HWND) -> Option<String> {
    let title = wide("자동 보관 폴더 선택");
    let info = BROWSEINFOW {
        hwndOwner: owner,
        lpszTitle: PCWSTR(title.as_ptr()),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        ..Default::default()
    };
    let list = unsafe { SHBrowseForFolderW(&info) };
    if list.is_null() {
        return None;
    }
    let mut path = [0_u16; 260];
    let success = unsafe { SHGetPathFromIDListW(list, &mut path) }.as_bool();
    unsafe { CoTaskMemFree(Some(list.cast())) };
    success.then(|| {
        String::from_utf16_lossy(
            &path[..path
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(path.len())],
        )
    })
}

#[cfg(windows)]
fn color_ref(color: &SettingsColor) -> COLORREF {
    let part = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    COLORREF(part(color.red) | (part(color.green) << 8) | (part(color.blue) << 16))
}

#[cfg(windows)]
fn create_child(
    parent: HWND,
    hinstance: HINSTANCE,
    class: PCWSTR,
    text: PCWSTR,
    style: WINDOW_STYLE,
    ex_style: WINDOW_EX_STYLE,
    id: i32,
) -> Result<HWND> {
    unsafe {
        CreateWindowExW(
            ex_style,
            class,
            text,
            style,
            0,
            0,
            1,
            1,
            Some(parent),
            Some(HMENU(id as usize as *mut c_void)),
            Some(hinstance),
            None,
        )
    }
}

#[cfg(windows)]
fn system_message_font(dpi: u32) -> Result<HFONT> {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0).0,
            dpi,
        )
    }?;
    Ok(unsafe { CreateFontIndirectW(&metrics.lfMessageFont) })
}

#[cfg(windows)]
fn set_text(hwnd: HWND, value: &str) {
    let value = wide(value);
    let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(value.as_ptr())) };
}

#[cfg(windows)]
fn window_text(hwnd: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return String::new();
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) } as usize;
    String::from_utf16_lossy(&buffer[..copied])
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn register_classes(hinstance: HINSTANCE) -> Result<()> {
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }?;
    let background = unsafe { GetSysColorBrush(COLOR_WINDOW) };
    for (name, procedure) in [
        (
            WINDOW_CLASS,
            settings_window_proc as unsafe extern "system" fn(_, _, _, _) -> _,
        ),
        (
            PAGE_CLASS,
            page_window_proc as unsafe extern "system" fn(_, _, _, _) -> _,
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
            if error.code() != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
                return Err(error);
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
unsafe extern "system" fn settings_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
    if message == WM_NCCREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        if !create.is_null() {
            state_ptr = unsafe { (*create).lpCreateParams.cast::<WindowState>() };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize) };
        }
    }
    if !state_ptr.is_null() {
        let state = unsafe { &mut *state_ptr };
        match message {
            WM_SIZE => {
                state.layout();
                return LRESULT(0);
            }
            WM_COMMAND => {
                state.handle_command(wparam, lparam);
                return LRESULT(0);
            }
            WM_HSCROLL => {
                state.handle_hscroll(wparam, lparam);
                return LRESULT(0);
            }
            WM_NOTIFY => {
                state.handle_notify(lparam);
                return LRESULT(0);
            }
            WM_DRAWITEM if state.draw_color_button(lparam) => return LRESULT(1),
            WM_APP_PAGE_SCROLL => {
                state.handle_page_scroll(HWND(lparam.0 as *mut c_void), wparam);
                return LRESULT(0);
            }
            WM_APP_PAGE_WHEEL => {
                state.handle_page_wheel(HWND(lparam.0 as *mut c_void), wparam);
                return LRESULT(0);
            }
            WM_GETMINMAXINFO => {
                let info = lparam.0 as *mut MINMAXINFO;
                if !info.is_null() {
                    let metrics = layout_metrics(window_dpi(hwnd, state.dpi));
                    unsafe {
                        (*info).ptMinTrackSize.x = metrics.minimum_width;
                        (*info).ptMinTrackSize.y = metrics.minimum_height;
                    }
                }
                return LRESULT(0);
            }
            WM_DPICHANGED => {
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
                let fallback = (wparam.0 & 0xffff) as u32;
                let old_dpi = state.dpi;
                state.dpi = window_dpi(hwnd, fallback);
                if old_dpi != state.dpi {
                    for page in &mut state.pages {
                        page.scroll_position = ((i64::from(page.scroll_position)
                            * i64::from(state.dpi)
                            + i64::from(old_dpi / 2))
                            / i64::from(normalized_dpi(old_dpi)))
                            as i32;
                    }
                }
                if let Err(error) = state.update_font() {
                    state
                        .logger
                        .event(&format!("설정 창 DPI 글꼴 갱신 실패: {error}"));
                }
                state.layout();
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                state.logger.event("설정 창 닫힘");
                return LRESULT(0);
            }
            WM_DESTROY => {
                state.logger.event("설정 창 파괴");
            }
            WM_NCDESTROY => {
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

#[cfg(windows)]
unsafe extern "system" fn page_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_COMMAND | WM_HSCROLL | WM_NOTIFY | WM_DRAWITEM => {
            let parent = unsafe { GetParent(hwnd) };
            if let Ok(parent) = parent {
                return unsafe { SendMessageW(parent, message, Some(wparam), Some(lparam)) };
            }
        }
        WM_VSCROLL => {
            if let Ok(parent) = unsafe { GetParent(hwnd) } {
                return unsafe {
                    SendMessageW(
                        parent,
                        WM_APP_PAGE_SCROLL,
                        Some(wparam),
                        Some(LPARAM(hwnd.0 as isize)),
                    )
                };
            }
        }
        WM_MOUSEWHEEL => {
            if let Ok(parent) = unsafe { GetParent(hwnd) } {
                return unsafe {
                    SendMessageW(
                        parent,
                        WM_APP_PAGE_WHEEL,
                        Some(wparam),
                        Some(LPARAM(hwnd.0 as isize)),
                    )
                };
            }
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

#[cfg(windows)]
pub fn selftest_failures(hinstance: HINSTANCE, logger: Logger) -> Vec<String> {
    let mut failures = pure_selftest_failures();
    match SettingsWindow::new(HWND::default(), hinstance, &AppSettings::default(), logger) {
        Ok(mut window) => {
            if window.field_control_count() != field_count() {
                failures.push(format!(
                    "settings window: HWND 필드 컨트롤 수 {} != 표 필드 수 {}",
                    window.field_control_count(),
                    field_count()
                ));
            }
            window.show(SettingsTab::General, &AppSettings::default());
            let hwnd = window.hwnd();
            drop(window);
            if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
                failures.push("settings window: 즉시 파괴 뒤 HWND가 남음".to_owned());
            }
        }
        Err(error) => failures.push(format!("settings window: 실제 창 생성 실패: {error}")),
    }
    failures
}

fn pure_selftest_failures() -> Vec<String> {
    let mut failures = Vec::new();
    for (expected, tab) in [
        SettingsTab::General,
        SettingsTab::Tools,
        SettingsTab::Eraser,
        SettingsTab::Zoom,
        SettingsTab::Board,
        SettingsTab::Timer,
        SettingsTab::Shortcuts,
    ]
    .into_iter()
    .enumerate()
    {
        if tab.index() != expected {
            failures.push(format!(
                "settings tabs: {tab:?} index={} expected={expected}",
                tab.index()
            ));
        }
    }
    if SettingsTab::Display.index() != 0 {
        failures
            .push("settings tabs: W10 화면 이동 호환 API가 일반 탭으로 폴백하지 않음".to_owned());
    }
    let counted = tab_field_counts()
        .into_iter()
        .map(|(_, count)| count)
        .sum::<usize>();
    if counted != field_count() {
        failures.push(format!(
            "settings fields: tab count sum {counted} != total {}",
            field_count()
        ));
    }
    for excluded in EXCLUDED_FIELDS {
        if excluded.reason.is_empty() {
            failures.push(format!(
                "settings fields: 제외 필드 {} 사유 없음",
                excluded.id
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for field in all_fields() {
        if !ids.insert(field.id) {
            failures.push(format!("settings fields: 중복 id {}", field.id));
        }
        if !matches!(field.kind, FieldKind::ReadOnly) {
            let mut settings = AppSettings::default();
            let current = (field.get)(&settings);
            let request = SettingsChangeRequest {
                field_id: field.id,
                value: current,
            };
            if apply_change(&mut settings, &request).is_err()
                || SettingsValidator::validate(&settings).is_err()
            {
                failures.push(format!(
                    "settings fields: {} get -> set -> validate 왕복 실패",
                    field.id
                ));
            }
        }
        match field.kind {
            FieldKind::Slider { range, step, .. } => {
                for value in [*range.start(), *range.end()] {
                    let mut settings = AppSettings::default();
                    settings.zoom.starting_magnification =
                        *SettingsRanges::ZOOM_MAGNIFICATION.start();
                    settings.zoom.maximum_magnification = *SettingsRanges::ZOOM_MAGNIFICATION.end();
                    let request = SettingsChangeRequest {
                        field_id: field.id,
                        value: Value::Number(value),
                    };
                    if apply_change(&mut settings, &request).is_err()
                        || SettingsValidator::validate(&settings).is_err()
                    {
                        failures.push(format!("settings fields: {} 경계값 {value} 거부", field.id));
                    }
                }
                let outside = *range.start() - step;
                let mut settings = AppSettings::default();
                let request = SettingsChangeRequest {
                    field_id: field.id,
                    value: Value::Number(outside),
                };
                if apply_change(&mut settings, &request).is_ok() {
                    failures.push(format!(
                        "settings fields: {} UI 하한 밖 {outside} 수락",
                        field.id
                    ));
                }
            }
            FieldKind::Combo { options } => {
                for option in options {
                    let mut settings = AppSettings::default();
                    settings.zoom.starting_magnification =
                        *SettingsRanges::ZOOM_MAGNIFICATION.start();
                    settings.zoom.maximum_magnification = *SettingsRanges::ZOOM_MAGNIFICATION.end();
                    let request = SettingsChangeRequest {
                        field_id: field.id,
                        value: Value::Text(option.value.to_owned()),
                    };
                    if apply_change(&mut settings, &request).is_err()
                        || SettingsValidator::validate(&settings).is_err()
                    {
                        failures.push(format!(
                            "settings fields: {} 옵션 {} 거부",
                            field.id, option.value
                        ));
                    }
                }
                let mut settings = AppSettings::default();
                let invalid = SettingsChangeRequest {
                    field_id: field.id,
                    value: Value::Text("__invalid__".to_owned()),
                };
                if apply_change(&mut settings, &invalid).is_ok() {
                    failures.push(format!("settings fields: {} 표 밖 옵션 수락", field.id));
                }
            }
            _ => {}
        }
    }

    let expected_ids = expected_field_and_exclusion_ids();
    let mut actual_ids = BTreeSet::new();
    for id in ids {
        if let Some((_, covered)) = COMPOSITE_FIELD_COVERAGE
            .iter()
            .find(|(field_id, _)| *field_id == id)
        {
            actual_ids.extend(covered.iter().copied());
        } else {
            actual_ids.insert(id);
        }
    }
    actual_ids.extend(EXCLUDED_FIELDS.iter().map(|field| field.id));
    if actual_ids != expected_ids {
        let missing = expected_ids
            .difference(&actual_ids)
            .copied()
            .collect::<Vec<_>>();
        let extra = actual_ids
            .difference(&expected_ids)
            .copied()
            .collect::<Vec<_>>();
        failures.push(format!(
            "settings fields: 사용자 노출 필드 열거 불일치 missing={missing:?} extra={extra:?}"
        ));
    }
    for (value, precision, expected) in [
        (8.0, 2, "8"),
        (0.756, 2, "0.76"),
        (0.05, 2, "0.05"),
        (1.24, 1, "1.2"),
    ] {
        let actual = format_slider_value(value, precision);
        if actual != expected {
            failures.push(format!(
                "settings format: {value} -> {actual}, expected {expected}"
            ));
        }
    }
    for (scale_percent, dpi, expected) in [
        (100, 96, (900, 620, 980, 700, 14, 25, 24)),
        (150, 144, (1350, 930, 1470, 1050, 21, 38, 36)),
        (240, 230, (2156, 1485, 2348, 1677, 34, 60, 58)),
    ] {
        let metrics = layout_metrics(dpi);
        let actual = (
            metrics.minimum_width,
            metrics.minimum_height,
            metrics.initial_width,
            metrics.initial_height,
            metrics.outer_margin,
            metrics.control_height,
            metrics.scroll_line,
        );
        if actual != expected {
            failures.push(format!(
                "settings layout: scale={scale_percent}% dpi={dpi} dimensions={actual:?} expected={expected:?}"
            ));
        }
    }
    failures
}

fn expected_field_and_exclusion_ids() -> BTreeSet<&'static str> {
    [
        "general.launchAtLogin",
        "general.clearsInkWhenAnnotationModeEnds",
        "general.autoArchiveEnabled",
        "general.archiveFolderPath",
        "general.displayDisconnectPolicy",
        "general.automaticallyRestoresDisconnectedDisplays",
        "tools.paletteStyle",
        "tools.paletteGlassOpacity",
        "tools.colors[0]",
        "tools.colors[1]",
        "tools.colors[2]",
        "tools.colors[3]",
        "tools.colors[4]",
        "tools.colors[5]",
        "tools.perToolMemory.pen.size",
        "tools.perToolMemory.pen.colorIndex",
        "tools.perToolMemory.highlighter.size",
        "tools.perToolMemory.highlighter.colorIndex",
        "tools.perToolMemory.arrow.size",
        "tools.perToolMemory.arrow.colorIndex",
        "tools.perToolMemory.rectangle.size",
        "tools.perToolMemory.rectangle.colorIndex",
        "tools.perToolMemory.ellipse.size",
        "tools.perToolMemory.ellipse.colorIndex",
        "tools.perToolMemory.text.size",
        "tools.perToolMemory.text.colorIndex",
        "tools.highlighterOpacity",
        "tools.pressureMinimumWidthRatio",
        "tools.laserColor",
        "tools.laserSize",
        "tools.laserTrailDuration",
        "tools.favorites",
        "tools.shapeRecognitionEnabled",
        "eraser.mode",
        "eraser.diameter",
        "eraser.presetDiameters[0]",
        "eraser.presetDiameters[1]",
        "eraser.presetDiameters[2]",
        "eraser.hitRule",
        "eraser.targetFilter",
        "eraser.areaShape",
        "zoom.startingMagnification",
        "zoom.maximumMagnification",
        "zoom.magnificationStep",
        "zoom.followMode",
        "zoom.followSmoothness",
        "zoom.previewDimming",
        "zoom.liveByDefault",
        "board.whiteboardColor",
        "board.blackboardColor",
        "board.backgroundPattern",
        "timer.lessonPresets",
        "timer.lessonDefaultMinutes",
        "timer.breakDefaultMinutes",
        "timer.notificationSound",
        "timer.breakMessage",
        "timer.breakBackgroundColor",
        "timer.playlistURLs",
        "timer.defaultPlaylistIndex",
        "timer.musicEnabled",
        "timer.musicBrowser",
        "timer.fadeEnabled",
        "timer.fadeDuration",
        "timer.autoStartBreak",
        "timer.lessonBadgeSize",
        "timer.lessonBadgeOpacity",
        "timer.lessonDisplayMode",
        "display",
        "shortcuts",
        "help.showsHelpInSharing",
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::pure_selftest_failures;

    #[test]
    fn declarative_settings_field_tables_are_consistent() {
        let failures = pure_selftest_failures();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
