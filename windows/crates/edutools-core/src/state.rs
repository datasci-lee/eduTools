//! AppState.swift의 OS 독립 상태 전이와 설정 변경 요청.

use crate::board::{BoardKind, BoardPresentation};
use crate::eraser::{EraserAreaShape, EraserMode};
use crate::ink::{ArrowStyle, InkCommand, InkLayerState, InkStroke, InkTool, ToolMemorySlot};
use crate::settings::{
    AppSettings, FavoriteTool, FavoritesDefaults, SettingsColor, SettingsRanges,
};
use crate::zoom::ZoomSessionState;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

// Swift: PassThroughDefaults
pub struct PassThroughDefaults;

impl PassThroughDefaults {
    // Swift: PassThroughDefaults.initialClickTimeout
    pub const INITIAL_CLICK_TIMEOUT: f64 = 10.0;
    // Swift: PassThroughDefaults.interactionIdleTimeout
    pub const INTERACTION_IDLE_TIMEOUT: f64 = 3.0;
    // Swift: PassThroughDefaults.keyMaximumHoldDuration
    pub const KEY_MAXIMUM_HOLD_DURATION: f64 = 30.0;
}

// Swift: PassThroughModeSource
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassThroughModeSource {
    Timed,
    Key(u32),
}

// Swift: AppState pass-through timeout work items
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PassThroughDeadlines {
    pub idle: Option<f64>,
    pub maximum_hold: Option<f64>,
}

// Swift: DisplayDisconnectPolicy
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayDisconnectPolicy {
    Archive,
    MoveToPrimary,
    Delete,
}

impl DisplayDisconnectPolicy {
    // Swift: DisplayDisconnectPolicy.init(rawValue:)
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "archive" => Some(Self::Archive),
            "moveToPrimary" => Some(Self::MoveToPrimary),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }

    // Swift: DisplayDisconnectPolicy.rawValue
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::MoveToPrimary => "moveToPrimary",
            Self::Delete => "delete",
        }
    }
}

// Swift: AppState settingsStore.update failure paths
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    InvalidSetting { field: &'static str, value: String },
}

impl fmt::Display for StateError {
    // Swift: SettingsValidationError.errorDescription
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSetting { field, value } => {
                write!(formatter, "invalid setting for {field}: {value}")
            }
        }
    }
}

impl Error for StateError {}

// Swift: SettingsStore.update closures issued by AppState
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsMutation {
    SetToolColor {
        slot: ToolMemorySlot,
        color_index: i64,
    },
    SetToolSize {
        slot: ToolMemorySlot,
        size: f64,
    },
    SetLaserColor(SettingsColor),
    SetLaserTrailDuration(f64),
    SetFavorites(Vec<FavoriteTool>),
    SetShapeRecognition(bool),
    SetEraserMode(EraserMode),
    SetEraserDiameter(f64),
    SetEraserAreaShape(EraserAreaShape),
    SetDisplayDisconnectPolicy(DisplayDisconnectPolicy),
    SetHelpSharing(bool),
}

impl SettingsMutation {
    // Swift: SettingsStore.update(_:)
    pub fn apply(&self, settings: &mut AppSettings) {
        match self {
            Self::SetToolColor { slot, color_index } => {
                slot.setting_mut(&mut settings.tools.per_tool_memory)
                    .color_index = *color_index;
            }
            Self::SetToolSize { slot, size } => {
                slot.setting_mut(&mut settings.tools.per_tool_memory).size = *size;
            }
            Self::SetLaserColor(color) => settings.tools.laser_color = color.clone(),
            Self::SetLaserTrailDuration(duration) => {
                settings.tools.laser_trail_duration = *duration;
            }
            Self::SetFavorites(favorites) => settings.tools.favorites = favorites.clone(),
            Self::SetShapeRecognition(is_enabled) => {
                settings.tools.shape_recognition_enabled = *is_enabled;
            }
            Self::SetEraserMode(mode) => settings.eraser.mode = eraser_mode_name(*mode).to_owned(),
            Self::SetEraserDiameter(diameter) => settings.eraser.diameter = *diameter,
            Self::SetEraserAreaShape(shape) => {
                settings.eraser.area_shape = eraser_area_shape_name(*shape).to_owned();
            }
            Self::SetDisplayDisconnectPolicy(policy) => {
                settings.general.display_disconnect_policy = policy.as_str().to_owned();
            }
            Self::SetHelpSharing(is_enabled) => {
                settings.help.shows_help_in_sharing = *is_enabled;
            }
        }
    }
}

