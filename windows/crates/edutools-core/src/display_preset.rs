use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

// Swift: DisplayPresetDefaults
pub struct DisplayPresetDefaults;

impl DisplayPresetDefaults {
    pub const MAXIMUM_PRESET_COUNT: usize = 8;
    pub const MAXIMUM_NAME_LENGTH: usize = 40;
}

// Swift: DisplayKind
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayKind {
    BuiltIn,
    External,
    Virtual,
}

// Swift: GridCell
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct GridCell {
    pub x: i64,
    pub y: i64,
}

impl GridCell {
    // Swift: GridCell.center
    pub const fn center() -> Self {
        Self { x: 0, y: 0 }
    }

    // Swift: GridCell.all
    pub fn all() -> Vec<Self> {
        (-1..=1)
            .flat_map(|y| (-1..=1).map(move |x| Self { x, y }))
            .collect()
    }

    // Swift: GridCell.placementOrder
    pub const fn placement_order() -> [Self; 8] {
        [
            Self { x: 1, y: 0 },
            Self { x: -1, y: 0 },
            Self { x: 0, y: -1 },
            Self { x: 0, y: 1 },
            Self { x: -1, y: -1 },
            Self { x: 1, y: -1 },
            Self { x: -1, y: 1 },
            Self { x: 1, y: 1 },
        ]
    }

    // Swift: GridCell.isInBounds
    pub fn is_in_bounds(self) -> bool {
        (-1..=1).contains(&self.x) && (-1..=1).contains(&self.y)
    }

    // Swift: GridCell.isOrthogonallyAdjacent(to:)
    pub fn is_orthogonally_adjacent(self, other: Self) -> bool {
        (self.x - other.x).abs() + (self.y - other.y).abs() == 1
    }
}

// Swift: UUID used by DisplayPreset.id
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PresetId(String);

// Swift: UUID(uuidString:)
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresetIdError;

impl PresetId {
    // Swift: UUID.init(uuidString:)
    pub fn parse(value: &str) -> Result<Self, PresetIdError> {
        let bytes = value.as_bytes();
        if bytes.len() != 36 {
            return Err(PresetIdError);
        }
        for (index, byte) in bytes.iter().copied().enumerate() {
            let is_separator = matches!(index, 8 | 13 | 18 | 23);
            if (is_separator && byte != b'-') || (!is_separator && !byte.is_ascii_hexdigit()) {
                return Err(PresetIdError);
            }
        }
        Ok(Self(value.to_ascii_uppercase()))
    }

    // Swift: UUID.uuidString
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Swift: UUID Codable encoding
impl Serialize for PresetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

// Swift: UUID Codable decoding
impl<'de> Deserialize<'de> for PresetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(|_| de::Error::custom("invalid UUID string"))
    }
}

// Swift: UUID parsing failure
impl fmt::Display for PresetIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid UUID string")
    }
}

impl Error for PresetIdError {}

// Swift: DisplayIdentity
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct DisplayIdentity {
    pub uuid: String,
    pub vendor: u32,
    pub model: u32,
    pub serial: u32,
    pub name: String,
    pub kind: DisplayKind,
}

impl DisplayIdentity {
    // Swift: DisplayIdentity.representsSameDisplay(as:)
    pub fn represents_same_display(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        if !self.uuid.is_empty()
            && !other.uuid.is_empty()
            && self.uuid.eq_ignore_ascii_case(&other.uuid)
        {
            return true;
        }
        if self.serial != 0
            && other.serial != 0
            && self.vendor == other.vendor
            && self.model == other.model
            && self.serial == other.serial
        {
            return true;
        }
        self.kind == DisplayKind::Virtual
            && other.kind == DisplayKind::Virtual
            && self.vendor == other.vendor
            && self.model == other.model
            && self.name == other.name
    }
}

// Swift: PresetMonitor
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetMonitor {
    pub identity: DisplayIdentity,
    pub auto_match: bool,
}

// Swift: PresetScreen
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PresetScreen {
    pub monitors: Vec<PresetMonitor>,
    pub cell: GridCell,
}

// Swift: DisplayPreset
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayPreset {
    pub id: PresetId,
    pub name: String,
    pub main: PresetScreen,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary1: Option<PresetScreen>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary2: Option<PresetScreen>,
}

impl DisplayPreset {
    // Swift: DisplayPreset.screens
    pub fn screens(&self) -> Vec<&PresetScreen> {
        let mut screens = vec![&self.main];
        if let Some(screen) = self.secondary1.as_ref() {
            screens.push(screen);
        }
        if let Some(screen) = self.secondary2.as_ref() {
            screens.push(screen);
        }
        screens
    }

    // Swift: DisplayPreset.monitorCount
    pub fn monitor_count(&self) -> usize {
        self.screens()
            .into_iter()
            .map(|screen| screen.monitors.len())
            .sum()
    }

    // Swift: DisplayPreset.occupiedCells
    pub fn occupied_cells(&self) -> Vec<GridCell> {
        self.screens()
            .into_iter()
            .map(|screen| screen.cell)
            .collect()
    }
}

