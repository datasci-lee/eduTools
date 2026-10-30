use crate::log::Logger;
use edutools_core::ink::{InkGeometry, InkStroke, PathCommand, Size};
use windows::core::{Error, Interface, Result};
use windows::Win32::Foundation::{HMODULE, HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
    D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateDevice, ID2D1Bitmap1, ID2D1Device, ID2D1DeviceContext, ID2D1Factory, ID2D1Image,
    ID2D1StrokeStyle, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1_LINE_JOIN_ROUND, D2D1_QUADRATIC_BEZIER_SEGMENT, D2D1_STROKE_STYLE_PROPERTIES,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGIOutput, IDXGISurface, IDXGISwapChain1, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

const TARGET_DPI: f32 = 96.0;

macro_rules! d2d_point {
    ($point:expr) => {{
        let point = $point;
        let mut value = D2D1_ELLIPSE::default().point;
        value.X = point.x as f32;
        value.Y = point.y as f32;
        value
    }};
}

pub struct GraphicsDevice {
    d3d_device: ID3D11Device,
    d2d_device: ID2D1Device,
    dxgi_factory: IDXGIFactory2,
    dcomp_device: IDCompositionDevice,
}

impl GraphicsDevice {
    pub fn new() -> Result<Self> {
        let mut d3d_device = None;
        // SAFETY: outputs point to initialized Options and the hardware driver needs no adapter.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                None,
                None,
            )
        }?;
        let d3d_device = d3d_device.ok_or_else(Error::from_thread)?;
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;
        // SAFETY: dxgi_device is the D3D11 device's DXGI interface.
        let adapter = unsafe { dxgi_device.GetAdapter() }?;
        let dxgi_factory: IDXGIFactory2 = unsafe { adapter.GetParent() }?;
        // SAFETY: the DXGI device supports BGRA and remains alive in this object.
        let d2d_device = unsafe { D2D1CreateDevice(&dxgi_device, None) }?;
        let dcomp_device: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi_device) }?;
        Ok(Self {
            d3d_device,
            d2d_device,
            dxgi_factory,
            dcomp_device,
        })
    }
}

pub struct OverlayRenderer {
    hwnd: HWND,
    swap_chain: IDXGISwapChain1,
    context: ID2D1DeviceContext,
    factory: ID2D1Factory,
    round_stroke: ID2D1StrokeStyle,
    swap_target: Option<ID2D1Bitmap1>,
    completed_bitmap: Option<ID2D1Bitmap1>,
    _dcomp_target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    width: u32,
    height: u32,
    monitor_key: String,
    logger: Logger,
}

impl OverlayRenderer {
    pub fn new(
        graphics: &GraphicsDevice,
        hwnd: HWND,
        width: i32,
        height: i32,
        monitor_key: String,
        logger: Logger,
    ) -> Result<Self> {
        let width = valid_dimension(width)?;
        let height = valid_dimension(height)?;
        let swap_chain = create_swap_chain(graphics, width, height)?;
        // SAFETY: the D2D device is live and all access is confined to the UI thread.
        let context = unsafe {
            graphics
                .d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
        }?;
        unsafe { context.SetDpi(TARGET_DPI, TARGET_DPI) };
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
        let completed_bitmap = Some(create_completed_bitmap(&context, width, height)?);
        let swap_image: ID2D1Image = swap_target
            .as_ref()
            .expect("swap target was just created")
            .cast()?;
        unsafe { context.SetTarget(&swap_image) };

        // The progressive layer is drawn last into this swap-chain visual. The completed layer is
        // a separate D2D bitmap, so resize/rebuild can redraw it without changing input state.
        let dcomp_target = unsafe { graphics.dcomp_device.CreateTargetForHwnd(hwnd, true) }?;
        let visual = unsafe { graphics.dcomp_device.CreateVisual() }?;
        unsafe {
            visual.SetContent(&swap_chain)?;
            dcomp_target.SetRoot(&visual)?;
            graphics.dcomp_device.Commit()?;
        }
        let renderer = Self {
            hwnd,
            swap_chain,
            context,
            factory,
            round_stroke,
            swap_target,
            completed_bitmap,
            _dcomp_target: dcomp_target,
            _visual: visual,
            width,
            height,
            monitor_key,
            logger,
        };
        renderer.clear_completed()?;
        renderer.log_size()?;
        Ok(renderer)
    }

