use super::{
    build_desired_paths, confirmation_message, delete_pending_marker, read_pending_marker,
    write_pending_marker, ConfirmationDecision, DisplayApplySource, DisplayApplyState,
    DisplayApplyStateMachine, ExpectedDisplayConfiguration, PendingDisplayMarker, SourceKey,
    TopologyPath,
};
use crate::log::Logger;
use crate::monitor::{
    current_display_configuration, hash_prefix8, query_active_display_config, DisplayConfiguration,
    DisplayTargetKey,
};
use crate::settings_file::FileSettingsStorage;
use crate::z_bands::{raise_band_order, Band, BandRegistration};
use edutools_core::display_operations::{DisplayConfigurationOperations, Operation};
use edutools_core::display_preset::{
    DisplayOutsidePlacement, DisplayPlan, DisplayPlanner, DisplayPoint, DisplayPreset,
    DisplayPresetDefaults,
};
use edutools_core::settings_store::SettingsStore;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Devices::Display::{
    SetDisplayConfig, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
    DISPLAYCONFIG_PATH_INFO, SDC_ALLOW_CHANGES, SDC_APPLY, SDC_SAVE_TO_DATABASE,
    SDC_USE_DATABASE_CURRENT, SDC_USE_SUPPLIED_DISPLAY_CONFIG, SDC_VALIDATE,
    SDC_VIRTUAL_MODE_AWARE,
};
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_CLASS_ALREADY_EXISTS, ERROR_SUCCESS, HINSTANCE, HWND, LPARAM,
    LRESULT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{GetSysColorBrush, COLOR_WINDOW};
use windows::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, GetWindowLongPtrW, IsChild,
    KillTimer, LoadCursorW, MessageBoxW, PostMessageW, RegisterClassW, SetForegroundWindow,
    SetTimer, SetWindowLongPtrW, SetWindowTextW, ShowWindow, BS_DEFPUSHBUTTON, BS_PUSHBUTTON,
    CREATESTRUCTW, GWLP_USERDATA, HMENU, IDC_ARROW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MSG,
    SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_KEYDOWN,
    WM_NCCREATE, WM_NCDESTROY, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

pub const WM_APP_DISPLAY_DECISION: u32 = 0x8000 + 240;
pub const WM_APP_DISPLAY_EXPIRED: u32 = 0x8000 + 241;

const CONFIRMATION_CLASS: PCWSTR = w!("EduToolsDisplayConfirmation");
const CONTROL_MESSAGE: i32 = 1_201;
const CONTROL_KEEP: i32 = 1_202;
const CONTROL_REVERT: i32 = 1_203;
const COUNTDOWN_TIMER_ID: usize = 0xEDA1;
const COUNTDOWN_TIMER_MS: u32 = 250;
const INVALID_MODE_INDEX: u16 = u16::MAX;

#[derive(Clone, Debug)]
pub struct DisplayPresetMenuEntry {
    pub index: usize,
    pub title: String,
    pub is_current: bool,
    pub is_enabled: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ApplyCompletion {
    pub expected: ExpectedDisplayConfiguration,
    pub awaiting_confirmation: bool,
}

#[derive(Clone, Debug)]
pub struct ConfirmationResolution {
    pub decision: ConfirmationDecision,
    pub success: bool,
    pub restored_previous: bool,
    pub expected: Option<ExpectedDisplayConfiguration>,
    pub message: Option<String>,
}

#[derive(Clone)]
struct RawDisplayConfiguration {
    paths: Vec<DISPLAYCONFIG_PATH_INFO>,
    modes: Vec<DISPLAYCONFIG_MODE_INFO>,
}

struct PendingShared {
    active: bool,
    desired: RawDisplayConfiguration,
    snapshot: RawDisplayConfiguration,
    marker_path: PathBuf,
    logger: Logger,
}

struct PendingConfirmation {
    shared: Arc<Mutex<PendingShared>>,
    window: ConfirmationWindow,
}

pub struct DisplayApplier {
    owner: HWND,
    hinstance: HINSTANCE,
    logger: Logger,
    machine: DisplayApplyStateMachine,
    pending: Option<PendingConfirmation>,
}

impl DisplayApplier {
    pub fn new(owner: HWND, hinstance: HINSTANCE, logger: Logger) -> Result<Self> {
        register_confirmation_class(hinstance)?;
        Ok(Self {
            owner,
            hinstance,
            logger,
            machine: DisplayApplyStateMachine::default(),
            pending: None,
        })
    }

    pub const fn state(&self) -> DisplayApplyState {
        self.machine.state()
    }

