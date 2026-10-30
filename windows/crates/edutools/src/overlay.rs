use crate::log::Logger;
use crate::monitor::{enumerate_monitors, monitor_by_key, plan_monitor_sync, Monitor};
use std::collections::HashMap;
use std::ffi::{c_void, OsString};
use windows::core::{w, Error, Interface, Result};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, ERROR_SUCCESS, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT,
    RECT, WIN32_ERROR, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateDevice, ID2D1Bitmap1, ID2D1Device, ID2D1DeviceContext, ID2D1Image, ID2D1StrokeStyle,
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
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
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, GetWindowRect,
    RegisterClassW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    CREATESTRUCTW, GWLP_USERDATA, GWL_EXSTYLE, HWND_TOPMOST, LWA_ALPHA, MA_NOACTIVATE,
    PA_NOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_MOUSEACTIVATE, WM_NCCREATE, WM_NCDESTROY,
    WM_POINTERACTIVATE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

pub const SYNC_MESSAGE: u32 = 0x8000 + 2;
const OVERLAY_CLASS: windows::core::PCWSTR = w!("EduToolsOverlay");
const TARGET_DPI: f32 = 96.0;

pub struct OverlayManager {
    overlays: HashMap<String, OverlayWindow>,
    graphics: GraphicsDevice,
    owner: HWND,
    hinstance: HINSTANCE,
    annotation_enabled: bool,
    debug_overlay: bool,
    logger: Logger,
}

impl OverlayManager {
    pub fn new(owner: HWND, hinstance: HINSTANCE, logger: Logger) -> Result<Self> {
        register_window_class(hinstance)?;
        let graphics = GraphicsDevice::new()?;
        let debug_overlay = std::env::var_os("EDUTOOLS_DEBUG_OVERLAY") == Some(OsString::from("1"));
        if debug_overlay {
            logger
                .event("EDUTOOLS_DEBUG_OVERLAY=1: 개발 확인용 틴트/테두리 활성화 (제품 기능 아님)");
        }
        let mut manager = Self {
            overlays: HashMap::new(),
            graphics,
            owner,
            hinstance,
            annotation_enabled: false,
            debug_overlay,
            logger,
        };
        manager.synchronize("시작")?;
        Ok(manager)
    }

    pub fn synchronize(&mut self, reason: &str) -> Result<()> {
        // EnumDisplayMonitors can include invisible pseudo monitors from mirroring drivers. W10's
        // CCD active-source comparison will filter those; this round intentionally keeps them.
        let monitors = enumerate_monitors()?;
        let current = self
            .overlays
            .values()
            .map(|overlay| overlay.monitor().snapshot())
            .collect::<Vec<_>>();
        let discovered = monitors.iter().map(Monitor::snapshot).collect::<Vec<_>>();
        let plan = plan_monitor_sync(&current, &discovered);

        for key in &plan.removed {
            self.overlays.remove(key);
        }

        for monitor in &monitors {
            debug_assert!(monitor_by_key(&monitors, &monitor.session_key).is_some());
            if let Some(overlay) = self.overlays.get_mut(&monitor.session_key) {
                if overlay.monitor() != monitor {
                    overlay.update_monitor(
                        monitor.clone(),
                        self.annotation_enabled,
                        self.debug_overlay,
                    )?;
                }
                continue;
            }
            let overlay = OverlayWindow::new(
                self.owner,
                self.hinstance,
                monitor.clone(),
                &self.graphics,
                self.annotation_enabled,
                self.debug_overlay,
                self.logger.clone(),
            )?;
            self.overlays.insert(monitor.session_key.clone(), overlay);
        }

        self.logger.event(&format!(
            "오버레이 모니터 동기화 ({reason}): 추가={} 제거={} 이동={} 유지={} 총={}",
            plan.added.len(),
            plan.removed.len(),
            plan.moved.len(),
            plan.unchanged.len(),
            self.overlays.len()
        ));
        Ok(())
    }

