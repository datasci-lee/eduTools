use crate::log::Logger;
use std::ffi::c_void;
use std::mem::size_of;
use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HINSTANCE, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_WARNING, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICON_VERSION_4, NOTIFY_ICON_DATA_FLAGS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos, LoadImageW, MessageBoxW,
    PostMessageW, SetForegroundWindow, TrackPopupMenu, GDI_IMAGE_TYPE, HICON, IMAGE_ICON,
    LR_DEFAULTCOLOR, MB_ICONWARNING, MB_OK, MENU_ITEM_FLAGS, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    SM_CXSMICON, TPM_RETURNCMD, TPM_RIGHTBUTTON, TRACK_POPUP_MENU_FLAGS, WM_NULL,
};

pub const CALLBACK_MESSAGE: u32 = 0x8000 + 1;
pub const MENU_TOGGLE_ANNOTATION: u16 = 1;
pub const MENU_EXIT: u16 = 2;
pub const ALL_ICON_RESOURCE_IDS: [u16; 5] = [1, 101, 102, 103, 104];

const TRAY_ID: u32 = 1;
const ICON_OFF_LIGHT: u16 = 101;
const ICON_ON_LIGHT: u16 = 102;
const ICON_OFF_DARK: u16 = 103;
const ICON_ON_DARK: u16 = 104;

pub struct Tray {
    hwnd: HWND,
    hinstance: HINSTANCE,
    icon: Option<HICON>,
    registered: bool,
    light_taskbar: bool,
    annotation_enabled: bool,
    dpi: u32,
    size: i32,
    logger: Logger,
}

impl Tray {
    pub fn add(hwnd: HWND, hinstance: HINSTANCE, logger: Logger) -> Result<Self> {
        let light_taskbar = read_light_taskbar(&logger).unwrap_or(true);
        let mut tray = Self {
            hwnd,
            hinstance,
            icon: None,
            registered: false,
            light_taskbar,
            annotation_enabled: false,
            dpi: 0,
            size: 0,
            logger,
        };
        tray.replace_icon("초기화", false)?;
        tray.add_to_shell(false)?;
        Ok(tray)
    }

    pub fn set_annotation_enabled(&mut self, enabled: bool) {
        self.annotation_enabled = enabled;
        if let Err(error) = self.replace_icon("판서 상태", true) {
            self.logger
                .event(&format!("트레이 상태 아이콘 교체 실패: {error}"));
        }
    }

    pub fn refresh_theme(&mut self) {
        let Some(light_taskbar) = read_light_taskbar(&self.logger) else {
            return;
        };
        if light_taskbar == self.light_taskbar {
            return;
        }
        self.light_taskbar = light_taskbar;
        if let Err(error) = self.replace_icon("작업 표시줄 테마", true) {
            self.logger
                .event(&format!("트레이 테마 아이콘 교체 실패: {error}"));
        }
    }

    pub fn refresh_display(&mut self, reason: &str) {
        if let Err(error) = self.replace_icon(reason, true) {
            self.logger
                .event(&format!("트레이 DPI 아이콘 교체 실패: {error}"));
        }
    }

    pub fn readd_after_taskbar_created(&mut self) {
        self.registered = false;
        if let Err(error) = self.add_to_shell(true) {
            self.logger
                .event(&format!("트레이 아이콘 재등록 실패: {error}"));
        }
    }

    pub fn show_menu(&self) -> Option<u16> {
        // SAFETY: CreatePopupMenu creates an owned menu handle checked below.
        let menu = match unsafe { CreatePopupMenu() } {
            Ok(menu) => menu,
            Err(error) => {
                self.logger
                    .event(&format!("트레이 메뉴 생성 실패: {error}"));
                return None;
            }
        };
        let checked = if self.annotation_enabled {
            MF_CHECKED.0
        } else {
            0
        };
        // SAFETY: menu is valid and all string pointers are static NUL-terminated values.
        let populated = unsafe {
            AppendMenuW(
                menu,
                MENU_ITEM_FLAGS(MF_STRING.0 | checked),
                usize::from(MENU_TOGGLE_ANNOTATION),
                w!("판서 모드"),
            )
            .and_then(|_| AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()))
            .and_then(|_| AppendMenuW(menu, MF_STRING, usize::from(MENU_EXIT), w!("EduTools 종료")))
        };
        if let Err(error) = populated {
            self.logger
                .event(&format!("트레이 메뉴 항목 생성 실패: {error}"));
            // SAFETY: menu is owned by this function and no longer in use.
            let _ = unsafe { DestroyMenu(menu) };
            return None;
        }

