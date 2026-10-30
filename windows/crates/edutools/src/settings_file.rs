use crate::palette::app_data_file;
use edutools_core::settings::AppSettings;
use edutools_core::settings_store::{SettingsStorage, SettingsStorageError, SettingsStore};
use edutools_core::settings_transfer::{SettingsBundle, SettingsTransfer};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FlushFileBuffers, MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

const SETTINGS_FILE_NAME: &str = "settings.json";
const CORRUPT_SETTINGS_FILE_NAME: &str = "settings.corrupt.json";
const BACKUPS_FILE_NAME: &str = "settings-backups.json";
const CORRUPT_BACKUPS_FILE_NAME: &str = "settings-backups.corrupt.json";

/// `%APPDATA%\EduTools` 안의 설정 문서를 core `SettingsStorage`에 연결한다.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSettingsStorage {
    path: PathBuf,
    corrupt_path: PathBuf,
}

impl FileSettingsStorage {
    pub fn app_settings() -> Result<Self, SettingsStorageError> {
        let path = app_data_file(SETTINGS_FILE_NAME).ok_or_else(|| {
            SettingsStorageError::new("APPDATA 환경 변수가 없어 설정 경로를 만들 수 없습니다")
        })?;
        let corrupt_path = app_data_file(CORRUPT_SETTINGS_FILE_NAME).ok_or_else(|| {
            SettingsStorageError::new("APPDATA 환경 변수가 없어 손상 설정 경로를 만들 수 없습니다")
        })?;
        Ok(Self::new(path, corrupt_path))
    }

    pub fn app_backups() -> Result<Self, SettingsStorageError> {
        let path = app_data_file(BACKUPS_FILE_NAME).ok_or_else(|| {
            SettingsStorageError::new("APPDATA 환경 변수가 없어 백업 경로를 만들 수 없습니다")
        })?;
        let corrupt_path = app_data_file(CORRUPT_BACKUPS_FILE_NAME).ok_or_else(|| {
            SettingsStorageError::new("APPDATA 환경 변수가 없어 손상 백업 경로를 만들 수 없습니다")
        })?;
        Ok(Self::new(path, corrupt_path))
    }