    pub fn set_annotation_mode(&mut self, enabled: bool) -> Result<()> {
        self.annotation_enabled = enabled;
        for overlay in self.overlays.values_mut() {
            overlay.set_click_through(!enabled)?;
            overlay.draw(enabled && self.debug_overlay)?;
            // The transparent overlays stay created and visible in both modes. This avoids losing
            // per-monitor rendering state; only the shared click-through property changes.
            let _ = unsafe { ShowWindow(overlay.hwnd(), SW_SHOWNOACTIVATE) };
        }
        Ok(())
    }

    pub fn selftest_failures(&mut self) -> Vec<String> {
        let mut failures = Vec::new();
        if self.overlays.is_empty() {
            failures.push("overlay: 열거된 모니터/생성된 창이 없음".to_owned());
            return failures;
        }
        let required =
            (WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE).0
                as isize;
        for overlay in self.overlays.values_mut() {
            let key = overlay.monitor().session_key.clone();
            // SAFETY: every overlay stores a live HWND until it is dropped.
            let style = unsafe { GetWindowLongPtrW(overlay.hwnd(), GWL_EXSTYLE) };
            if style & required != required {
                failures.push(format!("overlay {key}: 필수 확장 스타일 누락 ({style:#x})"));
            }

            let mut actual = RECT::default();
            // SAFETY: actual is writable and the overlay HWND is live.
            let window_size = match unsafe { GetWindowRect(overlay.hwnd(), &mut actual) } {
                Ok(()) => {
                    if actual != overlay.monitor().win32_rect() {
                        failures.push(format!(
                            "overlay {key}: GetWindowRect {:?} != rcMonitor {:?}",
                            actual,
                            overlay.monitor().win32_rect()
                        ));
                    }
                    Some((actual.right - actual.left, actual.bottom - actual.top))
                }
                Err(error) => {
                    failures.push(format!("overlay {key}: GetWindowRect 실패: {error}"));
                    None
                }
            };
            if let Some((window_width, window_height)) = window_size {
                match overlay.swap_chain_size() {
                    Ok((swap_width, swap_height))
                        if i64::from(swap_width) == i64::from(window_width)
                            && i64::from(swap_height) == i64::from(window_height) => {}
                    Ok((swap_width, swap_height)) => failures.push(format!(
                        "overlay {key}: 스왑 체인 버퍼 {swap_width}x{swap_height} != 창 {window_width}x{window_height}"
                    )),
                    Err(error) => failures.push(format!(
                        "overlay {key}: 스왑 체인 버퍼 크기 조회 실패: {error}"
                    )),
                }
            }

            match overlay.set_click_through(true) {
                Ok(()) => {
                    // SAFETY: the HWND remains live across the style query.
                    let enabled = unsafe { GetWindowLongPtrW(overlay.hwnd(), GWL_EXSTYLE) };
                    if enabled & WS_EX_TRANSPARENT.0 as isize == 0 {
                        failures.push(format!("overlay {key}: click-through 켜기 실패"));
                    }
                }
                Err(error) => failures.push(format!(
                    "overlay {key}: set_click_through(true) 실패: {error}"
                )),
            }
            match overlay.set_click_through(false) {
                Ok(()) => {
                    // SAFETY: the HWND remains live across the style query.
                    let disabled = unsafe { GetWindowLongPtrW(overlay.hwnd(), GWL_EXSTYLE) };
                    if disabled & WS_EX_TRANSPARENT.0 as isize != 0 {
                        failures.push(format!("overlay {key}: click-through 끄기 실패"));
                    }
                }
                Err(error) => failures.push(format!(
                    "overlay {key}: set_click_through(false) 실패: {error}"
                )),
            }
            if let Err(error) = overlay.set_click_through(true) {
                failures.push(format!("overlay {key}: click-through 복원 실패: {error}"));
            }
        }
        failures
    }
}

