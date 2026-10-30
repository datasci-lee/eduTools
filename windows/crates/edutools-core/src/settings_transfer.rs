//! `.pansetting` 문서의 OS 독립 전송 규칙.

use crate::display_preset::DisplayPreset;
use crate::settings::{
    AppSettings, BoardGroupSettings, EraserGroupSettings, GeneralSettings, HelpGroupSettings,
    SettingsError, SettingsValidator, ShortcutGroupSettings, TimerGroupSettings, ToolSettings,
    ZoomGroupSettings,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};

// Swift: SettingsModel.swift.SettingsBundle
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsBundle {
    General,
    Tools,
    Eraser,
    Zoom,
    Board,
    Timer,
    Display,
    Shortcuts,
    Help,
}

impl SettingsBundle {
    // Swift: SettingsModel.swift.SettingsBundle.allCases
    pub const ALL: [Self; 9] = [
        Self::General,
        Self::Tools,
        Self::Eraser,
        Self::Zoom,
        Self::Board,
        Self::Timer,
        Self::Display,
        Self::Shortcuts,
        Self::Help,
    ];
}

// Swift: SettingsModel.swift.DisplayPresetBundle
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisplayPresetBundle {
    pub presets: Vec<DisplayPreset>,
}

// Swift: SettingsModel.swift.SettingsPayload
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SettingsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub general: Option<GeneralSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eraser: Option<EraserGroupSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<ZoomGroupSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<BoardGroupSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timer: Option<TimerGroupSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<DisplayPresetBundle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcuts: Option<ShortcutGroupSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<HelpGroupSettings>,
}

// Swift: SettingsModel.swift.SettingsDocument
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDocument {
    pub format: String,
    pub format_version: i64,
    pub app_version: String,
    #[serde(rename = "createdOS")]
    pub created_os: String,
    pub included_bundles: Vec<SettingsBundle>,
    pub includes_computer_specific: bool,
    pub payload: SettingsPayload,
}

impl SettingsDocument {
    // Swift: SettingsModel.swift.SettingsDocument.format
    pub const FORMAT: &'static str = "kr.datasci.edutools.pansetting";
    // Swift: SettingsModel.swift.SettingsDocument.currentVersion
    pub const CURRENT_VERSION: i64 = 1;
}

// Swift: SettingsModel.swift.SettingsChange
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsChange {
    pub id: String,
    pub current_value: String,
    pub imported_value: String,
}

// Swift: SettingsModel.swift.SettingsTransfer
pub struct SettingsTransfer;

impl SettingsTransfer {
    // Swift: SettingsModel.swift.SettingsTransfer.exportDocument(settings:bundles:includeComputerSpecific:appVersion:osVersion:)
    pub fn export_document<I, B>(
        settings: &AppSettings,
        bundles: I,
        include_computer_specific: bool,
        app_version: &str,
        created_os: &str,
    ) -> Result<SettingsDocument, SettingsError>
    where
        I: IntoIterator<Item = B>,
        B: Borrow<SettingsBundle>,
    {
        SettingsValidator::validate(settings)?;
        let selected = bundle_set(bundles);
        let mut general = selected
            .contains(&SettingsBundle::General)
            .then(|| settings.general.clone());
        if !include_computer_specific {
            if let Some(general) = general.as_mut() {
                general.archive_folder_path = None;
            }
        }

        Ok(SettingsDocument {
            format: SettingsDocument::FORMAT.to_owned(),
            format_version: SettingsDocument::CURRENT_VERSION,
            app_version: app_version.to_owned(),
            created_os: created_os.to_owned(),
            included_bundles: SettingsBundle::ALL
                .into_iter()
                .filter(|bundle| selected.contains(bundle))
                .collect(),
            includes_computer_specific: include_computer_specific,
            payload: SettingsPayload {
                general,
                tools: selected
                    .contains(&SettingsBundle::Tools)
                    .then(|| settings.tools.clone()),
                eraser: selected
                    .contains(&SettingsBundle::Eraser)
                    .then(|| settings.eraser.clone()),
                zoom: selected
                    .contains(&SettingsBundle::Zoom)
                    .then(|| settings.zoom.clone()),
                board: selected
                    .contains(&SettingsBundle::Board)
                    .then(|| settings.board.clone()),
                timer: selected
                    .contains(&SettingsBundle::Timer)
                    .then(|| settings.timer.clone()),
                display: selected
                    .contains(&SettingsBundle::Display)
                    .then(|| DisplayPresetBundle {
                        presets: settings.display.presets.clone(),
                    }),
                shortcuts: selected
                    .contains(&SettingsBundle::Shortcuts)
                    .then(|| settings.shortcuts.clone()),
                help: selected
                    .contains(&SettingsBundle::Help)
                    .then(|| settings.help.clone()),
            },
        })
    }

