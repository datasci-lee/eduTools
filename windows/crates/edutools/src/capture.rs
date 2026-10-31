use crate::log::Logger;
use crate::render::GraphicsDevice;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::core::{factory, Error, IInspectable, Interface, Result};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub const WM_APP_ZOOM_FRAME: u32 = 0x8000 + 140;
pub const WM_APP_ZOOM_CAPTURE_FAILED: u32 = 0x8000 + 141;
pub const WM_APP_ZOOM_RECREATE: u32 = 0x8000 + 144;
pub const WM_APP_EXPORT_FRAME: u32 = 0x8000 + 330;
pub const WM_APP_EXPORT_CAPTURE_FAILED: u32 = 0x8000 + 331;
pub const WM_APP_EXPORT_RECREATE: u32 = 0x8000 + 332;
pub const WM_APP_EXPORT_COMPLETE: u32 = 0x8000 + 333;

#[derive(Clone, Copy)]
struct CaptureConfig {
    label: &'static str,
    buffer_count: i32,
    cursor_enabled: bool,
    frame_message: u32,
    failure_message: u32,
    recreate_message: Option<u32>,
    one_shot: bool,
}

impl CaptureConfig {
    const fn zoom() -> Self {
        Self {
            label: "줌",
            buffer_count: 3,
            cursor_enabled: false,
            frame_message: WM_APP_ZOOM_FRAME,
            failure_message: WM_APP_ZOOM_CAPTURE_FAILED,
            recreate_message: Some(WM_APP_ZOOM_RECREATE),
            one_shot: false,
        }
    }

    const fn export() -> Self {
        Self {
            label: "내보내기",
            buffer_count: 3,
            cursor_enabled: true,
            frame_message: WM_APP_EXPORT_FRAME,
            failure_message: WM_APP_EXPORT_CAPTURE_FAILED,
            recreate_message: Some(WM_APP_EXPORT_RECREATE),
            one_shot: true,
        }
    }
}

#[derive(Clone)]
pub struct CapturedFrame {
    pub texture: ID3D11Texture2D,
    pub size: SizeInt32,
    _frame: Direct3D11CaptureFrame,
}

// The capture device is created without D3D11_CREATE_DEVICE_SINGLETHREADED. D3D11 resources are
// free-threaded COM objects, while windows-rs conservatively leaves Win32 interfaces !Send/!Sync.
unsafe impl Send for CapturedFrame {}
unsafe impl Sync for CapturedFrame {}

struct FrameStats {
    interval_started: Instant,
    interval_frames: u64,
}

struct FrameSlot {
    owner: isize,
    latest: Mutex<Option<CapturedFrame>>,
    failure: Mutex<Option<String>>,
    current_size: Mutex<SizeInt32>,
    pending_size: Mutex<Option<SizeInt32>>,
    stopping: AtomicBool,
    accepting: AtomicBool,
    overwritten: AtomicU64,
    received: AtomicU64,
    stats: Mutex<FrameStats>,
    logger: Logger,
    config: CaptureConfig,
}

impl FrameSlot {
    fn owner(&self) -> HWND {
        HWND(self.owner as *mut std::ffi::c_void)
    }

