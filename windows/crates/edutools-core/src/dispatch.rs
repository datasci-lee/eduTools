//! ShortcutCommand의 OS 독립 라우팅과 keyDown/keyUp 연결.

use crate::board::{BoardKind, BoardPresentation};
use crate::ink::InkTool;
use crate::settings::{AppSettings, PaletteStyle};
use crate::shortcut::{
    ShortcutBoard, ShortcutCommand, ShortcutDefinition, ShortcutModifiers, ShortcutScope,
    ShortcutTool,
};
use crate::state::{AppState, SettingsMutation, StateError};
use std::collections::HashMap;

// Swift: AppCommandDispatcher stepBack branches
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepBackResult {
    PassThroughMode,
    RadialPalette,
    Help,
    Selection,
    Tool,
    AnnotationMode,
}

// Swift: AppCommand previousBoardPage/nextBoardPage
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardPageDirection {
    Previous,
    Next,
}

// Swift: AppCommandDispatcher controller calls and AppState observer-visible changes
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    FinishAllActiveInteractions,
    AnnotationModeChanged(bool),
    PassThroughModeChanged(bool),
    ToolChanged(InkTool),
    PaletteVisibilityChanged(bool),
    BoardChanged(Option<BoardPresentation>),
    SettingsMutation(SettingsMutation),
    ToggleZoom,
    ToggleSpotlight,
    ToggleCursorHighlight,
    ToggleKeystrokeDisplay,
    ApplyDisplayPreset(usize),
    ToggleLessonTimer,
    ShowBreakTimerSetup,
    FocusBoardPanel(String),
    BoardTargetMissing,
    MoveBoardPage {
        direction: BoardPageDirection,
        display_id: String,
    },
    AdjustSpotlightWidth(i32),
    Undo(String),
    Redo(String),
    ClearAll,
    DeleteSelection(String),
    ClearSelection(String),
    SaveCurrentContent,
    CopyCurrentImage,
    ToggleRadialPalette,
    CollapseRadialPalette,
    ShowHelp,
    CloseHelp,
    OpenSettings,
    AdjustZoom(i32),
    ResetZoom,
    ToggleZoomFreeze,
    StopZoom,
    BeginZoomPreview,
    FinishZoomPreview,
    CycleZoomFollowMode,
    HelpShortcutPressed(u32),
    HelpShortcutReleased,
    StepBack(StepBackResult),
}

// Swift: AppCommandDispatcher.execute context from controllers
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DispatchContext<'a> {
    pub scope: ShortcutScope,
    pub display_id: Option<&'a str>,
    pub board_display_id: Option<&'a str>,
    pub radial_palette_expanded: bool,
    pub help_visible: bool,
    pub has_selection: bool,
    pub spotlight_adjusted_width: bool,
    pub now: f64,
    pub pass_through_monitor_available: bool,
}

impl Default for DispatchContext<'_> {
    // Swift: AppCommandDispatcher.execute default arguments
    fn default() -> Self {
        Self {
            scope: ShortcutScope::Annotation,
            display_id: None,
            board_display_id: None,
            radial_palette_expanded: false,
            help_visible: false,
            has_selection: false,
            spotlight_adjusted_width: false,
            now: 0.0,
            pass_through_monitor_available: true,
        }
    }
}

// Swift: InkView pressed shortcut definitions
#[derive(Clone, Debug, Default)]
pub struct PressedShortcuts {
    pressed: HashMap<u32, ShortcutDefinition>,
}

impl PressedShortcuts {
    // Swift: InkView pressedShortcutDefinitionsByKeyCode
    pub fn new() -> Self {
        Self::default()
    }

    // Swift: InkView pressedShortcutDefinitionsByKeyCode[keyCode] != nil
    pub fn is_pressed(&self, key_code: u32) -> bool {
        self.pressed.contains_key(&key_code)
    }

    // Swift: InkView pressedShortcutDefinitionsByKeyCode.count
    pub fn len(&self) -> usize {
        self.pressed.len()
    }

    // Swift: InkView pressedShortcutDefinitionsByKeyCode.isEmpty
    pub fn is_empty(&self) -> bool {
        self.pressed.is_empty()
    }

