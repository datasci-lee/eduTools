//! 원형 팔레트의 OS 독립 카탈로그, 기하, 히트 판정, 탐색, 렌더 상태.

use crate::eraser::{EraserAreaShape, EraserMode};
use crate::ink::{InkTool, Point, Rect};
use crate::settings::{
    AppSettings, FavoriteTool, FavoritesDefaults, PaletteStyle, PerToolMemorySettings,
    RadialPaletteGlassStyle, SettingsColor,
};
use crate::shortcut::{
    ShortcutBoard, ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutTool,
};
use std::f64::consts::PI;

/// Swift: NSColor를 OS 독립 값으로 옮긴 표현
pub type RadialColor = [f64; 4];

// Swift: RadialPaletteMetrics
pub const COLLAPSED_DIAMETER: f64 = 44.0;
pub const EXPANDED_DIAMETER: f64 = 302.0;
pub const CENTER_RADIUS: f64 = COLLAPSED_DIAMETER / 2.0;
pub const DRAG_THRESHOLD: f64 = 4.0;
pub const GROUP_INNER_RADIUS: f64 = 28.0;
pub const GROUP_OUTER_RADIUS: f64 = 58.0;
pub const ENTRY_INNER_RADIUS: f64 = 64.0;
pub const ENTRY_OUTER_RADIUS: f64 = 100.0;
pub const OPTION_INNER_RADIUS: f64 = 113.0;
pub const OPTION_OUTER_RADIUS: f64 = 149.0;
pub const SECTOR_GAP: f64 = 2.0 * PI / 180.0;
pub const PROPERTY_GAP: f64 = 9.0 * PI / 180.0;
pub const OPTION_MAXIMUM_SLICE: f64 = PI / 6.0;
pub const WRITING_WIDTHS: [f64; 5] = [1.0, 3.0, 5.0, 8.0, 12.0];
pub const HIGHLIGHTER_WIDTHS: [f64; 5] = [6.0, 12.0, 18.0, 24.0, 30.0];
pub const TEXT_SIZES: [f64; 5] = [16.0, 24.0, 36.0, 48.0, 72.0];

/// Swift: RadialPaletteArcStyle
pub struct RadialPaletteArcStyle;

impl RadialPaletteArcStyle {
    // Swift: RadialPaletteArcStyle 색·알파 상수
    pub const SURFACE_RGB: [f64; 3] = [20.0 / 255.0, 23.0 / 255.0, 31.0 / 255.0];
    pub const OUTER_BORDER: RadialColor = [1.0, 1.0, 1.0, 0.08];
    pub const HOVER_FILL: RadialColor = [1.0, 1.0, 1.0, 0.07];
    pub const SELECTED_NODE_FILL: RadialColor = [1.0, 1.0, 1.0, 0.18];
    pub const INACTIVE_TRACK: RadialColor = [100.0 / 255.0, 116.0 / 255.0, 139.0 / 255.0, 0.6];
    pub const INACTIVE_TRACK_SHADOW: RadialColor = [0.0, 0.0, 0.0, 0.28];
    pub const ACTIVE: RadialColor = [56.0 / 255.0, 189.0 / 255.0, 248.0 / 255.0, 1.0];
    pub const ACTIVE_GLOW: RadialColor = [56.0 / 255.0, 189.0 / 255.0, 248.0 / 255.0, 0.5];
    pub const INACTIVE_ICON: RadialColor = [156.0 / 255.0, 163.0 / 255.0, 175.0 / 255.0, 1.0];
    pub const ACTIVE_ICON: RadialColor = [1.0, 1.0, 1.0, 1.0];
    pub const DISABLED_CONTENT_OPACITY: f64 = 0.55;
    pub const SELECTED_TINT_OPACITY: f64 = 0.72;

    /// Swift: RadialPaletteArcStyle.surfaceColor(opacity:)
    pub fn surface_color(stored_opacity: f64) -> RadialColor {
        let [red, green, blue] = Self::SURFACE_RGB;
        [
            red,
            green,
            blue,
            RadialPaletteGlassStyle::effective_opacity(stored_opacity),
        ]
    }
}

/// Swift: RadialPaletteSectorGeometry
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialPaletteSectorGeometry {
    pub inner_radius: f64,
    pub outer_radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

impl RadialPaletteSectorGeometry {
    /// Swift: RadialPaletteSectorGeometry.contains(radius:clockwiseAngle:)
    pub fn contains(self, radius: f64, clockwise_angle: f64) -> bool {
        if radius < self.inner_radius || radius > self.outer_radius {
            return false;
        }
        let relative = normalized_angle(clockwise_angle - self.start_angle);
        relative <= self.end_angle - self.start_angle
    }

    /// Swift: RadialPaletteSectorGeometry.inset(by:)
    pub fn inset(self, amount: f64) -> Self {
        Self {
            inner_radius: self.inner_radius + amount,
            outer_radius: (self.inner_radius + amount).max(self.outer_radius - amount),
            start_angle: self.start_angle,
            end_angle: self.end_angle,
        }
    }
}

/// D2D 부채꼴 경로로 옮기기 전의 화면 좌표 핵심 수치.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialPaletteSectorPath {
    pub inner_radius_px: f64,
    pub outer_radius_px: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub outer_start: [f64; 2],
    pub outer_end: [f64; 2],
    pub inner_end: [f64; 2],
    pub inner_start: [f64; 2],
}

impl RadialPaletteSectorPath {
    /// 12시 기준 시계방향 각도를 y가 아래로 증가하는 픽셀 좌표로 바꾼다.
    pub fn from_geometry(
        geometry: RadialPaletteSectorGeometry,
        pixels_per_dip: f64,
        center: [f64; 2],
    ) -> Self {
        let point = |radius: f64, angle: f64| {
            [
                center[0] + angle.sin() * radius * pixels_per_dip,
                center[1] - angle.cos() * radius * pixels_per_dip,
            ]
        };
        Self {
            inner_radius_px: geometry.inner_radius * pixels_per_dip,
            outer_radius_px: geometry.outer_radius * pixels_per_dip,
            start_angle: geometry.start_angle,
            end_angle: geometry.end_angle,
            outer_start: point(geometry.outer_radius, geometry.start_angle),
            outer_end: point(geometry.outer_radius, geometry.end_angle),
            inner_end: point(geometry.inner_radius, geometry.end_angle),
            inner_start: point(geometry.inner_radius, geometry.start_angle),
        }
    }
}

pub fn radial_drag_threshold_reached(
    delta_x_px: f64,
    delta_y_px: f64,
    pixels_per_dip: f64,
) -> bool {
    delta_x_px.hypot(delta_y_px) >= DRAG_THRESHOLD * pixels_per_dip
}

pub fn radial_palette_should_show(
    annotation_enabled: bool,
    palette_visible: bool,
    style: PaletteStyle,
) -> bool {
    annotation_enabled && palette_visible && style == PaletteStyle::Radial
}

/// Swift: RadialPaletteGroupID
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RadialPaletteGroupId {
    Writing,
    Shapes,
    Erasing,
    BoardAndSettings,
    Presenting,
    Favorites,
}

impl RadialPaletteGroupId {
    /// Swift: RadialPaletteGroupID.rawValue
    pub const fn index(self) -> usize {
        match self {
            Self::Writing => 0,
            Self::Shapes => 1,
            Self::Erasing => 2,
            Self::BoardAndSettings => 3,
            Self::Presenting => 4,
            Self::Favorites => 5,
        }
    }