    pub fn menu_entries(&self, presets: &[DisplayPreset]) -> Vec<DisplayPresetMenuEntry> {
        if presets.is_empty() {
            return Vec::new();
        }
        let state_reason = match self.state() {
            DisplayApplyState::Idle => None,
            DisplayApplyState::Applying => Some("다른 화면 구성을 적용하는 중입니다".to_owned()),
            DisplayApplyState::AwaitingConfirmation => {
                Some("현재 화면 구성을 먼저 유지하거나 되돌리세요".to_owned())
            }
        };
        let session_reason = match is_console_session() {
            Ok(true) => None,
            Ok(false) => Some("RDP/비콘솔 세션에서는 적용할 수 없습니다".to_owned()),
            Err(error) => Some(format!("콘솔 세션을 확인하지 못했습니다: {error}")),
        };
        let configuration = current_display_configuration();
        presets
            .iter()
            .enumerate()
            .map(|(index, preset)| match &configuration {
                Ok(configuration) => {
                    let plan = DisplayPlanner::plan(preset, &configuration.online);
                    let plan_reason = (!plan.is_applicable()).then(|| plan.summary());
                    let disabled_reason = state_reason
                        .clone()
                        .or_else(|| session_reason.clone())
                        .or(plan_reason);
                    DisplayPresetMenuEntry {
                        index,
                        title: preset.name.clone(),
                        is_current: plan.is_applicable()
                            && preset.matches_current(&configuration.online),
                        is_enabled: disabled_reason.is_none(),
                        disabled_reason,
                    }
                }
                Err(error) => DisplayPresetMenuEntry {
                    index,
                    title: preset.name.clone(),
                    is_current: false,
                    is_enabled: false,
                    disabled_reason: Some(format!("화면 구성을 읽지 못했습니다: {error}")),
                },
            })
            .collect()
    }

    pub fn apply_preset(
        &mut self,
        preset: &DisplayPreset,
        preset_index: usize,
        source: DisplayApplySource,
    ) -> std::result::Result<ApplyCompletion, String> {
        self.machine.begin_request().map_err(str::to_owned)?;
        let result = self.apply_preset_inner(preset, preset_index, source);
        if result.is_err() {
            self.machine.failed();
        }
        result
    }

    fn apply_preset_inner(
        &mut self,
        preset: &DisplayPreset,
        preset_index: usize,
        source: DisplayApplySource,
    ) -> std::result::Result<ApplyCompletion, String> {
        self.logger.event(&format!(
            "화면 구성 적용 요청: source={} index={} presetHash={}",
            source.log_name(),
            preset_index,
            hash_prefix8(&preset.name)
        ));
        ensure_console_session()?;

        let configuration = current_display_configuration()
            .map_err(|error| format!("적용 직전 화면 구성을 읽지 못했습니다: {error}"))?;

        let plan = DisplayPlanner::plan(preset, &configuration.online);
        self.logger.event(&format!(
            "화면 구성 계획: applicable={} summary={}",
            plan.is_applicable(),
            plan.summary()
        ));
        if !plan.is_applicable() {
            return Err(plan.summary());
        }
        let operations = DisplayConfigurationOperations::build(&plan);
        let snapshot = raw_snapshot()?;
        let snapshot_plan = snapshot_plan(&configuration.online)?;
        let snapshot_operations = DisplayConfigurationOperations::build(&snapshot_plan);
        let snapshot_id = snapshot_identifier(&snapshot);
        self.logger.event(&format!(
            "화면 구성 스냅샷 보관: id={snapshot_id} paths={} modes={} recoveryOperations={}",
            snapshot.paths.len(),
            snapshot.modes.len(),
            snapshot_operations.len()
        ));
        let desired = build_raw_configuration(&snapshot, &configuration.target_table, &operations)?;
        validate_configuration(&desired, &self.logger)?;

        let marker_path = pending_marker_path()?;
        let marker = PendingDisplayMarker {
            snapshot_id,
            created_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        };
        write_pending_marker(&marker_path, &marker)
            .map_err(|error| format!("미확정 화면 구성 마커를 쓰지 못했습니다: {error}"))?;
        self.logger.event(&format!(
            "화면 구성 마커 쓰기: snapshotId={} path={}",
            marker.snapshot_id,
            marker_path.display()
        ));

        if let Err(error) = apply_temporary(&desired, &self.logger) {
            let _ = delete_marker_logged(&marker_path, &self.logger);
            return Err(error);
        }
        let expected = match verify_applied(preset, &configuration, &self.logger) {
            Ok(expected) => expected,
            Err(error) => {
                let rollback = rollback_configuration(&snapshot, &self.logger);
                if rollback.is_ok() {
                    let _ = delete_marker_logged(&marker_path, &self.logger);
                }
                return Err(match rollback {
                    Ok(_) => format!("{error}; 이전 구성으로 되돌렸습니다"),
                    Err(rollback_error) => {
                        format!("{error}; 복구 실패: {rollback_error}")
                    }
                });
            }
        };

        match source {
            DisplayApplySource::Automatic => {
                let save = apply_permanent(&desired, &self.logger);
                if let Err(save_error) = save {
                    let rollback = rollback_configuration(&snapshot, &self.logger);
                    if rollback.is_ok() {
                        let _ = delete_marker_logged(&marker_path, &self.logger);
                    }
                    return Err(match rollback {
                        Ok(_) => {
                            format!("영구 적용 실패: {save_error}; 이전 구성으로 되돌렸습니다")
                        }
                        Err(rollback_error) => {
                            format!("영구 적용 실패: {save_error}; 복구 실패: {rollback_error}")
                        }
                    });
                }
                delete_marker_logged(&marker_path, &self.logger)?;
                self.machine.applied(source);
                Ok(ApplyCompletion {
                    expected,
                    awaiting_confirmation: false,
                })
            }
            DisplayApplySource::Manual => {
                let shared = Arc::new(Mutex::new(PendingShared {
                    active: true,
                    desired,
                    snapshot,
                    marker_path,
                    logger: self.logger.clone(),
                }));
                let deadline = Instant::now()
                    + Duration::from_secs_f64(DisplayPresetDefaults::MANUAL_CONFIRMATION_INTERVAL);
                let window = match ConfirmationWindow::show(
                    self.owner,
                    self.hinstance,
                    deadline,
                    &plan,
                    &self.logger,
                ) {
                    Ok(window) => window,
                    Err(error) => {
                        let resolution = resolve_shared(&shared, ConfirmationDecision::Revert);
                        return Err(format!(
                            "화면 구성 확인 창을 열지 못했습니다: {error}; {}",
                            resolution
                                .message
                                .unwrap_or_else(|| "이전 구성으로 되돌렸습니다".to_owned())
                        ));
                    }
                };
                spawn_expiration_thread(
                    shared.clone(),
                    self.owner,
                    Duration::from_secs_f64(DisplayPresetDefaults::MANUAL_CONFIRMATION_INTERVAL),
                );
                self.pending = Some(PendingConfirmation { shared, window });
                self.machine.applied(source);
                Ok(ApplyCompletion {
                    expected,
                    awaiting_confirmation: true,
                })
            }
        }
    }