// Swift: DisplaySettings
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettings {
    pub auto_switch_enabled: bool,
    pub presets: Vec<DisplayPreset>,
}

// Swift: DisplaySettings.default
impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            auto_switch_enabled: false,
            presets: Vec::new(),
        }
    }
}

// Swift: DisplayPresetValidationError
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisplayPresetValidationError {
    TooManyPresets,
    DuplicatePresetId,
    InvalidName,
    EmptyMainScreen,
    DuplicateMonitor(String),
    SecondaryTwoRequiresSecondaryOne,
    MainScreenMustUseCenter,
    InvalidGridCell,
    DuplicateGridCell,
    DisconnectedGrid,
}

// Swift: DisplayPresetValidationError.errorDescription
impl fmt::Display for DisplayPresetValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyPresets => formatter.write_str("too many display presets"),
            Self::DuplicatePresetId => formatter.write_str("duplicate display preset id"),
            Self::InvalidName => formatter.write_str("invalid display preset name"),
            Self::EmptyMainScreen => formatter.write_str("main screen has no monitor"),
            Self::DuplicateMonitor(name) => write!(formatter, "duplicate monitor: {name}"),
            Self::SecondaryTwoRequiresSecondaryOne => {
                formatter.write_str("secondary2 requires secondary1")
            }
            Self::MainScreenMustUseCenter => formatter.write_str("main screen is not at (0,0)"),
            Self::InvalidGridCell => formatter.write_str("display grid cell is out of bounds"),
            Self::DuplicateGridCell => formatter.write_str("duplicate display grid cell"),
            Self::DisconnectedGrid => formatter.write_str("display grid is disconnected"),
        }
    }
}

impl Error for DisplayPresetValidationError {}

// Swift: DisplayGrid
pub struct DisplayGrid;

impl DisplayGrid {
    // Swift: DisplayGrid.validationError(for:)
    pub fn validation_error(preset: &DisplayPreset) -> Option<DisplayPresetValidationError> {
        let cells = preset.occupied_cells();
        if preset.main.cell != GridCell::center() {
            return Some(DisplayPresetValidationError::MainScreenMustUseCenter);
        }
        if !cells.iter().all(|cell| cell.is_in_bounds()) {
            return Some(DisplayPresetValidationError::InvalidGridCell);
        }
        if cells.iter().copied().collect::<HashSet<_>>().len() != cells.len() {
            return Some(DisplayPresetValidationError::DuplicateGridCell);
        }
        if !Self::is_connected(&cells) {
            return Some(DisplayPresetValidationError::DisconnectedGrid);
        }
        None
    }

    // Swift: DisplayGrid.isConnected(_:)
    pub fn is_connected(cells: &[GridCell]) -> bool {
        let occupied: HashSet<GridCell> = cells.iter().copied().collect();
        if !occupied.contains(&GridCell::center()) {
            return false;
        }
        let mut reached = HashSet::from([GridCell::center()]);
        let mut pending = vec![GridCell::center()];
        while let Some(cell) = pending.pop() {
            for candidate in occupied.iter().copied() {
                if !reached.contains(&candidate) && cell.is_orthogonally_adjacent(candidate) {
                    reached.insert(candidate);
                    pending.push(candidate);
                }
            }
        }
        reached == occupied
    }
}

// Swift: DisplayPresetValidator
pub struct DisplayPresetValidator;

impl DisplayPresetValidator {
    // Swift: DisplayPresetValidator.validate(_:)
    pub fn validate(settings: &DisplaySettings) -> Result<(), DisplayPresetValidationError> {
        if settings.presets.len() > DisplayPresetDefaults::MAXIMUM_PRESET_COUNT {
            return Err(DisplayPresetValidationError::TooManyPresets);
        }
        let ids: HashSet<&PresetId> = settings.presets.iter().map(|preset| &preset.id).collect();
        if ids.len() != settings.presets.len() {
            return Err(DisplayPresetValidationError::DuplicatePresetId);
        }

        for preset in &settings.presets {
            // Swift String.count counts grapheme clusters; chars() counts Unicode scalars.
            if preset.name.trim().is_empty()
                || preset.name.chars().count() > DisplayPresetDefaults::MAXIMUM_NAME_LENGTH
            {
                return Err(DisplayPresetValidationError::InvalidName);
            }
            if preset.main.monitors.is_empty() {
                return Err(DisplayPresetValidationError::EmptyMainScreen);
            }
            if preset.secondary2.is_some() && preset.secondary1.is_none() {
                return Err(DisplayPresetValidationError::SecondaryTwoRequiresSecondaryOne);
            }
            if let Some(error) = DisplayGrid::validation_error(preset) {
                return Err(error);
            }

            let identities: Vec<&DisplayIdentity> = preset
                .screens()
                .into_iter()
                .flat_map(|screen| screen.monitors.iter().map(|monitor| &monitor.identity))
                .collect();
            for left in 0..identities.len() {
                for right in (left + 1)..identities.len() {
                    if identities[left].represents_same_display(identities[right]) {
                        return Err(DisplayPresetValidationError::DuplicateMonitor(
                            identities[right].name.clone(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
