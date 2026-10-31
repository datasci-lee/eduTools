use crate::break_screen::{
    BreakStartOutcome, BreakTimerManager, WM_APP_BREAK_ACTION, WM_APP_BREAK_TICK,
};
use crate::capture::{
    CapturedFrame, ZoomCapture, WM_APP_EXPORT_CAPTURE_FAILED, WM_APP_EXPORT_FRAME,
    WM_APP_ZOOM_CAPTURE_FAILED, WM_APP_ZOOM_FRAME, WM_APP_ZOOM_RECREATE,
};
use crate::display_apply::{
    decision_from_command, is_console_session, recover_pending_display_configuration,
    run_display_apply_dry_run, ConfirmationDecision, ConfirmationResolution, DisplayApplier,
    DisplayApplySource, DisplayApplyState, ExpectedDisplayConfiguration, WM_APP_DISPLAY_DECISION,
    WM_APP_DISPLAY_EXPIRED,
};
use crate::export::{ExportController, EXPORT_CAPTURE_TIMEOUT_TIMER_ID};
use crate::help::{
    monotonic_millis, monotonic_seconds, HelpManager, HELP_RELEASE_TIMER_ID, WM_APP_HELP_ACTION,
    WM_APP_HELP_CLOSE, WM_APP_HELP_REFRESH,
};
use crate::hotkey::HotkeyCenter;
use crate::ink_view::InkInputConfig;
use crate::kbhook::{
    current_modifiers, HookKeyEvent, HookScope, KeyboardHook, RadialKeyCapHookEvent,
    RadialPaletteKeyEvent, WM_APP_KEY, WM_APP_RADIAL_KEY, WM_APP_RADIAL_KEY_CAP,
};
use crate::keymap;
use crate::launch_at_login::{self, SyncOutcome, WindowsRunRegistry};
use crate::lesson_badge::{
    play_completion_sound, LessonBadgeManager, SettingsMutation as LessonSettingsMutation,
    WM_APP_LESSON_ACTION, WM_APP_LESSON_COMPLETED,
};
use crate::log::Logger;
use crate::monitor::{
    current_display_configuration, enumerate_monitors, hash_prefix8, local_point_in_monitor,
    monitor_at_point, ConnectionSetOutcome, DisplayChangeState, DisplayConfiguration,
    DisplayTargetKey, Monitor,
};
use crate::mousehook::{MouseHook, MouseHookEvent, MouseHookSubscriber, WM_APP_PRESENTER_MOUSE};
use crate::music::{BreakMusicManager, WM_APP_MUSIC_LAUNCH_RESULT, WM_APP_MUSIC_PAUSE_RESULT};
use crate::overlay::{OverlayManager, SYNC_MESSAGE};
use crate::palette::{
    command_for_item, item_enabled, PaletteItemId, PaletteManager, WM_APP_PALETTE_ACTION,
    WM_APP_PALETTE_REDRAW, WM_APP_PALETTE_SAVE_POSITION, WM_APP_PALETTE_TOGGLE_ORIENTATION,
};
use crate::pass_through::{key_poll_decision, KeyPollDecision, PassThroughTimerPlan};
use crate::presenter::{
    PresenterEffects, PRESENTER_FRAME_INTERVAL_MS, PRESENTER_FRAME_TIMER_ID, WM_APP_PRESENTER_KEY,
};
use crate::radial_palette::{
    RadialPaletteManager, WM_APP_RADIAL_ACTION, WM_APP_RADIAL_COLLAPSE, WM_APP_RADIAL_REDRAW,
    WM_APP_RADIAL_SAVE_POSITION,
};
use crate::render::GraphicsDevice;
use crate::settings_file::FileSettingsStorage;
use crate::settings_ui::{
    apply_change as apply_settings_window_change, choose_settings_export_path,
    choose_settings_import_path, new_preset_id, DisplayApplyAvailability, DisplayCommand,
    DisplayEdit, DisplayMutationKind, DisplayMutationRequest, SettingsAction,
    SettingsChangeRequest, SettingsTab, SettingsWindow, WM_APP_SETTINGS_ACTION,
    WM_APP_SETTINGS_CHANGE,
};
use crate::startup::{self, file_name_from_path, LaunchRequest, COPYDATA_TAG, MAX_COPYDATA_BYTES};
use crate::text_entry::{
    cancel_reason, TextEntryRequest, TextEntryWindow, WM_APP_TEXT_ENTRY_CANCEL,
    WM_APP_TEXT_ENTRY_COMMIT, WM_APP_TEXT_ENTRY_OPEN,
};
use crate::tray::{
    self, display_preset_index, Tray, CALLBACK_MESSAGE, MENU_DISPLAY_SETTINGS, MENU_EXIT,
    MENU_SETTINGS, MENU_TOGGLE_ANNOTATION,
};
use crate::zoom_view::{
    decode_pointer, decode_wheel, ZoomView, WM_APP_ZOOM_INK_INPUT, WM_APP_ZOOM_POINTER,
    WM_APP_ZOOM_REDRAW, WM_APP_ZOOM_WHEEL,
};
use edutools_core::board::BoardPresentation;
use edutools_core::dispatch::{dispatch, DispatchContext, Effect, PressedShortcuts};
use edutools_core::display_preset::{
    unique_name, DisplayAutoSwitchDecision, DisplayAutoSwitchPolicy, DisplayPlanner,
    DisplayPresetCapture, DisplayPresetDefaults,
};
use edutools_core::eraser::EraserMode;
use edutools_core::ink::{
    InkGeometry, InkPoint, InkStroke, InkStrokeContent, InkStrokeTool, InkTool, PathCommand, Point,
    Size,
};
use edutools_core::radial::{
    next_radial_key_cap_state, RadialKeyCapAction, RadialKeyCapEvent, RadialKeyCapState,
    RadialPaletteIntent, RadialPaletteKeyPassReason, RadialPaletteKeyboardInput,
    KEY_CAP_HOLD_DURATION_MS,
};
use edutools_core::settings::{
    changed_settings_groups, AppSettings, SettingsGroup, SettingsValidator,
};
use edutools_core::settings_store::{
    SettingsBackupStore, SettingsStore, SettingsStoreIssue, SettingsStoreIssueKind,
};
use edutools_core::settings_transfer::{SettingsBundle, SettingsTransfer};
use edutools_core::shortcut::{
    ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutModifiers, ShortcutScope,
};
use edutools_core::state::{
    AppState, FavoriteSaveResult, PassThroughDefaults, PassThroughModeSource, SettingsMutation,
};
use edutools_core::zoom::{
    stop_or_cancel_preview_action, zoom_gesture_control_locked, Point as ZoomPoint,
    Rect as ZoomRect, Size as ZoomSize, ZoomFollowMode, ZoomGeometry, ZoomGestureControl,
    ZoomSessionState, ZoomStopAction,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use windows::core::{w, Error, Result, PCWSTR};
use windows::Graphics::SizeInt32;
use windows::Win32::Devices::Display::GUID_DEVINTERFACE_MONITOR;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, E_FAIL, HANDLE, HINSTANCE, HWND, LPARAM,
    LRESULT, POINT, WPARAM,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext,
    DPI_AWARENESS_PER_MONITOR_AWARE,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_XBUTTON1, VK_XBUTTON2,
};
use windows::Win32::UI::Input::Pointer::EnableMouseInPointer;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, FindWindowW,
    GetCursorPos, GetForegroundWindow, GetMessageW, GetWindowLongPtrW, IsWindow, KillTimer,
    LoadImageW, MessageBoxW, PostQuitMessage, RegisterClassW, RegisterDeviceNotificationW,
    RegisterWindowMessageW, SendMessageTimeoutW, SetCursorPos, SetForegroundWindow, SetTimer,
    SetWindowLongPtrW, TranslateMessage, UnregisterDeviceNotification, DBT_DEVICEARRIVAL,
    DBT_DEVICEREMOVECOMPLETE, DBT_DEVTYP_DEVICEINTERFACE, DEVICE_NOTIFY_WINDOW_HANDLE,
    DEV_BROADCAST_DEVICEINTERFACE_W, GDI_IMAGE_TYPE, GWLP_USERDATA, HDEVNOTIFY, HICON, IDYES,
    IMAGE_ICON, LR_DEFAULTCOLOR, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO,
    MSG, SMTO_ABORTIFHUNG, WM_CONTEXTMENU, WM_COPYDATA, WM_DESTROY, WM_DEVICECHANGE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_HOTKEY, WM_KEYDOWN, WM_POWERBROADCAST, WM_RBUTTONUP,
    WM_SETTINGCHANGE, WM_TIMECHANGE, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

const WINDOW_CLASS: PCWSTR = w!("EduToolsMain");
const DISPLAY_SYNC_TIMER_ID: usize = 0xED31;
const DISPLAY_STABILITY_TIMER_ID: usize = 0xED32;
const DISPLAY_RETRY_TIMER_ID: usize = 0xED33;
const RADIAL_KEY_CAP_TIMER_ID: usize = 0xE118;
const PASS_THROUGH_IDLE_TIMER_ID: usize = 0xED42;
const PASS_THROUGH_MAXIMUM_HOLD_TIMER_ID: usize = 0xED43;
const PASS_THROUGH_KEY_POLL_TIMER_ID: usize = 0xED44;
const DISPLAY_SYNC_INTERVAL_MS: u32 = 300;

pub fn run() -> i32 {
    let logger = Logger::open();
    logger.event("EduTools 시작");

    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let request = startup::parse_arguments(&arguments);
    if let LaunchRequest::UsageError(message) = &request {
        eprintln!("{message}");
        logger.event(message);
        return 2;
    }
    if let LaunchRequest::SelftestIcons(output) = &request {
        return match crate::app_icon::run_selftest_icons(output, &logger) {
            Ok(()) => 0,
            Err(error) => {
                logger.event(&format!("--selftest-icons 실패: {error}"));
                eprintln!("--selftest-icons: {error}");
                1
            }
        };
    }

    // Development-only CCD dump; never used as a product feature.
    if request == LaunchRequest::PrintDisplays {
        return match current_display_configuration() {
            Ok(configuration) => {
                let line = configuration.diagnostic_line();
                println!("{line}");
                logger.event(&line);
                0
            }
            Err(error) => {
                logger.event(&format!("display-query 실패: {error}"));
                eprintln!("EduTools: 현재 디스플레이 구성을 읽지 못했습니다: {error}");
                1
            }
        };
    }

    if let LaunchRequest::DisplayApplyDryRun(index) = &request {
        return run_display_apply_dry_run(*index, &logger);
    }

    let selftest = request == LaunchRequest::Selftest;
    let import_path = match &request {
        LaunchRequest::App {
            import_path,
            warning,
        } => {
            if let Some(warning) = warning {
                logger.event(&format!(
                    "실행 인자 무시: file={} reason={}",
                    warning.file_name, warning.reason
                ));
            }
            import_path
                .as_deref()
                .and_then(|path| match startup::validate_import_path(path) {
                    Ok(path) => {
                        logger.event(&format!(
                            "실행 인자 승인: file={}",
                            file_name_from_path(&path)
                        ));
                        Some(path)
                    }
                    Err(reason) => {
                        logger.event(&format!(
                            "실행 인자 거부: file={} reason={reason}",
                            file_name_from_path(&path)
                        ));
                        None
                    }
                })
        }
        LaunchRequest::Selftest => None,
        _ => unreachable!("development requests returned before app startup"),
    };

    // SAFETY: this process-wide opt-in is called exactly once, before any app windows are created.
    if let Err(error) = unsafe { EnableMouseInPointer(true) } {
        logger.event(&format!("EnableMouseInPointer(TRUE) 실패: {error}"));
        eprintln!("EduTools: WM_POINTER 마우스 입력을 활성화할 수 없습니다: {error}");
        return 1;
    }

    if selftest {
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
            if let Some(path) = import_path.as_deref() {
                return if forward_import_path(path, &logger) {
                    0
                } else {
                    1
                };
            }
            logger.event("단일 인스턴스: 두 번째 실행 종료");
            return 0;
        }
        Err(error) => {
            logger.event(&format!("단일 인스턴스 뮤텍스 생성 실패: {error}"));
            eprintln!("EduTools: 단일 인스턴스 뮤텍스를 만들 수 없습니다: {error}");
            return 1;
        }
    };

    let result = run_message_loop(logger.clone(), import_path);
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

fn forward_import_path(path: &Path, logger: &Logger) -> bool {
    let mut payload = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let byte_count = payload.len() * 2;
    let file_name = file_name_from_path(path);
    if byte_count > MAX_COPYDATA_BYTES {
        logger.event(&format!(
            "COPYDATA 전송 거부: file={file_name} reason=경로가 너무 깁니다"
        ));
        return false;
    }

    let hwnd = (0..100).find_map(|_| {
        let found = unsafe { FindWindowW(WINDOW_CLASS, PCWSTR::null()) }.ok();
        if found.is_none() {
            std::thread::sleep(Duration::from_millis(50));
        }
        found
    });
    let Some(hwnd) = hwnd else {
        logger.event(&format!(
            "COPYDATA 전송 실패: file={file_name} reason=기존 창을 찾지 못함"
        ));
        return false;
    };

    let copy_data = COPYDATASTRUCT {
        dwData: COPYDATA_TAG,
        cbData: u32::try_from(byte_count).expect("COPYDATA length was capped"),
        lpData: payload.as_mut_ptr().cast(),
    };
    let mut receiver_result = 0usize;
    let sent = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_COPYDATA,
            WPARAM(0),
            LPARAM((&copy_data as *const COPYDATASTRUCT) as isize),
            SMTO_ABORTIFHUNG,
            5_000,
            Some(&mut receiver_result),
        )
    };
    if sent.0 == 0 || receiver_result == 0 {
        logger.event(&format!(
            "COPYDATA 전송 실패: file={file_name} reason=수신 거부 또는 시간 초과"
        ));
        false
    } else {
        logger.event(&format!("COPYDATA 전송 성공: file={file_name}"));
        true
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
    state: Rc<RefCell<AppState>>,
    settings: AppSettings,
    settings_store: SettingsStore<FileSettingsStorage>,
    settings_backups: Option<SettingsBackupStore<FileSettingsStorage>>,
    startup_settings_issue: Option<SettingsStoreIssue>,
    startup_backup_issue: Option<String>,
    settings_file_existed_at_launch: bool,
    settings_window: Option<SettingsWindow>,
    tray: Option<Tray>,
    hotkeys: Option<HotkeyCenter>,
    shortcut_recording: bool,
    keyboard_hook: Option<KeyboardHook>,
    mouse_hook: MouseHook,
    presenter: PresenterEffects,
    presenter_frame_pending: bool,
    pass_through_runtime_enabled: bool,
    pass_through_key_started_at: Option<f64>,
    pass_through_key_polling_disabled: bool,
    pressed_shortcuts: PressedShortcuts,
    radial_key_cap_state: RadialKeyCapState,
    overlays: Option<OverlayManager>,
    export: ExportController,
    palette: Option<PaletteManager>,
    radial_palette: Option<RadialPaletteManager>,
    text_entry: Option<TextEntryWindow>,
    help: Option<HelpManager>,
    music: Option<BreakMusicManager>,
    last_board_presentation: Option<BoardPresentation>,
    lesson_badge: Option<LessonBadgeManager>,
    break_timer: Option<BreakTimerManager>,
    pending_break_request: Option<i64>,
    zoom: Option<ZoomRuntime>,
    device_notification: Option<HDEVNOTIFY>,
    display_change: DisplayChangeState,
    display_stability_deadline_ms: Option<u64>,
    display_target_table: Vec<(u32, DisplayTargetKey)>,
    last_judged_uuids: Option<HashSet<String>>,
    last_auto_apply_at: Option<f64>,
    pending_display_apply: Option<usize>,
    expected_after_apply: Option<ExpectedDisplayConfiguration>,
    display_applier: DisplayApplier,
    pending_archived_display_keys: HashSet<String>,
    logger: Logger,
    shutting_down: bool,
}

struct ZoomRuntime {
    view: ZoomView,
    preview_original_frame: Option<CapturedFrame>,
    capture: ZoomCapture,
    _graphics: GraphicsDevice,
    monitor: Monitor,
    previous_foreground: HWND,
    wheel_accumulator: i32,
}

impl Drop for ZoomRuntime {
    fn drop(&mut self) {
        self.preview_original_frame = None;
        self.view.clear_frame();
    }
}

fn async_key_is_down(vk: u32) -> bool {
    i32::try_from(vk)
        .ok()
        .is_some_and(|vk| unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 })
}

fn mouse_buttons_pressed() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
        .into_iter()
        .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) as u16 & 0x8000 != 0 })
}

impl App {
    fn initialize(&mut self) -> Result<()> {
        self.music = Some(BreakMusicManager::new(self.hwnd, self.logger.clone())?);
        self.tray = Some(Tray::add(self.hwnd, self.hinstance, self.logger.clone())?);
        self.synchronize_launch_at_login("시작");
        if let Err(error) = recover_pending_display_configuration(&self.logger) {
            self.logger
                .event(&format!("시작 시 미확정 화면 구성 복구 실패: {error}"));
            if let Some(tray) = self.tray.as_ref() {
                tray.show_warning("화면 구성 복구 실패", &error);
            }
        }
        self.report_startup_settings_state();
        let hotkeys = HotkeyCenter::register(
            self.hwnd,
            self.tray.as_mut().expect("tray was just initialized"),
            self.logger.clone(),
            &self.settings,
        );
        self.hotkeys = Some(hotkeys);
        let input_config = InkInputConfig::from_core(&self.state.borrow(), &self.settings);
        match OverlayManager::new(
            self.hwnd,
            self.hinstance,
            input_config,
            self.state.clone(),
            &self.settings,
            self.logger.clone(),
        ) {
            Ok(overlays) => self.overlays = Some(overlays),
            Err(error) => {
                self.report_overlay_error("초기화", &error);
                return Err(error);
            }
        }
        let mut palette = PaletteManager::new(self.hwnd, self.hinstance, self.logger.clone())?;
        palette.refresh(&self.state.borrow(), &self.settings)?;
        self.palette = Some(palette);
        let mut radial_palette = RadialPaletteManager::new(
            self.hwnd,
            self.hinstance,
            &self.state.borrow(),
            &self.settings,
            self.logger.clone(),
        )?;
        radial_palette.refresh(&self.state.borrow(), &self.settings, "초기화")?;
        self.radial_palette = Some(radial_palette);
        let mut help = HelpManager::new(self.hwnd, self.hinstance, self.logger.clone())?;
        help.show_first_run_if_needed(&self.state.borrow(), &self.settings);
        self.help = Some(help);
        self.lesson_badge = Some(LessonBadgeManager::new(
            self.hwnd,
            self.hinstance,
            &self.settings,
            self.logger.clone(),
        )?);
        self.break_timer = Some(BreakTimerManager::new(
            self.hwnd,
            self.hinstance,
            &self.settings,
            self.logger.clone(),
        )?);
        self.register_display_notifications()?;
        self.initialize_display_change_state();
        Ok(())
    }

    fn report_startup_settings_state(&mut self) {
        if let Some(issue) = self.startup_settings_issue.take() {
            let preserved = issue
                .detail
                .contains("원본은 별도 손상 설정 위치에 보존했습니다")
                && self.settings_store.storage().corrupt_path().exists();
            self.logger.event(&format!(
                "설정 로드: 출처={} detail={}",
                if preserved {
                    "손상 원본 보존"
                } else {
                    "읽기 실패 후 기본값"
                },
                issue.detail
            ));
            let preservation = if preserved {
                format!(
                    "손상 원본은 {}에 보존했습니다. ",
                    self.settings_store.storage().corrupt_path().display()
                )
            } else {
                String::new()
            };
            if let Some(tray) = self.tray.as_ref() {
                tray.show_warning("설정 로드 경고", &format!("{preservation}{}", issue.detail));
            }
        } else {
            self.logger.event(if self.settings_file_existed_at_launch {
                "설정 로드: 출처=파일"
            } else {
                "설정 로드: 출처=기본값 (settings.json 없음)"
            });
        }

        if let Some(detail) = self.startup_backup_issue.take() {
            self.logger
                .event(&format!("설정 백업 저장소 로드 실패: {detail}"));
            if let Some(tray) = self.tray.as_ref() {
                tray.show_warning("설정 백업 경고", &detail);
            }
        }
    }

    fn runtime_shortcut_warnings(&self) -> BTreeMap<String, String> {
        self.hotkeys
            .as_ref()
            .map(|hotkeys| hotkeys.runtime_warnings().clone())
            .unwrap_or_default()
    }

