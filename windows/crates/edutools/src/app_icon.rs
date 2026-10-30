use crate::attr_icon_input::AttrIconVisualState;
use crate::log::Logger;
use crate::render::{count_nonzero_alpha, read_bitmap_bgra, GraphicsDevice};
use edutools_core::attr_icon::{AttrIconColor, AttrIconConfiguration, AttrIconPaint, AttrIconPure};
use edutools_core::shortcut::ShortcutCatalog;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::Once;
use windows::core::{w, Error, Interface, Result, BOOL, PCWSTR};
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, E_FAIL, GENERIC_WRITE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_F,
    D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Bitmap1, ID2D1DeviceContext, ID2D1DeviceContext5, ID2D1Image, ID2D1RenderTarget,
    ID2D1SvgDocument, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_PRIMITIVE_BLEND_SOURCE_OVER,
    D2D1_SVG_ATTRIBUTE_STRING_TYPE_SVG,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICBitmapFrameEncode, IWICImagingFactory,
    WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

include!(concat!(env!("OUT_DIR"), "/icons_embed.rs"));

const FIXED_KEYS: &[&str] = &[
    "group.writing",
    "group.shapes",
    "group.erasing",
    "group.board",
    "group.presenting",
    "group.favorites",
    "option.on",
    "option.off",
    "option.arrow.single",
    "option.arrow.double",
    "option.area.rectangle",
    "option.area.lasso",
    "status.annotation.on",
    "status.annotation.off",
];

const LOGICAL_ALIASES: &[(&str, &str)] = &[("tool.eraser-toggle", "tool.eraser")];

#[derive(Clone, Copy, Debug)]
pub struct IconTint {
    pub color: D2D1_COLOR_F,
}

