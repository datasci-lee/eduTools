use crate::break_screen::{
    BreakStartOutcome, BreakTimerManager, WM_APP_BREAK_ACTION, WM_APP_BREAK_TICK,
};
use crate::capture::{
    CapturedFrame, ZoomCapture, WM_APP_ZOOM_CAPTURE_FAILED, WM_APP_ZOOM_FRAME, WM_APP_ZOOM_RECREATE,
};
use crate::help::{
    monotonic_millis, monotonic_seconds, HelpManager, HELP_RELEASE_TIMER_ID, WM_APP_HELP_ACTION,
    WM_APP_HELP_CLOSE, WM_APP_HELP_REFRESH,
};
use crate::hotkey::HotkeyCenter;
use crate::ink_view::InkInputConfig;
use crate::kbhook::{HookKeyEvent, HookScope, KeyboardHook, WM_APP_KEY};
use crate::keymap;
use crate::lesson_badge::{
    play_completion_sound, LessonBadgeManager, SettingsMutation as LessonSettingsMutation,
    WM_APP_LESSON_ACTION, WM_APP_LESSON_COMPLETED,
};
use crate::log::Logger;
use crate::monitor::{enumerate_monitors, local_point_in_monitor, monitor_at_point, Monitor};
use crate::music::{BreakMusicManager, WM_APP_MUSIC_LAUNCH_RESULT, WM_APP_MUSIC_PAUSE_RESULT};
use crate::overlay::{OverlayManager, SYNC_MESSAGE};
use crate::palette::{
    command_for_item, item_enabled, PaletteItemId, PaletteManager, WM_APP_PALETTE_ACTION,
    WM_APP_PALETTE_REDRAW, WM_APP_PALETTE_SAVE_POSITION, WM_APP_PALETTE_TOGGLE_ORIENTATION,
};
use crate::radial_palette::{
    RadialPaletteManager, WM_APP_RADIAL_ACTION, WM_APP_RADIAL_COLLAPSE, WM_APP_RADIAL_REDRAW,
    WM_APP_RADIAL_SAVE_POSITION,
};
use crate::render::GraphicsDevice;
use crate::settings_file::FileSettingsStorage;
use crate::text_entry::{
    cancel_reason, TextEntryRequest, TextEntryWindow, WM_APP_TEXT_ENTRY_CANCEL,
    WM_APP_TEXT_ENTRY_COMMIT, WM_APP_TEXT_ENTRY_OPEN,
};
use crate::tray::{self, Tray, CALLBACK_MESSAGE, MENU_EXIT, MENU_TOGGLE_ANNOTATION};
use crate::zoom_view::{
    decode_pointer, decode_wheel, ZoomView, WM_APP_ZOOM_POINTER, WM_APP_ZOOM_REDRAW,
    WM_APP_ZOOM_WHEEL,
};
use edutools_core::board::BoardPresentation;
use edutools_core::dispatch::{dispatch, DispatchContext, Effect, PressedShortcuts};
use edutools_core::eraser::EraserMode;
use edutools_core::ink::{
    InkGeometry, InkPoint, InkStroke, InkStrokeContent, InkStrokeTool, InkTool, PathCommand, Point,
    Size,
};
use edutools_core::radial::RadialPaletteIntent;
use edutools_core::settings::{changed_settings_groups, AppSettings, SettingsGroup};
use edutools_core::settings_store::{
    SettingsBackupStore, SettingsStore, SettingsStoreIssue, SettingsStoreIssueKind,
};
use edutools_core::shortcut::{
    ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutScope,
};
use edutools_core::state::{AppState, FavoriteSaveResult, SettingsMutation};
use edutools_core::zoom::{
    stop_or_cancel_preview_action, Point as ZoomPoint, Size as ZoomSize, ZoomFollowMode,
    ZoomGeometry, ZoomSessionState, ZoomStopAction,
};
use std::cell::RefCell;
use std::mem::size_of;
use std::path::PathBuf;
use std::rc::Rc;
use windows::core::{w, Error, Result, PCWSTR};
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, E_FAIL, HANDLE, HINSTANCE, HWND, LPARAM,
    LRESULT, POINT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext,
    DPI_AWARENESS_PER_MONITOR_AWARE,
};
use windows::Win32::UI::Input::Pointer::EnableMouseInPointer;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetForegroundWindow, GetMessageW, GetWindowLongPtrW, IsWindow, LoadImageW, PostQuitMessage,
    RegisterClassW, RegisterWindowMessageW, SetForegroundWindow, SetWindowLongPtrW,
    TranslateMessage, GDI_IMAGE_TYPE, GWLP_USERDATA, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, MSG,
    WM_CONTEXTMENU, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_HOTKEY, WM_KEYDOWN,
    WM_POWERBROADCAST, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMECHANGE, WM_TIMER, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_OVERLAPPED,
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
    state: Rc<RefCell<AppState>>,
    settings: AppSettings,
    settings_store: SettingsStore<FileSettingsStorage>,
    settings_backups: Option<SettingsBackupStore<FileSettingsStorage>>,
    startup_settings_issue: Option<SettingsStoreIssue>,
    startup_backup_issue: Option<String>,
    settings_file_existed_at_launch: bool,
    tray: Option<Tray>,
    hotkeys: Option<HotkeyCenter>,
    keyboard_hook: Option<KeyboardHook>,
    pressed_shortcuts: PressedShortcuts,
    overlays: Option<OverlayManager>,
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