// Swift: AppState currentFavoriteSnapshot save result
#[derive(Clone, Debug, PartialEq)]
pub enum FavoriteSaveResult {
    Saved(SettingsMutation),
    UnsupportedTool,
    Duplicate,
    Full,
}

// Swift: AppState
#[derive(Clone, Debug, PartialEq)]
pub struct AppState {
    active_display_id: Option<String>,
    annotation_mode_enabled: bool,
    pass_through_mode_source: Option<PassThroughModeSource>,
    pass_through_deadlines: PassThroughDeadlines,
    current_tool: InkTool,
    last_color_tool: InkTool,
    previous_tool_before_eraser: InkTool,
    arrow_style: ArrowStyle,
    palette_visible: bool,
    presented_board: Option<BoardPresentation>,
    zoom_session: Option<ZoomSessionState>,
    selected_stroke_ids_by_display: BTreeMap<String, BTreeSet<String>>,
}

impl Default for AppState {
    // Swift: AppState property defaults
    fn default() -> Self {
        Self {
            active_display_id: None,
            annotation_mode_enabled: false,
            pass_through_mode_source: None,
            pass_through_deadlines: PassThroughDeadlines::default(),
            current_tool: InkTool::Pen,
            last_color_tool: InkTool::Pen,
            previous_tool_before_eraser: InkTool::Pen,
            arrow_style: ArrowStyle::Single,
            palette_visible: true,
            presented_board: None,
            zoom_session: None,
            selected_stroke_ids_by_display: BTreeMap::new(),
        }
    }
}

impl AppState {
    // Swift: AppState.init(defaults:settingsStore:)
    pub fn new() -> Self {
        Self::default()
    }

    // Swift: AppState.activeDisplayUUID
    pub fn active_display_id(&self) -> Option<&str> {
        self.active_display_id.as_deref()
    }

    // Swift: AppState.setActiveDisplay(_:)
    pub fn set_active_display(&mut self, display_id: impl Into<String>) {
        self.active_display_id = Some(display_id.into());
    }

    // Swift: AppState.selectedStrokeIdentifiers(on:)
    pub fn selected_stroke_ids(&self, display_id: &str) -> BTreeSet<String> {
        self.selected_stroke_ids_by_display
            .get(display_id)
            .cloned()
            .unwrap_or_default()
    }

    // Swift: AppState.setSelectedStrokeIdentifiers(_:on:)
    pub fn set_selected_stroke_ids(
        &mut self,
        display_id: &str,
        ids: BTreeSet<String>,
        strokes: &[InkStroke],
    ) -> bool {
        let available = strokes
            .iter()
            .map(|stroke| stroke.identifier.as_str())
            .collect::<BTreeSet<_>>();
        let selection = ids
            .into_iter()
            .filter(|identifier| available.contains(identifier.as_str()))
            .collect::<BTreeSet<_>>();
        if self.selected_stroke_ids(display_id) == selection {
            return false;
        }
        if selection.is_empty() {
            self.selected_stroke_ids_by_display.remove(display_id);
        } else {
            self.selected_stroke_ids_by_display
                .insert(display_id.to_owned(), selection);
        }
        true
    }

    // Swift: AppState.clearSelection(on:)
    pub fn clear_selection(&mut self, display_id: &str) -> bool {
        self.selected_stroke_ids_by_display
            .remove(display_id)
            .is_some_and(|selection| !selection.is_empty())
    }

    // Swift: AppState.deleteSelection(on:)
    pub fn delete_selection(&mut self, display_id: &str, layer: &mut InkLayerState) -> bool {
        self.reconcile_selection_after_ink_change(display_id, layer.strokes());
        let selected = self.selected_stroke_ids(display_id);
        if selected.is_empty() {
            return false;
        }
        let before = layer.strokes().to_vec();
        let after = before
            .iter()
            .filter(|stroke| !selected.contains(&stroke.identifier))
            .cloned()
            .collect::<Vec<_>>();
        self.clear_selection(display_id);
        self.execute_ink_command(
            display_id,
            layer,
            InkCommand::ReplaceStrokes { before, after },
        );
        true
    }

