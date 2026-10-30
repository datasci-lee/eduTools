use crate::capture::CapturedFrame;
use crate::ink_view::pointer_id_from_wparam;
use crate::log::Logger;
use crate::monitor::{Monitor, MonitorRect};
use crate::render::{
    create_swap_chain, create_swap_target, pixel_format, read_bitmap_bgra, GraphicsDevice,
};
use crate::text_entry;
use crate::z_bands::{raise_band_order, Band, BandRegistration};
use edutools_core::zoom::{Point, Rect, ZoomGeometry, ZoomSessionState};
use std::ffi::c_void;
use std::mem::size_of;
use std::time::{Duration, Instant};
use windows::core::{w, Error, Interface, Result};
use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D1_COMPOSITE_MODE_SOURCE_OVER, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1Brightness, CLSID_D2D1Saturation, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory,
    ID2D1Image, ID2D1PathGeometry, ID2D1RenderTarget, D2D1_BITMAP_OPTIONS_NONE,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_BRIGHTNESS_PROP_BLACK_POINT,
    D2D1_BRIGHTNESS_PROP_WHITE_POINT, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE,
    D2D1_INTERPOLATION_MODE_LINEAR, D2D1_PROPERTY_TYPE_FLOAT, D2D1_PROPERTY_TYPE_VECTOR2,
    D2D1_ROUNDED_RECT, D2D1_SATURATION_PROP_SATURATION,
};
use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::{IDXGISurface, IDXGISwapChain1};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ, RGBQUAD,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CopyIcon, CreateCursor, CreateWindowExW, DefWindowProcW, DestroyCursor, DestroyIcon,
    DestroyWindow, GetCursorInfo, GetCursorPos, GetForegroundWindow, GetIconInfo,
    GetWindowLongPtrW, KillTimer, LoadCursorW, PostMessageW, RegisterClassW, SendMessageW,
    SetCursor, SetForegroundWindow, SetTimer, SetWindowDisplayAffinity, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, CREATESTRUCTW, CURSORINFO, CURSOR_SHOWING, GWLP_USERDATA, HCURSOR,
    HICON, ICONINFO, IDC_ARROW, IDC_IBEAM, IDC_WAIT, SWP_SHOWWINDOW, SW_SHOW,
    WDA_EXCLUDEFROMCAPTURE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY,
    WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
    WM_SETCURSOR, WM_TIMER, WNDCLASSW, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

pub const WM_APP_ZOOM_POINTER: u32 = 0x8000 + 142;
pub const WM_APP_ZOOM_WHEEL: u32 = 0x8000 + 143;
pub const WM_APP_ZOOM_REDRAW: u32 = 0x8000 + 145;
pub const WM_APP_ZOOM_INK_INPUT: u32 = 0x8000 + 180;
const ZOOM_CLASS: windows::core::PCWSTR = w!("EduToolsZoomView");
const MESSAGE_TIMER_ID: usize = 0xED17;

struct ZoomWindowState {
    owner: HWND,
    monitor_left: i32,
    monitor_top: i32,
    transparent_cursor: HCURSOR,
    logger: Logger,
}

impl Drop for ZoomWindowState {
    fn drop(&mut self) {
        // CreateCursor 로 만든 커서는 DestroyCursor 가 FALSE 이면서 GetLastError 가 0 일 수 있다
        // (VM 실측) — 오류 코드가 없는 반환은 실패로 기록하지 않는다.
        if let Err(error) = unsafe { DestroyCursor(self.transparent_cursor) } {
            if error.code().is_ok() {
                return;
            }
            self.logger
                .event(&format!("줌 투명 커서 제거 실패: {error}"));
        }
    }
}

pub struct ZoomView {
    hwnd: HWND,
    state: Box<ZoomWindowState>,
    renderer: ZoomRenderer,
    current_frame: Option<CapturedFrame>,
    pointer: Point,
    message: Option<(String, Instant)>,
    _band: BandRegistration,
}

impl ZoomView {
    pub fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        monitor: &Monitor,
        graphics: &GraphicsDevice,
        logger: Logger,
    ) -> Result<Self> {
        register_window_class(hinstance)?;
        let transparent_cursor = make_transparent_cursor()?;
        let mut state = Box::new(ZoomWindowState {
            owner,
            monitor_left: monitor.rect.left,
            monitor_top: monitor.rect.top,
            transparent_cursor,
            logger: logger.clone(),
        });
        let state_ptr = state.as_mut() as *mut ZoomWindowState;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP,
                ZOOM_CLASS,
                w!("EduTools Zoom"),
                WS_POPUP,
                monitor.rect.left,
                monitor.rect.top,
                monitor.rect.width(),
                monitor.rect.height(),
                None,
                None,
                Some(hinstance),
                Some(state_ptr.cast::<c_void>()),
            )
        }?;
        let band = BandRegistration::new(hwnd, Band::Effect);
        if let Err(error) = unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) } {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(Error::new(
                error.code(),
                format!("줌 창을 화면 캡처에서 제외하지 못했습니다: {error}"),
            ));
        }
        if let Err(error) = unsafe {
            SetWindowPos(
                hwnd,
                None,
                monitor.rect.left,
                monitor.rect.top,
                monitor.rect.width(),
                monitor.rect.height(),
                SWP_SHOWWINDOW,
            )
        } {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }
        let mut renderer = match ZoomRenderer::new(
            graphics,
            hwnd,
            monitor.rect.width(),
            monitor.rect.height(),
            monitor.dpi.0,
            logger.clone(),
        ) {
            Ok(renderer) => renderer,
            Err(error) => {
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
        };
        let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
        let focused = focus_zoom_window(hwnd);
        logger.event(&format!(
            "줌 창 생성: monitor={} rect=({},{} {}x{}) WDA_EXCLUDEFROMCAPTURE=true focus={focused}",
            monitor.session_key,
            monitor.rect.left,
            monitor.rect.top,
            monitor.rect.width(),
            monitor.rect.height()
        ));
        if let Err(error) = raise_band_order() {
            renderer.release_target();
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }
        Ok(Self {
            hwnd,
            state,
            renderer,
            current_frame: None,
            pointer: Point::default(),
            message: None,
            _band: band,
        })
    }

    pub fn set_frame(&mut self, frame: CapturedFrame) {
        self.current_frame = Some(frame);
    }

    pub fn current_frame(&self) -> Option<CapturedFrame> {
        self.current_frame.clone()
    }

    pub fn restore_frame(&mut self, frame: CapturedFrame) {
        self.current_frame = Some(frame);
    }

    pub fn has_frame(&self) -> bool {
        self.current_frame.is_some()
    }

    pub fn clear_frame(&mut self) {
        self.current_frame = None;
    }

    pub fn set_pointer(&mut self, point: Point) {
        self.pointer = point;
    }

    pub fn show_message(&mut self, message: impl Into<String>) {
        self.message = Some((message.into(), Instant::now()));
        let timer = unsafe { SetTimer(Some(self.hwnd), MESSAGE_TIMER_ID, 1_300, None) };
        if timer == 0 {
            self.state
                .logger
                .event("줌 배율 메시지 숨김 타이머 시작 실패");
        }
    }

    pub fn render(
        &mut self,
        session: &ZoomSessionState,
        ink_frame: Option<&ID2D1Bitmap1>,
    ) -> Result<()> {
        if self
            .message
            .as_ref()
            .is_some_and(|(_, shown)| shown.elapsed() >= Duration::from_millis(1_300))
        {
            self.message = None;
        }
        self.renderer.render(
            self.current_frame.as_ref(),
            ink_frame,
            session,
            self.pointer,
            self.message.as_ref().map(|(message, _)| message.as_str()),
        )
    }

    pub fn focus(&self) -> bool {
        focus_zoom_window(self.hwnd)
    }
}