fn register_window_class(hinstance: HINSTANCE) -> Result<()> {
    let class = WNDCLASSW {
        lpfnWndProc: Some(overlay_window_proc),
        hInstance: hinstance,
        lpszClassName: OVERLAY_CLASS,
        ..Default::default()
    };
    // SAFETY: the class data and static class name remain valid for process lifetime.
    if unsafe { RegisterClassW(&class) } == 0 {
        // SAFETY: read immediately after RegisterClassW reports failure.
        let error = unsafe { windows::Win32::Foundation::GetLastError() };
        if error != ERROR_CLASS_ALREADY_EXISTS {
            return Err(Error::from_thread());
        }
    }
    Ok(())
}

struct OverlayWindowState {
    owner: HWND,
    monitor: Monitor,
    logger: Logger,
}

struct OverlayWindow {
    hwnd: HWND,
    state: Box<OverlayWindowState>,
    renderer: Option<OverlayRenderer>,
}

impl OverlayWindow {
    #[allow(clippy::too_many_arguments)]
    fn new(
        owner: HWND,
        hinstance: HINSTANCE,
        monitor: Monitor,
        graphics: &GraphicsDevice,
        annotation_enabled: bool,
        debug_overlay: bool,
        logger: Logger,
    ) -> Result<Self> {
        let rect = monitor.rect;
        let mut state = Box::new(OverlayWindowState {
            owner,
            monitor,
            logger,
        });
        let state_ptr = state.as_mut() as *mut OverlayWindowState;
        let ex_style =
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP;
        // SAFETY: the class is registered and state_ptr stays stable in its Box until WM_NCDESTROY.
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                OVERLAY_CLASS,
                w!("EduTools Overlay"),
                WS_POPUP,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                None,
                None,
                Some(hinstance),
                Some(state_ptr.cast::<c_void>()),
            )
        }?;
        // This is the required creation-time placement, not a standalone z-band re-raise. Later
        // moves use SWP_NOZORDER; the full five-band ordering belongs to z_bands.rs.
        if let Err(error) = unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        } {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(error);
        }

        let renderer = match OverlayRenderer::new(
            graphics,
            hwnd,
            rect.width(),
            rect.height(),
            state.monitor.session_key.clone(),
            state.logger.clone(),
        ) {
            Ok(renderer) => renderer,
            Err(error) => {
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(error);
            }
        };
        let mut window = Self {
            hwnd,
            state,
            renderer: Some(renderer),
        };
        if let Err(error) = window.set_click_through(!annotation_enabled) {
            return Err(error);
        }
        window.draw(annotation_enabled && debug_overlay)?;
        // SAFETY: hwnd is a valid NOACTIVATE top-level window.
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        Ok(window)
    }

    fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn monitor(&self) -> &Monitor {
        &self.state.monitor
    }

    fn update_monitor(
        &mut self,
        monitor: Monitor,
        annotation_enabled: bool,
        debug_overlay: bool,
    ) -> Result<()> {
        let old = self.state.monitor.clone();
        self.state.monitor = monitor;
        let rect = self.state.monitor.rect;
        // Do not pass HWND_TOPMOST here: overlay-only z-order re-raising would violate the
        // five-band model. SWP_NOZORDER changes geometry only.
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }?;
        if old.rect != rect {
            self.renderer
                .as_mut()
                .expect("renderer lives until OverlayWindow::drop")
                .resize(rect.width(), rect.height())?;
        }
        self.draw(annotation_enabled && debug_overlay)
    }

    pub fn set_click_through(&mut self, enabled: bool) -> Result<()> {
        // SAFETY: hwnd is live and GWL_EXSTYLE contains a pointer-sized style bitset.
        let current = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) };
        let toggle = (WS_EX_TRANSPARENT | WS_EX_LAYERED).0 as isize;
        let updated = if enabled {
            current | toggle
        } else {
            current & !toggle
        };
        if updated != current {
            // A zero return can be a valid previous value, so clear and inspect last-error.
            unsafe { windows::Win32::Foundation::SetLastError(WIN32_ERROR(0)) };
            let previous = unsafe { SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, updated) };
            let error = unsafe { windows::Win32::Foundation::GetLastError() };
            if previous == 0 && error != ERROR_SUCCESS {
                return Err(Error::from_thread());
            }
        }
        if enabled {
            // Layered alpha stays fully opaque; DirectComposition supplies per-pixel transparency.
            unsafe { SetLayeredWindowAttributes(self.hwnd, COLORREF(0), 255, LWA_ALPHA) }?;
        }
        Ok(())
    }

    fn draw(&mut self, debug_visible: bool) -> Result<()> {
        self.renderer
            .as_mut()
            .expect("renderer lives until OverlayWindow::drop")
            .draw(debug_visible)
    }

    fn swap_chain_size(&self) -> Result<(u32, u32)> {
        self.renderer
            .as_ref()
            .expect("renderer lives until OverlayWindow::drop")
            .swap_chain_size()
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        // Release D2D/DComp resources before destroying their target HWND. State remains alive so
        // the window procedure can safely process WM_NCDESTROY.
        self.renderer.take();
        if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
            self.state
                .logger
                .event(&format!("오버레이 창 제거 실패: {error}"));
        }
    }
}

