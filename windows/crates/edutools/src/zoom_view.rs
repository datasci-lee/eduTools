use crate::capture::CapturedFrame;
use crate::log::Logger;
use crate::monitor::{Monitor, MonitorRect};
use crate::render::{
    create_swap_chain, create_swap_target, pixel_format, read_bitmap_bgra, GraphicsDevice,
};
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
    D2D1_BRIGHTNESS_PROP_WHITE_POINT, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1_PROPERTY_TYPE_FLOAT, D2D1_PROPERTY_TYPE_VECTOR2, D2D1_ROUNDED_RECT,
    D2D1_SATURATION_PROP_SATURATION,
};
use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::{IDXGISurface, IDXGISwapChain1};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateCursor, CreateWindowExW, DefWindowProcW, DestroyCursor, DestroyWindow, GetCursorPos,
    GetWindowLongPtrW, KillTimer, PostMessageW, RegisterClassW, SetCursor, SetForegroundWindow,
    SetTimer, SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW,
    GWLP_USERDATA, HCURSOR, SWP_SHOWWINDOW, SW_SHOW, WDA_EXCLUDEFROMCAPTURE, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_SETCURSOR, WM_TIMER, WNDCLASSW,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

pub const WM_APP_ZOOM_POINTER: u32 = 0x8000 + 142;
pub const WM_APP_ZOOM_WHEEL: u32 = 0x8000 + 143;
pub const WM_APP_ZOOM_REDRAW: u32 = 0x8000 + 145;
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
        if let Err(error) = unsafe { DestroyCursor(self.transparent_cursor) } {
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
        let mut renderer =
            match ZoomRenderer::new(graphics, hwnd, monitor.rect.width(), monitor.rect.height()) {
                Ok(renderer) => renderer,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
        let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
        let focused = unsafe { SetForegroundWindow(hwnd) }.as_bool();
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

    pub fn render(&mut self, session: &ZoomSessionState) -> Result<()> {
        if self
            .message
            .as_ref()
            .is_some_and(|(_, shown)| shown.elapsed() >= Duration::from_millis(1_300))
        {
            self.message = None;
        }
        self.renderer.render(
            self.current_frame.as_ref(),
            session,
            self.pointer,
            self.message.as_ref().map(|(message, _)| message.as_str()),
        )
    }

    pub fn focus(&self) -> bool {
        unsafe { SetForegroundWindow(self.hwnd) }.as_bool()
    }
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
}

impl ZoomRenderer {
    fn new(graphics: &GraphicsDevice, hwnd: HWND, width: i32, height: i32) -> Result<Self> {
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
        })
    }

    fn render(
        &self,
        frame: Option<&CapturedFrame>,
        session: &ZoomSessionState,
        pointer: Point,
        message: Option<&str>,
    ) -> Result<()> {
        let bitmap = frame.map(|frame| self.capture_bitmap(frame)).transpose()?;
        self.draw_scene(bitmap.as_ref(), session, pointer, message)?;
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
        &self,
        bitmap: Option<&ID2D1Bitmap1>,
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

    fn draw_pointer(&self, session: &ZoomSessionState, pointer: Point) -> Result<()> {
        let presented = ZoomGeometry::pointer_presentation_point(pointer, session);
        let scale = ZoomGeometry::pointer_scale(session) as f32;
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

    fn selftest_failures(&self) -> Result<Vec<String>> {
        let result = self.selftest_failures_inner();
        if let Some(target) = self.swap_target.as_ref() {
            let image: ID2D1Image = target.cast()?;
            unsafe { self.context.SetTarget(&image) };
        }
        result
    }

    fn selftest_failures_inner(&self) -> Result<Vec<String>> {
        let width = self.width;
        let height = self.height;
        let target = create_test_bitmap(&self.context, width, height, true, None)?;
        let source_pixels = synthetic_grid(width, height);
        let source = create_test_bitmap(&self.context, width, height, false, Some(&source_pixels))?;
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
        self.draw_scene(Some(&source), &session, Point::new(300.0, 180.0), None)?;
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

        unsafe {
            self.context.BeginDraw();
            self.context.Clear(None);
        }
        self.draw_pointer(&session, Point::new(160.0, 100.0))?;
        unsafe { self.context.EndDraw(None, None)? };
        let (pointer_pixels, pointer_pitch) = read_bitmap_bgra(&self.context, &target)?;
        let pointer_alpha = pointer_pixels
            .chunks(pointer_pitch as usize)
            .flat_map(|row| row[..width as usize * 4].chunks_exact(4))
            .filter(|pixel| pixel[3] != 0)
            .count();
        if pointer_alpha == 0 {
            failures.push("zoom pointer: 화살표 영역 alpha가 모두 0".to_owned());
        }

        session.is_previewing = true;
        self.draw_scene(Some(&source), &session, Point::new(300.0, 180.0), None)?;
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
        Ok(view) => view
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