    fn fail(&self, message: String) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        let should_post = self
            .failure
            .lock()
            .map(|mut failure| {
                if failure.is_some() {
                    false
                } else {
                    *failure = Some(message.clone());
                    true
                }
            })
            .unwrap_or(false);
        if should_post {
            self.logger
                .event(&format!("{} 캡처 실패: {message}", self.config.label));
            let _ = unsafe {
                PostMessageW(
                    Some(self.owner()),
                    self.config.failure_message,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    }

    fn record_frame(&self, frame: CapturedFrame) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        if self.config.one_shot {
            if !self.accepting.swap(false, Ordering::AcqRel) {
                return;
            }
        } else if !self.accepting.load(Ordering::Acquire) {
            return;
        }
        self.received.fetch_add(1, Ordering::Relaxed);
        if !self.config.one_shot {
            if let Ok(mut stats) = self.stats.lock() {
                stats.interval_frames += 1;
                let elapsed = stats.interval_started.elapsed();
                if elapsed >= Duration::from_secs(1) {
                    let fps = stats.interval_frames as f64 / elapsed.as_secs_f64();
                    self.logger.event(&format!(
                        "줌 캡처 프레임: fps={fps:.1} 누적={} 슬롯덮어쓰기={}",
                        self.received.load(Ordering::Relaxed),
                        self.overwritten.load(Ordering::Relaxed)
                    ));
                    stats.interval_started = Instant::now();
                    stats.interval_frames = 0;
                }
            }
        }
        let should_post = match self.latest.lock() {
            Ok(mut latest) => {
                let was_empty = latest.is_none();
                if !was_empty {
                    self.overwritten.fetch_add(1, Ordering::Relaxed);
                }
                *latest = Some(frame);
                was_empty
            }
            Err(_) => {
                self.fail("최신 프레임 슬롯 잠금이 손상되었습니다.".to_owned());
                false
            }
        };
        if should_post {
            if unsafe {
                PostMessageW(
                    Some(self.owner()),
                    self.config.frame_message,
                    WPARAM(0),
                    LPARAM(0),
                )
            }
            .is_err()
            {
                self.fail(format!(
                    "{} 프레임을 UI 스레드로 전달하지 못했습니다.",
                    self.config.label
                ));
            }
        }
    }
}

pub struct ZoomCapture {
    frame_pool: Option<Direct3D11CaptureFramePool>,
    session: Option<GraphicsCaptureSession>,
    item: Option<GraphicsCaptureItem>,
    frame_arrived_token: Option<i64>,
    slot: Arc<FrameSlot>,
    winrt_device: IDirect3DDevice,
    logger: Logger,
    border_disabled: bool,
}

impl ZoomCapture {
    pub fn is_supported() -> Result<bool> {
        GraphicsCaptureSession::IsSupported()
    }

    pub fn start(
        owner: HWND,
        monitor: HMONITOR,
        physical_size: SizeInt32,
        graphics: &GraphicsDevice,
        logger: Logger,
    ) -> Result<Self> {
        Self::start_configured(
            owner,
            monitor,
            physical_size,
            graphics,
            logger,
            CaptureConfig::zoom(),
        )
    }