    // Every ink-list mutation goes through this wrapper so selection never keeps a missing id.
    pub fn execute_ink_command(
        &mut self,
        display_id: &str,
        layer: &mut InkLayerState,
        command: InkCommand,
    ) {
        layer.execute(command);
        self.reconcile_selection_after_ink_change(display_id, layer.strokes());
    }

    pub fn undo_ink(&mut self, display_id: &str, layer: &mut InkLayerState) -> bool {
        let changed = layer.undo();
        if changed {
            self.reconcile_selection_after_ink_change(display_id, layer.strokes());
        }
        changed
    }

    pub fn redo_ink(&mut self, display_id: &str, layer: &mut InkLayerState) -> bool {
        let changed = layer.redo();
        if changed {
            self.reconcile_selection_after_ink_change(display_id, layer.strokes());
        }
        changed
    }

    fn reconcile_selection_after_ink_change(&mut self, display_id: &str, strokes: &[InkStroke]) {
        let selected = self.selected_stroke_ids(display_id);
        self.set_selected_stroke_ids(display_id, selected, strokes);
    }

    // Swift: AppState.isAnnotationModeEnabled
    pub const fn is_annotation_mode_enabled(&self) -> bool {
        self.annotation_mode_enabled
    }

    // Swift: AppState.toggleAnnotationMode()
    pub fn toggle_annotation_mode(&mut self) {
        self.set_annotation_mode_enabled(!self.annotation_mode_enabled);
    }

    // Swift: AppState.setAnnotationModeEnabled(_:)
    pub fn set_annotation_mode_enabled(&mut self, is_enabled: bool) {
        if self.annotation_mode_enabled == is_enabled {
            return;
        }
        self.annotation_mode_enabled = is_enabled;
        if !is_enabled {
            self.end_pass_through_mode();
            self.presented_board = None;
        }
    }

    // Swift: AppState.passThroughModeSource
    pub const fn pass_through_mode_source(&self) -> Option<PassThroughModeSource> {
        self.pass_through_mode_source
    }

    // Swift: AppState.isPassThroughModeEnabled
    pub const fn is_pass_through_mode_enabled(&self) -> bool {
        self.pass_through_mode_source.is_some()
    }

    // Swift: AppState pass-through timeout work items
    pub const fn pass_through_deadlines(&self) -> PassThroughDeadlines {
        self.pass_through_deadlines
    }

    // Swift: AppState.togglePassThroughMode()
    pub fn toggle_pass_through_mode(&mut self, now: f64, monitor_available: bool) -> bool {
        match self.pass_through_mode_source {
            None => self.set_pass_through_mode_enabled(true, now, monitor_available),
            Some(PassThroughModeSource::Timed) => {
                self.end_pass_through_mode();
                true
            }
            Some(PassThroughModeSource::Key(_)) => false,
        }
    }

    // Swift: AppState.setPassThroughModeEnabled(_:)
    pub fn set_pass_through_mode_enabled(
        &mut self,
        is_enabled: bool,
        now: f64,
        monitor_available: bool,
    ) -> bool {
        if !is_enabled {
            let changed = self.pass_through_mode_source.is_some();
            self.end_pass_through_mode();
            return changed;
        }
        if self.pass_through_mode_source.is_some()
            || !self.annotation_mode_enabled
            || self.presented_board.is_some()
            || !monitor_available
        {
            return false;
        }
        self.pass_through_mode_source = Some(PassThroughModeSource::Timed);
        self.pass_through_deadlines = PassThroughDeadlines {
            idle: Some(now + PassThroughDefaults::INITIAL_CLICK_TIMEOUT),
            maximum_hold: None,
        };
        true
    }

    // Swift: AppState.beginKeyPassThrough(keyCode:)
    pub fn begin_key_pass_through(&mut self, key_code: u32, now: f64) -> bool {
        if !self.annotation_mode_enabled || self.presented_board.is_some() {
            return false;
        }
        if self.pass_through_mode_source == Some(PassThroughModeSource::Key(key_code)) {
            return false;
        }
        self.pass_through_mode_source = Some(PassThroughModeSource::Key(key_code));
        self.pass_through_deadlines = PassThroughDeadlines {
            idle: None,
            maximum_hold: Some(now + PassThroughDefaults::KEY_MAXIMUM_HOLD_DURATION),
        };
        true
    }