unsafe extern "system" fn overlay_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // SAFETY: WM_NCCREATE lParam is a valid CREATESTRUCTW for this call.
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let state = create.lpCreateParams as *mut OverlayWindowState;
        // SAFETY: the pointer refers to a Box that outlives the HWND.
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
    }
    // SAFETY: GWLP_USERDATA is either zero or the stable Box pointer installed above.
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayWindowState };
    if !state.is_null() {
        // SAFETY: all overlay window handling occurs on the UI thread before the Box is dropped.
        let state = unsafe { &mut *state };
        match message {
            WM_DPICHANGED => {
                let rect = state.monitor.rect;
                let result = unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        rect.left,
                        rect.top,
                        rect.width(),
                        rect.height(),
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                };
                match result {
                    Ok(()) => state.logger.event(&format!(
                        "오버레이 WM_DPICHANGED: key={} rcMonitor 적용, SetWindowPos 호출=1",
                        state.monitor.session_key
                    )),
                    Err(error) => state.logger.event(&format!(
                        "오버레이 WM_DPICHANGED 배치 실패: key={}, {error}",
                        state.monitor.session_key
                    )),
                }
                post_sync(state, "WM_DPICHANGED");
                return LRESULT(0);
            }
            WM_DISPLAYCHANGE => {
                post_sync(state, "WM_DISPLAYCHANGE");
                return LRESULT(0);
            }
            WM_POINTERACTIVATE => return LRESULT(PA_NOACTIVATE as isize),
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_NCDESTROY => {
                // SAFETY: clearing prevents later messages from observing state during teardown.
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            _ => {}
        }
    }
    // SAFETY: all unhandled messages retain default Win32 behavior.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn post_sync(state: &OverlayWindowState, reason: &str) {
    if state.owner.is_invalid() {
        return;
    }
    // SAFETY: owner is the UI thread's hidden main window and the message has no pointer payload.
    if let Err(error) = unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(state.owner),
            SYNC_MESSAGE,
            WPARAM(0),
            LPARAM(0),
        )
    } {
        state
            .logger
            .event(&format!("오버레이 {reason} 동기화 요청 실패: {error}"));
    }
}

struct GraphicsDevice {
    _d3d_device: ID3D11Device,
    d2d_device: ID2D1Device,
    dxgi_factory: IDXGIFactory2,
    dcomp_device: IDCompositionDevice,
}

