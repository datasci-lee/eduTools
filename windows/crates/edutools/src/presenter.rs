//! Presenter effects: portable animation math plus the Windows DComp overlay.

use edutools_core::keystroke::{KeystrokeDisplayFilter, Modifier, ModifierSet};
use edutools_core::shortcut::ShortcutModifiers;

pub const SPOTLIGHT_RADIUS_DEFAULT: f32 = 160.0;
pub const SPOTLIGHT_RADIUS_MIN: f32 = 60.0;
pub const SPOTLIGHT_RADIUS_MAX: f32 = 500.0;
pub const SPOTLIGHT_RADIUS_STEP: f32 = 16.0;
pub const SPOTLIGHT_ALPHA: f32 = 0.72;
pub const RIPPLE_DURATION_SECONDS: f64 = 0.65;
pub const KEYSTROKE_DURATION_SECONDS: f64 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RippleFrame {
    pub radius: f32,
    pub stroke_width: f32,
    pub alpha: f32,
}

pub fn adjust_spotlight_radius(current: f32, enabled: bool, steps: i32) -> (f32, bool) {
    if !enabled || steps == 0 {
        return (current, false);
    }
    (
        (current + steps as f32 * SPOTLIGHT_RADIUS_STEP)
            .clamp(SPOTLIGHT_RADIUS_MIN, SPOTLIGHT_RADIUS_MAX),
        true,
    )
}

pub fn ripple_frame(elapsed: f64) -> Option<RippleFrame> {
    if !(0.0..RIPPLE_DURATION_SECONDS).contains(&elapsed) {
        return None;
    }
    let progress = (elapsed / RIPPLE_DURATION_SECONDS) as f32;
    Some(RippleFrame {
        radius: 12.0 + progress * 38.0,
        stroke_width: 4.0 - progress * 2.0,
        alpha: 1.0 - progress,
    })
}