    // Swift: AppState.endKeyPassThrough(keyCode:)
    pub fn end_key_pass_through(&mut self, key_code: u32) -> bool {
        if self.pass_through_mode_source != Some(PassThroughModeSource::Key(key_code)) {
            return false;
        }
        self.end_pass_through_mode();
        true
    }

    // Swift: AppState.passThroughInteractionDidOccur()
    pub fn record_pass_through_interaction(&mut self, now: f64) -> bool {
        if self.pass_through_mode_source.is_none() {
            return false;
        }
        self.pass_through_deadlines.idle =
            Some(now + PassThroughDefaults::INTERACTION_IDLE_TIMEOUT);
        true
    }

    // Swift: AppState.passThroughTimeoutDidFire()
    pub fn expire_pass_through(&mut self, now: f64, mouse_buttons_pressed: bool) -> bool {
        if self.pass_through_mode_source.is_none() {
            return false;
        }
        if self
            .pass_through_deadlines
            .maximum_hold
            .is_some_and(|deadline| now >= deadline)
        {
            self.end_pass_through_mode();
            return true;
        }
        if !self
            .pass_through_deadlines
            .idle
            .is_some_and(|deadline| now >= deadline)
        {
            return false;
        }
        if mouse_buttons_pressed {
            self.pass_through_deadlines.idle =
                Some(now + PassThroughDefaults::INTERACTION_IDLE_TIMEOUT);
            return false;
        }
        self.end_pass_through_mode();
        true
    }

    // Swift: AppState.endPassThroughMode()
    pub fn end_pass_through_mode(&mut self) {
        self.pass_through_mode_source = None;
        self.pass_through_deadlines = PassThroughDeadlines::default();
    }

    // Swift: AppState.currentTool
    pub const fn current_tool(&self) -> InkTool {
        self.current_tool
    }

    // Swift: AppState.lastColorTool
    pub const fn last_color_tool(&self) -> InkTool {
        self.last_color_tool
    }

    // Swift: AppState.previousToolBeforeEraser
    pub const fn previous_tool_before_eraser(&self) -> InkTool {
        self.previous_tool_before_eraser
    }

    // Swift: AppState.arrowStyle
    pub const fn arrow_style(&self) -> ArrowStyle {
        self.arrow_style
    }

    // Swift: AppState.selectTool(_:)
    pub fn select_tool(&mut self, tool: InkTool) {
        if tool == InkTool::Arrow && self.current_tool == InkTool::Arrow {
            self.cycle_arrow_style();
            return;
        }
        if self.current_tool == tool {
            return;
        }
        let old_tool = self.current_tool;
        self.current_tool = tool;
        if old_tool != InkTool::Eraser && tool == InkTool::Eraser {
            self.previous_tool_before_eraser = old_tool;
        }
        if tool.memory_slot().is_some() {
            self.last_color_tool = tool;
        }
    }

    // Swift: ArrowStyle.next
    pub fn cycle_arrow_style(&mut self) {
        self.arrow_style = next_arrow_style(self.arrow_style);
    }

    // Swift: AppState.setArrowStyle(_:)
    pub fn set_arrow_style(&mut self, style: ArrowStyle) {
        self.arrow_style = style;
    }

    // Swift: AppState.selectOrCycleEraser()
    pub fn select_or_cycle_eraser(
        &mut self,
        settings: &AppSettings,
    ) -> Result<Option<SettingsMutation>, StateError> {
        if self.current_tool != InkTool::Eraser {
            self.select_tool(InkTool::Eraser);
            return Ok(None);
        }
        let mode = parse_eraser_mode(&settings.eraser.mode)?;
        Ok(Some(SettingsMutation::SetEraserMode(next_eraser_mode(
            mode,
        ))))
    }

    // Swift: AppState.toggleEraserTool()
    pub fn toggle_eraser_tool(&mut self) {
        if self.current_tool == InkTool::Eraser {
            self.select_tool(self.previous_tool_before_eraser);
        } else {
            self.select_tool(InkTool::Eraser);
        }
    }