impl GraphicsDevice {
    fn new() -> Result<Self> {
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
            _d3d_device: d3d_device,
            d2d_device,
            dxgi_factory,
            dcomp_device,
        })
    }
}

struct OverlayRenderer {
    hwnd: HWND,
    swap_chain: IDXGISwapChain1,
    context: ID2D1DeviceContext,
    target_bitmap: Option<ID2D1Bitmap1>,
    _dcomp_target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    width: u32,
    height: u32,
    monitor_key: String,
    logger: Logger,
}

impl OverlayRenderer {
    fn new(
        graphics: &GraphicsDevice,
        hwnd: HWND,
        width: i32,
        height: i32,
        monitor_key: String,
        logger: Logger,
    ) -> Result<Self> {
        let width = valid_dimension(width)?;
        let height = valid_dimension(height)?;
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
        // SAFETY: the shared D3D device and immutable description are valid for this call.
        let swap_chain = unsafe {
            graphics.dxgi_factory.CreateSwapChainForComposition(
                &graphics._d3d_device,
                &description,
                None::<&IDXGIOutput>,
            )
        }?;
        // SAFETY: the D2D device is live and no multithreaded access occurs.
        let context = unsafe {
            graphics
                .d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
        }?;
        unsafe { context.SetDpi(TARGET_DPI, TARGET_DPI) };
        let target_bitmap = Some(create_target_bitmap(&context, &swap_chain)?);
        let target_image: ID2D1Image = target_bitmap
            .as_ref()
            .expect("target bitmap was just created")
            .cast()?;
        unsafe { context.SetTarget(&target_image) };

        // DirectComposition initialization order is deliberate: D3D/D2D shared device, composition
        // swap chain, D2D target, then HWND target + visual content + root + commit.
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
            target_bitmap,
            _dcomp_target: dcomp_target,
            _visual: visual,
            width,
            height,
            monitor_key,
            logger,
        };
        renderer.log_size()?;
        Ok(renderer)
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        let width = valid_dimension(width)?;
        let height = valid_dimension(height)?;
        // SAFETY: detaching the target releases the swap-chain buffer before ResizeBuffers.
        unsafe { self.context.SetTarget(None::<&ID2D1Image>) };
        self.target_bitmap = None;
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
        let bitmap = create_target_bitmap(&self.context, &self.swap_chain)?;
        let image: ID2D1Image = bitmap.cast()?;
        unsafe { self.context.SetTarget(&image) };
        self.target_bitmap = Some(bitmap);
        self.width = width;
        self.height = height;
        self.log_size()?;
        Ok(())
    }

    fn swap_chain_size(&self) -> Result<(u32, u32)> {
        let description = unsafe { self.swap_chain.GetDesc1() }?;
        Ok((description.Width, description.Height))
    }

    fn log_size(&self) -> Result<()> {
        let (swap_width, swap_height) = self.swap_chain_size()?;
        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut window_rect) }?;
        let target = self
            .target_bitmap
            .as_ref()
            .expect("target bitmap is attached outside ResizeBuffers");
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

    fn draw(&mut self, debug_visible: bool) -> Result<()> {
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
        }
        if let Some(brush) = debug_brush {
            // 창, 스왑 체인, D2D 타깃은 물리 픽셀이고 96 DPI에서 D2D 1단위는 1픽셀이다.
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
        unsafe {
            self.context.EndDraw(None, None)?;
            self.swap_chain.Present(1, Default::default()).ok()?;
        }
        Ok(())
    }
}

fn create_target_bitmap(
    context: &ID2D1DeviceContext,
    swap_chain: &IDXGISwapChain1,
) -> Result<ID2D1Bitmap1> {
    let surface: IDXGISurface = unsafe { swap_chain.GetBuffer(0) }?;
    let properties = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: TARGET_DPI,
        dpiY: TARGET_DPI,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };
    unsafe { context.CreateBitmapFromDxgiSurface(&surface, Some(&properties)) }
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