        let mut point = POINT::default();
        // SAFETY: point is valid writable memory.
        if let Err(error) = unsafe { GetCursorPos(&mut point) } {
            self.logger
                .event(&format!("트레이 메뉴 위치 조회 실패: {error}"));
            // SAFETY: menu is owned by this function and no longer in use.
            let _ = unsafe { DestroyMenu(menu) };
            return None;
        }
        // SAFETY: hwnd is the hidden owner window; menu and coordinates are valid.
        let selected = unsafe {
            let _ = SetForegroundWindow(self.hwnd);
            TrackPopupMenu(
                menu,
                TRACK_POPUP_MENU_FLAGS(TPM_RIGHTBUTTON.0 | TPM_RETURNCMD.0),
                point.x,
                point.y,
                None,
                self.hwnd,
                None,
            )
        };
        // SAFETY: hwnd remains valid and WM_NULL carries no pointer payload.
        if let Err(error) = unsafe { PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0)) }
        {
            self.logger
                .event(&format!("트레이 메뉴 WM_NULL 전송 실패: {error}"));
        }
        // SAFETY: menu is owned by this function and TrackPopupMenu has returned.
        if let Err(error) = unsafe { DestroyMenu(menu) } {
            self.logger
                .event(&format!("트레이 메뉴 해제 실패: {error}"));
        }
        u16::try_from(selected.0).ok().filter(|value| *value != 0)
    }

    pub fn show_hotkey_failure(&self, name: &str, key: &str) {
        let mut data = self.base_data(NOTIFY_ICON_DATA_FLAGS(NIF_INFO.0));
        copy_wide(&mut data.szInfoTitle, "전역 단축키 등록 실패");
        let message = format!("{name} ({key}) 단축키를 다른 앱이 사용 중입니다.");
        copy_wide(&mut data.szInfo, &message);
        data.dwInfoFlags = NIIF_WARNING;
        // SAFETY: data is fully initialized for NIM_MODIFY and references the registered icon.
        let shown = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool();
        if !shown {
            self.logger
                .event("트레이 풍선 알림 실패; MessageBox로 전환");
            let wide = wide(&message);
            // SAFETY: strings are NUL-terminated and hwnd is a valid owner window.
            unsafe {
                MessageBoxW(
                    Some(self.hwnd),
                    PCWSTR(wide.as_ptr()),
                    w!("EduTools"),
                    MB_OK | MB_ICONWARNING,
                )
            };
        }
    }

    pub fn shutdown(&mut self) {
        if self.registered {
            let data = self.base_data(NOTIFY_ICON_DATA_FLAGS(0));
            // SAFETY: data identifies this tray icon by hwnd and uID.
            if !unsafe { Shell_NotifyIconW(NIM_DELETE, &data) }.as_bool() {
                self.logger.event("트레이 아이콘 제거 실패");
            }
            self.registered = false;
        }
        if let Some(icon) = self.icon.take() {
            // SAFETY: icon was returned by LoadImageW without LR_SHARED and is destroyed once.
            if let Err(error) = unsafe { DestroyIcon(icon) } {
                self.logger
                    .event(&format!("트레이 아이콘 핸들 해제 실패: {error}"));
            }
        }
    }

    fn add_to_shell(&mut self, readding: bool) -> Result<()> {
        let data = self.base_data(NOTIFY_ICON_DATA_FLAGS(
            NIF_MESSAGE.0 | NIF_ICON.0 | NIF_TIP.0,
        ));
        // SAFETY: data contains a live hwnd, icon handle, callback message, and fixed strings.
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
            return Err(Error::from_thread());
        }
        self.registered = true;

        let mut version = self.base_data(NOTIFY_ICON_DATA_FLAGS(0));
        version.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        // SAFETY: version identifies the just-added icon and sets the documented version field.
        if !unsafe { Shell_NotifyIconW(NIM_SETVERSION, &version) }.as_bool() {
            let delete = self.base_data(NOTIFY_ICON_DATA_FLAGS(0));
            // SAFETY: delete identifies the icon that failed version negotiation.
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &delete) };
            self.registered = false;
            return Err(Error::from_thread());
        }
        self.logger.event(if readding {
            "트레이 아이콘 재등록 성공"
        } else {
            "트레이 아이콘 등록 성공"
        });
        Ok(())
    }

    fn replace_icon(&mut self, reason: &str, modify_shell: bool) -> Result<()> {
        // SAFETY: hwnd belongs to this process. A zero DPI is handled explicitly below.
        let queried_dpi = unsafe { GetDpiForWindow(self.hwnd) };
        let dpi = if queried_dpi == 0 {
            self.logger.event("주 모니터 DPI 조회 실패; 96 DPI로 계속");
            96
        } else {
            queried_dpi
        };
        // SAFETY: SM_CXSMICON and a positive DPI are valid inputs.
        let queried_size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) };
        let size = if queried_size <= 0 {
            self.logger.event("작은 아이콘 크기 조회 실패; 16px로 계속");
            16
        } else {
            queried_size
        };
        let resource_id = self.resource_id();
        let icon = load_resource_icon(self.hinstance, resource_id, size)?;

        if modify_shell && self.registered {
            let mut data = self.base_data(NOTIFY_ICON_DATA_FLAGS(NIF_ICON.0 | NIF_TIP.0));
            data.hIcon = icon;
            // SAFETY: data references a new live icon; it is retained on success.
            if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool() {
                // SAFETY: icon was not adopted and is owned by this function.
                let _ = unsafe { DestroyIcon(icon) };
                return Err(Error::from_thread());
            }
        }

        if let Some(old) = self.icon.replace(icon) {
            // SAFETY: old was returned by LoadImageW and is no longer used by the shell.
            if let Err(error) = unsafe { DestroyIcon(old) } {
                self.logger
                    .event(&format!("이전 트레이 아이콘 해제 실패: {error}"));
            }
        }
        self.dpi = dpi;
        self.size = size;
        self.logger.event(&format!(
            "트레이 아이콘 교체: 이유={reason}, 테마={}, 상태={}, dpi={dpi}, 크기={size}px, 리소스={resource_id}",
            if self.light_taskbar { "light-taskbar" } else { "dark-taskbar" },
            if self.annotation_enabled { "켜짐" } else { "꺼짐" }
        ));
        Ok(())
    }

    fn resource_id(&self) -> u16 {
        match (self.annotation_enabled, self.light_taskbar) {
            (false, true) => ICON_OFF_LIGHT,
            (true, true) => ICON_ON_LIGHT,
            (false, false) => ICON_OFF_DARK,
            (true, false) => ICON_ON_DARK,
        }
    }

    fn base_data(&self, flags: NOTIFY_ICON_DATA_FLAGS) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: u32::try_from(size_of::<NOTIFYICONDATAW>())
                .expect("NOTIFYICONDATAW size must fit in u32"),
            hWnd: self.hwnd,
            uID: TRAY_ID,
            uFlags: flags,
            uCallbackMessage: CALLBACK_MESSAGE,
            hIcon: self.icon.unwrap_or_default(),
            ..Default::default()
        };
        copy_wide(&mut data.szTip, self.tooltip());
        data
    }

    fn tooltip(&self) -> &'static str {
        if self.annotation_enabled {
            "판서 모드 켜짐"
        } else {
            "판서 모드 꺼짐"
        }
    }
}