impl IconTint {
    pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            color: D2D1_COLOR_F {
                r: red,
                g: green,
                b: blue,
                a: alpha,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionKind {
    Direct,
    SymbolMap,
    OsSource,
    Alias,
    Fallback,
    Missing,
}

#[derive(Clone, Copy)]
struct EmbeddedAsset {
    key: &'static str,
    bytes: &'static [u8],
    source: &'static str,
    os_override: bool,
}

#[derive(Clone, Copy)]
struct EmbeddedAttrAsset {
    key: &'static str,
    bytes: &'static [u8],
    source: &'static str,
}

#[derive(Clone, Copy)]
struct FallbackGlyph {
    family: &'static str,
    glyph: char,
    label: &'static str,
}

#[derive(Clone, Copy)]
struct Resolution {
    kind: ResolutionKind,
    asset: Option<EmbeddedAsset>,
    fallback: Option<FallbackGlyph>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SvgTintMethod {
    RootColor,
    ReplacedCurrentColor,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SvgCacheKey {
    key: &'static str,
    color: Option<u32>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AttrSvgCacheKey {
    signature: String,
    key: &'static str,
    color: u32,
    dpi: u32,
    visual_state: AttrIconVisualState,
}

#[derive(Clone, Copy, Debug)]
struct TintProbe {
    root_color_pixels: usize,
    replacement_pixels: Option<usize>,
}

pub struct AppIconRenderer {
    context: ID2D1DeviceContext,
    context5: ID2D1DeviceContext5,
    dwrite: IDWriteFactory,
    cache: HashMap<SvgCacheKey, ID2D1SvgDocument>,
    attr_cache: HashMap<AttrSvgCacheKey, ID2D1SvgDocument>,
    missing: BTreeSet<String>,
    attr_failures: BTreeSet<String>,
    logged_fallbacks: BTreeSet<String>,
    fluent_available: bool,
    tint_method: SvgTintMethod,
    tint_probe: TintProbe,
    logger: Logger,
}

impl AppIconRenderer {
    pub fn new(context: ID2D1DeviceContext, logger: Logger) -> Result<Self> {
        let context5 = context.cast::<ID2D1DeviceContext5>()?;
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let fluent_available =
            font_available(&dwrite, w!("Segoe Fluent Icons")).unwrap_or_else(|error| {
                logger.event(&format!(
                    "아이콘 warn: Segoe Fluent Icons 확인 실패; ? 글리프로 전환: {error}"
                ));
                false
            });
        if !fluent_available {
            logger.event("아이콘 warn: Segoe Fluent Icons 없음; 폴백은 Segoe UI의 ? 글리프 사용");
        }
        let mut renderer = Self {
            context,
            context5,
            dwrite,
            cache: HashMap::new(),
            attr_cache: HashMap::new(),
            missing: BTreeSet::new(),
            attr_failures: BTreeSet::new(),
            logged_fallbacks: BTreeSet::new(),
            fluent_available,
            tint_method: SvgTintMethod::RootColor,
            tint_probe: TintProbe {
                root_color_pixels: 0,
                replacement_pixels: None,
            },
            logger,
        };
        renderer.select_tint_method()?;
        Ok(renderer)
    }

    pub fn audit_startup(logger: &Logger) {
        static AUDIT: Once = Once::new();
        AUDIT.call_once(|| {
            for definition in ShortcutCatalog::definitions() {
                let resolution = resolve_key(&definition.id, false);
                match resolution.kind {
                    ResolutionKind::Fallback => logger.event(&format!(
                        "아이콘 info: SVG 없는 카탈로그 키 폴백: id={}, icon_key={} ({})",
                        definition.id,
                        definition.icon_key,
                        resolution.fallback.map_or("?", |fallback| fallback.label)
                    )),
                    ResolutionKind::Missing => logger.event(&format!(
                        "아이콘 warn: 해석 실패 카탈로그 키: id={}, icon_key={}",
                        definition.id, definition.icon_key
                    )),
                    _ => {}
                }
            }
            for key in FIXED_KEYS {
                if resolve_key(key, false).kind == ResolutionKind::Missing {
                    logger.event(&format!("아이콘 warn: 해석 실패 고정/원형 키: {key}"));
                }
            }
            logger.event("아이콘 info: currentColor 틴트 방식은 런타임 픽셀 probe로 선택");
            match high_contrast() {
                Ok(enabled) => logger.event(&format!(
                    "아이콘 info: 시작 고대비 상태={enabled}; 호출자가 시스템 색을 IconTint로 전달"
                )),
                Err(error) => {
                    logger.event(&format!("아이콘 warn: 시작 고대비 상태 조회 실패: {error}"))
                }
            }
        });
    }

    pub fn draw(&mut self, key: &str, rect: D2D_RECT_F, tint: IconTint) -> Result<()> {
        self.draw_selected(key, false, rect, tint)
    }

    pub fn draw_selected(
        &mut self,
        key: &str,
        selected: bool,
        rect: D2D_RECT_F,
        tint: IconTint,
    ) -> Result<()> {
        let resolution = resolve_key(key, selected);
        if let Some(asset) = resolution.asset {
            match self.draw_svg(asset, rect, tint) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    self.handle_possible_recreate_target(&error);
                    self.logger.event(&format!(
                        "아이콘 warn: SVG 디코딩/그리기 실패 key={key}, source={}: {error}; ? 글리프 사용",
                        asset.source
                    ));
                    return self.draw_text_fallback(rect, tint, '?', false);
                }
            }
        }
        if let Some(fallback) = resolution.fallback {
            if self.logged_fallbacks.insert(key.to_owned()) {
                self.logger.event(&format!(
                    "아이콘 info: 폴백 글리프 사용 key={key}, glyph=U+{:04X} ({})",
                    u32::from(fallback.glyph),
                    fallback.label
                ));
            }
            let use_fluent = fallback.family == "Segoe Fluent Icons" && self.fluent_available;
            let glyph = if use_fluent { fallback.glyph } else { '?' };
            return self.draw_text_fallback(rect, tint, glyph, use_fluent);
        }

        if self.missing.insert(key.to_owned()) {
            self.logger.event(&format!(
                "아이콘 warn: 완전 해석 실패 key={key}; ? 글리프 사용"
            ));
        }
        self.draw_text_fallback(rect, tint, '?', false)
    }

    pub fn draw_attr_icon(
        &mut self,
        configuration: &AttrIconConfiguration,
        ui_color: IconTint,
        background_color: IconTint,
        rect: D2D_RECT_F,
        dpi: u32,
        visual_state: AttrIconVisualState,
    ) -> Result<()> {
        let ui = attr_color(ui_color);
        let background = attr_color(background_color);
        let layers = match AttrIconPure::layers(ATTR_ICON_MANIFEST, configuration, ui, background) {
            Ok(layers) => layers,
            Err(error) => {
                self.log_attr_failure_once(
                    format!("layers.{}", configuration.tool_key),
                    format!(
                        "속성 아이콘 레이어 해석 실패 tool={}: {error}; 일반 아이콘 사용",
                        configuration.tool_key
                    ),
                );
                return self.draw(&configuration.tool_key, rect, ui_color);
            }
        };
        let visible_layers = layers
            .iter()
            .filter(|layer| !visual_state.is_high_contrast || layer.paint != AttrIconPaint::Attr)
            .collect::<Vec<_>>();
        let layer_signature = format!(
            "{}|{}|bg={:08X}",
            configuration.tool_key,
            visible_layers
                .iter()
                .map(|layer| layer.name.as_str())
                .collect::<Vec<_>>()
                .join(","),
            tint_rgba(background_color),
        );
        let halo_offset = 0.5 * dpi.max(1) as f32 / 96.0;
        let draw_result = (|| -> Result<()> {
            for layer in visible_layers {
                let asset = embedded_attr_asset(&layer.name).ok_or_else(|| {
                    Error::new(
                        E_FAIL,
                        format!("embedded attr SVG is missing: {}", layer.name),
                    )
                })?;
                if let Some(halo) = layer.halo {
                    let tint = icon_tint(halo);
                    for (x, y) in [
                        (-halo_offset, 0.0),
                        (halo_offset, 0.0),
                        (0.0, -halo_offset),
                        (0.0, halo_offset),
                    ] {
                        self.draw_attr_svg(
                            asset,
                            offset_rect(rect, x, y),
                            tint,
                            &layer_signature,
                            dpi,
                            visual_state,
                        )?;
                    }
                }
                let tint = match layer.paint {
                    AttrIconPaint::Ui => ui_color,
                    AttrIconPaint::Attr => icon_tint(
                        configuration
                            .color
                            .expect("core requires color for every attr-painted layer"),
                    ),
                };
                self.draw_attr_svg(asset, rect, tint, &layer_signature, dpi, visual_state)?;
            }
            Ok(())
        })();
        if let Err(error) = draw_result {
            self.handle_possible_recreate_target(&error);
            self.log_attr_failure_once(
                format!("draw.{}", configuration.tool_key),
                format!(
                    "속성 아이콘 SVG 파싱/그리기 실패 tool={}: {error}; 일반 아이콘 사용",
                    configuration.tool_key
                ),
            );
            return self.draw(&configuration.tool_key, rect, ui_color);
        }
        Ok(())
    }

    pub fn missing_keys(&self) -> Vec<String> {
        self.missing.iter().cloned().collect()
    }

    pub(crate) fn tint_report(&self) -> String {
        match self.tint_probe.replacement_pixels {
            Some(pixels) => format!(
                "방식 1 픽셀 {} → 방식 2 전환 (픽셀 {pixels}, SVG currentColor 치환)",
                self.tint_probe.root_color_pixels
            ),
            None => format!(
                "방식 1 선택 (픽셀 {}, 루트 svg color 속성)",
                self.tint_probe.root_color_pixels
            ),
        }
    }

    pub(crate) fn icon_alpha_count(&mut self, key: &str, tint: IconTint) -> Result<usize> {
        let asset = resolve_key(key, false).asset.ok_or_else(|| {
            Error::new(
                E_FAIL,
                format!("icon pixel probe requires an embedded SVG: {key}"),
            )
        })?;
        self.render_icon_alpha_count(asset, tint)
    }

    pub fn handle_render_result(&mut self, result: Result<()>) -> Result<()> {
        if let Err(error) = &result {
            self.handle_possible_recreate_target(error);
        }
        result
    }

    fn draw_svg(&mut self, asset: EmbeddedAsset, rect: D2D_RECT_F, tint: IconTint) -> Result<()> {
        let rgb = tint_rgb(tint);
        let cache_key = SvgCacheKey {
            key: asset.key,
            color: match self.tint_method {
                SvgTintMethod::RootColor => None,
                SvgTintMethod::ReplacedCurrentColor => Some(rgb),
            },
        };
        if !self.cache.contains_key(&cache_key) {
            let replaced;
            let svg = match self.tint_method {
                SvgTintMethod::RootColor => asset.bytes,
                SvgTintMethod::ReplacedCurrentColor => {
                    let source = std::str::from_utf8(asset.bytes).map_err(|error| {
                        Error::new(
                            E_FAIL,
                            format!("{} SVG is not UTF-8: {error}", asset.source),
                        )
                    })?;
                    replaced = source.replace("currentColor", &color_hex(rgb));
                    replaced.as_bytes()
                }
            };
            let stream = unsafe { SHCreateMemStream(Some(svg)) }
                .ok_or_else(|| Error::new(E_FAIL, "SHCreateMemStream returned null"))?;
            let document = unsafe {
                self.context5.CreateSvgDocument(
                    &stream,
                    D2D_SIZE_F {
                        width: 24.0,
                        height: 24.0,
                    },
                )
            }?;
            self.cache.insert(cache_key, document);
            if asset.os_override {
                self.logger.event(&format!(
                    "아이콘 info: Windows OS 예외 적용 key={}, source={}",
                    asset.key, asset.source
                ));
            }
        }
        let document = self
            .cache
            .get(&cache_key)
            .expect("SVG document was inserted above")
            .clone();
        let root = unsafe { document.GetRoot() }?;
        if self.tint_method == SvgTintMethod::RootColor {
            let color = wide(&color_hex(rgb));
            unsafe {
                root.SetAttributeValue3(
                    w!("color"),
                    D2D1_SVG_ATTRIBUTE_STRING_TYPE_SVG,
                    PCWSTR(color.as_ptr()),
                )?;
            }
        }
        let opacity = format!("{:.4}", tint.color.a.clamp(0.0, 1.0));
        let opacity = wide(&opacity);
        unsafe {
            root.SetAttributeValue3(
                w!("opacity"),
                D2D1_SVG_ATTRIBUTE_STRING_TYPE_SVG,
                PCWSTR(opacity.as_ptr()),
            )?;
        }

        let transform = icon_transform(&self.context, rect);
        let mut previous = D2D1_BRUSH_PROPERTIES::default();
        unsafe {
            self.context.GetTransform(&mut previous.transform);
            self.context.SetTransform(&transform.transform);
            self.context5.DrawSvgDocument(&document);
            self.context.SetTransform(&previous.transform);
        }
        Ok(())
    }

    fn draw_attr_svg(
        &mut self,
        asset: EmbeddedAttrAsset,
        rect: D2D_RECT_F,
        tint: IconTint,
        signature: &str,
        dpi: u32,
        visual_state: AttrIconVisualState,
    ) -> Result<()> {
        let rgb = tint_rgb(tint);
        let cache_key = AttrSvgCacheKey {
            signature: signature.to_owned(),
            key: asset.key,
            color: tint_rgba(tint),
            dpi,
            visual_state,
        };
        if !self.attr_cache.contains_key(&cache_key) {
            let replaced;
            let svg = match self.tint_method {
                SvgTintMethod::RootColor => asset.bytes,
                SvgTintMethod::ReplacedCurrentColor => {
                    let source = std::str::from_utf8(asset.bytes).map_err(|error| {
                        Error::new(
                            E_FAIL,
                            format!("{} SVG is not UTF-8: {error}", asset.source),
                        )
                    })?;
                    replaced = source.replace("currentColor", &color_hex(rgb));
                    replaced.as_bytes()
                }
            };
            let stream = unsafe { SHCreateMemStream(Some(svg)) }
                .ok_or_else(|| Error::new(E_FAIL, "SHCreateMemStream returned null"))?;
            let document = unsafe {
                self.context5.CreateSvgDocument(
                    &stream,
                    D2D_SIZE_F {
                        width: 24.0,
                        height: 24.0,
                    },
                )
            }?;
            self.attr_cache.insert(cache_key.clone(), document);
        }
        let document = self
            .attr_cache
            .get(&cache_key)
            .expect("attr SVG document was inserted above")
            .clone();
        let root = unsafe { document.GetRoot() }?;
        if self.tint_method == SvgTintMethod::RootColor {
            let color = wide(&color_hex(rgb));
            unsafe {
                root.SetAttributeValue3(
                    w!("color"),
                    D2D1_SVG_ATTRIBUTE_STRING_TYPE_SVG,
                    PCWSTR(color.as_ptr()),
                )?;
            }
        }
        let opacity = wide(&format!("{:.4}", tint.color.a.clamp(0.0, 1.0)));
        unsafe {
            root.SetAttributeValue3(
                w!("opacity"),
                D2D1_SVG_ATTRIBUTE_STRING_TYPE_SVG,
                PCWSTR(opacity.as_ptr()),
            )?;
        }
        let transform = icon_transform(&self.context, rect);
        let mut previous = D2D1_BRUSH_PROPERTIES::default();
        unsafe {
            self.context.GetTransform(&mut previous.transform);
            self.context.SetTransform(&transform.transform);
            self.context5.DrawSvgDocument(&document);
            self.context.SetTransform(&previous.transform);
        }
        Ok(())
    }

    fn log_attr_failure_once(&mut self, key: String, message: String) {
        if self.attr_failures.insert(key) {
            self.logger.event(&format!("아이콘 warn: {message}"));
        }
    }

    fn draw_text_fallback(
        &self,
        rect: D2D_RECT_F,
        tint: IconTint,
        glyph: char,
        fluent: bool,
    ) -> Result<()> {
        let family = if fluent {
            w!("Segoe Fluent Icons")
        } else {
            w!("Segoe UI")
        };
        let font_size = ((rect.bottom - rect.top).min(rect.right - rect.left) * 0.78).max(1.0);
        let format = unsafe {
            self.dwrite.CreateTextFormat(
                family,
                None::<&IDWriteFontCollection>,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                w!("ko-KR"),
            )
        }?;
        unsafe {
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        let brush = unsafe { self.context.CreateSolidColorBrush(&tint.color, None) }?;
        let target: ID2D1RenderTarget = self.context.cast()?;
        let text = glyph.encode_utf16(&mut [0; 2]).to_vec();
        unsafe {
            target.DrawText(
                &text,
                &format,
                &rect,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }

    fn select_tint_method(&mut self) -> Result<()> {
        let probe = embedded_asset("tool.arrow")
            .expect("tool.arrow is part of the build-time validated icon set");
        let tint = IconTint::rgba(1.0, 1.0, 1.0, 1.0);
        let root_color_pixels = self.render_icon_alpha_count(probe, tint)?;
        self.tint_probe.root_color_pixels = root_color_pixels;
        if root_color_pixels > 0 {
            self.logger.event(&format!(
                "아이콘 info: currentColor 방식 1 선택; key=tool.arrow px={root_color_pixels}"
            ));
            return Ok(());
        }

        self.logger
            .event("아이콘 warn: 방식 1 픽셀 0 → 방식 2 전환; key=tool.arrow");
        self.tint_method = SvgTintMethod::ReplacedCurrentColor;
        self.cache.clear();
        let replacement_pixels = self.render_icon_alpha_count(probe, tint)?;
        self.tint_probe.replacement_pixels = Some(replacement_pixels);
        if replacement_pixels == 0 {
            self.logger.event(
                "아이콘 error: icon tool.arrow px=0; 방식 1(root color) 및 방식 2(currentColor 치환) 모두 실패",
            );
            return Ok(());
        }
        self.logger.event(&format!(
            "아이콘 info: currentColor 방식 2 선택; key=tool.arrow px={replacement_pixels}"
        ));
        Ok(())
    }

    fn render_icon_alpha_count(&mut self, asset: EmbeddedAsset, tint: IconTint) -> Result<usize> {
        let mut dpi_x = 96.0;
        let mut dpi_y = 96.0;
        unsafe { self.context.GetDpi(&mut dpi_x, &mut dpi_y) };
        let width = (24.0 * dpi_x / 96.0).round().max(1.0) as u32;
        let height = (24.0 * dpi_y / 96.0).round().max(1.0) as u32;
        let bitmap = create_target_bitmap(&self.context, width, height, dpi_x, dpi_y)?;
        let target: ID2D1Image = bitmap.cast()?;
        let previous_target = unsafe { self.context.GetTarget() }?;
        let previous_blend = unsafe { self.context.GetPrimitiveBlend() };
        unsafe {
            self.context.SetTarget(&target);
            self.context
                .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            self.context.BeginDraw();
            self.context.Clear(Some(&D2D1_COLOR_F::default()));
        }
        let draw_result = self.draw_svg(
            asset,
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: 24.0,
                bottom: 24.0,
            },
            tint,
        );
        let end_result = unsafe { self.context.EndDraw(None, None) };
        unsafe {
            self.context.SetTarget(&previous_target);
            self.context.SetPrimitiveBlend(previous_blend);
        }
        draw_result?;
        self.handle_render_result(end_result)?;
        count_nonzero_alpha(&self.context, &bitmap)
    }

    fn render_attr_asset_alpha_count(&mut self, asset: EmbeddedAttrAsset) -> Result<usize> {
        let bitmap = create_target_bitmap(&self.context, 24, 24, 96.0, 96.0)?;
        let target: ID2D1Image = bitmap.cast()?;
        let previous_target = unsafe { self.context.GetTarget() }?;
        let previous_blend = unsafe { self.context.GetPrimitiveBlend() };
        unsafe {
            self.context.SetTarget(&target);
            self.context
                .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            self.context.BeginDraw();
            self.context.Clear(Some(&D2D1_COLOR_F::default()));
        }
        let visual_state = AttrIconVisualState {
            is_selected: false,
            is_enabled: true,
            is_hovered: false,
            is_pass_through: false,
            is_high_contrast: false,
        };
        let draw_result = self.draw_attr_svg(
            asset,
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: 24.0,
                bottom: 24.0,
            },
            IconTint::rgba(1.0, 1.0, 1.0, 1.0),
            &format!("selftest|{}", asset.key),
            96,
            visual_state,
        );
        let end_result = unsafe { self.context.EndDraw(None, None) };
        unsafe {
            self.context.SetTarget(&previous_target);
            self.context.SetPrimitiveBlend(previous_blend);
        }
        draw_result?;
        self.handle_render_result(end_result)?;
        count_nonzero_alpha(&self.context, &bitmap)
    }

    fn render_attr_icon_alpha_count(
        &mut self,
        configuration: &AttrIconConfiguration,
        high_contrast: bool,
    ) -> Result<usize> {
        let bitmap = create_target_bitmap(&self.context, 24, 24, 96.0, 96.0)?;
        let target: ID2D1Image = bitmap.cast()?;
        let previous_target = unsafe { self.context.GetTarget() }?;
        let previous_blend = unsafe { self.context.GetPrimitiveBlend() };
        unsafe {
            self.context.SetTarget(&target);
            self.context
                .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            self.context.BeginDraw();
            self.context.Clear(Some(&D2D1_COLOR_F::default()));
        }
        let draw_result = self.draw_attr_icon(
            configuration,
            IconTint::rgba(1.0, 1.0, 1.0, 1.0),
            IconTint::rgba(0.07, 0.07, 0.07, 1.0),
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: 24.0,
                bottom: 24.0,
            },
            96,
            AttrIconVisualState {
                is_selected: false,
                is_enabled: true,
                is_hovered: false,
                is_pass_through: false,
                is_high_contrast: high_contrast,
            },
        );
        let end_result = unsafe { self.context.EndDraw(None, None) };
        unsafe {
            self.context.SetTarget(&previous_target);
            self.context.SetPrimitiveBlend(previous_blend);
        }
        draw_result?;
        self.handle_render_result(end_result)?;
        count_nonzero_alpha(&self.context, &bitmap)
    }

    fn handle_possible_recreate_target(&mut self, error: &Error) {
        if error.code() == D2DERR_RECREATE_TARGET {
            self.cache.clear();
            self.attr_cache.clear();
            self.logger
                .event("아이콘 info: D2DERR_RECREATE_TARGET 수신; SVG 캐시 비움");
        }
    }
}

pub fn high_contrast() -> Result<bool> {
    let mut value = HIGHCONTRASTW {
        cbSize: u32::try_from(size_of::<HIGHCONTRASTW>()).expect("HIGHCONTRASTW size fits in u32"),
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            value.cbSize,
            Some((&mut value as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }
    Ok(value.dwFlags & HCF_HIGHCONTRASTON == HCF_HIGHCONTRASTON)
}

pub fn embedded_keys() -> impl Iterator<Item = &'static str> {
    EMBEDDED_ICONS.iter().map(|(key, _, _, _)| *key)
}

pub fn run_selftest_icons(output_directory: &Path, logger: &Logger) -> Result<()> {
    std::fs::create_dir_all(output_directory).map_err(|error| {
        Error::new(
            E_FAIL,
            format!(
                "selftest icon output directory {}: {error}",
                output_directory.display()
            ),
        )
    })?;
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    let result = run_selftest_icons_initialized(output_directory, logger);
    unsafe { CoUninitialize() };
    result
}

pub fn run_selftest_attr_icons(logger: &Logger) -> Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    let result = (|| -> Result<()> {
        let graphics = GraphicsDevice::new()?;
        let context = graphics.create_d2d_context()?;
        unsafe { context.SetDpi(96.0, 96.0) };
        let bootstrap = create_target_bitmap(&context, 24, 24, 96.0, 96.0)?;
        let bootstrap_image: ID2D1Image = bootstrap.cast()?;
        unsafe { context.SetTarget(&bootstrap_image) };
        let mut renderer = AppIconRenderer::new(context, logger.clone())?;
        verify_attr_icons(&mut renderer)
    })();
    unsafe { CoUninitialize() };
    result
}

fn run_selftest_icons_initialized(output_directory: &Path, logger: &Logger) -> Result<()> {
    let graphics = GraphicsDevice::new()?;
    let factory: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }?;
    let context = graphics.create_d2d_context()?;
    unsafe { context.SetDpi(96.0, 96.0) };
    let bootstrap = create_target_bitmap(&context, 24, 24, 96.0, 96.0)?;
    let bootstrap_image: ID2D1Image = bootstrap.cast()?;
    unsafe { context.SetTarget(&bootstrap_image) };
    let mut renderer = AppIconRenderer::new(context.clone(), logger.clone())?;
    println!("currentColor: {}", renderer.tint_report());

    let mut empty = Vec::new();
    for key in embedded_keys() {
        let pixels = renderer.icon_alpha_count(key, IconTint::rgba(1.0, 1.0, 1.0, 1.0))?;
        println!("icon {key} px={pixels}");
        if pixels == 0 {
            empty.push(key);
        }
    }
    if !empty.is_empty() {
        return Err(Error::new(
            E_FAIL,
            format!(
                "alpha>0 pixel count is zero: {}; {}",
                empty.join(", "),
                renderer.tint_report()
            ),
        ));
    }

    verify_attr_icons(&mut renderer)?;

    for dpi in [96u32, 144, 240] {
        render_icon_sheet(
            &context,
            &mut renderer,
            &factory,
            output_directory,
            dpi,
            logger,
        )?;
    }
    for key in [
        "annotation.pass-through",
        "eraser.area-shape",
        "palette.radial.toggle",
        "color.*",
    ] {
        let lookup = if key == "color.*" { "color.1" } else { key };
        let fallback = fallback_glyph(lookup).expect("selftest fallback key must exist");
        println!(
            "fallback {key}: Segoe Fluent Icons U+{:04X} ({})",
            u32::from(fallback.glyph),
            fallback.label
        );
    }
    Ok(())
}

fn verify_attr_icons(renderer: &mut AppIconRenderer) -> Result<()> {
    let mut empty_attr = Vec::new();
    for key in embedded_attr_keys() {
        let asset = embedded_attr_asset(key).expect("embedded attr key must resolve");
        let pixels = renderer.render_attr_asset_alpha_count(asset)?;
        println!("attr-icon {key} px={pixels}");
        if pixels == 0 {
            empty_attr.push(key);
        }
    }
    if !empty_attr.is_empty() {
        return Err(Error::new(
            E_FAIL,
            format!(
                "attr SVG alpha>0 pixel count is zero: {}",
                empty_attr.join(", ")
            ),
        ));
    }

    let pen = |width: &str| AttrIconConfiguration {
        tool_key: "tool.pen".to_owned(),
        attributes: BTreeMap::from([("width".to_owned(), width.to_owned())]),
        color: Some(AttrIconColor::new(1.0, 0.231, 0.188, 1.0)),
    };
    let pen_thin_pixels = renderer.render_attr_icon_alpha_count(&pen("1"), false)?;
    let pen_thick_pixels = renderer.render_attr_icon_alpha_count(&pen("4"), false)?;
    println!("attr-icon pen width=1 px={pen_thin_pixels}, width=4 px={pen_thick_pixels}");
    if pen_thin_pixels == pen_thick_pixels {
        return Err(Error::new(
            E_FAIL,
            "attr pen width 1 and 4 rendered the same alpha pixel count",
        ));
    }
    let arrow = AttrIconConfiguration {
        tool_key: "tool.arrow".to_owned(),
        attributes: BTreeMap::from([
            ("heads".to_owned(), "single".to_owned()),
            ("width".to_owned(), "1".to_owned()),
        ]),
        color: Some(AttrIconColor::new(1.0, 0.231, 0.188, 1.0)),
    };
    let high_contrast_attr_pixels = renderer.render_attr_icon_alpha_count(&arrow, true)?;
    println!("attr-icon high-contrast attr px={high_contrast_attr_pixels}");
    if high_contrast_attr_pixels != 0 {
        return Err(Error::new(
            E_FAIL,
            format!(
                "high-contrast attr layer pixel count must be zero, got {high_contrast_attr_pixels}"
            ),
        ));
    }

    Ok(())
}

fn render_icon_sheet(
    context: &ID2D1DeviceContext,
    renderer: &mut AppIconRenderer,
    factory: &IWICImagingFactory,
    output_directory: &Path,
    dpi: u32,
    logger: &Logger,
) -> Result<()> {
    const COLUMNS: u32 = 10;
    const CELL_DIP: f32 = 48.0;
    const ICON_DIP: f32 = 24.0;
    let leading = 9usize;
    let total = leading + EMBEDDED_ICONS.len();
    let rows = u32::try_from(total.div_ceil(COLUMNS as usize)).expect("icon rows fit u32");
    let width = (COLUMNS as f32 * CELL_DIP * dpi as f32 / 96.0).round() as u32;
    let height = (rows as f32 * CELL_DIP * dpi as f32 / 96.0).round() as u32;
    unsafe { context.SetDpi(dpi as f32, dpi as f32) };
    let target = create_target_bitmap(context, width, height, dpi as f32, dpi as f32)?;
    let target_image: ID2D1Image = target.cast()?;
    let previous_target = unsafe { context.GetTarget() }?;
    unsafe {
        context.SetTarget(&target_image);
        context.BeginDraw();
        context.Clear(Some(&D2D1_COLOR_F {
            r: 42.0 / 255.0,
            g: 45.0 / 255.0,
            b: 51.0 / 255.0,
            a: 1.0,
        }));
    }
    let samples = ["tool.arrow", "board.black", "tool.highlighter"];
    let colors = [
        IconTint::rgba(1.0, 1.0, 1.0, 1.0),
        IconTint::rgba(0.0, 0.0, 0.0, 1.0),
        IconTint::rgba(0.2196, 0.7412, 0.9725, 1.0),
    ];
    let mut entries = Vec::with_capacity(total);
    for tint in colors {
        for key in samples {
            entries.push((key, tint));
        }
    }
    entries.extend(embedded_keys().map(|key| (key, IconTint::rgba(1.0, 1.0, 1.0, 1.0))));
    let draw_result = (|| -> Result<()> {
        for (index, (key, tint)) in entries.into_iter().enumerate() {
            let column = (index as u32 % COLUMNS) as f32;
            let row = (index as u32 / COLUMNS) as f32;
            let inset = (CELL_DIP - ICON_DIP) / 2.0;
            let left = column * CELL_DIP + inset;
            let top = row * CELL_DIP + inset;
            renderer.draw(
                key,
                D2D_RECT_F {
                    left,
                    top,
                    right: left + ICON_DIP,
                    bottom: top + ICON_DIP,
                },
                tint,
            )?;
        }
        Ok(())
    })();
    let end = unsafe { context.EndDraw(None, None) };
    unsafe { context.SetTarget(&previous_target) };
    draw_result?;
    renderer.handle_render_result(end)?;
    if !renderer.missing_keys().is_empty() {
        return Err(Error::new(
            E_FAIL,
            format!("missing icon keys: {}", renderer.missing_keys().join(", ")),
        ));
    }
    let (pixels, pitch) = read_bitmap_bgra(context, &target)?;
    let bitmap = unsafe {
        factory.CreateBitmapFromMemory(
            width,
            height,
            &GUID_WICPixelFormat32bppPBGRA,
            pitch,
            &pixels,
        )
    }?;
    let path = output_directory.join(format!("icons-{dpi}.png"));
    save_png(factory, &bitmap, &path, width, height, dpi)?;
    logger.event(&format!(
        "아이콘 selftest PNG 저장: {} ({}x{})",
        path.display(),
        width,
        height
    ));
    Ok(())
}

fn save_png(
    factory: &IWICImagingFactory,
    bitmap: &IWICBitmap,
    path: &Path,
    width: u32,
    height: u32,
    dpi: u32,
) -> Result<()> {
    let stream = unsafe { factory.CreateStream() }?;
    let path_wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe { stream.InitializeFromFilename(PCWSTR(path_wide.as_ptr()), GENERIC_WRITE.0) }?;
    let encoder = unsafe { factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()) }?;
    unsafe { encoder.Initialize(&stream, WICBitmapEncoderNoCache) }?;
    let mut frame: Option<IWICBitmapFrameEncode> = None;
    let mut options = None;
    unsafe { encoder.CreateNewFrame(&mut frame, &mut options) }?;
    let frame = frame.ok_or_else(Error::from_thread)?;
    unsafe {
        frame.Initialize(options.as_ref())?;
        frame.SetSize(width, height)?;
        frame.SetResolution(dpi as f64, dpi as f64)?;
    }
    let mut pixel_format = GUID_WICPixelFormat32bppPBGRA;
    unsafe {
        frame.SetPixelFormat(&mut pixel_format)?;
        if pixel_format != GUID_WICPixelFormat32bppPBGRA
            && pixel_format != GUID_WICPixelFormat32bppBGRA
        {
            return Err(Error::new(
                E_FAIL,
                format!("PNG encoder selected a format without verified alpha: {pixel_format:?}"),
            ));
        }
        frame.WriteSource(bitmap, std::ptr::null())?;
        frame.Commit()?;
        encoder.Commit()?;
    }
    Ok(())
}

fn resolve_key(key: &str, selected: bool) -> Resolution {
    if selected {
        let selected_key = format!("{key}.selected");
        if let Some(asset) = embedded_asset(&selected_key) {
            return Resolution {
                kind: ResolutionKind::Direct,
                asset: Some(asset),
                fallback: None,
            };
        }
    }
    if let Some(asset) = embedded_asset(key) {
        return Resolution {
            kind: if asset.os_override {
                ResolutionKind::OsSource
            } else {
                ResolutionKind::Direct
            },
            asset: Some(asset),
            fallback: None,
        };
    }
    if let Some((_, app_key)) = SYMBOL_MAP.iter().find(|(design, _)| *design == key) {
        if let Some(asset) = embedded_asset(app_key) {
            return Resolution {
                kind: ResolutionKind::SymbolMap,
                asset: Some(asset),
                fallback: None,
            };
        }
    }
    if let Some((app_key, _)) = OS_ICON_OVERRIDES.iter().find(|(_, source)| *source == key) {
        if let Some(asset) = embedded_asset(app_key) {
            return Resolution {
                kind: ResolutionKind::OsSource,
                asset: Some(asset),
                fallback: None,
            };
        }
    }
    if let Some((_, target)) = LOGICAL_ALIASES.iter().find(|(alias, _)| *alias == key) {
        if let Some(asset) = embedded_asset(target) {
            return Resolution {
                kind: ResolutionKind::Alias,
                asset: Some(asset),
                fallback: None,
            };
        }
    }
    let fallback = fallback_glyph(key);
    Resolution {
        kind: if fallback.is_some() {
            ResolutionKind::Fallback
        } else {
            ResolutionKind::Missing
        },
        asset: None,
        fallback,
    }
}

fn embedded_asset(key: &str) -> Option<EmbeddedAsset> {
    EMBEDDED_ICONS
        .iter()
        .find(|(candidate, _, _, _)| *candidate == key)
        .map(|(key, bytes, source, os_override)| EmbeddedAsset {
            key,
            bytes,
            source,
            os_override: *os_override,
        })
}

fn embedded_attr_keys() -> impl Iterator<Item = &'static str> {
    EMBEDDED_ATTR_ICONS.iter().map(|(key, _, _)| *key)
}

