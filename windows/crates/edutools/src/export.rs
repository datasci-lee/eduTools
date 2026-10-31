//! PNG/PDF/clipboard export. Pure container/layout rules stay available to native tests.

use std::path::{Path, PathBuf};

const DIBV5_HEADER_SIZE: usize = 124;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BgraImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl BgraImage {
    pub fn from_tight(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        let expected = image_byte_count(width, height)?;
        if pixels.len() != expected {
            return Err(format!(
                "BGRA byte count mismatch: expected {expected}, got {}",
                pixels.len()
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn from_pitched(
        width: u32,
        height: u32,
        pitch: u32,
        pixels: &[u8],
    ) -> Result<Self, String> {
        let row_bytes = usize::try_from(width)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| "BGRA row size overflow".to_owned())?;
        let pitch = pitch as usize;
        if pitch < row_bytes {
            return Err("BGRA pitch is shorter than a pixel row".to_owned());
        }
        let required = pitch
            .checked_mul(height as usize)
            .ok_or_else(|| "BGRA pitched size overflow".to_owned())?;
        if pixels.len() < required {
            return Err("BGRA pitched buffer is truncated".to_owned());
        }
        let mut tight = Vec::with_capacity(image_byte_count(width, height)?);
        for row in 0..height as usize {
            tight.extend_from_slice(&pixels[row * pitch..row * pitch + row_bytes]);
        }
        Self::from_tight(width, height, tight)
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn stride(&self) -> u32 {
        self.width * 4
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn composite_premultiplied(
    background: &BgraImage,
    foreground: &BgraImage,
) -> Result<BgraImage, String> {
    if background.width != foreground.width || background.height != foreground.height {
        return Err("background and foreground dimensions differ".to_owned());
    }
    let mut pixels = background.pixels.clone();
    for (destination, source) in pixels
        .chunks_exact_mut(4)
        .zip(foreground.pixels.chunks_exact(4))
    {
        let source_alpha = u32::from(source[3]);
        let inverse = 255 - source_alpha;
        for channel in 0..3 {
            destination[channel] = (u32::from(source[channel])
                + (u32::from(destination[channel]) * inverse + 127) / 255)
                .min(255) as u8;
        }
        destination[3] =
            (source_alpha + (u32::from(destination[3]) * inverse + 127) / 255).min(255) as u8;
    }
    BgraImage::from_tight(background.width, background.height, pixels)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportTimestamp {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

pub fn export_base_name(timestamp: ExportTimestamp, display_id: &str) -> String {
    let display = display_id
        .strip_prefix(r"\\.\")
        .unwrap_or(display_id)
        .chars()
        .map(|value| {
            if value.is_alphanumeric() || matches!(value, '-' | '_') {
                value
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{display}",
        timestamp.year,
        timestamp.month,
        timestamp.day,
        timestamp.hour,
        timestamp.minute,
        timestamp.second
    )
}

pub fn unique_file_name(
    base_name: &str,
    extension: &str,
    mut exists: impl FnMut(&str) -> bool,
) -> Result<String, String> {
    for index in 1..=10_000 {
        let suffix = if index == 1 {
            String::new()
        } else {
            format!("-{index}")
        };
        let candidate = format!("{base_name}{suffix}.{extension}");
        if !exists(&candidate) {
            return Ok(candidate);
        }
    }
    Err("could not allocate a collision-free export name".to_owned())
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn unique_export_path(
    directory: &Path,
    base_name: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    unique_file_name(base_name, extension, |name| directory.join(name).exists())
        .map(|name| directory.join(name))
}

pub fn dibv5_bytes(image: &BgraImage) -> Result<Vec<u8>, String> {
    let pixel_bytes = image_byte_count(image.width, image.height)?;
    let mut output = vec![0_u8; DIBV5_HEADER_SIZE + pixel_bytes];
    put_u32(&mut output, 0, DIBV5_HEADER_SIZE as u32);
    put_i32(
        &mut output,
        4,
        i32::try_from(image.width).map_err(|_| "DIB width is too large")?,
    );
    put_i32(
        &mut output,
        8,
        i32::try_from(image.height).map_err(|_| "DIB height is too large")?,
    );
    put_u16(&mut output, 12, 1);
    put_u16(&mut output, 14, 32);
    put_u32(&mut output, 16, 3); // BI_BITFIELDS
    put_u32(
        &mut output,
        20,
        u32::try_from(pixel_bytes).map_err(|_| "DIB image is too large")?,
    );
    put_i32(&mut output, 24, 3_780); // 96 DPI in pixels/metre
    put_i32(&mut output, 28, 3_780);
    put_u32(&mut output, 40, 0x00ff_0000);
    put_u32(&mut output, 44, 0x0000_ff00);
    put_u32(&mut output, 48, 0x0000_00ff);
    put_u32(&mut output, 52, 0xff00_0000);
    put_u32(&mut output, 56, 0x7352_4742); // LCS_sRGB
    put_u32(&mut output, 108, 4); // LCS_GM_IMAGES

    let row_bytes = image.width as usize * 4;
    for destination_row in 0..image.height as usize {
        let source_row = image.height as usize - 1 - destination_row;
        let source = &image.pixels[source_row * row_bytes..(source_row + 1) * row_bytes];
        let destination_start = DIBV5_HEADER_SIZE + destination_row * row_bytes;
        let destination = &mut output[destination_start..destination_start + row_bytes];
        for (source, destination) in source.chunks_exact(4).zip(destination.chunks_exact_mut(4)) {
            destination.copy_from_slice(&unpremultiplied_pixel(source));
        }
    }
    Ok(output)
}

#[derive(Clone, Copy)]
pub struct PdfPage<'a> {
    pub jpeg: &'a [u8],
    pub pixel_width: u32,
    pub pixel_height: u32,
}

pub fn build_pdf(
    pages: &[PdfPage<'_>],
    page_width: f64,
    page_height: f64,
) -> Result<Vec<u8>, String> {
    if pages.is_empty() || page_width <= 0.0 || page_height <= 0.0 {
        return Err("PDF requires at least one positive-sized page".to_owned());
    }
    if pages
        .iter()
        .any(|page| page.jpeg.is_empty() || page.pixel_width == 0 || page.pixel_height == 0)
    {
        return Err("PDF page image is empty".to_owned());
    }

    let object_count = 2 + pages.len() * 3;
    let mut output = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0_usize; object_count + 1];
    append_object(
        &mut output,
        &mut offsets,
        1,
        b"<< /Type /Catalog /Pages 2 0 R >>",
    );

    let kids = pages
        .iter()
        .enumerate()
        .map(|(index, _)| format!("{} 0 R", 3 + index * 3))
        .collect::<Vec<_>>()
        .join(" ");
    append_object(
        &mut output,
        &mut offsets,
        2,
        format!("<< /Type /Pages /Count {} /Kids [{kids}] >>", pages.len()).as_bytes(),
    );

    for (index, page) in pages.iter().enumerate() {
        let page_object = 3 + index * 3;
        let image_object = page_object + 1;
        let content_object = page_object + 2;
        append_object(
            &mut output,
            &mut offsets,
            page_object,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_width:.3} {page_height:.3}] /Resources << /XObject << /Im{} {image_object} 0 R >> >> /Contents {content_object} 0 R >>",
                index + 1
            )
            .as_bytes(),
        );

        offsets[image_object] = output.len();
        output.extend_from_slice(format!("{image_object} 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n", page.pixel_width, page.pixel_height, page.jpeg.len()).as_bytes());
        output.extend_from_slice(page.jpeg);
        output.extend_from_slice(b"\nendstream\nendobj\n");

        let content = format!(
            "q\n{page_width:.3} 0 0 {page_height:.3} 0 0 cm\n/Im{} Do\nQ\n",
            index + 1
        );
        offsets[content_object] = output.len();
        output.extend_from_slice(
            format!(
                "{content_object} 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n",
                content.len()
            )
            .as_bytes(),
        );
    }

    let xref_offset = output.len();
    output.extend_from_slice(format!("xref\n0 {}\n", object_count + 1).as_bytes());
    output.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    output.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            object_count + 1
        )
        .as_bytes(),
    );
    validate_pdf_structure(&output, pages.len())?;
    Ok(output)
}

pub fn validate_pdf_structure(data: &[u8], expected_pages: usize) -> Result<(), String> {
    if !data.starts_with(b"%PDF-1.4") || !data.ends_with(b"%%EOF\n") {
        return Err("PDF header or EOF marker is missing".to_owned());
    }
    let startxref_marker =
        find_last(data, b"startxref\n").ok_or_else(|| "PDF startxref is missing".to_owned())?;
    let value_start = startxref_marker + b"startxref\n".len();
    let value_end = data[value_start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| value_start + offset)
        .ok_or_else(|| "PDF startxref value is unterminated".to_owned())?;
    let xref_offset = std::str::from_utf8(&data[value_start..value_end])
        .map_err(|_| "PDF startxref is not ASCII")?
        .parse::<usize>()
        .map_err(|_| "PDF startxref is not numeric")?;
    if data.get(xref_offset..xref_offset + 5) != Some(b"xref\n") {
        return Err("PDF startxref does not point to xref".to_owned());
    }

    let xref_text = std::str::from_utf8(&data[xref_offset..startxref_marker])
        .map_err(|_| "PDF xref is not ASCII")?;
    let mut lines = xref_text.lines();
    if lines.next() != Some("xref") {
        return Err("PDF xref header is invalid".to_owned());
    }
    let range = lines
        .next()
        .ok_or_else(|| "PDF xref range is missing".to_owned())?;
    let mut range = range.split_whitespace();
    if range.next() != Some("0") {
        return Err("PDF xref does not start at object zero".to_owned());
    }
    let count = range
        .next()
        .ok_or_else(|| "PDF xref count is missing".to_owned())?
        .parse::<usize>()
        .map_err(|_| "PDF xref count is invalid")?;
    let entries = lines.take(count).collect::<Vec<_>>();
    if entries.len() != count || entries.first().is_none_or(|entry| !entry.ends_with(" f ")) {
        return Err("PDF xref entries are incomplete".to_owned());
    }
    for (object, entry) in entries.iter().enumerate().skip(1) {
        let offset = entry
            .get(..10)
            .ok_or_else(|| "PDF xref entry is short".to_owned())?
            .parse::<usize>()
            .map_err(|_| "PDF xref offset is invalid")?;
        let marker = format!("{object} 0 obj\n");
        if data.get(offset..offset + marker.len()) != Some(marker.as_bytes()) {
            return Err(format!("PDF xref offset for object {object} is incorrect"));
        }
    }
    let expected_count = format!("/Count {expected_pages}");
    if find_first(data, expected_count.as_bytes()).is_none() {
        return Err("PDF page count is incorrect".to_owned());
    }
    if count != 3 + expected_pages * 3 {
        return Err("PDF xref object count is incorrect".to_owned());
    }
    Ok(())
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn pure_selftest_failures() -> Vec<String> {
    let mut failures = Vec::new();
    let jpeg_a = [0xff, 0xd8, 0x10, 0x20, 0xff, 0xd9];
    let jpeg_b = [0xff, 0xd8, 0x30, 0x40, 0xff, 0xd9];
    let pages = [
        PdfPage {
            jpeg: &jpeg_a,
            pixel_width: 2,
            pixel_height: 1,
        },
        PdfPage {
            jpeg: &jpeg_b,
            pixel_width: 2,
            pixel_height: 1,
        },
    ];
    match build_pdf(&pages, 100.0, 50.0) {
        Ok(pdf) => {
            if let Err(error) = validate_pdf_structure(&pdf, 2) {
                failures.push(format!("export PDF: {error}"));
            }
        }
        Err(error) => failures.push(format!("export PDF build: {error}")),
    }
    failures
}

fn append_object(output: &mut Vec<u8>, offsets: &mut [usize], object: usize, body: &[u8]) {
    offsets[object] = output.len();
    output.extend_from_slice(format!("{object} 0 obj\n").as_bytes());
    output.extend_from_slice(body);
    output.extend_from_slice(b"\nendobj\n");
}

fn image_byte_count(width: u32, height: u32) -> Result<usize, String> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "BGRA image size overflow".to_owned())
}

fn unpremultiplied_pixel(pixel: &[u8]) -> [u8; 4] {
    let alpha = u32::from(pixel[3]);
    if alpha == 0 {
        return [0, 0, 0, 0];
    }
    let channel = |value: u8| ((u32::from(value) * 255 + alpha / 2) / alpha).min(255) as u8;
    [
        channel(pixel[0]),
        channel(pixel[1]),
        channel(pixel[2]),
        pixel[3],
    ]
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn find_first(data: &[u8], needle: &[u8]) -> Option<usize> {
    data.windows(needle.len())
        .position(|window| window == needle)
}

fn find_last(data: &[u8], needle: &[u8]) -> Option<usize> {
    data.windows(needle.len())
        .rposition(|window| window == needle)
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        build_pdf, composite_premultiplied, dibv5_bytes, export_base_name, unique_export_path,
        BgraImage, ExportTimestamp, PdfPage,
    };
    use crate::capture::{CapturedFrame, OneFrameCapture};
    use crate::log::Logger;
    use crate::overlay::OverlayManager;
    use crate::render::{pixel_format, read_bitmap_bgra, GraphicsDevice};
    use edutools_core::board::{BoardKind, BoardPagePosition};
    use edutools_core::settings::AppSettings;
    use edutools_core::state::AppState;
    use std::cell::RefCell;
    use std::fs;
    use std::mem::size_of;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::Duration;
    use windows::core::{w, Error, Interface, Result, BOOL, PCWSTR, PWSTR};
    use windows::Graphics::SizeInt32;
    use windows::Win32::Foundation::{E_FAIL, HANDLE, HGLOBAL, HWND, LPARAM, SYSTEMTIME};
    use windows::Win32::Graphics::Direct2D::{D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1};
    use windows::Win32::Graphics::Dwm::DwmFlush;
    use windows::Win32::Graphics::Dxgi::IDXGISurface;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_ContainerFormatJpeg, GUID_ContainerFormatPng,
        GUID_WICPixelFormat24bppBGR, GUID_WICPixelFormat32bppPBGRA, IWICBitmapFrameEncode,
        IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapEncoderNoCache,
        WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
    };
    use windows::Win32::System::Com::StructuredStorage::{
        CreateStreamOnHGlobal, GetHGlobalFromStream, PROPBAG2,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::SystemInformation::GetLocalTime;
    use windows::Win32::System::Variant::{VARIANT, VT_R4};
    use windows::Win32::UI::Shell::{FOLDERID_Pictures, SHCreateMemStream, SHGetKnownFolderPath};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowDisplayAffinity, GetWindowThreadProcessId, IsWindowVisible,
        KillTimer, MessageBoxW, SetTimer, SetWindowDisplayAffinity, MB_ICONWARNING, MB_OK,
        WDA_EXCLUDEFROMCAPTURE, WINDOW_DISPLAY_AFFINITY,
    };

    pub const EXPORT_CAPTURE_TIMEOUT_TIMER_ID: usize = 0xED70;
    const EXPORT_CAPTURE_TIMEOUT_MS: u32 = 5_000;
    const CF_DIBV5_FORMAT: u32 = 17;

    pub struct ExportController {
        owner: HWND,
        logger: Logger,
        pending: Option<PendingCapture>,
        fallback_notice: Option<(String, String)>,
    }

    struct PendingCapture {
        destination: ExportDestination,
        capture: OneFrameCapture,
        exclusion: CaptureExclusion,
        ink: BgraImage,
        graphics: GraphicsDevice,
    }

    enum ExportDestination {
        Png(PathBuf),
        Clipboard,
    }

    impl ExportDestination {
        const fn label(&self) -> &'static str {
            match self {
                Self::Png(_) => "PNG",
                Self::Clipboard => "clipboard",
            }
        }

        fn file_name(&self) -> String {
            match self {
                Self::Png(path) => path
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "-".to_owned()),
                Self::Clipboard => "-".to_owned(),
            }
        }
    }

    impl ExportController {
        pub fn new(owner: HWND, logger: Logger) -> Self {
            Self {
                owner,
                logger,
                pending: None,
                fallback_notice: None,
            }
        }

        pub fn take_fallback_notice(&mut self) -> Option<(String, String)> {
            self.fallback_notice.take()
        }

        pub fn save_current_content(
            &mut self,
            overlays: &mut OverlayManager,
            state: &Rc<RefCell<AppState>>,
            settings: &AppSettings,
        ) {
            if self.reject_if_busy() {
                return;
            }
            let Some(target) = self.target_display(overlays, state) else {
                self.report_failure(
                    "저장할 화면을 찾지 못했습니다",
                    "연결된 화면을 확인해 주세요.",
                );
                return;
            };
            let board_kind = state.borrow().board_kind_on(&target);
            if let Some(kind) = board_kind {
                self.save_board_pdf(overlays, state, settings, &target, kind);
            } else {
                let destination = match self.png_destination(settings, &target) {
                    Ok(destination) => ExportDestination::Png(destination),
                    Err(error) => {
                        self.report_failure("PNG 저장 위치를 만들지 못했습니다", &error);
                        return;
                    }
                };
                self.begin_screen_capture(overlays, &target, destination);
            }
        }

        pub fn copy_current_image(
            &mut self,
            overlays: &mut OverlayManager,
            state: &Rc<RefCell<AppState>>,
            settings: &AppSettings,
        ) {
            if self.reject_if_busy() {
                return;
            }
            let Some(target) = self.target_display(overlays, state) else {
                self.report_failure(
                    "복사할 화면을 찾지 못했습니다",
                    "연결된 화면을 확인해 주세요.",
                );
                return;
            };
            let board_kind = state.borrow().board_kind_on(&target);
            if let Some(kind) = board_kind {
                if let Err(error) = overlays.finish_export_interaction(&target) {
                    self.report_failure("보드 이미지를 만들지 못했습니다", &error.to_string());
                    return;
                }
                let (strokes, position) = {
                    let state = state.borrow();
                    let pages = state.board_pages(kind);
                    let position =
                        state
                            .board_page_position(&target)
                            .unwrap_or(BoardPagePosition {
                                current: 1,
                                total: pages.len(),
                            });
                    let strokes = pages
                        .get(position.current.saturating_sub(1))
                        .cloned()
                        .unwrap_or_default();
                    (strokes, position)
                };
                match overlays.export_board_page_bitmap(&target, &strokes, kind, position, settings)
                {
                    Ok(image) => {
                        self.log_start("clipboard", &image, "-");
                        self.complete(ExportDestination::Clipboard, image, false);
                    }
                    Err(error) => {
                        self.report_failure("보드 이미지를 만들지 못했습니다", &error.to_string())
                    }
                }
            } else {
                self.begin_screen_capture(overlays, &target, ExportDestination::Clipboard);
            }
        }

        pub fn handle_capture_frame(&mut self) {
            let Some(mut pending) = self.take_pending() else {
                self.logger
                    .event("내보내기 프레임 메시지 무시: 대기 작업 없음");
                return;
            };
            let frame = pending.capture.take_frame();
            pending.exclusion.restore();
            let Some(frame) = frame else {
                self.use_ink_only_fallback(
                    pending.destination,
                    pending.ink,
                    "WGC가 단일 프레임을 전달하지 않았습니다.",
                );
                return;
            };
            let background = match captured_frame_image(&frame, &pending.graphics) {
                Ok(image) => image,
                Err(error) => {
                    self.use_ink_only_fallback(
                        pending.destination,
                        pending.ink,
                        &format!("캡처 프레임을 읽지 못했습니다: {error}"),
                    );
                    return;
                }
            };
            match composite_premultiplied(&background, &pending.ink) {
                Ok(composite) => {
                    self.complete(pending.destination, composite, false);
                }
                Err(error) => self.report_failure("화면과 판서를 합치지 못했습니다", &error),
            }
        }

        pub fn handle_capture_recreate(&mut self) {
            let result = self
                .pending
                .as_ref()
                .map(|pending| pending.capture.recreate_if_needed());
            match result {
                Some(Ok(true)) => self.logger.event("내보내기 WGC 프레임 풀 크기 재생성 완료"),
                Some(Ok(false)) | None => {}
                Some(Err(error)) => {
                    let Some(mut pending) = self.take_pending() else {
                        return;
                    };
                    pending.exclusion.restore();
                    self.use_ink_only_fallback(
                        pending.destination,
                        pending.ink,
                        &format!("Direct3D11CaptureFramePool::Recreate 실패: {error}"),
                    );
                }
            }
        }

        pub fn handle_capture_failure(&mut self) {
            let Some(mut pending) = self.take_pending() else {
                return;
            };
            let reason = pending
                .capture
                .take_failure()
                .unwrap_or_else(|| "Windows Graphics Capture가 실패했습니다.".to_owned());
            pending.exclusion.restore();
            self.use_ink_only_fallback(pending.destination, pending.ink, &reason);
        }

        pub fn handle_capture_timeout(&mut self) {
            let Some(mut pending) = self.take_pending() else {
                return;
            };
            pending.exclusion.restore();
            self.use_ink_only_fallback(
                pending.destination,
                pending.ink,
                "화면 캡처가 제한 시간 안에 응답하지 않았습니다.",
            );
        }

        pub fn shutdown(&mut self) {
            if let Some(mut pending) = self.take_pending() {
                pending.exclusion.restore();
            }
        }

        fn begin_screen_capture(
            &mut self,
            overlays: &mut OverlayManager,
            target: &str,
            destination: ExportDestination,
        ) {
            let ink = match overlays.export_ink_bitmap(target) {
                Ok(image) => image,
                Err(error) => {
                    self.report_failure("판서 이미지를 만들지 못했습니다", &error.to_string());
                    return;
                }
            };
            let Some(monitor) = overlays.monitor(target) else {
                self.report_failure(
                    "화면 이미지를 만들지 못했습니다",
                    "대상 화면 정보를 찾지 못했습니다.",
                );
                return;
            };
            self.log_start(destination.label(), &ink, &destination.file_name());
            let graphics = overlays.graphics_device();
            let exclusion = match CaptureExclusion::begin(self.logger.clone()) {
                Ok(exclusion) => exclusion,
                Err(error) => {
                    self.use_ink_only_fallback(destination, ink, &error);
                    return;
                }
            };
            let size = SizeInt32 {
                Width: monitor.rect.width(),
                Height: monitor.rect.height(),
            };
            match OneFrameCapture::start(
                self.owner,
                monitor.handle,
                size,
                &graphics,
                self.logger.clone(),
            ) {
                Ok(capture) => {
                    let timer = unsafe {
                        SetTimer(
                            Some(self.owner),
                            EXPORT_CAPTURE_TIMEOUT_TIMER_ID,
                            EXPORT_CAPTURE_TIMEOUT_MS,
                            None,
                        )
                    };
                    if timer == 0 {
                        let mut exclusion = exclusion;
                        exclusion.restore();
                        self.use_ink_only_fallback(
                            destination,
                            ink,
                            "화면 캡처 제한 시간 타이머를 시작하지 못했습니다.",
                        );
                        return;
                    }
                    self.pending = Some(PendingCapture {
                        destination,
                        capture,
                        exclusion,
                        ink,
                        graphics,
                    });
                }
                Err(error) => {
                    drop(exclusion);
                    self.use_ink_only_fallback(destination, ink, &error.to_string());
                }
            }
        }

        fn save_board_pdf(
            &mut self,
            overlays: &mut OverlayManager,
            state: &Rc<RefCell<AppState>>,
            settings: &AppSettings,
            target: &str,
            kind: BoardKind,
        ) {
            if let Err(error) = overlays.finish_export_interaction(target) {
                self.report_failure("보드 PDF를 만들지 못했습니다", &error.to_string());
                return;
            }
            let pages = state.borrow().board_pages(kind);
            let Some(monitor) = overlays.monitor(target) else {
                self.report_failure(
                    "보드 PDF를 만들지 못했습니다",
                    "대상 화면 정보를 찾지 못했습니다.",
                );
                return;
            };
            let path = match self.pdf_destination(settings, target) {
                Ok(path) => path,
                Err(error) => {
                    self.report_failure("보드 PDF 저장 위치를 만들지 못했습니다", &error);
                    return;
                }
            };
            let file_name = path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| "-".to_owned());
            self.logger.event(&format!(
                "내보내기 시작: kind=PDF pixels={}x{} file={file_name}",
                monitor.rect.width(),
                monitor.rect.height()
            ));

            let mut jpeg_pages = Vec::with_capacity(pages.len());
            for (index, strokes) in pages.iter().enumerate() {
                let position = BoardPagePosition {
                    current: index + 1,
                    total: pages.len(),
                };
                let image = match overlays
                    .export_board_page_bitmap(target, strokes, kind, position, settings)
                {
                    Ok(image) => image,
                    Err(error) => {
                        self.report_failure("보드 PDF를 만들지 못했습니다", &error.to_string());
                        return;
                    }
                };
                match encode_jpeg(&image, 0.9) {
                    Ok(jpeg) => jpeg_pages.push(jpeg),
                    Err(error) => {
                        self.report_failure("보드 PDF를 만들지 못했습니다", &error.to_string());
                        return;
                    }
                }
            }
            let pdf_pages = jpeg_pages
                .iter()
                .map(|jpeg| PdfPage {
                    jpeg,
                    pixel_width: monitor.rect.width() as u32,
                    pixel_height: monitor.rect.height() as u32,
                })
                .collect::<Vec<_>>();
            let page_width = f64::from(monitor.rect.width()) * 96.0 / f64::from(monitor.dpi.0);
            let page_height = f64::from(monitor.rect.height()) * 96.0 / f64::from(monitor.dpi.1);
            let pdf = match build_pdf(&pdf_pages, page_width, page_height) {
                Ok(pdf) => pdf,
                Err(error) => {
                    self.report_failure("보드 PDF를 만들지 못했습니다", &error);
                    return;
                }
            };
            if let Err(error) = atomic_write(&path, &pdf) {
                self.report_failure("보드 PDF를 저장하지 못했습니다", &error);
                return;
            }
            self.logger.event(&format!(
                "내보내기 완료: kind=PDF pixels={}x{} file={file_name}",
                monitor.rect.width(),
                monitor.rect.height()
            ));
        }

        fn complete(
            &self,
            destination: ExportDestination,
            image: BgraImage,
            ink_only: bool,
        ) -> bool {
            let file_name = destination.file_name();
            match destination {
                ExportDestination::Png(path) => match encode_png(&image)
                    .map_err(|error| error.to_string())
                    .and_then(|data| atomic_write(&path, &data))
                {
                    Ok(()) => {
                        self.log_complete("PNG", &image, &file_name, ink_only);
                        true
                    }
                    Err(error) => {
                        self.report_failure("PNG를 저장하지 못했습니다", &error);
                        false
                    }
                },
                ExportDestination::Clipboard => match encode_png(&image)
                    .and_then(|png| set_clipboard(self.owner, &image, &png))
                {
                    Ok(retries) => {
                        self.logger
                            .event(&format!("클립보드 열기 재시도: count={retries}"));
                        self.log_complete("clipboard", &image, "-", ink_only);
                        true
                    }
                    Err(error) => {
                        self.report_failure(
                            "클립보드에 복사하지 못했습니다",
                            &format!("클립보드가 사용 중인지 확인해 주세요.\n{error}"),
                        );
                        false
                    }
                },
            }
        }

        fn use_ink_only_fallback(
            &mut self,
            destination: ExportDestination,
            ink: BgraImage,
            reason: &str,
        ) {
            let reason = reason.lines().collect::<Vec<_>>().join(" ");
            self.logger
                .event(&format!("내보내기 배경 캡처 실패: {reason}"));
            self.logger
                .event(&format!("내보내기 폴백: kind=ink-only reason={reason}"));
            if self.complete(destination, ink, true) {
                self.fallback_notice = Some((
                    "배경 없이 판서만 내보냈습니다".to_owned(),
                    format!("화면 캡처 권한과 상태를 확인해 주세요.\n{reason}"),
                ));
            }
        }

        fn target_display(
            &self,
            overlays: &OverlayManager,
            state: &Rc<RefCell<AppState>>,
        ) -> Option<String> {
            overlays
                .monitor_key_at_cursor()
                .ok()
                .flatten()
                .or_else(|| state.borrow().active_display_id().map(str::to_owned))
                .filter(|target| overlays.contains_monitor(target))
        }

        fn png_destination(
            &self,
            settings: &AppSettings,
            target: &str,
        ) -> std::result::Result<PathBuf, String> {
            export_destination(settings, target, "png")
        }

        fn pdf_destination(
            &self,
            settings: &AppSettings,
            target: &str,
        ) -> std::result::Result<PathBuf, String> {
            export_destination(settings, target, "pdf")
        }

        fn take_pending(&mut self) -> Option<PendingCapture> {
            let pending = self.pending.take();
            if pending.is_some() {
                let _ = unsafe { KillTimer(Some(self.owner), EXPORT_CAPTURE_TIMEOUT_TIMER_ID) };
            }
            pending
        }

        fn reject_if_busy(&self) -> bool {
            if self.pending.is_some() {
                self.report_failure(
                    "내보내기가 이미 진행 중입니다",
                    "현재 화면 캡처가 끝난 뒤 다시 시도해 주세요.",
                );
                true
            } else {
                false
            }
        }

        fn log_start(&self, kind: &str, image: &BgraImage, file_name: &str) {
            self.logger.event(&format!(
                "내보내기 시작: kind={kind} pixels={}x{} file={file_name}",
                image.width, image.height
            ));
        }

        fn log_complete(&self, kind: &str, image: &BgraImage, file_name: &str, ink_only: bool) {
            let mode = if ink_only { " (ink-only)" } else { "" };
            self.logger.event(&format!(
                "내보내기 완료: kind={kind}{mode} pixels={}x{} file={file_name}",
                image.width, image.height
            ));
        }

        fn report_failure(&self, title: &str, message: &str) {
            self.logger.event(&format!("내보내기 실패: {title}"));
            show_warning(self.owner, title, message);
        }
    }

    struct CaptureExclusion {
        windows: Vec<(HWND, WINDOW_DISPLAY_AFFINITY)>,
        logger: Logger,
    }

    struct ExclusionEnumeration {
        process_id: u32,
        changed: Vec<(HWND, WINDOW_DISPLAY_AFFINITY)>,
        failure: Option<String>,
    }

    impl CaptureExclusion {
        fn begin(logger: Logger) -> std::result::Result<Self, String> {
            let mut enumeration = ExclusionEnumeration {
                process_id: std::process::id(),
                changed: Vec::new(),
                failure: None,
            };
            let result = unsafe {
                EnumWindows(
                    Some(exclude_window_for_capture),
                    LPARAM((&mut enumeration as *mut ExclusionEnumeration) as isize),
                )
            };
            if let Err(error) = result {
                enumeration
                    .failure
                    .get_or_insert_with(|| format!("EnumWindows 실패: {error}"));
            }
            if let Some(error) = enumeration.failure {
                restore_window_affinities(&mut enumeration.changed, &logger);
                return Err(format!(
                    "EduTools 창을 배경 캡처에서 제외하지 못했습니다: {error}"
                ));
            }
            if let Err(error) = unsafe { DwmFlush() } {
                restore_window_affinities(&mut enumeration.changed, &logger);
                return Err(format!(
                    "DwmFlush 실패: EduTools 창 제외 상태를 화면 합성기에 반영하지 못했습니다: {error}"
                ));
            }
            logger.event(&format!(
                "내보내기 캡처 제외 시작: windows={}",
                enumeration.changed.len()
            ));
            Ok(Self {
                windows: enumeration.changed,
                logger,
            })
        }

        fn restore(&mut self) {
            restore_window_affinities(&mut self.windows, &self.logger);
        }
    }

    impl Drop for CaptureExclusion {
        fn drop(&mut self) {
            self.restore();
        }
    }

    unsafe extern "system" fn exclude_window_for_capture(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let enumeration = unsafe { &mut *(lparam.0 as *mut ExclusionEnumeration) };
        let mut process_id = 0_u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        if process_id != enumeration.process_id || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let mut previous = 0_u32;
        if let Err(error) = unsafe { GetWindowDisplayAffinity(hwnd, &mut previous) } {
            enumeration.failure = Some(format!(
                "GetWindowDisplayAffinity(hwnd={:#x}) 실패: {error}",
                hwnd.0 as usize
            ));
            return BOOL(1);
        }
        let previous = WINDOW_DISPLAY_AFFINITY(previous);
        if previous == WDA_EXCLUDEFROMCAPTURE {
            return BOOL(1);
        }
        match unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) } {
            Ok(()) => enumeration.changed.push((hwnd, previous)),
            Err(error) => {
                enumeration.failure = Some(format!(
                    "SetWindowDisplayAffinity(hwnd={:#x}, WDA_EXCLUDEFROMCAPTURE) 실패: {error}",
                    hwnd.0 as usize
                ))
            }
        }
        BOOL(1)
    }

    fn restore_window_affinities(
        windows: &mut Vec<(HWND, WINDOW_DISPLAY_AFFINITY)>,
        logger: &Logger,
    ) {
        let mut failures = 0;
        for (hwnd, affinity) in windows.drain(..).rev() {
            if unsafe { SetWindowDisplayAffinity(hwnd, affinity) }.is_err() {
                failures += 1;
            }
        }
        let _ = unsafe { DwmFlush() };
        if failures == 0 {
            logger.event("내보내기 캡처 제외 복원 완료");
        } else {
            logger.event(&format!("내보내기 캡처 제외 복원 실패: windows={failures}"));
        }
    }

    fn captured_frame_image(frame: &CapturedFrame, graphics: &GraphicsDevice) -> Result<BgraImage> {
        let context = graphics
            .create_d2d_context()
            .map_err(|error| call_error("GraphicsDevice::create_d2d_context", error))?;
        let surface: IDXGISurface = frame
            .texture
            .cast()
            .map_err(|error| call_error("ID3D11Texture2D::cast<IDXGISurface>", error))?;
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: pixel_format(),
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };
        let bitmap = unsafe { context.CreateBitmapFromDxgiSurface(&surface, Some(&properties)) }
            .map_err(|error| {
                call_error("ID2D1DeviceContext::CreateBitmapFromDxgiSurface", error)
            })?;
        let size = unsafe { bitmap.GetPixelSize() };
        if size.width != frame.size.Width as u32 || size.height != frame.size.Height as u32 {
            return Err(Error::new(
                E_FAIL,
                "WGC texture and frame dimensions differ",
            ));
        }
        let (pixels, pitch) = read_bitmap_bgra(&context, &bitmap)
            .map_err(|error| call_error("read_bitmap_bgra", error))?;
        BgraImage::from_pitched(size.width, size.height, pitch, &pixels)
            .map_err(|error| Error::new(E_FAIL, error))
    }

    fn call_error(operation: &str, error: Error) -> Error {
        Error::new(error.code(), format!("{operation} 실패: {error}"))
    }

    fn export_destination(
        settings: &AppSettings,
        target: &str,
        extension: &str,
    ) -> std::result::Result<PathBuf, String> {
        let directory = export_directory(settings)?;
        fs::create_dir_all(&directory)
            .map_err(|error| format!("내보내기 폴더를 만들지 못했습니다: {error}"))?;
        let timestamp = local_timestamp();
        unique_export_path(&directory, &export_base_name(timestamp, target), extension)
    }

    fn export_directory(settings: &AppSettings) -> std::result::Result<PathBuf, String> {
        if let Some(path) = settings
            .general
            .archive_folder_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
        {
            return Ok(PathBuf::from(path));
        }
        let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Pictures, Default::default(), None) }
            .map_err(|error| format!("사진 폴더를 찾지 못했습니다: {error}"))?;
        let text = unsafe { raw.to_string() }
            .map_err(|error| format!("사진 폴더 경로를 읽지 못했습니다: {error}"));
        unsafe { CoTaskMemFree(Some(raw.0.cast())) };
        text.map(|path| PathBuf::from(path).join("EduTools"))
    }

    fn local_timestamp() -> ExportTimestamp {
        let SYSTEMTIME {
            wYear,
            wMonth,
            wDay,
            wHour,
            wMinute,
            wSecond,
            ..
        } = unsafe { GetLocalTime() };
        ExportTimestamp {
            year: wYear,
            month: wMonth,
            day: wDay,
            hour: wHour,
            minute: wMinute,
            second: wSecond,
        }
    }

    fn atomic_write(path: &Path, data: &[u8]) -> std::result::Result<(), String> {
        crate::settings_file::FileSettingsStorage::atomic_write(path, data)
            .map_err(|error| error.to_string())
    }

    fn show_warning(owner: HWND, title: &str, message: &str) {
        let title = wide(title);
        let message = wide(message);
        let _ = unsafe {
            MessageBoxW(
                Some(owner),
                PCWSTR(message.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONWARNING,
            )
        };
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct ComApartment;

    impl ComApartment {
        fn initialize() -> Result<Self> {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
            Ok(Self)
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    fn encode_png(image: &BgraImage) -> Result<Vec<u8>> {
        encode_wic(image, false, 1.0)
    }

    fn encode_jpeg(image: &BgraImage, quality: f32) -> Result<Vec<u8>> {
        encode_wic(image, true, quality)
    }

    fn encode_wic(image: &BgraImage, jpeg: bool, quality: f32) -> Result<Vec<u8>> {
        use windows::Win32::System::Com::{STATFLAG_NONAME, STATSTG};

        let _apartment = ComApartment::initialize()?;
        let factory: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }?;
        let bitmap = unsafe {
            factory.CreateBitmapFromMemory(
                image.width,
                image.height,
                &GUID_WICPixelFormat32bppPBGRA,
                image.stride(),
                &image.pixels,
            )
        }?;
        let stream = unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true) }?;
        let container = if jpeg {
            &GUID_ContainerFormatJpeg
        } else {
            &GUID_ContainerFormatPng
        };
        let encoder = unsafe { factory.CreateEncoder(container, std::ptr::null()) }?;
        unsafe { encoder.Initialize(&stream, WICBitmapEncoderNoCache) }?;
        let mut frame: Option<IWICBitmapFrameEncode> = None;
        let mut options = None;
        unsafe { encoder.CreateNewFrame(&mut frame, &mut options) }?;
        let frame = frame.ok_or_else(Error::from_thread)?;
        let mut quality_name = wide("ImageQuality");
        if jpeg {
            let options = options
                .as_ref()
                .ok_or_else(|| Error::new(E_FAIL, "JPEG encoder options are unavailable"))?;
            let property = PROPBAG2 {
                vt: VT_R4,
                pstrName: PWSTR(quality_name.as_mut_ptr()),
                ..Default::default()
            };
            let value = VARIANT::from(quality);
            unsafe { options.Write(1, &property, &value) }?;
        }
        unsafe {
            frame.Initialize(options.as_ref())?;
            frame.SetSize(image.width, image.height)?;
            frame.SetResolution(96.0, 96.0)?;
        }
        let mut format = if jpeg {
            GUID_WICPixelFormat24bppBGR
        } else {
            GUID_WICPixelFormat32bppPBGRA
        };
        unsafe {
            frame.SetPixelFormat(&mut format)?;
            frame.WriteSource(&bitmap, std::ptr::null())?;
            frame.Commit()?;
            encoder.Commit()?;
        }

        let mut stat = STATSTG::default();
        unsafe { stream.Stat(&mut stat, STATFLAG_NONAME) }?;
        let byte_count = usize::try_from(stat.cbSize)
            .map_err(|_| Error::new(E_FAIL, "WIC stream is too large"))?;
        let global = unsafe { GetHGlobalFromStream(&stream) }?;
        if unsafe { GlobalSize(global) } < byte_count {
            return Err(Error::new(E_FAIL, "WIC stream storage is truncated"));
        }
        let pointer = unsafe { GlobalLock(global) };
        if pointer.is_null() {
            return Err(Error::from_thread());
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), byte_count) }.to_vec();
        let _ = unsafe { GlobalUnlock(global) };
        Ok(bytes)
    }

    fn decode_png(data: &[u8]) -> Result<BgraImage> {
        let _apartment = ComApartment::initialize()?;
        let factory: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }?;
        let stream = unsafe { SHCreateMemStream(Some(data)) }
            .ok_or_else(|| Error::new(E_FAIL, "PNG memory stream is unavailable"))?;
        let decoder = unsafe {
            factory.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)
        }?;
        let frame = unsafe { decoder.GetFrame(0) }?;
        let mut width = 0;
        let mut height = 0;
        unsafe { frame.GetSize(&mut width, &mut height) }?;
        let converter = unsafe { factory.CreateFormatConverter() }?;
        unsafe {
            converter.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None::<&windows::Win32::Graphics::Imaging::IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
        }?;
        let mut pixels = vec![0_u8; width as usize * height as usize * 4];
        unsafe { converter.CopyPixels(std::ptr::null(), width * 4, &mut pixels) }?;
        BgraImage::from_tight(width, height, pixels).map_err(|error| Error::new(E_FAIL, error))
    }

    struct GlobalMemory {
        handle: HGLOBAL,
        owned: bool,
    }

    impl GlobalMemory {
        fn from_bytes(bytes: &[u8]) -> Result<Self> {
            let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }?;
            let pointer = unsafe { GlobalLock(handle) };
            if pointer.is_null() {
                let _ = unsafe { windows::Win32::Foundation::GlobalFree(Some(handle)) };
                return Err(Error::from_thread());
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len())
            };
            let _ = unsafe { GlobalUnlock(handle) };
            Ok(Self {
                handle,
                owned: true,
            })
        }

        fn transfer(&mut self, format: u32) -> Result<()> {
            unsafe { SetClipboardData(format, Some(HANDLE(self.handle.0))) }?;
            self.owned = false;
            Ok(())
        }
    }

    impl Drop for GlobalMemory {
        fn drop(&mut self) {
            if self.owned {
                let _ = unsafe { windows::Win32::Foundation::GlobalFree(Some(self.handle)) };
            }
        }
    }

    struct ClipboardGuard;

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            let _ = unsafe { CloseClipboard() };
        }
    }

    fn set_clipboard(owner: HWND, image: &BgraImage, png: &[u8]) -> Result<usize> {
        let dib = dibv5_bytes(image).map_err(|error| Error::new(E_FAIL, error))?;
        let mut dib_memory = GlobalMemory::from_bytes(&dib)?;
        let mut png_memory = GlobalMemory::from_bytes(png)?;
        let png_format = unsafe { RegisterClipboardFormatW(w!("PNG")) };
        if png_format == 0 {
            return Err(Error::from_thread());
        }

        let mut retries = 0;
        loop {
            match unsafe { OpenClipboard(Some(owner)) } {
                Ok(()) => break,
                Err(error) if retries < 2 => {
                    retries += 1;
                    std::thread::sleep(Duration::from_millis(40));
                    let _ = error;
                }
                Err(_) => return Err(Error::new(E_FAIL, "클립보드가 사용 중입니다")),
            }
        }
        let _guard = ClipboardGuard;
        unsafe { EmptyClipboard() }?;
        dib_memory.transfer(CF_DIBV5_FORMAT)?;
        png_memory.transfer(png_format)?;
        Ok(retries)
    }

    pub fn selftest_failures() -> Vec<String> {
        let mut failures = super::pure_selftest_failures();
        let image = BgraImage::from_tight(
            2,
            2,
            vec![
                0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255,
            ],
        )
        .expect("literal selftest bitmap");
        match encode_png(&image).and_then(|png| decode_png(&png)) {
            Ok(decoded) if decoded == image => {}
            Ok(decoded) => failures.push(format!(
                "export PNG: encode/decode pixels differ ({:?} != {:?})",
                decoded.pixels, image.pixels
            )),
            Err(error) => failures.push(format!("export PNG: encode/decode failed: {error}")),
        }
        match dibv5_bytes(&image) {
            Ok(dib)
                if dib.len() == size_of::<windows::Win32::Graphics::Gdi::BITMAPV5HEADER>() + 16 => {
            }
            Ok(dib) => failures.push(format!("export DIBV5: unexpected byte count {}", dib.len())),
            Err(error) => failures.push(format!("export DIBV5: {error}")),
        }
        failures
    }
}

