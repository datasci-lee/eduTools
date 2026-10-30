use crate::hotkey::HotkeyCenter;
use crate::log::Logger;
use crate::overlay::{OverlayManager, SYNC_MESSAGE};
use crate::tray::{self, Tray, CALLBACK_MESSAGE, MENU_EXIT, MENU_TOGGLE_ANNOTATION};
use edutools_core::dispatch::{dispatch, DispatchContext, Effect};
use edutools_core::settings::AppSettings;
use edutools_core::shortcut::{
    ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutScope,
};
use edutools_core::state::AppState;
use std::mem::size_of;
use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT,
    WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext,
    DPI_AWARENESS_PER_MONITOR_AWARE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, LoadImageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
    SetWindowLongPtrW, TranslateMessage, GDI_IMAGE_TYPE, GWLP_USERDATA, HICON, IMAGE_ICON,
    LR_DEFAULTCOLOR, MSG, WM_CONTEXTMENU, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_HOTKEY,
    WM_RBUTTONUP, WM_SETTINGCHANGE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};

const WINDOW_CLASS: PCWSTR = w!("EduToolsMain");

pub fn run() -> i32 {
    let logger = Logger::open();
    logger.event("EduTools 시작");

    if std::env::args_os()
        .skip(1)
        .any(|argument| argument == "--selftest")
    {
        let passed = run_selftest(&logger);
        logger.event(if passed {
            "--selftest 통과"
        } else {
            "--selftest 실패"
        });
        return if passed { 0 } else { 1 };
    }

    let instance = match SingleInstance::acquire() {
        Ok(Some(instance)) => instance,
        Ok(None) => {
            logger.event("단일 인스턴스: 두 번째 실행 종료");
            return 0;
        }
        Err(error) => {
            logger.event(&format!("단일 인스턴스 뮤텍스 생성 실패: {error}"));
            eprintln!("EduTools: 단일 인스턴스 뮤텍스를 만들 수 없습니다: {error}");
            return 1;
        }
    };

    let result = run_message_loop(logger.clone());
    drop(instance);
    match result {
        Ok(code) => code,
        Err(error) => {
            logger.event(&format!("앱 실행 실패: {error}"));
            eprintln!("EduTools: 앱을 시작할 수 없습니다: {error}");
            1
        }
    }
}

struct SingleInstance(HANDLE);