    // Swift: AppState.toggleEraserAreaShape()
    pub fn toggle_eraser_area_shape(
        &self,
        settings: &AppSettings,
    ) -> Result<SettingsMutation, StateError> {
        let shape = parse_eraser_area_shape(&settings.eraser.area_shape)?;
        Ok(SettingsMutation::SetEraserAreaShape(shape.next()))
    }

    // Swift: AppState.toggleShapeRecognition()
    pub fn toggle_shape_recognition(&self, settings: &AppSettings) -> SettingsMutation {
        SettingsMutation::SetShapeRecognition(!settings.tools.shape_recognition_enabled)
    }

    // Swift: AppState.selectColor(at:)
    pub fn select_color(&self, index: usize, settings: &AppSettings) -> Option<SettingsMutation> {
        let color = settings.tools.colors.get(index)?;
        if self.current_tool == InkTool::Laser {
            return Some(SettingsMutation::SetLaserColor(color.clone()));
        }
        self.current_tool
            .memory_slot()
            .map(|slot| SettingsMutation::SetToolColor {
                slot,
                color_index: index as i64,
            })
    }

    // Swift: AppState.selectedColorIndex
    pub fn selected_color_index(&self, settings: &AppSettings) -> i64 {
        let slot = self
            .current_tool
            .memory_slot()
            .or_else(|| self.last_color_tool.memory_slot())
            .expect("lastColorTool always has a memory slot");
        slot.setting(&settings.tools.per_tool_memory).color_index
    }

    // Swift: AppState.width(for:)
    pub fn width_for(&self, tool: InkTool, settings: &AppSettings) -> Option<f64> {
        tool.memory_slot()
            .map(|slot| slot.setting(&settings.tools.per_tool_memory).size)
    }

    // Swift: AppState.setWidth(_:for:)
    pub fn set_width(&self, tool: InkTool, width: f64) -> Option<SettingsMutation> {
        let slot = tool.memory_slot()?;
        let range = slot.size_range();
        Some(SettingsMutation::SetToolSize {
            slot,
            size: width.clamp(*range.start(), *range.end()),
        })
    }

    // Swift: AppState.adjustCurrentToolWidth(by:)
    pub fn adjust_current_tool_width(
        &self,
        delta: f64,
        settings: &AppSettings,
    ) -> Option<SettingsMutation> {
        if let Some(slot) = self.current_tool.memory_slot() {
            let current = slot.setting(&settings.tools.per_tool_memory).size;
            return self.set_width(self.current_tool, current + delta);
        }
        if self.current_tool == InkTool::Eraser {
            let range = SettingsRanges::ERASER_DIAMETER;
            return Some(SettingsMutation::SetEraserDiameter(
                (settings.eraser.diameter + delta * 2.0).clamp(*range.start(), *range.end()),
            ));
        }
        None
    }

    // Swift: AppState.favoriteSnapshot(for:)
    pub fn favorite_snapshot(
        &self,
        tool: InkTool,
        settings: &AppSettings,
    ) -> Result<Option<FavoriteTool>, StateError> {
        if let Some(slot) = tool.memory_slot() {
            let memory = slot.setting(&settings.tools.per_tool_memory);
            return Ok(Some(FavoriteTool {
                tool: tool_name(tool).to_owned(),
                color_index: Some(memory.color_index),
                size: memory.size,
                arrow_style: (tool == InkTool::Arrow)
                    .then(|| arrow_style_name(self.arrow_style).to_owned()),
                eraser_mode: None,
                eraser_area_shape: None,
            }));
        }
        match tool {
            InkTool::Eraser => {
                let mode = parse_eraser_mode(&settings.eraser.mode)?;
                let shape = parse_eraser_area_shape(&settings.eraser.area_shape)?;
                Ok(Some(FavoriteTool {
                    tool: tool_name(tool).to_owned(),
                    color_index: None,
                    size: settings.eraser.diameter,
                    arrow_style: None,
                    eraser_mode: Some(eraser_mode_name(mode).to_owned()),
                    eraser_area_shape: Some(eraser_area_shape_name(shape).to_owned()),
                }))
            }
            InkTool::Laser => Ok(settings
                .tools
                .colors
                .iter()
                .position(|color| color == &settings.tools.laser_color)
                .map(|index| FavoriteTool {
                    tool: tool_name(tool).to_owned(),
                    color_index: Some(index as i64),
                    size: settings.tools.laser_trail_duration,
                    arrow_style: None,
                    eraser_mode: None,
                    eraser_area_shape: None,
                })),
            InkTool::Select | InkTool::Cover => Ok(None),
            InkTool::Pen
            | InkTool::Highlighter
            | InkTool::Arrow
            | InkTool::Rectangle
            | InkTool::Ellipse
            | InkTool::Text => unreachable!("memory-slot tools returned above"),
        }
    }

