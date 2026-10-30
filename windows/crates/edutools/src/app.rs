use crate::hotkey::HotkeyCenter;
use crate::ink_view::InkInputConfig;
use crate::kbhook::{HookKeyEvent, KeyboardHook, WM_APP_KEY};
use crate::log::Logger;
use crate::overlay::{OverlayManager, SYNC_MESSAGE};
use crate::palette::{
    command_for_item, item_enabled, PaletteItemId, PaletteManager, WM_APP_PALETTE_ACTION,
    WM_APP_PALETTE_REDRAW, WM_APP_PALETTE_SAVE_POSITION, WM_APP_PALETTE_TOGGLE_ORIENTATION,
};
use crate::tray::{self, Tray, CALLBACK_MESSAGE, MENU_EXIT, MENU_TOGGLE_ANNOTATION};
use edutools_core::dispatch::{dispatch, DispatchContext, Effect, PressedShortcuts};
use edutools_core::ink::{
    InkGeometry, InkPoint, InkStroke, InkStrokeContent, InkStrokeTool, InkTool, PathCommand, Point,
    Size,
};
use edutools_core::settings::AppSettings;
use edutools_core::shortcut::{
    ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutScope,
};
use edutools_core::state::AppState;
use std::mem::size_of;
use std::path::PathBuf;
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
use windows::Win32::UI::Input::Pointer::EnableMouseInPointer;
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

    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|value| value == "--selftest-icons")
    {
        if arguments.len() != 2 {
            eprintln!("사용법: edutools --selftest-icons <출력 폴더>");
            return 2;
        }
        let output = PathBuf::from(&arguments[1]);
        return match crate::app_icon::run_selftest_icons(&output, &logger) {
            Ok(()) => 0,
            Err(error) => {
                logger.event(&format!("--selftest-icons 실패: {error}"));
                eprintln!("--selftest-icons: {error}");
                1
            }
        };
    }

    // SAFETY: this process-wide opt-in is called exactly once, before any app windows are created.
    if let Err(error) = unsafe { EnableMouseInPointer(true) } {
        logger.event(&format!("EnableMouseInPointer(TRUE) 실패: {error}"));
        eprintln!("EduTools: WM_POINTER 마우스 입력을 활성화할 수 없습니다: {error}");
        return 1;
    }

    if arguments.iter().any(|argument| argument == "--selftest") {
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
    keyboard_hook: Option<KeyboardHook>,
    pressed_shortcuts: PressedShortcuts,
    overlays: Option<OverlayManager>,
    palette: Option<PaletteManager>,
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
        let input_config = InkInputConfig::from_core(&self.state, &self.settings);
        match OverlayManager::new(self.hwnd, self.hinstance, input_config, self.logger.clone()) {
            Ok(overlays) => self.overlays = Some(overlays),
            Err(error) => {
                self.report_overlay_error("초기화", &error);
                return Err(error);
            }
        }
        let mut palette = PaletteManager::new(self.hwnd, self.hinstance, self.logger.clone())?;
        palette.refresh(&self.state, &self.settings)?;
        self.palette = Some(palette);
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

    fn handle_annotation_key(&mut self, event: HookKeyEvent) {
        let definition = self
            .keyboard_hook
            .as_ref()
            .and_then(|hook| hook.definition(event.definition_index))
            .cloned();
        let Some(definition) = definition else {
            self.logger.event(&format!(
                "키보드 훅 정의 인덱스 범위 밖: physicalKey={:#x} index={}",
                event.key_code, event.definition_index
            ));
            return;
        };
        self.logger.event(&format!(
            "판서 키 {}: physicalKey={:#x} 정의={} 소비={}",
            if event.is_key_up { "up" } else { "down" },
            event.key_code,
            definition.id,
            event.consumed
        ));
        if !event.consumed {
            return;
        }

        let display_id = match self
            .overlays
            .as_ref()
            .map(OverlayManager::monitor_key_at_cursor)
        {
            Some(Ok(display_id)) => display_id,
            Some(Err(error)) => {
                self.logger
                    .event(&format!("커서 모니터 판정 실패: {error}"));
                None
            }
            None => None,
        };
        let context = DispatchContext {
            scope: ShortcutScope::Annotation,
            display_id: display_id.as_deref(),
            ..DispatchContext::default()
        };
        let dispatch = if event.is_key_up {
            Ok(self
                .pressed_shortcuts
                .key_up(event.key_code, &mut self.state))
        } else {
            self.pressed_shortcuts.key_down(
                event.key_code,
                &definition,
                &mut self.state,
                &self.settings,
                context,
            )
        };
        match dispatch {
            Ok(result) => {
                if result.consumed != event.consumed {
                    self.logger.event(&format!(
                        "키 소비 판정 불일치: 정의={} hook={} core={}",
                        definition.id, event.consumed, result.consumed
                    ));
                }
                self.apply_effects(result.effects, &format!("판서 키 {}", definition.id));
            }
            Err(error) => self.logger.event(&format!(
                "판서 키 core dispatch 실패: 정의={} error={error}",
                definition.id
            )),
        }
    }

    fn set_keyboard_hook(&mut self, enabled: bool) {
        if enabled {
            if self.keyboard_hook.is_some() {
                return;
            }
            match KeyboardHook::install(self.hwnd, &self.settings) {
                Ok(hook) => {
                    self.keyboard_hook = Some(hook);
                    self.logger.event("판서 키보드 훅 설치 성공");
                }
                Err(error) => {
                    let message = format!("판서 키보드 훅 설치 실패: {error}");
                    self.logger.event(&message);
                    if let Some(tray) = self.tray.as_ref() {
                        tray.show_warning("키보드 훅 오류", &message);
                    }
                }
            }
            return;
        }

        let Some(mut hook) = self.keyboard_hook.take() else {
            return;
        };
        match hook.uninstall() {
            Ok(()) => {
                self.pressed_shortcuts = PressedShortcuts::new();
                self.logger.event("판서 키보드 훅 해제 성공");
            }
            Err(error) => {
                let message = format!("판서 키보드 훅 해제 실패: {error}");
                self.logger.event(&message);
                if let Some(tray) = self.tray.as_ref() {
                    tray.show_warning("키보드 훅 오류", &message);
                }
                self.keyboard_hook = Some(hook);
            }
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>, source: &str) {
        for effect in effects {
            match effect {
                Effect::FinishAllActiveInteractions => {
                    if let Some(overlays) = self.overlays.as_mut() {
                        if let Err(error) = overlays.finish_all_active("core finish effect") {
                            self.logger
                                .event(&format!("활성 획 확정 실패 (core finish effect): {error}"));
                        }
                    }
                }
                Effect::AnnotationModeChanged(enabled) => {
                    self.set_keyboard_hook(enabled);
                    if let Some(tray) = self.tray.as_mut() {
                        tray.set_annotation_enabled(enabled);
                    }
                    let input_config = InkInputConfig::from_core(&self.state, &self.settings);
                    if let Some(overlays) = self.overlays.as_mut() {
                        if let Err(error) = overlays.set_annotation_mode(enabled, input_config) {
                            self.report_overlay_error("판서 모드 반영", &error);
                        }
                    }
                    self.logger.event(&format!(
                        "annotation.toggle 결과 ({source}): {}",
                        if enabled { "켜짐" } else { "꺼짐" }
                    ));
                }
                Effect::PassThroughModeChanged(enabled) => {
                    if let Some(overlays) = self.overlays.as_mut() {
                        if let Err(error) = overlays.set_pass_through(enabled) {
                            self.report_overlay_error("통과 모드 반영", &error);
                        }
                    }
                    self.logger.event(&format!(
                        "효과 ({source}): 통과 모드 {}",
                        if enabled { "켜짐" } else { "꺼짐" }
                    ));
                }
                Effect::ToolChanged(tool) => {
                    self.logger
                        .event(&format!("효과 ({source}): 도구 선택 {tool:?}"));
                    if tool == InkTool::Eraser {
                        // 지우개 모드 HUD는 후속 팔레트 라운드 전까지 로그로 대체한다.
                        self.logger.event(&format!(
                            "지우개 모드 HUD 대체 로그: {}",
                            self.settings.eraser.mode
                        ));
                    }
                }
                Effect::SettingsMutation(mutation) => {
                    mutation.apply(&mut self.settings);
                    self.logger
                        .event(&format!("효과 ({source}): 설정 변경 {mutation:?}"));
                    if self.state.current_tool() == InkTool::Eraser {
                        self.logger.event(&format!(
                            "지우개 모드 HUD 대체 로그: {}",
                            self.settings.eraser.mode
                        ));
                    }
                }
                Effect::Undo(display_id) => {
                    let result = self
                        .overlays
                        .as_mut()
                        .map(|overlays| overlays.undo(&display_id));
                    match result {
                        Some(Ok(changed)) => self.logger.event(&format!(
                            "효과 ({source}): undo monitor={display_id} changed={changed}"
                        )),
                        Some(Err(error)) => self.report_overlay_error("undo", &error),
                        None => {}
                    }
                }
                Effect::Redo(display_id) => {
                    let result = self
                        .overlays
                        .as_mut()
                        .map(|overlays| overlays.redo(&display_id));
                    match result {
                        Some(Ok(changed)) => self.logger.event(&format!(
                            "효과 ({source}): redo monitor={display_id} changed={changed}"
                        )),
                        Some(Err(error)) => self.report_overlay_error("redo", &error),
                        None => {}
                    }
                }
                Effect::ClearAll => {
                    let result = self.overlays.as_mut().map(OverlayManager::clear_all);
                    match result {
                        Some(Ok(changed_layers)) => self.logger.event(&format!(
                            "효과 ({source}): 전체 지우기 layers={changed_layers}"
                        )),
                        Some(Err(error)) => self.report_overlay_error("전체 지우기", &error),
                        None => {}
                    }
                }
                Effect::StepBack(result) => self
                    .logger
                    .event(&format!("효과 ({source}): 한 단계 나가기 {result:?}")),
                Effect::PaletteVisibilityChanged(visible) => self
                    .logger
                    .event(&format!("효과 ({source}): 선형 팔레트 표시 상태 {visible}")),
                other => self
                    .logger
                    .event(&format!("이번 라운드에서 UI 미구현 core effect: {other:?}")),
            }
        }
        let input_config = InkInputConfig::from_core(&self.state, &self.settings);
        if let Some(overlays) = self.overlays.as_mut() {
            if let Err(error) = overlays.update_input_context(input_config) {
                self.logger
                    .event(&format!("오버레이 입력 설정 반영 실패: {error}"));
            }
        }
        self.refresh_palette("core effect");
    }

    fn synchronize_overlays(&mut self, reason: &str) {
        if let Some(overlays) = self.overlays.as_mut() {
            if let Err(error) = overlays.synchronize(reason) {
                self.report_overlay_error("모니터 재구성", &error);
            }
        }
        if let Some(palette) = self.palette.as_mut() {
            if let Err(error) = palette.display_configuration_changed(&self.state, &self.settings) {
                self.report_palette_error("모니터 재구성", &error);
            }
        }
    }

    fn handle_palette_action(&mut self, value: usize) {
        let Some(item) = PaletteItemId::from_message(value) else {
            self.logger
                .event(&format!("팔레트 항목 메시지 범위 밖: {value}"));
            return;
        };
        let Some(command) = command_for_item(item) else {
            self.logger
                .event(&format!("팔레트 비동작 항목 클릭 무시: {item:?}"));
            return;
        };
        if !item_enabled(item, &self.state, &self.settings) {
            self.logger
                .event(&format!("팔레트 비활성 항목 클릭 무시: {item:?}"));
            return;
        }
        let display_id = match self.palette.as_ref().map(PaletteManager::monitor_key) {
            Some(Ok(display_id)) => display_id,
            Some(Err(error)) => {
                self.report_palette_error("대상 모니터 판정", &error);
                None
            }
            None => None,
        };
        let context = DispatchContext {
            scope: ShortcutScope::Annotation,
            display_id: display_id.as_deref(),
            board_display_id: display_id.as_deref(),
            ..DispatchContext::default()
        };
        match dispatch(&command, &mut self.state, &self.settings, context) {
            Ok(effects) => self.apply_effects(effects, &format!("팔레트 {item:?}")),
            Err(error) => self.logger.event(&format!(
                "팔레트 core dispatch 실패: item={item:?} command={command:?}: {error}"
            )),
        }
    }

    fn refresh_palette(&mut self, reason: &str) {
        if let Some(palette) = self.palette.as_mut() {
            if let Err(error) = palette.refresh(&self.state, &self.settings) {
                self.report_palette_error(reason, &error);
            }
        }
    }

    fn redraw_palette(&mut self) {
        if let Some(palette) = self.palette.as_mut() {
            if let Err(error) = palette.redraw(&self.state, &self.settings) {
                self.report_palette_error("다시 그리기", &error);
            }
        }
    }

    fn save_palette_position(&mut self) {
        if let Some(palette) = self.palette.as_mut() {
            if let Err(error) = palette.save_position() {
                self.report_palette_error("위치 저장", &error);
            }
        }
    }

    fn toggle_palette_orientation(&mut self) {
        if let Some(palette) = self.palette.as_mut() {
            if let Err(error) = palette.toggle_orientation(&self.state, &self.settings) {
                self.report_palette_error("가로/세로 전환", &error);
            }
        }
    }

    fn report_palette_error(&self, context: &str, error: &Error) {
        let message = format!("팔레트 {context} 실패: {error}");
        self.logger.event(&message);
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning("팔레트 오류", &message);
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
        self.set_keyboard_hook(false);
        // Overlay HWNDs and graphics resources must be released on the UI thread before the tray
        // and hidden owner window go away.
        self.palette = None;
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
        keyboard_hook: None,
        pressed_shortcuts: PressedShortcuts::new(),
        overlays: None,
        palette: None,
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
            WM_APP_KEY => {
                app.handle_annotation_key(HookKeyEvent::decode(wparam, lparam));
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
            WM_APP_PALETTE_ACTION => {
                app.handle_palette_action(wparam.0);
                return LRESULT(0);
            }
            WM_APP_PALETTE_REDRAW => {
                app.redraw_palette();
                return LRESULT(0);
            }
            WM_APP_PALETTE_SAVE_POSITION => {
                app.save_palette_position();
                return LRESULT(0);
            }
            WM_APP_PALETTE_TOGGLE_ORIENTATION => {
                app.toggle_palette_orientation();
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

        let state = AppState::new();
        let settings = AppSettings::default();
        match OverlayManager::new(
            HWND::default(),
            hinstance,
            InkInputConfig::from_core(&state, &settings),
            logger.clone(),
        ) {
            Ok(mut overlays) => failures.extend(overlays.selftest_failures()),
            Err(error) => failures.push(format!("overlay: 생성 실패: {error}")),
        }

        let state = AppState::new();
        let settings = AppSettings::default();
        match PaletteManager::new(HWND::default(), hinstance, logger.clone()) {
            Ok(mut palette) => failures.extend(palette.selftest_failures(&state, &settings)),
            Err(error) => failures.push(format!("palette: 생성 실패: {error}")),
        }
    }

    let size = Size {
        width: 1_920.0,
        height: 1_080.0,
    };
    let pixel = Point { x: 413.0, y: 777.0 };
    let round_trip = InkGeometry::screen_point(InkGeometry::normalized_point(pixel, size), size);
    if (round_trip.x - pixel.x).abs() > 1.0 || (round_trip.y - pixel.y).abs() > 1.0 {
        failures.push(format!(
            "ink: 픽셀/정규화 왕복 오차가 1px 초과 ({round_trip:?} != {pixel:?})"
        ));
    }

    let smoke_stroke = InkStroke::new(
        "selftest-outline",
        InkStrokeTool::Pen,
        [1.0, 0.2, 0.1, 1.0],
        6.0,
        true,
        vec![
            InkPoint::new(Point { x: 0.1, y: 0.2 }, 0.25, 0.0),
            InkPoint::new(Point { x: 0.5, y: 0.6 }, 0.75, 1.0),
        ],
        InkStrokeContent::Freehand,
    );
    let outline = InkGeometry::variable_width_path(
        &smoke_stroke,
        &[
            (Point { x: 100.0, y: 100.0 }, 0.25),
            (Point { x: 400.0, y: 600.0 }, 0.75),
        ],
        0.2,
    );
    if outline.is_empty()
        || outline
            .iter()
            .filter(|command| matches!(command, PathCommand::MoveTo(_)))
            .count()
            != 3
    {
        failures.push("ink: 가변 굵기 외곽선 스모크 결과가 비었거나 끝 원이 없음".to_owned());
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

    for binding in crate::kbhook::unmappable_annotation_bindings(&AppSettings::default()) {
        failures.push(format!(
            "keymap: 판서 바인딩을 VK로 바꿀 수 없음: {binding}"
        ));
    }

    match KeyboardHook::install(HWND::default(), &AppSettings::default()) {
        Ok(mut hook) => match hook.uninstall() {
            Ok(()) => logger.event("--selftest: 키보드 훅 설치/해제 왕복 통과"),
            Err(error) => failures.push(format!("keyboard hook: 해제 실패: {error}")),
        },
        Err(error) => failures.push(format!("keyboard hook: 설치 실패: {error}")),
    }

    let three_dip_at_144 = 3.0 * crate::render::px_per_dip(144);
    if (three_dip_at_144 - 4.5).abs() > f64::EPSILON {
        failures.push(format!(
            "DIP: 144 DPI에서 3 DIP가 4.5px가 아님 ({three_dip_at_144})"
        ));
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