    /// Swift: RadialPaletteGroupID.allCases
    pub const fn all() -> [Self; 6] {
        [
            Self::Writing,
            Self::Shapes,
            Self::Erasing,
            Self::BoardAndSettings,
            Self::Presenting,
            Self::Favorites,
        ]
    }
}

/// Swift: RadialPaletteEntryAction
#[derive(Clone, Debug, PartialEq)]
pub enum RadialPaletteEntryAction {
    Command(ShortcutCommand),
    SelectEraserMode(EraserMode),
    OpenSettings,
    SaveFavorite,
    ApplyFavorite {
        index: usize,
        favorite: FavoriteTool,
    },
}

/// Swift: RadialPaletteEntry
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteEntry {
    pub identifier: String,
    pub name: String,
    pub icon_key: String,
    pub shortcut: String,
    pub tool: Option<InkTool>,
    pub eraser_mode: Option<EraserMode>,
    pub favorite: Option<FavoriteTool>,
    pub is_enabled: bool,
    pub action: RadialPaletteEntryAction,
}

/// Swift: RadialPaletteGroupDefinition
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteGroupDefinition {
    pub id: RadialPaletteGroupId,
    pub icon_key: &'static str,
    pub name: &'static str,
    pub entries: Vec<RadialPaletteEntry>,
}

/// Swift: RadialPaletteRing
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadialPaletteRing {
    Group,
    Entry,
    Option,
}

/// Swift: RadialPalettePreview
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadialPalettePreview {
    StrokeWidth { relative: f64, color: RadialColor },
    TextSize { relative: f64, color: RadialColor },
    EraserDiameter { relative: f64 },
}

impl RadialPalettePreview {
    /// Swift: RadialPalettePreview.relativeSize(of:in:)
    pub fn relative_size(value: f64, values: &[f64]) -> f64 {
        let minimum = values.iter().copied().reduce(f64::min);
        let maximum = values.iter().copied().reduce(f64::max);
        match (minimum, maximum) {
            (Some(minimum), Some(maximum)) if maximum > minimum => {
                ((value - minimum) / (maximum - minimum)).clamp(0.0, 1.0)
            }
            _ => 0.5,
        }
    }
}

/// Swift: RadialPaletteOption의 클로저를 값으로 옮긴 표현
#[derive(Clone, Debug, PartialEq)]
pub enum RadialPaletteOptionAction {
    SelectColor(usize),
    SetWidth { tool: InkTool, value: f64 },
    SetEraserDiameter(f64),
    SetLaserColor { index: usize, color: RadialColor },
}

/// Swift: RadialPaletteOption
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteOption {
    pub identifier: String,
    pub name: String,
    pub short_label: String,
    pub color: Option<RadialColor>,
    pub preview: Option<RadialPalettePreview>,
    pub shortcut: String,
    pub is_selected: bool,
    pub is_enabled: bool,
    pub action: RadialPaletteOptionAction,
}

/// Swift: RadialPaletteOptionGroup
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteOptionGroup {
    pub name: &'static str,
    pub options: Vec<RadialPaletteOption>,
}

/// Swift: RadialPaletteCell.Action
#[derive(Clone, Debug, PartialEq)]
pub enum RadialPaletteCellAction {
    Group(RadialPaletteGroupId),
    Entry(RadialPaletteEntry),
    Option(RadialPaletteOption),
}

/// Swift: RadialPaletteCell
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteCell {
    pub identifier: String,
    pub ring: RadialPaletteRing,
    pub geometry: RadialPaletteSectorGeometry,
    pub name: String,
    pub icon_key: Option<String>,
    pub short_label: String,
    pub shortcut: String,
    pub color: Option<RadialColor>,
    pub preview: Option<RadialPalettePreview>,
    pub is_selected: bool,
    pub is_enabled: bool,
    pub action: RadialPaletteCellAction,
}

/// Swift: RadialPaletteHitTarget
#[derive(Clone, Debug, PartialEq)]
pub enum RadialPaletteHitTarget {
    Center,
    Cell(RadialPaletteCell),
}

impl RadialPaletteHitTarget {
    /// Swift: RadialPaletteHitTarget.identifier
    pub fn identifier(&self) -> &str {
        match self {
            Self::Center => "palette.center",
            Self::Cell(cell) => &cell.identifier,
        }
    }
}

/// Swift: RadialPaletteNavigationKey
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadialPaletteNavigationKey {
    Shallower,
    Deeper,
    Previous,
    Next,
}

/// Swift: RadialPaletteFocus
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RadialPaletteFocus {
    Center,
    Cell(String),
}

impl RadialPaletteFocus {
    /// Swift: RadialPaletteFocus.identifier
    pub fn identifier(&self) -> &str {
        match self {
            Self::Center => "palette.center",
            Self::Cell(identifier) => identifier,
        }
    }
}

/// Swift: AppState와 SettingsStore에서 layoutCells()가 읽는 값
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteState {
    pub is_expanded: bool,
    pub selected_group: RadialPaletteGroupId,
    pub selected_entry_identifier: Option<String>,
    pub current_tool: InkTool,
    pub eraser_mode: EraserMode,
    pub eraser_area_shape: EraserAreaShape,
    pub selected_color_index: usize,
    pub per_tool_memory: PerToolMemorySettings,
    pub color_presets: Vec<RadialColor>,
    pub eraser_diameter: f64,
    pub eraser_preset_diameters: Vec<f64>,
    pub laser_color: RadialColor,
    pub laser_size: f64,
    pub favorites: Vec<FavoriteTool>,
    pub current_favorite_snapshot: Option<FavoriteTool>,
    pub is_spotlight_enabled: bool,
    pub is_cursor_highlight_enabled: bool,
    pub is_pass_through_enabled: bool,
    pub is_zoom_enabled: bool,
    pub presented_board: Option<ShortcutBoard>,
    pub palette_glass_opacity: f64,
}

impl RadialPaletteState {
    /// Swift: RadialPaletteController가 AppState와 SettingsStore에서 읽는 초기 값
    pub fn from_settings(settings: &AppSettings) -> Self {
        let selected_color_index = usize::try_from(settings.tools.per_tool_memory.pen.color_index)
            .expect("validated pen color index must be non-negative");
        Self {
            is_expanded: false,
            selected_group: RadialPaletteGroupId::Writing,
            selected_entry_identifier: Some("tool.pen".to_owned()),
            current_tool: InkTool::Pen,
            eraser_mode: parse_eraser_mode(&settings.eraser.mode)
                .expect("validated eraser mode must be known"),
            eraser_area_shape: parse_eraser_area_shape(&settings.eraser.area_shape)
                .expect("validated eraser area shape must be known"),
            selected_color_index,
            per_tool_memory: settings.tools.per_tool_memory.clone(),
            color_presets: settings.tools.colors.iter().map(settings_color).collect(),
            eraser_diameter: settings.eraser.diameter,
            eraser_preset_diameters: settings.eraser.preset_diameters.clone(),
            laser_color: settings_color(&settings.tools.laser_color),
            laser_size: settings.tools.laser_size,
            favorites: settings.tools.favorites.clone(),
            current_favorite_snapshot: None,
            is_spotlight_enabled: false,
            is_cursor_highlight_enabled: false,
            is_pass_through_enabled: false,
            is_zoom_enabled: false,
            presented_board: None,
            palette_glass_opacity: settings.tools.palette_glass_opacity,
        }
    }