fn embedded_attr_asset(key: &str) -> Option<EmbeddedAttrAsset> {
    EMBEDDED_ATTR_ICONS
        .iter()
        .find(|(candidate, _, _)| *candidate == key)
        .map(|(key, bytes, source)| EmbeddedAttrAsset { key, bytes, source })
}

fn fallback_glyph(key: &str) -> Option<FallbackGlyph> {
    let (glyph, label) = match key {
        "annotation.pass-through" => ('\u{E7C9}', "pass-through"),
        "eraser.area-shape" => ('\u{E7A7}', "eraser area shape"),
        "palette.radial.toggle" => ('\u{F0E2}', "radial palette"),
        value if value.starts_with("color.") => ('\u{EA3A}', "color swatch"),
        value if value.starts_with("display.preset.") => ('\u{E7F4}', "display preset"),
        _ => return None,
    };
    Some(FallbackGlyph {
        family: "Segoe Fluent Icons",
        glyph,
        label,
    })
}

fn font_available(factory: &IDWriteFactory, family: PCWSTR) -> Result<bool> {
    let mut collection = None;
    unsafe { factory.GetSystemFontCollection(&mut collection, false) }?;
    let collection = collection.ok_or_else(Error::from_thread)?;
    let mut index = 0;
    let mut exists = BOOL::default();
    unsafe { collection.FindFamilyName(family, &mut index, &mut exists) }?;
    Ok(exists.as_bool())
}

