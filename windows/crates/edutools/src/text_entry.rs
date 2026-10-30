use crate::log::Logger;
use crate::render::px_per_dip;
use crate::z_bands::{raise_band_order, Band, BandRegistration};
use edutools_core::ink::{
    text_entry_action, text_entry_width, Point, Size, TextEntryAction, TextEntryKey,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{w, Error, Result};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_WRAP,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject,
    EndPaint, FillRect, GetStockObject, GetSysColor, GetSysColorBrush, RoundRect, SelectObject,
    SetBkColor, SetTextColor, SetWindowRgn, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    COLOR_HIGHLIGHT, COLOR_WINDOW, DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL, HBRUSH,
    HFONT, HGDIOBJ, NULL_BRUSH, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID,
};
use windows::Win32::UI::Input::Ime::{
    ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext, GCS_COMPSTR,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_ESCAPE, VK_PROCESSKEY, VK_RETURN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetForegroundWindow,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, PostMessageW, RegisterClassW,
    SendMessageW, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindow,
    CREATESTRUCTW, ES_AUTOVSCROLL, ES_MULTILINE, ES_WANTRETURN, GWLP_USERDATA, LWA_ALPHA, SW_SHOW,
    WA_INACTIVE, WINDOW_STYLE, WM_ACTIVATE, WM_CHAR, WM_CLOSE, WM_CTLCOLOREDIT, WM_ERASEBKGND,
    WM_KEYDOWN, WM_KILLFOCUS, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SETFONT, WNDCLASSW, WS_CHILD,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

pub const WM_APP_TEXT_ENTRY_OPEN: u32 = 0x8000 + 40;
pub const WM_APP_TEXT_ENTRY_COMMIT: u32 = 0x8000 + 41;
pub const WM_APP_TEXT_ENTRY_CANCEL: u32 = 0x8000 + 42;

pub const CANCEL_ESC: usize = 1;
pub const CANCEL_WINDOW_CLOSE: usize = 2;

const TEXT_ENTRY_CLASS: windows::core::PCWSTR = w!("EduToolsTextEntry");
const EDIT_SUBCLASS_ID: usize = 0xED08;
const EDIT_SET_RECT: u32 = 0x00B3;
const WINDOW_ALPHA: u8 = 240;

static TEXT_INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct TextEntryRequest {
    pub monitor_key: String,
    pub monitor_left: i32,
    pub monitor_top: i32,
    pub monitor_size: Size,
    pub origin: Point,
    pub display_origin: Point,
    pub dpi: u32,
    pub color: [f64; 4],
    pub font_size: f64,
}

#[derive(Clone, Debug)]
pub struct TextEntryCommit {
    pub request: TextEntryRequest,
    pub string: String,
    pub measured: Size,
}

struct TextEntryWindowState {
    owner: HWND,
    popup: HWND,
    edit: HWND,
    background_brush: HBRUSH,
    font: HFONT,
    text_color: COLORREF,
    dpi: u32,
    suppress_char: bool,
    action_posted: bool,
    logger: Logger,
}

impl Drop for TextEntryWindowState {
    fn drop(&mut self) {
        if !self.font.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(self.font.0)) };
        }
        if !self.background_brush.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(self.background_brush.0)) };
        }
    }
}

pub struct TextEntryWindow {
    hwnd: HWND,
    previous_foreground: HWND,
    request: TextEntryRequest,
    state: Box<TextEntryWindowState>,
    _band: BandRegistration,
}