    fn update_settings<F>(&mut self, source: &str, change: F) -> bool
    where
        F: FnOnce(&mut AppSettings),
    {
        let previous = self.settings.clone();
        match self.settings_store.update(change) {
            Ok(()) => {
                let current = self.settings_store.settings().clone();
                self.settings = current.clone();
                self.report_settings_store_issue();
                let changed = changed_settings_groups(&previous, &current);
                if changed.is_empty() {
                    self.logger
                        .event(&format!("설정 갱신: source={source} changed=없음"));
                    return false;
                }
                let names = changed
                    .iter()
                    .map(|group| group.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                self.logger
                    .event(&format!("설정 갱신: source={source} changed={names}"));
                self.settings_changed(&previous, &current, &changed);
                let warnings = self.runtime_shortcut_warnings();
                if let Some(window) = self.settings_window.as_mut() {
                    window.refresh_external(&current, &warnings, source);
                }
                true
            }
            Err(error) => {
                let message = format!("설정 갱신 검증 거부: source={source} error={error}");
                self.logger.event(&message);
                if let Some(tray) = self.tray.as_ref() {
                    tray.show_warning("설정 변경 거부", &message);
                }
                false
            }
        }
    }

    fn settings_changed(
        &mut self,
        previous: &AppSettings,
        current: &AppSettings,
        changed: &[SettingsGroup],
    ) {
        let mut targets = Vec::new();

        if previous.general.launch_at_login != current.general.launch_at_login {
            self.synchronize_launch_at_login("설정 변경");
            targets.push("launch-at-login-registry");
        }

        if previous.general.auto_archive_enabled != current.general.auto_archive_enabled
            || previous.general.archive_folder_path != current.general.archive_folder_path
            || previous.general.display_disconnect_policy
                != current.general.display_disconnect_policy
            || previous
                .general
                .automatically_restores_disconnected_displays
                != current.general.automatically_restores_disconnected_displays
        {
            if let Some(overlays) = self.overlays.as_mut() {
                overlays.update_archive_settings(current);
            }
            targets.push("archive-policy+paths");
        }

        if previous.shortcuts != current.shortcuts {
            if self.shortcut_recording {
                targets.push("shortcut-registration-deferred");
            } else {
                if let Some(hotkeys) = self.hotkeys.as_mut() {
                    hotkeys.shutdown();
                }
                if let Some(tray) = self.tray.as_mut() {
                    self.hotkeys = Some(HotkeyCenter::register(
                        self.hwnd,
                        tray,
                        self.logger.clone(),
                        current,
                    ));
                }
                // 훅은 설치 시점의 바인딩을 읽으므로 단축키가 바뀌면 내렸다가 현재 스코프로 다시 올린다.
                if self.keyboard_hook.is_some() {
                    self.uninstall_keyboard_hook_for_shutdown();
                    self.synchronize_keyboard_hook();
                }
                targets.push("global-hotkeys+keyboard-hook");
            }
        }

        let ink_input_changed =
            previous.tools != current.tools || previous.eraser != current.eraser;
        if ink_input_changed {
            let input_config = InkInputConfig::from_core(&self.state.borrow(), current);
            let error = self
                .overlays
                .as_mut()
                .and_then(|overlays| overlays.update_input_context(input_config).err());
            if let Some(error) = error {
                self.report_overlay_error("설정 변경 입력 구성", &error);
            }
            targets.push("ink-input+eraser+laser-defaults");
        }

        let palette_changed = previous.tools != current.tools || previous.eraser != current.eraser;
        if palette_changed {
            self.refresh_palette("설정 변경");
            self.refresh_radial_palette("설정 변경");
            targets.push("linear+radial-palette");
        }

        let lesson_badge_changed = previous.timer.lesson_presets != current.timer.lesson_presets
            || previous.timer.lesson_default_minutes != current.timer.lesson_default_minutes
            || previous.timer.break_default_minutes != current.timer.break_default_minutes
            || previous.timer.notification_sound != current.timer.notification_sound
            || previous.timer.auto_start_break != current.timer.auto_start_break
            || previous.timer.lesson_badge_size != current.timer.lesson_badge_size
            || previous.timer.lesson_badge_opacity != current.timer.lesson_badge_opacity
            || previous.timer.lesson_display_mode != current.timer.lesson_display_mode;
        if lesson_badge_changed {
            let error = self
                .lesson_badge
                .as_mut()
                .and_then(|badge| badge.apply_settings(current).err());
            if let Some(error) = error {
                self.report_lesson_badge_error("설정 변경", &error);
            }
            targets.push("lesson-badge");
        }

        let break_music_changed = previous.timer.playlist_urls != current.timer.playlist_urls
            || previous.timer.default_playlist_index != current.timer.default_playlist_index
            || previous.timer.music_enabled != current.timer.music_enabled
            || previous.timer.music_browser != current.timer.music_browser
            || previous.timer.fade_enabled != current.timer.fade_enabled
            || previous.timer.fade_duration != current.timer.fade_duration;
        if break_music_changed {
            if let Some(music) = self.music.as_mut() {
                music.settings_changed(previous, current);
            }
            targets.push("break-music");
        }

        if break_music_changed
            || previous.timer.break_message != current.timer.break_message
            || previous.timer.break_background_color != current.timer.break_background_color
        {
            self.refresh_break_settings("설정 변경");
            targets.push("break-screen");
        }

        if previous.board != current.board {
            let error = self
                .overlays
                .as_mut()
                .and_then(|overlays| overlays.refresh_board(current).err());
            if let Some(error) = error {
                self.report_overlay_error("설정 변경 보드", &error);
            }
            targets.push("board");
        }

        if previous.zoom != current.zoom {
            // 활성 줌 컨트롤러는 아직 앱 크레이트에 없다. 다음 줌 시작은 소유자 사본의 새
            // 기본값을 읽으며, W7 컨트롤러가 연결되면 이 분기에 런타임 재구성을 추가한다.
            targets.push("zoom-defaults");
        }

        if !changed.is_empty() && self.help.as_ref().is_some_and(HelpManager::is_visible) {
            self.refresh_help("설정 변경");
            targets.push("visible-help");
        }

        self.logger.event(&format!(
            "설정 변경 통지: targets={}",
            if targets.is_empty() {
                "owner-copy-only".to_owned()
            } else {
                targets.join(",")
            }
        ));
    }

    fn synchronize_launch_at_login(&mut self, source: &str) {
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                let detail = format!("현재 실행 파일 경로 조회 실패: {error}");
                self.report_launch_at_login_failure(source, &detail);
                return;
            }
        };
        let mut registry = WindowsRunRegistry;
        match launch_at_login::synchronize(
            &mut registry,
            self.settings.general.launch_at_login,
            &executable,
        ) {
            Ok(outcome) => self.logger.event(&format!(
                "자동 실행 동기화 성공: source={source} desired={} result={}",
                self.settings.general.launch_at_login,
                match outcome {
                    SyncOutcome::Unchanged => "unchanged",
                    SyncOutcome::Enabled => "enabled",
                    SyncOutcome::Disabled => "disabled",
                    SyncOutcome::UpdatedPath => "updated-path",
                }
            )),
            Err(error) => self.report_launch_at_login_failure(source, &error),
        }
    }

    fn report_launch_at_login_failure(&self, source: &str, detail: &str) {
        self.logger.event(&format!(
            "자동 실행 동기화 실패: source={source} desired={} error={detail}",
            self.settings.general.launch_at_login
        ));
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning(
                "자동 실행 설정 실패",
                &format!("로그인 시 자동 실행을 변경하지 못했습니다: {detail}"),
            );
        }
    }

    fn show_settings(&mut self, tab: SettingsTab, source: &str) {
        let warnings = self.runtime_shortcut_warnings();
        if self.settings_window.is_none() {
            match SettingsWindow::new(
                self.hwnd,
                self.hinstance,
                &self.settings,
                &warnings,
                self.logger.clone(),
            ) {
                Ok(window) => self.settings_window = Some(window),
                Err(error) => {
                    let message = format!("설정 창 생성 실패: source={source} error={error}");
                    self.logger.event(&message);
                    if let Some(tray) = self.tray.as_ref() {
                        tray.show_warning("설정 창을 열 수 없습니다", &message);
                    }
                    return;
                }
            }
        }
        let availability = self.display_apply_availability();
        if let Some(window) = self.settings_window.as_mut() {
            window.show(tab, &self.settings, &warnings, availability);
        }
    }

    fn display_apply_availability(&self) -> DisplayApplyAvailability {
        match self.display_applier.state() {
            DisplayApplyState::Applying => {
                return DisplayApplyAvailability::unavailable("다른 화면 구성을 적용하는 중입니다")
            }
            DisplayApplyState::AwaitingConfirmation => {
                return DisplayApplyAvailability::unavailable(
                    "현재 화면 구성을 먼저 유지하거나 되돌리세요",
                )
            }
            DisplayApplyState::Idle => {}
        }
        match is_console_session() {
            Ok(true) => DisplayApplyAvailability::available(),
            Ok(false) => DisplayApplyAvailability::unavailable(
                "원격 데스크톱(RDP)·비콘솔 세션에서는 적용할 수 없습니다",
            ),
            Err(error) => DisplayApplyAvailability::unavailable(format!(
                "현재 세션을 확인할 수 없어 적용을 비활성화했습니다: {error}"
            )),
        }
    }

    #[allow(dead_code)]
    fn show_display_settings(&mut self, source: &str) {
        self.show_settings(SettingsTab::Display, source);
    }

    fn handle_settings_change(&mut self, request: SettingsChangeRequest) {
        let field_id = request.field_id;
        let mut candidate = self.settings.clone();
        let validation = apply_settings_window_change(&mut candidate, &request).and_then(|()| {
            SettingsValidator::validate(&candidate).map_err(|error| error.to_string())
        });
        if let Err(reason) = validation {
            let warnings = self.runtime_shortcut_warnings();
            if let Some(window) = self.settings_window.as_mut() {
                window.reject_change(field_id, &reason, &self.settings, &warnings);
            }
            return;
        }

        let request_for_update = request.clone();
        self.update_settings("settings-window", move |settings| {
            let _ = apply_settings_window_change(settings, &request_for_update);
        });
        let warnings = self.runtime_shortcut_warnings();
        if let Some(window) = self.settings_window.as_mut() {
            window.accept_change(field_id, &self.settings, &warnings);
        }
    }

    fn handle_settings_action(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::BeginShortcutRecording { key } => self.begin_shortcut_recording(&key),
            SettingsAction::CancelShortcutRecording { key } => {
                self.logger
                    .event(&format!("단축키 기록 종료: id={key} result=취소"));
                self.finish_shortcut_recording();
            }
            SettingsAction::RecordShortcut { key, portable } => {
                let result = portable.clone().unwrap_or_else(|| "해제".to_owned());
                let update_key = key.clone();
                self.update_settings("shortcut-recorder", move |settings| {
                    if let Some(portable) = portable {
                        settings.shortcuts.overrides.insert(update_key, portable);
                    } else {
                        settings.shortcuts.overrides.remove(&update_key);
                    }
                });
                self.logger
                    .event(&format!("단축키 기록 종료: id={key} result={result}"));
                self.finish_shortcut_recording();
            }
            SettingsAction::ResetShortcuts => {
                self.update_settings("shortcut-reset", |settings| {
                    settings.shortcuts.overrides.clear();
                });
                self.logger.event("단축키 전체 기본값 복원");
            }
            SettingsAction::SetAppModifier(value) => {
                self.update_settings("shortcut-app-modifier", move |settings| {
                    settings.shortcuts.app_modifier = value;
                });
            }
            SettingsAction::Export {
                bundles,
                include_computer_specific,
            } => self.export_settings(bundles, include_computer_specific),
            SettingsAction::Import => self.import_settings(),
            SettingsAction::ApplyImport { document, bundles } => {
                self.apply_imported_settings(document, bundles)
            }
            SettingsAction::ResetAll => self.reset_all_settings(),
            SettingsAction::RestoreBackup => {
                self.logger.event("이전 설정 복원 요청");
                self.restore_latest_backup();
            }
            SettingsAction::DisplayMutation(request) => self.handle_display_mutation(request),
            SettingsAction::DisplayCommand(command) => self.handle_display_command(command),
        }
    }

    fn handle_display_mutation(&mut self, request: DisplayMutationRequest) {
        let mut candidate = self.settings.clone();
        candidate.display = request.display.clone();
        if let Err(error) = SettingsValidator::validate(&candidate) {
            let reason = error.to_string();
            self.logger.event(&format!(
                "화면 탭 검증 거부: action={} reason={reason}",
                display_mutation_name(&request.kind)
            ));
            let warnings = self.runtime_shortcut_warnings();
            if let Some(window) = self.settings_window.as_mut() {
                window.reject_display_change(&reason, &self.settings, &warnings);
            }
            return;
        }

        let previous = self.settings.display.clone();
        let replacement = request.display.clone();
        self.update_settings("display-tab", move |settings| {
            settings.display = replacement;
        });
        self.log_display_mutation(&request.kind, &previous, &self.settings.display);
        let warnings = self.runtime_shortcut_warnings();
        if let Some(window) = self.settings_window.as_mut() {
            window.accept_display_change(&self.settings, &warnings);
        }
    }

    fn log_display_mutation(
        &self,
        kind: &DisplayMutationKind,
        previous: &edutools_core::display_preset::DisplaySettings,
        current: &edutools_core::display_preset::DisplaySettings,
    ) {
        let detail = match kind {
            DisplayMutationKind::AutoSwitch => format!("자동 전환={}", current.auto_switch_enabled),
            DisplayMutationKind::Rename { preset } => format!(
                "이름 변경 index={} oldHash={} newHash={}",
                preset,
                previous
                    .presets
                    .get(*preset)
                    .map(|value| hash_prefix8(&value.name))
                    .unwrap_or_else(|| "missing".to_owned()),
                current
                    .presets
                    .get(*preset)
                    .map(|value| hash_prefix8(&value.name))
                    .unwrap_or_else(|| "missing".to_owned())
            ),
            DisplayMutationKind::Duplicate { source, inserted } => format!(
                "복제 source={} inserted={} presetHash={}",
                source,
                inserted,
                current
                    .presets
                    .get(*inserted)
                    .map(|value| hash_prefix8(&value.name))
                    .unwrap_or_else(|| "missing".to_owned())
            ),
            DisplayMutationKind::Delete { preset } => format!(
                "삭제 index={} presetHash={}",
                preset,
                previous
                    .presets
                    .get(*preset)
                    .map(|value| hash_prefix8(&value.name))
                    .unwrap_or_else(|| "missing".to_owned())
            ),
            DisplayMutationKind::MovePreset {
                source,
                destination,
            } => format!("순서 이동 source={source} destination={destination}"),
            DisplayMutationKind::Edit(edit) => display_edit_log(edit),
            DisplayMutationKind::Capture { preset, warnings } => format!(
                "캡처 index={} presetHash={} warnings={}",
                preset,
                current
                    .presets
                    .get(*preset)
                    .map(|value| hash_prefix8(&value.name))
                    .unwrap_or_else(|| "missing".to_owned()),
                warnings.len()
            ),
        };
        self.logger.event(&format!("화면 탭 변경: {detail}"));
    }

    fn handle_display_command(&mut self, command: DisplayCommand) {
        match command {
            DisplayCommand::ShowPlan(index) => {
                let Some(preset) = self.settings.display.presets.get(index).cloned() else {
                    let warnings = self.runtime_shortcut_warnings();
                    if let Some(window) = self.settings_window.as_mut() {
                        window.reject_display_change(
                            "선택한 프리셋이 없습니다",
                            &self.settings,
                            &warnings,
                        );
                    }
                    return;
                };
                match current_display_configuration() {
                    Ok(configuration) => {
                        let plan = DisplayPlanner::plan(&preset, &configuration.online);
                        let applicable = plan.is_applicable();
                        let summary = plan.summary();
                        self.logger.event(&format!(
                            "화면 탭 계획 보기: presetHash={} applicable={} summary={}",
                            hash_prefix8(&preset.name),
                            applicable,
                            summary.replace('\n', " | ")
                        ));
                        if let Some(window) = self.settings_window.as_ref() {
                            window.show_display_message(
                                if applicable {
                                    "화면 구성 계획"
                                } else {
                                    "화면 구성 계획 · 적용 불가"
                                },
                                &summary,
                            );
                        }
                    }
                    Err(error) => {
                        let reason = format!("계획을 계산할 수 없습니다: {error}");
                        self.logger.event(&reason);
                        let warnings = self.runtime_shortcut_warnings();
                        if let Some(window) = self.settings_window.as_mut() {
                            window.reject_display_change(&reason, &self.settings, &warnings);
                        }
                    }
                }
            }
            DisplayCommand::Capture => self.capture_display_preset(),
            DisplayCommand::Apply(index) => {
                self.apply_display_preset(index, DisplayApplySource::Manual);
                let availability = self.display_apply_availability();
                if let Some(window) = self.settings_window.as_mut() {
                    window.set_display_apply_availability(availability);
                }
            }
        }
    }

    fn capture_display_preset(&mut self) {
        if self.settings.display.presets.len() >= DisplayPresetDefaults::MAXIMUM_PRESET_COUNT {
            let warnings = self.runtime_shortcut_warnings();
            if let Some(window) = self.settings_window.as_mut() {
                window.reject_display_change(
                    "프리셋 최대 개수에 도달했습니다",
                    &self.settings,
                    &warnings,
                );
            }
            return;
        }
        let configuration = match current_display_configuration() {
            Ok(configuration) => configuration,
            Err(error) => {
                let reason = format!("현재 구성을 캡처할 수 없습니다: {error}");
                self.logger.event(&reason);
                let warnings = self.runtime_shortcut_warnings();
                if let Some(window) = self.settings_window.as_mut() {
                    window.reject_display_change(&reason, &self.settings, &warnings);
                }
                return;
            }
        };
        let name = unique_name(
            "현재 구성",
            self.settings
                .display
                .presets
                .iter()
                .map(|preset| preset.name.as_str()),
        );
        let capture =
            match DisplayPresetCapture::current(&configuration.online, &name, new_preset_id()) {
                Ok(capture) => capture,
                Err(error) => {
                    let reason = error.to_string();
                    self.logger.event(&format!("화면 탭 캡처 실패: {reason}"));
                    let warnings = self.runtime_shortcut_warnings();
                    if let Some(window) = self.settings_window.as_mut() {
                        window.reject_display_change(&reason, &self.settings, &warnings);
                    }
                    return;
                }
            };
        let warnings = capture.warnings.clone();
        let mut display = self.settings.display.clone();
        let index = display.presets.len();
        display.presets.push(capture.preset);
        self.handle_display_mutation(DisplayMutationRequest {
            display,
            kind: DisplayMutationKind::Capture {
                preset: index,
                warnings: warnings.clone(),
            },
        });
        if let Some(window) = self.settings_window.as_mut() {
            window.select_display_preset(index);
        }
        if !warnings.is_empty() {
            if let Some(window) = self.settings_window.as_ref() {
                window.show_display_message("현재 구성 캡처", &warnings.join("\n"));
            }
        }
    }

    fn begin_shortcut_recording(&mut self, key: &str) {
        if !self.shortcut_recording {
            self.shortcut_recording = true;
            if let Some(hotkeys) = self.hotkeys.as_mut() {
                hotkeys.shutdown();
            }
            self.uninstall_keyboard_hook_for_shutdown();
        }
        self.logger
            .event(&format!("단축키 기록 시작: id={key} hooks=paused"));
    }

    fn finish_shortcut_recording(&mut self) {
        if !self.shortcut_recording {
            return;
        }
        self.shortcut_recording = false;
        if let Some(tray) = self.tray.as_mut() {
            self.hotkeys = Some(HotkeyCenter::register(
                self.hwnd,
                tray,
                self.logger.clone(),
                &self.settings,
            ));
        }
        self.synchronize_keyboard_hook();
        let warnings = self.runtime_shortcut_warnings();
        if let Some(window) = self.settings_window.as_mut() {
            window.update_runtime_shortcut_warnings(&warnings);
        }
        self.logger.event("단축키 기록 훅·전역 핫키 재등록 완료");
    }

    fn settings_dialog_owner(&self) -> HWND {
        self.settings_window
            .as_ref()
            .map(SettingsWindow::hwnd)
            .unwrap_or(self.hwnd)
    }

    fn settings_message(
        &self,
        title: &str,
        message: &str,
        style: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
    ) {
        let title = title
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let message = message
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        unsafe {
            MessageBoxW(
                Some(self.settings_dialog_owner()),
                PCWSTR(message.as_ptr()),
                PCWSTR(title.as_ptr()),
                style,
            )
        };
    }

    fn export_settings(
        &mut self,
        bundles: std::collections::BTreeSet<SettingsBundle>,
        include_computer_specific: bool,
    ) {
        if bundles.contains(&SettingsBundle::Timer) && !self.settings.timer.playlist_urls.is_empty()
        {
            let answer = unsafe {
                MessageBoxW(
                    Some(self.settings_dialog_owner()),
                    w!("설정 파일을 공유하면 YouTube Music 재생목록 URL도 함께 전달됩니다."),
                    w!("재생목록 주소가 포함됩니다"),
                    MB_YESNO | MB_ICONWARNING,
                )
            };
            if answer != IDYES {
                self.logger.event("설정 내보내기 취소: 재생목록 주소 경고");
                return;
            }
        }
        let path = match choose_settings_export_path(self.settings_dialog_owner()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.logger.event("설정 내보내기 취소: 저장 대화상자");
                return;
            }
            Err(error) => {
                self.logger.event("설정 내보내기 실패: 저장 대화상자");
                self.settings_message(
                    "설정 내보내기 실패",
                    &error.to_string(),
                    MB_OK | MB_ICONERROR,
                );
                return;
            }
        };
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "EduTools-settings.pansetting".to_owned());
        let result = SettingsTransfer::export_document(
            &self.settings,
            &bundles,
            include_computer_specific,
            env!("CARGO_PKG_VERSION"),
            "Windows",
        )
        .and_then(|document| SettingsTransfer::encode(&document))
        .map_err(|error| error.to_string())
        .and_then(|data| {
            FileSettingsStorage::atomic_write(&path, &data).map_err(|error| error.to_string())
        });
        match result {
            Ok(()) => {
                let bundle_names = bundles
                    .iter()
                    .map(|bundle| settings_bundle_name(*bundle))
                    .collect::<Vec<_>>()
                    .join(",");
                self.logger.event(&format!(
                    "설정 내보내기 성공: bundles={bundle_names} computerSpecific={include_computer_specific} file={file_name}"
                ));
            }
            Err(error) => {
                self.logger
                    .event(&format!("설정 내보내기 실패: file={file_name}"));
                self.settings_message("설정 내보내기 실패", &error, MB_OK | MB_ICONERROR);
            }
        }
    }

    fn import_settings(&mut self) {
        let path = match choose_settings_import_path(self.settings_dialog_owner()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.logger.event("설정 가져오기 취소: 열기 대화상자");
                return;
            }
            Err(error) => {
                self.logger.event("설정 가져오기 실패: 열기 대화상자");
                self.settings_message(
                    "설정 가져오기 실패",
                    &error.to_string(),
                    MB_OK | MB_ICONERROR,
                );
                return;
            }
        };
        self.open_import_preview(&path, "열기 대화상자");
    }

    fn open_import_preview(&mut self, path: &Path, source: &str) {
        let file_name = file_name_from_path(path);
        let path = match startup::validate_import_path(path) {
            Ok(path) => path,
            Err(reason) => {
                self.logger.event(&format!(
                    "설정 가져오기 경로 거부: source={source} file={file_name} reason={reason}"
                ));
                self.settings_message("설정 가져오기 실패", reason, MB_OK | MB_ICONERROR);
                return;
            }
        };
        self.show_settings(SettingsTab::General, source);
        let document = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|data| SettingsTransfer::decode(&data).map_err(|error| error.to_string()));
        let document = match document {
            Ok(document) => document,
            Err(error) => {
                self.logger
                    .event(&format!("설정 가져오기 디코드 실패: file={file_name}"));
                self.settings_message("설정 가져오기 실패", &error, MB_OK | MB_ICONERROR);
                return;
            }
        };
        let changes =
            SettingsTransfer::changes(&self.settings, &document, document.included_bundles.iter())
                .map(|changes| changes.len())
                .unwrap_or(0);
        self.logger.event(&format!(
            "설정 가져오기 디코드 성공: file={file_name} includedBundles={} changes={changes}",
            document.included_bundles.len()
        ));
        if let Some(window) = self.settings_window.as_mut() {
            if let Err(error) = window.show_import_preview(&self.settings, document) {
                self.logger.event("설정 가져오기 미리보기 창 생성 실패");
                self.settings_message(
                    "설정 가져오기 실패",
                    &error.to_string(),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
    }

    fn handle_copydata(&mut self, lparam: LPARAM) -> bool {
        let copy_data = lparam.0 as *const COPYDATASTRUCT;
        if copy_data.is_null() {
            self.logger
                .event("COPYDATA 수신 거부: reason=구조체 포인터가 null임");
            return false;
        }
        let copy_data = unsafe { &*copy_data };
        if copy_data.dwData != COPYDATA_TAG {
            self.logger
                .event("COPYDATA 수신 거부: reason=알 수 없는 dwData");
            return false;
        }
        let byte_count = copy_data.cbData as usize;
        if copy_data.lpData.is_null()
            || byte_count < 2
            || byte_count > MAX_COPYDATA_BYTES
            || byte_count % 2 != 0
        {
            self.logger
                .event("COPYDATA 수신 거부: reason=페이로드 포인터 또는 길이 오류");
            return false;
        }
        let units =
            unsafe { std::slice::from_raw_parts(copy_data.lpData.cast::<u16>(), byte_count / 2) };
        let path = match startup::validate_copydata_utf16(units, byte_count) {
            Ok(path) => path,
            Err(reason) => {
                self.logger
                    .event(&format!("COPYDATA 수신 거부: reason={reason}"));
                return false;
            }
        };
        if !path.is_absolute() {
            self.logger.event(&format!(
                "COPYDATA 수신 거부: file={} reason=절대 경로가 아님",
                file_name_from_path(&path)
            ));
            return false;
        }
        let path = match startup::validate_import_path(&path) {
            Ok(path) => path,
            Err(reason) => {
                self.logger.event(&format!(
                    "COPYDATA 수신 거부: file={} reason={reason}",
                    file_name_from_path(&path)
                ));
                return false;
            }
        };
        self.logger.event(&format!(
            "COPYDATA 수신 승인: file={}",
            file_name_from_path(&path)
        ));
        self.open_import_preview(&path, "파일 연결");
        true
    }

    fn apply_imported_settings(
        &mut self,
        document: edutools_core::settings_transfer::SettingsDocument,
        bundles: std::collections::BTreeSet<SettingsBundle>,
    ) {
        let imported = match SettingsTransfer::applied_settings(&self.settings, &document, &bundles)
        {
            Ok(imported) => imported,
            Err(error) => {
                self.logger
                    .event("설정 가져오기 적용 거부: applied_settings 검증 실패");
                self.settings_message(
                    "설정을 적용할 수 없습니다",
                    &error.to_string(),
                    MB_OK | MB_ICONERROR,
                );
                return;
            }
        };
        if !self.backup_current() {
            return;
        }
        let changes = SettingsTransfer::changes(&self.settings, &document, &bundles)
            .map(|changes| changes.len())
            .unwrap_or(0);
        let candidate = imported.clone();
        self.update_settings("settings-import", move |settings| *settings = candidate);
        if self.settings == imported {
            if let Some(window) = self.settings_window.as_mut() {
                window.close_import_preview();
            }
            self.logger.event(&format!(
                "설정 가져오기 적용 성공: bundles={} changes={changes} backup=true",
                bundles.len()
            ));
        } else {
            self.logger.event("설정 가져오기 적용 실패: 저장 경로 거부");
        }
    }

    fn reset_all_settings(&mut self) {
        if !self.backup_current() {
            return;
        }
        let defaults = AppSettings::default();
        self.update_settings("settings-reset", move |settings| *settings = defaults);
        self.logger.event("모든 설정 초기화 완료: backup=true");
    }

    fn report_settings_store_issue(&mut self) {
        let Some(issue) = self.settings_store.take_issue() else {
            return;
        };
        let title = match issue.kind {
            SettingsStoreIssueKind::Load => "설정 로드 경고",
            SettingsStoreIssueKind::Save => "설정 저장 경고",
        };
        self.logger.event(&format!("{title}: {}", issue.detail));
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning(title, &issue.detail);
        }
    }

    fn refresh_runtime_state(&mut self, reason: &str) {
        let input_config = InkInputConfig::from_core(&self.state.borrow(), &self.settings);
        let error = self
            .overlays
            .as_mut()
            .and_then(|overlays| overlays.update_input_context(input_config).err());
        if let Some(error) = error {
            self.report_overlay_error("상태 변경 입력 구성", &error);
        }
        self.refresh_palette(reason);
        self.refresh_radial_palette(reason);
        self.refresh_help(reason);
    }

    /// 120b의 가져오기/일괄 변경 직전에 호출하는 백업 API.
    #[allow(dead_code)]
    fn backup_current(&mut self) -> bool {
        let result = self
            .settings_backups
            .as_mut()
            .ok_or_else(|| "설정 백업 저장소를 사용할 수 없습니다".to_owned())
            .and_then(|backups| {
                backups
                    .append(&self.settings)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(()) => {
                self.logger.event("설정 백업 생성 성공");
                true
            }
            Err(error) => {
                self.logger.event(&format!("설정 백업 생성 실패: {error}"));
                if let Some(tray) = self.tray.as_ref() {
                    tray.show_warning("설정 백업 실패", &error);
                }
                false
            }
        }
    }

    /// 120b UI가 연결할 최신 백업 복원 API.
    #[allow(dead_code)]
    fn restore_latest_backup(&mut self) -> bool {
        let restored = self
            .settings_backups
            .as_mut()
            .ok_or_else(|| "설정 백업 저장소를 사용할 수 없습니다".to_owned())
            .and_then(|backups| backups.pop_latest().map_err(|error| error.to_string()));
        match restored {
            Ok(Some(settings)) => {
                let restored = self
                    .update_settings("latest-backup-restore", move |current| *current = settings);
                self.logger.event(&format!(
                    "설정 백업 복원 {}",
                    if restored {
                        "성공"
                    } else {
                        "변경 없음 또는 실패"
                    }
                ));
                restored
            }
            Ok(None) => {
                self.logger.event("설정 백업 복원 생략: 백업 없음");
                self.settings_message(
                    "이전 설정 복원",
                    "되돌릴 이전 설정이 없습니다.",
                    MB_OK | MB_ICONINFORMATION,
                );
                false
            }
            Err(error) => {
                self.logger.event(&format!("설정 백업 복원 실패: {error}"));
                if let Some(tray) = self.tray.as_ref() {
                    tray.show_warning("설정 백업 복원 실패", &error);
                }
                false
            }
        }
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
        if definition.command == ShortcutCommand::ShowHelp {
            let Some(binding) = definition.binding(ShortcutScope::Global) else {
                self.logger.event("전역 도움말 정의에 global binding 없음");
                return;
            };
            let Some(vk) = keymap::token_to_vk(&binding.key) else {
                self.logger.event(&format!(
                    "전역 도움말 키 폴링용 VK 변환 실패: {}",
                    binding.key
                ));
                return;
            };
            let now_ms = monotonic_millis();
            if let Some(help) = self.help.as_mut() {
                help.shortcut_did_press(
                    now_ms as f64 / 1_000.0,
                    &self.state.borrow(),
                    &self.settings,
                    "전역 도움말 키 눌림",
                );
                if !help.start_global_release_poll(vk, now_ms) {
                    help.shortcut_did_release(
                        now_ms as f64 / 1_000.0,
                        &self.state.borrow(),
                        &self.settings,
                        "전역 키 폴링 시작 실패 강제 해제",
                    );
                }
            }
        } else if definition.command == ShortcutCommand::ToggleAnnotationMode {
            self.logger.event("전역 키 annotation.toggle 눌림");
            self.toggle_annotation("전역 키");
        } else if definition.command == ShortcutCommand::ToggleZoom {
            self.logger.event("전역 키 zoom.toggle 눌림");
            self.apply_effects(vec![Effect::ToggleZoom], "전역 키 zoom.toggle");
        } else if let ShortcutCommand::ApplyDisplayPreset(index) = &definition.command {
            self.logger.event(&format!(
                "전역 키 {} 눌림: 화면 구성 index={index}",
                definition.id
            ));
            self.apply_display_preset(*index, DisplayApplySource::Manual);
        } else if matches!(
            definition.command,
            ShortcutCommand::ToggleSpotlight
                | ShortcutCommand::ToggleCursorHighlight
                | ShortcutCommand::ToggleKeystrokeDisplay
        ) {
            let context = DispatchContext {
                scope: ShortcutScope::Global,
                spotlight_adjusted_width: self.presenter.spotlight_enabled(),
                now: monotonic_seconds(),
                ..DispatchContext::default()
            };
            let result = dispatch(
                &definition.command,
                &mut self.state.borrow_mut(),
                &self.settings,
                context,
            );
            match result {
                Ok(effects) => self.apply_effects(effects, &format!("전역 키 {}", definition.id)),
                Err(error) => self
                    .logger
                    .event(&format!("{} core dispatch 실패: {error}", definition.id)),
            }
        } else if definition.command == ShortcutCommand::ToggleLessonTimer
            || definition.command == ShortcutCommand::ShowBreakTimerSetup
        {
            self.logger
                .event(&format!("전역 키 {} 눌림", definition.id));
            let context = DispatchContext {
                scope: ShortcutScope::Global,
                ..DispatchContext::default()
            };
            let result = {
                let mut state = self.state.borrow_mut();
                dispatch(&definition.command, &mut state, &self.settings, context)
            };
            match result {
                Ok(effects) => self.apply_effects(effects, &format!("전역 키 {}", definition.id)),
                Err(error) => self
                    .logger
                    .event(&format!("{} core dispatch 실패: {error}", definition.id)),
            }
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
        let result = dispatch(
            &ShortcutCommand::ToggleAnnotationMode,
            &mut self.state.borrow_mut(),
            &self.settings,
            context,
        );
        match result {
            Ok(effects) => self.apply_effects(effects, source),
            Err(error) => self.logger.event(&format!(
                "annotation.toggle core dispatch 실패 ({source}): {error}"
            )),
        }
    }

    fn handle_mode_key(&mut self, event: HookKeyEvent) {
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
        if !event.is_key_up
            && event.scope == HookScope::Zoom
            && zoom_control_for_shortcut(&definition.command).is_some_and(|control| {
                zoom_gesture_control_locked(self.zoom_gesture_in_progress(), control)
            })
        {
            self.logger.event(&format!(
                "줌 제스처 잠금으로 키 거부: key={} definition={}",
                event.key_code, definition.id
            ));
            return;
        }
        self.logger.event(&format!(
            "모드 키 {:?} {}: physicalKey={:#x} 정의={} 소비={}",
            event.scope,
            if event.is_key_up { "up" } else { "down" },
            event.key_code,
            definition.id,
            event.consumed
        ));
        if !event.consumed {
            return;
        }

        let display_id = match event.scope {
            HookScope::Zoom => self
                .state
                .borrow()
                .zoom_session()
                .map(|session| session.display_id.clone()),
            HookScope::Annotation => match self
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
            },
        };
        let board_display_id = self
            .state
            .borrow()
            .presented_board()
            .map(|board| board.display_id.clone());
        let context = DispatchContext {
            scope: event.scope.shortcut_scope(),
            display_id: display_id.as_deref(),
            board_display_id: board_display_id.as_deref(),
            has_selection: display_id.as_deref().is_some_and(|display_id| {
                !self
                    .state
                    .borrow()
                    .selected_stroke_ids(display_id)
                    .is_empty()
            }),
            help_visible: self.help.as_ref().is_some_and(HelpManager::is_visible),
            spotlight_adjusted_width: self.presenter.spotlight_enabled(),
            now: monotonic_seconds(),
            pass_through_monitor_available: true,
            ..DispatchContext::default()
        };
        let dispatch = if event.is_key_up {
            Ok(self
                .pressed_shortcuts
                .key_up(event.key_code, &mut self.state.borrow_mut()))
        } else {
            self.pressed_shortcuts.key_down(
                event.key_code,
                &definition,
                &mut self.state.borrow_mut(),
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
                self.apply_effects(
                    result.effects,
                    &format!("{:?} 키 {}", event.scope, definition.id),
                );
            }
            Err(error) => self.logger.event(&format!(
                "판서 키 core dispatch 실패: 정의={} error={error}",
                definition.id
            )),
        }
        if event.is_key_up {
            self.synchronize_keyboard_hook();
        }
    }

    fn handle_radial_key(&mut self, event: RadialPaletteKeyEvent) {
        let reason = event
            .pass_reason
            .map(|reason| match reason {
                RadialPaletteKeyPassReason::Hidden => "숨김",
                RadialPaletteKeyPassReason::Collapsed => "접힘",
                RadialPaletteKeyPassReason::TextInput => "텍스트 입력",
                RadialPaletteKeyPassReason::Modifiers => "수정키",
                RadialPaletteKeyPassReason::Unsupported => "대상 키 아님",
            })
            .unwrap_or("조건 충족");
        self.logger.event(&format!(
            "원형 팔레트 키 {}: vk={:#x} consumed={} repeat={} reason={reason}",
            if event.is_key_up { "up" } else { "down" },
            event.key_code,
            event.consumed,
            event.is_repeat,
        ));
        if event.is_key_up {
            self.synchronize_keyboard_hook();
            return;
        }
        if !event.consumed {
            return;
        }
        let Some(input) = event.input() else {
            self.logger.event(&format!(
                "원형 팔레트 키 매핑 실패: vk={:#x}",
                event.key_code
            ));
            return;
        };
        match input {
            RadialPaletteKeyboardInput::Navigate(key) => {
                let preselection = self
                    .radial_palette
                    .as_ref()
                    .and_then(|radial| radial.keyboard_preselection(key));
                if let Some((target, intent)) = preselection {
                    self.execute_radial_intent(intent, &target);
                }
                let result = self
                    .radial_palette
                    .as_mut()
                    .map(|radial| radial.navigate_keyboard(key));
                match result {
                    Some(Ok(Some((target, intent)))) => self.execute_radial_intent(intent, &target),
                    Some(Ok(None)) | None => {}
                    Some(Err(error)) => self.report_radial_palette_error("키보드 이동", &error),
                }
            }
            RadialPaletteKeyboardInput::Activate => {
                let result = self
                    .radial_palette
                    .as_mut()
                    .map(RadialPaletteManager::activate_keyboard_focus);
                match result {
                    Some(Ok(Some((target, intent)))) => self.execute_radial_intent(intent, &target),
                    Some(Ok(None)) | None => {}
                    Some(Err(error)) => self.report_radial_palette_error("키보드 활성화", &error),
                }
            }
        }
    }

    fn handle_radial_key_cap_hook(&mut self, event: RadialKeyCapHookEvent) {
        let event = match event {
            RadialKeyCapHookEvent::ControlAloneDown => RadialKeyCapEvent::ControlAloneDown,
            RadialKeyCapHookEvent::OtherKeyDown => RadialKeyCapEvent::OtherKeyDown,
            RadialKeyCapHookEvent::ModifierAdded => RadialKeyCapEvent::ModifierAdded,
            RadialKeyCapHookEvent::ControlReleased => RadialKeyCapEvent::ControlReleased,
        };
        self.apply_radial_key_cap_event(event, "키보드 훅");
    }

    fn radial_key_cap_timer_elapsed(&mut self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), RADIAL_KEY_CAP_TIMER_ID) };
        let control_only = current_modifiers() == ShortcutModifiers::CONTROL;
        self.apply_radial_key_cap_event(
            RadialKeyCapEvent::TimerElapsed { control_only },
            "0.5초 타이머",
        );
    }

    fn apply_radial_key_cap_event(&mut self, event: RadialKeyCapEvent, source: &str) {
        let (state, action) = next_radial_key_cap_state(self.radial_key_cap_state, event);
        self.radial_key_cap_state = state;
        match action {
            RadialKeyCapAction::None => {}
            RadialKeyCapAction::Schedule => {
                if unsafe {
                    SetTimer(
                        Some(self.hwnd),
                        RADIAL_KEY_CAP_TIMER_ID,
                        KEY_CAP_HOLD_DURATION_MS,
                        None,
                    )
                } == 0
                {
                    self.radial_key_cap_state = RadialKeyCapState::Idle;
                    self.logger.event("원형 팔레트 키캡 타이머 시작 실패");
                } else {
                    self.logger
                        .event(&format!("원형 팔레트 키캡 타이머 시작: {source}"));
                }
            }
            RadialKeyCapAction::Cancel => {
                let _ = unsafe { KillTimer(Some(self.hwnd), RADIAL_KEY_CAP_TIMER_ID) };
                let result = self
                    .radial_palette
                    .as_mut()
                    .map(|radial| radial.set_key_caps(false));
                if let Some(Err(error)) = result {
                    self.report_radial_palette_error("키캡 취소", &error);
                }
                self.logger
                    .event(&format!("원형 팔레트 키캡 취소: {source}"));
            }
            RadialKeyCapAction::Show => {
                let result = self
                    .radial_palette
                    .as_mut()
                    .map(|radial| radial.set_key_caps(true));
                if let Some(Err(error)) = result {
                    self.report_radial_palette_error("키캡 표시", &error);
                }
                self.logger
                    .event(&format!("원형 팔레트 키캡 표시: {source}"));
            }
        }
    }

    fn reset_radial_key_caps(&mut self, reason: &str) {
        let was_active = self.radial_key_cap_state != RadialKeyCapState::Idle;
        self.radial_key_cap_state = RadialKeyCapState::Idle;
        let _ = unsafe { KillTimer(Some(self.hwnd), RADIAL_KEY_CAP_TIMER_ID) };
        let result = self
            .radial_palette
            .as_mut()
            .map(|radial| radial.set_key_caps(false));
        if let Some(Err(error)) = result {
            self.report_radial_palette_error("키캡 초기화", &error);
        }
        if was_active {
            self.logger
                .event(&format!("원형 팔레트 키캡 취소: {reason}"));
        }
    }

    fn synchronize_keyboard_hook(&mut self) {
        let annotation_active = self.state.borrow().is_annotation_mode_enabled();
        let zoom_active = self.zoom.is_some();
        if annotation_active || zoom_active {
            if let Some(hook) = self.keyboard_hook.as_ref() {
                hook.set_scopes(annotation_active, zoom_active);
                self.logger.event(&format!(
                    "키보드 훅 스코프 갱신: zoom={zoom_active} annotation={annotation_active}"
                ));
                return;
            }
            match KeyboardHook::install(self.hwnd, &self.settings, annotation_active, zoom_active) {
                Ok(hook) => {
                    self.keyboard_hook = Some(hook);
                    self.logger.event(&format!(
                        "키보드 훅 설치 성공: zoom={zoom_active} annotation={annotation_active}"
                    ));
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

        let hook_has_pressed_keys = self
            .keyboard_hook
            .as_ref()
            .is_some_and(KeyboardHook::has_pressed_keys);
        if !self.pressed_shortcuts.is_empty() || hook_has_pressed_keys {
            if let Some(hook) = self.keyboard_hook.as_ref() {
                hook.set_scopes(false, false);
                self.logger
                    .event("키보드 훅 해제 지연: 이미 삼킨 keyUp 쌍을 받은 뒤 해제");
            }
            return;
        }

        let Some(mut hook) = self.keyboard_hook.take() else {
            self.reset_radial_key_caps("훅 비활성");
            return;
        };
        self.reset_radial_key_caps("훅 해제");
        match hook.uninstall() {
            Ok(()) => {
                self.pressed_shortcuts = PressedShortcuts::new();
                self.logger.event("키보드 훅 해제 성공");
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

    fn uninstall_keyboard_hook_for_shutdown(&mut self) {
        let Some(mut hook) = self.keyboard_hook.take() else {
            self.reset_radial_key_caps("훅 비활성");
            return;
        };
        self.reset_radial_key_caps("훅 해제");
        if let Err(error) = hook.uninstall() {
            self.logger
                .event(&format!("앱 종료 중 키보드 훅 해제 실패: {error}"));
        }
        self.pressed_shortcuts = PressedShortcuts::new();
    }

    fn synchronize_presenter_mouse_hook(&mut self, reason: &str) -> bool {
        let desired = [
            (
                MouseHookSubscriber::Spotlight,
                self.presenter.spotlight_enabled(),
            ),
            (
                MouseHookSubscriber::CursorHighlight,
                self.presenter.cursor_highlight_enabled(),
            ),
            (
                MouseHookSubscriber::PassThrough,
                self.state.borrow().is_pass_through_mode_enabled(),
            ),
        ];
        for (subscriber, enabled) in desired {
            match self.mouse_hook.set_subscriber(subscriber, enabled) {
                Ok(true) => self.logger.event(&format!(
                    "전역 마우스 훅 구독 변경: subscriber={subscriber:?} enabled={enabled} count={} installed={} reason={reason}",
                    self.mouse_hook.subscriber_count(),
                    self.mouse_hook.is_installed()
                )),
                Ok(false) => {}
                Err(error) => {
                    let message = format!(
                        "전역 마우스 훅 구독 실패: subscriber={subscriber:?} enabled={enabled} count={} reason={reason} error={error}",
                        self.mouse_hook.subscriber_count()
                    );
                    self.logger.event(&message);
                    if let Some(tray) = self.tray.as_ref() {
                        tray.show_warning("전역 마우스 훅 오류", &message);
                    }
                    return false;
                }
            }
        }
        true
    }

    fn prepare_pass_through_mouse_hook(&mut self, reason: &str) -> bool {
        match self
            .mouse_hook
            .set_subscriber(MouseHookSubscriber::PassThrough, true)
        {
            Ok(changed) => {
                if changed {
                    self.logger.event(&format!(
                        "전역 마우스 훅 통과 모드 사전 설치: count={} reason={reason}",
                        self.mouse_hook.subscriber_count()
                    ));
                }
                true
            }
            Err(error) => {
                let message = format!("통과 모드 전역 마우스 훅 설치 실패: {error}");
                self.logger.event(&message);
                if let Some(tray) = self.tray.as_ref() {
                    tray.show_warning("통과 모드 훅 오류", &message);
                }
                false
            }
        }
    }

    fn schedule_presenter_frame(&mut self) {
        if self.presenter_frame_pending {
            return;
        }
        let timer = unsafe {
            SetTimer(
                Some(self.hwnd),
                PRESENTER_FRAME_TIMER_ID,
                PRESENTER_FRAME_INTERVAL_MS,
                None,
            )
        };
        if timer == 0 {
            self.logger.event("발표 효과 16ms 프레임 타이머 설정 실패");
        } else {
            self.presenter_frame_pending = true;
        }
    }

    fn render_presenter(&mut self, reason: &str) {
        let _ = unsafe { KillTimer(Some(self.hwnd), PRESENTER_FRAME_TIMER_ID) };
        self.presenter_frame_pending = false;
        if let Err(error) = self.presenter.render() {
            let message = format!("발표 효과 렌더 실패: reason={reason} error={error}");
            self.logger.event(&message);
            if let Some(tray) = self.tray.as_ref() {
                tray.show_warning("발표 효과 오류", &message);
            }
            return;
        }
        if self.presenter.needs_animation() {
            self.schedule_presenter_frame();
        }
    }

    fn handle_presenter_mouse(&mut self, event: MouseHookEvent) {
        let radius_adjusted = self.presenter.handle_mouse_event(event);
        if radius_adjusted {
            self.logger.event(&format!(
                "스포트라이트 반지름 변경: radius={}",
                self.presenter.spotlight_radius()
            ));
        }
        if event.is_interaction() && self.state.borrow().is_pass_through_mode_enabled() {
            let now = monotonic_seconds();
            if self.state.borrow_mut().record_pass_through_interaction(now) {
                self.logger
                    .event("통과 모드 타이머 재설정: reason=mouse interaction");
                self.schedule_pass_through_timers("마우스 상호작용");
            }
        }
        self.schedule_presenter_frame();
    }

    fn handle_presenter_key(&mut self, vk: u32) {
        self.presenter.handle_key(vk);
        self.schedule_presenter_frame();
    }

    fn schedule_pass_through_timers(&mut self, reason: &str) {
        let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_IDLE_TIMER_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_MAXIMUM_HOLD_TIMER_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_KEY_POLL_TIMER_ID) };
        let now = monotonic_seconds();
        let (source, deadlines) = {
            let state = self.state.borrow();
            (
                state.pass_through_mode_source(),
                state.pass_through_deadlines(),
            )
        };
        let plan = PassThroughTimerPlan::from_core(now, source, deadlines);
        if let Some(delay) = plan.idle_ms {
            if unsafe { SetTimer(Some(self.hwnd), PASS_THROUGH_IDLE_TIMER_ID, delay, None) } == 0 {
                self.logger.event("통과 모드 idle SetTimer 실패");
            }
        }
        if let Some(delay) = plan.maximum_hold_ms {
            if unsafe {
                SetTimer(
                    Some(self.hwnd),
                    PASS_THROUGH_MAXIMUM_HOLD_TIMER_ID,
                    delay,
                    None,
                )
            } == 0
            {
                self.logger.event("통과 모드 maximum-hold SetTimer 실패");
            }
        }
        let poll_enabled = plan.poll_key.is_some() && !self.pass_through_key_polling_disabled;
        if poll_enabled {
            if self.pass_through_key_started_at.is_none() {
                self.pass_through_key_started_at = Some(now);
            }
            let interval = (PassThroughDefaults::KEY_STATE_POLLING_INTERVAL * 1_000.0)
                .ceil()
                .max(1.0) as u32;
            if unsafe {
                SetTimer(
                    Some(self.hwnd),
                    PASS_THROUGH_KEY_POLL_TIMER_ID,
                    interval,
                    None,
                )
            } == 0
            {
                self.logger.event("통과 모드 key-poll SetTimer 실패");
            }
        } else if plan.poll_key.is_none() {
            self.pass_through_key_started_at = None;
            self.pass_through_key_polling_disabled = false;
        }
        self.logger.event(&format!(
            "통과 모드 타이머 설정: reason={reason} idleMs={:?} maximumHoldMs={:?} keyPoll={}",
            plan.idle_ms, plan.maximum_hold_ms, poll_enabled
        ));
    }

    fn cancel_pass_through_timers(&mut self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_IDLE_TIMER_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_MAXIMUM_HOLD_TIMER_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_KEY_POLL_TIMER_ID) };
        self.pass_through_key_started_at = None;
        self.pass_through_key_polling_disabled = false;
    }

    fn finish_pass_through_runtime(&mut self, reason: &str) {
        let was_enabled = self.pass_through_runtime_enabled;
        self.pass_through_runtime_enabled = false;
        self.cancel_pass_through_timers();
        if let Some(overlays) = self.overlays.as_mut() {
            if let Err(error) = overlays.set_pass_through(false) {
                self.report_overlay_error("통과 모드 종료 반영", &error);
            }
        }
        self.synchronize_presenter_mouse_hook(reason);
        if was_enabled {
            self.logger
                .event(&format!("통과 모드 종료: reason={reason}"));
        }
    }

    fn handle_pass_through_expiry(&mut self, maximum_hold: bool) {
        let timer_id = if maximum_hold {
            PASS_THROUGH_MAXIMUM_HOLD_TIMER_ID
        } else {
            PASS_THROUGH_IDLE_TIMER_ID
        };
        let _ = unsafe { KillTimer(Some(self.hwnd), timer_id) };
        let now = monotonic_seconds();
        let pressed = mouse_buttons_pressed();
        let ended = self.state.borrow_mut().expire_pass_through(now, pressed);
        if ended {
            self.finish_pass_through_runtime(if maximum_hold {
                "maximum hold timeout"
            } else {
                "interaction idle timeout"
            });
        } else if self.state.borrow().is_pass_through_mode_enabled() {
            self.logger.event(&format!(
                "통과 모드 타이머 만료 후 유지: reason={} mouseButtonsPressed={pressed}",
                if maximum_hold { "maximum hold" } else { "idle" }
            ));
            self.schedule_pass_through_timers("눌린 마우스 버튼으로 재설정");
        }
    }

    fn poll_pass_through_key(&mut self) {
        let source = self.state.borrow().pass_through_mode_source();
        let Some(PassThroughModeSource::Key(vk)) = source else {
            let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_KEY_POLL_TIMER_ID) };
            self.pass_through_key_started_at = None;
            return;
        };
        let started = self
            .pass_through_key_started_at
            .unwrap_or_else(monotonic_seconds);
        let elapsed = monotonic_seconds() - started;
        let key_is_down = async_key_is_down(vk);
        match key_poll_decision(
            key_is_down,
            elapsed,
            PassThroughDefaults::KEY_STATE_POLLING_TRUST_WINDOW,
        ) {
            KeyPollDecision::Continue => {}
            KeyPollDecision::DisableDuringTrustWindow => {
                let _ = unsafe { KillTimer(Some(self.hwnd), PASS_THROUGH_KEY_POLL_TIMER_ID) };
                self.pass_through_key_started_at = None;
                self.pass_through_key_polling_disabled = true;
                self.logger.event(&format!(
                    "통과 모드 키 폴링 종료: reason=trust-window releasedWithinMs={}",
                    (PassThroughDefaults::KEY_STATE_POLLING_TRUST_WINDOW * 1_000.0) as u32
                ));
            }
            KeyPollDecision::EndPassThrough => {
                if self.state.borrow_mut().end_key_pass_through(vk) {
                    self.finish_pass_through_runtime("key polling observed release");
                }
            }
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>, source: &str) {
        let mut runtime_state_changed = false;
        for effect in effects {
            runtime_state_changed |= !matches!(&effect, Effect::SettingsMutation(_));
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
                    if !enabled {
                        self.cancel_text_entry("판서 끔");
                        self.reset_radial_key_caps("판서 끔");
                        if self.pass_through_runtime_enabled {
                            self.finish_pass_through_runtime("annotation mode ended");
                        }
                    }
                    self.synchronize_keyboard_hook();
                    if let Some(tray) = self.tray.as_mut() {
                        tray.set_annotation_enabled(enabled);
                    }
                    let input_config =
                        InkInputConfig::from_core(&self.state.borrow(), &self.settings);
                    if let Some(overlays) = self.overlays.as_mut() {
                        let annotation_error =
                            overlays.set_annotation_mode(enabled, input_config).err();
                        let board_error = (!enabled)
                            .then(|| overlays.refresh_board(&self.settings).err())
                            .flatten();
                        if let Some(error) = annotation_error {
                            self.report_overlay_error("판서 모드 반영", &error);
                        }
                        if let Some(error) = board_error {
                            self.report_overlay_error("보드 닫기 반영", &error);
                        }
                    }
                    if !enabled {
                        if let Some(board) = self.last_board_presentation.take() {
                            self.logger.event(&format!(
                                "보드 닫기: kind={:?} display={}",
                                board.kind, board.display_id
                            ));
                        }
                    }
                    self.logger.event(&format!(
                        "annotation.toggle 결과 ({source}): {}",
                        if enabled { "켜짐" } else { "꺼짐" }
                    ));
                }
                Effect::PassThroughModeChanged(enabled) => {
                    if enabled {
                        self.pass_through_runtime_enabled = true;
                        if let Some(overlays) = self.overlays.as_mut() {
                            if let Err(error) = overlays.set_pass_through(true) {
                                self.report_overlay_error("통과 모드 반영", &error);
                            }
                        }
                        self.synchronize_presenter_mouse_hook("통과 모드 시작");
                        self.schedule_pass_through_timers("core deadline 시작");
                    } else {
                        self.finish_pass_through_runtime("keyUp/core effect");
                    }
                    self.logger.event(&format!(
                        "효과 ({source}): 통과 모드 {}",
                        if enabled { "켜짐" } else { "꺼짐" }
                    ));
                }
                Effect::ToolChanged(tool) => {
                    if tool != InkTool::Text {
                        self.cancel_text_entry("도구 변경");
                    }
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
                Effect::BoardChanged(presentation) => {
                    if let Some(overlays) = self.overlays.as_mut() {
                        if let Err(error) = overlays.refresh_board(&self.settings) {
                            self.report_overlay_error("보드 표시 반영", &error);
                        }
                    }
                    match presentation {
                        Some(board) => {
                            self.logger.event(&format!(
                                "보드 열기/변경: kind={:?} display={}",
                                board.kind, board.display_id
                            ));
                            self.last_board_presentation = Some(board);
                        }
                        None => {
                            if let Some(board) = self.last_board_presentation.take() {
                                self.logger.event(&format!(
                                    "보드 닫기: kind={:?} display={}",
                                    board.kind, board.display_id
                                ));
                            } else {
                                self.logger.event("보드 닫기: 기존 표시 없음");
                            }
                        }
                    }
                }
                Effect::FocusBoardPanel(display_id) => {
                    // Windows에는 별도 보드 패널이 없으므로 활성 디스플레이와 재그리기만 맞춘다.
                    self.state
                        .borrow_mut()
                        .set_active_display(display_id.clone());
                    if let Some(overlays) = self.overlays.as_mut() {
                        if let Err(error) = overlays.redraw_monitor(&display_id) {
                            self.report_overlay_error("보드 포커스 재그리기", &error);
                        }
                    }
                    self.logger
                        .event(&format!("보드 포커스: active_display={display_id}"));
                }
                Effect::BoardTargetMissing => {
                    self.logger.event("보드 대상 디스플레이 없음");
                }
                Effect::MoveBoardPage {
                    direction,
                    display_id,
                } => {
                    let result = self.overlays.as_mut().map(|overlays| {
                        overlays.move_board_page(direction, &display_id, &self.settings)
                    });
                    match result {
                        Some(Ok(moved)) if moved.changed => self.logger.event(&format!(
                            "보드 페이지 이동: direction={direction:?} display={display_id} {}→{}/{}",
                            moved.from, moved.to, moved.total
                        )),
                        Some(Ok(moved)) => self.logger.event(&format!(
                            "보드 페이지 이동 불가: direction={direction:?} display={display_id} current={}/{}",
                            moved.to, moved.total
                        )),
                        Some(Err(error)) => self.report_overlay_error("보드 페이지 이동", &error),
                        None => self.logger.event(&format!(
                            "보드 페이지 이동 불가: 오버레이 없음 display={display_id}"
                        )),
                    }
                }
                Effect::SettingsMutation(mutation) => {
                    let changed = self.update_settings(source, move |settings| {
                        mutation.apply(settings);
                    });
                    if changed && self.state.borrow().current_tool() == InkTool::Eraser {
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
                Effect::DeleteSelection(display_id) => {
                    let result = self
                        .overlays
                        .as_mut()
                        .map(|overlays| overlays.delete_selection(&display_id));
                    match result {
                        Some(Ok(changed)) => self.logger.event(&format!(
                            "효과 ({source}): 선택 삭제 monitor={display_id} changed={changed}"
                        )),
                        Some(Err(error)) => self.report_overlay_error("선택 삭제", &error),
                        None => {}
                    }
                }
                Effect::ClearSelection(display_id) => {
                    let result = self
                        .overlays
                        .as_mut()
                        .map(|overlays| overlays.clear_selection(&display_id));
                    match result {
                        Some(Ok(changed)) => self.logger.event(&format!(
                            "효과 ({source}): 선택 해제 monitor={display_id} changed={changed}"
                        )),
                        Some(Err(error)) => self.report_overlay_error("선택 해제", &error),
                        None => {}
                    }
                }
                Effect::SaveCurrentContent => {
                    if let Some(overlays) = self.overlays.as_mut() {
                        self.export
                            .save_current_content(overlays, &self.state, &self.settings);
                    }
                }
                Effect::CopyCurrentImage => {
                    if let Some(overlays) = self.overlays.as_mut() {
                        self.export
                            .copy_current_image(overlays, &self.state, &self.settings);
                    }
                }
                Effect::StepBack(result) => self
                    .logger
                    .event(&format!("효과 ({source}): 한 단계 나가기 {result:?}")),
                Effect::PaletteVisibilityChanged(visible) => self
                    .logger
                    .event(&format!("효과 ({source}): 선형 팔레트 표시 상태 {visible}")),
                Effect::ToggleRadialPalette => {
                    if !self.state.borrow().is_palette_visible() {
                        self.state.borrow_mut().toggle_palette_visibility();
                        self.logger
                            .event(&format!("효과 ({source}): 원형 팔레트 숨김 해제 후 펼침"));
                        self.refresh_radial_palette("M 키 숨김 해제");
                    }
                    let result = self
                        .radial_palette
                        .as_mut()
                        .map(RadialPaletteManager::toggle_expansion_from_keyboard);
                    if let Some(Err(error)) = result {
                        self.report_radial_palette_error("M 키 토글", &error);
                    }
                }
                Effect::CollapseRadialPalette => {
                    if let Some(radial) = self.radial_palette.as_mut() {
                        if let Err(error) = radial.collapse("stepBack") {
                            self.report_radial_palette_error("stepBack 접기", &error);
                        }
                    }
                }
                Effect::HelpShortcutPressed(_) => {
                    if let Some(help) = self.help.as_mut() {
                        help.shortcut_did_press(
                            monotonic_seconds(),
                            &self.state.borrow(),
                            &self.settings,
                            source,
                        );
                    }
                }
                Effect::HelpShortcutReleased => {
                    if let Some(help) = self.help.as_mut() {
                        help.shortcut_did_release(
                            monotonic_seconds(),
                            &self.state.borrow(),
                            &self.settings,
                            source,
                        );
                    }
                }
                Effect::ShowHelp => {
                    if let Some(help) = self.help.as_mut() {
                        help.show_pinned(&self.state.borrow(), &self.settings, source);
                    }
                }
                Effect::CloseHelp => {
                    if let Some(help) = self.help.as_mut() {
                        help.close(&self.state.borrow(), source);
                    }
                }
                Effect::ToggleZoom => {
                    if self.zoom.is_some() {
                        self.stop_zoom("토글 키", false);
                    } else {
                        self.start_zoom();
                    }
                }
                Effect::ToggleSpotlight => {
                    let enabled = self.presenter.toggle_spotlight();
                    self.synchronize_presenter_mouse_hook("스포트라이트 토글");
                    self.logger.event(&format!(
                        "효과 ({source}): 스포트라이트 {}",
                        if enabled { "켜짐" } else { "꺼짐" }
                    ));
                    self.render_presenter("스포트라이트 토글");
                }
                Effect::ToggleCursorHighlight => {
                    let enabled = self.presenter.toggle_cursor_highlight();
                    self.synchronize_presenter_mouse_hook("커서 강조 토글");
                    self.logger.event(&format!(
                        "효과 ({source}): 커서 강조 {}",
                        if enabled { "켜짐" } else { "꺼짐" }
                    ));
                    self.render_presenter("커서 강조 토글");
                }
                Effect::ToggleKeystrokeDisplay => match self.presenter.toggle_keystroke_display() {
                    Ok(enabled) => {
                        self.logger.event(&format!(
                            "효과 ({source}): 키 입력 표시 {}",
                            if enabled { "켜짐" } else { "꺼짐" }
                        ));
                        self.render_presenter("키 입력 표시 토글");
                    }
                    Err(error) => {
                        let message = format!("키 입력 표시 훅 토글 실패: {error}");
                        self.logger.event(&message);
                        if let Some(tray) = self.tray.as_ref() {
                            tray.show_warning("키 입력 표시 오류", &message);
                        }
                    }
                },
                Effect::AdjustSpotlightWidth(steps) => {
                    let applied = self.presenter.adjust_radius(steps);
                    self.logger.event(&format!(
                        "스포트라이트 반지름 변경: steps={steps} applied={applied} radius={}",
                        self.presenter.spotlight_radius()
                    ));
                    if applied {
                        self.render_presenter("스포트라이트 반지름 명령");
                    }
                }
                Effect::OpenSettings => {
                    self.show_settings(SettingsTab::General, source);
                }
                Effect::AdjustZoom(steps) => self.adjust_zoom(steps),
                Effect::ResetZoom => self.reset_zoom(),
                Effect::ToggleZoomFreeze => self.toggle_zoom_freeze(),
                Effect::StopZoom => self.stop_or_cancel_zoom_preview(),
                Effect::BeginZoomPreview => self.begin_zoom_preview(),
                Effect::FinishZoomPreview => self.finish_zoom_preview(false),
                Effect::CycleZoomFollowMode => self.cycle_zoom_follow_mode(),
                Effect::ApplyDisplayPreset(index) => {
                    self.apply_display_preset(index, DisplayApplySource::Manual);
                }
                Effect::ToggleLessonTimer => {
                    let result = self
                        .lesson_badge
                        .as_mut()
                        .map(|badge| badge.toggle_from_shortcut(&self.settings));
                    if let Some(Err(error)) = result {
                        self.report_lesson_badge_error("전역 토글", &error);
                    }
                }
                Effect::ShowBreakTimerSetup => {
                    let result = self
                        .break_timer
                        .as_mut()
                        .map(|timer| timer.open_setup(&self.settings));
                    if let Some(Err(error)) = result {
                        self.report_break_timer_error("설정 창 열기", &error);
                    }
                }
            }
        }
        if runtime_state_changed {
            self.refresh_runtime_state("core effect");
            self.render_zoom("core effect");
        }
    }

    fn zoom_gesture_in_progress(&self) -> bool {
        if crate::text_entry::text_input_active() {
            return true;
        }
        let Some(display_id) = self
            .state
            .borrow()
            .zoom_session()
            .map(|session| session.display_id.clone())
        else {
            return false;
        };
        self.overlays
            .as_ref()
            .is_some_and(|overlays| overlays.zoom_gesture_in_progress(&display_id))
    }

    fn start_zoom(&mut self) {
        if self.zoom.is_some() || self.state.borrow().zoom_session().is_some() {
            return;
        }
        let mut cursor = POINT::default();
        if let Err(error) = unsafe { GetCursorPos(&mut cursor) } {
            self.report_zoom_failure(format!("포인터 위치를 읽지 못했습니다: {error}"));
            return;
        }
        let monitors = match enumerate_monitors() {
            Ok(monitors) => monitors,
            Err(error) => {
                self.report_zoom_failure(format!("모니터 목록을 읽지 못했습니다: {error}"));
                return;
            }
        };
        let Some(monitor) = monitor_at_point(&monitors, cursor).cloned() else {
            self.report_zoom_failure("포인터가 있는 모니터를 찾지 못했습니다.".to_owned());
            return;
        };
        match ZoomCapture::is_supported() {
            Ok(true) => {}
            Ok(false) => {
                self.report_zoom_failure(
                    "이 Windows 환경은 Windows Graphics Capture를 지원하지 않습니다.".to_owned(),
                );
                return;
            }
            Err(error) => {
                self.report_zoom_failure(format!(
                    "Windows Graphics Capture 지원 여부를 확인하지 못했습니다: {error}"
                ));
                return;
            }
        }
        let (local_x, local_y) = local_point_in_monitor(monitor.rect, cursor.x, cursor.y);
        let screen_size = ZoomSize::new(
            f64::from(monitor.rect.width()),
            f64::from(monitor.rect.height()),
        );
        let magnification = self.settings.zoom.starting_magnification;
        let center = ZoomGeometry::clamped_center(
            ZoomPoint::new(f64::from(local_x), f64::from(local_y)),
            screen_size,
            magnification,
        );
        let follow_mode = match self.settings.zoom.follow_mode.as_str() {
            "always" => ZoomFollowMode::Always,
            "fixed" => ZoomFollowMode::Fixed,
            _ => ZoomFollowMode::Edge,
        };
        let session = ZoomSessionState {
            display_id: monitor.session_key.clone(),
            screen_size,
            magnification,
            starting_magnification: magnification,
            center,
            follow_mode,
            follow_smoothness: self.settings.zoom.follow_smoothness,
            preview_dimming: self.settings.zoom.preview_dimming,
            is_frozen: !self.settings.zoom.live_by_default,
            is_previewing: false,
        };
        self.state.borrow_mut().set_zoom_session(session.clone());
        let previous_foreground = unsafe { GetForegroundWindow() };
        let shared_graphics = self.overlays.as_ref().map(OverlayManager::graphics_device);
        let initial_ink_frame = self
            .overlays
            .as_ref()
            .and_then(|overlays| overlays.zoom_ink_frame(&session.display_id));
        let result = (|| -> Result<ZoomRuntime> {
            let graphics = match shared_graphics {
                Some(graphics) => graphics,
                None => GraphicsDevice::new()?,
            };
            let mut view = ZoomView::new(
                self.hwnd,
                self.hinstance,
                &monitor,
                &graphics,
                self.logger.clone(),
            )?;
            view.set_pointer(center);
            view.show_message(ZoomGeometry::magnification_label(magnification));
            view.render(&session, initial_ink_frame.as_ref())?;
            let capture = ZoomCapture::start(
                self.hwnd,
                monitor.handle,
                SizeInt32 {
                    Width: monitor.rect.width(),
                    Height: monitor.rect.height(),
                },
                &graphics,
                self.logger.clone(),
            )?;
            Ok(ZoomRuntime {
                view,
                preview_original_frame: None,
                capture,
                _graphics: graphics,
                monitor,
                previous_foreground,
                wheel_accumulator: 0,
            })
        })();
        match result {
            Ok(runtime) => {
                let border = runtime.capture.border_disabled();
                self.logger.event(&format!(
                    "줌 시작: monitor={} physical={}x{} center=({:.1},{:.1}) magnification={:.2} follow={:?} WGC=true borderDisabled={border}",
                    runtime.monitor.session_key,
                    runtime.monitor.rect.width(),
                    runtime.monitor.rect.height(),
                    center.x,
                    center.y,
                    magnification,
                    follow_mode,
                ));
                self.zoom = Some(runtime);
                self.synchronize_keyboard_hook();
            }
            Err(error) => {
                self.state.borrow_mut().end_zoom();
                self.report_zoom_failure(format!(
                    "Windows Graphics Capture로 줌을 시작하지 못했습니다: {error}"
                ));
            }
        }
    }

    fn stop_zoom(&mut self, reason: &str, notify: bool) {
        let warp_destination = self
            .state
            .borrow()
            .zoom_session()
            .filter(|session| session.is_frozen && !session.is_previewing)
            .and_then(|session| {
                self.zoom_pointer_location().map(|presentation| {
                    ZoomGeometry::original_point(
                        presentation,
                        session.screen_size,
                        session.source_rect(),
                    )
                })
            });
        let Some(mut runtime) = self.zoom.take() else {
            self.state.borrow_mut().end_zoom();
            self.synchronize_keyboard_hook();
            return;
        };
        if let Some(destination) = warp_destination {
            if warp_zoom_pointer(&self.logger, &runtime.monitor, destination).is_some() {
                runtime.view.set_pointer(destination);
            }
        }
        let previous_foreground = runtime.previous_foreground;
        let overwritten = runtime.capture.overwritten_count();
        drop(runtime);
        self.state.borrow_mut().end_zoom();
        self.synchronize_keyboard_hook();
        self.logger.event(&format!(
            "줌 종료: reason={reason} slotOverwrites={overwritten}"
        ));
        if !self.state.borrow().is_annotation_mode_enabled()
            && !previous_foreground.is_invalid()
            && unsafe { IsWindow(Some(previous_foreground)) }.as_bool()
        {
            let restored = unsafe { SetForegroundWindow(previous_foreground) }.as_bool();
            if !restored {
                self.logger.event("줌 시작 전 전경 앱 복원 실패");
            }
        }
        if notify {
            if let Some(tray) = self.tray.as_ref() {
                tray.show_warning(
                    "화면 줌 종료",
                    "모니터 구성이나 캡처 장치가 바뀌어 줌을 종료했습니다. 다시 시작하세요.",
                );
            }
        }
    }

    fn adjust_zoom(&mut self, steps: i32) {
        if steps == 0 || self.zoom.is_none() || self.zoom_gesture_in_progress() {
            return;
        }
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        let adjusted = ZoomGeometry::adjusted_magnification(session.magnification, steps);
        if adjusted == session.magnification {
            return;
        }
        session.magnification = adjusted;
        session.center = ZoomGeometry::clamped_center(
            session.center,
            session.screen_size,
            session.magnification,
        );
        self.state.borrow_mut().set_zoom_session(session);
        if let Some(runtime) = self.zoom.as_mut() {
            runtime
                .view
                .show_message(ZoomGeometry::magnification_label(adjusted));
        }
        self.logger.event(&format!(
            "줌 배율 변경: steps={steps} magnification={adjusted:.2}"
        ));
        self.render_zoom("배율 변경");
    }

    fn reset_zoom(&mut self) {
        if self.zoom_gesture_in_progress() {
            return;
        }
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        session.magnification = session.starting_magnification;
        session.center = ZoomGeometry::clamped_center(
            session.center,
            session.screen_size,
            session.magnification,
        );
        let magnification = session.magnification;
        self.state.borrow_mut().set_zoom_session(session);
        if let Some(runtime) = self.zoom.as_mut() {
            runtime
                .view
                .show_message(ZoomGeometry::magnification_label(magnification));
        }
        self.logger
            .event(&format!("줌 배율 초기화: magnification={magnification:.2}"));
        self.render_zoom("배율 초기화");
    }

    fn cycle_zoom_follow_mode(&mut self) {
        if self.zoom_gesture_in_progress() {
            return;
        }
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        session.follow_mode = session.follow_mode.next();
        let mode = session.follow_mode;
        self.state.borrow_mut().set_zoom_session(session);
        let label = match mode {
            ZoomFollowMode::Always => "항상",
            ZoomFollowMode::Edge => "가장자리",
            ZoomFollowMode::Fixed => "고정",
        };
        if let Some(runtime) = self.zoom.as_mut() {
            runtime.view.show_message(format!("따라가기: {label}"));
        }
        self.logger.event(&format!("줌 따라가기 변경: {mode:?}"));
        self.render_zoom("따라가기 변경");
    }

    fn toggle_zoom_freeze(&mut self) {
        if self.zoom_gesture_in_progress() {
            return;
        }
        let Some(runtime) = self.zoom.as_ref() else {
            return;
        };
        if !runtime.view.has_frame() {
            if let Some(runtime) = self.zoom.as_mut() {
                runtime.view.show_message("첫 화면을 캡처하는 중입니다");
            }
            return;
        }
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        let warp_destination = self.zoom_pointer_location().map(|pointer| {
            if session.is_frozen {
                ZoomGeometry::original_point(pointer, session.screen_size, session.source_rect())
            } else {
                ZoomGeometry::pointer_presentation_point(pointer, &session)
            }
        });
        session.is_frozen = !session.is_frozen;
        let frozen = session.is_frozen;
        if let Some(runtime) = self.zoom.as_mut() {
            runtime
                .capture
                .set_accepting_frames(!frozen || session.is_previewing);
            runtime.view.show_message(if frozen {
                "정지 줌 · 판서 켜짐"
            } else {
                "라이브 줌"
            });
            if let Some(destination) = warp_destination {
                if warp_zoom_pointer(&self.logger, &runtime.monitor, destination).is_some() {
                    runtime.view.set_pointer(destination);
                }
            }
        }
        self.state.borrow_mut().set_zoom_session(session);
        let should_enable_annotation = frozen && !self.state.borrow().is_annotation_mode_enabled();
        if should_enable_annotation {
            self.state.borrow_mut().set_annotation_mode_enabled(true);
            self.apply_effects(
                vec![Effect::AnnotationModeChanged(true)],
                "정지 줌 판서 활성화",
            );
        }
        self.logger.event(&format!("줌 정지 전이: frozen={frozen}"));
        self.render_zoom("정지 전이");
    }

    fn begin_zoom_preview(&mut self) {
        if self.zoom_gesture_in_progress() {
            return;
        }
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        if session.is_previewing {
            return;
        }
        let Some(runtime) = self.zoom.as_mut() else {
            return;
        };
        runtime.preview_original_frame = runtime.view.current_frame();
        runtime.capture.set_accepting_frames(true);
        session.is_previewing = true;
        self.state.borrow_mut().set_zoom_session(session);
        self.logger.event("줌 미리보기 시작");
        self.render_zoom("미리보기 시작");
        if let Some(runtime) = self.zoom.as_ref() {
            let _ = runtime.view.focus();
        }
    }

    fn finish_zoom_preview(&mut self, cancelled: bool) {
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        if !session.is_previewing {
            return;
        }
        let Some(runtime) = self.zoom.as_mut() else {
            return;
        };
        session.is_previewing = false;
        if cancelled && session.is_frozen {
            if let Some(frame) = runtime.preview_original_frame.take() {
                runtime.view.restore_frame(frame);
            }
        } else {
            runtime.preview_original_frame = None;
        }
        runtime.capture.set_accepting_frames(!session.is_frozen);
        self.state.borrow_mut().set_zoom_session(session);
        self.logger
            .event(&format!("줌 미리보기 종료: cancelled={cancelled}"));
        self.render_zoom("미리보기 종료");
        if let Some(runtime) = self.zoom.as_ref() {
            let _ = runtime.view.focus();
        }
    }

    fn stop_or_cancel_zoom_preview(&mut self) {
        let previewing = self
            .state
            .borrow()
            .zoom_session()
            .is_some_and(|session| session.is_previewing);
        match stop_or_cancel_preview_action(previewing) {
            ZoomStopAction::CancelPreview => self.finish_zoom_preview(true),
            ZoomStopAction::StopZoom => self.stop_zoom("Esc/StopZoom", false),
        }
    }

    fn handle_zoom_frame(&mut self) {
        let frame = self
            .zoom
            .as_ref()
            .and_then(|runtime| runtime.capture.take_latest());
        let Some(frame) = frame else { return };
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        let already_frozen = session.is_frozen
            && !session.is_previewing
            && self
                .zoom
                .as_ref()
                .is_some_and(|runtime| runtime.view.has_frame());
        if already_frozen {
            return;
        }
        if let Some(runtime) = self.zoom.as_mut() {
            runtime.view.set_frame(frame);
            if session.is_frozen && !session.is_previewing {
                runtime.capture.set_accepting_frames(false);
            }
        }
        if !session.is_frozen && !session.is_previewing && !self.zoom_gesture_in_progress() {
            if let Some(pointer) = self.zoom_pointer_location() {
                session.center = ZoomGeometry::followed_center(
                    session.center,
                    pointer,
                    session.screen_size,
                    session.magnification,
                    session.follow_mode,
                    session.follow_smoothness,
                );
                self.state.borrow_mut().set_zoom_session(session);
            }
        }
        self.render_zoom("프레임 수신");
    }

    fn handle_zoom_pointer(&mut self, point: ZoomPoint) {
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        if point.x < 0.0
            || point.y < 0.0
            || point.x >= session.screen_size.width
            || point.y >= session.screen_size.height
        {
            return;
        }
        if let Some(runtime) = self.zoom.as_mut() {
            runtime.view.set_pointer(point);
        }
        if self.zoom_gesture_in_progress() {
            self.render_zoom("제스처 중 포인터 이동");
            return;
        }
        if session.is_previewing {
            session.center =
                ZoomGeometry::clamped_center(point, session.screen_size, session.magnification);
            self.state.borrow_mut().set_zoom_session(session);
        } else if !session.is_frozen {
            session.center = ZoomGeometry::followed_center(
                session.center,
                point,
                session.screen_size,
                session.magnification,
                session.follow_mode,
                session.follow_smoothness,
            );
            self.state.borrow_mut().set_zoom_session(session);
        }
        self.render_zoom("포인터 이동");
    }

    fn handle_zoom_ink_input(&mut self, pointer_id: u32, message: u32) {
        let Some(session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        if session.is_previewing || !self.state.borrow().is_annotation_mode_enabled() {
            return;
        }
        let result = self
            .overlays
            .as_mut()
            .map(|overlays| overlays.handle_zoom_input(message, pointer_id, &session));
        match result {
            Some(Ok(())) => self.render_zoom("줌 판서 입력"),
            Some(Err(error)) => self.report_overlay_error("줌 판서 입력", &error),
            None => self
                .logger
                .event("줌 판서 입력 실패: 오버레이 관리자가 없음"),
        }
    }

    fn handle_zoom_wheel(&mut self, delta: i32, control_down: bool) {
        if self.zoom_gesture_in_progress() {
            if delta != 0 {
                self.logger
                    .event("줌 제스처 잠금으로 키 거부: key=Ctrl+Wheel");
            }
            if let Some(runtime) = self.zoom.as_mut() {
                runtime.wheel_accumulator = 0;
            }
            return;
        }
        let previewing = self
            .state
            .borrow()
            .zoom_session()
            .is_some_and(|session| session.is_previewing);
        if !previewing && !control_down {
            if let Some(runtime) = self.zoom.as_mut() {
                runtime.wheel_accumulator = 0;
            }
            return;
        }
        let steps = if let Some(runtime) = self.zoom.as_mut() {
            runtime.wheel_accumulator += delta;
            let steps = runtime.wheel_accumulator / 120;
            runtime.wheel_accumulator -= steps * 120;
            steps
        } else {
            0
        };
        self.adjust_zoom(steps);
    }

    fn handle_zoom_recreate(&mut self) {
        let result = self.zoom.as_mut().map(|runtime| {
            runtime.preview_original_frame = None;
            runtime.view.clear_frame();
            runtime.capture.set_accepting_frames(true);
            runtime.capture.recreate_if_needed()
        });
        match result {
            Some(Ok(true)) => self.logger.event("WGC 프레임 풀 재생성 완료"),
            Some(Ok(false)) | None => {}
            Some(Err(error)) => {
                let message = format!("WGC 프레임 풀 재생성 실패: {error}");
                self.stop_zoom("캡처 크기 변경 실패", false);
                self.report_zoom_failure(message);
            }
        }
    }

    fn handle_zoom_capture_failure(&mut self) {
        let message = self
            .zoom
            .as_ref()
            .and_then(|runtime| runtime.capture.take_failure())
            .unwrap_or_else(|| "Windows Graphics Capture가 중단되었습니다.".to_owned());
        self.stop_zoom("캡처 실패", false);
        self.report_zoom_failure(message);
    }

    fn zoom_pointer_location(&self) -> Option<ZoomPoint> {
        let runtime = self.zoom.as_ref()?;
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point) }.ok()?;
        if point.x < runtime.monitor.rect.left
            || point.x >= runtime.monitor.rect.right
            || point.y < runtime.monitor.rect.top
            || point.y >= runtime.monitor.rect.bottom
        {
            return None;
        }
        Some(ZoomPoint::new(
            f64::from(point.x - runtime.monitor.rect.left),
            f64::from(point.y - runtime.monitor.rect.top),
        ))
    }

    fn render_zoom(&mut self, reason: &str) {
        let Some(session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        let ink_frame = self
            .overlays
            .as_ref()
            .and_then(|overlays| overlays.zoom_ink_frame(&session.display_id));
        if let Some(pointer) = self.zoom_pointer_location() {
            if let Some(runtime) = self.zoom.as_mut() {
                runtime.view.set_pointer(pointer);
            }
        }
        let result = self
            .zoom
            .as_mut()
            .map(|runtime| runtime.view.render(&session, ink_frame.as_ref()));
        if let Some(Err(error)) = result {
            let message = format!("줌 창 렌더 실패 ({reason}): {error}");
            self.stop_zoom("렌더 장치 실패", false);
            self.report_zoom_failure(message);
        }
    }

    fn report_zoom_failure(&self, message: String) {
        self.logger.event(&format!("줌 실패: {message}"));
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning("화면 줌 오류", &message);
        }
    }

    fn register_display_notifications(&mut self) -> Result<()> {
        let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: u32::try_from(size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>())
                .expect("DEV_BROADCAST_DEVICEINTERFACE_W size fits u32"),
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
            dbcc_classguid: GUID_DEVINTERFACE_MONITOR,
            ..Default::default()
        };
        // SAFETY: hwnd is the live hidden top-level window and filter has the documented size/type.
        let notification = unsafe {
            RegisterDeviceNotificationW(
                HANDLE(self.hwnd.0),
                (&filter as *const DEV_BROADCAST_DEVICEINTERFACE_W).cast(),
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        }?;
        self.device_notification = Some(notification);
        self.logger
            .event("디스플레이 장치 알림 등록: GUID_DEVINTERFACE_MONITOR");
        Ok(())
    }

    fn apply_display_preset(
        &mut self,
        index: usize,
        source: DisplayApplySource,
    ) -> Option<HashSet<String>> {
        if self.display_applier.state() != DisplayApplyState::Idle {
            let reason = match self.display_applier.state() {
                DisplayApplyState::Applying => "다른 화면 구성을 적용하는 중입니다",
                DisplayApplyState::AwaitingConfirmation => {
                    "현재 화면 구성을 먼저 유지하거나 되돌리세요"
                }
                DisplayApplyState::Idle => unreachable!(),
            };
            self.logger.event(&format!(
                "화면 구성 적용 요청 거부: source={} index={} reason={reason}",
                source.log_name(),
                index
            ));
            if source == DisplayApplySource::Manual {
                self.show_display_apply_warning("화면 구성을 적용할 수 없습니다", reason);
            }
            return None;
        }
        let Some(preset) = self.settings.display.presets.get(index).cloned() else {
            let reason = format!("프리셋 {}이 없습니다", index + 1);
            self.logger.event(&format!(
                "화면 구성 적용 요청 거부: source={} index={} reason={reason}",
                source.log_name(),
                index
            ));
            if source == DisplayApplySource::Manual {
                self.show_display_apply_warning("화면 구성을 적용할 수 없습니다", &reason);
            }
            return None;
        };
        let configuration = match current_display_configuration() {
            Ok(configuration) => configuration,
            Err(error) => {
                let reason = format!("현재 화면 구성을 읽지 못했습니다: {error}");
                self.logger.event(&reason);
                if source == DisplayApplySource::Manual {
                    self.show_display_apply_warning("화면 구성을 적용할 수 없습니다", &reason);
                }
                return None;
            }
        };
        let plan = DisplayPlanner::plan(&preset, &configuration.online);
        let desired_mirror_ids = plan
            .mirrors
            .iter()
            .map(|mirror| mirror.slave_id)
            .collect::<HashSet<_>>();
        let disappearing = configuration
            .online
            .iter()
            .filter(|display| {
                display.current_mirror_master_id.is_none()
                    && desired_mirror_ids.contains(&display.id)
            })
            .map(|display| display.uuid.clone())
            .collect::<HashSet<_>>();
        let Some(archived) = self
            .overlays
            .as_mut()
            .and_then(|overlays| overlays.archive_disconnected_displays_for_preset(&disappearing))
        else {
            self.logger
                .event("화면 구성 적용 중단: 사라질 화면 보관 실패");
            if source == DisplayApplySource::Manual {
                self.show_display_apply_warning(
                    "화면 구성을 적용할 수 없습니다",
                    "사라질 화면의 판서를 보관하지 못했습니다.",
                );
            }
            return None;
        };
        if let Some(overlays) = self.overlays.as_mut() {
            overlays.set_display_preset_restoration_deferred(true, true);
        }

        let result = match self.display_applier.apply_preset(&preset, index, source) {
            Ok(completion) => {
                let active_uuids = completion.expected.active_uuids();
                self.expected_after_apply = Some(completion.expected);
                if completion.awaiting_confirmation {
                    self.pending_archived_display_keys = archived;
                } else {
                    if let Some(overlays) = self.overlays.as_mut() {
                        overlays.set_display_preset_restoration_deferred(false, true);
                    }
                    self.pending_archived_display_keys.clear();
                }
                Some(active_uuids)
            }
            Err(error) => {
                if let Some(overlays) = self.overlays.as_mut() {
                    overlays.restore_displays_archived_for_failed_preset(&archived);
                    overlays.set_display_preset_restoration_deferred(false, true);
                }
                self.pending_archived_display_keys.clear();
                self.expected_after_apply =
                    current_display_configuration().ok().map(|configuration| {
                        ExpectedDisplayConfiguration::capture(&configuration.online)
                    });
                self.logger.event(&format!(
                    "화면 구성 적용 실패: source={} {error}",
                    source.log_name()
                ));
                if source == DisplayApplySource::Manual {
                    self.show_display_apply_warning("화면 구성을 적용하지 못했습니다", &error);
                }
                None
            }
        };
        let availability = self.display_apply_availability();
        if let Some(window) = self.settings_window.as_mut() {
            window.set_display_apply_availability(availability);
        }
        result
    }

    fn handle_display_confirmation(&mut self, decision: ConfirmationDecision) {
        let Some(resolution) = self.display_applier.resolve_confirmation(decision) else {
            self.logger
                .event("화면 구성 확인 결정 무시: 확인 대기 상태가 아님");
            return;
        };
        self.finish_display_resolution(resolution, true);
    }

    fn handle_display_expiration(&mut self, resolution: ConfirmationResolution) {
        let Some(resolution) = self.display_applier.finish_expiration(resolution) else {
            self.logger
                .event("화면 구성 만료 결과 무시: 확인 대기 상태가 아님");
            return;
        };
        self.finish_display_resolution(resolution, true);
    }

    fn finish_display_resolution(
        &mut self,
        resolution: ConfirmationResolution,
        resume_pending: bool,
    ) {
        self.logger.event(&format!(
            "화면 구성 확인 결과 반영: decision={:?} success={} restoredPrevious={}",
            resolution.decision, resolution.success, resolution.restored_previous
        ));
        if resolution.restored_previous || !resolution.success {
            if let Some(overlays) = self.overlays.as_mut() {
                overlays.restore_displays_archived_for_failed_preset(
                    &self.pending_archived_display_keys,
                );
            }
        }
        if let Some(overlays) = self.overlays.as_mut() {
            overlays.set_display_preset_restoration_deferred(false, resume_pending);
        }
        self.pending_archived_display_keys.clear();
        self.expected_after_apply = resolution.expected;
        if let Some(message) = resolution.message.as_deref() {
            let rollback_failed = resolution
                .message
                .as_deref()
                .is_some_and(|value| value.contains("복구 실패"));
            if rollback_failed {
                if !self.display_applier.present_rollback_failure(message) {
                    self.show_display_apply_warning(
                        "직전 화면 구성으로 되돌리지 못했습니다",
                        message,
                    );
                }
            } else {
                self.show_display_apply_warning("화면 구성 확인을 완료하지 못했습니다", message);
            }
        }
        let availability = self.display_apply_availability();
        if let Some(window) = self.settings_window.as_mut() {
            window.set_display_apply_availability(availability);
        }
    }

    fn show_display_apply_warning(&self, title: &str, message: &str) {
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning(title, message);
        } else {
            self.logger.event(&format!("{title}: {message}"));
        }
    }

    fn initialize_display_change_state(&mut self) {
        match current_display_configuration() {
            Ok(configuration) => {
                self.record_display_configuration(&configuration);
                self.display_change
                    .initialize(configuration.connected_uuids.clone());
                self.last_judged_uuids = Some(active_display_uuids(&configuration));
            }
            Err(error) => {
                self.logger
                    .event(&format!("display-query 초기 조회 실패: {error}"));
                self.restart_display_stability_timer("초기 조회 실패");
            }
        }
    }

    fn record_display_configuration(&mut self, configuration: &DisplayConfiguration) {
        self.display_target_table = configuration.target_table.clone();
        self.logger.event(&configuration.diagnostic_line());
    }

    fn schedule_sync(&mut self, reason: &str) {
        self.logger
            .event(&format!("디스플레이 변경 알림 수신: {reason}"));
        if self.zoom.is_some() {
            self.stop_zoom(reason, true);
        }
        // Reusing the same timer ID resets the 0.3 second overlay debounce.
        if unsafe {
            SetTimer(
                Some(self.hwnd),
                DISPLAY_SYNC_TIMER_ID,
                DISPLAY_SYNC_INTERVAL_MS,
                None,
            )
        } == 0
        {
            self.logger.event(&format!(
                "디스플레이 오버레이 동기화 타이머 시작 실패: {}",
                Error::from_thread()
            ));
        }
        self.restart_display_stability_timer(reason);
    }

    fn restart_display_stability_timer(&mut self, reason: &str) {
        let restarted = self.display_change.pending_generation().is_some();
        let generation = self.display_change.notify();
        let interval_ms = (DisplayPresetDefaults::CONNECTION_STABILITY_INTERVAL * 1_000.0) as u32;
        self.display_stability_deadline_ms =
            Some(monotonic_millis().saturating_add(u64::from(interval_ms)));
        // Reusing the timer ID restarts the independent five second stability interval.
        if unsafe {
            SetTimer(
                Some(self.hwnd),
                DISPLAY_STABILITY_TIMER_ID,
                interval_ms,
                None,
            )
        } == 0
        {
            self.display_change.cancel(generation);
            self.display_stability_deadline_ms = None;
            self.logger.event(&format!(
                "디스플레이 안정 타이머 시작 실패: {}",
                Error::from_thread()
            ));
            return;
        }
        self.logger.event(&format!(
            "디스플레이 안정 타이머 {}: delay={}s generation={} reason={reason}",
            if restarted { "재시작" } else { "시작" },
            DisplayPresetDefaults::CONNECTION_STABILITY_INTERVAL,
            generation,
        ));
    }

    fn handle_display_sync_timer(&mut self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), DISPLAY_SYNC_TIMER_ID) };
        if let Some(tray) = self.tray.as_mut() {
            tray.refresh_display("디스플레이 동기화 타이머");
        }
        self.synchronize_overlays("0.3초 디스플레이 동기화 타이머");
        if let Some(window) = self.settings_window.as_mut() {
            window.refresh_display_configuration("모니터 구성 변경 알림");
        }
    }

    fn handle_display_stability_timer(&mut self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), DISPLAY_STABILITY_TIMER_ID) };
        let now_ms = monotonic_millis();
        if let Some(deadline_ms) = self.display_stability_deadline_ms {
            if now_ms < deadline_ms {
                let remaining_ms = u32::try_from(deadline_ms - now_ms).unwrap_or(u32::MAX);
                // A WM_TIMER from the replaced generation can already be queued; wait to the
                // newest deadline before consulting the connection set.
                let timer = unsafe {
                    SetTimer(
                        Some(self.hwnd),
                        DISPLAY_STABILITY_TIMER_ID,
                        remaining_ms.max(1),
                        None,
                    )
                };
                if timer == 0 {
                    self.display_stability_deadline_ms = None;
                    self.logger.event(&format!(
                        "디스플레이 안정 타이머 재대기 실패: {}",
                        Error::from_thread()
                    ));
                } else {
                    return;
                }
            }
        }
        self.display_stability_deadline_ms = None;
        let Some(generation) = self.display_change.pending_generation() else {
            self.logger
                .event("디스플레이 안정 타이머 만료 무시: pending 없음");
            return;
        };
        self.logger.event(&format!(
            "디스플레이 안정 타이머 만료: generation={generation}"
        ));
        let configuration = match current_display_configuration() {
            Ok(configuration) => configuration,
            Err(error) => {
                self.display_change.cancel(generation);
                self.logger
                    .event(&format!("display-query 안정 후 조회 실패: {error}"));
                return;
            }
        };
        self.record_display_configuration(&configuration);
        if configuration.unavailable_target_count > 0 {
            self.logger.event(&format!(
                "디스플레이 안정 판정 보류: targetAvailable=FALSE count={}",
                configuration.unavailable_target_count
            ));
            self.restart_display_stability_timer("targetAvailable=FALSE");
            return;
        }
        if let Some(expected) = self.expected_after_apply.take() {
            if expected.matches(&configuration.online) {
                let outcome = self
                    .display_change
                    .settle(generation, &configuration.connected_uuids);
                self.logger.event(&format!(
                    "디스플레이 자체 변경 무시: 기대 상태와 일치 generation={generation} settle={outcome:?}"
                ));
                return;
            }
            self.logger
                .event("디스플레이 변경은 적용 기대 상태와 다름: 사용자/외부 변경으로 계속 판정");
        }
        match self
            .display_change
            .settle(generation, &configuration.connected_uuids)
        {
            ConnectionSetOutcome::Stale => self.logger.event(&format!(
                "디스플레이 안정 판정 무시: stale generation={generation}"
            )),
            ConnectionSetOutcome::Unchanged => self
                .logger
                .event("디스플레이 자동 전환 판단=Ignore reason=연결 집합 동일"),
            ConnectionSetOutcome::Changed => {
                self.judge_display_auto_switch(&configuration, "연결 집합 변경")
            }
        }
    }

    fn judge_display_auto_switch(&mut self, configuration: &DisplayConfiguration, reason: &str) {
        let current_uuids = active_display_uuids(configuration);
        match is_console_session() {
            Ok(true) => {}
            Ok(false) => {
                self.last_judged_uuids = Some(current_uuids);
                self.logger.event(&format!(
                    "디스플레이 자동 전환 판단 비활성: RDP/비콘솔 세션 reason={reason}"
                ));
                return;
            }
            Err(error) => {
                self.logger.event(&format!(
                    "디스플레이 자동 전환 세션 확인 실패: {error} reason={reason}"
                ));
                return;
            }
        }
        let now = monotonic_seconds();
        let decision = DisplayAutoSwitchPolicy::decide(
            &configuration.online,
            &self.settings.display.presets,
            self.settings.display.auto_switch_enabled,
            self.last_judged_uuids.as_ref(),
            self.last_auto_apply_at,
            now,
            DisplayPresetDefaults::AUTOMATIC_APPLY_COOLDOWN,
        );
        match decision {
            DisplayAutoSwitchDecision::Ignore => {
                self.last_judged_uuids = Some(current_uuids);
                self.logger.event(&format!(
                    "디스플레이 자동 전환 판단=Ignore reason={reason} enabled={}",
                    self.settings.display.auto_switch_enabled
                ));
            }
            DisplayAutoSwitchDecision::Apply(index) => {
                let replaced = self.pending_display_apply.replace(index);
                if let Some(preset) = self.settings.display.presets.get(index) {
                    self.logger.event(&format!(
                        "디스플레이 자동 전환 판단=Apply 자동 전환 대상: presetHash={} index={} replacedPending={:?}",
                        hash_prefix8(&preset.name),
                        index,
                        replaced,
                    ));
                } else {
                    self.logger.event(&format!(
                        "디스플레이 자동 전환 판단=Apply 인덱스 범위 오류: index={index}"
                    ));
                }
                let pending = self.pending_display_apply.take();
                if let Some(index) = pending {
                    if let Some(applied_uuids) =
                        self.apply_display_preset(index, DisplayApplySource::Automatic)
                    {
                        self.last_auto_apply_at = Some(monotonic_seconds());
                        self.last_judged_uuids = Some(applied_uuids);
                        self.logger.event(&format!(
                            "디스플레이 자동 전환 적용 성공: index={index} lastAutoApplyAt/lastJudgedUUIDs 갱신"
                        ));
                    }
                }
            }
            DisplayAutoSwitchDecision::Retry(after) => {
                self.logger.event(&format!(
                    "디스플레이 자동 전환 판단=Retry after={after:.3}s reason={reason}"
                ));
                self.schedule_display_retry(after);
            }
        }
    }

    fn schedule_display_retry(&self, after: f64) {
        let delay_ms = (after.max(0.001) * 1_000.0).ceil().min(f64::from(u32::MAX)) as u32;
        if unsafe { SetTimer(Some(self.hwnd), DISPLAY_RETRY_TIMER_ID, delay_ms, None) } == 0 {
            self.logger.event(&format!(
                "디스플레이 자동 전환 재시도 타이머 시작 실패: {}",
                Error::from_thread()
            ));
        }
    }

    fn handle_display_retry_timer(&mut self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), DISPLAY_RETRY_TIMER_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), PRESENTER_FRAME_TIMER_ID) };
        self.cancel_pass_through_timers();
        let configuration = match current_display_configuration() {
            Ok(configuration) => configuration,
            Err(error) => {
                self.logger
                    .event(&format!("display-query 재시도 조회 실패: {error}"));
                return;
            }
        };
        self.record_display_configuration(&configuration);
        if configuration.unavailable_target_count > 0 {
            self.restart_display_stability_timer("재시도 중 targetAvailable=FALSE");
            return;
        }
        self.judge_display_auto_switch(&configuration, "쿨다운 재시도");
    }

    fn synchronize_overlays(&mut self, reason: &str) {
        self.cancel_text_entry("모니터 변경");
        if let Some(overlays) = self.overlays.as_mut() {
            let synchronize_error = overlays.synchronize(reason).err();
            let board_error = synchronize_error
                .is_none()
                .then(|| overlays.refresh_board(&self.settings).err())
                .flatten();
            if let Some(error) = synchronize_error {
                self.report_overlay_error("모니터 재구성", &error);
            }
            if let Some(error) = board_error {
                self.report_overlay_error("모니터 재구성 보드 반영", &error);
            }
        }
        if let Some(palette) = self.palette.as_mut() {
            let result =
                palette.display_configuration_changed(&self.state.borrow(), &self.settings);
            if let Err(error) = result {
                self.report_palette_error("모니터 재구성", &error);
            }
        }
        if let Some(radial) = self.radial_palette.as_mut() {
            if let Err(error) =
                radial.display_configuration_changed(&self.state.borrow(), &self.settings)
            {
                self.report_radial_palette_error("모니터 재구성", &error);
            }
        }
        if let Some(badge) = self.lesson_badge.as_mut() {
            if let Err(error) = badge.display_configuration_changed(reason) {
                self.report_lesson_badge_error("모니터 재구성", &error);
            }
        }
        if let Some(help) = self.help.as_mut() {
            help.display_configuration_changed(&self.state.borrow(), &self.settings);
        }
        let result = self
            .break_timer
            .as_mut()
            .map(|timer| timer.display_configuration_changed(reason));
        if let Some(Err(error)) = result {
            self.report_break_timer_error("모니터 재구성", &error);
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
        if !item_enabled(item, &self.state.borrow(), &self.settings) {
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
        let pass_through_command = matches!(&command, ShortcutCommand::TogglePassThroughMode);
        let pass_through_already_enabled = self.state.borrow().is_pass_through_mode_enabled();
        let pass_through_monitor_available = !pass_through_command
            || pass_through_already_enabled
            || self.prepare_pass_through_mouse_hook("선형 팔레트 통과 모드");
        let context = DispatchContext {
            scope: ShortcutScope::Annotation,
            display_id: display_id.as_deref(),
            board_display_id: display_id.as_deref(),
            spotlight_adjusted_width: self.presenter.spotlight_enabled(),
            now: monotonic_seconds(),
            pass_through_monitor_available,
            ..DispatchContext::default()
        };
        let result = dispatch(
            &command,
            &mut self.state.borrow_mut(),
            &self.settings,
            context,
        );
        match result {
            Ok(effects) => self.apply_effects(effects, &format!("팔레트 {item:?}")),
            Err(error) => self.logger.event(&format!(
                "팔레트 core dispatch 실패: item={item:?} command={command:?}: {error}"
            )),
        }
        if pass_through_command {
            self.synchronize_presenter_mouse_hook("선형 팔레트 dispatch 완료");
        }
    }

    fn refresh_palette(&mut self, reason: &str) {
        if let Some(palette) = self.palette.as_mut() {
            let result = palette.refresh(&self.state.borrow(), &self.settings);
            if let Err(error) = result {
                self.report_palette_error(reason, &error);
            }
        }
    }

    fn refresh_help(&mut self, reason: &str) {
        if let Some(help) = self.help.as_mut() {
            help.refresh(&self.state.borrow(), &self.settings, reason);
        }
    }

    fn handle_help_action(&mut self, value: usize) {
        let action = self
            .help
            .as_ref()
            .and_then(|help| help.action(value, &self.state.borrow()));
        let Some((definition, scope)) = action else {
            self.logger
                .event(&format!("도움말 항목 메시지 범위 밖: {value}"));
            return;
        };
        self.logger.event(&format!(
            "도움말 항목 실행: id={} scope={}",
            definition.id,
            scope.as_str()
        ));
        let display_id = self
            .overlays
            .as_ref()
            .and_then(|overlays| overlays.monitor_key_at_cursor().ok())
            .flatten();
        let pass_through_command =
            matches!(&definition.command, ShortcutCommand::TogglePassThroughMode);
        let pass_through_already_enabled = self.state.borrow().is_pass_through_mode_enabled();
        let pass_through_monitor_available = !pass_through_command
            || pass_through_already_enabled
            || self.prepare_pass_through_mouse_hook("도움말 통과 모드");
        let context = DispatchContext {
            scope,
            display_id: display_id.as_deref(),
            board_display_id: display_id.as_deref(),
            help_visible: true,
            spotlight_adjusted_width: self.presenter.spotlight_enabled(),
            now: monotonic_seconds(),
            pass_through_monitor_available,
            ..DispatchContext::default()
        };
        let dispatched = dispatch(
            &definition.command,
            &mut self.state.borrow_mut(),
            &self.settings,
            context,
        );
        match dispatched {
            Ok(effects) => self.apply_effects(effects, &format!("도움말 항목 {}", definition.id)),
            Err(error) => self.logger.event(&format!(
                "도움말 항목 core dispatch 실패: id={} scope={}: {error}",
                definition.id,
                scope.as_str()
            )),
        }
        if pass_through_command {
            self.synchronize_presenter_mouse_hook("도움말 dispatch 완료");
        }
        if let Some(help) = self.help.as_mut() {
            help.close(&self.state.borrow(), "항목 실행");
        }
    }

    fn poll_help_release(&mut self) {
        if let Some(help) = self.help.as_mut() {
            let _ =
                help.poll_global_release(monotonic_millis(), &self.state.borrow(), &self.settings);
        }
    }

    fn close_help(&mut self, reason: &str) {
        if let Some(help) = self.help.as_mut() {
            help.close(&self.state.borrow(), reason);
        }
    }

    fn pretranslate_help_message(&mut self, message: &MSG) -> bool {
        let Some(help) = self.help.as_ref() else {
            return false;
        };
        let matches_step_back = (message.message == WM_KEYDOWN)
            .then(|| keymap::vk_to_token(message.wParam.0 as u32))
            .flatten()
            .and_then(|key| {
                ShortcutCatalog::resolved_definitions(&self.settings.shortcuts)
                    .into_iter()
                    .find(|definition| definition.command == ShortcutCommand::StepBack)
                    .and_then(|definition| {
                        definition
                            .binding(ShortcutScope::Annotation)
                            .is_some_and(|binding| {
                                binding.matches(key, crate::kbhook::current_modifiers())
                            })
                            .then_some(())
                    })
            })
            .is_some();
        if help.is_pinned() && help.owns_hwnd(message.hwnd) && matches_step_back {
            self.close_help("한 단계 나가기 키");
            return true;
        }
        false
    }

    fn redraw_palette(&mut self) {
        if let Some(palette) = self.palette.as_mut() {
            let result = palette.redraw(&self.state.borrow(), &self.settings);
            if let Err(error) = result {
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
            let result = palette.toggle_orientation(&self.state.borrow(), &self.settings);
            if let Err(error) = result {
                self.report_palette_error("가로/세로 전환", &error);
            }
        }
    }

    fn handle_radial_action(&mut self) {
        let pending = self
            .radial_palette
            .as_mut()
            .and_then(RadialPaletteManager::take_pending_action);
        let Some((target, intent, secondary)) = pending else {
            self.logger
                .event("원형 팔레트 동작 메시지에 pending intent 없음");
            return;
        };
        self.logger.event(&format!(
            "원형 팔레트 칸 클릭: {target} -> {intent:?} secondary={secondary}"
        ));
        self.execute_radial_intent(intent, &target);
    }

    fn execute_radial_intent(&mut self, intent: RadialPaletteIntent, target: &str) {
        match intent {
            RadialPaletteIntent::ToggleExpansion => {
                if let Some(radial) = self.radial_palette.as_mut() {
                    if let Err(error) = radial.toggle_expansion("중심 클릭") {
                        self.report_radial_palette_error("중심 클릭", &error);
                    }
                }
            }
            RadialPaletteIntent::SelectGroup(group) => {
                if let Some(radial) = self.radial_palette.as_mut() {
                    if let Err(error) = radial.select_group(group) {
                        self.report_radial_palette_error("그룹 선택", &error);
                    }
                }
            }
            RadialPaletteIntent::Execute(command) => {
                self.dispatch_radial_command(command, target);
            }
            RadialPaletteIntent::SelectEraserMode(mode) => {
                if self.state.borrow().current_tool() != InkTool::Eraser {
                    self.dispatch_radial_command(ShortcutCommand::SelectOrCycleEraser, target);
                }
                if self.settings.eraser.mode != eraser_mode_name(mode) {
                    self.apply_effects(
                        vec![Effect::SettingsMutation(SettingsMutation::SetEraserMode(
                            mode,
                        ))],
                        &format!("원형 팔레트 {target}"),
                    );
                }
            }
            RadialPaletteIntent::ToggleEraserAreaShape => {
                self.dispatch_radial_command(ShortcutCommand::ToggleEraserAreaShape, target);
            }
            RadialPaletteIntent::SelectColor(index) => {
                let selected_mutation = self.state.borrow_mut().select_color(index, &self.settings);
                if let Some(mutation) = selected_mutation {
                    self.apply_effects(
                        vec![Effect::SettingsMutation(mutation)],
                        &format!("원형 팔레트 {target}"),
                    );
                }
            }
            RadialPaletteIntent::SetWidth { tool, value } => {
                let selected_mutation = self.state.borrow_mut().set_width(tool, value);
                if let Some(mutation) = selected_mutation {
                    self.apply_effects(
                        vec![Effect::SettingsMutation(mutation)],
                        &format!("원형 팔레트 {target}"),
                    );
                }
            }
            RadialPaletteIntent::SetEraserDiameter(value) => self.apply_effects(
                vec![Effect::SettingsMutation(
                    SettingsMutation::SetEraserDiameter(value),
                )],
                &format!("원형 팔레트 {target}"),
            ),
            RadialPaletteIntent::SetLaserColor { index, .. } => {
                let selected_mutation = self.state.borrow_mut().select_color(index, &self.settings);
                if let Some(mutation) = selected_mutation {
                    self.apply_effects(
                        vec![Effect::SettingsMutation(mutation)],
                        &format!("원형 팔레트 {target}"),
                    );
                } else {
                    self.logger.event(&format!(
                        "원형 팔레트 레이저 색 경로 거부: target={target} index={index}"
                    ));
                }
            }
            RadialPaletteIntent::OpenSettings => {
                self.apply_effects(vec![Effect::OpenSettings], "원형 팔레트");
            }
            RadialPaletteIntent::SaveFavorite => {
                let result = self.state.borrow().save_current_favorite(&self.settings);
                match result {
                    Ok(FavoriteSaveResult::Saved(mutation)) => {
                        if self.update_settings("radial-favorite-save", move |settings| {
                            mutation.apply(settings);
                        }) {
                            self.logger.event("원형 팔레트 즐겨찾기 저장 완료");
                        }
                    }
                    Ok(FavoriteSaveResult::UnsupportedTool) => {
                        self.logger.event("즐겨찾기 저장 거부: 지원하지 않는 도구");
                    }
                    Ok(FavoriteSaveResult::Duplicate) => {
                        self.logger.event("즐겨찾기 저장 거부: 중복");
                    }
                    Ok(FavoriteSaveResult::Full) => {
                        self.logger.event("즐겨찾기 저장 거부: 최대 개수 도달");
                    }
                    Err(error) => self
                        .logger
                        .event(&format!("즐겨찾기 저장 상태 판정 실패: {error}")),
                }
            }
            RadialPaletteIntent::ApplyFavorite { index, favorite } => {
                let previous_state = self.state.borrow().clone();
                let result = self
                    .state
                    .borrow_mut()
                    .apply_favorite(&favorite, &self.settings);
                match result {
                    Ok(Some(mutations)) => {
                        let settings_changed =
                            self.update_settings("radial-favorite-apply", move |settings| {
                                for mutation in mutations {
                                    mutation.apply(settings);
                                }
                            });
                        let state_changed = self.state.borrow().ne(&previous_state);
                        if state_changed && !settings_changed {
                            self.refresh_runtime_state("즐겨찾기 적용");
                        }
                        self.logger.event(&format!(
                            "원형 팔레트 즐겨찾기 적용: index={index} stateChanged={state_changed} settingsChanged={settings_changed}"
                        ));
                    }
                    Ok(None) => self.logger.event("즐겨찾기 적용 거부: 잘못된 항목"),
                    Err(error) => self
                        .logger
                        .event(&format!("즐겨찾기 적용 상태 전이 실패: {error}")),
                }
            }
            RadialPaletteIntent::RemoveFavorite { index, favorite } => {
                let matches = self.settings.tools.favorites.get(index) == Some(&favorite);
                if !matches {
                    self.logger
                        .event("즐겨찾기 삭제 거부: 현재 항목과 요청이 일치하지 않음");
                    return;
                }
                let mut favorites = self.settings.tools.favorites.clone();
                favorites.remove(index);
                if self.update_settings("radial-favorite-remove", move |settings| {
                    SettingsMutation::SetFavorites(favorites).apply(settings);
                }) {
                    self.logger
                        .event(&format!("원형 팔레트 즐겨찾기 삭제 완료: index={index}"));
                }
            }
        }
    }

    fn dispatch_radial_command(&mut self, command: ShortcutCommand, target: &str) {
        let display_id = match self
            .radial_palette
            .as_ref()
            .map(RadialPaletteManager::monitor_key)
        {
            Some(Ok(display_id)) => display_id,
            Some(Err(error)) => {
                self.report_radial_palette_error("대상 모니터 판정", &error);
                None
            }
            None => None,
        };
        let pass_through_command = matches!(&command, ShortcutCommand::TogglePassThroughMode);
        let pass_through_already_enabled = self.state.borrow().is_pass_through_mode_enabled();
        let pass_through_monitor_available = !pass_through_command
            || pass_through_already_enabled
            || self.prepare_pass_through_mouse_hook("원형 팔레트 통과 모드");
        let context = DispatchContext {
            scope: ShortcutScope::Annotation,
            display_id: display_id.as_deref(),
            board_display_id: display_id.as_deref(),
            radial_palette_expanded: self
                .radial_palette
                .as_ref()
                .is_some_and(RadialPaletteManager::is_expanded),
            spotlight_adjusted_width: self.presenter.spotlight_enabled(),
            now: monotonic_seconds(),
            pass_through_monitor_available,
            ..DispatchContext::default()
        };
        let dispatched = dispatch(
            &command,
            &mut self.state.borrow_mut(),
            &self.settings,
            context,
        );
        match dispatched {
            Ok(effects) => {
                self.apply_effects(effects, &format!("원형 팔레트 {target}"));
                if matches!(
                    command,
                    ShortcutCommand::SelectTool(_)
                        | ShortcutCommand::UseLaser
                        | ShortcutCommand::SelectOrCycleEraser
                ) {
                    self.logger
                        .event("원형 팔레트 펼침 유지: 도구 선택 (Swift 동작 대칭)");
                }
            }
            Err(error) => self.logger.event(&format!(
                "원형 팔레트 core dispatch 실패: target={target} command={command:?}: {error}"
            )),
        }
        if pass_through_command {
            self.synchronize_presenter_mouse_hook("원형 팔레트 dispatch 완료");
        }
    }

    fn refresh_radial_palette(&mut self, reason: &str) {
        let (hidden, error) = if let Some(radial) = self.radial_palette.as_mut() {
            let mut error = radial
                .refresh(&self.state.borrow(), &self.settings, reason)
                .err();
            if error.is_none() {
                error = radial
                    .set_presenter_effects(
                        self.presenter.spotlight_enabled(),
                        self.presenter.cursor_highlight_enabled(),
                    )
                    .err();
            }
            (!radial.is_visible(), error)
        } else {
            (true, None)
        };
        if let Some(error) = error {
            self.report_radial_palette_error(reason, &error);
        }
        if hidden && self.radial_key_cap_state != RadialKeyCapState::Idle {
            self.reset_radial_key_caps("팔레트 숨김");
        }
    }

    fn redraw_radial_palette(&mut self) {
        if let Some(radial) = self.radial_palette.as_mut() {
            if let Err(error) = radial.redraw() {
                self.report_radial_palette_error("다시 그리기", &error);
            }
        }
    }

    fn save_radial_palette_position(&mut self) {
        if let Some(radial) = self.radial_palette.as_mut() {
            if let Err(error) = radial.save_position() {
                self.report_radial_palette_error("위치 저장", &error);
            }
        }
    }

    fn collapse_radial_for_external_pointer_down(&mut self, hwnd: HWND, message: u32) -> bool {
        let should_collapse = message == windows::Win32::UI::WindowsAndMessaging::WM_POINTERDOWN
            && self
                .radial_palette
                .as_ref()
                .is_some_and(|radial| radial.is_expanded() && radial.hwnd() != hwnd);
        if !should_collapse {
            return false;
        }
        if let Some(radial) = self.radial_palette.as_mut() {
            if let Err(error) = radial.collapse("바깥 클릭") {
                self.report_radial_palette_error("바깥 클릭 접기", &error);
            }
        }
        self.logger.event(
            "원형 팔레트 바깥 클릭 메시지 폐기: Swift local monitor return nil 및 plan §2.3 대칭",
        );
        true
    }

    fn report_radial_palette_error(&self, context: &str, error: &Error) {
        let message = format!("원형 팔레트 {context} 실패: {error}");
        self.logger.event(&message);
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning("원형 팔레트 오류", &message);
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

    fn report_lesson_badge_error(&self, context: &str, error: &Error) {
        let message = format!("수업 배지 {context} 실패: {error}");
        self.logger.event(&message);
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning("수업 타이머 오류", &message);
        }
    }

    fn report_break_timer_error(&self, context: &str, error: &Error) {
        let message = format!("쉬는 시간 타이머 {context} 실패: {error}");
        self.logger.event(&message);
        if let Some(tray) = self.tray.as_ref() {
            tray.show_warning("쉬는 시간 타이머 오류", &message);
        }
    }

    fn handle_break_start_outcome(&mut self, outcome: BreakStartOutcome) {
        match outcome {
            BreakStartOutcome::Started => {
                if let Some(music) = self.music.as_mut() {
                    music.break_did_start(&self.settings);
                }
            }
            BreakStartOutcome::Restarted => {
                if let Some(music) = self.music.as_mut() {
                    music.break_did_start(&self.settings);
                }
            }
            BreakStartOutcome::Changed => {
                let timing = self
                    .break_timer
                    .as_ref()
                    .and_then(BreakTimerManager::running_timing);
                if let (Some((now, end_date)), Some(music)) = (timing, self.music.as_mut()) {
                    music.break_did_update(&self.settings, now, end_date);
                }
            }
            BreakStartOutcome::Warned => {}
        }
    }

    fn handle_break_action(&mut self, action: usize) {
        if BreakTimerManager::action_start_setup(action) {
            let result = self
                .break_timer
                .as_mut()
                .map(|timer| timer.start_from_setup(&self.settings));
            match result {
                Some(Ok(outcome)) => self.handle_break_start_outcome(outcome),
                Some(Err(error)) => self.report_break_timer_error("설정 시작", &error),
                None => {}
            }
        } else if BreakTimerManager::action_change_end(action) {
            let result = self
                .break_timer
                .as_mut()
                .map(|timer| timer.open_setup(&self.settings));
            if let Some(Err(error)) = result {
                self.report_break_timer_error("종료 시각 변경 창 열기", &error);
            }
        } else if BreakTimerManager::action_toggle_collapsed(action) {
            let result = self
                .break_timer
                .as_mut()
                .map(BreakTimerManager::toggle_collapsed);
            if let Some(Err(error)) = result {
                self.report_break_timer_error("접기/펼치기", &error);
            }
        } else if BreakTimerManager::action_end_early(action) {
            if let Some(music) = self.music.as_mut() {
                music.break_did_end();
            }
            if let Some(timer) = self.break_timer.as_mut() {
                timer.end_early();
            }
        } else if BreakTimerManager::action_toggle_music(action) {
            let enabled = BreakMusicManager::music_enabled_after_toggle(&self.settings);
            self.update_settings("break-screen-music-toggle", move |settings| {
                settings.timer.set_music_enabled(enabled);
            });
        } else if let Some(index) = BreakTimerManager::playlist_index_from_action(action) {
            if let Some(index) =
                BreakMusicManager::validated_playlist_selection(&self.settings, index)
            {
                self.update_settings("break-screen-playlist", move |settings| {
                    settings.timer.select_playlist(index);
                });
            } else {
                self.logger
                    .event(&format!("재생목록 선택 거부: index={index} 범위 밖"));
            }
        } else {
            self.logger
                .event(&format!("쉬는 시간 화면 알 수 없는 동작: {action}"));
        }
    }

    fn start_quick_break(&mut self, minutes: i64, source: &str) {
        let was_running = self
            .break_timer
            .as_ref()
            .is_some_and(BreakTimerManager::is_running);
        if was_running {
            if let Some(music) = self.music.as_mut() {
                music.break_did_end();
            }
        }
        let result = self
            .break_timer
            .as_mut()
            .map(|timer| timer.start_quick(minutes, &self.settings, source));
        match result {
            Some(Ok(outcome)) => self.handle_break_start_outcome(outcome),
            Some(Err(error)) => self.report_break_timer_error("빠른 시작", &error),
            None => self
                .logger
                .event("빠른 쉬는 시간 요청 소비 실패: 타이머 관리자가 없음"),
        }
    }

    fn handle_break_tick(&mut self, reason: &str) {
        let result = self
            .break_timer
            .as_mut()
            .map(|timer| timer.tick(&self.settings));
        match result {
            Some(Ok(outcome)) if outcome.did_finish => {
                if let Some(music) = self.music.as_mut() {
                    music.break_did_end();
                }
                play_completion_sound(&self.settings.timer.notification_sound, &self.logger);
            }
            Some(Ok(outcome)) => {
                if let (Some(end_date), Some(music)) =
                    (outcome.running_end_date, self.music.as_mut())
                {
                    music.break_did_update(&self.settings, outcome.now, end_date);
                }
            }
            Some(Err(error)) => self.report_break_timer_error(reason, &error),
            None => {}
        }
    }

    fn refresh_break_settings(&mut self, reason: &str) {
        let result = self
            .break_timer
            .as_mut()
            .map(|timer| timer.apply_settings(&self.settings));
        if let Some(Err(error)) = result {
            self.report_break_timer_error(reason, &error);
        }
    }

    fn handle_lesson_action(&mut self, command: u16) {
        let result = self
            .lesson_badge
            .as_mut()
            .map(|badge| badge.handle_menu_action(command, &self.settings));
        match result {
            Some(Ok(Some(mutation))) => self.apply_lesson_settings_mutation(mutation),
            Some(Ok(None)) | None => {}
            Some(Err(error)) => self.report_lesson_badge_error("메뉴 실행", &error),
        }
    }

    fn apply_lesson_settings_mutation(&mut self, mutation: LessonSettingsMutation) {
        self.update_settings("lesson-badge-menu", move |settings| {
            mutation.apply(settings);
        });
    }

    fn handle_lesson_completion(&mut self, minutes: i64) {
        if minutes > 0 {
            self.pending_break_request = Some(minutes);
            self.logger.event(&format!(
                "수업 타이머 자동 쉬는 시간 pending 요청 노출: minutes={minutes} consumer=118d"
            ));
            let request = self.pending_break_request.take();
            if let Some(minutes) = request {
                self.start_quick_break(minutes, "자동");
            }
        } else {
            self.logger
                .event("수업 타이머 완료: 자동 쉬는 시간 pending 요청 없음");
        }
    }

    fn synchronize_lesson_badge(&mut self, reason: &str) {
        let result = self
            .lesson_badge
            .as_mut()
            .map(|badge| badge.synchronize_after_system_event(reason));
        if let Some(Err(error)) = result {
            self.report_lesson_badge_error("시스템 이벤트 동기화", &error);
        }
    }

    fn begin_text_entry(&mut self, request: TextEntryRequest) {
        self.cancel_text_entry("새 입력");
        if !self
            .overlays
            .as_ref()
            .is_some_and(|overlays| overlays.contains_monitor(&request.monitor_key))
        {
            self.logger.event(&format!(
                "텍스트 편집 취소: reason=모니터 변경 monitor={}",
                request.monitor_key
            ));
            return;
        }
        match TextEntryWindow::open(self.hwnd, self.hinstance, request, self.logger.clone()) {
            Ok(entry) => self.text_entry = Some(entry),
            Err(error) => {
                let message = format!("텍스트 편집 창 열기 실패: {error}");
                self.logger.event(&message);
                if let Some(tray) = self.tray.as_ref() {
                    tray.show_warning("텍스트 도구 오류", &message);
                }
            }
        }
    }

    fn commit_text_entry(&mut self) {
        let commit = match self.text_entry.as_ref().map(TextEntryWindow::commit) {
            Some(Ok(commit)) => commit,
            Some(Err(error)) => {
                self.logger
                    .event(&format!("텍스트 편집 측정 실패: {error}"));
                self.cancel_text_entry("측정 오류");
                return;
            }
            None => return,
        };
        if commit.string.trim().is_empty() {
            self.cancel_text_entry("빈 문자열");
            return;
        }

        let character_count = commit.string.chars().count();
        let measured = commit.measured;
        let monitor_key = commit.request.monitor_key.clone();
        if let Some(mut entry) = self.text_entry.take() {
            if let Err(error) = entry.close() {
                self.logger
                    .event(&format!("텍스트 편집 창 확정 종료 실패: {error}"));
            }
        }
        let result = self
            .overlays
            .as_mut()
            .map(|overlays| overlays.add_text_stroke(commit));
        match result {
            Some(Ok(text_size)) => self.logger.event(&format!(
                "텍스트 확정: monitor={monitor_key} 글자수={character_count} 측정={:.2}x{:.2}DIP textSize={:.2}x{:.2}DIP Undo=1",
                measured.width, measured.height, text_size.width, text_size.height
            )),
            Some(Err(error)) => self.report_overlay_error("텍스트 스트로크 추가", &error),
            None => self
                .logger
                .event("텍스트 스트로크 추가 실패: 오버레이 관리자가 없음"),
        }
        self.render_zoom("텍스트 확정");
    }

    fn cancel_text_entry(&mut self, reason: &str) {
        let Some(mut entry) = self.text_entry.take() else {
            return;
        };
        if let Err(error) = entry.close() {
            self.logger
                .event(&format!("텍스트 편집 창 취소 종료 실패: {error}"));
        }
        self.logger
            .event(&format!("텍스트 편집 취소: reason={reason}"));
    }

    fn handle_tray_callback(&mut self, event: u32) {
        if event != WM_CONTEXTMENU && event != WM_RBUTTONUP {
            return;
        }
        let display_entries = self
            .display_applier
            .menu_entries(&self.settings.display.presets);
        let selected = self
            .tray
            .as_ref()
            .and_then(|tray| tray.show_menu(&display_entries));
        match selected {
            Some(MENU_TOGGLE_ANNOTATION) => {
                self.logger.event("트레이 메뉴 선택: 판서 모드");
                self.toggle_annotation("트레이 메뉴");
            }
            Some(MENU_SETTINGS) => {
                self.logger.event("트레이 메뉴 선택: 설정");
                self.show_settings(SettingsTab::General, "트레이 메뉴");
            }
            Some(MENU_DISPLAY_SETTINGS) => {
                self.logger.event("트레이 메뉴 선택: 프리셋 편집…");
                self.show_display_settings("트레이 화면 구성 하위 메뉴");
            }
            Some(MENU_EXIT) => {
                self.logger.event("트레이 메뉴 선택: EduTools 종료");
                // SAFETY: hwnd is this app's hidden top-level window.
                if let Err(error) = unsafe { DestroyWindow(self.hwnd) } {
                    self.logger.event(&format!("메인 창 종료 실패: {error}"));
                }
            }
            Some(command) if display_preset_index(command).is_some() => {
                let index = display_preset_index(command).expect("guarded display preset index");
                self.logger.event(&format!(
                    "트레이 메뉴 선택: 화면 구성 index={} source=manual",
                    index
                ));
                self.apply_display_preset(index, DisplayApplySource::Manual);
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
        if let Some(resolution) = self.display_applier.rollback_for_termination() {
            self.finish_display_resolution(resolution, false);
        }
        let _ = unsafe { KillTimer(Some(self.hwnd), DISPLAY_SYNC_TIMER_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), DISPLAY_STABILITY_TIMER_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), DISPLAY_RETRY_TIMER_ID) };
        if let Some(notification) = self.device_notification.take() {
            // SAFETY: notification was returned for this live hidden window and is released once.
            if let Err(error) = unsafe { UnregisterDeviceNotification(notification) } {
                self.logger
                    .event(&format!("디스플레이 장치 알림 해제 실패: {error}"));
            }
        }
        if let Some(music) = self.music.as_mut() {
            music.stop_music_for_termination();
        }
        self.cancel_text_entry("앱 종료");
        self.export.shutdown();
        self.zoom = None;
        self.state.borrow_mut().end_zoom();
        self.uninstall_keyboard_hook_for_shutdown();
        if let Err(error) = self.presenter.shutdown() {
            self.logger
                .event(&format!("앱 종료 중 발표 키보드 훅 해제 실패: {error}"));
        }
        if let Err(error) = self.mouse_hook.uninstall() {
            self.logger
                .event(&format!("앱 종료 중 전역 마우스 훅 해제 실패: {error}"));
        } else {
            self.logger.event("전역 마우스 훅 해제: reason=앱 종료");
        }
        // Overlay HWNDs and graphics resources must be released on the UI thread before the tray
        // and hidden owner window go away.
        self.radial_palette = None;
        self.settings_window = None;
        self.help = None;
        if let Some(badge) = self.lesson_badge.as_mut() {
            badge.hide("앱 종료");
        }
        self.lesson_badge = None;
        self.break_timer = None;
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

fn active_display_uuids(configuration: &DisplayConfiguration) -> HashSet<String> {
    configuration
        .online
        .iter()
        .filter_map(|display| (!display.uuid.is_empty()).then(|| display.uuid.clone()))
        .collect()
}

fn run_message_loop(logger: Logger, initial_import_path: Option<PathBuf>) -> Result<i32> {
    // SAFETY: None requests the current executable module handle.
    let module = unsafe { GetModuleHandleW(None) }?;
    let hinstance = HINSTANCE(module.0);
    let class_icon =
        match tray::load_shared_resource_icon(hinstance, tray::APP_ICON_RESOURCE_ID, 32) {
            Ok(icon) => {
                logger.event(&format!(
                    "앱 아이콘 리소스 로드 성공: id={}",
                    tray::APP_ICON_RESOURCE_ID
                ));
                icon
            }
            Err(error) => {
                logger.event(&format!(
                    "앱 아이콘 리소스 로드 실패: id={} error={error}",
                    tray::APP_ICON_RESOURCE_ID
                ));
                HICON::default()
            }
        };
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: hinstance,
        hIcon: class_icon,
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
            WS_POPUP,
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

    let settings_storage = FileSettingsStorage::app_settings()
        .map_err(|error| Error::new(E_FAIL, error.to_string()))?;
    let settings_file_existed_at_launch = settings_storage.path().exists();
    let mut settings_store = SettingsStore::load(settings_storage);
    let settings = settings_store.settings().clone();
    let startup_settings_issue = settings_store.take_issue();
    let (settings_backups, startup_backup_issue) = match FileSettingsStorage::app_backups() {
        Ok(storage) => match SettingsBackupStore::load(storage) {
            Ok(backups) => (Some(backups), None),
            Err(error) => (None, Some(error.to_string())),
        },
        Err(error) => (None, Some(error.to_string())),
    };
    let display_applier = DisplayApplier::new(hwnd, hinstance, logger.clone())?;

    let mut app = Box::new(App {
        hwnd,
        hinstance,
        taskbar_created,
        state: Rc::new(RefCell::new(AppState::new())),
        settings,
        settings_store,
        settings_backups,
        startup_settings_issue,
        startup_backup_issue,
        settings_file_existed_at_launch,
        settings_window: None,
        tray: None,
        hotkeys: None,
        shortcut_recording: false,
        keyboard_hook: None,
        mouse_hook: MouseHook::new(hwnd),
        presenter: PresenterEffects::new(hwnd, hinstance, logger.clone()),
        presenter_frame_pending: false,
        pass_through_runtime_enabled: false,
        pass_through_key_started_at: None,
        pass_through_key_polling_disabled: false,
        pressed_shortcuts: PressedShortcuts::new(),
        radial_key_cap_state: RadialKeyCapState::Idle,
        overlays: None,
        export: ExportController::new(hwnd, logger.clone()),
        palette: None,
        radial_palette: None,
        text_entry: None,
        help: None,
        music: None,
        last_board_presentation: None,
        lesson_badge: None,
        break_timer: None,
        pending_break_request: None,
        zoom: None,
        device_notification: None,
        display_change: DisplayChangeState::default(),
        display_stability_deadline_ms: None,
        display_target_table: Vec::new(),
        last_judged_uuids: None,
        last_auto_apply_at: None,
        pending_display_apply: None,
        expected_after_apply: None,
        display_applier,
        pending_archived_display_keys: HashSet::new(),
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
    if let Some(path) = initial_import_path.as_deref() {
        app.open_import_preview(path, "실행 인자");
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
        if app.collapse_radial_for_external_pointer_down(message.hwnd, message.message) {
            continue;
        }
        if app.display_applier.pretranslate_message(&message) {
            continue;
        }
        if app.break_timer.as_mut().is_some_and(|timer| {
            timer.pretranslate_message(message.hwnd, message.message, message.wParam)
        }) {
            continue;
        }
        if app.pretranslate_help_message(&message) {
            continue;
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
        if message == WM_COPYDATA {
            return if app.handle_copydata(lparam) {
                LRESULT(1)
            } else {
                LRESULT(0)
            };
        }
        if app.taskbar_created != 0 && message == app.taskbar_created {
            if let Some(tray) = app.tray.as_mut() {
                tray.readd_after_taskbar_created();
            }
            return LRESULT(0);
        }
        match message {
            WM_TIMER if wparam.0 == DISPLAY_SYNC_TIMER_ID => {
                app.handle_display_sync_timer();
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == DISPLAY_STABILITY_TIMER_ID => {
                app.handle_display_stability_timer();
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == DISPLAY_RETRY_TIMER_ID => {
                app.handle_display_retry_timer();
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == EXPORT_CAPTURE_TIMEOUT_TIMER_ID => {
                app.export.handle_capture_timeout();
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == PRESENTER_FRAME_TIMER_ID => {
                app.render_presenter("16ms 프레임 타이머");
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == PASS_THROUGH_IDLE_TIMER_ID => {
                app.handle_pass_through_expiry(false);
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == PASS_THROUGH_MAXIMUM_HOLD_TIMER_ID => {
                app.handle_pass_through_expiry(true);
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == PASS_THROUGH_KEY_POLL_TIMER_ID => {
                app.poll_pass_through_key();
                return LRESULT(0);
            }
            WM_HOTKEY => {
                app.handle_hotkey(wparam.0 as i32);
                return LRESULT(0);
            }
            WM_APP_KEY => {
                app.handle_mode_key(HookKeyEvent::decode(wparam, lparam));
                return LRESULT(0);
            }
            WM_APP_PRESENTER_MOUSE => {
                if let Some(event) = MouseHookEvent::decode(wparam, lparam) {
                    app.handle_presenter_mouse(event);
                } else {
                    app.logger.event("발표 마우스 훅 메시지 디코딩 실패");
                }
                return LRESULT(0);
            }
            WM_APP_PRESENTER_KEY => {
                app.handle_presenter_key(wparam.0 as u32);
                return LRESULT(0);
            }
            WM_APP_DISPLAY_DECISION => {
                if let Some(decision) = decision_from_command(wparam.0) {
                    app.handle_display_confirmation(decision);
                } else {
                    app.logger
                        .event(&format!("알 수 없는 화면 구성 확인 명령: {}", wparam.0));
                }
                return LRESULT(0);
            }
            WM_APP_DISPLAY_EXPIRED => {
                let pointer = lparam.0 as *mut ConfirmationResolution;
                if !pointer.is_null() {
                    // SAFETY: the expiration worker transfers one Box through this message.
                    let resolution = unsafe { *Box::from_raw(pointer) };
                    app.handle_display_expiration(resolution);
                }
                return LRESULT(0);
            }
            WM_APP_RADIAL_KEY => {
                app.handle_radial_key(RadialPaletteKeyEvent::decode(wparam, lparam));
                return LRESULT(0);
            }
            WM_APP_RADIAL_KEY_CAP => {
                if let Some(event) = RadialKeyCapHookEvent::decode(wparam) {
                    app.handle_radial_key_cap_hook(event);
                } else {
                    app.logger.event(&format!(
                        "원형 팔레트 키캡 훅 메시지 값 범위 밖: {}",
                        wparam.0
                    ));
                }
                return LRESULT(0);
            }
            WM_APP_SETTINGS_CHANGE => {
                let request = lparam.0 as *mut SettingsChangeRequest;
                if request.is_null() {
                    app.logger.event("설정 창 변경 요청 포인터가 null임");
                } else {
                    let request = unsafe { Box::from_raw(request) };
                    app.handle_settings_change(*request);
                }
                return LRESULT(0);
            }
            WM_APP_SETTINGS_ACTION => {
                let action = lparam.0 as *mut SettingsAction;
                if action.is_null() {
                    app.logger.event("설정 창 작업 요청 포인터가 null임");
                } else {
                    let action = unsafe { Box::from_raw(action) };
                    app.handle_settings_action(*action);
                }
                return LRESULT(0);
            }
            WM_APP_ZOOM_FRAME => {
                app.handle_zoom_frame();
                return LRESULT(0);
            }
            WM_APP_ZOOM_CAPTURE_FAILED => {
                app.handle_zoom_capture_failure();
                return LRESULT(0);
            }
            WM_APP_ZOOM_POINTER => {
                app.handle_zoom_pointer(decode_pointer(lparam));
                return LRESULT(0);
            }
            WM_APP_ZOOM_WHEEL => {
                app.handle_zoom_wheel(decode_wheel(wparam), lparam.0 != 0);
                return LRESULT(0);
            }
            WM_APP_ZOOM_INK_INPUT => {
                app.handle_zoom_ink_input(wparam.0 as u32, lparam.0 as u32);
                return LRESULT(0);
            }
            WM_APP_ZOOM_RECREATE => {
                app.handle_zoom_recreate();
                return LRESULT(0);
            }
            WM_APP_EXPORT_FRAME => {
                app.export.handle_capture_frame();
                return LRESULT(0);
            }
            WM_APP_EXPORT_CAPTURE_FAILED => {
                app.export.handle_capture_failure();
                return LRESULT(0);
            }
            WM_APP_ZOOM_REDRAW => {
                app.render_zoom("메시지 타이머");
                return LRESULT(0);
            }
            WM_APP_TEXT_ENTRY_OPEN => {
                let request = lparam.0 as *mut TextEntryRequest;
                if !request.is_null() {
                    let request = unsafe { Box::from_raw(request) };
                    app.begin_text_entry(*request);
                } else {
                    app.logger.event("텍스트 편집 열기 요청 포인터가 null임");
                }
                return LRESULT(0);
            }
            WM_APP_TEXT_ENTRY_COMMIT => {
                app.commit_text_entry();
                return LRESULT(0);
            }
            WM_APP_TEXT_ENTRY_CANCEL => {
                app.cancel_text_entry(cancel_reason(wparam.0));
                return LRESULT(0);
            }
            WM_APP_HELP_ACTION => {
                app.handle_help_action(wparam.0);
                return LRESULT(0);
            }
            WM_APP_HELP_REFRESH => {
                app.refresh_help("검색/테마 변경");
                return LRESULT(0);
            }
            WM_APP_HELP_CLOSE => {
                app.close_help(if wparam.0 == 1 {
                    "패널 밖 클릭"
                } else {
                    "창 요청"
                });
                return LRESULT(0);
            }
            WM_APP_LESSON_ACTION => {
                app.handle_lesson_action(u16::try_from(wparam.0).unwrap_or(0));
                return LRESULT(0);
            }
            WM_APP_LESSON_COMPLETED => {
                app.handle_lesson_completion(i64::try_from(wparam.0).unwrap_or(0));
                return LRESULT(0);
            }
            WM_APP_BREAK_ACTION => {
                app.handle_break_action(wparam.0);
                return LRESULT(0);
            }
            WM_APP_BREAK_TICK => {
                app.handle_break_tick("0.25초 갱신");
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == HELP_RELEASE_TIMER_ID => {
                app.poll_help_release();
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == RADIAL_KEY_CAP_TIMER_ID => {
                app.radial_key_cap_timer_elapsed();
                return LRESULT(0);
            }
            CALLBACK_MESSAGE => {
                app.handle_tray_callback((lparam.0 as u32) & 0xffff);
                return LRESULT(0);
            }
            SYNC_MESSAGE => {
                app.schedule_sync("오버레이 WM_DISPLAYCHANGE/WM_DPICHANGED");
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
            WM_APP_RADIAL_ACTION => {
                app.handle_radial_action();
                return LRESULT(0);
            }
            WM_APP_RADIAL_REDRAW => {
                app.redraw_radial_palette();
                return LRESULT(0);
            }
            WM_APP_RADIAL_SAVE_POSITION => {
                app.save_radial_palette_position();
                return LRESULT(0);
            }
            WM_APP_RADIAL_COLLAPSE => {
                if let Some(radial) = app.radial_palette.as_mut() {
                    if let Err(error) = radial.collapse("바깥 클릭") {
                        app.report_radial_palette_error("바깥 클릭 접기", &error);
                    }
                }
                return LRESULT(0);
            }
            WM_APP_MUSIC_LAUNCH_RESULT => {
                if let Some(music) = app.music.as_mut() {
                    music.handle_launch_result_message(lparam);
                }
                let result = app
                    .break_timer
                    .as_mut()
                    .map(BreakTimerManager::raise_to_front);
                if let Some(Err(error)) = result {
                    app.report_break_timer_error("음악 창 뒤에서 앞으로 올리기", &error);
                }
                return LRESULT(0);
            }
            WM_APP_MUSIC_PAUSE_RESULT => {
                if let Some(music) = app.music.as_ref() {
                    music.handle_pause_result_message(lparam);
                }
                return LRESULT(0);
            }
            WM_SETTINGCHANGE => {
                if let Some(tray) = app.tray.as_mut() {
                    tray.refresh_theme();
                }
                app.refresh_help("WM_SETTINGCHANGE");
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                if let Some(tray) = app.tray.as_mut() {
                    tray.refresh_display("WM_DPICHANGED");
                }
                app.schedule_presenter_frame();
                return LRESULT(0);
            }
            WM_DISPLAYCHANGE => {
                if let Some(tray) = app.tray.as_mut() {
                    tray.refresh_display("WM_DISPLAYCHANGE");
                }
                app.schedule_sync("WM_DISPLAYCHANGE");
                app.schedule_presenter_frame();
                return LRESULT(0);
            }
            WM_DEVICECHANGE
                if matches!(
                    wparam.0 as u32,
                    DBT_DEVICEARRIVAL | DBT_DEVICEREMOVECOMPLETE
                ) =>
            {
                let kind = if wparam.0 as u32 == DBT_DEVICEARRIVAL {
                    "DBT_DEVICEARRIVAL"
                } else {
                    "DBT_DEVICEREMOVECOMPLETE"
                };
                app.schedule_sync(kind);
                return LRESULT(0);
            }
            WM_TIMECHANGE => {
                app.synchronize_lesson_badge("WM_TIMECHANGE");
                let result = app.break_timer.as_mut().map(|timer| {
                    timer.synchronize_after_system_event(&app.settings, "WM_TIMECHANGE")
                });
                match result {
                    Some(Ok(outcome)) if outcome.did_finish => {
                        if let Some(music) = app.music.as_mut() {
                            music.break_did_end();
                        }
                        play_completion_sound(&app.settings.timer.notification_sound, &app.logger);
                    }
                    Some(Ok(outcome)) => {
                        if let (Some(end_date), Some(music)) =
                            (outcome.running_end_date, app.music.as_mut())
                        {
                            music.break_did_update(&app.settings, outcome.now, end_date);
                        }
                    }
                    Some(Err(error)) => {
                        app.report_break_timer_error("WM_TIMECHANGE 동기화", &error)
                    }
                    None => {}
                }
                return LRESULT(0);
            }
            WM_POWERBROADCAST if wparam.0 == 0x4 => {
                if let Some(overlays) = app.overlays.as_mut() {
                    if let Err(error) = overlays.archive_all_for_sleep() {
                        app.logger
                            .event(&format!("PBT_APMSUSPEND 화면 보관 실패: {error}"));
                    }
                }
                return LRESULT(1);
            }
            WM_POWERBROADCAST if wparam.0 == 0x12 => {
                app.schedule_sync("PBT_APMRESUMEAUTOMATIC");
                app.synchronize_lesson_badge("PBT_APMRESUMEAUTOMATIC");
                let result = app.break_timer.as_mut().map(|timer| {
                    timer.synchronize_after_system_event(&app.settings, "PBT_APMRESUMEAUTOMATIC")
                });
                match result {
                    Some(Ok(outcome)) if outcome.did_finish => {
                        if let Some(music) = app.music.as_mut() {
                            music.break_did_end();
                        }
                        play_completion_sound(&app.settings.timer.notification_sound, &app.logger);
                    }
                    Some(Ok(outcome)) => {
                        if let (Some(end_date), Some(music)) =
                            (outcome.running_end_date, app.music.as_mut())
                        {
                            music.break_did_update(&app.settings, outcome.now, end_date);
                        }
                    }
                    Some(Err(error)) => {
                        app.report_break_timer_error("PBT_APMRESUMEAUTOMATIC 동기화", &error)
                    }
                    None => {}
                }
                return LRESULT(1);
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
    if let Err(error) = crate::app_icon::run_selftest_attr_icons(logger) {
        failures.push(format!("attr icon: D2D selftest 실패: {error}"));
    }
    failures.extend(crate::display_apply::selftest_failures());
    failures.extend(crate::export::selftest_failures());
    failures.extend(crate::launch_at_login::selftest_failures());
    failures.extend(crate::monitor::online_display_selftest_failures());
    failures.extend(crate::archive::selftest_failures());
    failures.extend(crate::music::selftest_failures());
    failures.extend(crate::settings_file::selftest_failures());
    // SAFETY: None requests the current executable module handle.
    let hinstance = match unsafe { GetModuleHandleW(None) } {
        Ok(module) => HINSTANCE(module.0),
        Err(error) => {
            failures.push(format!("리소스: 모듈 핸들 조회 실패: {error}"));
            HINSTANCE::default()
        }
    };

    if !hinstance.is_invalid() {
        failures.extend(crate::settings_ui::selftest_failures(
            hinstance,
            logger.clone(),
        ));
        failures.extend(crate::zoom_view::selftest_failures(
            hinstance,
            logger.clone(),
        ));
        failures.extend(crate::presenter::selftest_failures(
            hinstance,
            logger.clone(),
        ));
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

        let state = Rc::new(RefCell::new(AppState::new()));
        let settings = AppSettings::default();
        let input_config = InkInputConfig::from_core(&state.borrow(), &settings);
        match OverlayManager::new(
            HWND::default(),
            hinstance,
            input_config,
            state,
            &settings,
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

        let state = AppState::new();
        let settings = AppSettings::default();
        match RadialPaletteManager::new(
            HWND::default(),
            hinstance,
            &state,
            &settings,
            logger.clone(),
        ) {
            Ok(mut palette) => failures.extend(palette.selftest_failures(&state, &settings)),
            Err(error) => failures.push(format!("radial palette: 생성 실패: {error}")),
        }
        match HelpManager::new(HWND::default(), hinstance, logger.clone()) {
            Ok(mut help) => failures.extend(help.selftest_failures(&state, &settings)),
            Err(error) => failures.push(format!("help: 생성 실패: {error}")),
        }
        match LessonBadgeManager::new(HWND::default(), hinstance, &settings, logger.clone()) {
            Ok(mut badge) => failures.extend(badge.selftest_failures()),
            Err(error) => failures.push(format!("lesson badge: 생성 실패: {error}")),
        }
        match BreakTimerManager::new(HWND::default(), hinstance, &settings, logger.clone()) {
            Ok(mut timer) => failures.extend(timer.selftest_failures()),
            Err(error) => failures.push(format!("break screen: 생성 실패: {error}")),
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
    for binding in crate::kbhook::unmappable_zoom_bindings(&AppSettings::default()) {
        failures.push(format!("keymap: 줌 바인딩을 VK로 바꿀 수 없음: {binding}"));
    }

    match KeyboardHook::install(HWND::default(), &AppSettings::default(), true, false) {
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

fn display_mutation_name(kind: &DisplayMutationKind) -> &'static str {
    match kind {
        DisplayMutationKind::AutoSwitch => "auto-switch",
        DisplayMutationKind::Rename { .. } => "rename",
        DisplayMutationKind::Duplicate { .. } => "duplicate",
        DisplayMutationKind::Delete { .. } => "delete",
        DisplayMutationKind::MovePreset { .. } => "move-preset",
        DisplayMutationKind::Edit(_) => "edit",
        DisplayMutationKind::Capture { .. } => "capture",
    }
}

fn display_edit_log(edit: &DisplayEdit) -> String {
    match edit {
        DisplayEdit::SetGridCell { preset, slot, cell } => format!(
            "격자 변경 preset={preset} slot={} cell={},{}",
            slot.display_name(),
            cell.x,
            cell.y
        ),
        DisplayEdit::AddScreen { preset, slot } => {
            format!(
                "화면 슬롯 추가 preset={preset} slot={}",
                slot.display_name()
            )
        }
        DisplayEdit::RemoveScreen { preset, slot } => format!(
            "화면 슬롯 제거 preset={preset} slot={}",
            slot.display_name()
        ),
        DisplayEdit::AddMonitor { preset, slot, .. } => {
            format!("모니터 추가 preset={preset} slot={}", slot.display_name())
        }
        DisplayEdit::ReplaceMonitor {
            preset,
            slot,
            monitor,
            ..
        } => format!(
            "모니터 교체 preset={preset} slot={} row={monitor}",
            slot.display_name()
        ),
        DisplayEdit::SetAutoMatch {
            preset,
            slot,
            monitor,
            enabled,
        } => format!(
            "모니터 자동 체크 preset={preset} slot={} row={monitor} enabled={enabled}",
            slot.display_name()
        ),
        DisplayEdit::RemoveMonitor {
            preset,
            slot,
            monitor,
        } => format!(
            "모니터 제거 preset={preset} slot={} row={monitor}",
            slot.display_name()
        ),
        DisplayEdit::MoveMonitorWithin {
            preset,
            slot,
            monitor,
            offset,
        } => format!(
            "모니터 순서 preset={preset} slot={} row={monitor} offset={offset}",
            slot.display_name()
        ),
        DisplayEdit::MoveMonitorBetween {
            preset,
            source,
            monitor,
            destination,
        } => format!(
            "모니터 슬롯 이동 preset={preset} source={} row={monitor} destination={}",
            source.display_name(),
            destination.display_name()
        ),
    }
}

const _: () = {
    assert!(size_of::<usize>() >= size_of::<u32>());
};

fn eraser_mode_name(mode: EraserMode) -> &'static str {
    match mode {
        EraserMode::Partial => "partial",
        EraserMode::Stroke => "stroke",
        EraserMode::Area => "area",
    }
}

fn settings_bundle_name(bundle: SettingsBundle) -> &'static str {
    match bundle {
        SettingsBundle::General => "general",
        SettingsBundle::Tools => "tools",
        SettingsBundle::Eraser => "eraser",
        SettingsBundle::Zoom => "zoom",
        SettingsBundle::Board => "board",
        SettingsBundle::Timer => "timer",
        SettingsBundle::Display => "display",
        SettingsBundle::Shortcuts => "shortcuts",
        SettingsBundle::Help => "help",
    }
}

fn zoom_control_for_shortcut(command: &ShortcutCommand) -> Option<ZoomGestureControl> {
    match command {
        ShortcutCommand::AdjustZoom(_) | ShortcutCommand::ResetZoom => {
            Some(ZoomGestureControl::Magnification)
        }
        ShortcutCommand::ToggleZoomFreeze => Some(ZoomGestureControl::FreezeKey),
        ShortcutCommand::BeginZoomPreview => Some(ZoomGestureControl::PreviewKey),
        ShortcutCommand::CycleZoomFollowMode => Some(ZoomGestureControl::FollowKey),
        _ => None,
    }
}

fn warp_zoom_pointer(
    logger: &Logger,
    monitor: &Monitor,
    view_point: ZoomPoint,
) -> Option<ZoomPoint> {
    let monitor_rect = ZoomRect::new(
        f64::from(monitor.rect.left),
        f64::from(monitor.rect.top),
        f64::from(monitor.rect.width()),
        f64::from(monitor.rect.height()),
    );
    let Some(screen_point) = ZoomGeometry::warp_screen_point(view_point, monitor_rect) else {
        logger.event(&format!(
            "줌 포인터 워프 건너뜀: monitor={} view=({:.1},{:.1}) reason=target-outside-monitor",
            monitor.session_key, view_point.x, view_point.y
        ));
        return None;
    };
    let x = screen_point.x.round() as i32;
    let y = screen_point.y.round() as i32;
    match unsafe { SetCursorPos(x, y) } {
        Ok(()) => {
            logger.event(&format!(
                "줌 포인터 워프 성공: monitor={} view=({:.1},{:.1}) screen=({x},{y})",
                monitor.session_key, view_point.x, view_point.y
            ));
            Some(screen_point)
        }
        Err(error) => {
            logger.event(&format!(
                "줌 포인터 워프 실패: monitor={} screen=({x},{y}) error={error}",
                monitor.session_key
            ));
            None
        }
    }
}