impl App {
    fn initialize(&mut self) -> Result<()> {
        self.music = Some(BreakMusicManager::new(self.hwnd, self.logger.clone())?);
        self.tray = Some(Tray::add(self.hwnd, self.hinstance, self.logger.clone())?);
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

        if previous.shortcuts != current.shortcuts {
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
                self.update_settings("latest-backup-restore", move |current| *current = settings)
            }
            Ok(None) => {
                self.logger.event("설정 백업 복원 생략: 백업 없음");
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

        if !self.pressed_shortcuts.is_empty() {
            if let Some(hook) = self.keyboard_hook.as_ref() {
                hook.set_scopes(false, false);
                self.logger
                    .event("키보드 훅 해제 지연: 이미 삼킨 keyUp 쌍을 받은 뒤 해제");
            }
            return;
        }

        let Some(mut hook) = self.keyboard_hook.take() else {
            return;
        };
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
            return;
        };
        if let Err(error) = hook.uninstall() {
            self.logger
                .event(&format!("앱 종료 중 키보드 훅 해제 실패: {error}"));
        }
        self.pressed_shortcuts = PressedShortcuts::new();
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
                Effect::StepBack(result) => self
                    .logger
                    .event(&format!("효과 ({source}): 한 단계 나가기 {result:?}")),
                Effect::PaletteVisibilityChanged(visible) => self
                    .logger
                    .event(&format!("효과 ({source}): 선형 팔레트 표시 상태 {visible}")),
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
                Effect::AdjustZoom(steps) => self.adjust_zoom(steps),
                Effect::ResetZoom => self.reset_zoom(),
                Effect::ToggleZoomFreeze => self.toggle_zoom_freeze(),
                Effect::StopZoom => self.stop_or_cancel_zoom_preview(),
                Effect::BeginZoomPreview => self.begin_zoom_preview(),
                Effect::FinishZoomPreview => self.finish_zoom_preview(false),
                Effect::CycleZoomFollowMode => self.cycle_zoom_follow_mode(),
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
                other => self
                    .logger
                    .event(&format!("이번 라운드에서 UI 미구현 core effect: {other:?}")),
            }
        }
        if runtime_state_changed {
            self.refresh_runtime_state("core effect");
        }
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
        let result = (|| -> Result<ZoomRuntime> {
            let graphics = GraphicsDevice::new()?;
            let mut view = ZoomView::new(
                self.hwnd,
                self.hinstance,
                &monitor,
                &graphics,
                self.logger.clone(),
            )?;
            view.set_pointer(center);
            view.show_message(ZoomGeometry::magnification_label(magnification));
            view.render(&session)?;
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
        let Some(runtime) = self.zoom.take() else {
            self.state.borrow_mut().end_zoom();
            self.synchronize_keyboard_hook();
            return;
        };
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
        if steps == 0 || self.zoom.is_none() {
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
        let Some(runtime) = self.zoom.as_mut() else {
            return;
        };
        if !runtime.view.has_frame() {
            runtime.view.show_message("첫 화면을 캡처하는 중입니다");
            return;
        }
        let Some(mut session) = self.state.borrow().zoom_session().cloned() else {
            return;
        };
        session.is_frozen = !session.is_frozen;
        let frozen = session.is_frozen;
        runtime
            .capture
            .set_accepting_frames(!frozen || session.is_previewing);
        runtime.view.show_message(if frozen {
            "정지 줌 · 판서 켜짐"
        } else {
            "라이브 줌"
        });
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
        if !session.is_frozen && !session.is_previewing {
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

    fn handle_zoom_wheel(&mut self, delta: i32, control_down: bool) {
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
        if let Some(pointer) = self.zoom_pointer_location() {
            if let Some(runtime) = self.zoom.as_mut() {
                runtime.view.set_pointer(pointer);
            }
        }
        let result = self
            .zoom
            .as_mut()
            .map(|runtime| runtime.view.render(&session));
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
        let context = DispatchContext {
            scope: ShortcutScope::Annotation,
            display_id: display_id.as_deref(),
            board_display_id: display_id.as_deref(),
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
        let context = DispatchContext {
            scope,
            display_id: display_id.as_deref(),
            board_display_id: display_id.as_deref(),
            help_visible: true,
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
                self.logger
                    .event("원형 팔레트 미구현 intent: 설정 창 미구현");
            }
            RadialPaletteIntent::SaveFavorite => {
                let result = self.state.borrow().save_current_favorite(&self.settings);
                match result {
                    Ok(FavoriteSaveResult::Saved(mutation)) => {
                        self.update_settings("radial-favorite-save", move |settings| {
                            mutation.apply(settings);
                        });
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
            RadialPaletteIntent::ApplyFavorite { favorite, .. } => {
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
                self.update_settings("radial-favorite-remove", move |settings| {
                    SettingsMutation::SetFavorites(favorites).apply(settings);
                });
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
        let context = DispatchContext {
            scope: ShortcutScope::Annotation,
            display_id: display_id.as_deref(),
            board_display_id: display_id.as_deref(),
            radial_palette_expanded: self
                .radial_palette
                .as_ref()
                .is_some_and(RadialPaletteManager::is_expanded),
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
    }

    fn refresh_radial_palette(&mut self, reason: &str) {
        if let Some(radial) = self.radial_palette.as_mut() {
            if let Err(error) = radial.refresh(&self.state.borrow(), &self.settings, reason) {
                self.report_radial_palette_error(reason, &error);
            }
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
        if let Some(music) = self.music.as_mut() {
            music.stop_music_for_termination();
        }
        self.cancel_text_entry("앱 종료");
        self.zoom = None;
        self.state.borrow_mut().end_zoom();
        self.uninstall_keyboard_hook_for_shutdown();
        // Overlay HWNDs and graphics resources must be released on the UI thread before the tray
        // and hidden owner window go away.
        self.radial_palette = None;
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
        tray: None,
        hotkeys: None,
        keyboard_hook: None,
        pressed_shortcuts: PressedShortcuts::new(),
        overlays: None,
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
        if app.collapse_radial_for_external_pointer_down(message.hwnd, message.message) {
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
                app.handle_mode_key(HookKeyEvent::decode(wparam, lparam));
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
            WM_APP_ZOOM_RECREATE => {
                app.handle_zoom_recreate();
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
                return LRESULT(0);
            }
            WM_DISPLAYCHANGE => {
                if app.zoom.is_some() {
                    app.stop_zoom("WM_DISPLAYCHANGE", true);
                }
                if let Some(tray) = app.tray.as_mut() {
                    tray.refresh_display("WM_DISPLAYCHANGE");
                }
                app.synchronize_overlays("WM_DISPLAYCHANGE");
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
            WM_POWERBROADCAST if wparam.0 == 0x12 => {
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
        failures.extend(crate::zoom_view::selftest_failures(
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
