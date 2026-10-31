pub(crate) fn is_chrome_session_source(source_app_user_model_id: &str) -> bool {
    source_app_user_model_id
        .to_ascii_lowercase()
        .contains("chrome")
}

const CHROME_SESSION_MATCH_CASES: &[(&str, bool)] = &[
    ("Chrome.exe", true),
    ("Google.Chrome_abc!App", true),
    ("Microsoft.MicrosoftEdge_8wekyb3d8bbwe!App", false),
    ("chromium", false),
    ("", false),
];

#[cfg(windows)]
pub(crate) fn selftest_failures() -> Vec<String> {
    CHROME_SESSION_MATCH_CASES
        .iter()
        .copied()
        .filter_map(|(source, expected)| {
            let actual = is_chrome_session_source(source);
            (actual != expected).then(|| {
                format!(
                    "music: Chrome AUMID 판정 불일치 source={source:?} expected={expected} actual={actual}"
                )
            })
        })
        .collect()
}

#[cfg(windows)]
mod platform {
    use super::is_chrome_session_source;
    use crate::log::Logger;
    use edutools_core::music::{
        collect_music_tracks, BreakMusicFade, BreakMusicUrl, FileSystemMusicDirectoryReader,
        MusicCollection, MusicOrder, MusicRepeat, Playlist, XorShift64,
    };
    use edutools_core::settings::AppSettings;
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows::core::{IInspectable, Result, HSTRING, PCWSTR, PWSTR};
    use windows::Foundation::{TypedEventHandler, Uri};
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
    use windows::Media::Core::MediaSource;
    use windows::Media::Playback::{MediaPlayer, MediaPlayerFailedEventArgs};
    use windows::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM, WPARAM};
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
    };
    use windows::Win32::System::WinRT::{
        RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED,
    };
    use windows::Win32::UI::Shell::{ShellExecuteW, UrlCreateFromPathW};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SW_SHOWNORMAL};

    pub(crate) const WM_APP_MUSIC_LAUNCH_RESULT: u32 = 0x8000 + 120;
    pub(crate) const WM_APP_MUSIC_PAUSE_RESULT: u32 = 0x8000 + 121;
    pub(crate) const WM_APP_MUSIC_FOLDER_EVENT: u32 = 0x8000 + 122;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum PlaybackLaunchState {
        Opening,
        Opened,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum PlaybackMode {
        Chrome,
        DefaultBrowser,
    }

    struct ActivePlayback {
        token: u64,
        mode: PlaybackMode,
        launch_state: PlaybackLaunchState,
    }

    enum LaunchOutcome {
        Opened {
            mode: PlaybackMode,
            chrome_fallback_reason: Option<String>,
        },
        Failed(String),
    }

    struct LaunchResult {
        token: u64,
        outcome: LaunchOutcome,
    }

    struct PauseResult {
        used_default_browser: bool,
        session_count: u32,
        chrome_session_count: u32,
        paused_count: u32,
        rejected_count: u32,
        errors: Vec<String>,
    }

    enum FolderEvent {
        ScanCompleted {
            token: u64,
            collection: MusicCollection,
            order: MusicOrder,
            repeat: MusicRepeat,
        },
        Opened {
            token: u64,
        },
        Ended {
            token: u64,
        },
        Failed {
            token: u64,
            reason: String,
        },
    }

    struct ComApartmentGuard;

    impl ComApartmentGuard {
        fn initialize_sta() -> Result<Self> {
            // SAFETY: initializes the current thread and is balanced by this guard's Drop.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
            Ok(Self)
        }
    }

    impl Drop for ComApartmentGuard {
        fn drop(&mut self) {
            // SAFETY: balances the successful CoInitializeEx on this same UI thread.
            unsafe { CoUninitialize() };
        }
    }

    struct WinRtApartmentGuard;

    impl WinRtApartmentGuard {
        fn initialize_sta() -> Result<Self> {
            // SAFETY: the UI thread owns this WinRT STA until the manager is dropped.
            unsafe { RoInitialize(RO_INIT_SINGLETHREADED)? };
            Ok(Self)
        }

        fn initialize_mta() -> Result<Self> {
            // SAFETY: each SMTC worker initializes and tears down its own MTA apartment.
            unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
            Ok(Self)
        }
    }

    struct FolderPlayback {
        token: u64,
        playlist: Playlist<XorShift64>,
        player: MediaPlayer,
        opened_event: i64,
        ended_event: i64,
        failed_event: i64,
    }

    impl Drop for FolderPlayback {
        fn drop(&mut self) {
            let _ = self.player.RemoveMediaOpened(self.opened_event);
            let _ = self.player.RemoveMediaEnded(self.ended_event);
            let _ = self.player.RemoveMediaFailed(self.failed_event);
            let _ = self.player.Pause();
            let _ = self.player.Close();
        }
    }

    impl Drop for WinRtApartmentGuard {
        fn drop(&mut self) {
            // SAFETY: balances the successful RoInitialize on this worker thread.
            unsafe { RoUninitialize() };
        }
    }

    struct SystemVolumeSnapshot {
        device_id: Vec<u16>,
        endpoint: IAudioEndpointVolume,
        original_volume: f32,
    }

    struct CoreAudioVolumeController {
        enumerator: IMMDeviceEnumerator,
    }

    impl CoreAudioVolumeController {
        fn new() -> Result<Self> {
            // SAFETY: the UI thread owns an initialized STA apartment for this controller's life.
            let enumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
            Ok(Self { enumerator })
        }

        fn capture_default_output_volume(
            &self,
        ) -> std::result::Result<SystemVolumeSnapshot, String> {
            let device = self.default_output_device()?;
            let device_id = device_id(&device)?;
            // SAFETY: IAudioEndpointVolume is activated from a valid render endpoint in this STA.
            let endpoint: IAudioEndpointVolume = unsafe {
                device
                    .Activate(CLSCTX_ALL, None)
                    .map_err(|error| format!("endpoint volume activation failed: {error}"))?
            };
            // SAFETY: endpoint is valid and owned by this UI-thread snapshot.
            let original_volume = unsafe {
                endpoint
                    .GetMasterVolumeLevelScalar()
                    .map_err(|error| format!("master volume query failed: {error}"))?
            };
            Ok(SystemVolumeSnapshot {
                device_id,
                endpoint,
                original_volume,
            })
        }

        fn set_snapshot_volume(
            &self,
            snapshot: &SystemVolumeSnapshot,
            volume: f32,
        ) -> std::result::Result<(), String> {
            let current_device = self.default_output_device()?;
            let current_id = device_id(&current_device)?;
            if current_id != snapshot.device_id {
                return Err("default output device changed".to_owned());
            }
            // SAFETY: the endpoint belongs to the captured default device and remains in this STA.
            unsafe {
                snapshot
                    .endpoint
                    .SetMasterVolumeLevelScalar(volume.clamp(0.0, 1.0), std::ptr::null())
                    .map_err(|error| format!("master volume update failed: {error}"))
            }
        }

        fn default_output_device(&self) -> std::result::Result<IMMDevice, String> {
            // SAFETY: enumerator is a valid COM interface used only on its owning UI thread.
            unsafe {
                self.enumerator
                    .GetDefaultAudioEndpoint(eRender, eConsole)
                    .map_err(|error| format!("default output query failed: {error}"))
            }
        }
    }

    fn device_id(device: &IMMDevice) -> std::result::Result<Vec<u16>, String> {
        // SAFETY: device is valid; GetId returns CoTaskMem-allocated, NUL-terminated UTF-16.
        let raw = unsafe {
            device
                .GetId()
                .map_err(|error| format!("output device id query failed: {error}"))?
        };
        if raw.0.is_null() {
            return Err("output device id was null".to_owned());
        }
        let mut length = 0usize;
        // SAFETY: GetId guarantees a NUL-terminated UTF-16 allocation.
        unsafe {
            while *raw.0.add(length) != 0 {
                length += 1;
            }
        }
        // SAFETY: the preceding scan found the allocation's terminator.
        let id = unsafe { std::slice::from_raw_parts(raw.0, length).to_vec() };
        // SAFETY: raw was allocated by COM for IMMDevice::GetId and is freed exactly once.
        unsafe { CoTaskMemFree(Some(raw.0.cast())) };
        Ok(id)
    }

    pub(crate) struct BreakMusicManager {
        owner: HWND,
        logger: Logger,
        audio: CoreAudioVolumeController,
        volume_snapshot: Option<SystemVolumeSnapshot>,
        active_playback: Option<ActivePlayback>,
        folder_scan_token: Option<u64>,
        folder_playback: Option<FolderPlayback>,
        is_break_active: bool,
        is_fade_available: bool,
        next_token: u64,
        _com_apartment: ComApartmentGuard,
        _winrt_apartment: WinRtApartmentGuard,
    }

    impl BreakMusicManager {
        pub(crate) fn new(owner: HWND, logger: Logger) -> Result<Self> {
            let com_apartment = ComApartmentGuard::initialize_sta()?;
            let winrt_apartment = WinRtApartmentGuard::initialize_sta()?;
            let audio = CoreAudioVolumeController::new()?;
            Ok(Self {
                owner,
                logger,
                audio,
                volume_snapshot: None,
                active_playback: None,
                folder_scan_token: None,
                folder_playback: None,
                is_break_active: false,
                is_fade_available: true,
                next_token: 1,
                _com_apartment: com_apartment,
                _winrt_apartment: winrt_apartment,
            })
        }

        pub(crate) fn break_did_start(&mut self, settings: &AppSettings) {
            self.is_break_active = true;
            if settings.timer.music_enabled {
                self.start_playback(settings, true);
            }
        }

        pub(crate) fn break_did_update(&mut self, settings: &AppSettings, now: f64, end_date: f64) {
            if !self.is_break_active
                || !settings.timer.music_enabled
                || !settings.timer.fade_enabled
                || !self.is_fade_available
            {
                return;
            }
            if settings.timer.music_source == "folder" {
                let Some(playback) = self.folder_playback.as_ref() else {
                    return;
                };
                let target =
                    BreakMusicFade::volume(1.0, now, end_date, settings.timer.fade_duration);
                match playback.player.SetVolume(f64::from(target)) {
                    Ok(()) => self.logger.event(&format!(
                        "로컬 쉬는 시간 음악 플레이어 페이드 적용 value={target:.3}"
                    )),
                    Err(error) => {
                        self.is_fade_available = false;
                        self.logger.event(&format!(
                            "경고: 로컬 쉬는 시간 음악 플레이어 페이드 중단 — {error}"
                        ));
                    }
                }
                return;
            }
            let Some(snapshot) = self.volume_snapshot.as_ref() else {
                return;
            };
            let target = BreakMusicFade::volume(
                snapshot.original_volume,
                now,
                end_date,
                settings.timer.fade_duration,
            );
            match self.audio.set_snapshot_volume(snapshot, target) {
                Ok(()) => self.logger.event(&format!(
                    "쉬는 시간 음악 볼륨 페이드 적용 value={target:.3}"
                )),
                Err(reason) => {
                    self.is_fade_available = false;
                    self.logger
                        .event(&format!("경고: 쉬는 시간 음악 볼륨 페이드 중단 — {reason}"));
                }
            }
        }

        pub(crate) fn break_did_end(&mut self) {
            if !self.is_break_active
                && self.active_playback.is_none()
                && self.volume_snapshot.is_none()
            {
                return;
            }
            self.is_break_active = false;
            self.stop_all_playback();
            self.restore_volume();
        }

        pub(crate) fn settings_changed(&mut self, previous: &AppSettings, current: &AppSettings) {
            if !self.is_break_active {
                return;
            }
            if previous.timer.music_enabled != current.timer.music_enabled {
                if current.timer.music_enabled {
                    self.start_playback(current, true);
                } else {
                    self.stop_all_playback();
                    self.restore_volume();
                }
                return;
            }
            let playback_source_changed = previous.timer.playlist_urls
                != current.timer.playlist_urls
                || previous.timer.default_playlist_index != current.timer.default_playlist_index
                || previous.timer.music_browser != current.timer.music_browser
                || previous.timer.music_source != current.timer.music_source
                || previous.timer.music_folder_path != current.timer.music_folder_path
                || previous.timer.music_order != current.timer.music_order
                || previous.timer.music_repeat != current.timer.music_repeat;
            if current.timer.music_enabled && playback_source_changed {
                self.stop_all_playback();
                self.restore_volume();
                self.start_playback(current, self.volume_snapshot.is_none());
            }
        }

        /// 설정 소유자는 `App::update_settings`이므로 UI는 이 요청 값을 store closure에 넘긴다.
        pub(crate) fn music_enabled_after_toggle(settings: &AppSettings) -> bool {
            !settings.timer.music_enabled
        }

        /// 저장 전 검증용 요청 생성기. 실제 갱신과 런타임 통지는 App의 단일 경로가 맡는다.
        pub(crate) fn validated_playlist_selection(
            settings: &AppSettings,
            index: i64,
        ) -> Option<i64> {
            let mut candidate = settings.timer.clone();
            candidate.select_playlist(index).then_some(index)
        }

        pub(crate) fn stop_music_for_termination(&mut self) {
            self.restore_volume();
            self.is_break_active = false;
            self.stop_all_playback();
        }

        pub(crate) fn next_folder_track(&mut self) -> Option<String> {
            let Some(playback) = self.folder_playback.as_mut() else {
                self.logger
                    .event("로컬 쉬는 시간 음악 다음 곡 요청 무시 — 재생 대기 중 또는 비활성");
                return None;
            };
            playback.playlist.advance(true);
            self.play_folder_current_or_skip()
        }

        pub(crate) fn handle_launch_result_message(&mut self, lparam: LPARAM) {
            if lparam.0 == 0 {
                self.logger
                    .event("경고: 쉬는 시간 음악 열기 결과 포인터가 null임");
                return;
            }
            // SAFETY: launch workers post exactly one Box<LaunchResult> pointer to this message.
            let result = unsafe { *Box::from_raw(lparam.0 as *mut LaunchResult) };
            let is_current = self
                .active_playback
                .as_ref()
                .is_some_and(|playback| playback.token == result.token);
            if !is_current {
                self.logger.event(&format!(
                    "쉬는 시간 음악 오래된 열기 결과 무시 token={}",
                    result.token
                ));
                return;
            }

            match result.outcome {
                LaunchOutcome::Opened {
                    mode,
                    chrome_fallback_reason,
                } => {
                    if let Some(reason) = chrome_fallback_reason {
                        self.logger.event(&format!(
                            "경고: Chrome 열기 불가 — 기본 브라우저로 폴백 ({reason})"
                        ));
                    }
                    if let Some(playback) = self.active_playback.as_mut() {
                        playback.mode = mode;
                        playback.launch_state = PlaybackLaunchState::Opened;
                    }
                    self.logger.event(match mode {
                        PlaybackMode::Chrome => "쉬는 시간 음악 열기 성공 browser=chrome",
                        PlaybackMode::DefaultBrowser => {
                            "쉬는 시간 음악 열기 성공 browser=defaultBrowser"
                        }
                    });
                }
                LaunchOutcome::Failed(reason) => {
                    self.logger
                        .event(&format!("경고: 쉬는 시간 음악 열기 실패 — {reason}"));
                    self.active_playback = None;
                    self.restore_volume();
                }
            }
        }

        pub(crate) fn handle_pause_result_message(&self, lparam: LPARAM) {
            if lparam.0 == 0 {
                self.logger
                    .event("경고: 쉬는 시간 음악 SMTC 결과 포인터가 null임");
                return;
            }
            // SAFETY: pause workers post exactly one Box<PauseResult> pointer to this message.
            let result = unsafe { *Box::from_raw(lparam.0 as *mut PauseResult) };
            if result.used_default_browser {
                self.logger
                    .event("defaultBrowser 재생은 소스 특정 불가 — Chrome SMTC 세션 일시정지 시도");
            }
            self.logger.event(&format!(
                "SMTC 일시정지 결과 sessions={} chrome={} paused={} rejected={}",
                result.session_count,
                result.chrome_session_count,
                result.paused_count,
                result.rejected_count
            ));
            for error in result.errors {
                self.logger
                    .event(&format!("경고: SMTC 일시정지 오류 — {error}"));
            }
            if result.chrome_session_count == 0 {
                self.logger
                    .event("경고: SMTC 에 Chrome 세션 없음 — 탭은 그대로");
            }
        }

        pub(crate) fn handle_folder_event_message(&mut self, lparam: LPARAM) -> Option<String> {
            if lparam.0 == 0 {
                self.logger
                    .event("경고: 로컬 쉬는 시간 음악 이벤트 포인터가 null임");
                return None;
            }
            // SAFETY: folder workers and MediaPlayer callbacks post one Box<FolderEvent> each.
            let event = unsafe { *Box::from_raw(lparam.0 as *mut FolderEvent) };
            match event {
                FolderEvent::ScanCompleted {
                    token,
                    collection,
                    order,
                    repeat,
                } => {
                    if self.folder_scan_token != Some(token) || !self.is_break_active {
                        self.logger.event(&format!(
                            "로컬 쉬는 시간 음악 오래된 스캔 결과 무시 token={token}"
                        ));
                        return None;
                    }
                    self.folder_scan_token = None;
                    if let Some(reason) = collection.failure_reason {
                        self.logger.event(&format!(
                            "경고: 로컬 쉬는 시간 음악 폴더 읽기 실패 — {reason}"
                        ));
                    }
                    if collection.tracks.is_empty() {
                        self.logger.event("로컬 쉬는 시간 음악 파일 없음");
                        return Some("재생할 음악 파일이 없습니다".to_owned());
                    }
                    let count = collection.tracks.len();
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|duration| duration.as_nanos() as u64)
                        .unwrap_or(1);
                    let playlist =
                        Playlist::new(collection.tracks, order, repeat, XorShift64::seeded(seed));
                    match create_folder_playback(self.owner, self.logger.clone(), token, playlist) {
                        Ok(playback) => {
                            self.folder_playback = Some(playback);
                            self.is_fade_available = true;
                            self.logger.event(&format!(
                                "로컬 쉬는 시간 음악 스캔 완료 tracks={count} token={token}"
                            ));
                            self.play_folder_current_or_skip()
                        }
                        Err(reason) => {
                            self.logger.event(&format!(
                                "경고: 로컬 쉬는 시간 음악 플레이어 생성 실패 — {reason}"
                            ));
                            Some("음악을 재생하지 못했습니다".to_owned())
                        }
                    }
                }
                FolderEvent::Opened { token } => {
                    let Some(playback) = self
                        .folder_playback
                        .as_mut()
                        .filter(|playback| playback.token == token)
                    else {
                        return None;
                    };
                    playback.playlist.report_success();
                    self.logger.event("로컬 쉬는 시간 음악 곡 열기 성공");
                    None
                }
                FolderEvent::Ended { token } => {
                    let Some(playback) = self
                        .folder_playback
                        .as_mut()
                        .filter(|playback| playback.token == token)
                    else {
                        return None;
                    };
                    playback.playlist.advance(false);
                    self.play_folder_current_or_skip()
                }
                FolderEvent::Failed { token, reason } => {
                    let Some(playback) = self
                        .folder_playback
                        .as_mut()
                        .filter(|playback| playback.token == token)
                    else {
                        return None;
                    };
                    self.logger
                        .event(&format!("경고: 로컬 쉬는 시간 음악 재생 실패 — {reason}"));
                    playback.playlist.report_failure();
                    if playback.playlist.exhausted() {
                        self.folder_playback = None;
                        return Some("음악을 재생하지 못했습니다".to_owned());
                    }
                    playback.playlist.advance(true);
                    self.play_folder_current_or_skip()
                }
            }
        }

        fn start_playback(&mut self, settings: &AppSettings, capture_volume: bool) {
            if settings.timer.music_source == "folder" {
                self.start_folder_scan(settings);
            } else {
                self.start_youtube_playback(settings, capture_volume);
            }
        }

        fn start_folder_scan(&mut self, settings: &AppSettings) {
            if self.folder_scan_token.is_some() || self.folder_playback.is_some() {
                return;
            }
            let token = self.take_token();
            self.folder_scan_token = Some(token);
            let path = PathBuf::from(&settings.timer.music_folder_path);
            let order = if settings.timer.music_order == "shuffle" {
                MusicOrder::Shuffle
            } else {
                MusicOrder::Name
            };
            let repeat = if settings.timer.music_repeat == "one" {
                MusicRepeat::One
            } else {
                MusicRepeat::All
            };
            let owner_value = self.owner.0 as isize;
            let logger = self.logger.clone();
            let worker_logger = logger.clone();
            let spawn_result = std::thread::Builder::new()
                .name("edutools-music-scan".to_owned())
                .spawn(move || {
                    let collection = if path.as_os_str().is_empty() {
                        MusicCollection::default()
                    } else {
                        collect_music_tracks(&path, &FileSystemMusicDirectoryReader)
                    };
                    post_boxed_result(
                        owner_value,
                        WM_APP_MUSIC_FOLDER_EVENT,
                        FolderEvent::ScanCompleted {
                            token,
                            collection,
                            order,
                            repeat,
                        },
                        &worker_logger,
                        "폴더 스캔",
                    );
                });
            if let Err(error) = spawn_result {
                self.folder_scan_token = None;
                logger.event(&format!(
                    "경고: 로컬 쉬는 시간 음악 스캔 작업 스레드 생성 실패 — {error}"
                ));
            } else {
                self.logger.event(&format!(
                    "로컬 쉬는 시간 음악 백그라운드 스캔 시작 token={token}"
                ));
            }
        }

        fn start_youtube_playback(&mut self, settings: &AppSettings, capture_volume: bool) {
            if self.active_playback.is_some() {
                return;
            }
            let Some(index) = usize::try_from(settings.timer.default_playlist_index).ok() else {
                self.logger
                    .event("경고: 쉬는 시간 음악 URL 거부 — 기본 인덱스가 음수임");
                return;
            };
            let Some(configured_url) = settings.timer.playlist_urls.get(index) else {
                self.logger
                    .event("경고: 쉬는 시간 음악 URL 거부 — 유효한 재생목록이 설정되지 않음");
                return;
            };
            let Some(validated_url) = BreakMusicUrl::validated(configured_url) else {
                self.logger
                    .event("경고: 쉬는 시간 음악 URL 거부 — 검증 실패");
                return;
            };
            let url = validated_url.to_owned();

            if capture_volume {
                match self.audio.capture_default_output_volume() {
                    Ok(snapshot) => {
                        self.logger.event(&format!(
                            "쉬는 시간 음악 볼륨 캡처 value={:.3}",
                            snapshot.original_volume
                        ));
                        self.volume_snapshot = Some(snapshot);
                        self.is_fade_available = true;
                    }
                    Err(reason) => {
                        self.volume_snapshot = None;
                        self.is_fade_available = false;
                        self.logger
                            .event(&format!("경고: 쉬는 시간 음악 볼륨 캡처 생략 — {reason}"));
                    }
                }
            }

            let browser = match settings.timer.music_browser.as_str() {
                "youtubeMusicApp" => "youtubeMusicApp",
                "defaultBrowser" => "defaultBrowser",
                _ => "chrome",
            };
            let prefer_chrome = browser != "defaultBrowser";
            if browser == "youtubeMusicApp" {
                self.logger
                    .event("youtubeMusicApp Windows 대응 앱 없음 — Chrome으로 대체");
            }
            let initial_mode = if prefer_chrome {
                PlaybackMode::Chrome
            } else {
                PlaybackMode::DefaultBrowser
            };
            let token = self.take_token();
            self.active_playback = Some(ActivePlayback {
                token,
                mode: initial_mode,
                launch_state: PlaybackLaunchState::Opening,
            });
            self.logger.event(&format!(
                "쉬는 시간 음악 시작 browser={} host=music.youtube.com token={token}",
                browser
            ));

            let owner_value = self.owner.0 as isize;
            let logger = self.logger.clone();
            let worker_logger = logger.clone();
            let spawn_result = std::thread::Builder::new()
                .name("edutools-music-open".to_owned())
                .spawn(move || {
                    let outcome = open_music_url(&url, prefer_chrome);
                    let result = LaunchResult { token, outcome };
                    post_boxed_result(
                        owner_value,
                        WM_APP_MUSIC_LAUNCH_RESULT,
                        result,
                        &worker_logger,
                        "열기",
                    );
                });
            if let Err(error) = spawn_result {
                logger.event(&format!(
                    "경고: 쉬는 시간 음악 열기 작업 스레드 생성 실패 — {error}"
                ));
                self.active_playback = None;
                self.restore_volume();
            }
        }

        fn stop_all_playback(&mut self) {
            self.folder_scan_token = None;
            self.folder_playback = None;
            self.stop_youtube_playback();
        }

        fn stop_youtube_playback(&mut self) {
            let Some(playback) = self.active_playback.take() else {
                return;
            };
            if playback.launch_state != PlaybackLaunchState::Opened {
                return;
            }
            self.spawn_pause_worker(playback.mode == PlaybackMode::DefaultBrowser);
        }

        fn take_token(&mut self) -> u64 {
            let token = self.next_token;
            self.next_token = self.next_token.wrapping_add(1).max(1);
            token
        }

        fn play_folder_current_or_skip(&mut self) -> Option<String> {
            let attempts = self
                .folder_playback
                .as_ref()
                .map(|playback| playback.playlist.track_count())
                .unwrap_or(0);
            for _ in 0..attempts {
                let (path, relative_path) = {
                    let playback = self.folder_playback.as_ref()?;
                    let track = playback.playlist.current()?;
                    (track.path.clone(), track.relative_path.clone())
                };
                let result = self
                    .folder_playback
                    .as_ref()
                    .ok_or_else(|| "player missing".to_owned())
                    .and_then(|playback| play_folder_path(&playback.player, &path));
                match result {
                    Ok(()) => {
                        self.logger.event(&format!(
                            "로컬 쉬는 시간 음악 재생 요청 track={relative_path:?}"
                        ));
                        return None;
                    }
                    Err(reason) => {
                        self.logger.event(&format!(
                            "경고: 로컬 쉬는 시간 음악 곡 열기 실패 track={relative_path:?} — {reason}"
                        ));
                        let playback = self.folder_playback.as_mut()?;
                        playback.playlist.report_failure();
                        if playback.playlist.exhausted() {
                            self.folder_playback = None;
                            return Some("음악을 재생하지 못했습니다".to_owned());
                        }
                        playback.playlist.advance(true);
                    }
                }
            }
            self.folder_playback = None;
            Some("음악을 재생하지 못했습니다".to_owned())
        }

        fn spawn_pause_worker(&self, used_default_browser: bool) {
            let owner_value = self.owner.0 as isize;
            let logger = self.logger.clone();
            let worker_logger = logger.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("edutools-music-smtc".to_owned())
                .spawn(move || {
                    let result = pause_chrome_sessions(used_default_browser);
                    post_boxed_result(
                        owner_value,
                        WM_APP_MUSIC_PAUSE_RESULT,
                        result,
                        &worker_logger,
                        "SMTC",
                    );
                })
            {
                logger.event(&format!(
                    "경고: SMTC 일시정지 작업 스레드 생성 실패 — {error}; 탭은 그대로"
                ));
            }
        }

        fn restore_volume(&mut self) {
            let Some(snapshot) = self.volume_snapshot.take() else {
                return;
            };
            match self
                .audio
                .set_snapshot_volume(&snapshot, snapshot.original_volume)
            {
                Ok(()) => self.logger.event(&format!(
                    "쉬는 시간 음악 볼륨 복원 value={:.3}",
                    snapshot.original_volume
                )),
                Err(reason) => self
                    .logger
                    .event(&format!("경고: 쉬는 시간 음악 볼륨 복원 생략 — {reason}")),
            }
            self.is_fade_available = true;
        }
    }

    fn create_folder_playback(
        owner: HWND,
        logger: Logger,
        token: u64,
        playlist: Playlist<XorShift64>,
    ) -> std::result::Result<FolderPlayback, String> {
        let player = MediaPlayer::new().map_err(|error| error.to_string())?;
        player
            .SetAutoPlay(false)
            .and_then(|_| player.SetIsLoopingEnabled(false))
            .and_then(|_| player.SetVolume(1.0))
            .map_err(|error| error.to_string())?;
        let owner_value = owner.0 as isize;

        let opened_logger = logger.clone();
        let opened_handler: TypedEventHandler<MediaPlayer, IInspectable> = TypedEventHandler::new(
            move |_: windows::core::Ref<MediaPlayer>, _: windows::core::Ref<IInspectable>| {
                post_boxed_result(
                    owner_value,
                    WM_APP_MUSIC_FOLDER_EVENT,
                    FolderEvent::Opened { token },
                    &opened_logger,
                    "곡 열기",
                );
                Ok(())
            },
        );
        let opened_event = player
            .MediaOpened(&opened_handler)
            .map_err(|error| format!("MediaOpened handler: {error}"))?;

        let ended_logger = logger.clone();
        let ended_handler: TypedEventHandler<MediaPlayer, IInspectable> = TypedEventHandler::new(
            move |_: windows::core::Ref<MediaPlayer>, _: windows::core::Ref<IInspectable>| {
                post_boxed_result(
                    owner_value,
                    WM_APP_MUSIC_FOLDER_EVENT,
                    FolderEvent::Ended { token },
                    &ended_logger,
                    "곡 종료",
                );
                Ok(())
            },
        );
        let ended_event = match player.MediaEnded(&ended_handler) {
            Ok(event) => event,
            Err(error) => {
                let _ = player.RemoveMediaOpened(opened_event);
                return Err(format!("MediaEnded handler: {error}"));
            }
        };

        let failed_logger = logger;
        let failed_handler: TypedEventHandler<MediaPlayer, MediaPlayerFailedEventArgs> =
            TypedEventHandler::new(
                move |_: windows::core::Ref<MediaPlayer>,
                      args: windows::core::Ref<MediaPlayerFailedEventArgs>| {
                    let reason = args
                        .as_ref()
                        .and_then(|args| args.ErrorMessage().ok())
                        .map(|message| message.to_string_lossy())
                        .unwrap_or_else(|| "MediaPlayer failure without details".to_owned());
                    post_boxed_result(
                        owner_value,
                        WM_APP_MUSIC_FOLDER_EVENT,
                        FolderEvent::Failed { token, reason },
                        &failed_logger,
                        "재생 실패",
                    );
                    Ok(())
                },
            );
        let failed_event = match player.MediaFailed(&failed_handler) {
            Ok(event) => event,
            Err(error) => {
                let _ = player.RemoveMediaOpened(opened_event);
                let _ = player.RemoveMediaEnded(ended_event);
                return Err(format!("MediaFailed handler: {error}"));
            }
        };
        Ok(FolderPlayback {
            token,
            playlist,
            player,
            opened_event,
            ended_event,
            failed_event,
        })
    }

    fn play_folder_path(player: &MediaPlayer, path: &Path) -> std::result::Result<(), String> {
        let uri = file_uri(path)?;
        let source = MediaSource::CreateFromUri(&uri)
            .map_err(|error| format!("MediaSource::CreateFromUri failed: {error}"))?;
        player
            .SetSource(&source)
            .and_then(|_| player.Play())
            .map_err(|error| format!("MediaPlayer source/play failed: {error}"))
    }

    fn file_uri(path: &Path) -> std::result::Result<Uri, String> {
        let wide_path = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let capacity = wide_path.len().saturating_mul(3).saturating_add(32);
        let mut buffer = vec![0_u16; capacity];
        let mut length = u32::try_from(buffer.len()).map_err(|_| "file URI too long".to_owned())?;
        // SAFETY: both buffers are valid for their reported sizes; the source is NUL-terminated.
        unsafe {
            UrlCreateFromPathW(
                PCWSTR(wide_path.as_ptr()),
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
                0,
            )
        }
        .map_err(|error| format!("UrlCreateFromPathW failed: {error}"))?;
        let length = usize::try_from(length)
            .unwrap_or(buffer.len())
            .min(buffer.len());
        let length = buffer[..length]
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(length);
        let uri = HSTRING::from(String::from_utf16_lossy(&buffer[..length]));
        Uri::CreateUri(&uri).map_err(|error| format!("file URI creation failed: {error}"))
    }

    fn open_music_url(url: &str, prefer_chrome: bool) -> LaunchOutcome {
        let _apartment = match ComApartmentGuard::initialize_sta() {
            Ok(apartment) => apartment,
            Err(error) => {
                return LaunchOutcome::Failed(format!(
                    "browser worker COM initialization failed: {error}"
                ))
            }
        };
        if prefer_chrome {
            if let Some(chrome_path) = chrome_app_path() {
                match Command::new(&chrome_path).arg(url).spawn() {
                    Ok(_) => {
                        return LaunchOutcome::Opened {
                            mode: PlaybackMode::Chrome,
                            chrome_fallback_reason: None,
                        }
                    }
                    Err(error) => {
                        let reason = format!("chrome.exe launch failed: {error}");
                        return match shell_open(url) {
                            Ok(()) => LaunchOutcome::Opened {
                                mode: PlaybackMode::DefaultBrowser,
                                chrome_fallback_reason: Some(reason),
                            },
                            Err(fallback_error) => LaunchOutcome::Failed(format!(
                                "{reason}; default browser fallback failed: {fallback_error}"
                            )),
                        };
                    }
                }
            }
            return match shell_open(url) {
                Ok(()) => LaunchOutcome::Opened {
                    mode: PlaybackMode::DefaultBrowser,
                    chrome_fallback_reason: Some(
                        "chrome.exe App Paths registry entry not found".to_owned(),
                    ),
                },
                Err(error) => LaunchOutcome::Failed(format!(
                    "chrome.exe App Paths registry entry not found; default browser fallback failed: {error}"
                )),
            };
        }

        match shell_open(url) {
            Ok(()) => LaunchOutcome::Opened {
                mode: PlaybackMode::DefaultBrowser,
                chrome_fallback_reason: None,
            },
            Err(error) => LaunchOutcome::Failed(format!("default browser launch failed: {error}")),
        }
    }

    fn chrome_app_path() -> Option<PathBuf> {
        let subkey = "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\chrome.exe"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
            .into_iter()
            .find_map(|root| registry_default_string(root, &subkey))
            .map(PathBuf::from)
    }

    fn registry_default_string(root: HKEY, subkey: &[u16]) -> Option<OsString> {
        let mut byte_count = 0u32;
        // SAFETY: subkey is NUL-terminated; the first call only requests the required size.
        let status = unsafe {
            RegGetValueW(
                root,
                PCWSTR(subkey.as_ptr()),
                PCWSTR::null(),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut byte_count),
            )
        };
        if status != ERROR_SUCCESS || byte_count < 2 {
            return None;
        }
        let mut value = vec![0u16; (byte_count as usize).div_ceil(2)];
        // SAFETY: value has the byte capacity reported by RegGetValueW.
        let status = unsafe {
            RegGetValueW(
                root,
                PCWSTR(subkey.as_ptr()),
                PCWSTR::null(),
                RRF_RT_REG_SZ,
                None,
                Some(value.as_mut_ptr().cast()),
                Some(&mut byte_count),
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut value = &value[..value
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(value.len())];
        if value.len() >= 2
            && value.first() == Some(&(b'"' as u16))
            && value.last() == Some(&(b'"' as u16))
        {
            value = &value[1..value.len() - 1];
        }
        (!value.is_empty()).then(|| OsString::from_wide(value))
    }

    fn shell_open(url: &str) -> std::result::Result<(), String> {
        let wide_url = OsString::from(url)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: parameters are separate NUL-terminated strings; no shell command is assembled.
        let result = unsafe {
            ShellExecuteW(
                None,
                windows::core::w!("open"),
                PCWSTR(wide_url.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = result.0 as isize;
        if code > 32 {
            Ok(())
        } else {
            Err(format!("ShellExecuteW code={code}"))
        }
    }

    fn pause_chrome_sessions(used_default_browser: bool) -> PauseResult {
        let mut result = PauseResult {
            used_default_browser,
            session_count: 0,
            chrome_session_count: 0,
            paused_count: 0,
            rejected_count: 0,
            errors: Vec::new(),
        };
        let _apartment = match WinRtApartmentGuard::initialize_mta() {
            Ok(apartment) => apartment,
            Err(error) => {
                result
                    .errors
                    .push(format!("WinRT MTA initialization failed: {error}"));
                return result;
            }
        };
        let manager = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|operation| operation.join())
        {
            Ok(manager) => manager,
            Err(error) => {
                result
                    .errors
                    .push(format!("session manager request failed: {error}"));
                return result;
            }
        };
        let sessions = match manager.GetSessions() {
            Ok(sessions) => sessions,
            Err(error) => {
                result
                    .errors
                    .push(format!("session enumeration failed: {error}"));
                return result;
            }
        };
        result.session_count = match sessions.Size() {
            Ok(count) => count,
            Err(error) => {
                result.errors.push(format!("session count failed: {error}"));
                return result;
            }
        };
        for index in 0..result.session_count {
            let session = match sessions.GetAt(index) {
                Ok(session) => session,
                Err(error) => {
                    result
                        .errors
                        .push(format!("session {index} access failed: {error}"));
                    continue;
                }
            };
            let source = match session.SourceAppUserModelId() {
                Ok(source) => source.to_string_lossy(),
                Err(error) => {
                    result
                        .errors
                        .push(format!("session {index} source query failed: {error}"));
                    continue;
                }
            };
            if !is_chrome_session_source(&source) {
                continue;
            }
            result.chrome_session_count += 1;
            match session
                .TryPauseAsync()
                .and_then(|operation| operation.join())
            {
                Ok(true) => result.paused_count += 1,
                Ok(false) => result.rejected_count += 1,
                Err(error) => result
                    .errors
                    .push(format!("Chrome session {index} pause failed: {error}")),
            }
        }
        result
    }

    fn post_boxed_result<T: 'static>(
        owner_value: isize,
        message: u32,
        result: T,
        logger: &Logger,
        operation: &str,
    ) {
        let raw = Box::into_raw(Box::new(result));
        let owner = HWND(owner_value as *mut c_void);
        // SAFETY: owner is the app's hidden window and raw remains owned by the message on success.
        if let Err(error) =
            unsafe { PostMessageW(Some(owner), message, WPARAM(0), LPARAM(raw as isize)) }
        {
            // SAFETY: PostMessageW failed, so ownership never transferred to the UI thread.
            unsafe { drop(Box::from_raw(raw)) };
            logger.event(&format!(
                "경고: 쉬는 시간 음악 {operation} 결과 PostMessage 실패 — {error}"
            ));
        }
    }
}

#[cfg(windows)]
pub(crate) use platform::{
    BreakMusicManager, WM_APP_MUSIC_FOLDER_EVENT, WM_APP_MUSIC_LAUNCH_RESULT,
    WM_APP_MUSIC_PAUSE_RESULT,
};

#[cfg(test)]
mod tests {
    use super::{is_chrome_session_source, CHROME_SESSION_MATCH_CASES};

    #[test]
    fn chrome_session_source_matching_table() {
        for &(source, expected) in CHROME_SESSION_MATCH_CASES {
            assert_eq!(
                is_chrome_session_source(source),
                expected,
                "source={source:?}"
            );
        }
    }
}
