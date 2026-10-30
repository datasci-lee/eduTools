use crate::app_icon::{AppIconRenderer, IconTint};
use crate::ink_view::{EraserRenderState, LaserRenderState, SelectionRenderState};
use crate::log::Logger;
use edutools_core::board::{BoardBackgroundPattern, BoardKind, BoardPagePosition};
use edutools_core::eraser::{
    EraserAreaShape, EraserEngine, EraserGesture, EraserHitRule, EraserMode, EraserSettings,
    EraserTargetFilter,
};
use edutools_core::ink::{
    ArrowStyle, InkGeometry, InkPoint, InkShapeKind, InkStroke, InkStrokeContent, InkStrokeTool,
    InkTextContent, PathCommand, Point, Size,
};
use edutools_core::shape::InkItemGeometry;
use windows::core::{Error, Interface, Result};
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, HMODULE, HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_COMPOSITE_MODE_SOURCE_OVER,
    D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
    D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1Shadow, D2D1CreateDevice, ID2D1Bitmap1, ID2D1Device, ID2D1DeviceContext, ID2D1Effect,
    ID2D1Factory, ID2D1GeometrySink, ID2D1Image, ID2D1Layer, ID2D1PathGeometry, ID2D1RenderTarget,
    ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_CUSTOM, D2D1_DASH_STYLE_SOLID,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE,
    D2D1_INTERPOLATION_MODE_LINEAR, D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1,
    D2D1_LINE_JOIN_ROUND, D2D1_MAP_OPTIONS_READ, D2D1_PRIMITIVE_BLEND_COPY,
    D2D1_PRIMITIVE_BLEND_SOURCE_OVER, D2D1_PROPERTY_TYPE_FLOAT, D2D1_PROPERTY_TYPE_VECTOR4,
    D2D1_QUADRATIC_BEZIER_SEGMENT, D2D1_ROUNDED_RECT, D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION,
    D2D1_SHADOW_PROP_COLOR, D2D1_STROKE_STYLE_PROPERTIES,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
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
// macOS systemGray/separator 근사.
pub const COVER_STROKE_COLOR: [f64; 4] = [142.0 / 255.0, 142.0 / 255.0, 147.0 / 255.0, 1.0];
const COVER_FILL_ALPHA: f32 = 0.96;
const COVER_BORDER_ALPHA: f32 = 0.1;
const COVER_BORDER_WIDTH_DIP: f64 = 1.0;
const SYSTEM_ACCENT_FALLBACK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.47,
    b: 0.84,
    a: 1.0,
};

#[derive(Clone, Copy, Debug)]
struct PixelSample {
    x: u32,
    y: u32,
    bgra: [u8; 4],
}

struct PreparedLaser {
    bitmap: ID2D1Bitmap1,
    _shadow: ID2D1Effect,
    shadow_output: ID2D1Image,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardRenderState {
    pub kind: BoardKind,
    pub background_color: [f64; 4],
    pub pattern: BoardBackgroundPattern,
    pub pattern_color: [f64; 4],
    pub page_position: BoardPagePosition,
}

macro_rules! d2d_point {
    ($point:expr) => {{
        let point = $point;
        let mut value = D2D1_ELLIPSE::default().point;
        value.X = point.x as f32;
        value.Y = point.y as f32;
        value
    }};
}

#[derive(Clone)]
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

    pub fn create_d2d_context(&self) -> Result<ID2D1DeviceContext> {
        unsafe {
            self.d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
        }
    }

    pub(crate) fn d3d_device(&self) -> ID3D11Device {
        self.d3d_device.clone()
    }

    pub fn attach_composition_swap_chain(
        &self,
        hwnd: HWND,
        swap_chain: &IDXGISwapChain1,
    ) -> Result<(IDCompositionTarget, IDCompositionVisual)> {
        let target = unsafe { self.dcomp_device.CreateTargetForHwnd(hwnd, true) }?;
        let visual = unsafe { self.dcomp_device.CreateVisual() }?;
        unsafe {
            visual.SetContent(swap_chain)?;
            target.SetRoot(&visual)?;
            self.dcomp_device.Commit()?;
        }
        Ok((target, visual))
    }
}

pub struct OverlayRenderer {
    hwnd: HWND,
    swap_chain: IDXGISwapChain1,
    context: ID2D1DeviceContext,
    factory: ID2D1Factory,
    round_stroke: ID2D1StrokeStyle,
    butt_stroke: ID2D1StrokeStyle,
    eraser_dash: ID2D1StrokeStyle,
    selection_lasso_dash: ID2D1StrokeStyle,
    dwrite: IDWriteFactory,
    swap_target: Option<ID2D1Bitmap1>,
    completed_bitmap: Option<ID2D1Bitmap1>,
    zoom_frame_bitmap: Option<ID2D1Bitmap1>,
    _dcomp_target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    width: u32,
    height: u32,
    dpi: u32,
    monitor_key: String,
    logger: Logger,
}