    pub fn resolve_confirmation(
        &mut self,
        decision: ConfirmationDecision,
    ) -> Option<ConfirmationResolution> {
        if self.state() != DisplayApplyState::AwaitingConfirmation {
            return None;
        }
        let pending = self.pending.take()?;
        let resolution = resolve_shared(&pending.shared, decision);
        drop(pending);
        self.machine.resolve(decision);
        Some(resolution)
    }

    pub fn finish_expiration(
        &mut self,
        resolution: ConfirmationResolution,
    ) -> Option<ConfirmationResolution> {
        if self.state() != DisplayApplyState::AwaitingConfirmation {
            return None;
        }
        self.pending = None;
        self.machine.resolve(ConfirmationDecision::Expire);
        Some(resolution)
    }

    pub fn rollback_for_termination(&mut self) -> Option<ConfirmationResolution> {
        self.resolve_confirmation(ConfirmationDecision::Terminate)
    }

    pub fn present_rollback_failure(&self, message: &str) -> bool {
        let anchor = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("EduTools Display Rollback Failure"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                Some(self.owner),
                None,
                Some(self.hinstance),
                None,
            )
        };
        let Ok(anchor) = anchor else {
            self.logger
                .event("복구 실패 대화 앵커 생성 실패; 트레이 알림으로 대체 필요");
            return false;
        };
        let _band = BandRegistration::new(anchor, Band::Dialog);
        let _ = raise_band_order();
        let message = wide(message);
        unsafe {
            MessageBoxW(
                Some(anchor),
                PCWSTR(message.as_ptr()),
                w!("직전 화면 구성으로 되돌리지 못했습니다"),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
            );
        }
        let _ = unsafe { DestroyWindow(anchor) };
        true
    }

    pub fn pretranslate_message(&self, message: &MSG) -> bool {
        let Some(pending) = self.pending.as_ref() else {
            return false;
        };
        if message.message != WM_KEYDOWN || message.wParam.0 != VK_ESCAPE.0 as usize {
            return false;
        }
        let window = pending.window.hwnd();
        if message.hwnd != window && !unsafe { IsChild(window, message.hwnd) }.as_bool() {
            return false;
        }
        let _ = unsafe {
            PostMessageW(
                Some(self.owner),
                WM_APP_DISPLAY_DECISION,
                WPARAM(usize::from(CONTROL_REVERT as u16)),
                LPARAM(0),
            )
        };
        true
    }
}

fn resolve_shared(
    shared: &Arc<Mutex<PendingShared>>,
    decision: ConfirmationDecision,
) -> ConfirmationResolution {
    let mut pending = match shared.lock() {
        Ok(pending) => pending,
        Err(_) => {
            return ConfirmationResolution {
                decision,
                success: false,
                restored_previous: false,
                expected: None,
                message: Some("화면 구성 확인 상태 잠금이 손상되었습니다".to_owned()),
            }
        }
    };
    if !pending.active {
        return ConfirmationResolution {
            decision,
            success: false,
            restored_previous: false,
            expected: None,
            message: Some("이미 처리된 화면 구성 확인입니다".to_owned()),
        };
    }
    pending.active = false;

    let (success, restored_previous, message) = match decision {
        ConfirmationDecision::Keep => match apply_permanent(&pending.desired, &pending.logger) {
            Ok(()) => (true, false, None),
            Err(keep_error) => match rollback_configuration(&pending.snapshot, &pending.logger) {
                Ok(_) => (
                    false,
                    true,
                    Some(format!(
                        "화면 구성을 저장하지 못해 이전 구성으로 되돌렸습니다: {keep_error}"
                    )),
                ),
                Err(rollback_error) => (
                    false,
                    false,
                    Some(format!(
                        "화면 구성을 저장하지 못했고 복구도 실패했습니다: {keep_error}; {rollback_error}"
                    )),
                ),
            },
        },
        ConfirmationDecision::Revert
        | ConfirmationDecision::Expire
        | ConfirmationDecision::Terminate => {
            match rollback_configuration(&pending.snapshot, &pending.logger) {
                Ok(_) => (true, true, None),
                Err(error) => (false, false, Some(format!("복구 실패: {error}"))),
            }
        }
    };
    if success || restored_previous {
        if let Err(error) = delete_marker_logged(&pending.marker_path, &pending.logger) {
            pending
                .logger
                .event(&format!("화면 구성 처리 후 마커 삭제 실패: {error}"));
        }
    } else {
        pending
            .logger
            .event("화면 구성 복구 미완료: 다음 시작 복구를 위해 마커 유지");
    }
    let expected = current_display_configuration()
        .ok()
        .map(|configuration| ExpectedDisplayConfiguration::capture(&configuration.online));
    pending.logger.event(&format!(
        "화면 구성 확인 결정: decision={decision:?} success={success} restoredPrevious={restored_previous}"
    ));
    ConfirmationResolution {
        decision,
        success,
        restored_previous,
        expected,
        message,
    }
}