    /// Swift: AppState.selectedColor
    pub fn selected_color(&self) -> RadialColor {
        *self
            .color_presets
            .get(self.selected_color_index)
            .expect("validated selected color index must reference a preset")
    }

    /// Swift: AppState.width(for:)
    pub fn remembered_width(&self, tool: InkTool) -> Option<f64> {
        tool.memory_slot()
            .map(|slot| slot.setting(&self.per_tool_memory).size)
    }
}

impl Default for RadialPaletteState {
    /// Swift: AppSettings.default + RadialPaletteController initial selection
    fn default() -> Self {
        Self::from_settings(&AppSettings::default())
    }
}

/// Swift: RadialPaletteRenderCell
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteRenderCell {
    pub id: String,
    pub ring: RadialPaletteRing,
    pub geometry: RadialPaletteSectorGeometry,
    pub icon_key: Option<String>,
    pub accessibility_label: String,
    pub short_label: String,
    pub shortcut: String,
    pub color: Option<RadialColor>,
    pub preview: Option<RadialPalettePreview>,
    pub is_selected: bool,
    pub is_enabled: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
    pub has_attribute_configuration: bool,
}

/// Swift: RadialPaletteRenderState
#[derive(Clone, Debug, PartialEq)]
pub struct RadialPaletteRenderState {
    pub is_expanded: bool,
    pub shows_key_caps: bool,
    pub glass_opacity: f64,
    pub cells: Vec<RadialPaletteRenderCell>,
    pub center_icon_key: Option<String>,
    pub center_accessibility_label: Option<String>,
    pub center_shortcut: String,
    pub center_color: RadialColor,
    pub center_stroke_width: f64,
    pub center_is_pass_through_enabled: bool,
    pub center_is_hovered: bool,
    pub center_is_focused: bool,
    pub center_has_attribute_configuration: bool,
}

/// Swift: RadialPaletteEntryAction과 RadialPaletteOption.action의 실행 의도 값
#[derive(Clone, Debug, PartialEq)]
pub enum RadialPaletteIntent {
    ToggleExpansion,
    SelectGroup(RadialPaletteGroupId),
    Execute(ShortcutCommand),
    SelectEraserMode(EraserMode),
    ToggleEraserAreaShape,
    OpenSettings,
    SaveFavorite,
    ApplyFavorite {
        index: usize,
        favorite: FavoriteTool,
    },
    RemoveFavorite {
        index: usize,
        favorite: FavoriteTool,
    },
    SelectColor(usize),
    SetWidth {
        tool: InkTool,
        value: f64,
    },
    SetEraserDiameter(f64),
    SetLaserColor {
        index: usize,
        color: RadialColor,
    },
}

/// Swift: RadialPaletteCatalog.groups + entries(for:)
pub fn group_definitions(state: &RadialPaletteState) -> Vec<RadialPaletteGroupDefinition> {
    vec![
        group(
            RadialPaletteGroupId::Writing,
            "group.writing",
            "작성",
            vec![
                command_entry(ShortcutCommand::SelectTool(ShortcutTool::Pen)),
                command_entry(ShortcutCommand::SelectTool(ShortcutTool::Highlighter)),
                command_entry(ShortcutCommand::SelectTool(ShortcutTool::Text)),
            ],
        ),
        group(
            RadialPaletteGroupId::Shapes,
            "group.shapes",
            "도형",
            vec![
                command_entry(ShortcutCommand::SelectTool(ShortcutTool::Arrow)),
                command_entry(ShortcutCommand::SelectTool(ShortcutTool::Rectangle)),
                command_entry(ShortcutCommand::SelectTool(ShortcutTool::Ellipse)),
            ],
        ),
        group(
            RadialPaletteGroupId::Erasing,
            "group.erasing",
            "지우기",
            vec![
                eraser_entry(EraserMode::Partial, state.eraser_area_shape),
                eraser_entry(EraserMode::Stroke, state.eraser_area_shape),
                eraser_entry(EraserMode::Area, state.eraser_area_shape),
                command_entry(ShortcutCommand::Undo),
                command_entry(ShortcutCommand::Redo),
                command_entry(ShortcutCommand::ClearAll),
            ],
        ),
        group(
            RadialPaletteGroupId::BoardAndSettings,
            "group.board",
            "보드·설정",
            vec![
                command_entry(ShortcutCommand::ToggleBoard(ShortcutBoard::White)),
                command_entry(ShortcutCommand::ToggleBoard(ShortcutBoard::Black)),
                command_entry(ShortcutCommand::PreviousBoardPage),
                command_entry(ShortcutCommand::NextBoardPage),
                command_entry(ShortcutCommand::SaveCurrentContent),
                command_entry(ShortcutCommand::CopyCurrentImage),
                settings_entry(),
            ],
        ),
        group(
            RadialPaletteGroupId::Presenting,
            "group.presenting",
            "발표",
            vec![
                command_entry(ShortcutCommand::UseLaser),
                command_entry(ShortcutCommand::ToggleSpotlight),
                command_entry(ShortcutCommand::SelectTool(ShortcutTool::Cover)),
                command_entry(ShortcutCommand::TogglePassThroughMode),
                command_entry(ShortcutCommand::ToggleZoom),
                command_entry(ShortcutCommand::ToggleCursorHighlight),
            ],
        ),
        group(
            RadialPaletteGroupId::Favorites,
            "group.favorites",
            "즐겨찾기",
            favorite_entries(state),
        ),
    ]
}

