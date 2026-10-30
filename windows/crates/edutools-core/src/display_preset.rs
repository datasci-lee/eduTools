use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

// Swift: DisplayPresetDefaults
pub struct DisplayPresetDefaults;

impl DisplayPresetDefaults {
    pub const MAXIMUM_PRESET_COUNT: usize = 8;
    pub const MAXIMUM_NAME_LENGTH: usize = 40;
    pub const CONNECTION_STABILITY_INTERVAL: f64 = 5.0;
    pub const AUTOMATIC_APPLY_COOLDOWN: f64 = 30.0;
    pub const MANUAL_CONFIRMATION_INTERVAL: f64 = 15.0;
}

// Swift: DisplayPresetsSettingsView.limitedName(_:)
pub fn limited_name(name: &str) -> String {
    name.chars()
        .take(DisplayPresetDefaults::MAXIMUM_NAME_LENGTH)
        .collect()
}

// Swift: DisplayPresetsSettingsView.isValidName(_:)
pub fn is_valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= DisplayPresetDefaults::MAXIMUM_NAME_LENGTH
}

// Swift: DisplayPresetsSettingsView.uniqueName(startingWith:)
pub fn unique_name<'a>(
    requested_name: &str,
    existing: impl IntoIterator<Item = &'a str>,
) -> String {
    let existing = existing.into_iter().collect::<HashSet<_>>();
    let initial = limited_name(requested_name);
    if !existing.contains(initial.as_str()) {
        return initial;
    }
    for suffix in 2_usize.. {
        let suffix_text = format!(" {suffix}");
        let base_limit = DisplayPresetDefaults::MAXIMUM_NAME_LENGTH
            .saturating_sub(suffix_text.chars().count())
            .max(1);
        let base = requested_name.chars().take(base_limit).collect::<String>();
        let candidate = format!("{base}{suffix_text}");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("the numeric suffix space is unbounded")
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
    // Swift: DisplayIdentity.matchLevel(with:)
    fn match_level(&self, other: &Self) -> Option<DisplayMatchLevel> {
        if !self.uuid.is_empty()
            && !other.uuid.is_empty()
            && self.uuid.eq_ignore_ascii_case(&other.uuid)
        {
            return Some(DisplayMatchLevel::Uuid);
        }
        if self.serial != 0
            && other.serial != 0
            && self.vendor == other.vendor
            && self.model == other.model
            && self.serial == other.serial
        {
            return Some(DisplayMatchLevel::HardwareSerial);
        }
        if self.kind == DisplayKind::Virtual
            && other.kind == DisplayKind::Virtual
            && self.vendor == other.vendor
            && self.model == other.model
            && self.name == other.name
        {
            return Some(DisplayMatchLevel::VirtualName);
        }
        None
    }

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

    // Swift: DisplayPreset.autoMatchSet
    pub fn auto_match_set(&self) -> HashSet<DisplayIdentity> {
        self.screens()
            .into_iter()
            .flat_map(|screen| screen.monitors.iter())
            .filter(|monitor| monitor.auto_match)
            .map(|monitor| monitor.identity.clone())
            .collect()
    }

    // Swift: DisplayPreset.matchesAutoSet(online:)
    pub fn matches_auto_set(&self, online: &[OnlineDisplay]) -> bool {
        let monitors: Vec<PresetMonitor> = self
            .screens()
            .into_iter()
            .flat_map(|screen| screen.monitors.iter())
            .filter(|monitor| monitor.auto_match)
            .cloned()
            .collect();
        !monitors.is_empty()
            && monitors.len() == online.len()
            && DisplayMatcher::matches(&monitors, online).len() == monitors.len()
    }

    // Swift: DisplayPreset.matchesCurrent(online:)
    pub fn matches_current(&self, online: &[OnlineDisplay]) -> bool {
        DisplayPlanner::matches_current(self, online)
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

// Swift: OnlineDisplay
#[derive(Clone, Debug, PartialEq)]
pub struct OnlineDisplay {
    pub id: u32,
    pub uuid: String,
    pub vendor: u32,
    pub model: u32,
    pub serial: u32,
    pub name: String,
    pub kind: DisplayKind,
    pub size: DisplaySize,
    pub origin: DisplayPoint,
    pub current_mirror_master_id: Option<u32>,
}

impl OnlineDisplay {
    // Swift: OnlineDisplay.identity
    pub fn identity(&self) -> DisplayIdentity {
        DisplayIdentity {
            uuid: self.uuid.clone(),
            vendor: self.vendor,
            model: self.model,
            serial: self.serial,
            name: self.name.clone(),
            kind: self.kind,
        }
    }
}

// Swift: DisplayMatchLevel
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayMatchLevel {
    Uuid,
    HardwareSerial,
    VirtualName,
}

impl DisplayMatchLevel {
    // Swift: DisplayMatchLevel.allCases
    const fn all() -> [Self; 3] {
        [Self::Uuid, Self::HardwareSerial, Self::VirtualName]
    }
}

// Swift: DisplayMatch
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayMatch {
    pub preset_index: usize,
    pub online_index: usize,
}

// Swift: DisplayMatcher
pub struct DisplayMatcher;

impl DisplayMatcher {
    // Swift: DisplayMatcher.matches(monitors:online:)
    pub fn matches(monitors: &[PresetMonitor], online: &[OnlineDisplay]) -> Vec<DisplayMatch> {
        let mut used_preset = HashSet::new();
        let mut used_online = HashSet::new();
        let mut result = Vec::new();

        for level in DisplayMatchLevel::all() {
            for preset_index in 0..monitors.len() {
                if used_preset.contains(&preset_index) {
                    continue;
                }
                let online_index = (0..online.len()).find(|online_index| {
                    !used_online.contains(online_index)
                        && monitors[preset_index]
                            .identity
                            .match_level(&online[*online_index].identity())
                            == Some(level)
                });
                if let Some(online_index) = online_index {
                    used_preset.insert(preset_index);
                    used_online.insert(online_index);
                    result.push(DisplayMatch {
                        preset_index,
                        online_index,
                    });
                }
            }
        }
        result.sort_by_key(|matched| matched.preset_index);
        result
    }
}

// Swift: DisplayPoint
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayPoint {
    pub x: f64,
    pub y: f64,
}

impl DisplayPoint {
    // Swift: DisplayPoint.zero
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

// Swift: DisplaySize
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplaySize {
    pub width: f64,
    pub height: f64,
}

// Swift: DisplayScreenRole
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayScreenRole {
    Main,
    Secondary1,
    Secondary2,
}

impl DisplayScreenRole {
    // Swift: DisplayScreenRole.displayName
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Main => "주 화면",
            Self::Secondary1 => "보조화면1",
            Self::Secondary2 => "보조화면2",
        }
    }
}

// Swift: DisplayMirrorPlan
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayMirrorPlan {
    pub slave_id: u32,
    pub master_id: u32,
}

// Swift: DisplayPlannedScreen
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayPlannedScreen {
    pub role: DisplayScreenRole,
    pub master_id: u32,
    pub master_name: String,
    pub mirrored_display_ids: Vec<u32>,
    pub mirrored_names: Vec<String>,
    pub cell: GridCell,
    pub origin: DisplayPoint,
    pub size: DisplaySize,
}

// Swift: DisplayOutsidePlacement
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayOutsidePlacement {
    pub display_id: u32,
    pub name: String,
    pub origin: DisplayPoint,
}

// Swift: DisplayPlan
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayPlan {
    pub main_display_id: Option<u32>,
    pub origins_by_master: HashMap<u32, DisplayPoint>,
    pub mirrors: Vec<DisplayMirrorPlan>,
    pub unmirror_display_ids: Vec<u32>,
    pub outside_placements: Vec<DisplayOutsidePlacement>,
    pub missing_preset_monitors: Vec<DisplayIdentity>,
    pub screens: Vec<DisplayPlannedScreen>,
    pub layout_issues: Vec<String>,
}

impl DisplayPlan {
    // Swift: DisplayPlan.isApplicable
    pub fn is_applicable(&self) -> bool {
        self.main_display_id.is_some() && self.layout_issues.is_empty()
    }

    // Swift: DisplayPlan.summary()
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.layout_issues.is_empty() {
            parts.push(format!("적용 불가: {}", self.layout_issues.join(" · ")));
        }
        if let Some(anchor) = self.screens.first() {
            if anchor.role != DisplayScreenRole::Main {
                parts.push(format!(
                    "주 화면 모니터 없음 → {} 이 주 디스플레이",
                    anchor.role.display_name()
                ));
            }
        }
        parts.extend(self.screens.iter().map(|screen| {
            let mut value = format!("{} = {}", screen.role.display_name(), screen.master_name);
            if !screen.mirrored_names.is_empty() {
                value.push_str(&format!("(미러: {})", screen.mirrored_names.join(", ")));
            }
            value.push_str(&format!(
                " ({},{}), 원점 {}",
                screen.cell.x,
                screen.cell.y,
                Self::point_description(screen.origin)
            ));
            value
        }));
        if self.screens.len() > 1 {
            let mut ordered = self.screens.iter().collect::<Vec<_>>();
            ordered.sort_by_key(|screen| (screen.cell.y, screen.cell.x));
            parts.push(format!(
                "격자 순서: {}",
                ordered
                    .iter()
                    .map(|screen| format!(
                        "{}({},{})",
                        screen.role.display_name(),
                        screen.cell.x,
                        screen.cell.y
                    ))
                    .collect::<Vec<_>>()
                    .join(" – ")
            ));
        }
        if !self.outside_placements.is_empty() {
            parts.push(format!(
                "프리셋 밖 = {} (배치 묶음 오른쪽에 확장)",
                self.outside_placements
                    .iter()
                    .map(|placement| placement.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.missing_preset_monitors.is_empty() {
            parts.push(format!(
                "연결 안 됨: {}",
                self.missing_preset_monitors
                    .iter()
                    .map(|identity| identity.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.unmirror_display_ids.is_empty() {
            parts.push(format!(
                "미러 해제: {}",
                self.unmirror_display_ids
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if parts.is_empty() {
            "계산할 수 있는 연결된 모니터가 없습니다.".to_owned()
        } else {
            parts.join(" · ")
        }
    }

    // Swift: DisplayPlan.pointDescription(_:)
    fn point_description(point: DisplayPoint) -> String {
        format!(
            "({},{})",
            Self::number_description(point.x),
            Self::number_description(point.y)
        )
    }

    // Swift: DisplayPlan.numberDescription(_:)
    fn number_description(value: f64) -> String {
        if value.round() == value {
            format!("{value:.0}")
        } else {
            format!("{value:.2}")
        }
    }
}

// Swift: DisplayGridMetric
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayGridMetric {
    pub cell: GridCell,
    pub size: DisplaySize,
}

// Swift: DisplayGridLayout
pub struct DisplayGridLayout;

impl DisplayGridLayout {
    // Swift: DisplayGridLayout.origins(for:)
    pub fn origins(metrics: &[DisplayGridMetric]) -> HashMap<GridCell, DisplayPoint> {
        let Some(anchor) = metrics.first().copied() else {
            return HashMap::new();
        };
        assert_eq!(
            metrics
                .iter()
                .map(|metric| metric.cell)
                .collect::<HashSet<_>>()
                .len(),
            metrics.len(),
            "Display layout requires unique grid cells"
        );

        let mut result = HashMap::from([(anchor.cell, DisplayPoint::zero())]);
        let mut placed_indices = vec![0_usize];
        let mut placed = HashSet::from([0_usize]);
        let mut pending = vec![0_usize];
        let mut pending_index = 0;

        while placed.len() < metrics.len() {
            while pending_index < pending.len() {
                let parent_index = pending[pending_index];
                pending_index += 1;
                let parent = metrics[parent_index];
                let parent_origin = *result
                    .get(&parent.cell)
                    .expect("Placed display is missing its origin");
                for candidate_index in 0..metrics.len() {
                    if placed.contains(&candidate_index) {
                        continue;
                    }
                    let candidate = metrics[candidate_index];
                    if !candidate.cell.is_orthogonally_adjacent(parent.cell) {
                        continue;
                    }
                    result.insert(
                        candidate.cell,
                        Self::adjacent_origin(candidate, parent, parent_origin),
                    );
                    placed.insert(candidate_index);
                    placed_indices.push(candidate_index);
                    pending.push(candidate_index);
                }
            }

            let Some(detached_index) = (0..metrics.len()).find(|index| !placed.contains(index))
            else {
                continue;
            };
            let origin = Self::detached_origin(
                metrics[detached_index],
                anchor.cell,
                metrics,
                &placed_indices,
                &result,
            );
            result.insert(metrics[detached_index].cell, origin);
            placed.insert(detached_index);
            placed_indices.push(detached_index);
            pending.push(detached_index);
        }
        result
    }

    // Swift: DisplayGridLayout.adjacentOrigin(for:nextTo:parentOrigin:)
    fn adjacent_origin(
        metric: DisplayGridMetric,
        parent: DisplayGridMetric,
        parent_origin: DisplayPoint,
    ) -> DisplayPoint {
        match (metric.cell.x - parent.cell.x, metric.cell.y - parent.cell.y) {
            (1, 0) => DisplayPoint {
                x: parent_origin.x + parent.size.width,
                y: parent_origin.y,
            },
            (-1, 0) => DisplayPoint {
                x: parent_origin.x - metric.size.width,
                y: parent_origin.y,
            },
            (0, 1) => DisplayPoint {
                x: parent_origin.x,
                y: parent_origin.y + parent.size.height,
            },
            (0, -1) => DisplayPoint {
                x: parent_origin.x,
                y: parent_origin.y - metric.size.height,
            },
            _ => panic!("Adjacent display cells must share an edge"),
        }
    }

    // Swift: DisplayGridLayout.detachedOrigin(for:anchorCell:metrics:placedIndices:origins:)
    fn detached_origin(
        metric: DisplayGridMetric,
        anchor_cell: GridCell,
        metrics: &[DisplayGridMetric],
        placed_indices: &[usize],
        origins: &HashMap<GridCell, DisplayPoint>,
    ) -> DisplayPoint {
        let delta_x = metric.cell.x - anchor_cell.x;
        let delta_y = metric.cell.y - anchor_cell.y;
        let is_horizontal = delta_x != 0 && delta_x.abs() >= delta_y.abs();

        if is_horizontal {
            if delta_x > 0 {
                let edge = placed_indices
                    .iter()
                    .map(|index| {
                        Self::origin(metrics[*index], origins).x + metrics[*index].size.width
                    })
                    .fold(f64::NEG_INFINITY, f64::max);
                let support = placed_indices
                    .iter()
                    .copied()
                    .find(|index| {
                        Self::origin(metrics[*index], origins).x + metrics[*index].size.width
                            == edge
                    })
                    .unwrap_or(placed_indices[0]);
                return DisplayPoint {
                    x: edge,
                    y: Self::origin(metrics[support], origins).y,
                };
            }
            let edge = placed_indices
                .iter()
                .map(|index| Self::origin(metrics[*index], origins).x)
                .fold(f64::INFINITY, f64::min);
            let support = placed_indices
                .iter()
                .copied()
                .find(|index| Self::origin(metrics[*index], origins).x == edge)
                .unwrap_or(placed_indices[0]);
            return DisplayPoint {
                x: edge - metric.size.width,
                y: Self::origin(metrics[support], origins).y,
            };
        }

        if delta_y > 0 {
            let edge = placed_indices
                .iter()
                .map(|index| Self::origin(metrics[*index], origins).y + metrics[*index].size.height)
                .fold(f64::NEG_INFINITY, f64::max);
            let support = placed_indices
                .iter()
                .copied()
                .find(|index| {
                    Self::origin(metrics[*index], origins).y + metrics[*index].size.height == edge
                })
                .unwrap_or(placed_indices[0]);
            return DisplayPoint {
                x: Self::origin(metrics[support], origins).x,
                y: edge,
            };
        }
        let edge = placed_indices
            .iter()
            .map(|index| Self::origin(metrics[*index], origins).y)
            .fold(f64::INFINITY, f64::min);
        let support = placed_indices
            .iter()
            .copied()
            .find(|index| Self::origin(metrics[*index], origins).y == edge)
            .unwrap_or(placed_indices[0]);
        DisplayPoint {
            x: Self::origin(metrics[support], origins).x,
            y: edge - metric.size.height,
        }
    }

    // Swift: DisplayGridLayout.origin(for:in:)
    fn origin(
        metric: DisplayGridMetric,
        origins: &HashMap<GridCell, DisplayPoint>,
    ) -> DisplayPoint {
        *origins
            .get(&metric.cell)
            .expect("Placed display is missing its origin")
    }
}

// Swift: DisplayPlanner.MonitorSlot
#[derive(Clone)]
struct MonitorSlot {
    role: DisplayScreenRole,
    monitor: PresetMonitor,
    global_index: usize,
}

// Swift: DisplayPlanner.ResolvedScreen
#[derive(Clone)]
struct ResolvedScreen {
    role: DisplayScreenRole,
    screen: PresetScreen,
    master: OnlineDisplay,
    slaves: Vec<OnlineDisplay>,
}

// Swift: DisplayPlanner
pub struct DisplayPlanner;

impl DisplayPlanner {
    // Swift: DisplayPlanner.plan(preset:online:)
    pub fn plan(preset: &DisplayPreset, online: &[OnlineDisplay]) -> DisplayPlan {
        let mut role_screens = vec![(DisplayScreenRole::Main, preset.main.clone())];
        if let Some(secondary1) = preset.secondary1.clone() {
            role_screens.push((DisplayScreenRole::Secondary1, secondary1));
        }
        if let Some(secondary2) = preset.secondary2.clone() {
            role_screens.push((DisplayScreenRole::Secondary2, secondary2));
        }

        let mut slots = Vec::new();
        for (role, screen) in &role_screens {
            for monitor in &screen.monitors {
                slots.push(MonitorSlot {
                    role: *role,
                    monitor: monitor.clone(),
                    global_index: slots.len(),
                });
            }
        }
        let matches = DisplayMatcher::matches(
            &slots
                .iter()
                .map(|slot| slot.monitor.clone())
                .collect::<Vec<_>>(),
            online,
        );
        let online_index_by_slot: HashMap<usize, usize> = matches
            .iter()
            .map(|matched| (matched.preset_index, matched.online_index))
            .collect();
        let matched_online_indices: HashSet<usize> =
            matches.iter().map(|matched| matched.online_index).collect();

        let resolved_screens = role_screens
            .iter()
            .filter_map(|(role, screen)| {
                let connected = slots
                    .iter()
                    .filter(|slot| slot.role == *role)
                    .filter_map(|slot| {
                        online_index_by_slot
                            .get(&slot.global_index)
                            .map(|index| online[*index].clone())
                    })
                    .collect::<Vec<_>>();
                let master = connected.first()?.clone();
                Some(ResolvedScreen {
                    role: *role,
                    screen: screen.clone(),
                    master,
                    slaves: connected.into_iter().skip(1).collect(),
                })
            })
            .collect::<Vec<_>>();
        let grid_origins = DisplayGridLayout::origins(
            &resolved_screens
                .iter()
                .map(|resolved| DisplayGridMetric {
                    cell: resolved.screen.cell,
                    size: resolved.master.size,
                })
                .collect::<Vec<_>>(),
        );
        let mut origins = HashMap::new();
        let mut mirrors = Vec::new();
        let mut planned_screens = Vec::new();
        for resolved in resolved_screens {
            let origin = *grid_origins
                .get(&resolved.screen.cell)
                .expect("Grid origin missing for a resolved preset screen");
            origins.insert(resolved.master.id, origin);
            mirrors.extend(resolved.slaves.iter().map(|slave| DisplayMirrorPlan {
                slave_id: slave.id,
                master_id: resolved.master.id,
            }));
            planned_screens.push(DisplayPlannedScreen {
                role: resolved.role,
                master_id: resolved.master.id,
                master_name: resolved.master.name.clone(),
                mirrored_display_ids: resolved.slaves.iter().map(|slave| slave.id).collect(),
                mirrored_names: resolved
                    .slaves
                    .iter()
                    .map(|slave| slave.name.clone())
                    .collect(),
                cell: resolved.screen.cell,
                origin,
                size: resolved.master.size,
            });
        }

        let mut outside = (0..online.len())
            .filter(|index| !matched_online_indices.contains(index))
            .collect::<Vec<_>>();
        if planned_screens.is_empty() {
            outside.sort_by_key(|index| {
                let display = &online[*index];
                let is_current_main =
                    Self::is_zero(display.origin) && display.current_mirror_master_id.is_none();
                (!is_current_main, *index)
            });
        }
        let mut outside_placements = Vec::new();
        let mut outside_x = planned_screens
            .iter()
            .map(|screen| screen.origin.x + screen.size.width)
            .fold(None, |current: Option<f64>, edge| {
                Some(current.map_or(edge, |value| value.max(edge)))
            })
            .unwrap_or(0.0);
        let outside_y = planned_screens
            .iter()
            .find(|screen| screen.origin.x + screen.size.width == outside_x)
            .map(|screen| screen.origin.y)
            .unwrap_or(0.0);
        for online_index in outside {
            let display = &online[online_index];
            let origin = DisplayPoint {
                x: outside_x,
                y: outside_y,
            };
            origins.insert(display.id, origin);
            outside_placements.push(DisplayOutsidePlacement {
                display_id: display.id,
                name: display.name.clone(),
                origin,
            });
            outside_x += display.size.width;
        }

        let desired_mirror_master: HashMap<u32, u32> = mirrors
            .iter()
            .map(|mirror| (mirror.slave_id, mirror.master_id))
            .collect();
        let unmirror_display_ids = online
            .iter()
            .filter_map(|display| {
                let current_master = display.current_mirror_master_id?;
                (desired_mirror_master.get(&display.id).copied() != Some(current_master))
                    .then_some(display.id)
            })
            .collect();
        let missing_preset_monitors = slots
            .iter()
            .filter(|slot| !online_index_by_slot.contains_key(&slot.global_index))
            .map(|slot| slot.monitor.identity.clone())
            .collect();
        let layout_issues = Self::overlapping_layout_issues(&planned_screens);
        DisplayPlan {
            main_display_id: planned_screens
                .first()
                .map(|screen| screen.master_id)
                .or_else(|| {
                    outside_placements
                        .first()
                        .map(|placement| placement.display_id)
                }),
            origins_by_master: origins,
            mirrors,
            unmirror_display_ids,
            outside_placements,
            missing_preset_monitors,
            screens: planned_screens,
            layout_issues,
        }
    }

    // Swift: DisplayPlanner.presetForConnectedSet(connected:presets:)
    pub fn preset_for_connected_set<'a>(
        connected: &[OnlineDisplay],
        presets: &'a [DisplayPreset],
    ) -> Option<&'a DisplayPreset> {
        presets
            .iter()
            .find(|preset| preset.matches_auto_set(connected))
    }

    // Swift: DisplayPlanner.matchesCurrent(preset:online:)
    pub fn matches_current(preset: &DisplayPreset, online: &[OnlineDisplay]) -> bool {
        let plan = Self::plan(preset, online);
        let current_main = online
            .iter()
            .find(|display| {
                display.current_mirror_master_id.is_none() && Self::is_zero(display.origin)
            })
            .map(|display| display.id);
        if current_main != plan.main_display_id {
            return false;
        }
        let desired_mirror_master: HashMap<u32, u32> = plan
            .mirrors
            .iter()
            .map(|mirror| (mirror.slave_id, mirror.master_id))
            .collect();
        for display in online {
            if display.current_mirror_master_id != desired_mirror_master.get(&display.id).copied() {
                return false;
            }
            if !desired_mirror_master.contains_key(&display.id) {
                if let Some(origin) = plan.origins_by_master.get(&display.id) {
                    if !Self::approximately_equal(*origin, display.origin) {
                        return false;
                    }
                }
            }
        }
        true
    }

    // Swift: DisplayPlanner.overlappingLayoutIssues(in:)
    fn overlapping_layout_issues(screens: &[DisplayPlannedScreen]) -> Vec<String> {
        let mut issues = Vec::new();
        for left_index in 0..screens.len() {
            for right_index in (left_index + 1)..screens.len() {
                let left = &screens[left_index];
                let right = &screens[right_index];
                if Self::rectangles_overlap(left, right) {
                    issues.push(format!(
                        "화면이 겹칩니다({} ↔ {})",
                        left.role.display_name(),
                        right.role.display_name()
                    ));
                }
            }
        }
        issues
    }

    // Swift: DisplayPlanner.rectanglesOverlap(_:_:)
    fn rectangles_overlap(left: &DisplayPlannedScreen, right: &DisplayPlannedScreen) -> bool {
        left.origin.x < right.origin.x + right.size.width
            && right.origin.x < left.origin.x + left.size.width
            && left.origin.y < right.origin.y + right.size.height
            && right.origin.y < left.origin.y + left.size.height
    }

    // Swift: DisplayPlanner.approximatelyEqual(_:_:)
    fn approximately_equal(left: DisplayPoint, right: DisplayPoint) -> bool {
        (left.x - right.x).abs() < 0.5 && (left.y - right.y).abs() < 0.5
    }

    // Swift: DisplayPlanner.isZero(_:)
    fn is_zero(point: DisplayPoint) -> bool {
        Self::approximately_equal(point, DisplayPoint::zero())
    }
}

// Swift: DisplayPresetCaptureResult
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayPresetCaptureResult {
    pub preset: DisplayPreset,
    pub warnings: Vec<String>,
}

// Swift: DisplayPresetCaptureError
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayPresetCaptureError {
    NoOnlineDisplays,
}

// Swift: DisplayPresetCaptureError.errorDescription
impl fmt::Display for DisplayPresetCaptureError {
    // Swift: DisplayPresetCaptureError.errorDescription
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOnlineDisplays => {
                formatter.write_str("연결된 모니터가 없어 현재 구성을 캡처할 수 없습니다.")
            }
        }
    }
}

impl Error for DisplayPresetCaptureError {}

// Swift: DisplayPresetCapture.MirrorGroup
#[derive(Clone)]
struct MirrorGroup {
    key: u32,
    first_online_index: usize,
    displays: Vec<OnlineDisplay>,
}

impl MirrorGroup {
    // Swift: DisplayPresetCapture.MirrorGroup.master
    fn master(&self) -> &OnlineDisplay {
        self.displays
            .iter()
            .find(|display| display.id == self.key)
            .unwrap_or(&self.displays[0])
    }
}

// Swift: DisplayPresetCapture.CellAssignment
struct CellAssignment {
    cells: Vec<GridCell>,
    score: f64,
    is_exact: bool,
}

// Swift: DisplayPresetCapture
pub struct DisplayPresetCapture;

impl DisplayPresetCapture {
    // Swift: DisplayPresetCapture.current(online:name:), with UUID generation injected
    pub fn current(
        online: &[OnlineDisplay],
        name: &str,
        preset_id: PresetId,
    ) -> Result<DisplayPresetCaptureResult, DisplayPresetCaptureError> {
        if online.is_empty() {
            return Err(DisplayPresetCaptureError::NoOnlineDisplays);
        }

        let mut grouped: Vec<(u32, Vec<(usize, OnlineDisplay)>)> = Vec::new();
        for (index, display) in online.iter().cloned().enumerate() {
            let key = display.current_mirror_master_id.unwrap_or(display.id);
            if let Some((_, values)) = grouped.iter_mut().find(|(group_key, _)| *group_key == key) {
                values.push((index, display));
            } else {
                grouped.push((key, vec![(index, display)]));
            }
        }
        let mut groups = grouped
            .into_iter()
            .map(|(key, mut indexed_displays)| {
                let first_online_index = indexed_displays
                    .iter()
                    .map(|(index, _)| *index)
                    .min()
                    .unwrap_or(0);
                indexed_displays.sort_by_key(|(index, display)| (display.id != key, *index));
                MirrorGroup {
                    key,
                    first_online_index,
                    displays: indexed_displays
                        .into_iter()
                        .map(|(_, display)| display)
                        .collect(),
                }
            })
            .collect::<Vec<_>>();
        let mut warnings = Vec::new();
        let main_group_index = groups
            .iter()
            .position(|group| {
                group.displays.iter().any(|display| {
                    display.current_mirror_master_id.is_none()
                        && DisplayPlanner::is_zero(display.origin)
                })
            })
            .unwrap_or_else(|| {
                warnings.push(
                    "주 화면을 원점에서 찾지 못해 온라인 목록의 첫 화면을 주 화면으로 사용했습니다."
                        .to_owned(),
                );
                groups
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, group)| group.first_online_index)
                    .map(|(index, _)| index)
                    .unwrap_or(0)
            });
        let main_group = groups.remove(main_group_index);
        groups.sort_by(|left, right| {
            let left_distance =
                Self::squared_distance(left.master().origin, main_group.master().origin);
            let right_distance =
                Self::squared_distance(right.master().origin, main_group.master().origin);
            left_distance
                .total_cmp(&right_distance)
                .then(left.first_online_index.cmp(&right.first_online_index))
        });
        let mut ordered = vec![main_group];
        ordered.extend(groups);
        if ordered.len() > 3 {
            warnings.push(format!(
                "화면이 {}개여서 원점 거리순으로 가까운 3개 화면만 담았습니다.",
                ordered.len()
            ));
        }
        let captured = ordered.into_iter().take(3).collect::<Vec<_>>();
        let assignment = Self::closest_connected_cells(&captured);
        if !assignment.is_exact {
            let coordinates = assignment
                .cells
                .iter()
                .map(|cell| format!("({},{})", cell.x, cell.y))
                .collect::<Vec<_>>()
                .join(" · ");
            warnings.push(format!(
                "현재 원점을 연결된 3×3 격자의 가장 가까운 칸으로 근사했습니다: {coordinates}"
            ));
        }
        let screens = captured
            .iter()
            .enumerate()
            .map(|(index, group)| PresetScreen {
                monitors: group
                    .displays
                    .iter()
                    .map(|display| PresetMonitor {
                        identity: display.identity(),
                        auto_match: false,
                    })
                    .collect(),
                cell: assignment.cells[index],
            })
            .collect::<Vec<_>>();
        Ok(DisplayPresetCaptureResult {
            preset: DisplayPreset {
                id: preset_id,
                name: name.to_owned(),
                main: screens[0].clone(),
                secondary1: screens.get(1).cloned(),
                secondary2: screens.get(2).cloned(),
            },
            warnings,
        })
    }

    // Swift: DisplayPresetCapture.closestConnectedCells(for:)
    fn closest_connected_cells(groups: &[MirrorGroup]) -> CellAssignment {
        if groups.len() <= 1 {
            return CellAssignment {
                cells: vec![GridCell::center()],
                score: 0.0,
                is_exact: true,
            };
        }
        let mut best = None;
        Self::assign_cells(groups, Vec::new(), &mut best);
        best.expect("No connected grid assignment for at most three screens")
    }

    // Swift: DisplayPresetCapture.closestConnectedCells(for:).assign(_:)
    fn assign_cells(
        groups: &[MirrorGroup],
        cells: Vec<GridCell>,
        best: &mut Option<CellAssignment>,
    ) {
        if cells.len() == groups.len() - 1 {
            Self::consider_cells(groups, cells, best);
            return;
        }
        for candidate in GridCell::placement_order() {
            if !cells.contains(&candidate) {
                let mut next = cells.clone();
                next.push(candidate);
                Self::assign_cells(groups, next, best);
            }
        }
    }

    // Swift: DisplayPresetCapture.closestConnectedCells(for:).consider(_:)
    fn consider_cells(
        groups: &[MirrorGroup],
        secondary_cells: Vec<GridCell>,
        best: &mut Option<CellAssignment>,
    ) {
        let mut cells = vec![GridCell::center()];
        cells.extend(secondary_cells);
        if !DisplayGrid::is_connected(&cells) {
            return;
        }
        let main_origin = groups[0].master().origin;
        let actual_origins = groups
            .iter()
            .map(|group| DisplayPoint {
                x: group.master().origin.x - main_origin.x,
                y: group.master().origin.y - main_origin.y,
            })
            .collect::<Vec<_>>();
        let metrics = cells
            .iter()
            .copied()
            .zip(groups.iter())
            .map(|(cell, group)| DisplayGridMetric {
                cell,
                size: group.master().size,
            })
            .collect::<Vec<_>>();
        let origins = DisplayGridLayout::origins(&metrics);
        let mut score = 0.0;
        let mut is_exact = true;
        for index in 0..cells.len() {
            let planned = *origins
                .get(&cells[index])
                .expect("Grid origin missing while capturing a preset");
            let actual = actual_origins[index];
            score += Self::squared_distance(planned, actual);
            if (planned.x - actual.x).abs() >= 0.5 || (planned.y - actual.y).abs() >= 0.5 {
                is_exact = false;
            }
        }
        if best
            .as_ref()
            .map(|current| score < current.score)
            .unwrap_or(true)
        {
            *best = Some(CellAssignment {
                cells,
                score,
                is_exact,
            });
        }
    }

    // Swift: DisplayPresetCapture.squaredDistance(_:_:)
    fn squared_distance(left: DisplayPoint, right: DisplayPoint) -> f64 {
        let delta_x = left.x - right.x;
        let delta_y = left.y - right.y;
        delta_x * delta_x + delta_y * delta_y
    }
}

// Swift: DisplayAutoSwitchDecision
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DisplayAutoSwitchDecision {
    Ignore,
    Apply(usize),
    Retry(f64),
}

// Swift: DisplayAutoSwitchPolicy
pub struct DisplayAutoSwitchPolicy;

impl DisplayAutoSwitchPolicy {
    // Swift: DisplayAutoSwitchPolicy.decide(online:presets:enabled:lastJudgedUUIDs:lastAutoApplyAt:now:cooldown:)
    pub fn decide(
        online: &[OnlineDisplay],
        presets: &[DisplayPreset],
        enabled: bool,
        last_judged_uuids: Option<&HashSet<String>>,
        last_auto_apply_at: Option<f64>,
        now: f64,
        cooldown: f64,
    ) -> DisplayAutoSwitchDecision {
        let online_uuids = online
            .iter()
            .map(|display| display.uuid.clone())
            .collect::<HashSet<_>>();
        if last_judged_uuids == Some(&online_uuids) || !enabled {
            return DisplayAutoSwitchDecision::Ignore;
        }
        if let Some(last_auto_apply_at) = last_auto_apply_at {
            let elapsed = now - last_auto_apply_at;
            if elapsed < cooldown {
                return DisplayAutoSwitchDecision::Retry(cooldown - elapsed);
            }
        }
        let Some(index) = presets
            .iter()
            .position(|preset| preset.matches_auto_set(online))
        else {
            return DisplayAutoSwitchDecision::Ignore;
        };
        if presets[index].matches_current(online) {
            DisplayAutoSwitchDecision::Ignore
        } else {
            DisplayAutoSwitchDecision::Apply(index)
        }
    }
}