    // Swift: SettingsModel.swift.SettingsTransfer.exportDocument(settings:)
    pub fn export_all(
        settings: &AppSettings,
        app_version: &str,
        created_os: &str,
    ) -> Result<SettingsDocument, SettingsError> {
        Self::export_document(
            settings,
            SettingsBundle::ALL,
            false,
            app_version,
            created_os,
        )
    }

    // Swift: SettingsModel.swift.SettingsTransfer.encode(_:)
    pub fn encode(document: &SettingsDocument) -> Result<Vec<u8>, SettingsError> {
        let value = serde_json::to_value(document)
            .map_err(|error| SettingsError::InvalidValue(error.to_string()))?;
        serde_json::to_vec_pretty(&value)
            .map_err(|error| SettingsError::InvalidValue(error.to_string()))
    }

    // Swift: SettingsModel.swift.SettingsTransfer.decode(_:)
    pub fn decode(data: &[u8]) -> Result<SettingsDocument, SettingsError> {
        let decoded: SettingsDocument = serde_json::from_slice(data)
            .map_err(|error| SettingsError::InvalidValue(error.to_string()))?;
        if decoded.format != SettingsDocument::FORMAT {
            return Err(SettingsError::UnsupportedFormat(decoded.format));
        }
        let migrated = migrate(decoded)?;
        Self::applied_settings(
            &AppSettings::default(),
            &migrated,
            migrated.included_bundles.iter(),
        )?;
        Ok(migrated)
    }

    // Swift: SettingsModel.swift.SettingsTransfer.appliedSettings(current:document:selectedBundles:)
    pub fn applied_settings<I, B>(
        current: &AppSettings,
        document: &SettingsDocument,
        selected_bundles: I,
    ) -> Result<AppSettings, SettingsError>
    where
        I: IntoIterator<Item = B>,
        B: Borrow<SettingsBundle>,
    {
        let selected = bundle_set(selected_bundles);
        applied_settings_with_set(current, document, &selected)
    }

    // Swift: SettingsModel.swift.SettingsTransfer.changes(current:document:selectedBundles:)
    pub fn changes<I, B>(
        current: &AppSettings,
        document: &SettingsDocument,
        selected_bundles: I,
    ) -> Result<Vec<SettingsChange>, SettingsError>
    where
        I: IntoIterator<Item = B>,
        B: Borrow<SettingsBundle>,
    {
        let selected = bundle_set(selected_bundles);
        let imported = applied_settings_with_set(current, document, &selected)?;
        let current_values = flattened_values(current)?;
        let imported_values = flattened_values(&imported)?;
        let mut result: Vec<SettingsChange> = imported_values
            .iter()
            .filter_map(|(key, imported_value)| {
                let current_value = current_values.get(key);
                (current_value != Some(imported_value)).then(|| SettingsChange {
                    id: key.clone(),
                    current_value: current_value.cloned().unwrap_or_else(|| "—".to_owned()),
                    imported_value: imported_value.clone(),
                })
            })
            .collect();

        if selected.contains(&SettingsBundle::Display) {
            if let Some(display) = document.payload.display.as_ref() {
                if current.display.presets != display.presets {
                    result.retain(|change| !change.id.starts_with("display.presets"));
                    result.push(SettingsChange {
                        id: "화면 구성".to_owned(),
                        current_value: format!("프리셋 {}개", current.display.presets.len()),
                        imported_value: format!("프리셋 {}개 추가/교체", display.presets.len()),
                    });
                }
            }
        }
        result.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(result)
    }
}