/// Swift: RadialPaletteController.layoutCells()
pub fn layout_cells(state: &RadialPaletteState) -> Vec<RadialPaletteCell> {
    if !state.is_expanded {
        return Vec::new();
    }

    let definitions = group_definitions(state);
    let full_circle = 2.0 * PI;
    let group_slice = full_circle / definitions.len() as f64;
    let mut cells = Vec::new();

    for (index, definition) in definitions.iter().enumerate() {
        let center_angle = index as f64 * group_slice;
        cells.push(RadialPaletteCell {
            identifier: format!("group.{}", definition.id.index()),
            ring: RadialPaletteRing::Group,
            geometry: RadialPaletteSectorGeometry {
                inner_radius: GROUP_INNER_RADIUS,
                outer_radius: GROUP_OUTER_RADIUS,
                start_angle: center_angle - group_slice / 2.0 + SECTOR_GAP,
                end_angle: center_angle + group_slice / 2.0 - SECTOR_GAP,
            },
            name: definition.name.to_owned(),
            icon_key: Some(definition.icon_key.to_owned()),
            short_label: String::new(),
            shortcut: String::new(),
            color: None,
            preview: None,
            is_selected: definition.id == state.selected_group,
            is_enabled: true,
            action: RadialPaletteCellAction::Group(definition.id),
        });
    }

    let selected_group = definitions
        .iter()
        .find(|definition| definition.id == state.selected_group)
        .expect("all radial groups must have a definition");
    if !selected_group.entries.is_empty() {
        let count = selected_group.entries.len() as f64;
        let span = (count * 24.0 * PI / 180.0).clamp(90.0 * PI / 180.0, 200.0 * PI / 180.0);
        let slice = span / count;
        let group_center = selected_group.id.index() as f64 * group_slice;
        let start = group_center - span / 2.0;
        for (index, entry) in selected_group.entries.iter().enumerate() {
            cells.push(RadialPaletteCell {
                identifier: format!("entry.{}", entry.identifier),
                ring: RadialPaletteRing::Entry,
                geometry: RadialPaletteSectorGeometry {
                    inner_radius: ENTRY_INNER_RADIUS,
                    outer_radius: ENTRY_OUTER_RADIUS,
                    start_angle: start + index as f64 * slice + SECTOR_GAP,
                    end_angle: start + (index + 1) as f64 * slice - SECTOR_GAP,
                },
                name: entry.name.clone(),
                icon_key: Some(entry.icon_key.clone()),
                short_label: String::new(),
                shortcut: entry.shortcut.clone(),
                color: None,
                preview: None,
                is_selected: entry_is_selected(entry, state),
                is_enabled: entry.is_enabled,
                action: RadialPaletteCellAction::Entry(entry.clone()),
            });
        }
    }

    let option_groups = selected_entry(selected_group, state)
        .map(|entry| option_groups_for_entry(entry, state))
        .unwrap_or_else(Vec::new);
    let option_count = option_groups
        .iter()
        .map(|group| group.options.len())
        .sum::<usize>();
    if option_count == 0 {
        return cells;
    }

    let total_gap = option_groups.len() as f64 * PROPERTY_GAP;
    let option_slice = OPTION_MAXIMUM_SLICE.min((full_circle - total_gap) / option_count as f64);
    let option_span = option_count as f64 * option_slice;
    let mut cursor = if option_groups.len() == 1 {
        let group_center = state.selected_group.index() as f64 * group_slice;
        normalized_angle(group_center + full_circle / 2.0 - option_span / 2.0)
    } else {
        PROPERTY_GAP / 2.0
    };
    for option_group in option_groups {
        for option in option_group.options {
            cells.push(RadialPaletteCell {
                identifier: format!("option.{}", option.identifier),
                ring: RadialPaletteRing::Option,
                geometry: RadialPaletteSectorGeometry {
                    inner_radius: OPTION_INNER_RADIUS,
                    outer_radius: OPTION_OUTER_RADIUS,
                    start_angle: cursor + SECTOR_GAP / 2.0,
                    end_angle: cursor + option_slice - SECTOR_GAP / 2.0,
                },
                name: option.name.clone(),
                icon_key: None,
                short_label: option.short_label.clone(),
                shortcut: option.shortcut.clone(),
                color: option.color,
                preview: option.preview,
                is_selected: option.is_selected,
                is_enabled: option.is_enabled,
                action: RadialPaletteCellAction::Option(option),
            });
            cursor += option_slice;
        }
        cursor += PROPERTY_GAP;
    }
    cells
}

/// Swift: RadialPaletteController.hitTarget(at:in:)
pub fn hit_target(
    state: &RadialPaletteState,
    point: Point,
    bounds: Rect,
) -> Option<RadialPaletteHitTarget> {
    let delta_x = point.x - (bounds.x + bounds.width / 2.0);
    let delta_y = point.y - (bounds.y + bounds.height / 2.0);
    let radius = delta_x.hypot(delta_y);
    if radius <= CENTER_RADIUS {
        return Some(RadialPaletteHitTarget::Center);
    }
    if !state.is_expanded {
        return None;
    }
    let angle = delta_x.atan2(delta_y);
    layout_cells(state)
        .into_iter()
        .find(|cell| cell.geometry.contains(radius, angle))
        .map(RadialPaletteHitTarget::Cell)
}

/// Swift: nextRadialPaletteFocus(in:from:for:selectedEntryIdentifier:)
pub fn next_radial_palette_focus(
    cells: &[RadialPaletteCell],
    current: &RadialPaletteFocus,
    key: RadialPaletteNavigationKey,
    selected_entry_identifier: Option<&str>,
) -> RadialPaletteFocus {
    let enabled_cells = cells
        .iter()
        .filter(|cell| cell.is_enabled)
        .collect::<Vec<_>>();
    let selected_cell = |ring| {
        enabled_cells
            .iter()
            .copied()
            .find(|cell| cell.ring == ring && cell.is_selected)
    };
    let selected_entry_priority = |cell: &RadialPaletteCell| {
        if cell.ring != RadialPaletteRing::Entry {
            return None;
        }
        let RadialPaletteCellAction::Entry(entry) = &cell.action else {
            return None;
        };
        if selected_entry_identifier == Some(entry.identifier.as_str()) {
            Some(0)
        } else if cell.is_selected && entry.tool.is_some() {
            Some(1)
        } else {
            None
        }
    };
    let selected_entry_cell = || {
        (0..=1).find_map(|priority| {
            enabled_cells
                .iter()
                .copied()
                .find(|cell| selected_entry_priority(cell) == Some(priority))
        })
    };
    let first_cell = |ring| enabled_cells.iter().copied().find(|cell| cell.ring == ring);
    let focus = |cell: Option<&RadialPaletteCell>| {
        cell.map(|cell| RadialPaletteFocus::Cell(cell.identifier.clone()))
    };

    match key {
        RadialPaletteNavigationKey::Shallower => match current {
            RadialPaletteFocus::Center => RadialPaletteFocus::Center,
            RadialPaletteFocus::Cell(identifier) => {
                let Some(cell) = enabled_cells
                    .iter()
                    .copied()
                    .find(|cell| cell.identifier == *identifier)
                else {
                    return current.clone();
                };
                match cell.ring {
                    RadialPaletteRing::Group => RadialPaletteFocus::Center,
                    RadialPaletteRing::Entry => focus(selected_cell(RadialPaletteRing::Group))
                        .unwrap_or_else(|| current.clone()),
                    RadialPaletteRing::Option => {
                        focus(selected_entry_cell()).unwrap_or_else(|| current.clone())
                    }
                }
            }
        },
        RadialPaletteNavigationKey::Deeper => match current {
            RadialPaletteFocus::Center => {
                focus(selected_cell(RadialPaletteRing::Group)).unwrap_or_else(|| current.clone())
            }
            RadialPaletteFocus::Cell(identifier) => {
                let Some(cell) = enabled_cells
                    .iter()
                    .copied()
                    .find(|cell| cell.identifier == *identifier)
                else {
                    return current.clone();
                };
                match cell.ring {
                    RadialPaletteRing::Group => focus(
                        selected_entry_cell().or_else(|| first_cell(RadialPaletteRing::Entry)),
                    )
                    .unwrap_or_else(|| current.clone()),
                    RadialPaletteRing::Entry => {
                        if selected_entry_priority(cell).is_none() {
                            current.clone()
                        } else {
                            focus(first_cell(RadialPaletteRing::Option))
                                .unwrap_or_else(|| current.clone())
                        }
                    }
                    RadialPaletteRing::Option => current.clone(),
                }
            }
        },
        RadialPaletteNavigationKey::Previous | RadialPaletteNavigationKey::Next => {
            let RadialPaletteFocus::Cell(identifier) = current else {
                return current.clone();
            };
            let Some(current_cell) = enabled_cells
                .iter()
                .copied()
                .find(|cell| cell.identifier == *identifier)
            else {
                return current.clone();
            };
            let ring_cells = enabled_cells
                .iter()
                .copied()
                .filter(|cell| cell.ring == current_cell.ring)
                .collect::<Vec<_>>();
            if ring_cells.len() <= 1 {
                return current.clone();
            }
            let Some(index) = ring_cells
                .iter()
                .position(|cell| cell.identifier == *identifier)
            else {
                return current.clone();
            };
            let offset = if key == RadialPaletteNavigationKey::Next {
                1_isize
            } else {
                -1_isize
            };
            let target = if current_cell.ring == RadialPaletteRing::Entry {
                (index as isize + offset).clamp(0, ring_cells.len() as isize - 1) as usize
            } else {
                (index as isize + offset).rem_euclid(ring_cells.len() as isize) as usize
            };
            RadialPaletteFocus::Cell(ring_cells[target].identifier.clone())
        }
    }
}