fn icon_transform(context: &ID2D1DeviceContext, rect: D2D_RECT_F) -> D2D1_BRUSH_PROPERTIES {
    let mut dpi_x = 96.0;
    let mut dpi_y = 96.0;
    unsafe { context.GetDpi(&mut dpi_x, &mut dpi_y) };
    let dip_size = (rect.right - rect.left)
        .min(rect.bottom - rect.top)
        .max(0.0);
    let pixels = (dip_size * dpi_x / 96.0).round().max(1.0);
    let snapped_dip_size = pixels * 96.0 / dpi_x;
    let centered_left = rect.left + ((rect.right - rect.left) - dip_size) / 2.0;
    let centered_top = rect.top + ((rect.bottom - rect.top) - dip_size) / 2.0;
    let left = (centered_left * dpi_x / 96.0).round() * 96.0 / dpi_x;
    let top = (centered_top * dpi_y / 96.0).round() * 96.0 / dpi_y;
    let scale = snapped_dip_size / 24.0;
    let mut properties = D2D1_BRUSH_PROPERTIES::default();
    properties.transform.M11 = scale;
    properties.transform.M22 = scale;
    properties.transform.M31 = left;
    properties.transform.M32 = top;
    properties
}

fn create_target_bitmap(
    context: &ID2D1DeviceContext,
    width: u32,
    height: u32,
    dpi_x: f32,
    dpi_y: f32,
) -> Result<ID2D1Bitmap1> {
    let properties = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi_x,
        dpiY: dpi_y,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
        ..Default::default()
    };
    unsafe { context.CreateBitmap(D2D_SIZE_U { width, height }, None, 0, &properties) }
}