fn spawn_expiration_thread(shared: Arc<Mutex<PendingShared>>, owner: HWND, delay: Duration) {
    let owner_value = owner.0 as isize;
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let resolution = resolve_shared(&shared, ConfirmationDecision::Expire);
        if resolution.message.as_deref() == Some("이미 처리된 화면 구성 확인입니다") {
            return;
        }
        let pointer = Box::into_raw(Box::new(resolution));
        let owner = HWND(owner_value as *mut c_void);
        if unsafe {
            PostMessageW(
                Some(owner),
                WM_APP_DISPLAY_EXPIRED,
                WPARAM(0),
                LPARAM(pointer as isize),
            )
        }
        .is_err()
        {
            // SAFETY: PostMessage failed, so ownership never left this thread.
            drop(unsafe { Box::from_raw(pointer) });
        }
    });
}

pub fn recover_pending_display_configuration(logger: &Logger) -> std::result::Result<bool, String> {
    let path = pending_marker_path()?;
    if !path.exists() {
        return Ok(false);
    }
    match read_pending_marker(&path) {
        Ok(marker) => logger.event(&format!(
            "시작 시 미확정 화면 구성 복구 시작: snapshotId={} createdUnixMs={}",
            marker.snapshot_id, marker.created_unix_ms
        )),
        Err(error) => logger.event(&format!(
            "시작 시 미확정 화면 구성 마커 해석 실패; 안전 복구는 계속: {error}"
        )),
    }
    let status = unsafe { SetDisplayConfig(None, None, SDC_APPLY | SDC_USE_DATABASE_CURRENT) };
    if status != ERROR_SUCCESS.0 as i32 {
        return Err(status_error("시작 시 저장 구성 복구", status));
    }
    delete_marker_logged(&path, logger)?;
    logger.event("시작 시 미확정 화면 구성 복구 성공");
    Ok(true)
}

pub fn run_display_apply_dry_run(index: usize, logger: &Logger) -> i32 {
    let storage = match FileSettingsStorage::app_settings() {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("display-apply-dry-run settings-error={error}");
            return 1;
        }
    };
    let store = SettingsStore::load(storage);
    let Some(preset) = store.settings().display.presets.get(index) else {
        eprintln!(
            "display-apply-dry-run invalid-index={} (1-based)",
            index + 1
        );
        return 2;
    };
    if let Err(error) = ensure_console_session() {
        eprintln!("display-apply-dry-run session-error={error}");
        return 1;
    }
    let configuration = match current_display_configuration() {
        Ok(configuration) => configuration,
        Err(error) => {
            eprintln!("display-apply-dry-run query-error={error}");
            return 1;
        }
    };
    let plan = DisplayPlanner::plan(preset, &configuration.online);
    println!(
        "display-apply-dry-run source=manual index={} presetHash={}",
        index + 1,
        hash_prefix8(&preset.name)
    );
    println!(
        "plan applicable={} summary={}",
        plan.is_applicable(),
        plan.summary()
    );
    if !plan.is_applicable() {
        return 1;
    }
    let snapshot = match raw_snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("snapshot error={error}");
            return 1;
        }
    };
    println!(
        "snapshot paths={} modes={} id={}",
        snapshot.paths.len(),
        snapshot.modes.len(),
        snapshot_identifier(&snapshot)
    );
    let operations = DisplayConfigurationOperations::build(&plan);
    let desired = match build_raw_configuration(&snapshot, &configuration.target_table, &operations)
    {
        Ok(desired) => desired,
        Err(error) => {
            eprintln!("path-build error={error}");
            return 1;
        }
    };
    let status = validate_status(&desired);
    println!(
        "validate status={} flags=SDC_VALIDATE|SDC_USE_SUPPLIED_DISPLAY_CONFIG|SDC_VIRTUAL_MODE_AWARE",
        status
    );
    let matches_current = preset.matches_current(&configuration.online);
    println!("matches_current={matches_current} apply=skipped");
    logger.event(&format!(
        "--display-apply-dry-run index={} validateStatus={} matchesCurrent={} APPLY=skipped",
        index + 1,
        status,
        matches_current
    ));
    if status == ERROR_SUCCESS.0 as i32 {
        0
    } else {
        1
    }
}