/// Swift: RadialPaletteController.activate(_:)
pub fn primary_intent(
    target: &RadialPaletteHitTarget,
    state: &RadialPaletteState,
) -> Option<RadialPaletteIntent> {
    match target {
        RadialPaletteHitTarget::Center => Some(RadialPaletteIntent::ToggleExpansion),
        RadialPaletteHitTarget::Cell(cell) if !cell.is_enabled => None,
        RadialPaletteHitTarget::Cell(cell) => match &cell.action {
            RadialPaletteCellAction::Group(group) => Some(RadialPaletteIntent::SelectGroup(*group)),
            RadialPaletteCellAction::Entry(entry) => match &entry.action {
                RadialPaletteEntryAction::Command(command) => {
                    Some(RadialPaletteIntent::Execute(command.clone()))
                }
                RadialPaletteEntryAction::SelectEraserMode(EraserMode::Area)
                    if state.current_tool == InkTool::Eraser
                        && state.eraser_mode == EraserMode::Area =>
                {
                    Some(RadialPaletteIntent::ToggleEraserAreaShape)
                }
                RadialPaletteEntryAction::SelectEraserMode(mode) => {
                    Some(RadialPaletteIntent::SelectEraserMode(*mode))
                }
                RadialPaletteEntryAction::OpenSettings => Some(RadialPaletteIntent::OpenSettings),
                RadialPaletteEntryAction::SaveFavorite => Some(RadialPaletteIntent::SaveFavorite),
                RadialPaletteEntryAction::ApplyFavorite { index, favorite } => {
                    Some(RadialPaletteIntent::ApplyFavorite {
                        index: *index,
                        favorite: favorite.clone(),
                    })
                }
            },
            RadialPaletteCellAction::Option(option) => Some(option_intent(&option.action)),
        },
    }
}

/// Swift: RadialPaletteView.rightMouseDown(with:) + RadialPaletteController.stepBack()
pub fn secondary_intent(target: &RadialPaletteHitTarget) -> Option<RadialPaletteIntent> {
    match target {
        RadialPaletteHitTarget::Center => {
            Some(RadialPaletteIntent::Execute(ShortcutCommand::StepBack))
        }
        RadialPaletteHitTarget::Cell(cell) => {
            let RadialPaletteCellAction::Entry(entry) = &cell.action else {
                return None;
            };
            let RadialPaletteEntryAction::ApplyFavorite { index, favorite } = &entry.action else {
                return None;
            };
            Some(RadialPaletteIntent::RemoveFavorite {
                index: *index,
                favorite: favorite.clone(),
            })
        }
    }
}

/// Swift: RadialPaletteController.updateRenderModel(_:hoveredIdentifier:)
pub fn render_state(
    state: &RadialPaletteState,
    shows_key_caps: bool,
    hovered_identifier: Option<&str>,
    focused: Option<&RadialPaletteFocus>,
) -> RadialPaletteRenderState {
    let focused_identifier = focused.map(RadialPaletteFocus::identifier);
    let center_command = if state.is_pass_through_enabled {
        ShortcutCommand::TogglePassThroughMode
    } else {
        command_for_center_tool(state.current_tool)
    };
    let center_definition = required_definition(&center_command);
    let center_color = if state.current_tool == InkTool::Laser {
        state.laser_color
    } else {
        state.selected_color()
    };
    RadialPaletteRenderState {
        is_expanded: state.is_expanded,
        shows_key_caps,
        glass_opacity: RadialPaletteGlassStyle::effective_opacity(state.palette_glass_opacity),
        cells: layout_cells(state)
            .into_iter()
            .map(|cell| {
                let has_attribute_configuration = cell_has_attribute_configuration(&cell);
                RadialPaletteRenderCell {
                    id: cell.identifier.clone(),
                    ring: cell.ring,
                    geometry: cell.geometry,
                    icon_key: cell.icon_key,
                    accessibility_label: cell.name,
                    short_label: cell.short_label,
                    shortcut: cell.shortcut,
                    color: cell.color,
                    preview: cell.preview,
                    is_selected: cell.is_selected,
                    is_enabled: cell.is_enabled,
                    is_hovered: hovered_identifier == Some(cell.identifier.as_str()),
                    is_focused: focused_identifier == Some(cell.identifier.as_str()),
                    has_attribute_configuration,
                }
            })
            .collect(),
        center_icon_key: Some(center_definition.id.clone()),
        center_accessibility_label: Some(center_definition.name.clone()),
        center_shortcut: center_definition.display_keys(),
        center_color,
        center_stroke_width: center_stroke_width(state),
        center_is_pass_through_enabled: state.is_pass_through_enabled,
        center_is_hovered: hovered_identifier == Some("palette.center"),
        center_is_focused: focused_identifier == Some("palette.center"),
        center_has_attribute_configuration: !state.is_pass_through_enabled
            && tool_has_attribute_configuration(state.current_tool),
    }
}

// Swift: RadialPaletteSectorGeometry.normalizedAngle(_:)
fn normalized_angle(angle: f64) -> f64 {
    angle.rem_euclid(2.0 * PI)
}

// Swift: SettingsColor.nsColor의 역방향 값 변환
fn settings_color(color: &SettingsColor) -> RadialColor {
    [color.red, color.green, color.blue, color.alpha]
}

// Swift: RadialPaletteGroupDefinition.init(...)
fn group(
    id: RadialPaletteGroupId,
    icon_key: &'static str,
    name: &'static str,
    entries: Vec<RadialPaletteEntry>,
) -> RadialPaletteGroupDefinition {
    RadialPaletteGroupDefinition {
        id,
        icon_key,
        name,
        entries,
    }
}

// Swift: ShortcutCatalog.definition(for:)
fn required_definition(command: &ShortcutCommand) -> ShortcutDefinition {
    ShortcutCatalog::definitions()
        .into_iter()
        .find(|definition| definition.command == *command)
        .expect("radial palette command must be present in ShortcutCatalog")
}

// Swift: RadialPaletteEntry.command(_:)
fn command_entry(command: ShortcutCommand) -> RadialPaletteEntry {
    let definition = required_definition(&command);
    let tool = tool_for_command(&command);
    let shortcut = definition.display_keys();
    RadialPaletteEntry {
        identifier: definition.id.clone(),
        name: definition.name,
        icon_key: definition.id,
        shortcut,
        tool,
        eraser_mode: None,
        favorite: None,
        is_enabled: true,
        action: RadialPaletteEntryAction::Command(command),
    }
}