fn focus_zoom_window(hwnd: HWND) -> bool {
    if text_entry::text_input_active() || unsafe { GetForegroundWindow() } == hwnd {
        return true;
    }
    unsafe { SetForegroundWindow(hwnd) }.as_bool()
}

impl Drop for ZoomView {
    fn drop(&mut self) {
        self.current_frame = None;
        self.renderer.release_target();
        let _ = unsafe { KillTimer(Some(self.hwnd), MESSAGE_TIMER_ID) };
        if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
            self.state
                .logger
                .event(&format!("줌 창 제거 실패: {error}"));
        }
    }
}

struct ZoomRenderer {
    swap_chain: IDXGISwapChain1,
    context: ID2D1DeviceContext,
    factory: ID2D1Factory,
    text_format: IDWriteTextFormat,
    swap_target: Option<ID2D1Bitmap1>,
    _dcomp_target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    width: u32,
    height: u32,
    dpi: u32,
    cursor_cache: Option<CursorBitmap>,
    last_cursor_failure: Option<String>,
    logger: Logger,
}

#[derive(Clone)]
struct CursorBitmap {
    source_handle: HCURSOR,
    dpi: u32,
    bitmap: ID2D1Bitmap1,
    width: u32,
    height: u32,
    hotspot: Point,
}

impl ZoomRenderer {
    fn new(
        graphics: &GraphicsDevice,
        hwnd: HWND,
        width: i32,
        height: i32,
        dpi: u32,
        logger: Logger,
    ) -> Result<Self> {
        let width = valid_dimension(width)?;
        let height = valid_dimension(height)?;
        let swap_chain = create_swap_chain(graphics, width, height)?;
        let context = graphics.create_d2d_context()?;
        unsafe { context.SetDpi(96.0, 96.0) };
        let factory = unsafe { context.GetFactory() }?;
        let swap_target = Some(create_swap_target(&context, &swap_chain)?);
        let image: ID2D1Image = swap_target.as_ref().expect("swap target exists").cast()?;
        unsafe { context.SetTarget(&image) };
        let (dcomp_target, visual) = graphics.attach_composition_swap_chain(hwnd, &swap_chain)?;
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let text_format = unsafe {
            dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                16.0,
                w!("ko-KR"),
            )
        }?;
        unsafe {
            text_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        Ok(Self {
            swap_chain,
            context,
            factory,
            text_format,
            swap_target,
            _dcomp_target: dcomp_target,
            _visual: visual,
            width,
            height,
            dpi,
            cursor_cache: None,
            last_cursor_failure: None,
            logger,
        })
    }

    fn render(
        &mut self,
        frame: Option<&CapturedFrame>,
        ink_frame: Option<&ID2D1Bitmap1>,
        session: &ZoomSessionState,
        pointer: Point,
        message: Option<&str>,
    ) -> Result<()> {
        let bitmap = frame.map(|frame| self.capture_bitmap(frame)).transpose()?;
        self.draw_scene(bitmap.as_ref(), ink_frame, session, pointer, message)?;
        unsafe { self.swap_chain.Present(1, Default::default()).ok()? };
        Ok(())
    }

