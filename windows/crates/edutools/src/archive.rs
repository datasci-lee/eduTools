//! Windows 자동 보관과 사라진 화면 복원.

use crate::monitor::hash_prefix8;
use edutools_core::ink::{decode_windows_archive, encode_windows_archive, InkLayerState, Size};
use edutools_core::state::DisplayDisconnectPolicy;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisconnectAction {
    None,
    Archive,
    MoveToPrimary,
    Delete,
}

pub const fn disconnect_action(
    policy: DisplayDisconnectPolicy,
    has_restorable_state: bool,
    primary_available: bool,
    move_succeeded: bool,
) -> DisconnectAction {
    if !has_restorable_state {
        return DisconnectAction::None;
    }
    match policy {
        DisplayDisconnectPolicy::Archive => DisconnectAction::Archive,
        DisplayDisconnectPolicy::Delete => DisconnectAction::Delete,
        DisplayDisconnectPolicy::MoveToPrimary if !primary_available => DisconnectAction::Archive,
        DisplayDisconnectPolicy::MoveToPrimary if move_succeeded => DisconnectAction::MoveToPrimary,
        DisplayDisconnectPolicy::MoveToPrimary => DisconnectAction::Archive,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreRequest {
    Missing,
    AlreadyOffered,
    Deferred,
    Offer,
}

#[derive(Debug, Default)]
pub struct RestoreOfferState {
    offered_keys: HashSet<String>,
    deferred_keys: HashSet<String>,
    is_deferred: bool,
}

impl RestoreOfferState {
    pub fn display_was_removed(&mut self, archive_key: &str) {
        self.offered_keys.remove(archive_key);
        self.deferred_keys.remove(archive_key);
    }

    pub fn request(&mut self, archive_key: &str, archive_exists: bool) -> RestoreRequest {
        if self.offered_keys.contains(archive_key) {
            return RestoreRequest::AlreadyOffered;
        }
        if !archive_exists {
            self.deferred_keys.remove(archive_key);
            return RestoreRequest::Missing;
        }
        if self.is_deferred {
            self.deferred_keys.insert(archive_key.to_owned());
            return RestoreRequest::Deferred;
        }
        self.deferred_keys.remove(archive_key);
        self.offered_keys.insert(archive_key.to_owned());
        RestoreRequest::Offer
    }

    pub fn set_deferred(&mut self, deferred: bool, resume_pending: bool) -> Vec<String> {
        self.is_deferred = deferred;
        if deferred || !resume_pending {
            return Vec::new();
        }
        let mut pending = self.deferred_keys.drain().collect::<Vec<_>>();
        pending.sort();
        pending
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn disconnected_archive_path(root: &Path, archive_key: &str) -> PathBuf {
    root.join("disconnected")
        .join(format!("{}.json", hash_prefix8(archive_key)))
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn automatic_archive_path(root: &Path, day: &str, time: &str, archive_key: &str) -> PathBuf {
    root.join(day)
        .join(format!("{}-{}.json", time, hash_prefix8(archive_key)))
}

pub fn write_archive(path: &Path, state: &InkLayerState, resolution: Size) -> Result<(), String> {
    let data = encode_windows_archive(state, resolution)?;
    atomic_write(path, &data)
}

pub fn read_archive_preserving_corrupt(
    path: &Path,
) -> Result<Option<(InkLayerState, Size)>, String> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            preserve_corrupt(path)?;
            return Err("archive file could not be read and was preserved as corrupt".to_owned());
        }
    };
    match decode_windows_archive(&data) {
        Ok(document) => Ok(Some(document)),
        Err(_) => {
            preserve_corrupt(path)?;
            Err("archive document was corrupt and was preserved".to_owned())
        }
    }
}

fn preserve_corrupt(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err("archive source was unavailable for corrupt preservation".to_owned());
    }
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("archive");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for sequence in 1_u32..=10_000 {
        let suffix = if sequence == 1 {
            "corrupt".to_owned()
        } else {
            format!("corrupt-{sequence}")
        };
        let candidate = parent.join(format!("{file_stem}.{suffix}.json"));
        if candidate.exists() {
            continue;
        }
        fs::rename(path, &candidate)
            .map_err(|_| "archive corrupt preservation rename failed".to_owned())?;
        return Ok(candidate);
    }
    Err("archive corrupt preservation exhausted file names".to_owned())
}

#[cfg(windows)]
fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    crate::settings_file::FileSettingsStorage::atomic_write(path, data)
        .map_err(|_| "archive atomic write failed".to_owned())
}

#[cfg(not(windows))]
fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "archive path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "archive directory creation failed".to_owned())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(|_| "archive temporary write failed".to_owned())?;
    fs::rename(temporary, path).map_err(|_| "archive replacement failed".to_owned())
}