    // Swift: AppState.currentFavoriteSnapshot()
    pub fn current_favorite_snapshot(
        &self,
        settings: &AppSettings,
    ) -> Result<Option<FavoriteTool>, StateError> {
        self.favorite_snapshot(self.current_tool, settings)
    }

    // Swift: AppState favorites append path using currentFavoriteSnapshot()
    pub fn save_current_favorite(
        &self,
        settings: &AppSettings,
    ) -> Result<FavoriteSaveResult, StateError> {
        let Some(favorite) = self.current_favorite_snapshot(settings)? else {
            return Ok(FavoriteSaveResult::UnsupportedTool);
        };
        if settings.tools.favorites.contains(&favorite) {
            return Ok(FavoriteSaveResult::Duplicate);
        }
        if settings.tools.favorites.len() >= FavoritesDefaults::MAXIMUM_COUNT {
            return Ok(FavoriteSaveResult::Full);
        }
        let mut favorites = settings.tools.favorites.clone();
        favorites.push(favorite);
        Ok(FavoriteSaveResult::Saved(SettingsMutation::SetFavorites(
            favorites,
        )))
    }

    // Swift: AppState.applyFavorite(_:)
    pub fn apply_favorite(
        &mut self,
        favorite: &FavoriteTool,
        settings: &AppSettings,
    ) -> Result<Option<Vec<SettingsMutation>>, StateError> {
        if !favorite.is_valid(settings.tools.colors.len()) {
            return Ok(None);
        }
        let Some(tool) = parse_tool(&favorite.tool) else {
            return Ok(None);
        };

        let arrow_style = favorite
            .arrow_style
            .as_deref()
            .map(parse_arrow_style)
            .transpose()?;
        let eraser_mode = favorite
            .eraser_mode
            .as_deref()
            .map(parse_eraser_mode)
            .transpose()?;
        let eraser_shape = favorite
            .eraser_area_shape
            .as_deref()
            .map(parse_eraser_area_shape)
            .transpose()?;

        self.select_tool(tool);
        let mut mutations = Vec::new();
        if let Some(color_index) = favorite.color_index {
            let index = usize::try_from(color_index).expect("validated favorite color index");
            if let Some(mutation) = self.select_color(index, settings) {
                mutations.push(mutation);
            }
        }
        if let Some(slot) = tool.memory_slot() {
            mutations.push(SettingsMutation::SetToolSize {
                slot,
                size: favorite.size,
            });
        } else if tool == InkTool::Eraser {
            mutations.push(SettingsMutation::SetEraserDiameter(favorite.size));
        } else if tool == InkTool::Laser {
            mutations.push(SettingsMutation::SetLaserTrailDuration(favorite.size));
        }

        if let Some(style) = arrow_style {
            self.set_arrow_style(style);
        }
        if let Some(mode) = eraser_mode {
            mutations.push(SettingsMutation::SetEraserMode(mode));
        }
        if let Some(shape) = eraser_shape {
            mutations.push(SettingsMutation::SetEraserAreaShape(shape));
        }
        Ok(Some(mutations))
    }

    // Swift: AppState.setDisplayDisconnectPolicy(_:)
    pub const fn set_display_disconnect_policy(
        policy: DisplayDisconnectPolicy,
    ) -> SettingsMutation {
        SettingsMutation::SetDisplayDisconnectPolicy(policy)
    }

    // Swift: AppState.displayDisconnectPolicy
    pub fn display_disconnect_policy(
        settings: &AppSettings,
    ) -> Result<DisplayDisconnectPolicy, StateError> {
        DisplayDisconnectPolicy::parse(&settings.general.display_disconnect_policy).ok_or_else(
            || StateError::InvalidSetting {
                field: "general.displayDisconnectPolicy",
                value: settings.general.display_disconnect_policy.clone(),
            },
        )
    }