    fn capture_bitmap(&self, frame: &CapturedFrame) -> Result<ID2D1Bitmap1> {
        let surface: IDXGISurface = frame.texture.cast()?;
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: pixel_format(),
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };
        let bitmap = unsafe {
            self.context
                .CreateBitmapFromDxgiSurface(&surface, Some(&properties))
        }?;
        let size = unsafe { bitmap.GetPixelSize() };
        if size.width != frame.size.Width as u32 || size.height != frame.size.Height as u32 {
            return Err(Error::new(
                E_FAIL,
                format!(
                    "capture texture size {}x{} != ContentSize {}x{}",
                    size.width, size.height, frame.size.Width, frame.size.Height
                ),
            ));
        }
        Ok(bitmap)
    }

    fn draw_scene(
        &mut self,
        bitmap: Option<&ID2D1Bitmap1>,
        ink_frame: Option<&ID2D1Bitmap1>,
        session: &ZoomSessionState,
        pointer: Point,
        message: Option<&str>,
    ) -> Result<()> {
        let result = (|| -> Result<()> {
            unsafe { self.context.BeginDraw() };
            let black = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            };
            unsafe { self.context.Clear(Some(&black)) };
            let full = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: self.width as f32,
                bottom: self.height as f32,
            };
            if let Some(bitmap) = bitmap {
                let bitmap_size = unsafe { bitmap.GetPixelSize() };
                let full_source = D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: bitmap_size.width as f32,
                    bottom: bitmap_size.height as f32,
                };
                if session.is_previewing {
                    unsafe {
                        self.context.DrawBitmap(
                            bitmap,
                            Some(&full),
                            1.0,
                            D2D1_INTERPOLATION_MODE_LINEAR,
                            Some(&full_source),
                            None,
                        )
                    };
                    self.draw_dimmed_preview(bitmap, &full, session.preview_dimming)?;
                    let source = d2d_rect(session.source_rect());
                    unsafe {
                        self.context.DrawBitmap(
                            bitmap,
                            Some(&source),
                            1.0,
                            D2D1_INTERPOLATION_MODE_LINEAR,
                            Some(&source),
                            None,
                        )
                    };
                    let accent = D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.47,
                        b: 0.84,
                        a: 1.0,
                    };
                    let accent_brush =
                        unsafe { self.context.CreateSolidColorBrush(&accent, None) }?;
                    unsafe {
                        self.context
                            .DrawRectangle(&source, &accent_brush, 2.0, None)
                    };
                    self.draw_label(
                        &ZoomGeometry::magnification_label(session.magnification),
                        preview_label_rect(session.source_rect(), self.width, self.height),
                    )?;
                } else {
                    let source = d2d_rect(session.source_rect());
                    unsafe {
                        self.context.DrawBitmap(
                            bitmap,
                            Some(&full),
                            1.0,
                            D2D1_INTERPOLATION_MODE_LINEAR,
                            Some(&source),
                            None,
                        )
                    };
                }
            }
            if let Some(ink_frame) = ink_frame {
                let source = if session.is_previewing {
                    D2D_RECT_F {
                        left: 0.0,
                        top: 0.0,
                        right: self.width as f32,
                        bottom: self.height as f32,
                    }
                } else {
                    d2d_rect(session.source_rect())
                };
                unsafe {
                    self.context.DrawBitmap(
                        ink_frame,
                        Some(&full),
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        Some(&source),
                        None,
                    )
                };
            }
            self.draw_pointer(session, pointer)?;
            if let Some(message) = message {
                let width = 300.0_f32.min((self.width as f32 - 32.0).max(0.0));
                let rect = D2D_RECT_F {
                    left: ((self.width as f32 - width) / 2.0).max(16.0),
                    top: (self.height as f32 - 68.0).max(0.0),
                    right: ((self.width as f32 + width) / 2.0).min(self.width as f32 - 16.0),
                    bottom: (self.height as f32 - 30.0).max(0.0),
                };
                self.draw_label(message, rect)?;
            }
            unsafe { self.context.EndDraw(None, None)? };
            Ok(())
        })();
        if result.is_err() {
            let _ = unsafe { self.context.EndDraw(None, None) };
        }
        result
    }

    fn draw_dimmed_preview(
        &self,
        bitmap: &ID2D1Bitmap1,
        full: &D2D_RECT_F,
        dimming: f64,
    ) -> Result<()> {
        let saturation = unsafe { self.context.CreateEffect(&CLSID_D2D1Saturation) }?;
        let image: ID2D1Image = bitmap.cast()?;
        let zero = 0.0_f32;
        unsafe {
            saturation.SetInput(0, &image, true);
            saturation.SetValue(
                D2D1_SATURATION_PROP_SATURATION.0 as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                bytes_of(&zero),
            )?;
        }
        let brightness = unsafe { self.context.CreateEffect(&CLSID_D2D1Brightness) }?;
        let saturation_output = unsafe { saturation.GetOutput() }?;
        let black_point = [0.0_f32, 0.0_f32];
        let white_point = [1.0_f32, (1.0 - 0.42 * dimming.clamp(0.0, 1.0)) as f32];
        unsafe {
            brightness.SetInput(0, &saturation_output, true);
            brightness.SetValue(
                D2D1_BRIGHTNESS_PROP_BLACK_POINT.0 as u32,
                D2D1_PROPERTY_TYPE_VECTOR2,
                bytes_of(&black_point),
            )?;
            brightness.SetValue(
                D2D1_BRIGHTNESS_PROP_WHITE_POINT.0 as u32,
                D2D1_PROPERTY_TYPE_VECTOR2,
                bytes_of(&white_point),
            )?;
        }
        let output = unsafe { brightness.GetOutput() }?;
        unsafe {
            self.context.DrawImage(
                &output,
                None,
                Some(full),
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            )
        };
        Ok(())
    }

    fn draw_pointer(&mut self, session: &ZoomSessionState, pointer: Point) -> Result<()> {
        let presented = ZoomGeometry::pointer_presentation_point(pointer, session);
        let scale = ZoomGeometry::pointer_scale(session) as f32;
        match self.system_cursor_bitmap() {
            Ok(None) => {
                self.last_cursor_failure = None;
                return Ok(());
            }
            Ok(Some(cursor)) => {
                self.last_cursor_failure = None;
                self.draw_cursor_bitmap(&cursor, presented, scale);
                return Ok(());
            }
            Err(error) => {
                let reason = error.to_string();
                if self.last_cursor_failure.as_deref() != Some(reason.as_str()) {
                    self.logger
                        .event(&format!("줌 커서 모양 취득 실패; 화살표 폴백: {reason}"));
                    self.last_cursor_failure = Some(reason);
                }
            }
        }
        self.draw_fallback_pointer(presented, scale)
    }

    fn draw_cursor_bitmap(&self, cursor: &CursorBitmap, presented: Point, scale: f32) {
        let origin =
            ZoomGeometry::cursor_bitmap_origin(presented, cursor.hotspot, f64::from(scale));
        let destination = D2D_RECT_F {
            left: origin.x as f32,
            top: origin.y as f32,
            right: origin.x as f32 + cursor.width as f32 * scale,
            bottom: origin.y as f32 + cursor.height as f32 * scale,
        };
        let source = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: cursor.width as f32,
            bottom: cursor.height as f32,
        };
        unsafe {
            self.context.DrawBitmap(
                &cursor.bitmap,
                Some(&destination),
                1.0,
                D2D1_INTERPOLATION_MODE_LINEAR,
                Some(&source),
                None,
            )
        };
    }

    fn system_cursor_bitmap(&mut self) -> Result<Option<CursorBitmap>> {
        let mut info = CURSORINFO {
            cbSize: size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetCursorInfo(&mut info) }?;
        if info.flags != CURSOR_SHOWING || info.hCursor.is_invalid() {
            return Ok(None);
        }
        let cache_matches = self
            .cursor_cache
            .as_ref()
            .is_some_and(|cached| cached.source_handle == info.hCursor && cached.dpi == self.dpi);
        if !cache_matches {
            let acquired = acquire_cursor_bitmap(&self.context, info.hCursor, self.dpi)?;
            self.logger.event(&format!(
                "줌 커서 모양 취득: handle={:p} hotspot=({:.1},{:.1}) size={}x{} dpi={}",
                info.hCursor.0,
                acquired.hotspot.x,
                acquired.hotspot.y,
                acquired.width,
                acquired.height,
                self.dpi
            ));
            self.cursor_cache = Some(acquired);
        }
        Ok(self.cursor_cache.clone())
    }

    fn draw_fallback_pointer(&self, presented: Point, scale: f32) -> Result<()> {
        let geometry = make_pointer_geometry(&self.factory, presented, scale)?;
        let fill = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let outline = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let fill_brush = unsafe { self.context.CreateSolidColorBrush(&fill, None) }?;
        let outline_brush = unsafe { self.context.CreateSolidColorBrush(&outline, None) }?;
        unsafe {
            self.context.FillGeometry(&geometry, &fill_brush, None);
            self.context
                .DrawGeometry(&geometry, &outline_brush, 1.25 * scale, None);
        }
        Ok(())
    }

    fn draw_label(&self, value: &str, rect: D2D_RECT_F) -> Result<()> {
        let background = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.78,
        };
        let white = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let background_brush = unsafe { self.context.CreateSolidColorBrush(&background, None) }?;
        let white_brush = unsafe { self.context.CreateSolidColorBrush(&white, None) }?;
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: 8.0,
            radiusY: 8.0,
        };
        unsafe {
            self.context
                .FillRoundedRectangle(&rounded, &background_brush)
        };
        let target: ID2D1RenderTarget = self.context.cast()?;
        let text = value.encode_utf16().collect::<Vec<_>>();
        unsafe {
            target.DrawText(
                &text,
                &self.text_format,
                &rect,
                &white_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    fn selftest_failures(&mut self) -> Result<Vec<String>> {
        let result = self.selftest_failures_inner();
        if let Some(target) = self.swap_target.as_ref() {
            let image: ID2D1Image = target.cast()?;
            unsafe { self.context.SetTarget(&image) };
        }
        result
    }

    fn selftest_failures_inner(&mut self) -> Result<Vec<String>> {
        let width = self.width;
        let height = self.height;
        let target = create_test_bitmap(&self.context, width, height, true, None)?;
        let source_pixels = synthetic_grid(width, height);
        let source = create_test_bitmap(&self.context, width, height, false, Some(&source_pixels))?;
        let ink = create_synthetic_ink_bitmap(&self.context, width, height, Point::new(0.5, 0.5))?;
        let target_image: ID2D1Image = target.cast()?;
        unsafe { self.context.SetTarget(&target_image) };
        let mut failures = Vec::new();
        let mut session = ZoomSessionState {
            display_id: "selftest".to_owned(),
            screen_size: edutools_core::zoom::Size::new(f64::from(width), f64::from(height)),
            magnification: 2.0,
            starting_magnification: 2.0,
            center: Point::new(200.0, 100.0),
            follow_mode: edutools_core::zoom::ZoomFollowMode::Edge,
            follow_smoothness: 0.22,
            preview_dimming: 0.62,
            is_frozen: false,
            is_previewing: false,
        };
        self.draw_scene(
            Some(&source),
            None,
            &session,
            Point::new(300.0, 180.0),
            None,
        )?;
        let (pixels, pitch) = read_bitmap_bgra(&self.context, &target)?;
        let sample = pixel(&pixels, pitch, 60, 30);
        let expected = grid_color(150, 65);
        if sample
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.abs_diff(expected) > 3)
        {
            failures.push(format!(
                "zoom transform: destination (60,30) BGRA={sample:?}, expected source (150,65) {expected:?}"
            ));
        }

        for frozen in [false, true] {
            session.is_frozen = frozen;
            self.draw_scene(
                Some(&source),
                Some(&ink),
                &session,
                Point::new(300.0, 180.0),
                None,
            )?;
            let (ink_pixels, ink_pitch) = read_bitmap_bgra(&self.context, &target)?;
            let ink_sample = pixel(&ink_pixels, ink_pitch, 80, 100);
            if ink_sample[2] < 180 || ink_sample[3] == 0 {
                failures.push(format!(
                    "zoom ink {}: expected red/alpha at (80,100), BGRA={ink_sample:?}",
                    if frozen { "frozen" } else { "live" }
                ));
            }
        }

        let synthetic_mask = [
            0x30, 0, 0, 0, // AND: 0, 0, 1, 1
            0x50, 0, 0, 0, // XOR: 0, 1, 0, 1
        ];
        let synthetic_rgba = compose_cursor_mask(None, &synthetic_mask, 4, 4, 1);
        let expected_rgba = vec![
            0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255,
        ];
        if synthetic_rgba.as_deref() != Some(expected_rgba.as_slice()) {
            failures.push(format!(
                "zoom cursor mask composition: BGRA={synthetic_rgba:?}, expected={expected_rgba:?}"
            ));
        }

        for (name, resource) in [
            ("arrow", IDC_ARROW),
            ("I-beam", IDC_IBEAM),
            ("wait", IDC_WAIT),
        ] {
            let handle = unsafe { LoadCursorW(None, resource) }?;
            let cursor = acquire_cursor_bitmap(&self.context, handle, self.dpi)?;
            if cursor.hotspot.x < 0.0
                || cursor.hotspot.y < 0.0
                || cursor.hotspot.x >= f64::from(cursor.width)
                || cursor.hotspot.y >= f64::from(cursor.height)
            {
                failures.push(format!(
                    "zoom cursor: {name} hotspot outside bitmap ({:.1},{:.1}) {}x{}",
                    cursor.hotspot.x, cursor.hotspot.y, cursor.width, cursor.height
                ));
            }
            unsafe {
                self.context.BeginDraw();
                self.context.Clear(None);
            }
            self.draw_cursor_bitmap(&cursor, Point::new(160.0, 100.0), 1.0);
            unsafe { self.context.EndDraw(None, None)? };
            let (pointer_pixels, pointer_pitch) = read_bitmap_bgra(&self.context, &target)?;
            let has_alpha = pointer_pixels
                .chunks(pointer_pitch as usize)
                .flat_map(|row| row[..width as usize * 4].chunks_exact(4))
                .any(|pixel| pixel[3] != 0);
            if !has_alpha {
                failures.push(format!("zoom cursor: {name} bitmap alpha가 모두 0"));
            }
        }

        session.is_frozen = false;
        session.is_previewing = true;
        self.draw_scene(
            Some(&source),
            Some(&ink),
            &session,
            Point::new(300.0, 180.0),
            None,
        )?;
        let (preview_pixels, preview_pitch) = read_bitmap_bgra(&self.context, &target)?;
        let preview_sample = pixel(&preview_pixels, preview_pitch, 20, 20);
        let original = grid_color(20, 20);
        let preview_chroma = preview_sample[..3]
            .iter()
            .max()
            .unwrap()
            .saturating_sub(*preview_sample[..3].iter().min().unwrap());
        let original_chroma = original[..3]
            .iter()
            .max()
            .unwrap()
            .saturating_sub(*original[..3].iter().min().unwrap());
        if preview_chroma >= original_chroma {
            failures.push(format!(
                "zoom preview: 회색 효과 채도 감소 없음 original={original:?} preview={preview_sample:?}"
            ));
        }
        Ok(failures)
    }

    fn release_target(&mut self) {
        unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
        self.swap_target = None;
    }
}