pub fn display_keystroke(modifiers: ShortcutModifiers, key_name: &str) -> Option<String> {
    let mut core = ModifierSet::empty();
    if modifiers.contains(ShortcutModifiers::CONTROL) {
        core.insert(Modifier::Control);
    }
    if modifiers.contains(ShortcutModifiers::ALT) {
        core.insert(Modifier::Alt);
    }
    if modifiers.contains(ShortcutModifiers::SHIFT) {
        core.insert(Modifier::Shift);
    }
    if modifiers.contains(ShortcutModifiers::WINDOWS) {
        core.insert(Modifier::Meta);
    }
    KeystrokeDisplayFilter::display_string(core, key_name)
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        adjust_spotlight_radius, display_keystroke, ripple_frame, KEYSTROKE_DURATION_SECONDS,
        SPOTLIGHT_ALPHA, SPOTLIGHT_RADIUS_DEFAULT,
    };
    use crate::kbhook::current_modifiers;
    use crate::keymap;
    use crate::log::Logger;
    use crate::monitor::{enumerate_monitors, monitor_at_point, Monitor};
    use crate::mousehook::{MouseButton, MouseHookEvent, MouseHookEventKind};
    use crate::render::{
        create_swap_chain, create_swap_target, pixel_format, read_bitmap_bgra, GraphicsDevice,
    };
    use crate::z_bands::{raise_band_order, Band, BandRegistration};
    use std::cell::Cell;
    use std::time::Instant;
    use windows::core::{w, Error, Interface, Result};
    use windows::Win32::Foundation::{
        E_FAIL, E_INVALIDARG, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
    };
    use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
    use windows::Win32::Graphics::Direct2D::{
        ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory, ID2D1Geometry, ID2D1Image,
        ID2D1RenderTarget, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
        D2D1_COMBINE_MODE_EXCLUDE, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
    };
    use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
    use windows::Win32::Graphics::Dxgi::DXGI_SWAP_CHAIN_FLAG;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, PostMessageW,
        RegisterClassW, SetWindowPos, SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx, HHOOK,
        KBDLLHOOKSTRUCT, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE,
        SW_SHOWNOACTIVATE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT, WS_POPUP,
    };

    pub const WM_APP_PRESENTER_KEY: u32 = 0x8000 + 301;
    pub const PRESENTER_FRAME_TIMER_ID: usize = 0xED41;
    pub const PRESENTER_FRAME_INTERVAL_MS: u32 = 16;
    const PRESENTER_CLASS: windows::core::PCWSTR = w!("EduToolsPresenterEffects");

    const SYSTEM_BLUE: D2D1_COLOR_F = D2D1_COLOR_F {
        r: 0.0,
        g: 0.478,
        b: 1.0,
        a: 1.0,
    };
    const SYSTEM_RED: D2D1_COLOR_F = D2D1_COLOR_F {
        r: 1.0,
        g: 0.231,
        b: 0.188,
        a: 1.0,
    };

    macro_rules! d2d_point {
        ($x:expr, $y:expr $(,)?) => {{
            let mut point = D2D1_ELLIPSE::default().point;
            point.X = $x;
            point.Y = $y;
            point
        }};
    }

    #[derive(Clone, Copy)]
    struct Ripple {
        center: POINT,
        started_at: Instant,
        color: D2D1_COLOR_F,
    }

    struct Scene<'a> {
        spotlight: bool,
        cursor_highlight: bool,
        radius: f32,
        pointer: (f32, f32),
        ripples: &'a [Ripple],
        keystroke: Option<&'a str>,
        monitor_origin: POINT,
        now: Instant,
    }

    pub struct PresenterEffects {
        owner: HWND,
        hinstance: HINSTANCE,
        logger: Logger,
        spotlight: bool,
        cursor_highlight: bool,
        keystroke_display: bool,
        radius: f32,
        pointer: POINT,
        window: Option<PresenterWindow>,
        ripples: Vec<Ripple>,
        keystroke: Option<(String, Instant)>,
        key_hook: Option<PresenterKeyboardHook>,
    }

    impl PresenterEffects {
        pub fn new(owner: HWND, hinstance: HINSTANCE, logger: Logger) -> Self {
            Self {
                owner,
                hinstance,
                logger,
                spotlight: false,
                cursor_highlight: false,
                keystroke_display: false,
                radius: SPOTLIGHT_RADIUS_DEFAULT,
                pointer: POINT::default(),
                window: None,
                ripples: Vec::new(),
                keystroke: None,
                key_hook: None,
            }
        }

        pub const fn spotlight_enabled(&self) -> bool {
            self.spotlight
        }

        pub const fn cursor_highlight_enabled(&self) -> bool {
            self.cursor_highlight
        }

        pub const fn spotlight_radius(&self) -> f32 {
            self.radius
        }

        pub fn toggle_spotlight(&mut self) -> bool {
            self.spotlight = !self.spotlight;
            let _ = unsafe { GetCursorPos(&mut self.pointer) };
            self.spotlight
        }

        pub fn toggle_cursor_highlight(&mut self) -> bool {
            self.cursor_highlight = !self.cursor_highlight;
            let _ = unsafe { GetCursorPos(&mut self.pointer) };
            self.cursor_highlight
        }

        pub fn toggle_keystroke_display(&mut self) -> Result<bool> {
            if self.keystroke_display {
                if let Some(mut hook) = self.key_hook.take() {
                    if let Err(error) = hook.uninstall() {
                        self.key_hook = Some(hook);
                        return Err(error);
                    }
                }
                self.keystroke_display = false;
                self.keystroke = None;
                return Ok(false);
            }
            let hook = PresenterKeyboardHook::install(self.owner)?;
            self.key_hook = Some(hook);
            self.keystroke_display = true;
            Ok(true)
        }

        pub fn adjust_radius(&mut self, steps: i32) -> bool {
            let (radius, applied) = adjust_spotlight_radius(self.radius, self.spotlight, steps);
            self.radius = radius;
            applied
        }

        pub fn handle_mouse_event(&mut self, event: MouseHookEvent) -> bool {
            self.pointer = POINT {
                x: event.x,
                y: event.y,
            };
            match event.kind {
                MouseHookEventKind::ButtonDown(MouseButton::Left) if self.cursor_highlight => {
                    self.ripples.push(Ripple {
                        center: self.pointer,
                        started_at: Instant::now(),
                        color: SYSTEM_BLUE,
                    });
                }
                MouseHookEventKind::ButtonDown(MouseButton::Right) if self.cursor_highlight => {
                    self.ripples.push(Ripple {
                        center: self.pointer,
                        started_at: Instant::now(),
                        color: SYSTEM_RED,
                    });
                }
                MouseHookEventKind::Wheel {
                    delta,
                    horizontal: false,
                } if self.spotlight && delta != 0 => {
                    return self.adjust_radius(if delta > 0 { 1 } else { -1 });
                }
                _ => {}
            }
            false
        }

        pub fn handle_key(&mut self, vk: u32) {
            if !self.keystroke_display {
                return;
            }
            let Some(key_name) = keymap::vk_to_token(vk) else {
                return;
            };
            let Some(display) = display_keystroke(current_modifiers(), key_name) else {
                return;
            };
            let _ = unsafe { GetCursorPos(&mut self.pointer) };
            self.keystroke = Some((display, Instant::now()));
        }

        pub fn render(&mut self) -> Result<()> {
            let now = Instant::now();
            self.ripples.retain(|ripple| {
                ripple_frame(now.duration_since(ripple.started_at).as_secs_f64()).is_some()
            });
            if self.keystroke.as_ref().is_some_and(|(_, shown)| {
                now.duration_since(*shown).as_secs_f64() >= KEYSTROKE_DURATION_SECONDS
            }) {
                self.keystroke = None;
            }
            let visible = self.spotlight
                || self.cursor_highlight
                || !self.ripples.is_empty()
                || self.keystroke.is_some();
            if !visible {
                if let Some(window) = self.window.as_ref() {
                    window.hide();
                }
                return Ok(());
            }

            let monitors = enumerate_monitors()?;
            let monitor = monitor_at_point(&monitors, self.pointer)
                .or_else(|| monitors.first())
                .ok_or_else(|| Error::new(E_FAIL, "발표 효과 대상 모니터가 없습니다"))?
                .clone();
            if self.window.is_none() {
                self.window = Some(PresenterWindow::new(
                    self.hinstance,
                    &monitor,
                    self.logger.clone(),
                )?);
            }
            let window = self.window.as_mut().expect("presenter window exists");
            window.set_monitor(&monitor)?;
            let local_pointer = (
                (self.pointer.x - monitor.rect.left) as f32,
                (self.pointer.y - monitor.rect.top) as f32,
            );
            window.render(&Scene {
                spotlight: self.spotlight,
                cursor_highlight: self.cursor_highlight,
                radius: self.radius,
                pointer: local_pointer,
                ripples: &self.ripples,
                keystroke: self.keystroke.as_ref().map(|(text, _)| text.as_str()),
                monitor_origin: POINT {
                    x: monitor.rect.left,
                    y: monitor.rect.top,
                },
                now,
            })?;
            window.show();
            Ok(())
        }

        pub fn needs_animation(&self) -> bool {
            !self.ripples.is_empty() || self.keystroke.is_some()
        }

        pub fn shutdown(&mut self) -> Result<()> {
            if let Some(mut hook) = self.key_hook.take() {
                hook.uninstall()?;
            }
            self.window = None;
            Ok(())
        }
    }

    struct PresenterWindow {
        hwnd: HWND,
        monitor: Monitor,
        renderer: PresenterRenderer,
        _graphics: GraphicsDevice,
        _band: BandRegistration,
        logger: Logger,
    }

    impl PresenterWindow {
        fn new(hinstance: HINSTANCE, monitor: &Monitor, logger: Logger) -> Result<Self> {
            register_window_class(hinstance)?;
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST
                        | WS_EX_TOOLWINDOW
                        | WS_EX_NOACTIVATE
                        | WS_EX_TRANSPARENT
                        | WS_EX_LAYERED
                        | WS_EX_NOREDIRECTIONBITMAP,
                    PRESENTER_CLASS,
                    w!("EduTools Presenter Effects"),
                    WS_POPUP,
                    monitor.rect.left,
                    monitor.rect.top,
                    monitor.rect.width(),
                    monitor.rect.height(),
                    None,
                    None,
                    Some(hinstance),
                    None,
                )
            }?;
            let graphics = GraphicsDevice::new()?;
            let renderer = match PresenterRenderer::new(
                &graphics,
                hwnd,
                monitor.rect.width(),
                monitor.rect.height(),
            ) {
                Ok(renderer) => renderer,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            let band = BandRegistration::new(hwnd, Band::Effect);
            logger.event(&format!(
                "발표 효과 창 생성: monitor={} rect=({},{} {}x{}) captureIncluded=true",
                monitor.session_key,
                monitor.rect.left,
                monitor.rect.top,
                monitor.rect.width(),
                monitor.rect.height()
            ));
            Ok(Self {
                hwnd,
                monitor: monitor.clone(),
                renderer,
                _graphics: graphics,
                _band: band,
                logger,
            })
        }

        fn set_monitor(&mut self, monitor: &Monitor) -> Result<()> {
            if self.monitor.rect == monitor.rect && self.monitor.session_key == monitor.session_key
            {
                return Ok(());
            }
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    monitor.rect.left,
                    monitor.rect.top,
                    monitor.rect.width(),
                    monitor.rect.height(),
                    SWP_NOACTIVATE,
                )
            }?;
            self.renderer
                .resize(monitor.rect.width(), monitor.rect.height())?;
            self.logger.event(&format!(
                "발표 효과 창 모니터 변경: {} -> {} rect=({},{} {}x{})",
                self.monitor.session_key,
                monitor.session_key,
                monitor.rect.left,
                monitor.rect.top,
                monitor.rect.width(),
                monitor.rect.height()
            ));
            self.monitor = monitor.clone();
            Ok(())
        }

        fn render(&mut self, scene: &Scene<'_>) -> Result<()> {
            self.renderer.render(scene)
        }

        fn show(&self) {
            let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };
            let _ = unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                )
            };
            if let Err(error) = raise_band_order() {
                self.logger
                    .event(&format!("발표 효과 z-band 정렬 실패: {error}"));
            }
        }

        fn hide(&self) {
            let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }
    }

    impl Drop for PresenterWindow {
        fn drop(&mut self) {
            self.renderer.release_target();
            if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
                self.logger
                    .event(&format!("발표 효과 창 제거 실패: {error}"));
            }
        }
    }

    struct PresenterRenderer {
        swap_chain: windows::Win32::Graphics::Dxgi::IDXGISwapChain1,
        context: ID2D1DeviceContext,
        factory: ID2D1Factory,
        text_format: IDWriteTextFormat,
        swap_target: Option<ID2D1Bitmap1>,
        _dcomp_target: IDCompositionTarget,
        _visual: IDCompositionVisual,
        width: u32,
        height: u32,
    }

    impl PresenterRenderer {
        fn new(graphics: &GraphicsDevice, hwnd: HWND, width: i32, height: i32) -> Result<Self> {
            let (width, height) = valid_size(width, height)?;
            let swap_chain = create_swap_chain(graphics, width, height)?;
            let context = graphics.create_d2d_context()?;
            unsafe { context.SetDpi(96.0, 96.0) };
            let factory = unsafe { context.GetFactory() }?;
            let swap_target = Some(create_swap_target(&context, &swap_chain)?);
            let image: ID2D1Image = swap_target.as_ref().expect("swap target").cast()?;
            unsafe { context.SetTarget(&image) };
            let (dcomp_target, visual) =
                graphics.attach_composition_swap_chain(hwnd, &swap_chain)?;
            let text_format = create_text_format()?;
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

        fn resize(&mut self, width: i32, height: i32) -> Result<()> {
            let (width, height) = valid_size(width, height)?;
            if self.width == width && self.height == height {
                return Ok(());
            }
            self.release_target();
            unsafe {
                self.swap_chain.ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?
            };
            self.swap_target = Some(create_swap_target(&self.context, &self.swap_chain)?);
            let image: ID2D1Image = self.swap_target.as_ref().expect("swap target").cast()?;
            unsafe { self.context.SetTarget(&image) };
            self.width = width;
            self.height = height;
            Ok(())
        }

        fn render(&mut self, scene: &Scene<'_>) -> Result<()> {
            draw_scene(
                &self.context,
                &self.factory,
                &self.text_format,
                self.width,
                self.height,
                scene,
            )?;
            unsafe { self.swap_chain.Present(1, Default::default()).ok()? };
            Ok(())
        }

        fn release_target(&mut self) {
            unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
            self.swap_target = None;
        }
    }

    fn draw_scene(
        context: &ID2D1DeviceContext,
        factory: &ID2D1Factory,
        text_format: &IDWriteTextFormat,
        width: u32,
        height: u32,
        scene: &Scene<'_>,
    ) -> Result<()> {
        let result = (|| -> Result<()> {
            unsafe {
                context.BeginDraw();
                context.Clear(None);
            }
            if scene.spotlight {
                draw_spotlight(context, factory, width, height, scene.pointer, scene.radius)?;
            }
            if scene.cursor_highlight {
                let point = d2d_point!(scene.pointer.0, scene.pointer.1);
                let ellipse = D2D1_ELLIPSE {
                    point,
                    radiusX: 18.0,
                    radiusY: 18.0,
                };
                let fill = unsafe {
                    context.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 1.0,
                            g: 0.8,
                            b: 0.0,
                            a: 0.3,
                        },
                        None,
                    )
                }?;
                let stroke = unsafe {
                    context.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 1.0,
                            g: 0.5,
                            b: 0.0,
                            a: 0.9,
                        },
                        None,
                    )
                }?;
                unsafe {
                    context.FillEllipse(&ellipse, &fill);
                    context.DrawEllipse(&ellipse, &stroke, 2.0, None);
                }
            }
            for ripple in scene.ripples {
                let elapsed = scene.now.duration_since(ripple.started_at).as_secs_f64();
                let Some(frame) = ripple_frame(elapsed) else {
                    continue;
                };
                let brush = unsafe {
                    context.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            a: frame.alpha,
                            ..ripple.color
                        },
                        None,
                    )
                }?;
                let center = d2d_point!(
                    (ripple.center.x - scene.monitor_origin.x) as f32,
                    (ripple.center.y - scene.monitor_origin.y) as f32,
                );
                unsafe {
                    context.DrawEllipse(
                        &D2D1_ELLIPSE {
                            point: center,
                            radiusX: frame.radius,
                            radiusY: frame.radius,
                        },
                        &brush,
                        frame.stroke_width,
                        None,
                    )
                };
            }
            if let Some(text) = scene.keystroke {
                draw_keystroke(context, text_format, width, height, text)?;
            }
            unsafe { context.EndDraw(None, None)? };
            Ok(())
        })();
        if result.is_err() {
            let _ = unsafe { context.EndDraw(None, None) };
        }
        result
    }

    fn draw_spotlight(
        context: &ID2D1DeviceContext,
        factory: &ID2D1Factory,
        width: u32,
        height: u32,
        pointer: (f32, f32),
        radius: f32,
    ) -> Result<()> {
        let full = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: width as f32,
            bottom: height as f32,
        };
        let rectangle = unsafe { factory.CreateRectangleGeometry(&full) }?;
        let hole = unsafe {
            factory.CreateEllipseGeometry(&D2D1_ELLIPSE {
                point: d2d_point!(pointer.0, pointer.1),
                radiusX: radius,
                radiusY: radius,
            })
        }?;
        let path = unsafe { factory.CreatePathGeometry() }?;
        let sink = unsafe { path.Open() }?;
        let rectangle_geometry: ID2D1Geometry = rectangle.cast()?;
        let hole_geometry: ID2D1Geometry = hole.cast()?;
        unsafe {
            rectangle_geometry.CombineWithGeometry(
                &hole_geometry,
                D2D1_COMBINE_MODE_EXCLUDE,
                None,
                0.25,
                &sink,
            )?;
            sink.Close()?;
        }
        let brush = unsafe {
            context.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: SPOTLIGHT_ALPHA,
                },
                None,
            )
        }?;
        unsafe { context.FillGeometry(&path, &brush, None) };
        Ok(())
    }

    fn draw_keystroke(
        context: &ID2D1DeviceContext,
        text_format: &IDWriteTextFormat,
        width: u32,
        height: u32,
        value: &str,
    ) -> Result<()> {
        let text_width = (value.chars().count() as f32 * 18.0 + 44.0).max(88.0);
        let rect = D2D_RECT_F {
            left: (width as f32 - text_width) / 2.0,
            top: height as f32 - 34.0 - 54.0,
            right: (width as f32 + text_width) / 2.0,
            bottom: height as f32 - 34.0,
        };
        let background = unsafe {
            context.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.78,
                },
                None,
            )
        }?;
        let white = unsafe {
            context.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                None,
            )
        }?;
        unsafe {
            context.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: 12.0,
                    radiusY: 12.0,
                },
                &background,
            );
        }
        let target: ID2D1RenderTarget = context.cast()?;
        let utf16 = value.encode_utf16().collect::<Vec<_>>();
        unsafe {
            target.DrawText(
                &utf16,
                text_format,
                &rect,
                &white,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    fn create_text_format() -> Result<IDWriteTextFormat> {
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let format = unsafe {
            dwrite.CreateTextFormat(
                w!("Consolas"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                28.0,
                w!("ko-KR"),
            )
        }?;
        unsafe {
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        Ok(format)
    }

    fn valid_size(width: i32, height: i32) -> Result<(u32, u32)> {
        let width = u32::try_from(width).ok().filter(|value| *value > 0);
        let height = u32::try_from(height).ok().filter(|value| *value > 0);
        width
            .zip(height)
            .ok_or_else(|| Error::new(E_INVALIDARG, "발표 효과 창 크기는 양수여야 합니다"))
    }

    fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
        let class = WNDCLASSW {
            lpfnWndProc: Some(presenter_window_proc),
            hInstance: hinstance,
            lpszClassName: PRESENTER_CLASS,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            let error = unsafe { windows::Win32::Foundation::GetLastError() };
            if error != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err(Error::from_thread());
            }
        }
        Ok(())
    }

    unsafe extern "system" fn presenter_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    thread_local! {
        static KEY_HOOK_OWNER: Cell<Option<HWND>> = const { Cell::new(None) };
    }

    struct PresenterKeyboardHook {
        handle: Option<HHOOK>,
    }

    impl PresenterKeyboardHook {
        fn install(owner: HWND) -> Result<Self> {
            if KEY_HOOK_OWNER.with(Cell::get).is_some() {
                return Err(Error::new(
                    E_FAIL,
                    "발표 키 관찰 훅이 이미 설치되어 있습니다",
                ));
            }
            let module = unsafe { GetModuleHandleW(None) }?;
            KEY_HOOK_OWNER.with(|slot| slot.set(Some(owner)));
            let handle = unsafe {
                SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(presenter_keyboard_hook_proc),
                    Some(HINSTANCE(module.0)),
                    0,
                )
            };
            match handle {
                Ok(handle) => Ok(Self {
                    handle: Some(handle),
                }),
                Err(error) => {
                    KEY_HOOK_OWNER.with(|slot| slot.set(None));
                    Err(Error::new(
                        error.code(),
                        format!("키 표시 WH_KEYBOARD_LL 설치 실패: {error}"),
                    ))
                }
            }
        }

        fn uninstall(&mut self) -> Result<()> {
            let Some(handle) = self.handle else {
                KEY_HOOK_OWNER.with(|slot| slot.set(None));
                return Ok(());
            };
            unsafe { UnhookWindowsHookEx(handle) }?;
            self.handle = None;
            KEY_HOOK_OWNER.with(|slot| slot.set(None));
            Ok(())
        }
    }

    unsafe extern "system" fn presenter_keyboard_hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 && matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
            // SAFETY: WH_KEYBOARD_LL supplies KBDLLHOOKSTRUCT for HC_ACTION callbacks.
            let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            KEY_HOOK_OWNER.with(|slot| {
                if let Some(owner) = slot.get() {
                    let _ = unsafe {
                        PostMessageW(
                            Some(owner),
                            WM_APP_PRESENTER_KEY,
                            WPARAM(event.vkCode as usize),
                            LPARAM(0),
                        )
                    };
                }
            });
        }
        // Observation only: never consume or alter keyboard input.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    pub fn selftest_failures(_hinstance: HINSTANCE, _logger: Logger) -> Vec<String> {
        let result = (|| -> Result<Vec<String>> {
            let graphics = GraphicsDevice::new()?;
            let context = graphics.create_d2d_context()?;
            unsafe { context.SetDpi(96.0, 96.0) };
            let factory = unsafe { context.GetFactory() }?;
            let format = create_text_format()?;
            let width = 320;
            let height = 200;
            let properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: pixel_format(),
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let target = unsafe {
                context.CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &properties)
            }?;
            let image: ID2D1Image = target.cast()?;
            unsafe { context.SetTarget(&image) };
            let now = Instant::now();
            let ripple = Ripple {
                center: POINT { x: 80, y: 80 },
                started_at: now,
                color: SYSTEM_BLUE,
            };
            draw_scene(
                &context,
                &factory,
                &format,
                width,
                height,
                &Scene {
                    spotlight: true,
                    cursor_highlight: false,
                    radius: 60.0,
                    pointer: (220.0, 70.0),
                    ripples: &[ripple],
                    keystroke: None,
                    monitor_origin: POINT::default(),
                    now,
                },
            )?;
            let (pixels, pitch) = read_bitmap_bgra(&context, &target)?;
            let pixel = |x: u32, y: u32| -> [u8; 4] {
                let offset = y as usize * pitch as usize + x as usize * 4;
                [
                    pixels[offset],
                    pixels[offset + 1],
                    pixels[offset + 2],
                    pixels[offset + 3],
                ]
            };
            let mut failures = Vec::new();
            if pixel(220, 70)[3] != 0 {
                failures.push("presenter spotlight: 구멍 중심 알파가 0이 아님".to_owned());
            }
            if pixel(10, 10)[3] == 0 {
                failures.push("presenter spotlight: 구멍 밖 알파가 0임".to_owned());
            }
            if pixel(92, 80)[3] == 0 {
                failures.push("presenter ripple: 링 위치 알파가 0임".to_owned());
            }

            draw_scene(
                &context,
                &factory,
                &format,
                width,
                height,
                &Scene {
                    spotlight: false,
                    cursor_highlight: false,
                    radius: 60.0,
                    pointer: (0.0, 0.0),
                    ripples: &[],
                    keystroke: Some("Ctrl+A"),
                    monitor_origin: POINT::default(),
                    now,
                },
            )?;
            let (pixels, pitch) = read_bitmap_bgra(&context, &target)?;
            let text_pixels = (112..166)
                .flat_map(|y| (80..240).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let offset = *y as usize * pitch as usize + *x as usize * 4;
                    pixels[offset + 3] > 220 && pixels[offset + 2] > 180
                })
                .count();
            if text_pixels == 0 {
                failures.push("presenter keystroke: 표시 영역에 글자 픽셀이 없음".to_owned());
            }
            unsafe { context.SetTarget(None::<&ID2D1Image>) };
            Ok(failures)
        })();
        result.unwrap_or_else(|error| vec![format!("presenter: 오프스크린 렌더 실패: {error}")])
    }
}