    // Swift: AppState.isPaletteVisible
    pub const fn is_palette_visible(&self) -> bool {
        self.palette_visible
    }

    // Swift: AppState.togglePaletteVisibility()
    pub fn toggle_palette_visibility(&mut self) {
        self.palette_visible = !self.palette_visible;
    }

    // Swift: AppState.presentedBoard
    pub fn presented_board(&self) -> Option<&BoardPresentation> {
        self.presented_board.as_ref()
    }

    // Swift: AppState.toggleBoard(_:on:)
    pub fn toggle_board(&mut self, kind: BoardKind, display_id: impl Into<String>) {
        let display_id = display_id.into();
        self.active_display_id = Some(display_id.clone());
        if self.presented_board.as_ref().map(|board| board.kind) == Some(kind) {
            self.presented_board = None;
            return;
        }
        self.end_pass_through_mode();
        self.presented_board = Some(BoardPresentation { kind, display_id });
    }

    // Swift: AppState.dismissBoard()
    pub fn dismiss_board(&mut self) {
        self.presented_board = None;
    }

    // Swift: AppState.zoomSession
    pub fn zoom_session(&self) -> Option<&ZoomSessionState> {
        self.zoom_session.as_ref()
    }

    // Swift: AppState.beginZoom(on:screenSize:center:)
    pub fn set_zoom_session(&mut self, session: ZoomSessionState) {
        self.active_display_id = Some(session.display_id.clone());
        self.zoom_session = Some(session);
    }

    // Swift: AppState.endZoom()
    pub fn end_zoom(&mut self) {
        self.zoom_session = None;
    }

    // Swift: AppState.activateLaserShortcut()
    pub fn activate_laser_shortcut(&mut self) {
        if self.annotation_mode_enabled && self.current_tool == InkTool::Laser {
            self.set_annotation_mode_enabled(false);
        } else {
            self.select_tool(InkTool::Laser);
            self.set_annotation_mode_enabled(true);
        }
    }
}

// Swift: ArrowStyle.next
fn next_arrow_style(style: ArrowStyle) -> ArrowStyle {
    match style {
        ArrowStyle::Single => ArrowStyle::Double,
        ArrowStyle::Double => ArrowStyle::Single,
    }
}

// Swift: EraserMode.next
fn next_eraser_mode(mode: EraserMode) -> EraserMode {
    match mode {
        EraserMode::Partial => EraserMode::Stroke,
        EraserMode::Stroke => EraserMode::Area,
        EraserMode::Area => EraserMode::Partial,
    }
}

// Swift: EraserMode.init(rawValue:)
fn parse_eraser_mode(value: &str) -> Result<EraserMode, StateError> {
    match value {
        "partial" => Ok(EraserMode::Partial),
        "stroke" => Ok(EraserMode::Stroke),
        "area" => Ok(EraserMode::Area),
        _ => Err(StateError::InvalidSetting {
            field: "eraser.mode",
            value: value.to_owned(),
        }),
    }
}

// Swift: EraserMode.rawValue
const fn eraser_mode_name(mode: EraserMode) -> &'static str {
    match mode {
        EraserMode::Partial => "partial",
        EraserMode::Stroke => "stroke",
        EraserMode::Area => "area",
    }
}

// Swift: EraserAreaShape.init(rawValue:)
fn parse_eraser_area_shape(value: &str) -> Result<EraserAreaShape, StateError> {
    match value {
        "rectangle" => Ok(EraserAreaShape::Rectangle),
        "lasso" => Ok(EraserAreaShape::Lasso),
        _ => Err(StateError::InvalidSetting {
            field: "eraser.areaShape",
            value: value.to_owned(),
        }),
    }
}

// Swift: EraserAreaShape.rawValue
const fn eraser_area_shape_name(shape: EraserAreaShape) -> &'static str {
    match shape {
        EraserAreaShape::Rectangle => "rectangle",
        EraserAreaShape::Lasso => "lasso",
    }
}