pub fn selftest_failures(hinstance: HINSTANCE, logger: Logger) -> Vec<String> {
    let graphics = match GraphicsDevice::new() {
        Ok(graphics) => graphics,
        Err(error) => return vec![format!("zoom: 그래픽 장치 생성 실패: {error}")],
    };
    let rect = MonitorRect {
        left: -30_000,
        top: -30_000,
        right: -29_680,
        bottom: -29_800,
    };
    let monitor = Monitor {
        handle: Default::default(),
        rect,
        work_rect: rect,
        dpi: (96, 96),
        is_primary: false,
        session_key: "zoom-selftest".to_owned(),
    };
    match ZoomView::new(HWND::default(), hinstance, &monitor, &graphics, logger) {
        Ok(mut view) => view
            .renderer
            .selftest_failures()
            .unwrap_or_else(|error| vec![format!("zoom: 오프스크린 렌더 실패: {error}")]),
        Err(error) => vec![format!("zoom: 오프스크린 창 생성 실패: {error}")],
    }
}

fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
    let class = WNDCLASSW {
        lpfnWndProc: Some(zoom_window_proc),
        hInstance: hinstance,
        lpszClassName: ZOOM_CLASS,
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

fn make_transparent_cursor() -> Result<HCURSOR> {
    let and_plane = [0xff_u8; 2];
    let xor_plane = [0_u8; 2];
    unsafe {
        CreateCursor(
            None,
            0,
            0,
            1,
            1,
            and_plane.as_ptr().cast(),
            xor_plane.as_ptr().cast(),
        )
    }
}

fn acquire_cursor_bitmap(
    context: &ID2D1DeviceContext,
    source_cursor: HCURSOR,
    dpi: u32,
) -> Result<CursorBitmap> {
    let mut icon_info = ICONINFO::default();
    let copied = unsafe { CopyIcon(HICON(source_cursor.0)) }.map_err(|error| {
        cursor_bitmap_error(
            &format!("CopyIcon ({error})"),
            &icon_info,
            &BITMAP::default(),
        )
    })?;
    let mut diagnostic = BITMAP::default();
    let result = (|| -> Result<CursorBitmap> {
        unsafe { GetIconInfo(copied, &mut icon_info) }.map_err(|error| {
            cursor_bitmap_error(&format!("GetIconInfo ({error})"), &icon_info, &diagnostic)
        })?;
        let (width, height, pixels) = if !icon_info.hbmColor.is_invalid() {
            let color_info =
                cursor_bitmap_info(icon_info.hbmColor, "hbmColor", &icon_info, &diagnostic)?;
            diagnostic = color_info;
            let width = diagnostic.bmWidth.unsigned_abs();
            let height = diagnostic.bmHeight.unsigned_abs();
            if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
                return Err(cursor_bitmap_error(
                    "GetObjectW(hbmColor) dimensions",
                    &icon_info,
                    &diagnostic,
                ));
            }
            let mut pixels = read_cursor_color_bitmap(
                icon_info.hbmColor,
                width,
                height,
                &icon_info,
                &diagnostic,
            )?;
            if pixels.chunks_exact(4).all(|pixel| pixel[3] == 0) {
                if icon_info.hbmMask.is_invalid() {
                    return Err(cursor_bitmap_error(
                        "GetDIBits(hbmMask): missing AND mask",
                        &icon_info,
                        &diagnostic,
                    ));
                }
                let mask_info =
                    cursor_bitmap_info(icon_info.hbmMask, "hbmMask", &icon_info, &diagnostic)?;
                if mask_info.bmWidth.unsigned_abs() != width
                    || mask_info.bmHeight.unsigned_abs() != height
                {
                    return Err(cursor_bitmap_error(
                        "GetObjectW(hbmMask) dimensions",
                        &icon_info,
                        &mask_info,
                    ));
                }
                let (mask, mask_stride) = read_cursor_mask_bitmap(
                    icon_info.hbmMask,
                    width,
                    height,
                    &icon_info,
                    &mask_info,
                )?;
                pixels =
                    compose_cursor_mask(Some(pixels.as_slice()), &mask, mask_stride, width, height)
                        .ok_or_else(|| {
                            cursor_bitmap_error(
                                "GetDIBits(hbmMask) composition",
                                &icon_info,
                                &mask_info,
                            )
                        })?;
            } else if pixels
                .chunks_exact(4)
                .any(|pixel| pixel[..3].iter().any(|channel| *channel > pixel[3]))
            {
                for pixel in pixels.chunks_exact_mut(4) {
                    let alpha = u16::from(pixel[3]);
                    for channel in &mut pixel[..3] {
                        *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
                    }
                }
            }
            (width, height, pixels)
        } else {
            if icon_info.hbmMask.is_invalid() {
                return Err(cursor_bitmap_error(
                    "GetIconInfo: cursor has no bitmap",
                    &icon_info,
                    &diagnostic,
                ));
            }
            let mask_info =
                cursor_bitmap_info(icon_info.hbmMask, "hbmMask", &icon_info, &diagnostic)?;
            diagnostic = mask_info;
            let width = diagnostic.bmWidth.unsigned_abs();
            let total_height = diagnostic.bmHeight.unsigned_abs();
            if width == 0
                || total_height == 0
                || total_height % 2 != 0
                || width > i32::MAX as u32
                || total_height > i32::MAX as u32
            {
                return Err(cursor_bitmap_error(
                    "GetObjectW(hbmMask) monochrome dimensions",
                    &icon_info,
                    &diagnostic,
                ));
            }
            let height = total_height / 2;
            let (mask, mask_stride) = read_cursor_mask_bitmap(
                icon_info.hbmMask,
                width,
                total_height,
                &icon_info,
                &diagnostic,
            )?;
            let pixels =
                compose_cursor_mask(None, &mask, mask_stride, width, height).ok_or_else(|| {
                    cursor_bitmap_error(
                        "GetDIBits(hbmMask) monochrome composition",
                        &icon_info,
                        &diagnostic,
                    )
                })?;
            (width, height, pixels)
        };
        if pixels.chunks_exact(4).all(|pixel| pixel[3] == 0) {
            return Err(cursor_bitmap_error(
                "GetDIBits: cursor bitmap alpha is empty",
                &icon_info,
                &diagnostic,
            ));
        }
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: pixel_format(),
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };
        let pitch = width.checked_mul(4).ok_or_else(|| {
            cursor_bitmap_error(
                "GetDIBits: cursor bitmap pitch overflow",
                &icon_info,
                &diagnostic,
            )
        })?;
        let bitmap = unsafe {
            context.CreateBitmap(
                D2D_SIZE_U { width, height },
                Some(pixels.as_ptr().cast()),
                pitch,
                &properties,
            )
        }?;
        Ok(CursorBitmap {
            source_handle: source_cursor,
            dpi,
            bitmap,
            width,
            height,
            hotspot: Point::new(f64::from(icon_info.xHotspot), f64::from(icon_info.yHotspot)),
        })
    })();
    let mut cleanup_failures = Vec::new();
    if !icon_info.hbmColor.is_invalid() {
        if !unsafe { DeleteObject(HGDIOBJ(icon_info.hbmColor.0)) }.as_bool() {
            cleanup_failures.push("DeleteObject(hbmColor)");
        }
    }
    if !icon_info.hbmMask.is_invalid() {
        if !unsafe { DeleteObject(HGDIOBJ(icon_info.hbmMask.0)) }.as_bool() {
            cleanup_failures.push("DeleteObject(hbmMask)");
        }
    }
    if unsafe { DestroyIcon(copied) }.is_err() {
        cleanup_failures.push("DestroyIcon");
    }
    if cleanup_failures.is_empty() {
        result
    } else {
        let cleanup_error =
            cursor_bitmap_error(&cleanup_failures.join(" + "), &icon_info, &diagnostic);
        match result {
            Ok(_) => Err(cleanup_error),
            Err(error) => Err(Error::new(E_FAIL, format!("{error}; {cleanup_error}"))),
        }
    }
}