#[cfg(windows)]
pub use windows_impl::*;

#[cfg(test)]
mod tests {
    use super::{
        adjust_spotlight_radius, display_keystroke, ripple_frame, RippleFrame,
        KEYSTROKE_DURATION_SECONDS, RIPPLE_DURATION_SECONDS, SPOTLIGHT_ALPHA,
        SPOTLIGHT_RADIUS_DEFAULT,
    };
    use edutools_core::shortcut::ShortcutModifiers;

    #[test]
    fn spotlight_radius_clamp_table() {
        assert_eq!(SPOTLIGHT_RADIUS_DEFAULT, 160.0);
        assert_eq!(SPOTLIGHT_ALPHA, 0.72);
        let cases = [
            (160.0, false, 1, 160.0, false),
            (160.0, true, 0, 160.0, false),
            (160.0, true, 1, 176.0, true),
            (61.0, true, -1, 60.0, true),
            (499.0, true, 1, 500.0, true),
        ];
        for (current, enabled, steps, expected, applied) in cases {
            assert_eq!(
                adjust_spotlight_radius(current, enabled, steps),
                (expected, applied)
            );
        }
    }

    #[test]
    fn ripple_progress_table() {
        let cases = [
            (
                0.0,
                Some(RippleFrame {
                    radius: 12.0,
                    stroke_width: 4.0,
                    alpha: 1.0,
                }),
            ),
            (
                0.325,
                Some(RippleFrame {
                    radius: 31.0,
                    stroke_width: 3.0,
                    alpha: 0.5,
                }),
            ),
            (RIPPLE_DURATION_SECONDS, None),
        ];
        for (elapsed, expected) in cases {
            assert_eq!(ripple_frame(elapsed), expected, "elapsed={elapsed}");
        }
    }

    #[test]
    fn keystroke_normalization_table() {
        assert_eq!(KEYSTROKE_DURATION_SECONDS, 1.5);
        let cases = [
            (ShortcutModifiers::empty(), "A", None),
            (ShortcutModifiers::CONTROL, "A", Some("Ctrl+A".to_owned())),
            (ShortcutModifiers::CONTROL, "", None),
            (ShortcutModifiers::SHIFT, "A", None),
        ];
        for (modifiers, key, expected) in cases {
            assert_eq!(display_keystroke(modifiers, key), expected);
        }
    }
}