    fn start_configured(
        owner: HWND,
        monitor: HMONITOR,
        physical_size: SizeInt32,
        graphics: &GraphicsDevice,
        logger: Logger,
        config: CaptureConfig,
    ) -> Result<Self> {
        if !Self::is_supported()
            .map_err(|error| capture_call_error("GraphicsCaptureSession::IsSupported", error))?
        {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "이 Windows 환경은 Windows Graphics Capture를 지원하지 않습니다.",
            ));
        }
        let d3d_device = graphics.d3d_device();
        let winrt_device = create_winrt_device(&d3d_device)?;
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|error| capture_call_error("GraphicsCaptureItem activation factory", error))?;
        let item: GraphicsCaptureItem =
            unsafe { interop.CreateForMonitor(monitor) }.map_err(|error| {
                capture_call_error("IGraphicsCaptureItemInterop::CreateForMonitor", error)
            })?;
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            config.buffer_count,
            physical_size,
        )
        .map_err(|error| {
            capture_call_error("Direct3D11CaptureFramePool::CreateFreeThreaded", error)
        })?;
        let session = match frame_pool.CreateCaptureSession(&item) {
            Ok(session) => session,
            Err(error) => {
                let _ = frame_pool.Close();
                return Err(capture_call_error(
                    "Direct3D11CaptureFramePool::CreateCaptureSession",
                    error,
                ));
            }
        };
        if let Err(error) = session.SetIsCursorCaptureEnabled(config.cursor_enabled) {
            let _ = frame_pool.Close();
            let _ = session.Close();
            return Err(capture_call_error(
                "GraphicsCaptureSession::SetIsCursorCaptureEnabled",
                error,
            ));
        }
        let cursor_capture_enabled = match session.IsCursorCaptureEnabled() {
            Ok(enabled) => enabled,
            Err(error) => {
                let _ = frame_pool.Close();
                let _ = session.Close();
                return Err(capture_call_error(
                    "GraphicsCaptureSession::IsCursorCaptureEnabled",
                    error,
                ));
            }
        };
        if cursor_capture_enabled != config.cursor_enabled {
            let _ = frame_pool.Close();
            let _ = session.Close();
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "WGC 캡처의 시스템 커서 설정을 확인하지 못했습니다.",
            ));
        }
        let border_disabled = match session.SetIsBorderRequired(false) {
            Ok(()) => match session.IsBorderRequired() {
                Ok(false) => true,
                Ok(true) => {
                    logger.event(
                        "WGC 경계 숨김 요청은 반환됐지만 IsBorderRequired가 true여서 캡처는 계속함",
                    );
                    false
                }
                Err(error) => {
                    logger.event(&format!(
                        "WGC 경계 숨김 결과 조회 실패, 캡처는 계속함: {error}"
                    ));
                    false
                }
            },
            Err(error) => {
                logger.event(&format!("WGC 경계 숨김 요청 실패, 캡처는 계속함: {error}"));
                false
            }
        };

        let slot = Arc::new(FrameSlot {
            owner: owner.0 as isize,
            latest: Mutex::new(None),
            failure: Mutex::new(None),
            current_size: Mutex::new(physical_size),
            pending_size: Mutex::new(None),
            stopping: AtomicBool::new(false),
            accepting: AtomicBool::new(true),
            overwritten: AtomicU64::new(0),
            received: AtomicU64::new(0),
            stats: Mutex::new(FrameStats {
                interval_started: Instant::now(),
                interval_frames: 0,
            }),
            logger: logger.clone(),
            config,
        });
        let callback_slot = slot.clone();
        let handler: TypedEventHandler<Direct3D11CaptureFramePool, IInspectable> =
            TypedEventHandler::new(
                move |sender: windows::core::Ref<Direct3D11CaptureFramePool>,
                      _: windows::core::Ref<IInspectable>| {
                    let Some(pool) = sender.as_ref() else {
                        callback_slot
                            .fail("WGC 프레임 풀 콜백의 sender가 비어 있습니다.".to_owned());
                        return Ok(());
                    };
                    if callback_slot.stopping.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let frame = match pool.TryGetNextFrame() {
                        Ok(frame) => frame,
                        Err(error) => {
                            callback_slot.fail(format!("WGC 프레임을 꺼내지 못했습니다: {error}"));
                            return Ok(());
                        }
                    };
                    let size = match frame.ContentSize() {
                        Ok(size) if size.Width > 0 && size.Height > 0 => size,
                        Ok(size) => {
                            callback_slot.fail(format!(
                                "WGC가 잘못된 프레임 크기를 반환했습니다: {}x{}",
                                size.Width, size.Height
                            ));
                            return Ok(());
                        }
                        Err(error) => {
                            callback_slot
                                .fail(format!("WGC 프레임 크기를 읽지 못했습니다: {error}"));
                            return Ok(());
                        }
                    };
                    let size_changed = callback_slot
                        .current_size
                        .lock()
                        .map(|mut current| {
                            if *current == size {
                                false
                            } else {
                                *current = size;
                                true
                            }
                        })
                        .unwrap_or(false);
                    if size_changed {
                        if let Some(recreate_message) = callback_slot.config.recreate_message {
                            callback_slot.logger.event(&format!(
                                "WGC ContentSize 변경: {}x{}, 프레임 풀 재생성",
                                size.Width, size.Height
                            ));
                            if let Ok(mut pending) = callback_slot.pending_size.lock() {
                                *pending = Some(size);
                            }
                            if unsafe {
                                PostMessageW(
                                    Some(callback_slot.owner()),
                                    recreate_message,
                                    WPARAM(0),
                                    LPARAM(0),
                                )
                            }
                            .is_err()
                            {
                                callback_slot.fail(
                                    "WGC 프레임 풀 재생성 요청을 UI 스레드로 보내지 못했습니다."
                                        .to_owned(),
                                );
                            }
                        } else {
                            callback_slot.fail(format!(
                                "WGC 단일 프레임 크기가 변경되었습니다: {}x{}",
                                size.Width, size.Height
                            ));
                        }
                        return Ok(());
                    }
                    let captured = (|| -> Result<CapturedFrame> {
                        let surface = frame.Surface()?;
                        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
                        let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
                        Ok(CapturedFrame {
                            texture,
                            size,
                            _frame: frame,
                        })
                    })();
                    match captured {
                        Ok(frame) => callback_slot.record_frame(frame),
                        Err(error) => callback_slot.fail(format!(
                            "WGC 프레임의 D3D11 텍스처를 열지 못했습니다: {error}"
                        )),
                    }
                    Ok(())
                },
            );
        let token = match frame_pool.FrameArrived(&handler) {
            Ok(token) => token,
            Err(error) => {
                let _ = frame_pool.Close();
                let _ = session.Close();
                return Err(capture_call_error(
                    "Direct3D11CaptureFramePool::FrameArrived",
                    error,
                ));
            }
        };
        if let Err(error) = session.StartCapture() {
            let _ = frame_pool.RemoveFrameArrived(token);
            let _ = frame_pool.Close();
            let _ = session.Close();
            return Err(capture_call_error(
                "GraphicsCaptureSession::StartCapture",
                error,
            ));
        }
        logger.event(&format!(
            "WGC 시작: purpose={} physical={}x{} buffers={} cursor={} borderRequest={}",
            config.label,
            physical_size.Width,
            physical_size.Height,
            config.buffer_count,
            config.cursor_enabled,
            if border_disabled {
                "accepted"
            } else {
                "rejected"
            }
        ));
        Ok(Self {
            frame_pool: Some(frame_pool),
            session: Some(session),
            item: Some(item),
            frame_arrived_token: Some(token),
            slot,
            winrt_device,
            logger,
            border_disabled,
        })
    }

    pub fn take_latest(&self) -> Option<CapturedFrame> {
        self.slot.latest.lock().ok()?.take()
    }

    pub fn take_failure(&self) -> Option<String> {
        self.slot.failure.lock().ok()?.take()
    }

    pub fn set_accepting_frames(&self, accepting: bool) {
        self.slot.accepting.store(accepting, Ordering::Release);
        if !accepting {
            if let Ok(mut latest) = self.slot.latest.lock() {
                *latest = None;
            }
        }
    }

    pub fn recreate_if_needed(&self) -> Result<bool> {
        let size = self
            .slot
            .pending_size
            .lock()
            .ok()
            .and_then(|mut size| size.take());
        let Some(size) = size else {
            return Ok(false);
        };
        let Some(pool) = self.frame_pool.as_ref() else {
            return Ok(false);
        };
        pool.Recreate(
            &self.winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            3,
            size,
        )?;
        Ok(true)
    }

    pub const fn border_disabled(&self) -> bool {
        self.border_disabled
    }

    pub fn overwritten_count(&self) -> u64 {
        self.slot.overwritten.load(Ordering::Relaxed)
    }

    fn stop(&mut self) {
        if self.slot.stopping.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut latest) = self.slot.latest.lock() {
            *latest = None;
        }
        if let (Some(pool), Some(token)) =
            (self.frame_pool.as_ref(), self.frame_arrived_token.take())
        {
            if let Err(error) = pool.RemoveFrameArrived(token) {
                self.logger
                    .event(&format!("WGC FrameArrived 해제 실패: {error}"));
            }
        }
        if let Some(pool) = self.frame_pool.take() {
            if let Err(error) = pool.Close() {
                self.logger
                    .event(&format!("WGC 프레임 풀 종료 실패: {error}"));
            }
        }
        if let Some(session) = self.session.take() {
            if let Err(error) = session.Close() {
                self.logger.event(&format!("WGC 세션 종료 실패: {error}"));
            }
        }
        self.item = None;
        self.logger.event(&format!(
            "WGC 종료: purpose={} frames={} slotOverwrites={}",
            self.slot.config.label,
            self.slot.received.load(Ordering::Relaxed),
            self.slot.overwritten.load(Ordering::Relaxed)
        ));
        let _ = &self.winrt_device;
    }
}