    // Swift: InkView keyDown(with:)
    pub fn key_down(
        &mut self,
        key_code: u32,
        definition: &ShortcutDefinition,
        state: &mut AppState,
        settings: &AppSettings,
        context: DispatchContext<'_>,
    ) -> Result<KeyDispatch, StateError> {
        if self.pressed.contains_key(&key_code) {
            return Ok(KeyDispatch {
                consumed: true,
                effects: Vec::new(),
            });
        }
        if !should_consume(&definition.command, settings) {
            return Ok(KeyDispatch {
                consumed: false,
                effects: Vec::new(),
            });
        }
        self.pressed.insert(key_code, definition.clone());

        let effects = match definition.command {
            ShortcutCommand::ShowHelp => vec![Effect::HelpShortcutPressed(key_code)],
            ShortcutCommand::TogglePassThroughMode => {
                let is_unmodified_annotation_binding =
                    definition.binding(context.scope).is_some_and(|binding| {
                        context.scope == ShortcutScope::Annotation
                            && binding.modifiers == ShortcutModifiers::empty()
                    });
                if !is_unmodified_annotation_binding {
                    Vec::new()
                } else {
                    let mut effects = vec![Effect::FinishAllActiveInteractions];
                    if state.begin_key_pass_through(key_code, context.now) {
                        effects.push(Effect::PassThroughModeChanged(true));
                    }
                    effects
                }
            }
            _ => dispatch(&definition.command, state, settings, context)?,
        };
        Ok(KeyDispatch {
            consumed: true,
            effects,
        })
    }