// Swift: RadialPaletteEntry.eraserMode(_:)
fn eraser_entry(mode: EraserMode, area_shape: EraserAreaShape) -> RadialPaletteEntry {
    let (identifier, name, icon_key) = match mode {
        EraserMode::Partial => ("eraser.mode.partial", "부분 지우개", "tool.eraser"),
        EraserMode::Stroke => ("eraser.mode.stroke", "획 지우개", "eraser.mode.stroke"),
        EraserMode::Area => (
            "eraser.mode.area",
            "영역 지우개",
            match area_shape {
                EraserAreaShape::Rectangle => "option.area.rectangle",
                EraserAreaShape::Lasso => "option.area.lasso",
            },
        ),
    };
    let shortcut = required_definition(&ShortcutCommand::SelectOrCycleEraser).display_keys();
    RadialPaletteEntry {
        identifier: identifier.to_owned(),
        name: name.to_owned(),
        icon_key: icon_key.to_owned(),
        shortcut,
        tool: Some(InkTool::Eraser),
        eraser_mode: Some(mode),
        favorite: None,
        is_enabled: true,
        action: RadialPaletteEntryAction::SelectEraserMode(mode),
    }
}

// Swift: RadialPaletteEntry.openSettings
fn settings_entry() -> RadialPaletteEntry {
    RadialPaletteEntry {
        identifier: "palette.settings".to_owned(),
        name: "설정 열기".to_owned(),
        icon_key: "palette.settings".to_owned(),
        shortcut: String::new(),
        tool: None,
        eraser_mode: None,
        favorite: None,
        is_enabled: true,
        action: RadialPaletteEntryAction::OpenSettings,
    }
}

// Swift: RadialPaletteController.entries(for:) 즐겨찾기 분기
fn favorite_entries(state: &RadialPaletteState) -> Vec<RadialPaletteEntry> {
    let is_full = state.favorites.len() >= FavoritesDefaults::MAXIMUM_COUNT;
    let is_duplicate = state
        .current_favorite_snapshot
        .as_ref()
        .is_some_and(|current| state.favorites.contains(current));
    let (status, can_save) = if state.current_favorite_snapshot.is_none() {
        ("현재 도구 저장 불가", false)
    } else if is_full {
        ("최대 8개", false)
    } else if is_duplicate {
        ("이미 저장됨", false)
    } else {
        ("", true)
    };
    let suffix = if status.is_empty() {
        String::new()
    } else {
        format!(" ({status})")
    };
    let mut entries = vec![RadialPaletteEntry {
        identifier: "palette.favorite.save".to_owned(),
        name: format!("지금 설정 저장{suffix} · 저장한 칸을 우클릭하면 삭제됩니다"),
        icon_key: "palette.favorite.save".to_owned(),
        shortcut: String::new(),
        tool: None,
        eraser_mode: None,
        favorite: None,
        is_enabled: can_save,
        action: RadialPaletteEntryAction::SaveFavorite,
    }];
    entries.extend(
        state
            .favorites
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, favorite)| favorite_entry(index, favorite)),
    );
    entries
}

// Swift: RadialPaletteEntry.favorite(_:index:name:)
fn favorite_entry(index: usize, favorite: FavoriteTool) -> RadialPaletteEntry {
    let tool =
        favorite_tool(&favorite).expect("validated favorite must reference a radial palette tool");
    RadialPaletteEntry {
        identifier: format!("palette.favorite.{index}"),
        name: favorite_description(&favorite),
        icon_key: format!("palette.favorite.{index}"),
        shortcut: String::new(),
        tool: Some(tool),
        eraser_mode: favorite_eraser_mode(&favorite),
        favorite: Some(favorite.clone()),
        is_enabled: true,
        action: RadialPaletteEntryAction::ApplyFavorite { index, favorite },
    }
}

// Swift: RadialPaletteController.favoriteDescription(_:)
fn favorite_description(favorite: &FavoriteTool) -> String {
    let tool =
        favorite_tool(favorite).expect("validated favorite must reference a radial palette tool");
    let mut parts = vec![tool_name(tool).to_owned()];
    if let Some(color_index) = favorite.color_index {
        parts.push(format!("색 {}", color_index + 1));
    }
    match tool {
        InkTool::Text => parts.push(format!("크기 {}", format_number(favorite.size))),
        InkTool::Eraser => {
            parts.push(format!("크기 {}", format_number(favorite.size)));
            match favorite_eraser_mode(favorite) {
                Some(EraserMode::Partial) => parts.push("부분 지우개".to_owned()),
                Some(EraserMode::Stroke) => parts.push("획 지우개".to_owned()),
                Some(EraserMode::Area) => {
                    let shape = if favorite.eraser_area_shape.as_deref() == Some("lasso") {
                        "올가미"
                    } else {
                        "사각형"
                    };
                    parts.push(format!("영역 지우개({shape})"));
                }
                None => {}
            }
        }
        InkTool::Laser => parts.push(format!("꼬리 {}초", format_number(favorite.size))),
        InkTool::Arrow
        | InkTool::Pen
        | InkTool::Highlighter
        | InkTool::Rectangle
        | InkTool::Ellipse => parts.push(format!("굵기 {}", format_number(favorite.size))),
        InkTool::Select | InkTool::Cover => {}
    }
    parts.join(" · ")
}

// Swift: RadialPaletteController.selectedEntry()
fn selected_entry<'a>(
    group: &'a RadialPaletteGroupDefinition,
    state: &RadialPaletteState,
) -> Option<&'a RadialPaletteEntry> {
    let identifier = state.selected_entry_identifier.as_deref()?;
    group
        .entries
        .iter()
        .find(|entry| entry.identifier == identifier)
}

// Swift: RadialPaletteController.entryIsSelected(_:)
fn entry_is_selected(entry: &RadialPaletteEntry, state: &RadialPaletteState) -> bool {
    if let Some(favorite) = &entry.favorite {
        return state.current_favorite_snapshot.as_ref() == Some(favorite);
    }
    if let Some(mode) = entry.eraser_mode {
        return state.current_tool == InkTool::Eraser && state.eraser_mode == mode;
    }
    if let Some(tool) = entry.tool {
        return state.current_tool == tool;
    }
    match &entry.action {
        RadialPaletteEntryAction::Command(ShortcutCommand::ToggleSpotlight) => {
            state.is_spotlight_enabled
        }
        RadialPaletteEntryAction::Command(ShortcutCommand::ToggleCursorHighlight) => {
            state.is_cursor_highlight_enabled
        }
        RadialPaletteEntryAction::Command(ShortcutCommand::TogglePassThroughMode) => {
            state.is_pass_through_enabled
        }
        RadialPaletteEntryAction::Command(ShortcutCommand::ToggleZoom) => state.is_zoom_enabled,
        RadialPaletteEntryAction::Command(ShortcutCommand::ToggleBoard(board)) => {
            state.presented_board == Some(*board)
        }
        _ => state.selected_entry_identifier.as_deref() == Some(entry.identifier.as_str()),
    }
}

// Swift: RadialPaletteController.optionGroups(for:)
fn option_groups_for_entry(
    entry: &RadialPaletteEntry,
    state: &RadialPaletteState,
) -> Vec<RadialPaletteOptionGroup> {
    let Some(tool) = entry.tool else {
        return Vec::new();
    };
    match tool {
        InkTool::Pen => {
            color_and_width_options(state, tool, &WRITING_WIDTHS, "굵기", "pen.width", false)
        }
        InkTool::Highlighter => color_and_width_options(
            state,
            tool,
            &HIGHLIGHTER_WIDTHS,
            "굵기",
            "highlighter.width",
            false,
        ),
        InkTool::Text => {
            color_and_width_options(state, tool, &TEXT_SIZES, "크기", "text.size", true)
        }
        InkTool::Arrow => {
            color_and_width_options(state, tool, &WRITING_WIDTHS, "굵기", "arrow.width", false)
        }
        InkTool::Rectangle | InkTool::Ellipse => {
            color_and_width_options(state, tool, &WRITING_WIDTHS, "굵기", "shape.width", false)
        }
        InkTool::Eraser => eraser_options(entry.eraser_mode.unwrap_or(state.eraser_mode), state),
        InkTool::Laser => vec![laser_color_options(state)],
        InkTool::Select | InkTool::Cover => Vec::new(),
    }
}