impl TextEntryWindow {
    pub fn open(
        owner: HWND,
        hinstance: HINSTANCE,
        request: TextEntryRequest,
        logger: Logger,
    ) -> Result<Self> {
        register_window_class(hinstance)?;
        let scale = px_per_dip(request.dpi);
        let origin_x_dip = request.display_origin.x / scale;
        let monitor_width_dip = request.monitor_size.width / scale;
        let width_dip = text_entry_width(monitor_width_dip, origin_x_dip);
        let width = (width_dip * scale).round().max(1.0) as i32;
        let height = (110.0 * scale).round().max(1.0) as i32;
        let x = request.monitor_left + request.display_origin.x.round() as i32;
        let y = request.monitor_top + request.display_origin.y.round() as i32;
        let background = COLORREF(unsafe { GetSysColor(COLOR_WINDOW) });
        let background_brush = unsafe { CreateSolidBrush(background) };
        if background_brush.is_invalid() {
            return Err(Error::from_thread());
        }
        let font_height = -(request.font_size * scale).round().max(1.0) as i32;
        let font = unsafe {
            CreateFontW(
                font_height,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                u32::from(DEFAULT_PITCH.0) | u32::from(FF_DONTCARE.0),
                w!("Segoe UI"),
            )
        };
        if font.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(background_brush.0)) };
            return Err(Error::from_thread());
        }

        let mut state = Box::new(TextEntryWindowState {
            owner,
            popup: HWND::default(),
            edit: HWND::default(),
            background_brush,
            font,
            text_color: colorref(request.color),
            dpi: request.dpi,
            suppress_char: false,
            action_posted: false,
            logger: logger.clone(),
        });
        let state_ptr = state.as_mut() as *mut TextEntryWindowState;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                TEXT_ENTRY_CLASS,
                w!("EduTools Text"),
                WS_POPUP,
                x,
                y,
                width,
                height,
                Some(owner),
                None,
                Some(hinstance),
                Some(state_ptr.cast::<c_void>()),
            )
        }?;
        state.popup = hwnd;

        let border = (1.5 * scale).ceil().max(1.0) as i32;
        let edit_style = WINDOW_STYLE(
            WS_CHILD.0
                | WS_VISIBLE.0
                | ES_MULTILINE as u32
                | ES_AUTOVSCROLL as u32
                | ES_WANTRETURN as u32,
        );
        let edit = match unsafe {
            CreateWindowExW(
                Default::default(),
                w!("EDIT"),
                w!(""),
                edit_style,
                border,
                border,
                (width - border * 2).max(1),
                (height - border * 2).max(1),
                Some(hwnd),
                None,
                Some(hinstance),
                None,
            )
        } {
            Ok(edit) => edit,
            Err(error) => {
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
        };
        state.edit = edit;
        if !unsafe {
            SetWindowSubclass(
                edit,
                Some(edit_subclass_proc),
                EDIT_SUBCLASS_ID,
                state_ptr as usize,
            )
        }
        .as_bool()
        {
            let error = Error::from_thread();
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }

        unsafe {
            SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        let horizontal = (5.0 * scale).round() as i32;
        let vertical = (4.0 * scale).round() as i32;
        let formatting_rect = RECT {
            left: horizontal,
            top: vertical,
            right: (width - border * 2 - horizontal).max(horizontal + 1),
            bottom: (height - border * 2 - vertical).max(vertical + 1),
        };
        unsafe {
            SendMessageW(
                edit,
                EDIT_SET_RECT,
                Some(WPARAM(0)),
                Some(LPARAM((&formatting_rect as *const RECT) as isize)),
            );
        }

        let radius = (5.0 * scale).round().max(1.0) as i32;
        let region =
            unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2) };
        if region.is_invalid() || unsafe { SetWindowRgn(hwnd, Some(region), true) } == 0 {
            if !region.is_invalid() {
                let _ = unsafe { DeleteObject(HGDIOBJ(region.0)) };
            }
            let error = Error::from_thread();
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }
        if let Err(error) =
            unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), WINDOW_ALPHA, LWA_ALPHA) }
        {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }

        let previous_foreground = unsafe { GetForegroundWindow() };
        TEXT_INPUT_ACTIVE.store(true, Ordering::Release);
        let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
        if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            logger.event("텍스트 편집 창 전경 전환 실패; Windows foreground lock 제한 가능");
        }
        if let Err(error) = unsafe { SetFocus(Some(edit)) } {
            logger.event(&format!("텍스트 EDIT 포커스 설정 실패: {error}"));
        }
        let band = BandRegistration::new(hwnd, Band::Dialog);
        if let Err(error) = raise_band_order() {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }
        logger.event(&format!(
            "텍스트 편집 창 열림: monitor={} 위치=({x},{y}) 폭={width_dip:.2}DIP/{width}px 글꼴={:.2}DIP dpi={}",
            request.monitor_key, request.font_size, request.dpi
        ));

        Ok(Self {
            hwnd,
            previous_foreground,
            request,
            state,
            _band: band,
        })
    }

    pub fn commit(&self) -> Result<TextEntryCommit> {
        let string = window_text(self.state.edit)?;
        let measured = measure_text(&string, self.request.font_size)?;
        Ok(TextEntryCommit {
            request: self.request.clone(),
            string,
            measured,
        })
    }

    pub fn close(&mut self) -> Result<()> {
        if self.hwnd.is_invalid() {
            return Ok(());
        }
        let hwnd = self.hwnd;
        unsafe { DestroyWindow(hwnd) }?;
        self.hwnd = HWND::default();
        if !self.previous_foreground.is_invalid()
            && !unsafe { SetForegroundWindow(self.previous_foreground) }.as_bool()
        {
            self.state
                .logger
                .event("텍스트 편집 종료 후 이전 전경 앱 복원 실패; foreground lock 제한 가능");
        }
        Ok(())
    }
}