fn cursor_bitmap_info(
    bitmap: HBITMAP,
    label: &str,
    icon_info: &ICONINFO,
    diagnostic: &BITMAP,
) -> Result<BITMAP> {
    let mut bitmap_info = BITMAP::default();
    let copied_bytes = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            size_of::<BITMAP>() as i32,
            Some((&mut bitmap_info as *mut BITMAP).cast()),
        )
    };
    if copied_bytes != size_of::<BITMAP>() as i32 {
        return Err(cursor_bitmap_error(
            &format!("GetObjectW({label}), returned {copied_bytes}"),
            icon_info,
            diagnostic,
        ));
    }
    Ok(bitmap_info)
}

fn read_cursor_color_bitmap(
    bitmap: HBITMAP,
    width: u32,
    height: u32,
    icon_info: &ICONINFO,
    diagnostic: &BITMAP,
) -> Result<Vec<u8>> {
    let byte_count = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            cursor_bitmap_error("GetDIBits(hbmColor) buffer size", icon_info, diagnostic)
        })?;
    let mut pixels = vec![0_u8; byte_count];
    let mut dib_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: u32::try_from(byte_count).unwrap_or(0),
            ..Default::default()
        },
        ..Default::default()
    };
    let hdc = unsafe { CreateCompatibleDC(None) };
    if hdc.is_invalid() {
        return Err(cursor_bitmap_error(
            "CreateCompatibleDC(hbmColor)",
            icon_info,
            diagnostic,
        ));
    }
    let read_lines = unsafe {
        GetDIBits(
            hdc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut dib_info,
            DIB_RGB_COLORS,
        )
    };
    let deleted = unsafe { DeleteDC(hdc) }.as_bool();
    if read_lines != height as i32 {
        return Err(cursor_bitmap_error(
            &format!("GetDIBits(hbmColor), returned {read_lines}/{height} lines"),
            icon_info,
            diagnostic,
        ));
    }
    if !deleted {
        return Err(cursor_bitmap_error(
            "DeleteDC(hbmColor)",
            icon_info,
            diagnostic,
        ));
    }
    Ok(pixels)
}