    pub fn resize(
        &mut self,
        width: i32,
        height: i32,
        strokes: &[InkStroke],
        minimum_ratio: f64,
    ) -> Result<()> {
        let width = valid_dimension(width)?;
        let height = valid_dimension(height)?;
        unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
        self.swap_target = None;
        self.completed_bitmap = None;
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
        self.completed_bitmap = Some(create_completed_bitmap(&self.context, width, height)?);
        let swap_image: ID2D1Image = self
            .swap_target
            .as_ref()
            .expect("swap target recreated")
            .cast()?;
        unsafe { self.context.SetTarget(&swap_image) };
        self.width = width;
        self.height = height;
        self.rebuild_completed(strokes, minimum_ratio)?;
        self.log_size()?;
        Ok(())
    }

    pub fn swap_chain_size(&self) -> Result<(u32, u32)> {
        let description = unsafe { self.swap_chain.GetDesc1() }?;
        Ok((description.Width, description.Height))
    }

    pub fn append_completed(&self, stroke: &InkStroke, minimum_ratio: f64) -> Result<()> {
        self.begin_completed_draw();
        let draw_result = self.draw_stroke(stroke, minimum_ratio);
        let end_result = unsafe { self.context.EndDraw(None, None) };
        self.restore_swap_target()?;
        draw_result?;
        end_result
    }

    pub fn rebuild_completed(&self, strokes: &[InkStroke], minimum_ratio: f64) -> Result<()> {
        self.begin_completed_draw();
        unsafe { self.context.Clear(Some(&D2D1_COLOR_F::default())) };
        let draw_result: Result<()> = (|| {
            for stroke in strokes {
                self.draw_stroke(stroke, minimum_ratio)?;
            }
            Ok(())
        })();
        let end_result = unsafe { self.context.EndDraw(None, None) };
        self.restore_swap_target()?;
        draw_result?;
        end_result
    }