    // Swift: InkView keyUp(with:)
    pub fn key_up(&mut self, key_code: u32, state: &mut AppState) -> KeyDispatch {
        let Some(definition) = self.pressed.remove(&key_code) else {
            return KeyDispatch {
                consumed: false,
                effects: Vec::new(),
            };
        };
        let effects = match definition.command {
            ShortcutCommand::ShowHelp => vec![Effect::HelpShortcutReleased],
            ShortcutCommand::BeginZoomPreview => vec![Effect::FinishZoomPreview],
            ShortcutCommand::TogglePassThroughMode => {
                if state.end_key_pass_through(key_code) {
                    vec![Effect::PassThroughModeChanged(false)]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
        KeyDispatch {
            consumed: true,
            effects,
        }
    }
}

// Swift: AppCommandDispatcher.keyDown/keyUp handling result
#[derive(Clone, Debug, PartialEq)]
pub struct KeyDispatch {
    pub consumed: bool,
    pub effects: Vec<Effect>,
}

// Swift: AppCommandDispatcher.supports(_:)
pub const fn supports(command: &ShortcutCommand) -> bool {
    match command {
        ShortcutCommand::ToggleAnnotationMode
        | ShortcutCommand::TogglePassThroughMode
        | ShortcutCommand::ToggleZoom
        | ShortcutCommand::ToggleSpotlight
        | ShortcutCommand::ToggleCursorHighlight
        | ShortcutCommand::ToggleKeystrokeDisplay
        | ShortcutCommand::ToggleHelpSharing
        | ShortcutCommand::ApplyDisplayPreset(_)
        | ShortcutCommand::ToggleLessonTimer
        | ShortcutCommand::ShowBreakTimerSetup
        | ShortcutCommand::ToggleBoard(_)
        | ShortcutCommand::PreviousBoardPage
        | ShortcutCommand::NextBoardPage
        | ShortcutCommand::SelectTool(_)
        | ShortcutCommand::UseLaser
        | ShortcutCommand::SelectOrCycleEraser
        | ShortcutCommand::ToggleEraserTool
        | ShortcutCommand::ToggleEraserAreaShape
        | ShortcutCommand::ToggleShapeRecognition
        | ShortcutCommand::SelectColor(_)
        | ShortcutCommand::AdjustWidth(_)
        | ShortcutCommand::Undo
        | ShortcutCommand::Redo
        | ShortcutCommand::ClearAll
        | ShortcutCommand::DeleteSelection
        | ShortcutCommand::SaveCurrentContent
        | ShortcutCommand::CopyCurrentImage
        | ShortcutCommand::TogglePalette
        | ShortcutCommand::ToggleRadialPalette
        | ShortcutCommand::StepBack
        | ShortcutCommand::ShowHelp
        | ShortcutCommand::AdjustZoom(_)
        | ShortcutCommand::ResetZoom
        | ShortcutCommand::ToggleZoomFreeze
        | ShortcutCommand::StopZoom
        | ShortcutCommand::BeginZoomPreview
        | ShortcutCommand::CycleZoomFollowMode => true,
    }
}

// Swift: AppCommandDispatcher.shouldConsume(_:)
pub fn should_consume(command: &ShortcutCommand, settings: &AppSettings) -> bool {
    match command {
        ShortcutCommand::ToggleRadialPalette => {
            settings.tools.palette_style == PaletteStyle::Radial
        }
        _ => true,
    }
}

// Swift: AppCommandDispatcher.execute(_:scope:displayUUID:)
pub fn dispatch(
    command: &ShortcutCommand,
    state: &mut AppState,
    settings: &AppSettings,
    context: DispatchContext<'_>,
) -> Result<Vec<Effect>, StateError> {
    let effects = match command {
        ShortcutCommand::ToggleAnnotationMode => {
            state.toggle_annotation_mode();
            vec![
                Effect::FinishAllActiveInteractions,
                Effect::AnnotationModeChanged(state.is_annotation_mode_enabled()),
            ]
        }
        ShortcutCommand::TogglePassThroughMode => {
            let was_enabled = state.is_pass_through_mode_enabled();
            state.toggle_pass_through_mode(context.now, context.pass_through_monitor_available);
            let mut effects = vec![Effect::FinishAllActiveInteractions];
            let is_enabled = state.is_pass_through_mode_enabled();
            if was_enabled != is_enabled {
                effects.push(Effect::PassThroughModeChanged(is_enabled));
            }
            effects
        }
        ShortcutCommand::ToggleZoom => vec![Effect::ToggleZoom],
        ShortcutCommand::ToggleSpotlight => vec![Effect::ToggleSpotlight],
        ShortcutCommand::ToggleCursorHighlight => vec![Effect::ToggleCursorHighlight],
        ShortcutCommand::ToggleKeystrokeDisplay => vec![Effect::ToggleKeystrokeDisplay],
        ShortcutCommand::ToggleHelpSharing => {
            vec![Effect::SettingsMutation(SettingsMutation::SetHelpSharing(
                !settings.help.shows_help_in_sharing,
            ))]
        }
        ShortcutCommand::ApplyDisplayPreset(index) => {
            vec![Effect::ApplyDisplayPreset(*index)]
        }
        ShortcutCommand::ToggleLessonTimer => vec![Effect::ToggleLessonTimer],
        ShortcutCommand::ShowBreakTimerSetup => vec![Effect::ShowBreakTimerSetup],
        ShortcutCommand::ToggleBoard(board) => {
            let target = context
                .board_display_id
                .or(context.display_id)
                .map(str::to_owned)
                .or_else(|| state.active_display_id().map(str::to_owned));
            if let Some(display_id) = target {
                let pass_through_was_enabled = state.is_pass_through_mode_enabled();
                state.toggle_board(shortcut_board(*board), display_id.clone());
                let mut effects = vec![
                    Effect::BoardChanged(state.presented_board().cloned()),
                    Effect::FocusBoardPanel(display_id),
                ];
                if pass_through_was_enabled && !state.is_pass_through_mode_enabled() {
                    effects.insert(0, Effect::PassThroughModeChanged(false));
                }
                effects
            } else {
                vec![Effect::BoardTargetMissing]
            }
        }
        ShortcutCommand::PreviousBoardPage => board_page_effect(
            BoardPageDirection::Previous,
            state,
            context.board_display_id.or(context.display_id),
        ),
        ShortcutCommand::NextBoardPage => board_page_effect(
            BoardPageDirection::Next,
            state,
            context.board_display_id.or(context.display_id),
        ),
        ShortcutCommand::SelectTool(tool) => {
            state.select_tool(shortcut_tool(*tool));
            vec![Effect::ToolChanged(state.current_tool())]
        }
        ShortcutCommand::UseLaser => {
            if context.scope == ShortcutScope::Global {
                state.activate_laser_shortcut();
                vec![
                    Effect::ToolChanged(state.current_tool()),
                    Effect::AnnotationModeChanged(state.is_annotation_mode_enabled()),
                ]
            } else {
                state.select_tool(InkTool::Laser);
                vec![Effect::ToolChanged(state.current_tool())]
            }
        }
        ShortcutCommand::SelectOrCycleEraser => {
            let mut effects = vec![Effect::ToolChanged(InkTool::Eraser)];
            if let Some(mutation) = state.select_or_cycle_eraser(settings)? {
                effects.push(Effect::SettingsMutation(mutation));
            }
            effects
        }
        ShortcutCommand::ToggleEraserTool => {
            state.toggle_eraser_tool();
            vec![Effect::ToolChanged(state.current_tool())]
        }
        ShortcutCommand::ToggleEraserAreaShape => vec![Effect::SettingsMutation(
            state.toggle_eraser_area_shape(settings)?,
        )],
        ShortcutCommand::ToggleShapeRecognition => vec![Effect::SettingsMutation(
            state.toggle_shape_recognition(settings),
        )],
        ShortcutCommand::SelectColor(index) => state
            .select_color(*index, settings)
            .map(Effect::SettingsMutation)
            .into_iter()
            .collect(),
        ShortcutCommand::AdjustWidth(delta) => {
            if context.spotlight_adjusted_width {
                vec![Effect::AdjustSpotlightWidth(*delta)]
            } else {
                state
                    .adjust_current_tool_width(f64::from(*delta), settings)
                    .map(Effect::SettingsMutation)
                    .into_iter()
                    .collect()
            }
        }
        ShortcutCommand::Undo => display_effect(state, context.display_id, Effect::Undo),
        ShortcutCommand::Redo => display_effect(state, context.display_id, Effect::Redo),
        ShortcutCommand::ClearAll => {
            vec![Effect::FinishAllActiveInteractions, Effect::ClearAll]
        }
        ShortcutCommand::DeleteSelection => {
            display_effect(state, context.display_id, Effect::DeleteSelection)
        }
        ShortcutCommand::SaveCurrentContent => vec![Effect::SaveCurrentContent],
        ShortcutCommand::CopyCurrentImage => vec![Effect::CopyCurrentImage],
        ShortcutCommand::TogglePalette => {
            state.toggle_palette_visibility();
            vec![Effect::PaletteVisibilityChanged(state.is_palette_visible())]
        }
        ShortcutCommand::ToggleRadialPalette => vec![Effect::ToggleRadialPalette],
        ShortcutCommand::StepBack => dispatch_step_back(state, context),
        ShortcutCommand::ShowHelp => vec![Effect::ShowHelp],
        ShortcutCommand::AdjustZoom(steps) => vec![Effect::AdjustZoom(*steps)],
        ShortcutCommand::ResetZoom => vec![Effect::ResetZoom],
        ShortcutCommand::ToggleZoomFreeze => vec![Effect::ToggleZoomFreeze],
        ShortcutCommand::StopZoom => vec![Effect::StopZoom],
        ShortcutCommand::BeginZoomPreview => vec![Effect::BeginZoomPreview],
        ShortcutCommand::CycleZoomFollowMode => vec![Effect::CycleZoomFollowMode],
    };
    Ok(effects)
}

// Swift: AppCommandDispatcher.execute previousBoardPage/nextBoardPage
fn board_page_effect(
    direction: BoardPageDirection,
    state: &AppState,
    display_id: Option<&str>,
) -> Vec<Effect> {
    display_id
        .or_else(|| state.active_display_id())
        .map(|display_id| Effect::MoveBoardPage {
            direction,
            display_id: display_id.to_owned(),
        })
        .into_iter()
        .collect()
}

// Swift: AppCommandDispatcher.execute display-targeted command branches
fn display_effect(
    state: &AppState,
    display_id: Option<&str>,
    effect: fn(String) -> Effect,
) -> Vec<Effect> {
    display_id
        .or_else(|| state.active_display_id())
        .map(str::to_owned)
        .map(effect)
        .into_iter()
        .collect()
}

// Swift: AppCommandDispatcher.execute(.stepBack)
fn dispatch_step_back(state: &mut AppState, context: DispatchContext<'_>) -> Vec<Effect> {
    if state.is_pass_through_mode_enabled() {
        state.end_pass_through_mode();
        return vec![
            Effect::PassThroughModeChanged(false),
            Effect::StepBack(StepBackResult::PassThroughMode),
        ];
    }
    if context.radial_palette_expanded {
        return vec![
            Effect::CollapseRadialPalette,
            Effect::StepBack(StepBackResult::RadialPalette),
        ];
    }
    if context.help_visible {
        return vec![Effect::CloseHelp, Effect::StepBack(StepBackResult::Help)];
    }
    let selection_display = context
        .display_id
        .map(str::to_owned)
        .or_else(|| state.active_display_id().map(str::to_owned));
    if context.has_selection {
        if let Some(display_id) = selection_display {
            return vec![
                Effect::ClearSelection(display_id),
                Effect::StepBack(StepBackResult::Selection),
            ];
        }
    }
    if state.current_tool() != InkTool::Pen {
        state.select_tool(InkTool::Pen);
        return vec![
            Effect::ToolChanged(InkTool::Pen),
            Effect::StepBack(StepBackResult::Tool),
        ];
    }
    state.set_annotation_mode_enabled(false);
    vec![
        Effect::FinishAllActiveInteractions,
        Effect::AnnotationModeChanged(false),
        Effect::StepBack(StepBackResult::AnnotationMode),
    ]
}

// Swift: AppCommand.selectTool(InkTool)
const fn shortcut_tool(tool: ShortcutTool) -> InkTool {
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

// Swift: AppCommand.toggleBoard(BoardKind)
const fn shortcut_board(board: ShortcutBoard) -> BoardKind {
    match board {
        ShortcutBoard::White => BoardKind::White,
        ShortcutBoard::Black => BoardKind::Black,
    }
}