#[cfg(windows)]
use crate::log::Logger;
#[cfg(windows)]
use crate::palette::app_data_file;
#[cfg(windows)]
use crate::z_bands::{raise_band_order, Band, BandRegistration};
#[cfg(windows)]
use edutools_core::settings::AppSettings;
#[cfg(windows)]
use edutools_core::state::AppState;
#[cfg(windows)]
use windows::core::{w, PCWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::System::SystemInformation::GetLocalTime;
#[cfg(windows)]
use windows::Win32::UI::Controls::{
    TaskDialogIndirect, TASKDIALOGCONFIG, TASKDIALOG_BUTTON, TDF_POSITION_RELATIVE_TO_WINDOW,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONWARNING, MB_SETFOREGROUND, MB_YESNO,
};

#[cfg(windows)]
pub struct ArchiveController {
    owner: HWND,
    logger: Logger,
    app_archive_root: Option<PathBuf>,
    automatic_archive_root: Option<PathBuf>,
    auto_archive_enabled: bool,
    policy: DisplayDisconnectPolicy,
    automatically_restores: bool,
    offers: RestoreOfferState,
}

#[cfg(windows)]
impl ArchiveController {
    pub fn new(owner: HWND, settings: &AppSettings, logger: Logger) -> Self {
        let mut controller = Self {
            owner,
            logger,
            app_archive_root: app_data_file("archive"),
            automatic_archive_root: None,
            auto_archive_enabled: false,
            policy: DisplayDisconnectPolicy::Archive,
            automatically_restores: false,
            offers: RestoreOfferState::default(),
        };
        controller.update_settings(settings);
        controller
    }

    pub fn update_settings(&mut self, settings: &AppSettings) {
        self.auto_archive_enabled = settings.general.auto_archive_enabled;
        self.automatic_archive_root = settings
            .general
            .archive_folder_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| self.app_archive_root.clone());
        self.policy = AppState::display_disconnect_policy(settings)
            .unwrap_or(DisplayDisconnectPolicy::Archive);
        self.automatically_restores = settings
            .general
            .automatically_restores_disconnected_displays;
    }

    pub const fn auto_archive_enabled(&self) -> bool {
        self.auto_archive_enabled
    }

    pub const fn policy(&self) -> DisplayDisconnectPolicy {
        self.policy
    }

    pub fn display_was_removed(&mut self, archive_key: &str) {
        self.offers.display_was_removed(archive_key);
    }

    pub fn archive_visible(
        &self,
        archive_key: &str,
        state: &InkLayerState,
        resolution: Size,
    ) -> Result<(), String> {
        let root = self
            .automatic_archive_root
            .as_deref()
            .ok_or_else(|| "automatic archive root is unavailable".to_owned())?;
        let (day, time) = local_archive_components();
        let path = automatic_archive_path(root, &day, &time, archive_key);
        write_archive(&path, state, resolution)?;
        self.logger.event(&format!(
            "자동 보관 저장: monitorHash={} formatVersion=1",
            hash_prefix8(archive_key)
        ));
        Ok(())
    }

    pub fn store_disconnected(
        &self,
        archive_key: &str,
        state: &InkLayerState,
        resolution: Size,
        reason: &str,
    ) -> Result<bool, String> {
        if !state.has_unscoped_restorable_state() {
            return Ok(false);
        }
        let root = self
            .app_archive_root
            .as_deref()
            .ok_or_else(|| "APPDATA archive root is unavailable".to_owned())?;
        let path = disconnected_archive_path(root, archive_key);
        write_archive(&path, state, resolution)?;
        self.logger.event(&format!(
            "연결 해제 화면 보관 저장: reason={reason} monitorHash={} formatVersion=1",
            hash_prefix8(archive_key)
        ));
        Ok(true)
    }

    pub fn restore_if_needed(
        &mut self,
        archive_key: &str,
    ) -> Result<Option<InkLayerState>, String> {
        let Some(root) = self.app_archive_root.as_deref() else {
            return Ok(None);
        };
        let path = disconnected_archive_path(root, archive_key);
        match self.offers.request(archive_key, path.exists()) {
            RestoreRequest::Missing | RestoreRequest::AlreadyOffered => return Ok(None),
            RestoreRequest::Deferred => {
                self.logger.event(&format!(
                    "연결 화면 복원 제안 보류: monitorHash={}",
                    hash_prefix8(archive_key)
                ));
                return Ok(None);
            }
            RestoreRequest::Offer => {}
        }
        self.logger.event(&format!(
            "연결 화면 복원 제안: monitorHash={} automatic={}",
            hash_prefix8(archive_key),
            self.automatically_restores
        ));
        if !self.automatically_restores && !self.ask_restore() {
            self.logger.event(&format!(
                "연결 화면 복원 나중에: monitorHash={}",
                hash_prefix8(archive_key)
            ));
            return Ok(None);
        }
        self.restore_stored(archive_key)
    }

    pub fn restore_stored(&self, archive_key: &str) -> Result<Option<InkLayerState>, String> {
        let root = self
            .app_archive_root
            .as_deref()
            .ok_or_else(|| "APPDATA archive root is unavailable".to_owned())?;
        let path = disconnected_archive_path(root, archive_key);
        let Some((state, _resolution)) = read_archive_preserving_corrupt(&path)? else {
            return Ok(None);
        };
        if !state.has_unscoped_restorable_state() {
            preserve_corrupt(&path)?;
            return Err("archive document contained no restorable state".to_owned());
        }
        if fs::remove_file(&path).is_err() {
            self.logger.event(&format!(
                "연결 화면 복원 후 보관 파일 제거 실패: monitorHash={}",
                hash_prefix8(archive_key)
            ));
        }
        self.logger.event(&format!(
            "연결 화면 복원: monitorHash={} normalizedCoordinates=true",
            hash_prefix8(archive_key)
        ));
        Ok(Some(state))
    }

    pub fn set_preset_restoration_deferred(
        &mut self,
        deferred: bool,
        resume_pending: bool,
    ) -> Vec<String> {
        let pending = self.offers.set_deferred(deferred, resume_pending);
        self.logger.event(&format!(
            "set_display_preset_restoration_deferred: deferred={deferred} resumePending={resume_pending} pending={}",
            pending.len()
        ));
        pending
    }

    pub fn report_failure(&self, operation: &str, error: &str) {
        self.logger.event(&format!(
            "보관/복원 실패: operation={operation} detail={error}"
        ));
        self.show_warning(
            "판서 보관 오류",
            "판서 보관 또는 복원을 완료하지 못했습니다. 원본 상태와 손상 파일은 가능한 한 유지했습니다.",
        );
    }

    fn ask_restore(&self) -> bool {
        let _band = BandRegistration::new(self.owner, Band::Dialog);
        let _ = raise_band_order();
        let restore = wide("복원");
        let later = wide("나중에");
        let buttons = [
            TASKDIALOG_BUTTON {
                nButtonID: 1001,
                pszButtonText: PCWSTR(restore.as_ptr()),
            },
            TASKDIALOG_BUTTON {
                nButtonID: 1002,
                pszButtonText: PCWSTR(later.as_ptr()),
            },
        ];
        let configuration = TASKDIALOGCONFIG {
            cbSize: u32::try_from(std::mem::size_of::<TASKDIALOGCONFIG>())
                .expect("TASKDIALOGCONFIG size fits u32"),
            hwndParent: self.owner,
            dwFlags: TDF_POSITION_RELATIVE_TO_WINDOW,
            pszWindowTitle: w!("EduTools"),
            pszMainInstruction: w!("이전 판서를 복원할까요?"),
            pszContent: w!("다시 연결된 화면의 판서와 실행 취소 기록을 복원합니다."),
            cButtons: u32::try_from(buttons.len()).expect("button count fits u32"),
            pButtons: buttons.as_ptr(),
            nDefaultButton: 1001,
            ..Default::default()
        };
        let mut selected = 0;
        if unsafe { TaskDialogIndirect(&configuration, Some(&mut selected), None, None) }.is_ok() {
            selected == 1001
        } else {
            unsafe {
                MessageBoxW(
                    Some(self.owner),
                    w!("다시 연결된 화면의 판서와 실행 취소 기록을 복원합니다.\n\n예: 복원\n아니요: 나중에"),
                    w!("이전 판서를 복원할까요?"),
                    MB_YESNO | MB_ICONINFORMATION | MB_SETFOREGROUND,
                ) == IDYES
            }
        }
    }

    fn show_warning(&self, title: &str, message: &str) {
        let title = wide(title);
        let message = wide(message);
        let _band = BandRegistration::new(self.owner, Band::Dialog);
        let _ = raise_band_order();
        unsafe {
            MessageBoxW(
                Some(self.owner),
                PCWSTR(message.as_ptr()),
                PCWSTR(title.as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::MB_OK | MB_ICONWARNING | MB_SETFOREGROUND,
            );
        }
    }
}

#[cfg(windows)]
fn local_archive_components() -> (String, String) {
    let value = unsafe { GetLocalTime() };
    (
        format!("{:04}-{:02}-{:02}", value.wYear, value.wMonth, value.wDay),
        format!("{:02}{:02}{:02}", value.wHour, value.wMinute, value.wSecond),
    )
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn selftest_failures() -> Vec<String> {
    use edutools_core::ink::InkCommand;

    let mut failures = Vec::new();
    let mut state = InkLayerState::new();
    state.execute(InkCommand::ReplaceStrokes {
        before: Vec::new(),
        after: Vec::new(),
    });
    let resolution = Size {
        width: 1_920.0,
        height: 1_080.0,
    };
    match encode_windows_archive(&state, resolution).and_then(|data| decode_windows_archive(&data))
    {
        Ok((decoded, decoded_resolution))
            if decoded == state && decoded_resolution == resolution => {}
        Ok(_) => failures.push("archive: JSON 왕복 결과 불일치".to_owned()),
        Err(error) => failures.push(format!("archive: JSON 왕복 실패: {error}")),
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::{
        disconnect_action, read_archive_preserving_corrupt, write_archive, DisconnectAction,
        RestoreOfferState, RestoreRequest,
    };
    use edutools_core::ink::{InkCommand, InkLayerState, Size};
    use edutools_core::state::DisplayDisconnectPolicy;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn disconnect_policy_branch_table() {
        let cases = [
            (
                "archive-with-state",
                DisplayDisconnectPolicy::Archive,
                true,
                false,
                false,
                DisconnectAction::Archive,
            ),
            (
                "archive-without-state",
                DisplayDisconnectPolicy::Archive,
                false,
                false,
                false,
                DisconnectAction::None,
            ),
            (
                "move-success",
                DisplayDisconnectPolicy::MoveToPrimary,
                true,
                true,
                true,
                DisconnectAction::MoveToPrimary,
            ),
            (
                "move-failure-falls-back",
                DisplayDisconnectPolicy::MoveToPrimary,
                true,
                true,
                false,
                DisconnectAction::Archive,
            ),
            (
                "move-without-primary-falls-back",
                DisplayDisconnectPolicy::MoveToPrimary,
                true,
                false,
                false,
                DisconnectAction::Archive,
            ),
            (
                "delete-with-state",
                DisplayDisconnectPolicy::Delete,
                true,
                false,
                false,
                DisconnectAction::Delete,
            ),
            (
                "delete-without-state",
                DisplayDisconnectPolicy::Delete,
                false,
                false,
                false,
                DisconnectAction::None,
            ),
        ];
        for (name, policy, has_state, primary, moved, expected) in cases {
            assert_eq!(
                disconnect_action(policy, has_state, primary, moved),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn archive_json_round_trip_and_corrupt_preservation_table() {
        let directory = unique_temp_directory("round-trip-corrupt");
        let path = directory.join("display.json");
        let corrupt_path = directory.join("display.corrupt.json");
        let mut state = InkLayerState::new();
        state.execute(InkCommand::ReplaceStrokes {
            before: Vec::new(),
            after: Vec::new(),
        });
        let resolution = Size {
            width: 2_560.0,
            height: 1_440.0,
        };
        write_archive(&path, &state, resolution).expect("archive write");
        let encoded =
            String::from_utf8(fs::read(&path).expect("encoded archive")).expect("archive UTF-8");
        let round_trip = read_archive_preserving_corrupt(&path)
            .expect("archive read")
            .expect("archive document");
        fs::write(&path, b"{not-json").expect("corrupt fixture write");
        let corrupt_result = read_archive_preserving_corrupt(&path);
        let observations = [
            ("format-version", encoded.contains("\"formatVersion\": 1")),
            (
                "round-trip",
                round_trip.0 == state && round_trip.1 == resolution,
            ),
            (
                "corrupt-preserved",
                corrupt_result.is_err() && !path.exists() && corrupt_path.exists(),
            ),
        ];
        assert_eq!(
            observations,
            [
                ("format-version", true),
                ("round-trip", true),
                ("corrupt-preserved", true),
            ]
        );
        fs::remove_dir_all(directory).expect("remove archive test directory");
    }

    #[test]
    fn restore_offer_deduplication_and_deferred_resume_table() {
        let mut state = RestoreOfferState::default();
        let mut observations = Vec::new();
        observations.push(("missing", state.request("display-a", false)));
        assert!(state.set_deferred(true, true).is_empty());
        observations.push(("deferred", state.request("display-a", true)));
        assert_eq!(state.set_deferred(false, true), vec!["display-a"]);
        observations.push(("offer", state.request("display-a", true)));
        observations.push(("duplicate", state.request("display-a", true)));
        state.display_was_removed("display-a");
        observations.push(("reoffer-after-removal", state.request("display-a", true)));
        assert_eq!(
            observations,
            [
                ("missing", RestoreRequest::Missing),
                ("deferred", RestoreRequest::Deferred),
                ("offer", RestoreRequest::Offer),
                ("duplicate", RestoreRequest::AlreadyOffered),
                ("reoffer-after-removal", RestoreRequest::Offer),
            ]
        );
    }

    fn unique_temp_directory(label: &str) -> std::path::PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "edutools-archive-{label}-{}-{timestamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
