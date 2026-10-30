use edutools_core::settings::{AppSettings, SettingsError};
use edutools_core::settings_store::{
    MemorySettingsStorage, SettingsBackupStore, SettingsStorage, SettingsStorageError,
    SettingsStore, SettingsStoreIssueKind,
};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
struct FaultStorage {
    data: Option<Vec<u8>>,
    preserved: Option<Vec<u8>>,
    fail_preserve: bool,
    mismatch_after_write: bool,
    writes: usize,
    wrote: bool,
}

impl SettingsStorage for FaultStorage {
    fn read(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError> {
        if self.wrote && self.mismatch_after_write {
            Ok(Some(b"mismatch".to_vec()))
        } else {
            Ok(self.data.clone())
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<(), SettingsStorageError> {
        self.writes += 1;
        self.wrote = true;
        self.data = Some(data.to_vec());
        Ok(())
    }

    fn read_preserved(&mut self) -> Result<Option<Vec<u8>>, SettingsStorageError> {
        Ok(self.preserved.clone())
    }

    fn preserve_corrupt(&mut self, data: &[u8]) -> Result<(), SettingsStorageError> {
        if self.fail_preserve {
            Err(SettingsStorageError::new("preserve failed"))
        } else {
            self.preserved = Some(data.to_vec());
            Ok(())
        }
    }

    fn delete(&mut self) -> Result<(), SettingsStorageError> {
        self.data = None;
        Ok(())
    }
}

#[test]
fn corrupt_load_preserves_and_only_then_removes_original() {
    let mut invalid_settings = AppSettings::default();
    invalid_settings.zoom.maximum_magnification = 9.0;
    let invalid_but_decodable = serde_json::to_vec(&invalid_settings).unwrap();

    for corrupt in [b"not settings json".to_vec(), invalid_but_decodable] {
        let mut store = SettingsStore::load(MemorySettingsStorage::new(Some(corrupt.clone())));

        assert_eq!(store.settings(), &AppSettings::default());
        assert_eq!(store.storage().bytes(), None);
        assert_eq!(store.storage().preserved_bytes(), Some(corrupt.as_slice()));
        assert_eq!(
            store.take_issue().unwrap().kind,
            SettingsStoreIssueKind::Load
        );
        assert_eq!(store.take_issue(), None);
    }
}

#[test]
fn corrupt_load_keeps_original_when_preservation_fails() {
    let corrupt = b"not settings json".to_vec();
    let storage = FaultStorage {
        data: Some(corrupt.clone()),
        fail_preserve: true,
        ..FaultStorage::default()
    };
    let mut store = SettingsStore::load(storage);

    assert_eq!(store.settings(), &AppSettings::default());
    assert_eq!(store.storage().data.as_deref(), Some(corrupt.as_slice()));
    assert_eq!(store.storage().preserved, None);
    assert_eq!(
        store.take_issue().unwrap().kind,
        SettingsStoreIssueKind::Load
    );
}

#[test]
fn update_skips_equal_values_and_rejects_invalid_candidates_before_commit() {
    let notifications = Rc::new(Cell::new(0));
    let observed_notifications = Rc::clone(&notifications);
    let mut store = SettingsStore::load(FaultStorage::default());
    store.add_observer(move |_| observed_notifications.set(observed_notifications.get() + 1));

    store.update(|_| {}).unwrap();
    assert_eq!(store.storage().writes, 0);
    assert_eq!(notifications.get(), 1);

    let result = store.update(|settings| settings.zoom.maximum_magnification = 9.0);
    assert!(matches!(result, Err(SettingsError::InvalidValue(_))));
    assert_eq!(store.settings(), &AppSettings::default());
    assert_eq!(store.storage().writes, 0);
    assert_eq!(notifications.get(), 1);

    store
        .update(|settings| settings.timer.lesson_default_minutes = 45)
        .unwrap();
    assert_eq!(store.settings().timer.lesson_default_minutes, 45);
    assert_eq!(store.storage().writes, 1);
    assert_eq!(notifications.get(), 2);
}

#[test]
fn save_readback_mismatch_reports_issue_after_committing_observable_value() {
    let storage = FaultStorage {
        mismatch_after_write: true,
        ..FaultStorage::default()
    };
    let mut store = SettingsStore::load(storage);

    store
        .update(|settings| settings.timer.lesson_default_minutes = 45)
        .unwrap();

    assert_eq!(store.settings().timer.lesson_default_minutes, 45);
    assert_eq!(
        store.take_issue().unwrap().kind,
        SettingsStoreIssueKind::Save
    );
}

#[test]
fn backups_rotate_to_most_recent_five() {
    let mut backups = SettingsBackupStore::load(MemorySettingsStorage::default()).unwrap();
    for minutes in 1..=6 {
        let mut settings = AppSettings::default();
        settings.timer.lesson_default_minutes = minutes;
        backups.append(&settings).unwrap();
    }

    assert_eq!(backups.count(), 5);
    let latest = backups.pop_latest().unwrap().unwrap();
    assert_eq!(latest.timer.lesson_default_minutes, 6);
    assert_eq!(backups.count(), 4);
}