fn raw_snapshot() -> std::result::Result<RawDisplayConfiguration, String> {
    let (paths, modes) = query_active_display_config().map_err(|error| error.to_string())?;
    Ok(RawDisplayConfiguration { paths, modes })
}

fn snapshot_plan(
    online: &[edutools_core::display_preset::OnlineDisplay],
) -> std::result::Result<DisplayPlan, String> {
    let main = online
        .iter()
        .find(|display| {
            display.current_mirror_master_id.is_none() && display.origin == DisplayPoint::zero()
        })
        .ok_or_else(|| "원점 (0,0)의 독립 주 디스플레이를 찾지 못했습니다".to_owned())?;
    let origins_by_master = online
        .iter()
        .filter(|display| display.current_mirror_master_id.is_none())
        .map(|display| (display.id, display.origin))
        .collect();
    let mirrors = online
        .iter()
        .filter_map(|display| {
            display.current_mirror_master_id.map(|master_id| {
                edutools_core::display_preset::DisplayMirrorPlan {
                    slave_id: display.id,
                    master_id,
                }
            })
        })
        .collect();
    Ok(DisplayPlan {
        main_display_id: Some(main.id),
        origins_by_master,
        mirrors,
        unmirror_display_ids: online.iter().map(|display| display.id).collect(),
        outside_placements: Vec::<DisplayOutsidePlacement>::new(),
        missing_preset_monitors: Vec::new(),
        screens: Vec::new(),
        layout_issues: Vec::new(),
    })
}

fn build_raw_configuration(
    snapshot: &RawDisplayConfiguration,
    target_table: &[(u32, DisplayTargetKey)],
    operations: &[Operation],
) -> std::result::Result<RawDisplayConfiguration, String> {
    let display_by_target = target_table
        .iter()
        .map(|(display_id, target)| ((*target), *display_id))
        .collect::<HashMap<_, _>>();
    let mut topology = Vec::with_capacity(snapshot.paths.len());
    let mut original_source_mode = HashMap::new();
    for path in &snapshot.paths {
        let target = DisplayTargetKey {
            adapter_low: path.targetInfo.adapterId.LowPart,
            adapter_high: path.targetInfo.adapterId.HighPart,
            target_id: path.targetInfo.id,
        };
        let display_id = display_by_target.get(&target).copied().ok_or_else(|| {
            format!(
                "CCD target {}의 불투명 display id가 없습니다",
                target.target_id
            )
        })?;
        let source_index = source_mode_index(path)?;
        let source_mode = snapshot
            .modes
            .get(source_index)
            .ok_or_else(|| format!("source mode index {source_index}가 범위를 벗어났습니다"))?;
        if source_mode.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
            return Err(format!("mode {source_index}가 source mode가 아닙니다"));
        }
        let mode = unsafe { source_mode.Anonymous.sourceMode };
        topology.push(TopologyPath {
            display_id,
            target_id: path.targetInfo.id,
            adapter_low: path.sourceInfo.adapterId.LowPart,
            adapter_high: path.sourceInfo.adapterId.HighPart,
            source_id: path.sourceInfo.id,
            width: mode.width,
            height: mode.height,
            origin: DisplayPoint {
                x: f64::from(mode.position.x),
                y: f64::from(mode.position.y),
            },
        });
        original_source_mode.insert(display_id, *source_mode);
    }
    let desired = build_desired_paths(&topology, operations).map_err(|error| error.to_string())?;
    let desired_by_id = desired
        .iter()
        .map(|path| (path.display_id, path))
        .collect::<HashMap<_, _>>();

    let mut modes = Vec::new();
    let mut remapped_non_source = HashMap::<usize, u16>::new();
    let mut source_indices = HashMap::<SourceKey, u16>::new();
    let mut clone_groups = HashMap::<SourceKey, u16>::new();
    let mut paths = snapshot.paths.clone();

    for path in &mut paths {
        let target = DisplayTargetKey {
            adapter_low: path.targetInfo.adapterId.LowPart,
            adapter_high: path.targetInfo.adapterId.HighPart,
            target_id: path.targetInfo.id,
        };
        let display_id = display_by_target[&target];
        let desired = desired_by_id[&display_id];
        let source_index = if let Some(index) = source_indices.get(&desired.source).copied() {
            index
        } else {
            let mut source_mode = original_source_mode[&desired.mode_master_id];
            source_mode.id = desired.source.source_id;
            source_mode.adapterId.LowPart = desired.source.adapter_low;
            source_mode.adapterId.HighPart = desired.source.adapter_high;
            let mut value = unsafe { source_mode.Anonymous.sourceMode };
            value.position.x = exact_i32(desired.origin.x)?;
            value.position.y = exact_i32(desired.origin.y)?;
            source_mode.Anonymous.sourceMode = value;
            let index = push_mode(&mut modes, source_mode)?;
            source_indices.insert(desired.source, index);
            let clone_group = u16::try_from(clone_groups.len())
                .map_err(|_| "CCD clone group이 너무 많습니다".to_owned())?;
            clone_groups.insert(desired.source, clone_group);
            index
        };
        path.sourceInfo.adapterId.LowPart = desired.source.adapter_low;
        path.sourceInfo.adapterId.HighPart = desired.source.adapter_high;
        path.sourceInfo.id = desired.source.source_id;
        let clone_group = clone_groups[&desired.source];
        path.sourceInfo.Anonymous.modeInfoIdx =
            (u32::from(source_index) << 16) | u32::from(clone_group);

        let packed_target = unsafe { path.targetInfo.Anonymous.modeInfoIdx };
        let target_index = (packed_target & 0xffff) as u16;
        let desktop_index = ((packed_target >> 16) & 0xffff) as u16;
        let remapped_target = remap_mode(
            target_index,
            &snapshot.modes,
            &mut modes,
            &mut remapped_non_source,
        )?;
        let remapped_desktop = remap_mode(
            desktop_index,
            &snapshot.modes,
            &mut modes,
            &mut remapped_non_source,
        )?;
        path.targetInfo.Anonymous.modeInfoIdx =
            u32::from(remapped_target) | (u32::from(remapped_desktop) << 16);
    }
    Ok(RawDisplayConfiguration { paths, modes })
}