impl Drop for TextEntryWindow {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            self.state
                .logger
                .event(&format!("텍스트 편집 창 제거 실패: {error}"));
        }
    }
}

pub fn text_input_active() -> bool {
    TEXT_INPUT_ACTIVE.load(Ordering::Acquire)
}

pub fn cancel_reason(code: usize) -> &'static str {
    match code {
        CANCEL_ESC => "Esc",
        CANCEL_WINDOW_CLOSE => "창 닫힘",
        _ => "알 수 없는 요청",
    }
}

fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
    let class = WNDCLASSW {
        lpfnWndProc: Some(text_entry_window_proc),
        hInstance: hinstance,
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        lpszClassName: TEXT_ENTRY_CLASS,
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

fn measure_text(string: &str, font_size: f64) -> Result<Size> {
    let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
    let format = unsafe {
        factory.CreateTextFormat(
            w!("Segoe UI"),
            None::<&IDWriteFontCollection>,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_size as f32,
            w!("ko-KR"),
        )
    }?;
    unsafe { format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP) }?;
    let text = string.encode_utf16().collect::<Vec<_>>();
    let layout = unsafe { factory.CreateTextLayout(&text, &format, 320.0, 1_000_000.0) }?;
    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe { layout.GetMetrics(&mut metrics) }?;
    Ok(Size {
        width: f64::from(metrics.width).ceil(),
        height: f64::from(metrics.height).ceil(),
    })
}