impl Drop for ZoomCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct OneFrameCapture(ZoomCapture);

impl OneFrameCapture {
    pub fn start(
        owner: HWND,
        monitor: HMONITOR,
        physical_size: SizeInt32,
        graphics: &GraphicsDevice,
        logger: Logger,
    ) -> Result<Self> {
        ZoomCapture::start_configured(
            owner,
            monitor,
            physical_size,
            graphics,
            logger,
            CaptureConfig::export(),
        )
        .map(Self)
    }

    pub fn take_frame(&self) -> Option<CapturedFrame> {
        self.0.take_latest()
    }

    pub fn take_failure(&self) -> Option<String> {
        self.0.take_failure()
    }

    pub fn recreate_if_needed(&self) -> Result<bool> {
        self.0.recreate_if_needed()
    }
}

fn create_winrt_device(device: &ID3D11Device) -> Result<IDirect3DDevice> {
    let dxgi_device: IDXGIDevice = device
        .cast()
        .map_err(|error| capture_call_error("ID3D11Device::cast<IDXGIDevice>", error))?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
        .map_err(|error| capture_call_error("CreateDirect3D11DeviceFromDXGIDevice", error))?;
    inspectable
        .cast()
        .map_err(|error| capture_call_error("IInspectable::cast<IDirect3DDevice>", error))
}

fn capture_call_error(operation: &str, error: Error) -> Error {
    Error::new(error.code(), format!("{operation} 실패: {error}"))
}