fn source_mode_index(path: &DISPLAYCONFIG_PATH_INFO) -> std::result::Result<usize, String> {
    let packed = unsafe { path.sourceInfo.Anonymous.modeInfoIdx };
    let index = ((packed >> 16) & 0xffff) as u16;
    if index == INVALID_MODE_INDEX {
        Err("활성 CCD 경로에 source mode가 없습니다".to_owned())
    } else {
        Ok(index as usize)
    }
}

fn remap_mode(
    old_index: u16,
    old_modes: &[DISPLAYCONFIG_MODE_INFO],
    modes: &mut Vec<DISPLAYCONFIG_MODE_INFO>,
    remapped: &mut HashMap<usize, u16>,
) -> std::result::Result<u16, String> {
    if old_index == INVALID_MODE_INDEX {
        return Ok(INVALID_MODE_INDEX);
    }
    let old_index = usize::from(old_index);
    if let Some(index) = remapped.get(&old_index) {
        return Ok(*index);
    }
    let mode = old_modes
        .get(old_index)
        .copied()
        .ok_or_else(|| format!("target/desktop mode index {old_index}가 범위를 벗어났습니다"))?;
    let index = push_mode(modes, mode)?;
    remapped.insert(old_index, index);
    Ok(index)
}

fn push_mode(
    modes: &mut Vec<DISPLAYCONFIG_MODE_INFO>,
    mode: DISPLAYCONFIG_MODE_INFO,
) -> std::result::Result<u16, String> {
    let index = u16::try_from(modes.len()).map_err(|_| "CCD mode 배열이 너무 큽니다".to_owned())?;
    if index == INVALID_MODE_INDEX {
        return Err("CCD mode 배열이 INVALID 인덱스에 도달했습니다".to_owned());
    }
    modes.push(mode);
    Ok(index)
}

fn exact_i32(value: f64) -> std::result::Result<i32, String> {
    if !value.is_finite()
        || value.round() != value
        || value < f64::from(i32::MIN)
        || value > f64::from(i32::MAX)
    {
        return Err(format!("화면 원점 {value}을 i32로 적용할 수 없습니다"));
    }
    Ok(value as i32)
}

fn validate_status(configuration: &RawDisplayConfiguration) -> i32 {
    unsafe {
        SetDisplayConfig(
            Some(&configuration.paths),
            Some(&configuration.modes),
            SDC_VALIDATE | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_VIRTUAL_MODE_AWARE,
        )
    }
}

fn validate_configuration(
    configuration: &RawDisplayConfiguration,
    logger: &Logger,
) -> std::result::Result<(), String> {
    let status = validate_status(configuration);
    logger.event(&format!("화면 구성 VALIDATE 결과: status={status}"));
    if status == ERROR_SUCCESS.0 as i32 {
        Ok(())
    } else {
        Err(status_error("SDC_VALIDATE", status))
    }
}

fn apply_temporary(
    configuration: &RawDisplayConfiguration,
    logger: &Logger,
) -> std::result::Result<(), String> {
    let status = unsafe {
        SetDisplayConfig(
            Some(&configuration.paths),
            Some(&configuration.modes),
            SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_VIRTUAL_MODE_AWARE,
        )
    };
    logger.event(&format!(
        "화면 구성 APPLY 결과: status={status} saveToDatabase=false allowChanges=false"
    ));
    if status == ERROR_SUCCESS.0 as i32 {
        Ok(())
    } else {
        Err(status_error("임시 SDC_APPLY", status))
    }
}

fn apply_permanent(
    configuration: &RawDisplayConfiguration,
    logger: &Logger,
) -> std::result::Result<(), String> {
    let status = unsafe {
        SetDisplayConfig(
            Some(&configuration.paths),
            Some(&configuration.modes),
            SDC_APPLY
                | SDC_SAVE_TO_DATABASE
                | SDC_USE_SUPPLIED_DISPLAY_CONFIG
                | SDC_VIRTUAL_MODE_AWARE,
        )
    };
    logger.event(&format!(
        "화면 구성 확정 저장 결과: status={status} saveToDatabase=true allowChanges=false"
    ));
    if status == ERROR_SUCCESS.0 as i32 {
        Ok(())
    } else {
        Err(status_error("SDC_SAVE_TO_DATABASE", status))
    }
}