fn tint_rgb(tint: IconTint) -> u32 {
    (u32::from(channel(tint.color.r)) << 16)
        | (u32::from(channel(tint.color.g)) << 8)
        | u32::from(channel(tint.color.b))
}

fn tint_rgba(tint: IconTint) -> u32 {
    (tint_rgb(tint) << 8) | u32::from(channel(tint.color.a))
}

fn attr_color(tint: IconTint) -> AttrIconColor {
    AttrIconColor::new(
        f64::from(tint.color.r),
        f64::from(tint.color.g),
        f64::from(tint.color.b),
        f64::from(tint.color.a),
    )
}

pub fn icon_tint(color: AttrIconColor) -> IconTint {
    IconTint::rgba(
        color.red as f32,
        color.green as f32,
        color.blue as f32,
        color.alpha as f32,
    )
}

fn offset_rect(rect: D2D_RECT_F, x: f32, y: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left + x,
        top: rect.top + y,
        right: rect.right + x,
        bottom: rect.bottom + y,
    }
}

fn color_hex(rgb: u32) -> String {
    format!("#{rgb:06X}")
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{resolve_key, ResolutionKind};

    #[test]
    fn key_resolution_priority_table_covers_direct_map_os_fallback_and_missing() {
        let cases = [
            ("tool.pen", ResolutionKind::Direct),
            ("tool.rect", ResolutionKind::SymbolMap),
            ("keystroke-display.toggle", ResolutionKind::OsSource),
            ("annotation.pass-through", ResolutionKind::Fallback),
            ("not.a.real.icon", ResolutionKind::Missing),
        ];
        for (key, expected) in cases {
            assert_eq!(resolve_key(key, false).kind, expected, "{key}");
        }
    }
}