// Swift: ArrowStyle.init(rawValue:)
fn parse_arrow_style(value: &str) -> Result<ArrowStyle, StateError> {
    match value {
        "single" => Ok(ArrowStyle::Single),
        "double" => Ok(ArrowStyle::Double),
        _ => Err(StateError::InvalidSetting {
            field: "favorite.arrowStyle",
            value: value.to_owned(),
        }),
    }
}

// Swift: ArrowStyle.rawValue
const fn arrow_style_name(style: ArrowStyle) -> &'static str {
    match style {
        ArrowStyle::Single => "single",
        ArrowStyle::Double => "double",
    }
}

// Swift: InkTool.init(rawValue:)
fn parse_tool(value: &str) -> Option<InkTool> {
    match value {
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

// Swift: InkTool.rawValue
const fn tool_name(tool: InkTool) -> &'static str {
    match tool {
        InkTool::Pen => "pen",
        InkTool::Highlighter => "highlighter",
        InkTool::Laser => "laser",
        InkTool::Eraser => "eraser",
        InkTool::Arrow => "arrow",
        InkTool::Rectangle => "rectangle",
        InkTool::Ellipse => "ellipse",
        InkTool::Text => "text",
        InkTool::Select => "select",
        InkTool::Cover => "cover",
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use crate::ink::{
        InkCommand, InkLayerState, InkPoint, InkStroke, InkStrokeContent, InkStrokeTool, Point,
    };
    use std::collections::BTreeSet;

    #[test]
    fn selection_intersection_is_preserved_after_undo_redo_and_delete_table() {
        let display = "display-a";
        let first = stroke("first");
        let second = stroke("second");
        let mut layer = InkLayerState::new();
        let mut state = AppState::new();
        state.execute_ink_command(display, &mut layer, InkCommand::AddStroke(first.clone()));
        state.execute_ink_command(display, &mut layer, InkCommand::AddStroke(second.clone()));
        state.set_selected_stroke_ids(
            display,
            ids(&["first", "second", "missing"]),
            layer.strokes(),
        );
        assert_eq!(
            state.selected_stroke_ids(display),
            ids(&["first", "second"]),
            "set intersects with current strokes"
        );

        let mut observations = Vec::new();
        state.execute_ink_command(
            display,
            &mut layer,
            InkCommand::ReplaceStrokes {
                before: vec![first.clone(), second.clone()],
                after: vec![first.clone()],
            },
        );
        observations.push(snapshot("replace", &state, display, &layer));
        assert!(state.undo_ink(display, &mut layer), "undo");
        observations.push(snapshot("undo", &state, display, &layer));
        assert!(state.redo_ink(display, &mut layer), "redo");
        observations.push(snapshot("redo", &state, display, &layer));

        assert!(state.delete_selection(display, &mut layer), "delete");
        observations.push(snapshot("delete", &state, display, &layer));
        assert!(state.undo_ink(display, &mut layer), "undo delete");
        observations.push(snapshot("undo-delete", &state, display, &layer));

        let expected = [
            ("replace", vec!["first"], vec!["first"]),
            ("undo", vec!["first"], vec!["first", "second"]),
            ("redo", vec!["first"], vec!["first"]),
            ("delete", Vec::new(), Vec::new()),
            ("undo-delete", Vec::new(), vec!["first"]),
        ];
        for ((name, selected, strokes), (expected_name, expected_selected, expected_strokes)) in
            observations.into_iter().zip(expected)
        {
            assert_eq!(name, expected_name);
            assert_eq!(selected, expected_selected, "{name} selection");
            assert_eq!(strokes, expected_strokes, "{name} strokes");
        }
    }

    fn ids(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn snapshot<'a>(
        name: &'a str,
        state: &AppState,
        display: &str,
        layer: &InkLayerState,
    ) -> (&'a str, Vec<String>, Vec<String>) {
        (
            name,
            state.selected_stroke_ids(display).into_iter().collect(),
            layer
                .strokes()
                .iter()
                .map(|stroke| stroke.identifier.clone())
                .collect(),
        )
    }

    fn stroke(identifier: &str) -> InkStroke {
        InkStroke::new(
            identifier,
            InkStrokeTool::Pen,
            [0.1, 0.2, 0.3, 1.0],
            4.0,
            false,
            vec![InkPoint::new(Point { x: 0.5, y: 0.5 }, 1.0, 0.0)],
            InkStrokeContent::Freehand,
        )
    }
}