fn rollback_configuration(
    snapshot: &RawDisplayConfiguration,
    logger: &Logger,
) -> std::result::Result<bool, String> {
    let supplied_status = unsafe {
        SetDisplayConfig(
            Some(&snapshot.paths),
            Some(&snapshot.modes),
            SDC_APPLY
                | SDC_USE_SUPPLIED_DISPLAY_CONFIG
                | SDC_ALLOW_CHANGES
                | SDC_VIRTUAL_MODE_AWARE,
        )
    };
    logger.event(&format!(
        "화면 구성 복구 결과: suppliedStatus={supplied_status} allowChanges=true"
    ));
    if supplied_status == ERROR_SUCCESS.0 as i32 {
        return Ok(false);
    }
    let database_status =
        unsafe { SetDisplayConfig(None, None, SDC_APPLY | SDC_USE_DATABASE_CURRENT) };
    logger.event(&format!(
        "화면 구성 복구 fallback 결과: databaseStatus={database_status}"
    ));
    if database_status == ERROR_SUCCESS.0 as i32 {
        Ok(true)
    } else {
        Err(format!(
            "{}; {}",
            status_error("스냅샷 복구", supplied_status),
            status_error("저장 구성 복구", database_status)
        ))
    }
}

fn verify_applied(
    preset: &DisplayPreset,
    before: &DisplayConfiguration,
    logger: &Logger,
) -> std::result::Result<ExpectedDisplayConfiguration, String> {
    let after = current_display_configuration().map_err(|error| error.to_string())?;
    let matches_current = preset.matches_current(&after.online);
    let plan = DisplayPlanner::plan(preset, &before.online);
    let independent_ids = plan
        .origins_by_master
        .keys()
        .copied()
        .chain(
            plan.outside_placements
                .iter()
                .map(|placement| placement.display_id),
        )
        .collect::<HashSet<_>>();
    let before_sizes = before
        .online
        .iter()
        .filter(|display| independent_ids.contains(&display.id))
        .map(|display| (display.identity(), display.size))
        .collect::<Vec<_>>();
    let dimensions_unchanged = before_sizes.iter().all(|(identity, size)| {
        after
            .online
            .iter()
            .find(|display| display.identity().represents_same_display(identity))
            .is_none_or(|display| display.size == *size)
    });
    logger.event(&format!(
        "화면 구성 적용 후 검증: matchesCurrent={matches_current} masterDimensionsUnchanged={dimensions_unchanged}"
    ));
    if !matches_current {
        return Err("적용 후 실제 화면 구성이 계획과 일치하지 않습니다".to_owned());
    }
    if !dimensions_unchanged {
        return Err("적용 과정에서 마스터 화면 해상도가 바뀌었습니다".to_owned());
    }
    Ok(ExpectedDisplayConfiguration::capture(&after.online))
}

fn status_error(context: &str, status: i32) -> String {
    if status == ERROR_ACCESS_DENIED.0 as i32 {
        format!("{context} 실패: ERROR_ACCESS_DENIED (RDP/비콘솔 세션에서는 적용 불가)")
    } else {
        format!("{context} 실패: Win32 status={status}")
    }
}

fn ensure_console_session() -> std::result::Result<(), String> {
    match is_console_session() {
        Ok(true) => Ok(()),
        Ok(false) => Err("RDP/비콘솔 세션에서는 화면 구성을 적용할 수 없습니다".to_owned()),
        Err(error) => Err(format!("콘솔 세션을 확인하지 못했습니다: {error}")),
    }
}

pub fn is_console_session() -> Result<bool> {
    let mut process_session = 0;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut process_session) }?;
    let console_session = unsafe { WTSGetActiveConsoleSessionId() };
    Ok(console_session != u32::MAX && process_session == console_session)
}

fn pending_marker_path() -> std::result::Result<PathBuf, String> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("EduTools/display-pending.json"))
        .ok_or_else(|| "APPDATA 환경 변수가 없습니다".to_owned())
}

fn delete_marker_logged(path: &Path, logger: &Logger) -> std::result::Result<(), String> {
    delete_pending_marker(path).map_err(|error| error.to_string())?;
    logger.event(&format!("화면 구성 마커 삭제: path={}", path.display()));
    Ok(())
}

fn snapshot_identifier(snapshot: &RawDisplayConfiguration) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for path in &snapshot.paths {
        for value in [
            path.sourceInfo.adapterId.LowPart,
            path.sourceInfo.adapterId.HighPart as u32,
            path.sourceInfo.id,
            path.targetInfo.adapterId.LowPart,
            path.targetInfo.adapterId.HighPart as u32,
            path.targetInfo.id,
        ] {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }
    format!("{:016x}", hash)
}

struct ConfirmationWindowState {
    owner: HWND,
    hwnd: HWND,
    message: HWND,
    keep: HWND,
    deadline: Instant,
}

struct ConfirmationWindow {
    state: Box<ConfirmationWindowState>,
    _band: BandRegistration,
}

struct WindowConstructionGuard(HWND);

impl Drop for WindowConstructionGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }
}