impl SingleInstance {
    fn acquire() -> Result<Option<Self>> {
        // SAFETY: the name is a static NUL-terminated string and no security attributes are used.
        let handle = unsafe { CreateMutexW(None, false, w!("Local\\EduTools.SingleInstance")) }?;
        // SAFETY: GetLastError must be read immediately after CreateMutexW succeeds.
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if already_exists {
            // SAFETY: handle is owned by this function and will not be used again.
            unsafe { CloseHandle(handle) }?;
            Ok(None)
        } else {
            Ok(Some(Self(handle)))
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: the mutex handle is owned by this guard and closed once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct App {
    hwnd: HWND,
    hinstance: HINSTANCE,
    taskbar_created: u32,
    state: AppState,
    settings: AppSettings,
    tray: Option<Tray>,
    hotkeys: Option<HotkeyCenter>,
    overlays: Option<OverlayManager>,
    logger: Logger,
    shutting_down: bool,
}

impl App {
    fn initialize(&mut self) -> Result<()> {
        self.tray = Some(Tray::add(self.hwnd, self.hinstance, self.logger.clone())?);
        let hotkeys = HotkeyCenter::register_defaults(
            self.hwnd,
            self.tray.as_mut().expect("tray was just initialized"),
            self.logger.clone(),
        );
        self.hotkeys = Some(hotkeys);
        match OverlayManager::new(self.hwnd, self.hinstance, self.logger.clone()) {
            Ok(overlays) => self.overlays = Some(overlays),
            Err(error) => {
                self.report_overlay_error("초기화", &error);
                return Err(error);
            }
        }
        Ok(())
    }

    fn handle_hotkey(&mut self, id: i32) {
        let definition = self
            .hotkeys
            .as_ref()
            .and_then(|center| center.definition(id));
        let Some(definition) = definition else {
            self.logger.event(&format!("알 수 없는 WM_HOTKEY id={id}"));
            return;
        };
        if definition.command == ShortcutCommand::ToggleAnnotationMode {
            self.logger.event("전역 키 annotation.toggle 눌림");
            self.toggle_annotation("전역 키");
        } else {
            self.logger.event(&format!(
                "미구현 명령: {} ({})",
                definition.id, definition.name
            ));
        }
    }

    fn toggle_annotation(&mut self, source: &str) {
        let context = DispatchContext {
            scope: ShortcutScope::Global,
            ..DispatchContext::default()
        };
        match dispatch(
            &ShortcutCommand::ToggleAnnotationMode,
            &mut self.state,
            &self.settings,
            context,
        ) {
            Ok(effects) => self.apply_effects(effects, source),
            Err(error) => self.logger.event(&format!(
                "annotation.toggle core dispatch 실패 ({source}): {error}"
            )),
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>, source: &str) {
        for effect in effects {
            match effect {
                Effect::FinishAllActiveInteractions => {
                    self.logger
                        .event("core Effect::FinishAllActiveInteractions (W1 활성 상호작용 없음)");
                }
                Effect::AnnotationModeChanged(enabled) => {
                    if let Some(tray) = self.tray.as_mut() {
                        tray.set_annotation_enabled(enabled);
                    }
                    if let Some(overlays) = self.overlays.as_mut() {
                        if let Err(error) = overlays.set_annotation_mode(enabled) {
                            self.report_overlay_error("판서 모드 반영", &error);
                        }
                    }
                    self.logger.event(&format!(
                        "annotation.toggle 결과 ({source}): {}",
                        if enabled { "켜짐" } else { "꺼짐" }
                    ));
                }
                other => self
                    .logger
                    .event(&format!("W1에서 해석하지 않는 core effect: {other:?}")),
            }
        }
    }

    fn synchronize_overlays(&mut self, reason: &str) {
        if let Some(overlays) = self.overlays.as_mut() {
            if let Err(error) = overlays.synchronize(reason) {
                self.report_overlay_error("모니터 재구성", &error);
            }
        }
    }

    fn report_overlay_error(&self, context: &str, error: &Error) {
        let message = format!("오버레이 {context} 실패: {error}");
        self.logger.event(&message);
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning("오버레이 오류", &message);
        }
    }

    fn handle_tray_callback(&mut self, event: u32) {
        if event != WM_CONTEXTMENU && event != WM_RBUTTONUP {
            return;
        }
        let selected = self.tray.as_ref().and_then(Tray::show_menu);
        match selected {
            Some(MENU_TOGGLE_ANNOTATION) => {
                self.logger.event("트레이 메뉴 선택: 판서 모드");
                self.toggle_annotation("트레이 메뉴");
            }
            Some(MENU_EXIT) => {
                self.logger.event("트레이 메뉴 선택: EduTools 종료");
                // SAFETY: hwnd is this app's hidden top-level window.
                if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
                    self.logger.event(&format!("메인 창 종료 실패: {error}"));
                }
            }
            Some(command) => self
                .logger
                .event(&format!("알 수 없는 트레이 메뉴 명령: {command}")),
            None => {}
        }
    }

    fn shutdown(&mut self) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        // Overlay HWNDs and graphics resources must be released on the UI thread before the tray
        // and hidden owner window go away.
        self.overlays = None;
        if let Some(hotkeys) = self.hotkeys.as_mut() {
            hotkeys.shutdown();
        }
        if let Some(tray) = self.tray.as_mut() {
            tray.shutdown();
        }
        self.logger.event("EduTools 종료");
    }
}

fn run_message_loop(logger: Logger) -> Result<i32> {
    // SAFETY: None requests the current executable module handle.
    let module = unsafe { GetModuleHandleW(None) }?;
    let hinstance = HINSTANCE(module.0);
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: hinstance,
        lpszClassName: WINDOW_CLASS,
        ..Default::default()
    };
    // SAFETY: class points to initialized process-lifetime data and a valid window procedure.
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(Error::from_thread());
    }
    // SAFETY: class is registered, strings are static, and this creates a hidden top-level window.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            WINDOW_CLASS,
            w!("EduTools"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance),
            None,
        )
    }?;
    // SAFETY: the registered message name is a static NUL-terminated string.
    let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    if taskbar_created == 0 {
        logger.event("TaskbarCreated 메시지 등록 실패");
    }

    let mut app = Box::new(App {
        hwnd,
        hinstance,
        taskbar_created,
        state: AppState::new(),
        settings: AppSettings::default(),
        tray: None,
        hotkeys: None,
        overlays: None,
        logger,
        shutting_down: false,
    });
    let app_ptr: *mut App = app.as_mut();
    // SAFETY: app is boxed and remains at a stable address until after the message loop.
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_ptr as isize) };
    if let Err(error) = app.initialize() {
        app.shutdown();
        // SAFETY: clearing user data prevents callbacks from observing a soon-to-drop App.
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
        // SAFETY: hwnd was created above and has not yet been destroyed.
        let _ = unsafe { DestroyWindow(hwnd) };
        return Err(error);
    }

    let mut message = MSG::default();
    let exit_code = loop {
        // SAFETY: message points to writable storage and this is the UI thread's message loop.
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if status.0 == -1 {
            app.shutdown();
            break Err(Error::from_thread());
        }
        if status.0 == 0 {
            break Ok(i32::try_from(message.wParam.0).unwrap_or(1));
        }
        // SAFETY: message was populated by GetMessageW.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    };
    app.shutdown();
    exit_code
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: hwnd is supplied by Windows to this procedure; GWLP_USERDATA is either zero or App.
    let app_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App };
    if !app_ptr.is_null() {
        // SAFETY: App is boxed for the duration of the message loop and accessed only on this thread.
        let app = unsafe { &mut *app_ptr };
        if app.taskbar_created != 0 && message == app.taskbar_created {
            if let Some(tray) = app.tray.as_mut() {
                tray.readd_after_taskbar_created();
            }
            return LRESULT(0);
        }
        match message {
            WM_HOTKEY => {
                app.handle_hotkey(wparam.0 as i32);
                return LRESULT(0);
            }
            CALLBACK_MESSAGE => {
                app.handle_tray_callback((lparam.0 as u32) & 0xffff);
                return LRESULT(0);
            }
            SYNC_MESSAGE => {
                app.synchronize_overlays("오버레이 메시지");
                return LRESULT(0);
            }
            WM_SETTINGCHANGE => {
                if let Some(tray) = app.tray.as_mut() {
                    tray.refresh_theme();
                }
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                if let Some(tray) = app.tray.as_mut() {
                    tray.refresh_display("WM_DPICHANGED");
                }
                return LRESULT(0);
            }
            WM_DISPLAYCHANGE => {
                if let Some(tray) = app.tray.as_mut() {
                    tray.refresh_display("WM_DISPLAYCHANGE");
                }
                app.synchronize_overlays("WM_DISPLAYCHANGE");
                return LRESULT(0);
            }
            WM_DESTROY => {
                app.shutdown();
                // SAFETY: posting quit from the UI thread ends the matching message loop.
                unsafe { PostQuitMessage(0) };
                return LRESULT(0);
            }
            _ => {}
        }
    }
    // SAFETY: unhandled messages are delegated to the system default procedure.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn run_selftest(logger: &Logger) -> bool {
    let mut failures = Vec::new();
    // SAFETY: None requests the current executable module handle.
    let hinstance = match unsafe { GetModuleHandleW(None) } {
        Ok(module) => HINSTANCE(module.0),
        Err(error) => {
            failures.push(format!("리소스: 모듈 핸들 조회 실패: {error}"));
            HINSTANCE::default()
        }
    };

    if !hinstance.is_invalid() {
        for id in tray::ALL_ICON_RESOURCE_IDS {
            for size in [16, 20, 24, 32, 48] {
                let resource = PCWSTR(usize::from(id) as *const u16);
                // SAFETY: integer resources use a MAKEINTRESOURCE-style pointer and sizes are valid.
                match unsafe {
                    LoadImageW(
                        Some(hinstance),
                        resource,
                        GDI_IMAGE_TYPE(IMAGE_ICON.0),
                        size,
                        size,
                        LR_DEFAULTCOLOR,
                    )
                } {
                    Ok(handle) => {
                        // SAFETY: handle came from LoadImageW without LR_SHARED and is destroyed once.
                        if let Err(error) = unsafe { DestroyIcon(HICON(handle.0)) } {
                            failures
                                .push(format!("리소스: 아이콘 {id} {size}px 해제 실패: {error}"));
                        }
                    }
                    Err(error) => {
                        failures.push(format!("리소스: 아이콘 {id} {size}px 로드 실패: {error}"))
                    }
                }
            }
        }

        match OverlayManager::new(HWND::default(), hinstance, logger.clone()) {
            Ok(mut overlays) => failures.extend(overlays.selftest_failures()),
            Err(error) => failures.push(format!("overlay: 생성 실패: {error}")),
        }
    }

    // SAFETY: these functions inspect the current thread's manifest-selected DPI context.
    let awareness = unsafe { GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext()) };
    if awareness != DPI_AWARENESS_PER_MONITOR_AWARE {
        failures.push(format!(
            "매니페스트: DPI awareness가 PerMonitor가 아님 ({})",
            awareness.0
        ));
    }

    let global = ShortcutCatalog::definitions()
        .into_iter()
        .filter_map(|definition| {
            definition
                .binding(ShortcutScope::Global)
                .cloned()
                .map(|binding| (definition.id, binding))
        })
        .collect::<Vec<_>>();
    if global.len() != 7 {
        failures.push(format!(
            "keymap: 전역 기본 바인딩 수가 7이 아님 ({})",
            global.len()
        ));
    }
    for (id, binding) in &global {
        match crate::keymap::token_to_vk(&binding.key) {
            Some(vk) if crate::keymap::vk_to_token(vk) == Some(binding.key.as_str()) => {}
            Some(vk) => failures.push(format!(
                "keymap: {id} 키 토큰 {}의 VK {vk:#x} 역변환 불일치",
                binding.key
            )),
            None => failures.push(format!(
                "keymap: {id} 키 토큰 {}을 VK로 바꿀 수 없음",
                binding.key
            )),
        }
    }

    let annotation = ShortcutCatalog::definitions()
        .into_iter()
        .find(|definition| definition.id == "annotation.toggle");
    match annotation.and_then(|definition: ShortcutDefinition| {
        definition.binding(ShortcutScope::Global).cloned()
    }) {
        Some(binding) if binding.portable_string == "AppMod+P" => {}
        Some(binding) => failures.push(format!(
            "카탈로그: annotation.toggle 기본 바인딩이 AppMod+P가 아님 ({})",
            binding.portable_string
        )),
        None => failures.push("카탈로그: annotation.toggle 전역 기본 바인딩이 없음".to_owned()),
    }

    for failure in &failures {
        eprintln!("--selftest: {failure}");
        logger.event(&format!("--selftest 실패: {failure}"));
    }
    failures.is_empty()
}

const _: () = {
    assert!(size_of::<usize>() >= size_of::<u32>());
};