// Swift: RadialPaletteController.colorOptions() + widthOptions(...)
fn color_and_width_options(
    state: &RadialPaletteState,
    tool: InkTool,
    values: &[f64],
    property_name: &'static str,
    id_prefix: &str,
    is_text: bool,
) -> Vec<RadialPaletteOptionGroup> {
    vec![
        color_options(state),
        width_options(state, tool, values, property_name, id_prefix, is_text),
    ]
}

// Swift: RadialPaletteController.colorOptions()
fn color_options(state: &RadialPaletteState) -> RadialPaletteOptionGroup {
    let options = state
        .color_presets
        .iter()
        .copied()
        .enumerate()
        .map(|(index, color)| {
            let definition = required_definition(&ShortcutCommand::SelectColor(index));
            let shortcut = definition.display_keys();
            RadialPaletteOption {
                identifier: format!("ink.color.{index}"),
                name: definition.name,
                short_label: String::new(),
                color: Some(color),
                preview: None,
                shortcut,
                is_selected: state.selected_color_index == index,
                is_enabled: true,
                action: RadialPaletteOptionAction::SelectColor(index),
            }
        })
        .collect();
    RadialPaletteOptionGroup {
        name: "색",
        options,
    }
}

// Swift: RadialPaletteController.widthOptions(...)
fn width_options(
    state: &RadialPaletteState,
    tool: InkTool,
    values: &[f64],
    property_name: &'static str,
    id_prefix: &str,
    is_text: bool,
) -> RadialPaletteOptionGroup {
    let selected = state
        .remembered_width(tool)
        .expect("option-bearing drawing tool must have remembered width");
    let ink_color = state.selected_color();
    let options = values
        .iter()
        .copied()
        .map(|value| {
            let label = format_number(value);
            let relative = RadialPalettePreview::relative_size(value, values);
            let preview = if is_text {
                RadialPalettePreview::TextSize {
                    relative,
                    color: ink_color,
                }
            } else {
                RadialPalettePreview::StrokeWidth {
                    relative,
                    color: ink_color,
                }
            };
            RadialPaletteOption {
                identifier: format!("{id_prefix}.{label}"),
                name: format!("{property_name} {label}"),
                short_label: label,
                color: None,
                preview: Some(preview),
                shortcut: String::new(),
                is_selected: (selected - value).abs() < 0.001,
                is_enabled: true,
                action: RadialPaletteOptionAction::SetWidth { tool, value },
            }
        })
        .collect();
    RadialPaletteOptionGroup {
        name: property_name,
        options,
    }
}

// Swift: RadialPaletteController.eraserOptions(for:)
fn eraser_options(mode: EraserMode, state: &RadialPaletteState) -> Vec<RadialPaletteOptionGroup> {
    if mode == EraserMode::Area {
        return Vec::new();
    }
    let options = state
        .eraser_preset_diameters
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| {
            let label = format_number(value);
            RadialPaletteOption {
                identifier: format!("eraser.size.{index}"),
                name: format!("지우개 크기 {label}"),
                short_label: label,
                color: None,
                preview: Some(RadialPalettePreview::EraserDiameter {
                    relative: RadialPalettePreview::relative_size(
                        value,
                        &state.eraser_preset_diameters,
                    ),
                }),
                shortcut: String::new(),
                is_selected: (state.eraser_diameter - value).abs() < 0.001,
                is_enabled: true,
                action: RadialPaletteOptionAction::SetEraserDiameter(value),
            }
        })
        .collect();
    vec![RadialPaletteOptionGroup {
        name: "크기",
        options,
    }]
}

// Swift: RadialPaletteController.laserColorOptions()
fn laser_color_options(state: &RadialPaletteState) -> RadialPaletteOptionGroup {
    let options = state
        .color_presets
        .iter()
        .copied()
        .enumerate()
        .map(|(index, color)| {
            let definition = required_definition(&ShortcutCommand::SelectColor(index));
            RadialPaletteOption {
                identifier: format!("laser.color.{index}"),
                name: definition.name,
                short_label: String::new(),
                color: Some(color),
                preview: None,
                shortcut: String::new(),
                is_selected: state.laser_color == color,
                is_enabled: true,
                action: RadialPaletteOptionAction::SetLaserColor { index, color },
            }
        })
        .collect();
    RadialPaletteOptionGroup {
        name: "색",
        options,
    }
}

// Swift: RadialPaletteController.activate(_:) 옵션 분기
fn option_intent(action: &RadialPaletteOptionAction) -> RadialPaletteIntent {
    match action {
        RadialPaletteOptionAction::SelectColor(index) => RadialPaletteIntent::SelectColor(*index),
        RadialPaletteOptionAction::SetWidth { tool, value } => RadialPaletteIntent::SetWidth {
            tool: *tool,
            value: *value,
        },
        RadialPaletteOptionAction::SetEraserDiameter(value) => {
            RadialPaletteIntent::SetEraserDiameter(*value)
        }
        RadialPaletteOptionAction::SetLaserColor { index, color } => {
            RadialPaletteIntent::SetLaserColor {
                index: *index,
                color: *color,
            }
        }
    }
}

// Swift: RadialPaletteController.attributeConfiguration(for:)
fn cell_has_attribute_configuration(cell: &RadialPaletteCell) -> bool {
    let RadialPaletteCellAction::Entry(entry) = &cell.action else {
        return false;
    };
    entry.favorite.is_some()
        || (entry.eraser_mode.is_none() && entry.tool.is_some_and(tool_has_attribute_configuration))
}

// Swift: AttrIconToolCatalog.manifestKeyByTool
fn tool_has_attribute_configuration(tool: InkTool) -> bool {
    matches!(
        tool,
        InkTool::Pen
            | InkTool::Highlighter
            | InkTool::Laser
            | InkTool::Eraser
            | InkTool::Arrow
            | InkTool::Rectangle
            | InkTool::Ellipse
            | InkTool::Text
    )
}

// Swift: RadialPaletteController.centerDefinition()
fn command_for_center_tool(tool: InkTool) -> ShortcutCommand {
    match tool {
        InkTool::Laser => ShortcutCommand::UseLaser,
        InkTool::Eraser => ShortcutCommand::SelectOrCycleEraser,
        _ => ShortcutCommand::SelectTool(
            shortcut_tool(tool).expect("non-laser and non-eraser center tool must be selectable"),
        ),
    }
}

// Swift: RadialPaletteController.centerStrokeWidth()
fn center_stroke_width(state: &RadialPaletteState) -> f64 {
    let value = match state.current_tool {
        InkTool::Pen
        | InkTool::Highlighter
        | InkTool::Arrow
        | InkTool::Rectangle
        | InkTool::Ellipse => state
            .remembered_width(state.current_tool)
            .expect("drawing tool must have remembered width"),
        InkTool::Laser => state.laser_size,
        InkTool::Eraser => state.eraser_diameter / 3.0,
        InkTool::Text => {
            state
                .remembered_width(InkTool::Text)
                .expect("text tool must have remembered size")
                / 3.0
        }
        InkTool::Select | InkTool::Cover => 2.0,
    };
    (value / 3.0).clamp(2.0, 6.0)
}