fn window_text(hwnd: HWND) -> Result<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length < 0 {
        return Err(Error::from_thread());
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    buffer.truncate(copied.max(0) as usize);
    Ok(String::from_utf16_lossy(&buffer))
}

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    let state = reference_data as *mut TextEntryWindowState;
    if state.is_null() {
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    let state = unsafe { &mut *state };
    match message {
        WM_KEYDOWN => {
            let virtual_key = wparam.0 as u16;
            let key = if virtual_key == VK_RETURN.0 {
                TextEntryKey::Enter
            } else if virtual_key == VK_ESCAPE.0 {
                TextEntryKey::Escape
            } else {
                TextEntryKey::Other
            };
            let shift_pressed = (unsafe { GetKeyState(VK_SHIFT.0 as i32) } as u16 & 0x8000) != 0;
            let ime_composing = ime_is_composing(hwnd, virtual_key);
            if virtual_key == VK_PROCESSKEY.0 {
                state
                    .logger
                    .event("IME 조합 키 기본 처리: VK_PROCESSKEY를 EDIT에 전달");
            }
            match text_entry_action(key, shift_pressed, ime_composing) {
                TextEntryAction::Commit => {
                    state.suppress_char = true;
                    post_action(state, WM_APP_TEXT_ENTRY_COMMIT, 0);
                    return LRESULT(0);
                }
                TextEntryAction::Cancel => {
                    state.suppress_char = true;
                    post_action(state, WM_APP_TEXT_ENTRY_CANCEL, CANCEL_ESC);
                    return LRESULT(0);
                }
                TextEntryAction::Default if key == TextEntryKey::Enter && ime_composing => {
                    state.logger.event("IME 조합 중 Enter 무시: EDIT 기본 처리");
                }
                TextEntryAction::LineBreak | TextEntryAction::Default => {}
            }
        }
        WM_CHAR if state.suppress_char => {
            state.suppress_char = false;
            return LRESULT(0);
        }
        WM_KILLFOCUS => {
            state
                .logger
                .event("텍스트 EDIT 포커스 이탈: Swift와 같이 자동 확정/취소하지 않음");
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn post_action(state: &mut TextEntryWindowState, message: u32, reason: usize) {
    if state.action_posted {
        return;
    }
    match unsafe { PostMessageW(Some(state.owner), message, WPARAM(reason), LPARAM(0)) } {
        Ok(()) => state.action_posted = true,
        Err(error) => state
            .logger
            .event(&format!("텍스트 편집 동작 전달 실패: {error}")),
    }
}

fn ime_is_composing(hwnd: HWND, virtual_key: u16) -> bool {
    if virtual_key == VK_PROCESSKEY.0 {
        return true;
    }
    let context = unsafe { ImmGetContext(hwnd) };
    if context.is_invalid() {
        return false;
    }
    let length = unsafe { ImmGetCompositionStringW(context, GCS_COMPSTR, None, 0) };
    let _ = unsafe { ImmReleaseContext(hwnd, context) };
    length > 0
}

unsafe extern "system" fn text_entry_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let state = create.lpCreateParams as *mut TextEntryWindowState;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
    }
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TextEntryWindowState };
    if !state.is_null() {
        let state = unsafe { &mut *state };
        match message {
            WM_CTLCOLOREDIT => {
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut c_void);
                unsafe {
                    SetTextColor(hdc, state.text_color);
                    SetBkColor(hdc, COLORREF(GetSysColor(COLOR_WINDOW)));
                }
                return LRESULT(state.background_brush.0 as isize);
            }
            WM_ERASEBKGND => {
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut c_void);
                let mut rect = RECT::default();
                if unsafe { GetClientRect(hwnd, &mut rect) }.is_ok() {
                    unsafe { FillRect(hdc, &rect, state.background_brush) };
                }
                return LRESULT(1);
            }
            WM_PAINT => {
                paint_border(hwnd, state);
                return LRESULT(0);
            }
            WM_ACTIVATE if (wparam.0 & 0xffff) == WA_INACTIVE as usize => {
                state
                    .logger
                    .event("텍스트 편집 창 비활성: Swift와 같이 자동 확정/취소하지 않음");
                return LRESULT(0);
            }
            WM_CLOSE => {
                post_action(state, WM_APP_TEXT_ENTRY_CANCEL, CANCEL_WINDOW_CLOSE);
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                TEXT_INPUT_ACTIVE.store(false, Ordering::Release);
                state.popup = HWND::default();
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn paint_border(hwnd: HWND, state: &TextEntryWindowState) {
    let mut paint = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut rect = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut rect) }.is_ok() {
        unsafe { FillRect(hdc, &rect, state.background_brush) };
        let scale = px_per_dip(state.dpi);
        let pen_width = (1.5 * scale).round().max(1.0) as i32;
        let pen = unsafe { CreatePen(PS_SOLID, pen_width, COLORREF(GetSysColor(COLOR_HIGHLIGHT))) };
        if !pen.is_invalid() {
            let old_pen = unsafe { SelectObject(hdc, HGDIOBJ(pen.0)) };
            let old_brush = unsafe { SelectObject(hdc, GetStockObject(NULL_BRUSH)) };
            let radius = (10.0 * scale).round().max(2.0) as i32;
            let inset = (pen_width / 2).max(1);
            unsafe {
                let _ = RoundRect(
                    hdc,
                    inset,
                    inset,
                    rect.right - inset,
                    rect.bottom - inset,
                    radius,
                    radius,
                );
                SelectObject(hdc, old_brush);
                SelectObject(hdc, old_pen);
                let _ = DeleteObject(HGDIOBJ(pen.0));
            }
        }
    }
    let _ = unsafe { EndPaint(hwnd, &paint) };
}

fn colorref(color: [f64; 4]) -> COLORREF {
    let component = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    COLORREF(component(color[0]) | component(color[1]) << 8 | component(color[2]) << 16)
}