    pub fn draw_frame(
        &self,
        active_stroke: Option<&InkStroke>,
        minimum_ratio: f64,
        debug_visible: bool,
    ) -> Result<()> {
        self.restore_swap_target()?;
        let clear = if debug_visible {
            D2D1_COLOR_F {
                r: 0.12,
                g: 0.35,
                b: 0.95,
                a: 0.06,
            }
        } else {
            D2D1_COLOR_F::default()
        };
        let debug_brush = if debug_visible {
            let orange = D2D1_COLOR_F {
                r: 1.0,
                g: 0.43,
                b: 0.08,
                a: 1.0,
            };
            Some(unsafe { self.context.CreateSolidColorBrush(&orange, None) }?)
        } else {
            None
        };
        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&clear));
            self.context.DrawBitmap(
                self.completed_bitmap
                    .as_ref()
                    .expect("completed bitmap exists outside resize"),
                None,
                1.0,
                D2D1_INTERPOLATION_MODE_LINEAR,
                None,
                None,
            );
        }
        let draw_result = if let Some(stroke) = active_stroke {
            self.draw_stroke(stroke, minimum_ratio)
        } else {
            Ok(())
        };
        if let Some(brush) = debug_brush {
            let stroke = 4.0;
            let inset = stroke / 2.0;
            let border = D2D_RECT_F {
                left: inset,
                top: inset,
                right: self.width as f32 - inset,
                bottom: self.height as f32 - inset,
            };
            unsafe {
                self.context
                    .DrawRectangle(&border, &brush, stroke, None::<&ID2D1StrokeStyle>);
            }
        }
        let end_result = unsafe { self.context.EndDraw(None, None) };
        draw_result?;
        end_result?;
        unsafe { self.swap_chain.Present(1, Default::default()).ok() }
    }

    fn draw_stroke(&self, stroke: &InkStroke, minimum_ratio: f64) -> Result<()> {
        let size = Size {
            width: f64::from(self.width),
            height: f64::from(self.height),
        };
        let samples = InkGeometry::smoothed_points(&stroke.points)
            .into_iter()
            .map(|point| {
                (
                    InkGeometry::screen_point(point.location, size),
                    point.pressure,
                )
            })
            .collect::<Vec<_>>();
        let Some(first) = samples.first().copied() else {
            return Ok(());
        };
        let color = D2D1_COLOR_F {
            r: stroke.color[0] as f32,
            g: stroke.color[1] as f32,
            b: stroke.color[2] as f32,
            a: 1.0,
        };
        let brush = unsafe { self.context.CreateSolidColorBrush(&color, None) }?;
        if samples.len() == 1 {
            let radius = (stroke.effective_width(first.1, minimum_ratio) / 2.0) as f32;
            let ellipse = D2D1_ELLIPSE {
                point: d2d_point!(first.0),
                radiusX: radius,
                radiusY: radius,
            };
            unsafe { self.context.FillEllipse(&ellipse, &brush) };
            return Ok(());
        }
        if !stroke.uses_pressure {
            let geometry = unsafe { self.factory.CreatePathGeometry() }?;
            let sink = unsafe { geometry.Open() }?;
            unsafe {
                sink.BeginFigure(d2d_point!(first.0), D2D1_FIGURE_BEGIN_HOLLOW);
                for sample in samples.iter().skip(1) {
                    sink.AddLine(d2d_point!(sample.0));
                }
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                sink.Close()?;
                self.context.DrawGeometry(
                    &geometry,
                    &brush,
                    stroke.width as f32,
                    &self.round_stroke,
                );
            }
            return Ok(());
        }

        let commands = InkGeometry::variable_width_path(stroke, &samples, minimum_ratio);
        let geometry = unsafe { self.factory.CreatePathGeometry() }?;
        let sink = unsafe { geometry.Open() }?;
        let mut figure_open = false;
        for command in commands {
            match command {
                PathCommand::MoveTo(point) => {
                    if figure_open {
                        unsafe { sink.EndFigure(D2D1_FIGURE_END_CLOSED) };
                    }
                    unsafe { sink.BeginFigure(d2d_point!(point), D2D1_FIGURE_BEGIN_FILLED) };
                    figure_open = true;
                }
                PathCommand::LineTo(point) => unsafe { sink.AddLine(d2d_point!(point)) },
                PathCommand::QuadTo { control, to } => unsafe {
                    sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                        point1: d2d_point!(control),
                        point2: d2d_point!(to),
                    })
                },
            }
        }
        if figure_open {
            unsafe { sink.EndFigure(D2D1_FIGURE_END_CLOSED) };
        }
        unsafe {
            sink.Close()?;
            self.context.FillGeometry(
                &geometry,
                &brush,
                None::<&windows::Win32::Graphics::Direct2D::ID2D1Brush>,
            );
        }
        Ok(())
    }

    fn clear_completed(&self) -> Result<()> {
        self.rebuild_completed(&[], 0.2)
    }

    fn begin_completed_draw(&self) {
        let image: ID2D1Image = self
            .completed_bitmap
            .as_ref()
            .expect("completed bitmap exists outside resize")
            .cast()
            .expect("ID2D1Bitmap1 implements ID2D1Image");
        unsafe {
            self.context.SetTarget(&image);
            self.context.BeginDraw();
        }
    }

    fn restore_swap_target(&self) -> Result<()> {
        let image: ID2D1Image = self
            .swap_target
            .as_ref()
            .expect("swap target exists outside resize")
            .cast()?;
        unsafe { self.context.SetTarget(&image) };
        Ok(())
    }

    fn log_size(&self) -> Result<()> {
        let (swap_width, swap_height) = self.swap_chain_size()?;
        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut window_rect) }?;
        let target = self
            .swap_target
            .as_ref()
            .expect("swap target is attached outside resize");
        let mut target_dpi_x = 0.0;
        let mut target_dpi_y = 0.0;
        unsafe { target.GetDpi(&mut target_dpi_x, &mut target_dpi_y) };
        let target_size = unsafe { target.GetSize() };
        debug_assert_eq!(target_dpi_x, target_dpi_y);
        self.logger.event(&format!(
            "오버레이 크기: {}, window={}x{}(물리), swapchain={}x{}, target dpi={:.0}, D2D 그리기 사각형={:.0}x{:.0}",
            self.monitor_key,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
            swap_width,
            swap_height,
            target_dpi_x,
            target_size.width,
            target_size.height
        ));
        Ok(())
    }
}

fn create_swap_chain(
    graphics: &GraphicsDevice,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    let description = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        ..Default::default()
    };
    unsafe {
        graphics.dxgi_factory.CreateSwapChainForComposition(
            &graphics.d3d_device,
            &description,
            None::<&IDXGIOutput>,
        )
    }
}

fn create_swap_target(
    context: &ID2D1DeviceContext,
    swap_chain: &IDXGISwapChain1,
) -> Result<ID2D1Bitmap1> {
    let surface: IDXGISurface = unsafe { swap_chain.GetBuffer(0) }?;
    let properties = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: pixel_format(),
        dpiX: TARGET_DPI,
        dpiY: TARGET_DPI,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };
    unsafe { context.CreateBitmapFromDxgiSurface(&surface, Some(&properties)) }
}

fn create_completed_bitmap(
    context: &ID2D1DeviceContext,
    width: u32,
    height: u32,
) -> Result<ID2D1Bitmap1> {
    let properties = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: pixel_format(),
        dpiX: TARGET_DPI,
        dpiY: TARGET_DPI,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
        ..Default::default()
    };
    unsafe { context.CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &properties) }
}

fn pixel_format() -> D2D1_PIXEL_FORMAT {
    D2D1_PIXEL_FORMAT {
        format: DXGI_FORMAT_B8G8R8A8_UNORM,
        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
    }
}

fn valid_dimension(value: i32) -> Result<u32> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                "overlay dimension must be positive",
            )
        })
}