#[repr(C)]
struct MonochromeBitmapInfo {
    header: BITMAPINFOHEADER,
    colors: [RGBQUAD; 2],
}

fn read_cursor_mask_bitmap(
    bitmap: HBITMAP,
    width: u32,
    height: u32,
    icon_info: &ICONINFO,
    diagnostic: &BITMAP,
) -> Result<(Vec<u8>, usize)> {
    let stride = width
        .checked_add(31)
        .map(|bits| bits / 32)
        .and_then(|words| words.checked_mul(4))
        .and_then(|stride| usize::try_from(stride).ok())
        .ok_or_else(|| cursor_bitmap_error("GetDIBits(hbmMask) stride", icon_info, diagnostic))?;
    let byte_count = stride
        .checked_mul(usize::try_from(height).unwrap_or(usize::MAX))
        .ok_or_else(|| {
            cursor_bitmap_error("GetDIBits(hbmMask) buffer size", icon_info, diagnostic)
        })?;
    let mut mask = vec![0_u8; byte_count];
    let mut dib_info = MonochromeBitmapInfo {
        header: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 1,
            biCompression: BI_RGB.0,
            biSizeImage: u32::try_from(byte_count).unwrap_or(0),
            biClrUsed: 2,
            ..Default::default()
        },
        colors: [
            RGBQUAD::default(),
            RGBQUAD {
                rgbBlue: 255,
                rgbGreen: 255,
                rgbRed: 255,
                rgbReserved: 0,
            },
        ],
    };
    let hdc = unsafe { CreateCompatibleDC(None) };
    if hdc.is_invalid() {
        return Err(cursor_bitmap_error(
            "CreateCompatibleDC(hbmMask)",
            icon_info,
            diagnostic,
        ));
    }
    let read_lines = unsafe {
        GetDIBits(
            hdc,
            bitmap,
            0,
            height,
            Some(mask.as_mut_ptr().cast()),
            (&mut dib_info as *mut MonochromeBitmapInfo).cast::<BITMAPINFO>(),
            DIB_RGB_COLORS,
        )
    };
    let deleted = unsafe { DeleteDC(hdc) }.as_bool();
    if read_lines != height as i32 {
        return Err(cursor_bitmap_error(
            &format!("GetDIBits(hbmMask), returned {read_lines}/{height} lines"),
            icon_info,
            diagnostic,
        ));
    }
    if !deleted {
        return Err(cursor_bitmap_error(
            "DeleteDC(hbmMask)",
            icon_info,
            diagnostic,
        ));
    }
    Ok((mask, stride))
}