    pub fn new(path: PathBuf, corrupt_path: PathBuf) -> Self {
        Self { path, corrupt_path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn corrupt_path(&self) -> &Path {
        &self.corrupt_path
    }

    fn read_path(path: &Path) -> Result<Option<Vec<u8>>, SettingsStorageError> {
        match fs::read(path) {
            Ok(data) => Ok(Some(data)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(storage_error("설정 파일 읽기", path, error)),
        }
    }

    pub(crate) fn atomic_write(path: &Path, data: &[u8]) -> Result<(), SettingsStorageError> {
        let parent = path.parent().ok_or_else(|| {
            SettingsStorageError::new(format!(
                "설정 경로에 상위 폴더가 없습니다: {}",
                path.display()
            ))
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| storage_error("설정 폴더 만들기", parent, error))?;

        let temporary_path = temporary_path(path)?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary_path)
            .map_err(|error| storage_error("설정 임시 파일 열기", &temporary_path, error))?;
        file.write_all(data)
            .map_err(|error| storage_error("설정 임시 파일 쓰기", &temporary_path, error))?;
        file.flush()
            .map_err(|error| storage_error("설정 임시 파일 flush", &temporary_path, error))?;
        // SAFETY: `file` owns a live Windows file handle for the temporary file.
        unsafe { FlushFileBuffers(HANDLE(file.as_raw_handle())) }.map_err(|error| {
            SettingsStorageError::new(format!(
                "설정 임시 파일 FlushFileBuffers 실패: {}: {error}",
                temporary_path.display()
            ))
        })?;
        drop(file);

        let temporary_wide = wide_path(&temporary_path);
        let destination_wide = wide_path(path);
        // SAFETY: both buffers are NUL-terminated for the duration of the call. The destination
        // stays on the same volume because the temporary file is next to it.
        unsafe {
            MoveFileExW(
                PCWSTR(temporary_wide.as_ptr()),
                PCWSTR(destination_wide.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|error| {
            SettingsStorageError::new(format!(
                "설정 원자 교체 실패: {} -> {}: {error}",
                temporary_path.display(),
                path.display()
            ))
        })
    }
}

impl SettingsStorage for FileSettingsStorage {
    fn read(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError> {
        Self::read_path(&self.path)
    }

    fn write(&mut self, data: &[u8]) -> Result<(), SettingsStorageError> {
        Self::atomic_write(&self.path, data)
    }

    fn read_preserved(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError> {
        Self::read_path(&self.corrupt_path)
    }

    fn preserve_corrupt(&mut self, data: &[u8]) -> Result<(), SettingsStorageError> {
        Self::atomic_write(&self.corrupt_path, data)
    }

    fn delete(&mut self) -> Result<(), SettingsStorageError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(storage_error("설정 파일 제거", &self.path, error)),
        }
    }
}

pub fn selftest_failures() -> Vec<String> {
    let mut failures = Vec::new();
    let directory = unique_temp_directory("selftest");
    let path = directory.join(SETTINGS_FILE_NAME);
    let corrupt_path = directory.join(CORRUPT_SETTINGS_FILE_NAME);
    let storage = FileSettingsStorage::new(path, corrupt_path);
    let mut store = SettingsStore::load(storage.clone());
    if let Err(error) = store.update(|settings| settings.help.shows_help_in_sharing = true) {
        failures.push(format!(
            "settings storage: selftest 저장 갱신 거부: {error}"
        ));
    }
    if let Some(issue) = store.take_issue() {
        failures.push(format!(
            "settings storage: selftest 저장 문제({:?}): {}",
            issue.kind, issue.detail
        ));
    }
    let expected = store.settings().clone();
    let mut reloaded = SettingsStore::load(storage);
    if let Some(issue) = reloaded.take_issue() {
        failures.push(format!(
            "settings storage: selftest 재로드 문제({:?}): {}",
            issue.kind, issue.detail
        ));
    }
    if reloaded.settings() != &expected {
        failures.push("settings storage: 임시 경로 저장/재로드 결과 불일치".to_owned());
    }
    let transfer_path = directory.join("selftest.pansetting");
    let transfer_result = SettingsTransfer::export_all(
        &AppSettings::default(),
        env!("CARGO_PKG_VERSION"),
        "Windows selftest",
    )
    .and_then(|document| SettingsTransfer::encode(&document))
    .map_err(|error| error.to_string())
    .and_then(|data| {
        FileSettingsStorage::atomic_write(&transfer_path, &data)
            .map_err(|error| error.to_string())?;
        let written = fs::read(&transfer_path).map_err(|error| error.to_string())?;
        let decoded = SettingsTransfer::decode(&written).map_err(|error| error.to_string())?;
        SettingsTransfer::applied_settings(&AppSettings::default(), &decoded, SettingsBundle::ALL)
            .map_err(|error| error.to_string())
    });
    match transfer_result {
        Ok(applied) if applied == AppSettings::default() => {}
        Ok(_) => {
            failures.push("settings transfer: export/decode/apply 왕복 결과 불일치".to_owned())
        }
        Err(error) => failures.push(format!(
            "settings transfer: export/decode/apply 왕복 실패: {error}"
        )),
    }
    if let Err(error) = fs::remove_dir_all(&directory) {
        failures.push(format!(
            "settings storage: selftest 임시 폴더 정리 실패 {}: {error}",
            directory.display()
        ));
    }
    failures
}

fn storage_error(operation: &str, path: &Path, error: std::io::Error) -> SettingsStorageError {
    SettingsStorageError::new(format!("{operation} 실패: {}: {error}", path.display()))
}

fn temporary_path(path: &Path) -> Result<PathBuf, SettingsStorageError> {
    let file_name = path.file_name().ok_or_else(|| {
        SettingsStorageError::new(format!("설정 파일 이름이 없습니다: {}", path.display()))
    })?;
    let mut temporary_name = file_name.to_os_string();
    temporary_name.push(".tmp");
    Ok(path.with_file_name(temporary_name))
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn unique_temp_directory(label: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "edutools-settings-{label}-{}-{timestamp}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::{unique_temp_directory, FileSettingsStorage};
    use edutools_core::settings_store::{SettingsStorage, SettingsStore, SettingsStoreIssueKind};
    use std::fs;

    fn storage(label: &str) -> (std::path::PathBuf, FileSettingsStorage) {
        let directory = unique_temp_directory(label);
        let storage = FileSettingsStorage::new(
            directory.join("settings.json"),
            directory.join("settings.corrupt.json"),
        );
        (directory, storage)
    }

    #[test]
    fn file_storage_round_trip_and_atomic_replacement_table() {
        let cases: &[(&str, &[u8], &[u8])] = &[
            (
                "round-trip",
                b"first settings document",
                b"first settings document",
            ),
            (
                "replacement-does-not-keep-tail",
                b"a much longer previous settings document",
                b"{}",
            ),
        ];
        for (label, first, expected) in cases {
            let (directory, mut storage) = storage(label);
            storage.write(first).expect("first write");
            if *label == "replacement-does-not-keep-tail" {
                storage.write(expected).expect("replacement write");
            }
            assert_eq!(
                storage.read().expect("read"),
                Some(expected.to_vec()),
                "{label}"
            );
            fs::remove_dir_all(directory).expect("remove temporary settings directory");
        }
    }

    #[test]
    fn corrupt_document_is_preserved_and_original_is_removed() {
        let (directory, mut storage) = storage("corrupt");
        let corrupt = br#"{"general":"not-an-AppSettings"}"#;
        storage.write(corrupt).expect("write corrupt settings");
        let mut store = SettingsStore::load(storage.clone());
        let issue = store.take_issue().expect("corrupt settings issue");
        assert_eq!(issue.kind, SettingsStoreIssueKind::Load);
        assert_eq!(
            fs::read(storage.corrupt_path()).expect("preserved bytes"),
            corrupt
        );
        assert!(
            !storage.path().exists(),
            "verified corrupt source must be removed"
        );
        fs::remove_dir_all(directory).expect("remove temporary settings directory");
    }
}