// Swift: SettingsModel.swift.SettingsTransfer.migrate(_:)
fn migrate(document: SettingsDocument) -> Result<SettingsDocument, SettingsError> {
    if document.format_version == SettingsDocument::CURRENT_VERSION {
        Ok(document)
    } else {
        Err(SettingsError::UnsupportedVersion(document.format_version))
    }
}

// Swift: SettingsModel.swift.SettingsTransfer.appliedSettings(current:document:selectedBundles:)
fn applied_settings_with_set(
    current: &AppSettings,
    document: &SettingsDocument,
    selected: &BTreeSet<SettingsBundle>,
) -> Result<AppSettings, SettingsError> {
    let mut result = current.clone();
    let payload = &document.payload;
    if selected.contains(&SettingsBundle::General) {
        if let Some(mut imported) = payload.general.clone() {
            if !document.includes_computer_specific {
                imported.archive_folder_path = current.general.archive_folder_path.clone();
            }
            result.general = imported;
        }
    }
    if selected.contains(&SettingsBundle::Tools) {
        if let Some(value) = payload.tools.as_ref() {
            result.tools = value.clone();
        }
    }
    if selected.contains(&SettingsBundle::Eraser) {
        if let Some(value) = payload.eraser.as_ref() {
            result.eraser = value.clone();
        }
    }
    if selected.contains(&SettingsBundle::Zoom) {
        if let Some(value) = payload.zoom.as_ref() {
            result.zoom = value.clone();
        }
    }
    if selected.contains(&SettingsBundle::Board) {
        if let Some(value) = payload.board.as_ref() {
            result.board = value.clone();
        }
    }
    if selected.contains(&SettingsBundle::Timer) {
        if let Some(value) = payload.timer.as_ref() {
            result.timer = value.clone();
        }
    }
    if selected.contains(&SettingsBundle::Display) {
        if let Some(value) = payload.display.as_ref() {
            result.display.presets = value.presets.clone();
        }
    }
    if selected.contains(&SettingsBundle::Shortcuts) {
        if let Some(value) = payload.shortcuts.as_ref() {
            result.shortcuts = value.clone();
        }
    }
    if selected.contains(&SettingsBundle::Help) {
        if let Some(value) = payload.help.as_ref() {
            result.help = value.clone();
        }
    }
    SettingsValidator::validate(&result)?;
    Ok(result)
}

// Swift: SettingsModel.swift.SettingsTransfer selected bundle Set construction
fn bundle_set<I, B>(bundles: I) -> BTreeSet<SettingsBundle>
where
    I: IntoIterator<Item = B>,
    B: Borrow<SettingsBundle>,
{
    bundles.into_iter().map(|value| *value.borrow()).collect()
}

// Swift: SettingsModel.swift.SettingsTransfer.flattenedValues(_:)
fn flattened_values<T: Serialize>(value: &T) -> Result<BTreeMap<String, String>, SettingsError> {
    let value = serde_json::to_value(value)
        .map_err(|error| SettingsError::InvalidValue(error.to_string()))?;
    let mut result = BTreeMap::new();
    flatten(&value, "", &mut result)?;
    Ok(result)
}

// Swift: SettingsModel.swift.SettingsTransfer.flatten(_:path:result:)
fn flatten(
    value: &Value,
    path: &str,
    result: &mut BTreeMap<String, String>,
) -> Result<(), SettingsError> {
    match value {
        Value::Object(values) => {
            for (key, value) in values {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                flatten(value, &child_path, result)?;
            }
        }
        Value::Array(_) => {
            result.insert(
                path.to_owned(),
                serde_json::to_string(value)
                    .map_err(|error| SettingsError::InvalidValue(error.to_string()))?,
            );
        }
        Value::Null => {
            result.insert(path.to_owned(), "—".to_owned());
        }
        Value::String(value) => {
            result.insert(path.to_owned(), value.clone());
        }
        Value::Bool(_) | Value::Number(_) => {
            result.insert(path.to_owned(), value.to_string());
        }
    }
    Ok(())
}