pub fn load_resource_icon(hinstance: HINSTANCE, id: u16, size: i32) -> Result<HICON> {
    let resource = PCWSTR(usize::from(id) as *const u16);
    // SAFETY: integer resources are represented by a MAKEINTRESOURCE-style pointer.
    let handle = unsafe {
        LoadImageW(
            Some(hinstance),
            resource,
            GDI_IMAGE_TYPE(IMAGE_ICON.0),
            size,
            size,
            LR_DEFAULTCOLOR,
        )
    }?;
    Ok(HICON(handle.0))
}

fn read_light_taskbar(logger: &Logger) -> Option<bool> {
    let mut value = 0u32;
    let mut size = u32::try_from(size_of::<u32>()).expect("DWORD size must fit in u32");
    // SAFETY: value points to a writable DWORD buffer with the matching byte count.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast::<c_void>()),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS {
        Some(value == 1)
    } else {
        logger.event(&format!(
            "SystemUsesLightTheme 조회 실패: Win32 오류 {}",
            status.0
        ));
        None
    }
}

fn copy_wide<const N: usize>(target: &mut [u16; N], value: &str) {
    target.fill(0);
    for (destination, source) in target
        .iter_mut()
        .take(N.saturating_sub(1))
        .zip(value.encode_utf16())
    {
        *destination = source;
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