impl OverlayRenderer {
    pub fn new(
        graphics: &GraphicsDevice,
        hwnd: HWND,
        width: i32,
        height: i32,
        dpi: u32,
        monitor_key: String,
        logger: Logger,
    ) -> Result<Self> {
        let width = valid_dimension(width)?;
        let height = valid_dimension(height)?;
        let swap_chain = create_swap_chain(graphics, width, height)?;
        // SAFETY: the D2D device is live and all access is confined to the UI thread.
        let context = graphics.create_d2d_context()?;
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
        let butt_properties = D2D1_STROKE_STYLE_PROPERTIES {
            startCap: D2D1_CAP_STYLE_FLAT,
            endCap: D2D1_CAP_STYLE_FLAT,
            dashCap: D2D1_CAP_STYLE_FLAT,
            ..stroke_properties
        };
        let butt_stroke = unsafe { factory.CreateStrokeStyle(&butt_properties, None) }?;
        let dash_properties = D2D1_STROKE_STYLE_PROPERTIES {
            dashStyle: D2D1_DASH_STYLE_CUSTOM,
            ..stroke_properties
        };
        let eraser_dash =
            unsafe { factory.CreateStrokeStyle(&dash_properties, Some(&[4.0, 2.666_666_7])) }?;
        let selection_lasso_dash = unsafe {
            factory.CreateStrokeStyle(&dash_properties, Some(&[3.333_333_3, 2.666_666_7]))
        }?;
        let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let swap_target = Some(create_swap_target(&context, &swap_chain)?);
        let completed_bitmap = Some(create_completed_bitmap(&context, width, height)?);
        let zoom_frame_bitmap = Some(create_completed_bitmap(&context, width, height)?);
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
        AppIconRenderer::audit_startup(&logger);
        let renderer = Self {
            hwnd,
            swap_chain,
            context,
            factory,
            round_stroke,
            butt_stroke,
            eraser_dash,
            selection_lasso_dash,
            dwrite,
            swap_target,
            completed_bitmap,
            zoom_frame_bitmap,
            _dcomp_target: dcomp_target,
            _visual: visual,
            width,
            height,
            dpi,
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
        dpi: u32,
        strokes: &[InkStroke],
        minimum_ratio: f64,
        highlighter_opacity: f64,
    ) -> Result<()> {
        let width = valid_dimension(width)?;
        let height = valid_dimension(height)?;
        unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
        self.swap_target = None;
        self.completed_bitmap = None;
        self.zoom_frame_bitmap = None;
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
        self.zoom_frame_bitmap = Some(create_completed_bitmap(&self.context, width, height)?);
        let swap_image: ID2D1Image = self
            .swap_target
            .as_ref()
            .expect("swap target recreated")
            .cast()?;
        unsafe { self.context.SetTarget(&swap_image) };
        self.width = width;
        self.height = height;
        self.dpi = dpi;
        self.rebuild_completed(strokes, minimum_ratio, highlighter_opacity)?;
        self.log_size()?;
        Ok(())
    }

    pub fn swap_chain_size(&self) -> Result<(u32, u32)> {
        let description = unsafe { self.swap_chain.GetDesc1() }?;
        Ok((description.Width, description.Height))
    }

    pub fn frame_bitmap(&self) -> Option<ID2D1Bitmap1> {
        self.zoom_frame_bitmap.clone()
    }

    pub fn selftest_pixel_failures(
        &self,
        minimum_ratio: f64,
        highlighter_opacity: f64,
    ) -> Result<Vec<String>> {
        let result = self.selftest_pixel_failures_inner(minimum_ratio, highlighter_opacity);
        self.finish_offscreen_draw(result)
    }

    fn selftest_pixel_failures_inner(
        &self,
        minimum_ratio: f64,
        highlighter_opacity: f64,
    ) -> Result<Vec<String>> {
        let completed = create_completed_bitmap(&self.context, self.width, self.height)?;
        let frame = create_completed_bitmap(&self.context, self.width, self.height)?;
        let opaque_black = D2D1_COLOR_F {
            a: 1.0,
            ..Default::default()
        };
        let transparent_points = transparency_probe_points(self.width, self.height);
        let mut failures = Vec::new();

        self.clear_bitmap(&completed, &opaque_black)?;
        self.clear_bitmap(&frame, &opaque_black)?;
        self.draw_strokes_to_bitmap(&completed, &[], minimum_ratio, highlighter_opacity, true)?;
        self.draw_frame_to_bitmap(
            &frame,
            &completed,
            None,
            None,
            None,
            None,
            None,
            minimum_ratio,
            highlighter_opacity,
            false,
        )?;
        self.restore_swap_target()?;
        check_transparent_samples(
            "빈 판서 프레임",
            &read_pixel_samples(&self.context, &frame, &transparent_points)?,
            &mut failures,
        );

        let white_board = BoardRenderState {
            kind: BoardKind::White,
            background_color: [1.0, 1.0, 1.0, 1.0],
            pattern: BoardBackgroundPattern::Grid,
            pattern_color: [0.0, 0.0, 0.0, 0.12],
            page_position: BoardPagePosition {
                current: 1,
                total: 1,
            },
        };
        self.draw_frame_to_bitmap(
            &frame,
            &completed,
            None,
            None,
            None,
            None,
            Some(&white_board),
            minimum_ratio,
            highlighter_opacity,
            false,
        )?;
        self.restore_swap_target()?;
        let spacing = (40.0 * px_per_dip(self.dpi)).round() as u32;
        let white_samples = read_pixel_samples(
            &self.context,
            &frame,
            &[(5, 5), (spacing.min(self.width - 1), 5)],
        )?;
        if white_samples[0].bgra != [255, 255, 255, 255] {
            failures.push(format!(
                "화이트보드: 배경 BGRA={:?}, 불투명 흰색이 아님",
                white_samples[0].bgra
            ));
        }
        if white_samples[1].bgra == white_samples[0].bgra {
            failures.push(format!(
                "화이트보드: 격자 BGRA={:?}, 배경과 같음",
                white_samples[1].bgra
            ));
        }
        let indicator_rect = self.board_page_indicator_rect();
        let text_pixels = pixel_count_in_rect(&self.context, &frame, indicator_rect, |pixel| {
            pixel[3] == 255 && pixel[0] < 180 && pixel[1] < 180 && pixel[2] < 180
        })?;
        if text_pixels == 0 {
            failures.push("화이트보드: 페이지 표시 영역에 글자 픽셀이 없음".to_owned());
        }

        let black_board = BoardRenderState {
            kind: BoardKind::Black,
            background_color: [0.0, 0.0, 0.0, 1.0],
            pattern: BoardBackgroundPattern::None,
            pattern_color: [1.0, 1.0, 1.0, 0.18],
            page_position: BoardPagePosition {
                current: 1,
                total: 1,
            },
        };
        self.draw_frame_to_bitmap(
            &frame,
            &completed,
            None,
            None,
            None,
            None,
            Some(&black_board),
            minimum_ratio,
            highlighter_opacity,
            false,
        )?;
        self.restore_swap_target()?;
        let black_sample = read_pixel_samples(&self.context, &frame, &[(5, 5)])?[0];
        if black_sample.bgra[3] != 255 || black_sample.bgra[..3].iter().any(|value| *value > 8) {
            failures.push(format!(
                "블랙보드: 배경 BGRA={:?}, 불투명 검정 계열이 아님",
                black_sample.bgra
            ));
        }

        for (name, tool, width) in [
            ("펜 획", InkStrokeTool::Pen, 12.0),
            ("형광펜 획", InkStrokeTool::Highlighter, 24.0),
        ] {
            let stroke = selftest_stroke(name, tool, width);
            self.clear_bitmap(&completed, &opaque_black)?;
            self.clear_bitmap(&frame, &opaque_black)?;
            self.draw_strokes_to_bitmap(
                &completed,
                std::slice::from_ref(&stroke),
                minimum_ratio,
                highlighter_opacity,
                true,
            )?;
            self.draw_frame_to_bitmap(
                &frame,
                &completed,
                None,
                None,
                None,
                None,
                None,
                minimum_ratio,
                highlighter_opacity,
                false,
            )?;
            self.restore_swap_target()?;
            check_transparent_samples(
                name,
                &read_pixel_samples(&self.context, &frame, &transparent_points)?,
                &mut failures,
            );

            let stroke_point = [(self.width / 2, self.height / 4)];
            let sample = read_pixel_samples(&self.context, &frame, &stroke_point)?[0];
            if sample.bgra[3] == 0 {
                failures.push(format!(
                    "{name}: 획 위 ({}, {}) BGRA={:?}, alpha가 0임",
                    sample.x, sample.y, sample.bgra
                ));
            }
        }

        let rectangle = selftest_vector_stroke(
            "rectangle",
            InkStrokeContent::Shape(InkShapeKind::Rectangle),
            Point { x: 0.25, y: 0.25 },
            Point { x: 0.75, y: 0.75 },
            8.0,
        );
        self.draw_strokes_to_bitmap(
            &frame,
            std::slice::from_ref(&rectangle),
            minimum_ratio,
            highlighter_opacity,
            true,
        )?;
        self.restore_swap_target()?;
        let rectangle_samples = read_pixel_samples(
            &self.context,
            &frame,
            &[
                (self.width / 4, self.height / 2),
                (self.width / 2, self.height / 2),
            ],
        )?;
        if rectangle_samples[0].bgra[3] == 0 {
            failures.push(format!(
                "사각형: 테두리 BGRA={:?}, alpha가 0임",
                rectangle_samples[0].bgra
            ));
        }
        if rectangle_samples[1].bgra[3] != 0 {
            failures.push(format!(
                "사각형: 내부 BGRA={:?}, alpha가 0이 아님",
                rectangle_samples[1].bgra
            ));
        }

        let selection_bounds = InkItemGeometry::item_bounds(
            &rectangle,
            Size {
                width: f64::from(self.width),
                height: f64::from(self.height),
            },
        )
        .expect("selftest rectangle has points");
        let selection = SelectionRenderState {
            bounds: Some(selection_bounds),
            ..SelectionRenderState::default()
        };
        self.draw_strokes_to_bitmap(
            &completed,
            std::slice::from_ref(&rectangle),
            minimum_ratio,
            highlighter_opacity,
            true,
        )?;
        self.draw_frame_to_bitmap(
            &frame,
            &completed,
            None,
            None,
            None,
            Some(&selection),
            None,
            minimum_ratio,
            highlighter_opacity,
            false,
        )?;
        self.restore_swap_target()?;
        let selection_samples = read_pixel_samples(
            &self.context,
            &frame,
            &[
                (
                    selection_bounds.x.round() as u32,
                    selection_bounds.y.round() as u32,
                ),
                (
                    (selection_bounds.x + 20.0 * px_per_dip(self.dpi)).round() as u32,
                    selection_bounds.y.round() as u32,
                ),
            ],
        )?;
        if selection_samples[0].bgra[3] == 0 {
            failures.push(format!(
                "선택 핸들: 모서리 BGRA={:?}, alpha가 0임",
                selection_samples[0].bgra
            ));
        }
        if selection_samples[1].bgra[3] == 0 {
            failures.push(format!(
                "선택 윤곽: 위쪽 BGRA={:?}, alpha가 0임",
                selection_samples[1].bgra
            ));
        }

        let ellipse = selftest_vector_stroke(
            "ellipse",
            InkStrokeContent::Shape(InkShapeKind::Ellipse),
            Point { x: 0.25, y: 0.25 },
            Point { x: 0.75, y: 0.75 },
            8.0,
        );
        self.draw_strokes_to_bitmap(
            &frame,
            std::slice::from_ref(&ellipse),
            minimum_ratio,
            highlighter_opacity,
            true,
        )?;
        self.restore_swap_target()?;
        let ellipse_samples = read_pixel_samples(
            &self.context,
            &frame,
            &[
                (self.width / 2, self.height / 2),
                (self.width / 2, self.height / 4),
            ],
        )?;
        if ellipse_samples[0].bgra[3] != 0 {
            failures.push(format!(
                "원: 중심 BGRA={:?}, alpha가 0이 아님",
                ellipse_samples[0].bgra
            ));
        }
        if ellipse_samples[1].bgra[3] == 0 {
            failures.push(format!(
                "원: 테두리 BGRA={:?}, alpha가 0임",
                ellipse_samples[1].bgra
            ));
        }

        let arrow_width = 8.0;
        let arrow = selftest_vector_stroke(
            "arrow",
            InkStrokeContent::Shape(InkShapeKind::Arrow(ArrowStyle::Single)),
            Point { x: 0.25, y: 0.5 },
            Point { x: 0.75, y: 0.5 },
            arrow_width,
        );
        self.draw_strokes_to_bitmap(
            &frame,
            std::slice::from_ref(&arrow),
            minimum_ratio,
            highlighter_opacity,
            true,
        )?;
        self.restore_swap_target()?;
        let arrow_head_length = 10.0_f64.max(arrow_width * 4.0) * px_per_dip(self.dpi);
        let arrow_tip = Point {
            x: f64::from(self.width) * 0.75,
            y: f64::from(self.height) * 0.5,
        };
        let arrow_head_sample = [(
            (arrow_tip.x - (std::f64::consts::PI / 7.0).cos() * arrow_head_length * 0.5).round()
                as u32,
            (arrow_tip.y + (std::f64::consts::PI / 7.0).sin() * arrow_head_length * 0.5).round()
                as u32,
        )];
        let arrow_sample = read_pixel_samples(&self.context, &frame, &arrow_head_sample)?[0];
        if arrow_sample.bgra[3] == 0 {
            failures.push(format!(
                "화살표: 머리 BGRA={:?}, alpha가 0임",
                arrow_sample.bgra
            ));
        }

        let cover = selftest_vector_stroke(
            "cover",
            InkStrokeContent::Cover,
            Point { x: 0.25, y: 0.25 },
            Point { x: 0.75, y: 0.75 },
            COVER_BORDER_WIDTH_DIP,
        );
        self.draw_strokes_to_bitmap(
            &frame,
            std::slice::from_ref(&cover),
            minimum_ratio,
            highlighter_opacity,
            true,
        )?;
        self.restore_swap_target()?;
        let cover_sample =
            read_pixel_samples(&self.context, &frame, &[(self.width / 2, self.height / 2)])?[0];
        if cover_sample.bgra[3] <= 229 {
            failures.push(format!(
                "가리개: 중심 BGRA={:?}, alpha가 0.9 이하임",
                cover_sample.bgra
            ));
        }

        let text_start = Point { x: 0.2, y: 0.2 };
        let text_end = Point { x: 0.8, y: 0.5 };
        let text = InkStroke::new(
            "selftest-text",
            InkStrokeTool::Text,
            [0.9, 0.95, 1.0, 1.0],
            1.0,
            false,
            vec![
                InkPoint::new(text_start, 1.0, 0.0),
                InkPoint::new(text_end, 1.0, 1.0),
            ],
            InkStrokeContent::Text(InkTextContent {
                string: "가A".to_owned(),
                font_size: 32.0,
            }),
        );
        self.draw_strokes_to_bitmap(
            &frame,
            std::slice::from_ref(&text),
            minimum_ratio,
            highlighter_opacity,
            true,
        )?;
        self.restore_swap_target()?;
        let text_rect = D2D_RECT_F {
            left: text_start.x as f32 * self.width as f32,
            top: text_start.y as f32 * self.height as f32,
            right: text_end.x as f32 * self.width as f32,
            bottom: text_end.y as f32 * self.height as f32,
        };
        let (inside_alpha, outside_alpha) = alpha_counts_in_rect(&self.context, &frame, text_rect)?;
        if inside_alpha == 0 {
            failures.push("텍스트: 영역 안 alpha>0 픽셀이 없음".to_owned());
        }
        if outside_alpha != 0 {
            failures.push(format!(
                "텍스트: 영역 밖 alpha>0 픽셀이 존재함 ({outside_alpha})"
            ));
        }

        let eraser_stroke = InkStroke::new(
            "selftest-partial-eraser",
            InkStrokeTool::Pen,
            [0.2, 0.8, 1.0, 1.0],
            12.0,
            false,
            vec![
                InkPoint::new(Point { x: 0.2, y: 0.5 }, 1.0, 0.0),
                InkPoint::new(Point { x: 0.8, y: 0.5 }, 1.0, 1.0),
            ],
            InkStrokeContent::Freehand,
        );
        let canvas_size = Size {
            width: f64::from(self.width),
            height: f64::from(self.height),
        };
        let eraser_settings = EraserSettings {
            diameter: (f64::from(self.width) * 0.08).max(24.0),
            mode: EraserMode::Partial,
            hit_rule: EraserHitRule::Touching,
            target_filter: EraserTargetFilter::All,
            area_shape: EraserAreaShape::Rectangle,
        };
        let erased = EraserEngine::erase(
            std::slice::from_ref(&eraser_stroke),
            &EraserGesture::Sweep(vec![Point {
                x: f64::from(self.width) * 0.5,
                y: f64::from(self.height) * 0.5,
            }]),
            &eraser_settings,
            canvas_size,
            minimum_ratio,
        );
        self.draw_strokes_to_bitmap(
            &completed,
            &erased,
            minimum_ratio,
            highlighter_opacity,
            true,
        )?;
        self.draw_frame_to_bitmap(
            &frame,
            &completed,
            None,
            None,
            None,
            None,
            None,
            minimum_ratio,
            highlighter_opacity,
            false,
        )?;
        self.restore_swap_target()?;
        let eraser_samples = read_pixel_samples(
            &self.context,
            &frame,
            &[
                (self.width / 2, self.height / 2),
                (self.width * 3 / 10, self.height / 2),
                (self.width * 7 / 10, self.height / 2),
            ],
        )?;
        if eraser_samples[0].bgra[3] != 0 {
            failures.push(format!(
                "부분 지우개: 가운데 BGRA={:?}, alpha가 0이 아님",
                eraser_samples[0].bgra
            ));
        }
        for sample in &eraser_samples[1..] {
            if sample.bgra[3] == 0 {
                failures.push(format!(
                    "부분 지우개: 바깥 ({}, {}) BGRA={:?}, alpha가 0임",
                    sample.x, sample.y, sample.bgra
                ));
            }
        }

        self.draw_strokes_to_bitmap(&completed, &[], minimum_ratio, highlighter_opacity, true)?;
        let laser = LaserRenderState {
            point: Some(Point {
                x: f64::from(self.width) * 0.75,
                y: f64::from(self.height) * 0.75,
            }),
            trails: vec![vec![
                Point {
                    x: f64::from(self.width) * 0.25,
                    y: f64::from(self.height) * 0.75,
                },
                Point {
                    x: f64::from(self.width) * 0.75,
                    y: f64::from(self.height) * 0.75,
                },
            ]],
            color: [1.0, 0.1, 0.1, 1.0],
            size_dip: 8.0,
        };
        self.draw_frame_to_bitmap(
            &frame,
            &completed,
            None,
            None,
            Some(&laser),
            None,
            None,
            minimum_ratio,
            highlighter_opacity,
            false,
        )?;
        self.restore_swap_target()?;
        let laser_sample = read_pixel_samples(
            &self.context,
            &frame,
            &[(self.width / 2, self.height * 3 / 4)],
        )?[0];
        if laser_sample.bgra[3] == 0 {
            failures.push(format!(
                "레이저 꼬리: 가운데 BGRA={:?}, alpha가 0임",
                laser_sample.bgra
            ));
        }

        Ok(failures)
    }

    pub fn append_completed(
        &self,
        stroke: &InkStroke,
        minimum_ratio: f64,
        highlighter_opacity: f64,
    ) -> Result<()> {
        let result = self.draw_strokes_to_bitmap(
            self.completed_bitmap(),
            std::slice::from_ref(stroke),
            minimum_ratio,
            highlighter_opacity,
            false,
        );
        self.finish_offscreen_draw(result)
    }

    pub fn rebuild_completed(
        &self,
        strokes: &[InkStroke],
        minimum_ratio: f64,
        highlighter_opacity: f64,
    ) -> Result<()> {
        let result = self.draw_strokes_to_bitmap(
            self.completed_bitmap(),
            strokes,
            minimum_ratio,
            highlighter_opacity,
            true,
        );
        self.finish_offscreen_draw(result)
    }

    pub fn draw_frame(
        &self,
        active_stroke: Option<&InkStroke>,
        eraser: Option<&EraserRenderState>,
        laser: Option<&LaserRenderState>,
        selection: Option<&SelectionRenderState>,
        board: Option<&BoardRenderState>,
        minimum_ratio: f64,
        highlighter_opacity: f64,
        debug_visible: bool,
    ) -> Result<()> {
        let swap_target = self
            .swap_target
            .as_ref()
            .expect("swap target exists outside resize");
        let zoom_frame = self
            .zoom_frame_bitmap
            .as_ref()
            .expect("zoom frame bitmap exists outside resize");
        self.draw_frame_to_bitmap(
            zoom_frame,
            self.completed_bitmap(),
            active_stroke,
            eraser,
            laser,
            selection,
            board,
            minimum_ratio,
            highlighter_opacity,
            debug_visible,
        )?;
        self.set_target(swap_target)?;
        let bounds = frame_bounds(self.width, self.height);
        unsafe {
            self.context.BeginDraw();
            self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
            self.context.DrawBitmap(
                zoom_frame,
                Some(&bounds),
                1.0,
                D2D1_INTERPOLATION_MODE_LINEAR,
                Some(&bounds),
                None,
            );
            self.context
                .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            self.context.EndDraw(None, None)?;
        }
        unsafe { self.swap_chain.Present(1, Default::default()).ok() }
    }

    fn draw_frame_to_bitmap(
        &self,
        target: &ID2D1Bitmap1,
        completed: &ID2D1Bitmap1,
        active_stroke: Option<&InkStroke>,
        eraser: Option<&EraserRenderState>,
        laser: Option<&LaserRenderState>,
        selection: Option<&SelectionRenderState>,
        board: Option<&BoardRenderState>,
        minimum_ratio: f64,
        highlighter_opacity: f64,
        debug_visible: bool,
    ) -> Result<()> {
        let preview_strokes = selection
            .and_then(|state| state.preview_strokes.as_deref())
            .or_else(|| eraser.and_then(|state| state.preview_strokes.as_deref()));
        let preview_bitmap = if let Some(strokes) = preview_strokes {
            let bitmap = create_completed_bitmap(&self.context, self.width, self.height)?;
            let result = self.draw_strokes_to_bitmap(
                &bitmap,
                strokes,
                minimum_ratio,
                highlighter_opacity,
                true,
            );
            self.finish_offscreen_draw(result)?;
            Some(bitmap)
        } else {
            None
        };
        let prepared_laser = laser.map(|laser| self.prepare_laser(laser)).transpose()?;
        self.set_target(target)?;
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
        let bounds = frame_bounds(self.width, self.height);
        unsafe { self.context.BeginDraw() };
        let draw_result: Result<()> = (|| {
            if let Some(board) = board {
                // Copy an opaque full-frame board first so transparent/opaque transitions never
                // retain a stale flip-buffer pixel, then source-over pattern and ink.
                unsafe { self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY) };
                self.draw_board_background(board, bounds)?;
                unsafe {
                    self.context
                        .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER)
                };
                self.draw_board_pattern(board, bounds)?;
                unsafe {
                    self.context.DrawBitmap(
                        preview_bitmap.as_ref().unwrap_or(completed),
                        Some(&bounds),
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        Some(&bounds),
                        None,
                    );
                }
            } else if debug_visible {
                unsafe {
                    self.context.Clear(Some(&clear));
                    self.context.DrawBitmap(
                        preview_bitmap.as_ref().unwrap_or(completed),
                        Some(&bounds),
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        Some(&bounds),
                        None,
                    );
                }
            } else {
                // Transparent source-over cannot erase stale flip-buffer alpha; copy exact bounds.
                unsafe {
                    self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                    self.context.DrawBitmap(
                        preview_bitmap.as_ref().unwrap_or(completed),
                        Some(&bounds),
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        Some(&bounds),
                        None,
                    );
                    self.context
                        .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
                }
            }
            if let Some(selection) = selection {
                self.draw_selection(selection)?;
            }
            if let Some(stroke) = active_stroke {
                self.draw_stroke(stroke, minimum_ratio, highlighter_opacity)?;
            }
            if let Some(eraser) = eraser {
                self.draw_eraser(eraser, minimum_ratio, highlighter_opacity)?;
            }
            if let Some(prepared) = prepared_laser.as_ref() {
                self.draw_prepared_laser(prepared)?;
            }
            if let Some(board) = board {
                self.draw_board_page_indicator(board)?;
            }
            Ok(())
        })();
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
        end_result
    }

    fn draw_board_background(&self, board: &BoardRenderState, bounds: D2D_RECT_F) -> Result<()> {
        let color = d2d_color(board.background_color);
        let brush = unsafe { self.context.CreateSolidColorBrush(&color, None) }?;
        unsafe { self.context.FillRectangle(&bounds, &brush) };
        Ok(())
    }

    fn draw_board_pattern(&self, board: &BoardRenderState, bounds: D2D_RECT_F) -> Result<()> {
        if board.pattern == BoardBackgroundPattern::None {
            return Ok(());
        }
        let scale = px_per_dip(self.dpi) as f32;
        let spacing = 40.0 * scale;
        let stroke = scale;
        let color = d2d_color(board.pattern_color);
        let brush = unsafe { self.context.CreateSolidColorBrush(&color, None) }?;
        unsafe {
            if board.pattern == BoardBackgroundPattern::Grid {
                let mut x = spacing;
                while x <= bounds.right {
                    let mut start = D2D1_ELLIPSE::default().point;
                    start.X = x;
                    start.Y = bounds.top;
                    let mut end = D2D1_ELLIPSE::default().point;
                    end.X = x;
                    end.Y = bounds.bottom;
                    self.context
                        .DrawLine(start, end, &brush, stroke, &self.butt_stroke);
                    x += spacing;
                }
            }
            let mut y = spacing;
            while y <= bounds.bottom {
                let mut start = D2D1_ELLIPSE::default().point;
                start.X = bounds.left;
                start.Y = y;
                let mut end = D2D1_ELLIPSE::default().point;
                end.X = bounds.right;
                end.Y = y;
                self.context
                    .DrawLine(start, end, &brush, stroke, &self.butt_stroke);
                y += spacing;
            }
        }
        Ok(())
    }

    fn draw_board_page_indicator(&self, board: &BoardRenderState) -> Result<()> {
        let rect = self.board_page_indicator_rect();
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return Ok(());
        }
        let scale = px_per_dip(self.dpi) as f32;
        let (foreground, background) = match board.kind {
            BoardKind::White => ([0.0, 0.0, 0.0, 0.68], [0.0, 0.0, 0.0, 0.08]),
            BoardKind::Black => ([1.0, 1.0, 1.0, 0.82], [1.0, 1.0, 1.0, 0.12]),
        };
        let foreground = d2d_color(foreground);
        let background = d2d_color(background);
        let foreground_brush = unsafe { self.context.CreateSolidColorBrush(&foreground, None) }?;
        let background_brush = unsafe { self.context.CreateSolidColorBrush(&background, None) }?;
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: 8.0 * scale,
            radiusY: 8.0 * scale,
        };
        unsafe {
            self.context
                .FillRoundedRectangle(&rounded, &background_brush)
        };

        let format = unsafe {
            self.dwrite.CreateTextFormat(
                windows::core::w!("Segoe UI"),
                None::<&IDWriteFontCollection>,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                15.0 * scale,
                windows::core::w!("ko-KR"),
            )
        }?;
        unsafe {
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        let text = format!(
            "{} / {}",
            board.page_position.current, board.page_position.total
        )
        .encode_utf16()
        .collect::<Vec<_>>();
        let target: ID2D1RenderTarget = self.context.cast()?;
        unsafe {
            target.DrawText(
                &text,
                &format,
                &rect,
                &foreground_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    fn board_page_indicator_rect(&self) -> D2D_RECT_F {
        let scale = px_per_dip(self.dpi) as f32;
        let width = (self.width as f32 - 32.0 * scale)
            .max(0.0)
            .min(84.0 * scale);
        let left = (16.0 * scale).max(self.width as f32 - width - 20.0 * scale);
        D2D_RECT_F {
            left,
            top: 20.0 * scale,
            right: left + width,
            bottom: 50.0 * scale,
        }
    }

    fn draw_selection(&self, selection: &SelectionRenderState) -> Result<()> {
        let accent_brush = unsafe {
            self.context
                .CreateSolidColorBrush(&SYSTEM_ACCENT_FALLBACK, None)
        }?;
        let fill = D2D1_COLOR_F {
            a: 0.08,
            ..SYSTEM_ACCENT_FALLBACK
        };
        let fill_brush = unsafe { self.context.CreateSolidColorBrush(&fill, None) }?;
        let scale = px_per_dip(self.dpi) as f32;
        let stroke_width = 1.5 * scale;

        if selection.lasso.len() >= 2 {
            let geometry = unsafe { self.factory.CreatePathGeometry() }?;
            let sink = unsafe { geometry.Open() }?;
            unsafe {
                sink.BeginFigure(d2d_point!(selection.lasso[0]), D2D1_FIGURE_BEGIN_FILLED);
                for point in selection.lasso.iter().skip(1) {
                    sink.AddLine(d2d_point!(*point));
                }
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close()?;
                self.context.FillGeometry(
                    &geometry,
                    &fill_brush,
                    None::<&windows::Win32::Graphics::Direct2D::ID2D1Brush>,
                );
                self.context.DrawGeometry(
                    &geometry,
                    &accent_brush,
                    stroke_width,
                    &self.selection_lasso_dash,
                );
            }
        }

        let Some(bounds) = selection.bounds else {
            return Ok(());
        };
        let bounds = bounds.standardized();
        let rect = D2D_RECT_F {
            left: bounds.x as f32,
            top: bounds.y as f32,
            right: (bounds.x + bounds.width) as f32,
            bottom: (bounds.y + bounds.height) as f32,
        };
        unsafe {
            self.context
                .DrawRectangle(&rect, &accent_brush, stroke_width, &self.eraser_dash);
        }

        let white = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let white_brush = unsafe { self.context.CreateSolidColorBrush(&white, None) }?;
        let half_handle = 5.0 * scale;
        for (x, y) in [
            (rect.left, rect.top),
            (rect.right, rect.top),
            (rect.right, rect.bottom),
            (rect.left, rect.bottom),
        ] {
            let handle = D2D_RECT_F {
                left: x - half_handle,
                top: y - half_handle,
                right: x + half_handle,
                bottom: y + half_handle,
            };
            unsafe {
                self.context.FillRectangle(&handle, &white_brush);
                self.context.DrawRectangle(
                    &handle,
                    &accent_brush,
                    stroke_width,
                    None::<&ID2D1StrokeStyle>,
                );
            }
        }
        Ok(())
    }

    fn draw_eraser(
        &self,
        eraser: &EraserRenderState,
        minimum_ratio: f64,
        _highlighter_opacity: f64,
    ) -> Result<()> {
        if !eraser.highlighted_strokes.is_empty() {
            let parameters = opacity_layer_parameters(frame_bounds(self.width, self.height), 0.45);
            unsafe { self.context.PushLayer(&parameters, None::<&ID2D1Layer>) };
            let result: Result<()> = (|| {
                for stroke in &eraser.highlighted_strokes {
                    let mut highlight = stroke.clone();
                    highlight.tool = InkStrokeTool::Pen;
                    highlight.color = [
                        f64::from(SYSTEM_ACCENT_FALLBACK.r),
                        f64::from(SYSTEM_ACCENT_FALLBACK.g),
                        f64::from(SYSTEM_ACCENT_FALLBACK.b),
                        f64::from(SYSTEM_ACCENT_FALLBACK.a),
                    ];
                    highlight.width += 4.0;
                    highlight.uses_pressure = false;
                    self.draw_opaque_stroke(&highlight, minimum_ratio)?;
                }
                Ok(())
            })();
            unsafe { self.context.PopLayer() };
            result?;
        }

        if eraser.settings.mode == EraserMode::Area {
            self.draw_eraser_area(eraser)?;
        }
        if let Some(point) = eraser.cursor {
            let radius = (eraser.settings.diameter / 2.0) as f32;
            let ellipse = D2D1_ELLIPSE {
                point: d2d_point!(point),
                radiusX: radius,
                radiusY: radius,
            };
            let black = D2D1_COLOR_F {
                a: 0.8,
                ..Default::default()
            };
            let white = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            };
            let black_brush = unsafe { self.context.CreateSolidColorBrush(&black, None) }?;
            let white_brush = unsafe { self.context.CreateSolidColorBrush(&white, None) }?;
            let scale = px_per_dip(self.dpi) as f32;
            unsafe {
                self.context
                    .DrawEllipse(&ellipse, &black_brush, 3.5 * scale, &self.round_stroke);
                self.context
                    .DrawEllipse(&ellipse, &white_brush, 1.5 * scale, &self.round_stroke);
            }
        }
        Ok(())
    }

    fn draw_eraser_area(&self, eraser: &EraserRenderState) -> Result<()> {
        let Some(first) = eraser.points.first().copied() else {
            return Ok(());
        };
        let accent = SYSTEM_ACCENT_FALLBACK;
        let fill = D2D1_COLOR_F { a: 0.12, ..accent };
        let stroke_brush = unsafe { self.context.CreateSolidColorBrush(&accent, None) }?;
        let fill_brush = unsafe { self.context.CreateSolidColorBrush(&fill, None) }?;
        let stroke_width = (1.5 * px_per_dip(self.dpi)) as f32;
        match eraser.settings.area_shape {
            EraserAreaShape::Rectangle => {
                let last = eraser.points.last().copied().unwrap_or(first);
                let rect = D2D_RECT_F {
                    left: first.x.min(last.x) as f32,
                    top: first.y.min(last.y) as f32,
                    right: first.x.max(last.x) as f32,
                    bottom: first.y.max(last.y) as f32,
                };
                unsafe {
                    self.context.FillRectangle(&rect, &fill_brush);
                    self.context.DrawRectangle(
                        &rect,
                        &stroke_brush,
                        stroke_width,
                        &self.eraser_dash,
                    );
                }
            }
            EraserAreaShape::Lasso if eraser.points.len() >= 2 => {
                let geometry = unsafe { self.factory.CreatePathGeometry() }?;
                let sink = unsafe { geometry.Open() }?;
                unsafe {
                    sink.BeginFigure(d2d_point!(first), D2D1_FIGURE_BEGIN_FILLED);
                    for point in eraser.points.iter().skip(1) {
                        sink.AddLine(d2d_point!(*point));
                    }
                    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                    sink.Close()?;
                    self.context.FillGeometry(
                        &geometry,
                        &fill_brush,
                        None::<&windows::Win32::Graphics::Direct2D::ID2D1Brush>,
                    );
                    self.context.DrawGeometry(
                        &geometry,
                        &stroke_brush,
                        stroke_width,
                        &self.eraser_dash,
                    );
                }
            }
            EraserAreaShape::Lasso => {}
        }
        Ok(())
    }

    fn prepare_laser(&self, laser: &LaserRenderState) -> Result<PreparedLaser> {
        let bitmap = create_completed_bitmap(&self.context, self.width, self.height)?;
        self.set_target(&bitmap)?;
        let color = D2D1_COLOR_F {
            r: laser.color[0] as f32,
            g: laser.color[1] as f32,
            b: laser.color[2] as f32,
            a: 1.0,
        };
        let brush = unsafe { self.context.CreateSolidColorBrush(&color, None) }?;
        let scale = px_per_dip(self.dpi);
        let radius = laser.size_dip * scale;
        let trail_width = (4.0_f64.max(laser.size_dip * 1.25) * scale) as f32;
        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&D2D1_COLOR_F::default()));
        }
        let draw_result = (|| {
            if let Some(geometry) = self.laser_geometry(&laser.trails)? {
                let parameters =
                    opacity_layer_parameters(frame_bounds(self.width, self.height), 0.7);
                unsafe {
                    self.context.PushLayer(&parameters, None::<&ID2D1Layer>);
                    self.context
                        .DrawGeometry(&geometry, &brush, trail_width, &self.round_stroke);
                    self.context.PopLayer();
                }
            }
            if let Some(point) = laser.point {
                let ellipse = D2D1_ELLIPSE {
                    point: d2d_point!(point),
                    radiusX: radius as f32,
                    radiusY: radius as f32,
                };
                unsafe { self.context.FillEllipse(&ellipse, &brush) };
            }
            Ok(())
        })();
        let end_result = unsafe { self.context.EndDraw(None, None) };
        self.finish_offscreen_draw(draw_result)?;
        end_result?;

        let shadow = unsafe { self.context.CreateEffect(&CLSID_D2D1Shadow) }?;
        let image: ID2D1Image = bitmap.cast()?;
        let blur = radius as f32;
        let shadow_color = [
            laser.color[0] as f32,
            laser.color[1] as f32,
            laser.color[2] as f32,
            0.9_f32,
        ];
        unsafe {
            shadow.SetInput(0, &image, true);
            shadow.SetValue(
                D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                bytes_of(&blur),
            )?;
            shadow.SetValue(
                D2D1_SHADOW_PROP_COLOR.0 as u32,
                D2D1_PROPERTY_TYPE_VECTOR4,
                bytes_of(&shadow_color),
            )?;
        }
        let shadow_output = unsafe { shadow.GetOutput() }?;
        Ok(PreparedLaser {
            bitmap,
            _shadow: shadow,
            shadow_output,
        })
    }

    fn laser_geometry(&self, trails: &[Vec<Point>]) -> Result<Option<ID2D1PathGeometry>> {
        let geometry = unsafe { self.factory.CreatePathGeometry() }?;
        let sink = unsafe { geometry.Open() }?;
        let mut has_figure = false;
        let mut figure_open = false;
        for trail in trails {
            for command in InkGeometry::laser_trail_path(trail) {
                match command {
                    PathCommand::MoveTo(point) => {
                        if figure_open {
                            unsafe { sink.EndFigure(D2D1_FIGURE_END_OPEN) };
                        }
                        unsafe { sink.BeginFigure(d2d_point!(point), D2D1_FIGURE_BEGIN_HOLLOW) };
                        figure_open = true;
                        has_figure = true;
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
        }
        if figure_open {
            unsafe { sink.EndFigure(D2D1_FIGURE_END_OPEN) };
        }
        unsafe { sink.Close()? };
        Ok(has_figure.then_some(geometry))
    }

    fn draw_prepared_laser(&self, laser: &PreparedLaser) -> Result<()> {
        let bounds = frame_bounds(self.width, self.height);
        unsafe {
            self.context.DrawImage(
                &laser.shadow_output,
                None,
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
            self.context.DrawBitmap(
                &laser.bitmap,
                Some(&bounds),
                1.0,
                D2D1_INTERPOLATION_MODE_LINEAR,
                Some(&bounds),
                None,
            );
        }
        Ok(())
    }

    pub fn selftest_icons(&self) -> Result<()> {
        self.restore_swap_target()?;
        let mut renderer = AppIconRenderer::new(self.context.clone(), self.logger.clone())?;
        let cases = [
            ("tool.arrow", IconTint::rgba(1.0, 1.0, 1.0, 1.0)),
            ("board.black", IconTint::rgba(0.0, 0.0, 0.0, 1.0)),
            (
                "tool.highlighter",
                IconTint::rgba(0.2196, 0.7412, 0.9725, 0.55),
            ),
        ];
        for (key, tint) in cases {
            let pixels = renderer.icon_alpha_count(key, tint)?;
            self.logger.event(&format!(
                "--selftest icon {key} px={pixels}; {}",
                renderer.tint_report()
            ));
            if pixels == 0 {
                return Err(Error::new(
                    E_FAIL,
                    format!("icon {key} px=0; {}", renderer.tint_report()),
                ));
            }
        }
        Ok(())
    }

    fn draw_stroke(
        &self,
        stroke: &InkStroke,
        minimum_ratio: f64,
        highlighter_opacity: f64,
    ) -> Result<()> {
        match &stroke.content {
            InkStrokeContent::Shape(kind) => return self.draw_shape(stroke, *kind),
            InkStrokeContent::Text(content) => return self.draw_text(stroke, content),
            InkStrokeContent::Cover => return self.draw_cover(stroke),
            InkStrokeContent::Freehand => {}
        }
        if stroke.tool != InkStrokeTool::Highlighter {
            return self.draw_opaque_stroke(stroke, minimum_ratio);
        }

        let mut parameters = D2D1_LAYER_PARAMETERS1 {
            contentBounds: frame_bounds(self.width, self.height),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            opacity: highlighter_opacity as f32,
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
            ..Default::default()
        };
        parameters.maskTransform.M11 = 1.0;
        parameters.maskTransform.M22 = 1.0;
        // Each highlighter stroke is first rendered opaquely into its own layer, then alpha
        // composited once. Unlike macOS multiplyBlendMode, overlapping strokes use normal alpha.
        unsafe {
            self.context.PushLayer(&parameters, None::<&ID2D1Layer>);
        }
        let result = self.draw_opaque_stroke(stroke, minimum_ratio);
        unsafe { self.context.PopLayer() };
        result
    }

    fn draw_shape(&self, stroke: &InkStroke, kind: InkShapeKind) -> Result<()> {
        let Some(first) = stroke.points.first() else {
            return Ok(());
        };
        let Some(last) = stroke.points.last() else {
            return Ok(());
        };
        let size = Size {
            width: f64::from(self.width),
            height: f64::from(self.height),
        };
        let start = InkGeometry::screen_point(first.location, size);
        let end = InkGeometry::screen_point(last.location, size);
        let color = D2D1_COLOR_F {
            r: stroke.color[0] as f32,
            g: stroke.color[1] as f32,
            b: stroke.color[2] as f32,
            a: stroke.color[3] as f32,
        };
        let brush = unsafe { self.context.CreateSolidColorBrush(&color, None) }?;
        let width_scale = px_per_dip(self.dpi);
        let stroke_width = (stroke.width * width_scale) as f32;
        match kind {
            InkShapeKind::Line => unsafe {
                self.context.DrawLine(
                    d2d_point!(start),
                    d2d_point!(end),
                    &brush,
                    stroke_width,
                    &self.round_stroke,
                )
            },
            InkShapeKind::Arrow(style) => {
                let geometry = unsafe { self.factory.CreatePathGeometry() }?;
                let sink = unsafe { geometry.Open() }?;
                unsafe {
                    sink.BeginFigure(d2d_point!(start), D2D1_FIGURE_BEGIN_HOLLOW);
                    sink.AddLine(d2d_point!(end));
                    sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    add_arrow_head(
                        &sink,
                        end,
                        start,
                        (10.0_f64.max(stroke.width * 4.0)) * width_scale,
                    );
                    if style == ArrowStyle::Double {
                        add_arrow_head(
                            &sink,
                            start,
                            end,
                            (10.0_f64.max(stroke.width * 4.0)) * width_scale,
                        );
                    }
                    sink.Close()?;
                    self.context
                        .DrawGeometry(&geometry, &brush, stroke_width, &self.round_stroke);
                }
            }
            InkShapeKind::Rectangle => {
                let rect = standardized_rect(start, end);
                unsafe {
                    self.context
                        .DrawRectangle(&rect, &brush, stroke_width, &self.round_stroke)
                }
            }
            InkShapeKind::Ellipse => {
                let rect = standardized_rect(start, end);
                let ellipse = D2D1_ELLIPSE {
                    point: d2d_point!(Point {
                        x: f64::from(rect.left + rect.right) / 2.0,
                        y: f64::from(rect.top + rect.bottom) / 2.0,
                    }),
                    radiusX: (rect.right - rect.left) / 2.0,
                    radiusY: (rect.bottom - rect.top) / 2.0,
                };
                unsafe {
                    self.context
                        .DrawEllipse(&ellipse, &brush, stroke_width, &self.round_stroke)
                }
            }
        }
        Ok(())
    }

    fn draw_cover(&self, stroke: &InkStroke) -> Result<()> {
        let Some(first) = stroke.points.first() else {
            return Ok(());
        };
        let Some(last) = stroke.points.last() else {
            return Ok(());
        };
        let size = Size {
            width: f64::from(self.width),
            height: f64::from(self.height),
        };
        let rect = standardized_rect(
            InkGeometry::screen_point(first.location, size),
            InkGeometry::screen_point(last.location, size),
        );
        let fill = D2D1_COLOR_F {
            r: COVER_STROKE_COLOR[0] as f32,
            g: COVER_STROKE_COLOR[1] as f32,
            b: COVER_STROKE_COLOR[2] as f32,
            a: COVER_FILL_ALPHA,
        };
        let border = D2D1_COLOR_F {
            a: COVER_BORDER_ALPHA,
            ..Default::default()
        };
        let fill_brush = unsafe { self.context.CreateSolidColorBrush(&fill, None) }?;
        let border_brush = unsafe { self.context.CreateSolidColorBrush(&border, None) }?;
        unsafe {
            self.context.FillRectangle(&rect, &fill_brush);
            self.context.DrawRectangle(
                &rect,
                &border_brush,
                (COVER_BORDER_WIDTH_DIP * px_per_dip(self.dpi)) as f32,
                &self.round_stroke,
            );
        }
        Ok(())
    }

    fn draw_text(&self, stroke: &InkStroke, content: &InkTextContent) -> Result<()> {
        let (Some(first), Some(last)) = (stroke.points.first(), stroke.points.last()) else {
            return Ok(());
        };
        let canvas = Size {
            width: f64::from(self.width),
            height: f64::from(self.height),
        };
        let rect = standardized_rect(
            InkGeometry::screen_point(first.location, canvas),
            InkGeometry::screen_point(last.location, canvas),
        );
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return Ok(());
        }
        let format = unsafe {
            self.dwrite.CreateTextFormat(
                windows::core::w!("Segoe UI"),
                None::<&IDWriteFontCollection>,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                (content.font_size * px_per_dip(self.dpi)) as f32,
                windows::core::w!("ko-KR"),
            )
        }?;
        unsafe {
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
        }
        let color = D2D1_COLOR_F {
            r: stroke.color[0] as f32,
            g: stroke.color[1] as f32,
            b: stroke.color[2] as f32,
            a: stroke.color[3] as f32,
        };
        let brush = unsafe { self.context.CreateSolidColorBrush(&color, None) }?;
        let target: ID2D1RenderTarget = self.context.cast()?;
        let text = content.string.encode_utf16().collect::<Vec<_>>();
        unsafe {
            target.DrawText(
                &text,
                &format,
                &rect,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    fn draw_opaque_stroke(&self, stroke: &InkStroke, minimum_ratio: f64) -> Result<()> {
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
        let width_scale = px_per_dip(self.dpi);
        if samples.len() == 1 {
            let width = stroke.effective_width(first.1, minimum_ratio) * width_scale;
            if stroke.tool == InkStrokeTool::Highlighter {
                let half = (width / 2.0) as f32;
                let rect = D2D_RECT_F {
                    left: first.0.x as f32 - half,
                    top: first.0.y as f32 - half,
                    right: first.0.x as f32 + half,
                    bottom: first.0.y as f32 + half,
                };
                unsafe { self.context.FillRectangle(&rect, &brush) };
                return Ok(());
            }
            let radius = (width / 2.0) as f32;
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
                    (stroke.width * width_scale) as f32,
                    if stroke.tool == InkStrokeTool::Highlighter {
                        &self.butt_stroke
                    } else {
                        &self.round_stroke
                    },
                );
            }
            return Ok(());
        }

        let mut scaled_stroke = stroke.clone();
        scaled_stroke.width *= width_scale;
        let commands = InkGeometry::variable_width_path(&scaled_stroke, &samples, minimum_ratio);
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
        self.rebuild_completed(&[], 0.2, 1.0)
    }

    fn completed_bitmap(&self) -> &ID2D1Bitmap1 {
        self.completed_bitmap
            .as_ref()
            .expect("completed bitmap exists outside resize")
    }

    fn set_target(&self, bitmap: &ID2D1Bitmap1) -> Result<()> {
        let image: ID2D1Image = bitmap.cast()?;
        unsafe { self.context.SetTarget(&image) };
        Ok(())
    }

    fn draw_strokes_to_bitmap(
        &self,
        bitmap: &ID2D1Bitmap1,
        strokes: &[InkStroke],
        minimum_ratio: f64,
        highlighter_opacity: f64,
        clear: bool,
    ) -> Result<()> {
        self.set_target(bitmap)?;
        unsafe {
            self.context.BeginDraw();
            if clear {
                self.context.Clear(Some(&D2D1_COLOR_F::default()));
            }
        }
        let draw_result: Result<()> = (|| {
            for stroke in strokes {
                self.draw_stroke(stroke, minimum_ratio, highlighter_opacity)?;
            }
            Ok(())
        })();
        let end_result = unsafe { self.context.EndDraw(None, None) };
        draw_result?;
        end_result
    }

    fn finish_offscreen_draw<T>(&self, result: Result<T>) -> Result<T> {
        let restore_result = self.restore_swap_target();
        match (result, restore_result) {
            (Err(error), _) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn clear_bitmap(&self, bitmap: &ID2D1Bitmap1, color: &D2D1_COLOR_F) -> Result<()> {
        self.set_target(bitmap)?;
        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(color));
            self.context.EndDraw(None, None)
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

fn frame_bounds(width: u32, height: u32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: width as f32,
        bottom: height as f32,
    }
}

fn d2d_color(components: [f64; 4]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: components[0] as f32,
        g: components[1] as f32,
        b: components[2] as f32,
        a: components[3] as f32,
    }
}

fn opacity_layer_parameters(bounds: D2D_RECT_F, opacity: f32) -> D2D1_LAYER_PARAMETERS1 {
    let mut parameters = D2D1_LAYER_PARAMETERS1 {
        contentBounds: bounds,
        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        opacity,
        layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        ..Default::default()
    };
    parameters.maskTransform.M11 = 1.0;
    parameters.maskTransform.M22 = 1.0;
    parameters
}

fn bytes_of<T>(value: &T) -> &[u8] {
    // SAFETY: the returned byte view is limited to value's lifetime and exact initialized size.
    unsafe {
        std::slice::from_raw_parts((value as *const T).cast::<u8>(), std::mem::size_of::<T>())
    }
}

fn standardized_rect(first: Point, second: Point) -> D2D_RECT_F {
    D2D_RECT_F {
        left: first.x.min(second.x) as f32,
        top: first.y.min(second.y) as f32,
        right: first.x.max(second.x) as f32,
        bottom: first.y.max(second.y) as f32,
    }
}

fn add_arrow_head(sink: &ID2D1GeometrySink, tip: Point, tail: Point, length: f64) {
    let angle = (tip.y - tail.y).atan2(tip.x - tail.x);
    let spread = std::f64::consts::PI / 7.0;
    for head_angle in [angle - spread, angle + spread] {
        let endpoint = Point {
            x: tip.x - head_angle.cos() * length,
            y: tip.y - head_angle.sin() * length,
        };
        unsafe {
            sink.BeginFigure(d2d_point!(tip), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddLine(d2d_point!(endpoint));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
        }
    }
}

fn transparency_probe_points(width: u32, height: u32) -> [(u32, u32); 6] {
    [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
        (width / 2, height - 1),
        (width / 2, height / 2),
    ]
}

fn selftest_stroke(name: &str, tool: InkStrokeTool, width: f64) -> InkStroke {
    InkStroke::new(
        format!("selftest-{name}"),
        tool,
        [1.0, 0.8, 0.1, 1.0],
        width,
        false,
        vec![
            InkPoint::new(Point { x: 0.25, y: 0.25 }, 1.0, 0.0),
            InkPoint::new(Point { x: 0.75, y: 0.25 }, 1.0, 1.0),
        ],
        InkStrokeContent::Freehand,
    )
}

fn selftest_vector_stroke(
    name: &str,
    content: InkStrokeContent,
    start: Point,
    end: Point,
    width: f64,
) -> InkStroke {
    let tool = if matches!(&content, InkStrokeContent::Cover) {
        InkStrokeTool::Cover
    } else {
        InkStrokeTool::Shape
    };
    InkStroke::new(
        format!("selftest-{name}"),
        tool,
        [1.0, 0.8, 0.1, 1.0],
        width,
        false,
        vec![InkPoint::new(start, 1.0, 0.0), InkPoint::new(end, 1.0, 1.0)],
        content,
    )
}

fn read_pixel_samples(
    context: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
    points: &[(u32, u32)],
) -> Result<Vec<PixelSample>> {
    let size = unsafe { bitmap.GetPixelSize() };
    if points
        .iter()
        .any(|(x, y)| *x >= size.width || *y >= size.height)
    {
        return Err(Error::new(
            E_INVALIDARG,
            "pixel sample is outside the bitmap",
        ));
    }
    let (pixels, pitch) = read_bitmap_bgra(context, bitmap)?;

    let samples = points
        .iter()
        .map(|&(x, y)| {
            let offset = y as usize * pitch as usize + x as usize * 4;
            let pixel = &pixels[offset..offset + 4];
            PixelSample {
                x,
                y,
                bgra: [pixel[0], pixel[1], pixel[2], pixel[3]],
            }
        })
        .collect();
    Ok(samples)
}

fn alpha_counts_in_rect(
    context: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
    rect: D2D_RECT_F,
) -> Result<(usize, usize)> {
    let size = unsafe { bitmap.GetPixelSize() };
    let (pixels, pitch) = read_bitmap_bgra(context, bitmap)?;
    let mut inside = 0;
    let mut outside = 0;
    for y in 0..size.height {
        for x in 0..size.width {
            let offset = y as usize * pitch as usize + x as usize * 4;
            if pixels[offset + 3] == 0 {
                continue;
            }
            let is_inside = (x as f32) >= rect.left
                && (x as f32) < rect.right
                && (y as f32) >= rect.top
                && (y as f32) < rect.bottom;
            if is_inside {
                inside += 1;
            } else {
                outside += 1;
            }
        }
    }
    Ok((inside, outside))
}

fn pixel_count_in_rect(
    context: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
    rect: D2D_RECT_F,
    predicate: impl Fn([u8; 4]) -> bool,
) -> Result<usize> {
    let size = unsafe { bitmap.GetPixelSize() };
    let (pixels, pitch) = read_bitmap_bgra(context, bitmap)?;
    let left = rect.left.max(0.0).floor() as u32;
    let top = rect.top.max(0.0).floor() as u32;
    let right = rect.right.max(0.0).ceil().min(size.width as f32) as u32;
    let bottom = rect.bottom.max(0.0).ceil().min(size.height as f32) as u32;
    let mut count = 0;
    for y in top..bottom {
        for x in left..right {
            let offset = y as usize * pitch as usize + x as usize * 4;
            if predicate([
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            ]) {
                count += 1;
            }
        }
    }
    Ok(count)
}

pub(crate) fn count_nonzero_alpha(
    context: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
) -> Result<usize> {
    let size = unsafe { bitmap.GetPixelSize() };
    let (pixels, pitch) = read_bitmap_bgra(context, bitmap)?;
    let row_bytes = usize::try_from(size.width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(|| Error::new(E_FAIL, "bitmap row size overflow"))?;
    let pitch = pitch as usize;
    Ok((0..size.height as usize)
        .map(|row| &pixels[row * pitch..row * pitch + row_bytes])
        .flat_map(|row| row.chunks_exact(4))
        .filter(|pixel| pixel[3] != 0)
        .count())
}

pub(crate) fn read_bitmap_bgra(
    context: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
) -> Result<(Vec<u8>, u32)> {
    let size = unsafe { bitmap.GetPixelSize() };
    let staging = create_cpu_read_bitmap(context, size.width, size.height)?;
    unsafe { staging.CopyFromBitmap(None, bitmap, None) }?;
    let mapped = unsafe { staging.Map(D2D1_MAP_OPTIONS_READ) }?;
    if mapped.bits.is_null() {
        let _ = unsafe { staging.Unmap() };
        return Err(Error::new(E_FAIL, "mapped bitmap returned a null pointer"));
    }
    let byte_count = (mapped.pitch as usize)
        .checked_mul(size.height as usize)
        .ok_or_else(|| Error::new(E_FAIL, "mapped bitmap size overflow"));
    let pixels = match byte_count {
        Ok(byte_count) => unsafe { std::slice::from_raw_parts(mapped.bits, byte_count) }.to_vec(),
        Err(error) => {
            let _ = unsafe { staging.Unmap() };
            return Err(error);
        }
    };
    unsafe { staging.Unmap() }?;
    Ok((pixels, mapped.pitch))
}

fn check_transparent_samples(name: &str, samples: &[PixelSample], failures: &mut Vec<String>) {
    for sample in samples {
        if sample.bgra[3] != 0 {
            failures.push(format!(
                "{name}: 획 밖 ({}, {}) BGRA={:?}, alpha가 0이 아님",
                sample.x, sample.y, sample.bgra
            ));
        }
    }
}

pub fn px_per_dip(dpi: u32) -> f64 {
    if dpi == 0 {
        1.0
    } else {
        f64::from(dpi) / f64::from(TARGET_DPI)
    }
}

pub(crate) fn create_swap_chain(
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

pub(crate) fn create_swap_target(
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

fn create_cpu_read_bitmap(
    context: &ID2D1DeviceContext,
    width: u32,
    height: u32,
) -> Result<ID2D1Bitmap1> {
    let properties = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: pixel_format(),
        dpiX: TARGET_DPI,
        dpiY: TARGET_DPI,
        bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };
    unsafe { context.CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &properties) }
}

pub(crate) fn pixel_format() -> D2D1_PIXEL_FORMAT {
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