#[cfg(windows)]
pub use windows_impl::{selftest_failures, ExportController, EXPORT_CAPTURE_TIMEOUT_TIMER_ID};

#[cfg(test)]
mod tests {
    use super::{
        build_pdf, dibv5_bytes, export_base_name, unique_file_name, validate_pdf_structure,
        BgraImage, ExportTimestamp, PdfPage, DIBV5_HEADER_SIZE,
    };

    #[test]
    fn pdf_writer_structure_and_xref_table() {
        let pages = [
            PdfPage {
                jpeg: &[0xff, 0xd8, 1, 2, 0xff, 0xd9],
                pixel_width: 640,
                pixel_height: 480,
            },
            PdfPage {
                jpeg: &[0xff, 0xd8, 3, 4, 0xff, 0xd9],
                pixel_width: 640,
                pixel_height: 480,
            },
        ];
        let pdf = build_pdf(&pages, 320.0, 240.0).expect("two-page PDF");
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(8).any(|value| value == b"/Count 2"));
        validate_pdf_structure(&pdf, 2).expect("xref offsets");
        assert!(pdf.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn export_file_name_and_collision_suffix_table() {
        let timestamp = ExportTimestamp {
            year: 2026,
            month: 10,
            day: 31,
            hour: 9,
            minute: 8,
            second: 7,
        };
        let base = export_base_name(timestamp, r"\\.\DISPLAY1");
        assert_eq!(base, "20261031-090807-DISPLAY1");
        let occupied = [
            "20261031-090807-DISPLAY1.png",
            "20261031-090807-DISPLAY1-2.png",
        ];
        assert_eq!(
            unique_file_name(&base, "png", |name| occupied.contains(&name)).unwrap(),
            "20261031-090807-DISPLAY1-3.png"
        );
    }

    #[test]
    fn dibv5_header_masks_and_bottom_up_rows_table() {
        let image = BgraImage::from_tight(
            2,
            2,
            vec![
                1, 2, 3, 255, 4, 5, 6, 255, // top
                7, 8, 9, 255, 10, 11, 12, 255, // bottom
            ],
        )
        .unwrap();
        let dib = dibv5_bytes(&image).unwrap();
        assert_eq!(u32::from_le_bytes(dib[0..4].try_into().unwrap()), 124);
        assert_eq!(i32::from_le_bytes(dib[8..12].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(dib[40..44].try_into().unwrap()),
            0x00ff_0000
        );
        assert_eq!(
            u32::from_le_bytes(dib[52..56].try_into().unwrap()),
            0xff00_0000
        );
        assert_eq!(
            &dib[DIBV5_HEADER_SIZE..],
            &[
                7, 8, 9, 255, 10, 11, 12, 255, // bottom first
                1, 2, 3, 255, 4, 5, 6, 255, // top second
            ]
        );
    }
}