fn compose_cursor_mask(
    color: Option<&[u8]>,
    mask: &[u8],
    mask_stride: usize,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let mask_rows = if color.is_some() {
        height
    } else {
        height.checked_mul(2)?
    };
    if mask.len() < mask_stride.checked_mul(mask_rows)? {
        return None;
    }
    let color_byte_count = width.checked_mul(height)?.checked_mul(4)?;
    if color.is_some_and(|pixels| pixels.len() < color_byte_count) {
        return None;
    }
    let mut pixels = vec![0_u8; color_byte_count];
    for y in 0..height {
        for x in 0..width {
            let mask_bit = |row: usize| mask[row * mask_stride + x / 8] & (0x80_u8 >> (x % 8)) != 0;
            let and = mask_bit(y);
            let offset = (y * width + x) * 4;
            if let Some(color) = color {
                let source = &color[offset..offset + 4];
                if and && source[..3].iter().any(|channel| *channel != 0) {
                    // XOR 반전 픽셀은 배경을 알 수 없으므로 불투명 흰색 윤곽으로 근사한다.
                    pixels[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
                } else if !and {
                    pixels[offset..offset + 3].copy_from_slice(&source[..3]);
                    pixels[offset + 3] = 255;
                }
            } else {
                let xor = mask_bit(y + height);
                pixels[offset..offset + 4].copy_from_slice(match (and, xor) {
                    (false, false) => &[0, 0, 0, 255],
                    (false, true) => &[255, 255, 255, 255],
                    (true, false) => &[0, 0, 0, 0],
                    (true, true) => &[255, 255, 255, 255],
                });
            }
        }
    }
    Some(pixels)
}

fn cursor_bitmap_error(call: &str, icon_info: &ICONINFO, bitmap_info: &BITMAP) -> Error {
    Error::new(
        E_FAIL,
        format!(
            "{call} failed (ICONINFO fIcon={} hbmColor_null={} bitmap={}x{} bpp={})",
            icon_info.fIcon.as_bool(),
            icon_info.hbmColor.is_invalid(),
            bitmap_info.bmWidth,
            bitmap_info.bmHeight,
            bitmap_info.bmBitsPixel
        ),
    )
}

unsafe extern "system" fn zoom_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let state = create.lpCreateParams as *mut ZoomWindowState;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
    }
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ZoomWindowState };
    if !state.is_null() {
        let state = unsafe { &mut *state };
        match message {
            WM_SETCURSOR => {
                unsafe { SetCursor(Some(state.transparent_cursor)) };
                return LRESULT(1);
            }
            WM_MOUSEMOVE => {
                let mut point = POINT::default();
                if unsafe { GetCursorPos(&mut point) }.is_ok() {
                    let x = point.x - state.monitor_left;
                    let y = point.y - state.monitor_top;
                    let payload = pack_point(x, y);
                    let _ = unsafe {
                        PostMessageW(
                            Some(state.owner),
                            WM_APP_ZOOM_POINTER,
                            WPARAM(0),
                            LPARAM(payload),
                        )
                    };
                }
                return LRESULT(0);
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) as u16) as i16;
                let control = (wparam.0 & 0x0008) != 0;
                let encoded_delta = u16::from_ne_bytes(delta.to_ne_bytes()) as usize;
                let _ = unsafe {
                    PostMessageW(
                        Some(state.owner),
                        WM_APP_ZOOM_WHEEL,
                        WPARAM(encoded_delta),
                        LPARAM(isize::from(control)),
                    )
                };
                return LRESULT(0);
            }
            WM_POINTERDOWN
            | WM_POINTERUPDATE
            | WM_POINTERUP
            | WM_POINTERCAPTURECHANGED
            | WM_POINTERLEAVE => {
                let pointer_id = pointer_id_from_wparam(wparam.0);
                unsafe {
                    SendMessageW(
                        state.owner,
                        WM_APP_ZOOM_INK_INPUT,
                        Some(WPARAM(pointer_id as usize)),
                        Some(LPARAM(message as isize)),
                    )
                };
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == MESSAGE_TIMER_ID => {
                let _ = unsafe { KillTimer(Some(hwnd), MESSAGE_TIMER_ID) };
                let _ = unsafe {
                    PostMessageW(Some(state.owner), WM_APP_ZOOM_REDRAW, WPARAM(0), LPARAM(0))
                };
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

pub fn decode_pointer(lparam: LPARAM) -> Point {
    let raw = lparam.0 as u64;
    Point::new(
        (raw as u32 as i32) as f64,
        ((raw >> 32) as u32 as i32) as f64,
    )
}

pub fn decode_wheel(wparam: WPARAM) -> i32 {
    i32::from(i16::from_ne_bytes((wparam.0 as u16).to_ne_bytes()))
}

fn pack_point(x: i32, y: i32) -> isize {
    ((x as u32 as u64) | ((y as u32 as u64) << 32)) as isize
}

fn make_pointer_geometry(
    factory: &ID2D1Factory,
    presented: Point,
    scale: f32,
) -> Result<ID2D1PathGeometry> {
    let geometry = unsafe { factory.CreatePathGeometry() }?;
    let sink = unsafe { geometry.Open() }?;
    let base_points = [
        (1.8_f32, 22.08_f32),
        (1.8, 3.84),
        (7.02, 8.64),
        (10.44, 1.44),
        (13.68, 3.6),
        (10.26, 10.56),
        (16.38, 10.56),
    ];
    let points = base_points.map(|(x, y)| {
        let mut point = windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE::default().point;
        point.X = presented.x as f32 + (x - 1.8) * scale;
        point.Y = presented.y as f32 + (y - 22.08) * scale;
        point
    });
    unsafe {
        sink.BeginFigure(points[0], D2D1_FIGURE_BEGIN_FILLED);
        for candidate in points.iter().copied().skip(1) {
            sink.AddLine(candidate);
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(geometry)
}

fn preview_label_rect(source: Rect, width: u32, height: u32) -> D2D_RECT_F {
    let label_width = 76.0_f64;
    let label_height = 30.0_f64;
    let x = (source.x + source.width - label_width)
        .clamp(8.0, (f64::from(width) - label_width - 8.0).max(8.0));
    let y = (source.y + source.height + 6.0)
        .clamp(8.0, (f64::from(height) - label_height - 8.0).max(8.0));
    D2D_RECT_F {
        left: x as f32,
        top: y as f32,
        right: (x + label_width) as f32,
        bottom: (y + label_height) as f32,
    }
}

fn d2d_rect(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x as f32,
        top: rect.y as f32,
        right: (rect.x + rect.width) as f32,
        bottom: (rect.y + rect.height) as f32,
    }
}

fn create_test_bitmap(
    context: &ID2D1DeviceContext,
    width: u32,
    height: u32,
    target: bool,
    pixels: Option<&[u8]>,
) -> Result<ID2D1Bitmap1> {
    let properties = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: pixel_format(),
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: if target {
            D2D1_BITMAP_OPTIONS_TARGET
        } else {
            D2D1_BITMAP_OPTIONS_NONE
        },
        ..Default::default()
    };
    let pitch = width
        .checked_mul(4)
        .ok_or_else(|| Error::new(E_FAIL, "zoom selftest pitch overflow"))?;
    unsafe {
        context.CreateBitmap(
            D2D_SIZE_U { width, height },
            pixels.map(|pixels| pixels.as_ptr().cast()),
            if pixels.is_some() { pitch } else { 0 },
            &properties,
        )
    }
}

fn create_synthetic_ink_bitmap(
    context: &ID2D1DeviceContext,
    width: u32,
    height: u32,
    normalized_point: Point,
) -> Result<ID2D1Bitmap1> {
    let bitmap = create_test_bitmap(context, width, height, true, None)?;
    let image: ID2D1Image = bitmap.cast()?;
    unsafe {
        context.SetTarget(&image);
        context.BeginDraw();
        context.Clear(None);
    }
    let red = D2D1_COLOR_F {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let brush = unsafe { context.CreateSolidColorBrush(&red, None) }?;
    let ellipse = D2D1_ELLIPSE {
        point: D2D1_ELLIPSE::default().point,
        radiusX: 4.0,
        radiusY: 4.0,
    };
    let mut ellipse = ellipse;
    ellipse.point.X = (normalized_point.x * f64::from(width)) as f32;
    ellipse.point.Y = (normalized_point.y * f64::from(height)) as f32;
    unsafe {
        context.FillEllipse(&ellipse, &brush);
        context.EndDraw(None, None)?;
    }
    Ok(bitmap)
}

fn synthetic_grid(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&grid_color(x, y));
        }
    }
    pixels
}

fn grid_color(x: u32, y: u32) -> [u8; 4] {
    let column = (x / 40).min(8) as u8;
    let row = (y / 40).min(8) as u8;
    [
        20_u8.saturating_add(column.saturating_mul(20)),
        30_u8.saturating_add(row.saturating_mul(30)),
        200_u8.saturating_sub(column.saturating_mul(10)),
        255,
    ]
}

fn pixel(pixels: &[u8], pitch: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = y as usize * pitch as usize + x as usize * 4;
    [
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
        pixels[offset + 3],
    ]
}

fn bytes_of<T>(value: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}

fn valid_dimension(value: i32) -> Result<u32> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::new(E_FAIL, "zoom view dimension must be positive"))
}
