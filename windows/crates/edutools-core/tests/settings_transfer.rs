use edutools_core::display_preset::{
    DisplayIdentity, DisplayKind, DisplayPreset, GridCell, PresetId, PresetMonitor, PresetScreen,
};
use edutools_core::settings::{AppSettings, SettingsError};
use edutools_core::settings_transfer::{SettingsBundle, SettingsTransfer};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("fixtures/pansetting")
        .join(name)
}

fn classroom_preset() -> DisplayPreset {
    DisplayPreset {
        id: PresetId::parse("11111111-2222-3333-4444-555555555555").unwrap(),
        name: "교실".to_owned(),
        main: PresetScreen {
            monitors: vec![PresetMonitor {
                identity: DisplayIdentity {
                    uuid: "display-test-uuid".to_owned(),
                    vendor: 1,
                    model: 2,
                    serial: 3,
                    name: "교실 모니터".to_owned(),
                    kind: DisplayKind::External,
                },
                auto_match: true,
            }],
            cell: GridCell::center(),
        },
        secondary1: None,
        secondary2: None,
    }
}

#[test]
fn platform_fixtures_apply_all_bundles_to_default() {
    for name in ["macos-v1.pansetting", "windows-v1.pansetting"] {
        let data = fs::read(fixture_path(name)).unwrap();
        let document = SettingsTransfer::decode(&data).unwrap();
        let imported = SettingsTransfer::applied_settings(
            &AppSettings::default(),
            &document,
            document.included_bundles.iter(),
        )
        .unwrap();

        assert_eq!(imported, AppSettings::default(), "fixture: {name}");
        assert_eq!(document.included_bundles, SettingsBundle::ALL);
        assert!(!document.includes_computer_specific);
    }
}

#[test]
fn export_includes_presets_but_excludes_computer_specific_values() {
    let mut settings = AppSettings::default();
    settings.general.archive_folder_path = Some("C:\\Classroom\\Archive".to_owned());
    settings.display.auto_switch_enabled = true;
    settings.display.presets = vec![classroom_preset()];

    let mut current = AppSettings::default();
    current.general.archive_folder_path = Some("C:\\Existing\\Archive".to_owned());
    let mut old_preset = classroom_preset();
    old_preset.id = PresetId::parse("AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE").unwrap();
    old_preset.name = "기존 프리셋".to_owned();
    current.display.presets = vec![old_preset];

    let document = SettingsTransfer::export_all(&settings, "0.1.0", "Windows 11").unwrap();
    let encoded: Value =
        serde_json::from_slice(&SettingsTransfer::encode(&document).unwrap()).unwrap();
    let imported =
        SettingsTransfer::applied_settings(&current, &document, document.included_bundles.iter())
            .unwrap();

    assert_eq!(document.included_bundles, SettingsBundle::ALL);
    assert_eq!(document.payload.general.unwrap().archive_folder_path, None);
    assert_eq!(
        document.payload.display.unwrap().presets,
        settings.display.presets
    );
    assert!(encoded["payload"]["display"]
        .get("autoSwitchEnabled")
        .is_none());
    assert_eq!(
        imported.general.archive_folder_path,
        current.general.archive_folder_path
    );
    assert_eq!(imported.display.presets, settings.display.presets);
    assert!(!imported.display.auto_switch_enabled);
}

#[test]
fn display_preview_is_one_changed_row() {
    let mut settings = AppSettings::default();
    settings.display.presets = vec![classroom_preset()];
    let document = SettingsTransfer::export_document(
        &settings,
        [SettingsBundle::Display],
        false,
        "ignored-version",
        "any operating system",
    )
    .unwrap();

    let changes = SettingsTransfer::changes(
        &AppSettings::default(),
        &document,
        [SettingsBundle::Display],
    )
    .unwrap();

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].id, "화면 구성");
    assert_eq!(changes[0].imported_value, "프리셋 1개 추가/교체");
}

#[test]
fn decode_rejects_format_version_and_unknown_bundle_in_that_order() {
    let document =
        SettingsTransfer::export_all(&AppSettings::default(), "0.1.0", "Windows 11").unwrap();

    let mut wrong_format = document.clone();
    wrong_format.format = "other-format".to_owned();
    wrong_format.format_version = 99;
    assert!(matches!(
        SettingsTransfer::decode(&SettingsTransfer::encode(&wrong_format).unwrap()),
        Err(SettingsError::UnsupportedFormat(value)) if value == "other-format"
    ));

    let mut wrong_version = document.clone();
    wrong_version.format_version = 99;
    assert!(matches!(
        SettingsTransfer::decode(&SettingsTransfer::encode(&wrong_version).unwrap()),
        Err(SettingsError::UnsupportedVersion(99))
    ));

    let mut unknown_bundle = serde_json::to_value(document).unwrap();
    unknown_bundle["includedBundles"] = serde_json::json!(["futureBundle"]);
    assert!(matches!(
        SettingsTransfer::decode(&serde_json::to_vec(&unknown_bundle).unwrap()),
        Err(SettingsError::InvalidValue(_))
    ));
}

#[test]
fn windows_fixture_matches_guarded_rust_export() {
    let document =
        SettingsTransfer::export_all(&AppSettings::default(), "0.1.0", "Windows 11").unwrap();
    let encoded = SettingsTransfer::encode(&document).unwrap();
    let path = fixture_path("windows-v1.pansetting");

    if std::env::var_os("WRITE_FIXTURES").as_deref() == Some(std::ffi::OsStr::new("1")) {
        fs::write(path, encoded).unwrap();
        return;
    }

    let stored = fs::read(path).unwrap();
    let stored_document = SettingsTransfer::decode(&stored).unwrap();
    assert_eq!(stored_document, document);
}
