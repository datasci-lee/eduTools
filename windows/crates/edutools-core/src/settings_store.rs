//! 검증된 설정 갱신, 손상 원본 보존, 백업 회전 규칙.

use crate::settings::{AppSettings, SettingsError, SettingsValidator};
use std::error::Error;
use std::fmt;

// Swift: SettingsModel.swift.SettingsPersistenceError
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsStorageError {
    pub detail: String,
}

impl SettingsStorageError {
    // Swift: SettingsModel.swift.SettingsPersistenceError.errorDescription
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

// Swift: SettingsModel.swift.SettingsPersistenceError.errorDescription
impl fmt::Display for SettingsStorageError {
    // Swift: SettingsModel.swift.SettingsPersistenceError.errorDescription
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for SettingsStorageError {}

// Swift: SettingsModel.swift.SettingsStore UserDefaults persistence operations
pub trait SettingsStorage {
    // Swift: UserDefaults.data(forKey: SettingsStore.storageKey)
    fn read(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError>;

    // Swift: UserDefaults.set(_:forKey: SettingsStore.storageKey)
    fn write(&mut self, data: &[u8]) -> Result<(), SettingsStorageError>;

    // Swift: UserDefaults.data(forKey: SettingsStore.corruptStorageKey)
    fn read_preserved(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError>;

    // Swift: UserDefaults.set(_:forKey: SettingsStore.corruptStorageKey)
    fn preserve_corrupt(&mut self, data: &[u8]) -> Result<(), SettingsStorageError>;

    // Swift: UserDefaults.removeObject(forKey: SettingsStore.storageKey)
    fn delete(&mut self) -> Result<(), SettingsStorageError>;
}

// Swift: SettingsModel.swift.SettingsStore test UserDefaults replacement
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemorySettingsStorage {
    data: Option<Vec<u8>>,
    preserved: Option<Vec<u8>>,
}

impl MemorySettingsStorage {
    // Swift: SettingsModel.swift.SettingsStore.init(defaults:)
    pub fn new(data: Option<Vec<u8>>) -> Self {
        Self {
            data,
            preserved: None,
        }
    }

    // Swift: UserDefaults.data(forKey: SettingsStore.storageKey)
    pub fn bytes(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    // Swift: UserDefaults.data(forKey: SettingsStore.corruptStorageKey)
    pub fn preserved_bytes(&self) -> Option<&[u8]> {
        self.preserved.as_deref()
    }
}

impl SettingsStorage for MemorySettingsStorage {
    // Swift: UserDefaults.data(forKey: SettingsStore.storageKey)
    fn read(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError> {
        Ok(self.data.clone())
    }

    // Swift: UserDefaults.set(_:forKey: SettingsStore.storageKey)
    fn write(&mut self, data: &[u8]) -> Result<(), SettingsStorageError> {
        self.data = Some(data.to_vec());
        Ok(())
    }

    // Swift: UserDefaults.data(forKey: SettingsStore.corruptStorageKey)
    fn read_preserved(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError> {
        Ok(self.preserved.clone())
    }

    // Swift: UserDefaults.set(_:forKey: SettingsStore.corruptStorageKey)
    fn preserve_corrupt(&mut self, data: &[u8]) -> Result<(), SettingsStorageError> {
        self.preserved = Some(data.to_vec());
        Ok(())
    }

    // Swift: UserDefaults.removeObject(forKey: SettingsStore.storageKey)
    fn delete(&mut self) -> Result<(), SettingsStorageError> {
        self.data = None;
        Ok(())
    }
}

// Swift: SettingsModel.swift.SettingsStoreIssue.Kind
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsStoreIssueKind {
    Load,
    Save,
}

// Swift: SettingsModel.swift.SettingsStoreIssue
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsStoreIssue {
    pub kind: SettingsStoreIssueKind,
    pub detail: String,
}

// Swift: SettingsModel.swift.SettingsStore
pub struct SettingsStore<S: SettingsStorage> {
    settings: AppSettings,
    storage: S,
    observers: Vec<Box<dyn FnMut(&AppSettings)>>,
    pending_issue: Option<SettingsStoreIssue>,
}

impl<S: SettingsStorage> SettingsStore<S> {
    // Swift: SettingsModel.swift.SettingsStore.init(defaults:)
    pub fn load(mut storage: S) -> Self {
        let mut pending_issue = None;
        let settings = match storage.read() {
            Ok(None) => AppSettings::default(),
            Ok(Some(data)) => match decode_and_validate(&data) {
                Ok(settings) => settings,
                Err(error) => {
                    let preservation = preserve_corrupt_original(&mut storage, &data);
                    pending_issue = Some(SettingsStoreIssue {
                        kind: SettingsStoreIssueKind::Load,
                        detail: format!(
                            "저장 설정을 읽지 못해 기본값으로 시작합니다. {error} {preservation}"
                        ),
                    });
                    AppSettings::default()
                }
            },
            Err(error) => {
                pending_issue = Some(SettingsStoreIssue {
                    kind: SettingsStoreIssueKind::Load,
                    detail: format!(
                        "저장 설정을 읽지 못해 기본값으로 시작합니다. {error} 원본 바이트를 읽지 못해 별도 보존하지 못했습니다."
                    ),
                });
                AppSettings::default()
            }
        };

        Self {
            settings,
            storage,
            observers: Vec::new(),
            pending_issue,
        }
    }

    // Swift: SettingsModel.swift.SettingsStore.settings
    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    // Swift: SettingsModel.swift.SettingsStore.addObserver(_:)
    pub fn add_observer<F>(&mut self, mut observer: F)
    where
        F: FnMut(&AppSettings) + 'static,
    {
        observer(&self.settings);
        self.observers.push(Box::new(observer));
    }

    // Swift: SettingsModel.swift.SettingsStore.update(_:)
    pub fn update<F>(&mut self, change: F) -> Result<(), SettingsError>
    where
        F: FnOnce(&mut AppSettings),
    {
        let mut candidate = self.settings.clone();
        change(&mut candidate);
        SettingsValidator::validate(&candidate)?;
        if candidate == self.settings {
            return Ok(());
        }

        self.settings = candidate;
        self.persist();
        for observer in &mut self.observers {
            observer(&self.settings);
        }
        Ok(())
    }

    // Swift: SettingsModel.swift.SettingsStore.pendingIssue
    pub fn take_issue(&mut self) -> Option<SettingsStoreIssue> {
        self.pending_issue.take()
    }

    // Swift: SettingsModel.swift.SettingsStore defaults dependency access
    pub fn storage(&self) -> &S {
        &self.storage
    }

    // Swift: SettingsModel.swift.SettingsStore defaults dependency access
    pub fn storage_mut(&mut self) -> &mut S {
        &mut self.storage
    }

    // Swift: SettingsModel.swift.SettingsStore defaults dependency ownership
    pub fn into_storage(self) -> S {
        self.storage
    }

    // Swift: SettingsModel.swift.SettingsStore.persist()
    fn persist(&mut self) {
        let result = serde_json::to_vec(&self.settings)
            .map_err(|error| SettingsStorageError::new(error.to_string()))
            .and_then(|data| {
                self.storage.write(&data)?;
                match self.storage.read()? {
                    Some(stored) if stored == data => Ok(()),
                    _ => Err(SettingsStorageError::new(
                        "저장소에서 기록한 값을 다시 확인하지 못했습니다",
                    )),
                }
            });
        if let Err(error) = result {
            self.pending_issue = Some(SettingsStoreIssue {
                kind: SettingsStoreIssueKind::Save,
                detail: format!(
                    "설정을 저장하지 못했습니다. 현재 변경은 다시 실행하면 사라질 수 있습니다. {error}"
                ),
            });
        }
    }
}

// Swift: SettingsModel.swift.SettingsStore JSON decode and SettingsValidator.validate(_:)
fn decode_and_validate(data: &[u8]) -> Result<AppSettings, String> {
    let settings: AppSettings = serde_json::from_slice(data).map_err(|error| error.to_string())?;
    SettingsValidator::validate(&settings).map_err(|error| error.to_string())?;
    Ok(settings)
}

// Swift: SettingsModel.swift.SettingsStore corrupt original preservation branch
fn preserve_corrupt_original<S: SettingsStorage>(storage: &mut S, data: &[u8]) -> String {
    if let Err(error) = storage.preserve_corrupt(data) {
        return format!("별도 키 보존에 실패해 원본을 기존 위치에 유지했습니다. {error}");
    }
    match storage.read_preserved() {
        Ok(Some(preserved)) if preserved == data => match storage.delete() {
            Ok(()) => "원본은 별도 손상 설정 위치에 보존했습니다.".to_owned(),
            Err(error) => {
                format!("원본은 별도 위치에 보존했지만 기존 위치에서 제거하지 못했습니다. {error}")
            }
        },
        Ok(_) => "별도 키 보존을 다시 확인하지 못해 원본을 기존 위치에 유지했습니다.".to_owned(),
        Err(error) => {
            format!("별도 키 보존을 다시 확인하지 못해 원본을 기존 위치에 유지했습니다. {error}")
        }
    }
}

// Swift: SettingsModel.swift.SettingsBackupStore persistence failures
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsBackupError {
    Storage(SettingsStorageError),
    Encoding(String),
    Validation(SettingsError),
    VerificationFailed,
}

// Swift: SettingsModel.swift.SettingsBackupStore persistence failure description
impl fmt::Display for SettingsBackupError {
    // Swift: SettingsModel.swift.SettingsBackupStore thrown error description
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "backup storage error: {error}"),
            Self::Encoding(error) => write!(formatter, "backup encoding error: {error}"),
            Self::Validation(error) => write!(formatter, "invalid backup: {error}"),
            Self::VerificationFailed => formatter.write_str("backup write verification failed"),
        }
    }
}

impl Error for SettingsBackupError {}

// Swift: SettingsModel.swift.SettingsBackupStore
pub struct SettingsBackupStore<S: SettingsStorage> {
    storage: S,
    backups: Vec<Vec<u8>>,
}

impl<S: SettingsStorage> SettingsBackupStore<S> {
    // Swift: SettingsModel.swift.SettingsBackupStore.maximumCount
    pub const MAXIMUM_COUNT: usize = 5;

    // Swift: SettingsModel.swift.SettingsBackupStore.init(defaults:)
    pub fn load(mut storage: S) -> Result<Self, SettingsBackupError> {
        let backups = match storage.read().map_err(SettingsBackupError::Storage)? {
            Some(data) => serde_json::from_slice(&data)
                .map_err(|error| SettingsBackupError::Encoding(error.to_string()))?,
            None => Vec::new(),
        };
        Ok(Self { storage, backups })
    }

    // Swift: SettingsModel.swift.SettingsBackupStore.count
    pub fn count(&self) -> usize {
        self.backups.len()
    }

    // Swift: SettingsModel.swift.SettingsBackupStore.append(_:)
    pub fn append(&mut self, settings: &AppSettings) -> Result<(), SettingsBackupError> {
        SettingsValidator::validate(settings).map_err(SettingsBackupError::Validation)?;
        let encoded = serde_json::to_vec(settings)
            .map_err(|error| SettingsBackupError::Encoding(error.to_string()))?;
        let mut backups = self.backups.clone();
        backups.push(encoded);
        if backups.len() > Self::MAXIMUM_COUNT {
            backups.drain(..backups.len() - Self::MAXIMUM_COUNT);
        }
        persist_backups(&mut self.storage, &backups)?;
        self.backups = backups;
        Ok(())
    }

    // Swift: SettingsModel.swift.SettingsBackupStore.popLatest()
    pub fn pop_latest(&mut self) -> Result<Option<AppSettings>, SettingsBackupError> {
        let Some(data) = self.backups.last() else {
            return Ok(None);
        };
        let settings: AppSettings = serde_json::from_slice(data)
            .map_err(|error| SettingsBackupError::Encoding(error.to_string()))?;
        SettingsValidator::validate(&settings).map_err(SettingsBackupError::Validation)?;
        let mut backups = self.backups.clone();
        backups.pop();
        persist_backups(&mut self.storage, &backups)?;
        self.backups = backups;
        Ok(Some(settings))
    }

    // Swift: SettingsModel.swift.SettingsBackupStore defaults dependency access
    pub fn storage(&self) -> &S {
        &self.storage
    }

    // Swift: SettingsModel.swift.SettingsBackupStore defaults dependency ownership
    pub fn into_storage(self) -> S {
        self.storage
    }
}

// Swift: SettingsModel.swift.SettingsBackupStore UserDefaults.set(_:forKey:)
fn persist_backups<S: SettingsStorage>(
    storage: &mut S,
    backups: &[Vec<u8>],
) -> Result<(), SettingsBackupError> {
    let data = serde_json::to_vec(backups)
        .map_err(|error| SettingsBackupError::Encoding(error.to_string()))?;
    storage.write(&data).map_err(SettingsBackupError::Storage)?;
    match storage.read().map_err(SettingsBackupError::Storage)? {
        Some(stored) if stored == data => Ok(()),
        _ => Err(SettingsBackupError::VerificationFailed),
    }
}