impl ConfirmationWindow {
    fn show(
        owner: HWND,
        hinstance: HINSTANCE,
        deadline: Instant,
        plan: &DisplayPlan,
        logger: &Logger,
    ) -> Result<Self> {
        let mut state = Box::new(ConfirmationWindowState {
            owner,
            hwnd: HWND::default(),
            message: HWND::default(),
            keep: HWND::default(),
            deadline,
        });
        let width = 520;
        let height = 190;
        let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let x = (screen_width - width) / 2;
        let y = (screen_height - height) / 2;
        let state_ptr = state.as_mut() as *mut ConfirmationWindowState;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                CONFIRMATION_CLASS,
                w!("화면 구성 확인"),
                WS_POPUP | WS_CAPTION | WS_SYSMENU,
                x,
                y,
                width,
                height,
                Some(owner),
                None,
                Some(hinstance),
                Some(state_ptr.cast()),
            )
        }?;
        state.hwnd = hwnd;
        let mut construction_guard = WindowConstructionGuard(hwnd);
        let message = create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            CONTROL_MESSAGE,
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
            20,
            20,
            470,
            58,
            &confirmation_message(DisplayPresetDefaults::MANUAL_CONFIRMATION_INTERVAL as u64),
        )?;
        state.message = message;
        state.keep = create_control(
            hwnd,
            hinstance,
            w!("BUTTON"),
            CONTROL_KEEP,
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_DEFPUSHBUTTON as u32),
            292,
            100,
            95,
            32,
            "유지",
        )?;
        let _revert = create_control(
            hwnd,
            hinstance,
            w!("BUTTON"),
            CONTROL_REVERT,
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32),
            395,
            100,
            95,
            32,
            "되돌리기",
        )?;
        if unsafe { SetTimer(Some(hwnd), COUNTDOWN_TIMER_ID, COUNTDOWN_TIMER_MS, None) } == 0 {
            return Err(Error::from_thread());
        }
        let band = BandRegistration::new(hwnd, Band::Dialog);
        let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
        raise_band_order()?;
        let _ = unsafe { SetForegroundWindow(hwnd) };
        let _ = unsafe { SetFocus(Some(state.keep)) };
        logger.event(&format!(
            "화면 구성 확인 창 표시: mainDisplay={:?} missingMonitors={} timeout={}s",
            plan.main_display_id,
            plan.missing_preset_monitors.len(),
            DisplayPresetDefaults::MANUAL_CONFIRMATION_INTERVAL
        ));
        construction_guard.0 = HWND::default();
        Ok(Self { state, _band: band })
    }

    const fn hwnd(&self) -> HWND {
        self.state.hwnd
    }
}

impl Drop for ConfirmationWindow {
    fn drop(&mut self) {
        let _ = unsafe { KillTimer(Some(self.state.hwnd), COUNTDOWN_TIMER_ID) };
        let _ = unsafe { DestroyWindow(self.state.hwnd) };
    }
}

fn create_control(
    parent: HWND,
    hinstance: HINSTANCE,
    class: PCWSTR,
    id: i32,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    title: &str,
) -> Result<HWND> {
    let title = wide(title);
    unsafe {
        CreateWindowExW(
            Default::default(),
            class,
            PCWSTR(title.as_ptr()),
            style,
            x,
            y,
            width,
            height,
            Some(parent),
            Some(HMENU(id as usize as *mut c_void)),
            Some(hinstance),
            None,
        )
    }
}

fn register_confirmation_class(hinstance: HINSTANCE) -> Result<()> {
    let class = WNDCLASSW {
        lpfnWndProc: Some(confirmation_window_proc),
        hInstance: hinstance,
        lpszClassName: CONFIRMATION_CLASS,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }?,
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        let error = Error::from_thread();
        if error.code() != ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
            return Err(error);
        }
    }
    Ok(())
}

unsafe extern "system" fn confirmation_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut state =
        unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ConfirmationWindowState };
    if message == WM_NCCREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        if !create.is_null() {
            state = unsafe { (*create).lpCreateParams.cast::<ConfirmationWindowState>() };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize) };
        }
    }
    if !state.is_null() {
        let state = unsafe { &mut *state };
        match message {
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as i32;
                if id == CONTROL_KEEP || id == CONTROL_REVERT {
                    let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                    let _ = unsafe {
                        PostMessageW(
                            Some(state.owner),
                            WM_APP_DISPLAY_DECISION,
                            WPARAM(id as usize),
                            LPARAM(0),
                        )
                    };
                }
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                let _ = unsafe {
                    PostMessageW(
                        Some(state.owner),
                        WM_APP_DISPLAY_DECISION,
                        WPARAM(CONTROL_REVERT as usize),
                        LPARAM(0),
                    )
                };
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == COUNTDOWN_TIMER_ID => {
                let remaining = state
                    .deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs_f64()
                    .ceil() as u64;
                let text = wide(&confirmation_message(remaining));
                let _ = unsafe { SetWindowTextW(state.message, PCWSTR(text.as_ptr())) };
                return LRESULT(0);
            }
            WM_NCDESTROY => unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            },
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

pub const fn decision_from_command(command: usize) -> Option<ConfirmationDecision> {
    match command as i32 {
        CONTROL_KEEP => Some(ConfirmationDecision::Keep),
        CONTROL_REVERT => Some(ConfirmationDecision::Revert),
        _ => None,
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