// Swift: RadialPaletteEntry.inkTool
fn tool_for_command(command: &ShortcutCommand) -> Option<InkTool> {
    match command {
        ShortcutCommand::SelectTool(tool) => Some(ink_tool(*tool)),
        ShortcutCommand::UseLaser => Some(InkTool::Laser),
        _ => None,
    }
}

// Swift: InkTool ↔ AppCommand.selectTool 대응
fn ink_tool(tool: ShortcutTool) -> InkTool {
    match tool {
        ShortcutTool::Pen => InkTool::Pen,
        ShortcutTool::Highlighter => InkTool::Highlighter,
        ShortcutTool::Arrow => InkTool::Arrow,
        ShortcutTool::Rectangle => InkTool::Rectangle,
        ShortcutTool::Ellipse => InkTool::Ellipse,
        ShortcutTool::Text => InkTool::Text,
        ShortcutTool::Select => InkTool::Select,
        ShortcutTool::Cover => InkTool::Cover,
    }
}

// Swift: AppCommand.selectTool(_:) 입력 대응
fn shortcut_tool(tool: InkTool) -> Option<ShortcutTool> {
    match tool {
        InkTool::Pen => Some(ShortcutTool::Pen),
        InkTool::Highlighter => Some(ShortcutTool::Highlighter),
        InkTool::Arrow => Some(ShortcutTool::Arrow),
        InkTool::Rectangle => Some(ShortcutTool::Rectangle),
        InkTool::Ellipse => Some(ShortcutTool::Ellipse),
        InkTool::Text => Some(ShortcutTool::Text),
        InkTool::Select => Some(ShortcutTool::Select),
        InkTool::Cover => Some(ShortcutTool::Cover),
        InkTool::Laser | InkTool::Eraser => None,
    }
}

// Swift: FavoriteTool.inkTool
fn favorite_tool(favorite: &FavoriteTool) -> Option<InkTool> {
    match favorite.tool.as_str() {
        "pen" => Some(InkTool::Pen),
        "highlighter" => Some(InkTool::Highlighter),
        "laser" => Some(InkTool::Laser),
        "eraser" => Some(InkTool::Eraser),
        "arrow" => Some(InkTool::Arrow),
        "rectangle" => Some(InkTool::Rectangle),
        "ellipse" => Some(InkTool::Ellipse),
        "text" => Some(InkTool::Text),
        "select" => Some(InkTool::Select),
        "cover" => Some(InkTool::Cover),
        _ => None,
    }
}

// Swift: FavoriteTool.decodedEraserMode
fn favorite_eraser_mode(favorite: &FavoriteTool) -> Option<EraserMode> {
    favorite.eraser_mode.as_deref().and_then(parse_eraser_mode)
}

// Swift: EraserMode.init(rawValue:)
fn parse_eraser_mode(value: &str) -> Option<EraserMode> {
    match value {
        "partial" => Some(EraserMode::Partial),
        "stroke" => Some(EraserMode::Stroke),
        "area" => Some(EraserMode::Area),
        _ => None,
    }
}

// Swift: EraserAreaShape.init(rawValue:)
fn parse_eraser_area_shape(value: &str) -> Option<EraserAreaShape> {
    match value {
        "rectangle" => Some(EraserAreaShape::Rectangle),
        "lasso" => Some(EraserAreaShape::Lasso),
        _ => None,
    }
}

// Swift: InkTool.displayName
fn tool_name(tool: InkTool) -> &'static str {
    match tool {
        InkTool::Pen => "펜",
        InkTool::Highlighter => "형광펜",
        InkTool::Laser => "레이저",
        InkTool::Eraser => "지우개",
        InkTool::Arrow => "화살표",
        InkTool::Rectangle => "사각형",
        InkTool::Ellipse => "원",
        InkTool::Text => "텍스트",
        InkTool::Select => "선택",
        InkTool::Cover => "화면 가리개",
    }
}

// Swift: RadialPaletteController.formattedNumber(_:)
fn format_number(value: f64) -> String {
    if value.round() == value {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        radial_drag_threshold_reached, radial_palette_should_show, RadialPaletteSectorGeometry,
        RadialPaletteSectorPath,
    };
    use crate::settings::PaletteStyle;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn sector_path_conversion_table_uses_clockwise_screen_coordinates_and_dpi_scale() {
        let cases = [
            (
                RadialPaletteSectorGeometry {
                    inner_radius: 10.0,
                    outer_radius: 20.0,
                    start_angle: 0.0,
                    end_angle: FRAC_PI_2,
                },
                1.5,
                [100.0, 100.0],
                15.0,
                30.0,
                [100.0, 70.0],
                [130.0, 100.0],
                [115.0, 100.0],
                [100.0, 85.0],
            ),
            (
                RadialPaletteSectorGeometry {
                    inner_radius: 4.0,
                    outer_radius: 8.0,
                    start_angle: -FRAC_PI_2,
                    end_angle: 0.0,
                },
                2.0,
                [50.0, 40.0],
                8.0,
                16.0,
                [34.0, 40.0],
                [50.0, 24.0],
                [50.0, 32.0],
                [42.0, 40.0],
            ),
        ];
        for (
            geometry,
            scale,
            center,
            expected_inner,
            expected_outer,
            outer_start,
            outer_end,
            inner_end,
            inner_start,
        ) in cases
        {
            let path = RadialPaletteSectorPath::from_geometry(geometry, scale, center);
            assert_eq!(path.inner_radius_px, expected_inner);
            assert_eq!(path.outer_radius_px, expected_outer);
            assert_eq!(path.start_angle, geometry.start_angle);
            assert_eq!(path.end_angle, geometry.end_angle);
            assert_eq!(path.outer_start, outer_start);
            assert_eq!(path.outer_end, outer_end);
            assert_eq!(path.inner_end, inner_end);
            assert_eq!(path.inner_start, inner_start);
        }
    }

    #[test]
    fn drag_threshold_table_is_inclusive_at_four_dip_at_150_percent() {
        let cases = [
            (3.9 * 1.5, 0.0, false),
            (4.0 * 1.5, 0.0, true),
            (3.0 * 1.5, 2.5 * 1.5, false),
            (3.0 * 1.5, 3.0 * 1.5, true),
        ];
        for (dx, dy, expected) in cases {
            assert_eq!(radial_drag_threshold_reached(dx, dy, 1.5), expected);
        }
    }

    #[test]
    fn visibility_rule_table_requires_annotation_visibility_and_radial_style() {
        let cases = [
            (false, false, PaletteStyle::Linear, false),
            (false, false, PaletteStyle::Radial, false),
            (false, true, PaletteStyle::Linear, false),
            (false, true, PaletteStyle::Radial, false),
            (true, false, PaletteStyle::Linear, false),
            (true, false, PaletteStyle::Radial, false),
            (true, true, PaletteStyle::Linear, false),
            (true, true, PaletteStyle::Radial, true),
        ];
        for (annotation, visible, style, expected) in cases {
            assert_eq!(
                radial_palette_should_show(annotation, visible, style),
                expected
            );
        }
    }
}
