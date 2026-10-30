//! 가로·세로 선형 팔레트의 순수 레이아웃과 Windows 창·D2D 구현.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Orientation {
    #[cfg(windows)]
    fn toggled(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }

    #[cfg(windows)]
    fn storage_value(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }

    #[cfg(windows)]
    fn from_storage(value: &str) -> Option<Self> {
        match value.trim() {
            "horizontal" => Some(Self::Horizontal),
            "vertical" => Some(Self::Vertical),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum PaletteItemId {
    DragHandle,
    ToolPen,
    ToolHighlighter,
    ToolLaser,
    ToolEraser,
    ToolArrow,
    ToolRectangle,
    ToolEllipse,
    ToolText,
    ToolSelect,
    ToolCover,
    ShapeRecognition,
    EraserModeLabel,
    EraserAreaShape,
    BoardWhite,
    BoardBlack,
    Color1,
    Color2,
    Color3,
    Color4,
    Color5,
    Color6,
    WidthDecrease,
    WidthLabel,
    WidthIncrease,
    Undo,
    Redo,
    ClearAll,
    ShowHelp,
}

impl PaletteItemId {
    const ALL: [Self; 29] = [
        Self::DragHandle,
        Self::ToolPen,
        Self::ToolHighlighter,
        Self::ToolLaser,
        Self::ToolEraser,
        Self::ToolArrow,
        Self::ToolRectangle,
        Self::ToolEllipse,
        Self::ToolText,
        Self::ToolSelect,
        Self::ToolCover,
        Self::ShapeRecognition,
        Self::EraserModeLabel,
        Self::EraserAreaShape,
        Self::BoardWhite,
        Self::BoardBlack,
        Self::Color1,
        Self::Color2,
        Self::Color3,
        Self::Color4,
        Self::Color5,
        Self::Color6,
        Self::WidthDecrease,
        Self::WidthLabel,
        Self::WidthIncrease,
        Self::Undo,
        Self::Redo,
        Self::ClearAll,
        Self::ShowHelp,
    ];

    #[cfg(windows)]
    pub fn from_message(value: usize) -> Option<Self> {
        let value = u16::try_from(value).ok()?;
        Self::ALL
            .into_iter()
            .find(|candidate| *candidate as u16 == value)
    }

    #[cfg(windows)]
    fn color_index(self) -> Option<usize> {
        match self {
            Self::Color1 => Some(0),
            Self::Color2 => Some(1),
            Self::Color3 => Some(2),
            Self::Color4 => Some(3),
            Self::Color5 => Some(4),
            Self::Color6 => Some(5),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PixelRect {
    #[cfg(windows)]
    fn width(self) -> i32 {
        self.right - self.left
    }

    #[cfg(windows)]
    fn height(self) -> i32 {
        self.bottom - self.top
    }

    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaletteItemRect {
    pub id: PaletteItemId,
    pub rect: PixelRect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaletteLayout {
    pub width: i32,
    pub height: i32,
    pub items: Vec<PaletteItemRect>,
}

impl PaletteLayout {
    pub fn calculate(orientation: Orientation, dpi: u32) -> Self {
        const PADDING: f64 = 8.0;
        const SPACING: f64 = 6.0;
        let scale = f64::from(dpi.max(1)) / 96.0;
        let sizes = PaletteItemId::ALL.map(item_size_dip);
        let primary_sum = sizes
            .iter()
            .map(|(width, height)| match orientation {
                Orientation::Horizontal => *width,
                Orientation::Vertical => *height,
            })
            .sum::<f64>();
        let cross_max = sizes
            .iter()
            .map(|(width, height)| match orientation {
                Orientation::Horizontal => *height,
                Orientation::Vertical => *width,
            })
            .fold(0.0, f64::max);
        let primary_total = PADDING * 2.0
            + primary_sum
            + SPACING * (PaletteItemId::ALL.len().saturating_sub(1) as f64);
        let cross_total = PADDING * 2.0 + cross_max;
        let (width_dip, height_dip) = match orientation {
            Orientation::Horizontal => (primary_total, cross_total),
            Orientation::Vertical => (cross_total, primary_total),
        };
        let width = pixels(width_dip, scale);
        let height = pixels(height_dip, scale);
        let mut cursor = PADDING;
        let mut items = Vec::with_capacity(PaletteItemId::ALL.len());
        for (id, (item_width, item_height)) in PaletteItemId::ALL.into_iter().zip(sizes) {
            let (left, top) = match orientation {
                Orientation::Horizontal => (cursor, PADDING + (cross_max - item_height) / 2.0),
                Orientation::Vertical => (PADDING + (cross_max - item_width) / 2.0, cursor),
            };
            items.push(PaletteItemRect {
                id,
                rect: PixelRect {
                    left: pixels(left, scale),
                    top: pixels(top, scale),
                    right: pixels(left + item_width, scale),
                    bottom: pixels(top + item_height, scale),
                },
            });
            cursor += match orientation {
                Orientation::Horizontal => item_width,
                Orientation::Vertical => item_height,
            } + SPACING;
        }
        Self {
            width,
            height,
            items,
        }
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<PaletteItemId> {
        self.items
            .iter()
            .find(|item| item.rect.contains(x, y))
            .map(|item| item.id)
    }

    #[cfg(test)]
    fn rect_for(&self, id: PaletteItemId) -> PixelRect {
        self.items
            .iter()
            .find(|item| item.id == id)
            .expect("all palette items have a layout rect")
            .rect
    }
}

fn item_size_dip(id: PaletteItemId) -> (f64, f64) {
    match id {
        PaletteItemId::DragHandle => (28.0, 28.0),
        PaletteItemId::EraserModeLabel => (44.0, 20.0),
        PaletteItemId::WidthLabel => (32.0, 20.0),
        PaletteItemId::Color1
        | PaletteItemId::Color2
        | PaletteItemId::Color3
        | PaletteItemId::Color4
        | PaletteItemId::Color5
        | PaletteItemId::Color6 => (18.0, 18.0),
        _ => (32.0, 32.0),
    }
}

fn pixels(dip: f64, scale: f64) -> i32 {
    (dip * scale).round().max(1.0) as i32
}

#[cfg(windows)]
mod windows_impl {
    use super::{Orientation, PaletteItemId, PaletteLayout, PixelRect};
    use crate::app_icon::{high_contrast, AppIconRenderer, IconTint};
    use crate::log::Logger;
    use crate::monitor::{enumerate_monitors, monitor_at_point, Monitor};
    use crate::render::{
        create_swap_chain, create_swap_target, pixel_format, px_per_dip, read_bitmap_bgra,
        GraphicsDevice,
    };
    use crate::z_bands::{raise_band_order, Band, BandRegistration};
    use edutools_core::board::BoardKind;
    use edutools_core::ink::InkTool;
    use edutools_core::radial::RadialPaletteArcStyle;
    use edutools_core::screen_position::{
        restored_origin, saved_ratio, PositionRatio, ScreenFrame, ScreenPositionDocument,
        WindowOrigin, WindowSize,
    };
    use edutools_core::settings::{AppSettings, PaletteStyle};
    use edutools_core::shortcut::{
        ShortcutBoard, ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutTool,
    };
    use edutools_core::state::AppState;
    use std::ffi::c_void;
    use std::fs;
    use std::path::{Path, PathBuf};
    use windows::core::{w, Error, Interface, Result};
    use windows::Win32::Foundation::{
        ERROR_CLASS_ALREADY_EXISTS, E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
    };
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
    };
    use windows::Win32::Graphics::Direct2D::{
        ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Image, ID2D1RenderTarget,
        D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DRAW_TEXT_OPTIONS_NONE,
        D2D1_ELLIPSE, D2D1_PRIMITIVE_BLEND_COPY, D2D1_PRIMITIVE_BLEND_SOURCE_OVER,
        D2D1_ROUNDED_RECT,
    };
    use windows::Win32::Graphics::DirectComposition::{IDCompositionTarget, IDCompositionVisual};
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, DWRITE_FACTORY_TYPE_SHARED,
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        DWRITE_TEXT_ALIGNMENT_CENTER,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
    use windows::Win32::Graphics::Dxgi::IDXGISwapChain1;
    use windows::Win32::Graphics::Gdi::{
        GetSysColor, ScreenToClient, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT,
        COLOR_WINDOW, COLOR_WINDOWTEXT,
    };
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::Pointer::{
        GetPointerInfo, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE, POINTER_FLAG_SECONDBUTTON,
        POINTER_INFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, GetWindowRect,
        PostMessageW, RegisterClassW, SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW,
        GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE, MA_NOACTIVATE, PA_NOACTIVATE, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, WM_DPICHANGED,
        WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCCREATE, WM_NCDESTROY, WM_POINTERACTIVATE,
        WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
        WM_SETTINGCHANGE, WM_SYSCOLORCHANGE, WM_THEMECHANGED, WNDCLASSW, WS_EX_NOACTIVATE,
        WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    pub const WM_APP_PALETTE_ACTION: u32 = 0x8000 + 4;
    pub const WM_APP_PALETTE_REDRAW: u32 = 0x8000 + 5;
    pub const WM_APP_PALETTE_SAVE_POSITION: u32 = 0x8000 + 6;
    pub const WM_APP_PALETTE_TOGGLE_ORIENTATION: u32 = 0x8000 + 7;

    const PALETTE_CLASS: windows::core::PCWSTR = w!("EduToolsPalette");
    const DEFAULT_POSITION: PositionRatio = PositionRatio { x: 0.5, y: 0.15 };

    struct PointerPress {
        pointer_id: u32,
        item: Option<PaletteItemId>,
        screen_start: POINT,
        window_start: POINT,
        dragging: bool,
        secondary: bool,
    }

    struct PaletteWindowState {
        owner: HWND,
        logger: Logger,
        orientation: Orientation,
        dpi: u32,
        layout: PaletteLayout,
        hover: Option<PaletteItemId>,
        press: Option<PointerPress>,
        renderer: Option<PaletteRenderer>,
    }

    struct PaletteWindow {
        hwnd: HWND,
        state: Box<PaletteWindowState>,
        _band: BandRegistration,
    }

    pub struct PaletteManager {
        window: PaletteWindow,
        _graphics: GraphicsDevice,
        positions: ScreenPositionDocument,
        positions_path: Option<PathBuf>,
        orientation_path: Option<PathBuf>,
        visible: bool,
    }

    impl PaletteManager {
        pub fn new(owner: HWND, hinstance: HINSTANCE, logger: Logger) -> Result<Self> {
            register_window_class(hinstance)?;
            let positions_path = app_data_file("positions.json");
            let orientation_path = app_data_file("palette-orientation.txt");
            let positions = load_positions(positions_path.as_deref(), &logger);
            let orientation = load_orientation(orientation_path.as_deref(), &logger);
            let monitors = enumerate_monitors()?;
            let monitor = target_monitor(&monitors, positions.last_display_key())
                .ok_or_else(|| Error::new(E_FAIL, "palette requires at least one monitor"))?;
            let dpi = monitor.dpi.0.max(96);
            let layout = PaletteLayout::calculate(orientation, dpi);
            let origin = restored_palette_origin(&positions, monitor, &layout);
            let mut state = Box::new(PaletteWindowState {
                owner,
                logger: logger.clone(),
                orientation,
                dpi,
                layout,
                hover: None,
                press: None,
                renderer: None,
            });
            let state_ptr = state.as_mut() as *mut PaletteWindowState;
            let ex_style =
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP;
            let hwnd = unsafe {
                CreateWindowExW(
                    ex_style,
                    PALETTE_CLASS,
                    w!("EduTools Palette"),
                    WS_POPUP,
                    origin.x.round() as i32,
                    origin.y.round() as i32,
                    state.layout.width,
                    state.layout.height,
                    None,
                    None,
                    Some(hinstance),
                    Some(state_ptr.cast::<c_void>()),
                )
            }?;
            let band = BandRegistration::new(hwnd, Band::Tool);
            let graphics = match GraphicsDevice::new() {
                Ok(graphics) => graphics,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            let renderer = match PaletteRenderer::new(
                &graphics,
                hwnd,
                state.layout.width,
                state.layout.height,
                dpi,
                logger,
            ) {
                Ok(renderer) => renderer,
                Err(error) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                    return Err(error);
                }
            };
            state.renderer = Some(renderer);
            Ok(Self {
                window: PaletteWindow {
                    hwnd,
                    state,
                    _band: band,
                },
                _graphics: graphics,
                positions,
                positions_path,
                orientation_path,
                visible: false,
            })
        }

        pub fn refresh(&mut self, state: &AppState, settings: &AppSettings) -> Result<()> {
            self.render(state, settings)?;
            let visible = state.is_annotation_mode_enabled()
                && state.is_palette_visible()
                && settings.tools.palette_style == PaletteStyle::Linear;
            if visible {
                let _ = unsafe { ShowWindow(self.window.hwnd, SW_SHOWNOACTIVATE) };
                if !self.visible {
                    raise_band_order()?;
                }
            } else {
                let _ = unsafe { ShowWindow(self.window.hwnd, SW_HIDE) };
            }
            self.visible = visible;
            Ok(())
        }

        pub fn redraw(&mut self, state: &AppState, settings: &AppSettings) -> Result<()> {
            self.render(state, settings)
        }

        pub fn display_configuration_changed(
            &mut self,
            state: &AppState,
            settings: &AppSettings,
        ) -> Result<()> {
            self.restore_position()?;
            self.refresh(state, settings)
        }

        pub fn toggle_orientation(
            &mut self,
            state: &AppState,
            settings: &AppSettings,
        ) -> Result<()> {
            self.window.state.orientation = self.window.state.orientation.toggled();
            self.window.state.layout =
                PaletteLayout::calculate(self.window.state.orientation, self.window.state.dpi);
            self.resize_to_layout()?;
            self.persist_orientation()?;
            self.restore_position()?;
            self.refresh(state, settings)
        }

        pub fn save_position(&mut self) -> Result<()> {
            let mut rect = RECT::default();
            unsafe { GetWindowRect(self.window.hwnd, &mut rect) }?;
            let monitors = enumerate_monitors()?;
            let center = POINT {
                x: rect.left + (rect.right - rect.left) / 2,
                y: rect.top + (rect.bottom - rect.top) / 2,
            };
            let Some(monitor) = monitor_at_point(&monitors, center)
                .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
                .or_else(|| monitors.first())
            else {
                return Err(Error::new(
                    E_FAIL,
                    "cannot save palette position without a monitor",
                ));
            };
            let ratio = saved_ratio(
                WindowOrigin {
                    x: f64::from(rect.left),
                    y: f64::from(rect.top),
                },
                screen_frame(monitor),
                WindowSize {
                    width: f64::from(rect.right - rect.left),
                    height: f64::from(rect.bottom - rect.top),
                },
            );
            // monitor.rs의 session_key는 이번 로그인 세션용 reconciliation 키다. 포트 변경을
            // 견디는 영속 DisplayIdentity로 바꾸는 일은 W10 후속 범위다.
            self.positions.save(ratio, monitor.session_key.clone());
            self.persist_positions()
        }

        pub fn monitor_key(&self) -> Result<Option<String>> {
            let mut rect = RECT::default();
            unsafe { GetWindowRect(self.window.hwnd, &mut rect) }?;
            let point = POINT {
                x: rect.left + (rect.right - rect.left) / 2,
                y: rect.top + (rect.bottom - rect.top) / 2,
            };
            let monitors = enumerate_monitors()?;
            Ok(monitor_at_point(&monitors, point).map(|monitor| monitor.session_key.clone()))
        }

        pub fn selftest_failures(
            &mut self,
            state: &AppState,
            settings: &AppSettings,
        ) -> Vec<String> {
            let mut failures = Vec::new();
            let required_ex =
                (WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP).0
                    as isize;
            let actual_ex = unsafe { GetWindowLongPtrW(self.window.hwnd, GWL_EXSTYLE) };
            if actual_ex & required_ex != required_ex {
                failures.push(format!("palette: 필수 확장 스타일 누락 ({actual_ex:#x})"));
            }
            let actual_style = unsafe { GetWindowLongPtrW(self.window.hwnd, GWL_STYLE) };
            if actual_style & WS_POPUP.0 as isize == 0 {
                failures.push(format!("palette: WS_POPUP 누락 ({actual_style:#x})"));
            }
            let mut rect = RECT::default();
            match unsafe { GetWindowRect(self.window.hwnd, &mut rect) } {
                Ok(())
                    if rect.right - rect.left == self.window.state.layout.width
                        && rect.bottom - rect.top == self.window.state.layout.height => {}
                Ok(()) => failures.push(format!(
                    "palette: 창 {}x{} != layout {}x{}",
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    self.window.state.layout.width,
                    self.window.state.layout.height
                )),
                Err(error) => failures.push(format!("palette: GetWindowRect 실패: {error}")),
            }
            match self
                .window
                .state
                .renderer
                .as_mut()
                .expect("palette renderer exists")
                .selftest_pixel_failures(
                    &self.window.state.layout,
                    self.window.state.hover,
                    state,
                    settings,
                ) {
                Ok(pixel_failures) => failures.extend(pixel_failures),
                Err(error) => failures.push(format!("palette: 픽셀 readback 실패: {error}")),
            }
            failures
        }

        fn render(&mut self, state: &AppState, settings: &AppSettings) -> Result<()> {
            self.window
                .state
                .renderer
                .as_mut()
                .expect("palette renderer exists")
                .render(
                    &self.window.state.layout,
                    self.window.state.hover,
                    state,
                    settings,
                )
        }

        fn resize_to_layout(&mut self) -> Result<()> {
            let layout = &self.window.state.layout;
            unsafe {
                SetWindowPos(
                    self.window.hwnd,
                    None,
                    0,
                    0,
                    layout.width,
                    layout.height,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOZORDER,
                )
            }?;
            self.window
                .state
                .renderer
                .as_mut()
                .expect("palette renderer exists")
                .resize(layout.width, layout.height, self.window.state.dpi)
        }

        fn restore_position(&mut self) -> Result<()> {
            let monitors = enumerate_monitors()?;
            let monitor = target_monitor(&monitors, self.positions.last_display_key())
                .ok_or_else(|| Error::new(E_FAIL, "cannot restore palette without a monitor"))?;
            let dpi = monitor.dpi.0.max(96);
            if dpi != self.window.state.dpi {
                self.window.state.dpi = dpi;
                self.window.state.layout =
                    PaletteLayout::calculate(self.window.state.orientation, dpi);
                self.resize_to_layout()?;
            }
            let origin =
                restored_palette_origin(&self.positions, monitor, &self.window.state.layout);
            unsafe {
                SetWindowPos(
                    self.window.hwnd,
                    None,
                    origin.x.round() as i32,
                    origin.y.round() as i32,
                    self.window.state.layout.width,
                    self.window.state.layout.height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            }
        }

        fn persist_positions(&self) -> Result<()> {
            let Some(path) = self.positions_path.as_deref() else {
                return Err(Error::new(
                    E_FAIL,
                    "APPDATA is unavailable for positions.json",
                ));
            };
            let json = self
                .positions
                .to_json()
                .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
            write_file(path, json.as_bytes(), "positions.json")
        }

        fn persist_orientation(&self) -> Result<()> {
            let Some(path) = self.orientation_path.as_deref() else {
                return Err(Error::new(
                    E_FAIL,
                    "APPDATA is unavailable for palette orientation",
                ));
            };
            write_file(
                path,
                self.window.state.orientation.storage_value().as_bytes(),
                "palette orientation",
            )
        }
    }

    impl Drop for PaletteWindow {
        fn drop(&mut self) {
            self.state.renderer.take();
            if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
                self.state
                    .logger
                    .event(&format!("팔레트 창 제거 실패: {error}"));
            }
        }
    }

    pub fn command_for_item(id: PaletteItemId) -> Option<ShortcutCommand> {
        match id {
            PaletteItemId::ToolPen => Some(select_tool(ShortcutTool::Pen)),
            PaletteItemId::ToolHighlighter => Some(select_tool(ShortcutTool::Highlighter)),
            PaletteItemId::ToolLaser => Some(ShortcutCommand::UseLaser),
            PaletteItemId::ToolEraser => Some(ShortcutCommand::SelectOrCycleEraser),
            PaletteItemId::ToolArrow => Some(select_tool(ShortcutTool::Arrow)),
            PaletteItemId::ToolRectangle => Some(select_tool(ShortcutTool::Rectangle)),
            PaletteItemId::ToolEllipse => Some(select_tool(ShortcutTool::Ellipse)),
            PaletteItemId::ToolText => Some(select_tool(ShortcutTool::Text)),
            PaletteItemId::ToolSelect => Some(select_tool(ShortcutTool::Select)),
            PaletteItemId::ToolCover => Some(select_tool(ShortcutTool::Cover)),
            PaletteItemId::ShapeRecognition => Some(ShortcutCommand::ToggleShapeRecognition),
            PaletteItemId::EraserAreaShape => Some(ShortcutCommand::ToggleEraserAreaShape),
            PaletteItemId::BoardWhite => Some(ShortcutCommand::ToggleBoard(ShortcutBoard::White)),
            PaletteItemId::BoardBlack => Some(ShortcutCommand::ToggleBoard(ShortcutBoard::Black)),
            PaletteItemId::Color1
            | PaletteItemId::Color2
            | PaletteItemId::Color3
            | PaletteItemId::Color4
            | PaletteItemId::Color5
            | PaletteItemId::Color6 => id.color_index().map(ShortcutCommand::SelectColor),
            PaletteItemId::WidthDecrease => Some(ShortcutCommand::AdjustWidth(-1)),
            PaletteItemId::WidthIncrease => Some(ShortcutCommand::AdjustWidth(1)),
            PaletteItemId::Undo => Some(ShortcutCommand::Undo),
            PaletteItemId::Redo => Some(ShortcutCommand::Redo),
            PaletteItemId::ClearAll => Some(ShortcutCommand::ClearAll),
            PaletteItemId::ShowHelp => Some(ShortcutCommand::ShowHelp),
            PaletteItemId::DragHandle
            | PaletteItemId::EraserModeLabel
            | PaletteItemId::WidthLabel => None,
        }
    }

    pub fn item_enabled(id: PaletteItemId, state: &AppState, _settings: &AppSettings) -> bool {
        match id {
            PaletteItemId::WidthDecrease | PaletteItemId::WidthIncrease => !matches!(
                state.current_tool(),
                InkTool::Laser | InkTool::Select | InkTool::Cover
            ),
            _ => true,
        }
    }

    fn select_tool(tool: ShortcutTool) -> ShortcutCommand {
        ShortcutCommand::SelectTool(tool)
    }

    fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
        let class = WNDCLASSW {
            lpfnWndProc: Some(palette_window_proc),
            hInstance: hinstance,
            lpszClassName: PALETTE_CLASS,
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

    unsafe extern "system" fn palette_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            let state = create.lpCreateParams as *mut PaletteWindowState;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
        }
        let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PaletteWindowState };
        if !state.is_null() {
            let state = unsafe { &mut *state };
            match message {
                WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {
                    let pointer_id = (wparam.0 as u32) & 0xffff;
                    if let Err(error) = handle_pointer(hwnd, state, message, pointer_id) {
                        state.logger.event(&format!(
                            "팔레트 포인터 처리 실패: pointerId={pointer_id} message={message:#x}: {error}"
                        ));
                    }
                    return LRESULT(0);
                }
                WM_POINTERLEAVE => {
                    if state.press.is_none() && state.hover.take().is_some() {
                        post_owner(state, WM_APP_PALETTE_REDRAW, 0);
                    }
                    return LRESULT(0);
                }
                WM_POINTERCAPTURECHANGED => {
                    state.press = None;
                    post_owner(state, WM_APP_PALETTE_REDRAW, 0);
                    return LRESULT(0);
                }
                WM_DPICHANGED => {
                    let queried = unsafe { GetDpiForWindow(hwnd) };
                    state.dpi = queried.max(96);
                    state.layout = PaletteLayout::calculate(state.orientation, state.dpi);
                    let suggested = unsafe { &*(lparam.0 as *const RECT) };
                    let result = unsafe {
                        SetWindowPos(
                            hwnd,
                            None,
                            suggested.left,
                            suggested.top,
                            state.layout.width,
                            state.layout.height,
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        )
                    }
                    .and_then(|_| {
                        state
                            .renderer
                            .as_mut()
                            .expect("palette renderer exists after creation")
                            .resize(state.layout.width, state.layout.height, state.dpi)
                    });
                    if let Err(error) = result {
                        state
                            .logger
                            .event(&format!("팔레트 WM_DPICHANGED 크기 반영 실패: {error}"));
                    }
                    post_owner(state, WM_APP_PALETTE_REDRAW, 0);
                    return LRESULT(0);
                }
                WM_POINTERACTIVATE => return LRESULT(PA_NOACTIVATE as isize),
                WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
                WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
                    post_owner(state, WM_APP_PALETTE_REDRAW, 0);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => return LRESULT(1),
                WM_NCDESTROY => unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                },
                _ => {}
            }
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn handle_pointer(
        hwnd: HWND,
        state: &mut PaletteWindowState,
        message: u32,
        pointer_id: u32,
    ) -> Result<()> {
        let (info, client) = pointer_snapshot(hwnd, pointer_id)?;
        let hit = state.layout.hit_test(client.x, client.y);
        if message == WM_POINTERDOWN {
            if state.press.is_some() {
                return Ok(());
            }
            let mut rect = RECT::default();
            unsafe { GetWindowRect(hwnd, &mut rect) }?;
            state.press = Some(PointerPress {
                pointer_id,
                item: hit,
                screen_start: info.ptPixelLocation,
                window_start: POINT {
                    x: rect.left,
                    y: rect.top,
                },
                dragging: false,
                secondary: info.pointerFlags & POINTER_FLAG_SECONDBUTTON
                    == POINTER_FLAG_SECONDBUTTON,
            });
            if state.hover != hit {
                state.hover = hit;
            }
            post_owner(state, WM_APP_PALETTE_REDRAW, 0);
            return Ok(());
        }

        if message == WM_POINTERUPDATE {
            let in_range = info.pointerFlags & POINTER_FLAG_INRANGE == POINTER_FLAG_INRANGE;
            let in_contact = info.pointerFlags & POINTER_FLAG_INCONTACT == POINTER_FLAG_INCONTACT;
            if !in_contact && in_range && state.hover != hit {
                state.hover = hit;
                post_owner(state, WM_APP_PALETTE_REDRAW, 0);
            }
            if let Some(press) = state.press.as_mut() {
                if press.pointer_id == pointer_id
                    && press.item == Some(PaletteItemId::DragHandle)
                    && !press.secondary
                {
                    let dx = info.ptPixelLocation.x - press.screen_start.x;
                    let dy = info.ptPixelLocation.y - press.screen_start.y;
                    let threshold = (4.0 * px_per_dip(state.dpi)).round().max(1.0) as i32;
                    if press.dragging || dx.abs().max(dy.abs()) >= threshold {
                        press.dragging = true;
                        let (x, y) = clamped_drag_origin(
                            info.ptPixelLocation,
                            press.window_start.x + dx,
                            press.window_start.y + dy,
                            state.layout.width,
                            state.layout.height,
                        )?;
                        unsafe {
                            SetWindowPos(
                                hwnd,
                                None,
                                x,
                                y,
                                0,
                                0,
                                SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                            )
                        }?;
                    }
                }
            }
            return Ok(());
        }

        let Some(press) = state.press.take() else {
            return Ok(());
        };
        if press.pointer_id != pointer_id {
            state.press = Some(press);
            return Ok(());
        }
        if press.dragging {
            post_owner(state, WM_APP_PALETTE_SAVE_POSITION, 0);
        } else if press.secondary {
            post_owner(state, WM_APP_PALETTE_TOGGLE_ORIENTATION, 0);
        } else if press.item == hit {
            if let Some(item) = hit {
                post_owner(state, WM_APP_PALETTE_ACTION, item as usize);
            }
        }
        state.hover = hit;
        post_owner(state, WM_APP_PALETTE_REDRAW, 0);
        Ok(())
    }

    fn pointer_snapshot(hwnd: HWND, pointer_id: u32) -> Result<(POINTER_INFO, POINT)> {
        let mut info = POINTER_INFO::default();
        unsafe { GetPointerInfo(pointer_id, &mut info) }?;
        let mut client = info.ptPixelLocation;
        if !unsafe { ScreenToClient(hwnd, &mut client) }.as_bool() {
            return Err(Error::from_thread());
        }
        Ok((info, client))
    }

    fn clamped_drag_origin(
        pointer: POINT,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(i32, i32)> {
        let monitors = enumerate_monitors()?;
        let monitor = monitor_at_point(&monitors, pointer)
            .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
            .or_else(|| monitors.first())
            .ok_or_else(|| Error::new(E_FAIL, "palette drag has no target monitor"))?;
        let work = monitor.work_rect;
        let maximum_x = (work.right - width).max(work.left);
        let maximum_y = (work.bottom - height).max(work.top);
        Ok((x.clamp(work.left, maximum_x), y.clamp(work.top, maximum_y)))
    }

    fn post_owner(state: &PaletteWindowState, message: u32, value: usize) {
        if let Err(error) =
            unsafe { PostMessageW(Some(state.owner), message, WPARAM(value), LPARAM(0)) }
        {
            state.logger.event(&format!(
                "팔레트 owner 메시지 전송 실패: {message:#x}: {error}"
            ));
        }
    }

    struct PaletteRenderer {
        swap_chain: IDXGISwapChain1,
        context: ID2D1DeviceContext,
        swap_target: Option<ID2D1Bitmap1>,
        icons: AppIconRenderer,
        dwrite: IDWriteFactory,
        _dcomp_target: IDCompositionTarget,
        _visual: IDCompositionVisual,
        width: u32,
        height: u32,
        dpi: u32,
    }

    #[derive(Clone, Copy)]
    struct PaletteTheme {
        surface: [f64; 4],
        border: [f64; 4],
        hover_fill: [f64; 4],
        selected_fill: [f64; 4],
        active_outline: [f64; 4],
        inactive_icon: [f64; 4],
        active_icon: [f64; 4],
        disabled_icon: [f64; 4],
    }

    impl PaletteRenderer {
        fn new(
            graphics: &GraphicsDevice,
            hwnd: HWND,
            width: i32,
            height: i32,
            dpi: u32,
            logger: Logger,
        ) -> Result<Self> {
            let width = positive_dimension(width)?;
            let height = positive_dimension(height)?;
            let swap_chain = create_swap_chain(graphics, width, height)?;
            let context = graphics.create_d2d_context()?;
            unsafe { context.SetDpi(96.0, 96.0) };
            let swap_target = Some(create_swap_target(&context, &swap_chain)?);
            set_context_target(
                &context,
                swap_target.as_ref().expect("palette swap target exists"),
            )?;
            let (dcomp_target, visual) =
                graphics.attach_composition_swap_chain(hwnd, &swap_chain)?;
            let icons = AppIconRenderer::new(context.clone(), logger)?;
            let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
            Ok(Self {
                swap_chain,
                context,
                swap_target,
                icons,
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
                    .expect("palette swap target recreated"),
            )?;
            self.width = width;
            self.height = height;
            self.dpi = dpi;
            Ok(())
        }

        fn render(
            &mut self,
            layout: &PaletteLayout,
            hover: Option<PaletteItemId>,
            state: &AppState,
            settings: &AppSettings,
        ) -> Result<()> {
            let target = self
                .swap_target
                .as_ref()
                .expect("palette swap target exists")
                .clone();
            self.render_to(&target, layout, hover, state, settings)?;
            unsafe { self.swap_chain.Present(1, Default::default()).ok() }
        }

        fn selftest_pixel_failures(
            &mut self,
            layout: &PaletteLayout,
            hover: Option<PaletteItemId>,
            state: &AppState,
            settings: &AppSettings,
        ) -> Result<Vec<String>> {
            let bitmap = create_target_bitmap(&self.context, self.width, self.height)?;
            self.render_to(&bitmap, layout, hover, state, settings)?;
            let (pixels, pitch) = read_bitmap_bgra(&self.context, &bitmap)?;
            let alpha_at =
                |x: u32, y: u32| -> u8 { pixels[y as usize * pitch as usize + x as usize * 4 + 3] };
            let mut failures = Vec::new();
            if alpha_at(self.width / 2, self.height / 2) == 0 {
                failures.push("palette: 면 중앙 alpha가 0".to_owned());
            }
            for (x, y) in [
                (0, 0),
                (self.width - 1, 0),
                (0, self.height - 1),
                (self.width - 1, self.height - 1),
            ] {
                if alpha_at(x, y) != 0 {
                    failures.push(format!(
                        "palette: 둥근 면 바깥 모서리 ({x},{y}) alpha={} (기대 0)",
                        alpha_at(x, y)
                    ));
                }
            }
            let target = self
                .swap_target
                .as_ref()
                .expect("palette swap target exists");
            set_context_target(&self.context, target)?;
            Ok(failures)
        }

        fn render_to(
            &mut self,
            target: &ID2D1Bitmap1,
            layout: &PaletteLayout,
            hover: Option<PaletteItemId>,
            state: &AppState,
            settings: &AppSettings,
        ) -> Result<()> {
            set_context_target(&self.context, target)?;
            unsafe {
                self.context.BeginDraw();
                self.context.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                self.context.Clear(Some(&D2D1_COLOR_F::default()));
            }
            let draw_result = self.draw_contents(layout, hover, state, settings);
            let end_result = unsafe { self.context.EndDraw(None, None) };
            draw_result?;
            self.icons.handle_render_result(end_result)
        }

        fn draw_contents(
            &mut self,
            layout: &PaletteLayout,
            hover: Option<PaletteItemId>,
            state: &AppState,
            settings: &AppSettings,
        ) -> Result<()> {
            let theme = palette_theme(settings.tools.palette_glass_opacity)?;
            let surface = color(theme.surface);
            let surface_brush = unsafe { self.context.CreateSolidColorBrush(&surface, None) }?;
            let panel = rounded_rect(
                PixelRect {
                    left: 0,
                    top: 0,
                    right: layout.width,
                    bottom: layout.height,
                },
                12.0 * px_per_dip(self.dpi()),
            );
            unsafe { self.context.FillRoundedRectangle(&panel, &surface_brush) };
            unsafe {
                self.context
                    .SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER)
            };
            let border = color(theme.border);
            let border_brush = unsafe { self.context.CreateSolidColorBrush(&border, None) }?;
            unsafe {
                self.context
                    .DrawRoundedRectangle(&panel, &border_brush, 1.0, None)
            };

            let definitions = ShortcutCatalog::definitions();
            for item in &layout.items {
                let selected = is_selected(item.id, state, settings);
                let enabled = item_enabled(item.id, state, settings);
                if hover == Some(item.id) || selected {
                    let fill = if selected {
                        theme.selected_fill
                    } else {
                        theme.hover_fill
                    };
                    let fill_brush =
                        unsafe { self.context.CreateSolidColorBrush(&color(fill), None) }?;
                    let button = rounded_rect(item.rect, 9.0 * px_per_dip(self.dpi()));
                    unsafe { self.context.FillRoundedRectangle(&button, &fill_brush) };
                    if selected {
                        let active = unsafe {
                            self.context
                                .CreateSolidColorBrush(&color(theme.active_outline), None)
                        }?;
                        unsafe {
                            self.context
                                .DrawRoundedRectangle(&button, &active, 1.5, None)
                        };
                    }
                }
                self.draw_item(
                    item.id,
                    item.rect,
                    selected,
                    enabled,
                    state,
                    settings,
                    &definitions,
                    theme,
                )?;
            }
            Ok(())
        }

        fn draw_item(
            &mut self,
            id: PaletteItemId,
            rect: PixelRect,
            selected: bool,
            enabled: bool,
            state: &AppState,
            settings: &AppSettings,
            definitions: &[ShortcutDefinition],
            theme: PaletteTheme,
        ) -> Result<()> {
            if id == PaletteItemId::DragHandle {
                return self.draw_drag_handle(rect, theme.inactive_icon);
            }
            if id == PaletteItemId::EraserModeLabel {
                return self.draw_label(
                    rect,
                    eraser_mode_label(&settings.eraser.mode),
                    10.0,
                    theme.inactive_icon,
                );
            }
            if id == PaletteItemId::WidthLabel {
                return self.draw_label(
                    rect,
                    &width_label(state, settings),
                    11.0,
                    theme.inactive_icon,
                );
            }
            if let Some(index) = id.color_index() {
                return self.draw_color(rect, index, selected, state, settings, theme);
            }
            let Some(command) = command_for_item(id) else {
                return Ok(());
            };
            let Some(definition) = definitions
                .iter()
                .find(|definition| definition.command == command)
            else {
                return Err(Error::new(
                    E_FAIL,
                    format!("palette command missing from catalog: {command:?}"),
                ));
            };
            let inset = (7.0 * px_per_dip(self.dpi())).round() as i32;
            let icon_rect = D2D_RECT_F {
                left: (rect.left + inset) as f32,
                top: (rect.top + inset) as f32,
                right: (rect.right - inset) as f32,
                bottom: (rect.bottom - inset) as f32,
            };
            let tint = if !enabled {
                theme.disabled_icon
            } else if selected {
                theme.active_icon
            } else {
                theme.inactive_icon
            };
            self.icons
                .draw_selected(&definition.icon_key, selected, icon_rect, icon_tint(tint))
        }

        fn draw_drag_handle(&self, rect: PixelRect, tint: [f64; 4]) -> Result<()> {
            let brush = unsafe { self.context.CreateSolidColorBrush(&color(tint), None) }?;
            let center_x = (rect.left + rect.right) as f32 / 2.0;
            let center_y = (rect.top + rect.bottom) as f32 / 2.0;
            let scale = px_per_dip(self.dpi()) as f32;
            for row in [-1.0f32, 0.0, 1.0] {
                for column in [-1.0f32, 1.0] {
                    let mut point = D2D1_ELLIPSE::default().point;
                    point.X = center_x + column * 3.5 * scale;
                    point.Y = center_y + row * 4.0 * scale;
                    unsafe {
                        self.context.FillEllipse(
                            &D2D1_ELLIPSE {
                                point,
                                radiusX: 1.2 * scale,
                                radiusY: 1.2 * scale,
                            },
                            &brush,
                        )
                    };
                }
            }
            Ok(())
        }

        fn draw_label(
            &self,
            rect: PixelRect,
            label: &str,
            font_size: f32,
            tint: [f64; 4],
        ) -> Result<()> {
            let format = unsafe {
                self.dwrite.CreateTextFormat(
                    w!("Segoe UI"),
                    None::<&IDWriteFontCollection>,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    font_size * px_per_dip(self.dpi()) as f32,
                    w!("ko-KR"),
                )
            }?;
            unsafe {
                format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            }
            let brush = unsafe { self.context.CreateSolidColorBrush(&color(tint), None) }?;
            let target: ID2D1RenderTarget = self.context.cast()?;
            let text = label.encode_utf16().collect::<Vec<_>>();
            unsafe {
                target.DrawText(
                    &text,
                    &format,
                    &d2d_rect(rect),
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                )
            };
            Ok(())
        }

        fn draw_color(
            &self,
            rect: PixelRect,
            index: usize,
            selected: bool,
            state: &AppState,
            settings: &AppSettings,
            theme: PaletteTheme,
        ) -> Result<()> {
            let Some(value) = settings.tools.colors.get(index) else {
                return Ok(());
            };
            let fill = D2D1_COLOR_F {
                r: value.red as f32,
                g: value.green as f32,
                b: value.blue as f32,
                a: value.alpha as f32,
            };
            let brush = unsafe { self.context.CreateSolidColorBrush(&fill, None) }?;
            let mut center = D2D1_ELLIPSE {
                radiusX: rect.width() as f32 / 2.0,
                radiusY: rect.height() as f32 / 2.0,
                ..Default::default()
            };
            center.point.X = (rect.left + rect.right) as f32 / 2.0;
            center.point.Y = (rect.top + rect.bottom) as f32 / 2.0;
            unsafe { self.context.FillEllipse(&center, &brush) };
            let outline = if selected
                || state.selected_color_index(settings) == i64::try_from(index).unwrap_or(-1)
            {
                theme.active_outline
            } else {
                theme.border
            };
            let outline_brush =
                unsafe { self.context.CreateSolidColorBrush(&color(outline), None) }?;
            unsafe {
                self.context.DrawEllipse(
                    &center,
                    &outline_brush,
                    if selected { 3.0 } else { 1.0 },
                    None,
                )
            };
            Ok(())
        }

        fn dpi(&self) -> u32 {
            self.dpi.max(1)
        }
    }

    fn is_selected(id: PaletteItemId, state: &AppState, settings: &AppSettings) -> bool {
        let tool = match id {
            PaletteItemId::ToolPen => Some(InkTool::Pen),
            PaletteItemId::ToolHighlighter => Some(InkTool::Highlighter),
            PaletteItemId::ToolLaser => Some(InkTool::Laser),
            PaletteItemId::ToolEraser => Some(InkTool::Eraser),
            PaletteItemId::ToolArrow => Some(InkTool::Arrow),
            PaletteItemId::ToolRectangle => Some(InkTool::Rectangle),
            PaletteItemId::ToolEllipse => Some(InkTool::Ellipse),
            PaletteItemId::ToolText => Some(InkTool::Text),
            PaletteItemId::ToolSelect => Some(InkTool::Select),
            PaletteItemId::ToolCover => Some(InkTool::Cover),
            _ => None,
        };
        if tool.is_some() {
            return tool == Some(state.current_tool());
        }
        match id {
            PaletteItemId::ShapeRecognition => settings.tools.shape_recognition_enabled,
            PaletteItemId::EraserAreaShape => settings.eraser.area_shape == "lasso",
            PaletteItemId::BoardWhite => state
                .presented_board()
                .is_some_and(|board| board.kind == BoardKind::White),
            PaletteItemId::BoardBlack => state
                .presented_board()
                .is_some_and(|board| board.kind == BoardKind::Black),
            _ => id
                .color_index()
                .is_some_and(|index| state.selected_color_index(settings) == index as i64),
        }
    }

    fn width_label(state: &AppState, settings: &AppSettings) -> String {
        let value = match state.current_tool() {
            InkTool::Pen
            | InkTool::Highlighter
            | InkTool::Arrow
            | InkTool::Rectangle
            | InkTool::Ellipse
            | InkTool::Text => state.width_for(state.current_tool(), settings),
            InkTool::Eraser => Some(settings.eraser.diameter),
            InkTool::Laser | InkTool::Select | InkTool::Cover => None,
        };
        match value {
            Some(value) if value.round() == value => format!("{}", value as i64),
            Some(value) => format!("{value:.1}"),
            None => "—".to_owned(),
        }
    }

    fn eraser_mode_label(value: &str) -> &str {
        match value {
            "partial" => "부분",
            "stroke" => "획",
            "area" => "영역",
            _ => "?",
        }
    }

    fn palette_theme(stored_opacity: f64) -> Result<PaletteTheme> {
        if high_contrast()? {
            return Ok(PaletteTheme {
                surface: system_color(COLOR_WINDOW),
                border: system_color(COLOR_WINDOWTEXT),
                hover_fill: system_color(COLOR_HIGHLIGHT),
                selected_fill: system_color(COLOR_HIGHLIGHT),
                active_outline: system_color(COLOR_HIGHLIGHT),
                inactive_icon: system_color(COLOR_WINDOWTEXT),
                active_icon: system_color(COLOR_HIGHLIGHTTEXT),
                disabled_icon: system_color(COLOR_GRAYTEXT),
            });
        }
        let mut disabled = RadialPaletteArcStyle::INACTIVE_ICON;
        disabled[3] *= RadialPaletteArcStyle::DISABLED_CONTENT_OPACITY;
        Ok(PaletteTheme {
            surface: RadialPaletteArcStyle::surface_color(stored_opacity),
            border: RadialPaletteArcStyle::OUTER_BORDER,
            hover_fill: RadialPaletteArcStyle::HOVER_FILL,
            selected_fill: RadialPaletteArcStyle::SELECTED_NODE_FILL,
            active_outline: RadialPaletteArcStyle::ACTIVE,
            inactive_icon: RadialPaletteArcStyle::INACTIVE_ICON,
            active_icon: RadialPaletteArcStyle::ACTIVE_ICON,
            disabled_icon: disabled,
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

    fn target_monitor<'a>(monitors: &'a [Monitor], saved: Option<&str>) -> Option<&'a Monitor> {
        saved
            .and_then(|key| monitors.iter().find(|monitor| monitor.session_key == key))
            .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
            .or_else(|| monitors.first())
    }

    fn restored_palette_origin(
        positions: &ScreenPositionDocument,
        monitor: &Monitor,
        layout: &PaletteLayout,
    ) -> WindowOrigin {
        restored_origin(
            positions.ratio_for(Some(&monitor.session_key)),
            DEFAULT_POSITION,
            screen_frame(monitor),
            WindowSize {
                width: f64::from(layout.width),
                height: f64::from(layout.height),
            },
        )
    }

    fn screen_frame(monitor: &Monitor) -> ScreenFrame {
        ScreenFrame {
            x: f64::from(monitor.work_rect.left),
            y: f64::from(monitor.work_rect.top),
            width: f64::from(monitor.work_rect.width()),
            height: f64::from(monitor.work_rect.height()),
        }
    }

    fn app_data_file(name: &str) -> Option<PathBuf> {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("EduTools").join(name))
    }

    fn load_positions(path: Option<&Path>, logger: &Logger) -> ScreenPositionDocument {
        let Some(path) = path else {
            logger.event("팔레트 위치 로드 생략: APPDATA 환경 변수가 없음");
            return ScreenPositionDocument::default();
        };
        match fs::read_to_string(path) {
            Ok(json) => match ScreenPositionDocument::from_json(&json) {
                Ok(document) => document,
                Err(error) => {
                    logger.event(&format!(
                        "팔레트 위치 JSON 손상; 원본 유지 후 기본 위치 사용: {}: {error}",
                        path.display()
                    ));
                    ScreenPositionDocument::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ScreenPositionDocument::default()
            }
            Err(error) => {
                logger.event(&format!(
                    "팔레트 위치 읽기 실패; 기본 위치 사용: {}: {error}",
                    path.display()
                ));
                ScreenPositionDocument::default()
            }
        }
    }

    fn load_orientation(path: Option<&Path>, logger: &Logger) -> Orientation {
        let Some(path) = path else {
            logger.event("팔레트 방향 로드 생략: APPDATA 환경 변수가 없음");
            return Orientation::Horizontal;
        };
        match fs::read_to_string(path) {
            Ok(value) => Orientation::from_storage(&value).unwrap_or_else(|| {
                logger.event(&format!(
                    "팔레트 방향 값이 잘못되어 가로 사용: {}",
                    path.display()
                ));
                Orientation::Horizontal
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Orientation::Horizontal,
            Err(error) => {
                logger.event(&format!(
                    "팔레트 방향 읽기 실패; 가로 사용: {}: {error}",
                    path.display()
                ));
                Orientation::Horizontal
            }
        }
    }

    fn write_file(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| Error::new(E_FAIL, format!("{label} path has no parent")))?;
        fs::create_dir_all(parent)
            .map_err(|error| Error::new(E_FAIL, format!("create {}: {error}", parent.display())))?;
        fs::write(path, bytes)
            .map_err(|error| Error::new(E_FAIL, format!("write {}: {error}", path.display())))
    }

    fn positive_dimension(value: i32) -> Result<u32> {
        u32::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| Error::new(E_FAIL, "palette dimension must be positive"))
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

    fn rounded_rect(rect: PixelRect, radius: f64) -> D2D1_ROUNDED_RECT {
        D2D1_ROUNDED_RECT {
            rect: d2d_rect(rect),
            radiusX: radius as f32,
            radiusY: radius as f32,
        }
    }

    fn d2d_rect(rect: PixelRect) -> D2D_RECT_F {
        D2D_RECT_F {
            left: rect.left as f32,
            top: rect.top as f32,
            right: rect.right as f32,
            bottom: rect.bottom as f32,
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

    fn icon_tint(value: [f64; 4]) -> IconTint {
        IconTint::rgba(
            value[0] as f32,
            value[1] as f32,
            value[2] as f32,
            value[3] as f32,
        )
    }
}

#[cfg(windows)]
pub use windows_impl::*;

#[cfg(test)]
mod tests {
    use super::{Orientation, PaletteItemId, PaletteLayout};

    #[test]
    fn layout_table_stays_inside_without_overlap_and_scales_with_dpi() {
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            let base = PaletteLayout::calculate(orientation, 96);
            let scaled = PaletteLayout::calculate(orientation, 144);
            assert_eq!(base.items.len(), PaletteItemId::ALL.len());
            assert_eq!(scaled.items.len(), PaletteItemId::ALL.len());
            assert!((f64::from(scaled.width) / f64::from(base.width) - 1.5).abs() < 0.02);
            assert!((f64::from(scaled.height) / f64::from(base.height) - 1.5).abs() < 0.02);
            for layout in [&base, &scaled] {
                for (index, item) in layout.items.iter().enumerate() {
                    assert!(item.rect.left >= 0 && item.rect.top >= 0);
                    assert!(item.rect.right <= layout.width && item.rect.bottom <= layout.height);
                    assert!(item.rect.left < item.rect.right && item.rect.top < item.rect.bottom);
                    for other in layout.items.iter().skip(index + 1) {
                        let overlaps = item.rect.left < other.rect.right
                            && item.rect.right > other.rect.left
                            && item.rect.top < other.rect.bottom
                            && item.rect.bottom > other.rect.top;
                        assert!(!overlaps, "{:?} overlaps {:?}", item.id, other.id);
                    }
                }
            }
        }
    }

    #[test]
    fn hit_test_table_selects_centers_and_rejects_gaps_and_bounds() {
        let layout = PaletteLayout::calculate(Orientation::Horizontal, 144);
        for id in [
            PaletteItemId::DragHandle,
            PaletteItemId::ToolPen,
            PaletteItemId::Color3,
            PaletteItemId::WidthLabel,
            PaletteItemId::ShowHelp,
        ] {
            let rect = layout.rect_for(id);
            assert_eq!(
                layout.hit_test((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2),
                Some(id),
                "{id:?}"
            );
        }
        let first = layout.rect_for(PaletteItemId::DragHandle);
        let second = layout.rect_for(PaletteItemId::ToolPen);
        assert_eq!(
            layout.hit_test((first.right + second.left) / 2, first.top),
            None
        );
        assert_eq!(layout.hit_test(-1, 0), None);
        assert_eq!(layout.hit_test(layout.width, layout.height / 2), None);
    }
}
