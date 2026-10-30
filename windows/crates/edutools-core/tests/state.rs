use edutools_core::dispatch::{
    dispatch, should_consume, supports, DispatchContext, Effect, PressedShortcuts, StepBackResult,
};
use edutools_core::ink::{ArrowStyle, InkTool};
use edutools_core::settings::{AppSettings, FavoriteTool, PaletteStyle, SettingsValidator};
use edutools_core::shortcut::{
    ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutScope,
};
use edutools_core::state::{AppState, FavoriteSaveResult, PassThroughModeSource, SettingsMutation};

// Swift: SettingsStore.update(_:)
fn apply(settings: &mut AppSettings, mutation: SettingsMutation) {
    mutation.apply(settings);
    SettingsValidator::validate(settings).expect("state mutation must keep settings valid");
}

// Swift: AppCommandDispatcher.execute(.stepBack)
fn step_back_result(effects: &[Effect]) -> StepBackResult {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::StepBack(result) => Some(*result),
            _ => None,
        })
        .expect("stepBack must report the handled stage")
}

// Swift: ShortcutCatalog.definitions
fn definition(command: ShortcutCommand) -> ShortcutDefinition {
    ShortcutCatalog::definitions()
        .into_iter()
        .find(|definition| definition.command == command)
        .expect("catalog command")
}

// Swift: AppCommandDispatcher.execute(.stepBack)
#[test]
fn step_back_handles_exactly_one_stage_in_order() {
    let settings = AppSettings::default();
    let mut state = AppState::new();
    state.set_annotation_mode_enabled(true);
    state.select_tool(InkTool::Highlighter);
    assert!(state.begin_key_pass_through(0x55, 0.0));

    let all_visible = DispatchContext {
        display_id: Some("display-1"),
        radial_palette_expanded: true,
        help_visible: true,
        has_selection: true,
        ..DispatchContext::default()
    };
    let effects = dispatch(
        &ShortcutCommand::StepBack,
        &mut state,
        &settings,
        all_visible,
    )
    .unwrap();
    assert_eq!(step_back_result(&effects), StepBackResult::PassThroughMode);
    assert!(!state.is_pass_through_mode_enabled());
    assert_eq!(state.current_tool(), InkTool::Highlighter);
    assert!(state.is_annotation_mode_enabled());

    let effects = dispatch(
        &ShortcutCommand::StepBack,
        &mut state,
        &settings,
        all_visible,
    )
    .unwrap();
    assert_eq!(step_back_result(&effects), StepBackResult::RadialPalette);
    assert_eq!(state.current_tool(), InkTool::Highlighter);

    let help_and_selection = DispatchContext {
        radial_palette_expanded: false,
        ..all_visible
    };
    let effects = dispatch(
        &ShortcutCommand::StepBack,
        &mut state,
        &settings,
        help_and_selection,
    )
    .unwrap();
    assert_eq!(step_back_result(&effects), StepBackResult::Help);
    assert_eq!(state.current_tool(), InkTool::Highlighter);

    let selection = DispatchContext {
        help_visible: false,
        ..help_and_selection
    };
    let effects = dispatch(&ShortcutCommand::StepBack, &mut state, &settings, selection).unwrap();
    assert_eq!(step_back_result(&effects), StepBackResult::Selection);
    assert_eq!(state.current_tool(), InkTool::Highlighter);

    let tool = DispatchContext {
        has_selection: false,
        ..selection
    };
    let effects = dispatch(&ShortcutCommand::StepBack, &mut state, &settings, tool).unwrap();
    assert_eq!(step_back_result(&effects), StepBackResult::Tool);
    assert_eq!(state.current_tool(), InkTool::Pen);
    assert!(state.is_annotation_mode_enabled());

    let effects = dispatch(&ShortcutCommand::StepBack, &mut state, &settings, tool).unwrap();
    assert_eq!(step_back_result(&effects), StepBackResult::AnnotationMode);
    assert!(!state.is_annotation_mode_enabled());
}

// Swift: AppState.toggleEraserTool(), AppState.selectOrCycleEraser()
#[test]
fn eraser_toggle_restores_previous_tool_and_cycles_modes() {
    let mut settings = AppSettings::default();
    let mut state = AppState::new();
    state.select_tool(InkTool::Highlighter);

    state.toggle_eraser_tool();
    assert_eq!(state.current_tool(), InkTool::Eraser);
    assert_eq!(state.previous_tool_before_eraser(), InkTool::Highlighter);
    state.toggle_eraser_tool();
    assert_eq!(state.current_tool(), InkTool::Highlighter);

    state.select_tool(InkTool::Pen);
    assert_eq!(state.select_or_cycle_eraser(&settings).unwrap(), None);
    assert_eq!(state.current_tool(), InkTool::Eraser);
    for expected in ["stroke", "area", "partial"] {
        let mutation = state
            .select_or_cycle_eraser(&settings)
            .unwrap()
            .expect("eraser re-selection cycles mode");
        apply(&mut settings, mutation);
        assert_eq!(settings.eraser.mode, expected);
    }
}

// Swift: AppState.selectColor(at:), AppState.selectedColorIndex
#[test]
fn select_color_updates_each_slot_laser_value_and_no_slot_tools() {
    let mut settings = AppSettings::default();
    let mut state = AppState::new();
    let tools = [
        InkTool::Pen,
        InkTool::Highlighter,
        InkTool::Arrow,
        InkTool::Rectangle,
        InkTool::Ellipse,
        InkTool::Text,
    ];
    for (index, tool) in tools.into_iter().enumerate() {
        state.select_tool(tool);
        let mutation = state
            .select_color(index, &settings)
            .expect("memory-slot tool accepts a preset");
        apply(&mut settings, mutation);
        assert_eq!(state.selected_color_index(&settings), index as i64);
    }

    let remembered_tool = state.last_color_tool();
    state.select_tool(InkTool::Laser);
    let laser_color = settings.tools.colors[3].clone();
    let mutation = state
        .select_color(3, &settings)
        .expect("laser accepts color");
    apply(&mut settings, mutation);
    assert_eq!(settings.tools.laser_color, laser_color);
    assert_eq!(state.last_color_tool(), remembered_tool);
    assert_eq!(state.selected_color_index(&settings), 5);

    for tool in [InkTool::Eraser, InkTool::Select, InkTool::Cover] {
        state.select_tool(tool);
        assert_eq!(state.select_color(2, &settings), None);
    }
    assert_eq!(state.select_color(6, &settings), None);
}

// Swift: AppState.currentFavoriteSnapshot(), AppState.applyFavorite(_:)
#[test]
fn favorite_snapshot_round_trips_and_rejects_duplicate_or_excess() {
    let mut settings = AppSettings::default();
    let mut state = AppState::new();
    state.select_tool(InkTool::Arrow);
    let mutation = state.select_color(4, &settings).expect("arrow color");
    apply(&mut settings, mutation);
    apply(
        &mut settings,
        state.set_width(InkTool::Arrow, 8.0).expect("arrow size"),
    );
    state.set_arrow_style(ArrowStyle::Double);

    let favorite = state
        .current_favorite_snapshot(&settings)
        .unwrap()
        .expect("arrow supports favorites");
    let FavoriteSaveResult::Saved(mutation) = state.save_current_favorite(&settings).unwrap()
    else {
        panic!("first unique favorite must be saved");
    };
    apply(&mut settings, mutation);
    assert_eq!(
        state.save_current_favorite(&settings).unwrap(),
        FavoriteSaveResult::Duplicate
    );

    state.select_tool(InkTool::Pen);
    state.set_arrow_style(ArrowStyle::Single);
    let mutations = state
        .apply_favorite(&favorite, &settings)
        .unwrap()
        .expect("valid favorite applies");
    for mutation in mutations {
        apply(&mut settings, mutation);
    }
    assert_eq!(state.current_tool(), InkTool::Arrow);
    assert_eq!(state.arrow_style(), ArrowStyle::Double);
    assert_eq!(state.selected_color_index(&settings), 4);
    assert_eq!(state.width_for(InkTool::Arrow, &settings), Some(8.0));

    settings.tools.favorites = (1..=8)
        .map(|size| FavoriteTool {
            tool: "pen".to_owned(),
            color_index: Some(0),
            size: f64::from(size),
            arrow_style: None,
            eraser_mode: None,
            eraser_area_shape: None,
        })
        .collect();
    state.select_tool(InkTool::Text);
    assert_eq!(
        state.save_current_favorite(&settings).unwrap(),
        FavoriteSaveResult::Full
    );
}

// Swift: AppState pass-through timing and source transitions
#[test]
fn pass_through_respects_guards_idle_deadline_and_key_maximum() {
    let mut state = AppState::new();
    assert!(!state.toggle_pass_through_mode(0.0, true));
    state.set_annotation_mode_enabled(true);
    assert!(!state.toggle_pass_through_mode(0.0, false));
    assert!(state.toggle_pass_through_mode(0.0, true));
    assert_eq!(
        state.pass_through_mode_source(),
        Some(PassThroughModeSource::Timed)
    );
    assert_eq!(state.pass_through_deadlines().idle, Some(10.0));
    assert!(state.record_pass_through_interaction(2.0));
    assert_eq!(state.pass_through_deadlines().idle, Some(5.0));
    assert!(!state.expire_pass_through(5.0, true));
    assert_eq!(state.pass_through_deadlines().idle, Some(8.0));
    assert!(state.expire_pass_through(8.0, false));

    assert!(state.begin_key_pass_through(0x55, 10.0));
    assert_eq!(state.pass_through_deadlines().maximum_hold, Some(40.0));
    assert!(!state.end_key_pass_through(0x56));
    assert!(state.expire_pass_through(40.0, false));
}

// Swift: AppCommandDispatcher.shouldConsume(_:)
#[test]
fn radial_palette_consumption_depends_on_style_and_unconsumed_keys_are_not_recorded() {
    let mut settings = AppSettings::default();
    let command = ShortcutCommand::ToggleRadialPalette;
    assert!(should_consume(&command, &settings));
    settings.tools.palette_style = PaletteStyle::Linear;
    assert!(!should_consume(&command, &settings));

    let definition = definition(command);
    let mut pressed = PressedShortcuts::new();
    let mut state = AppState::new();
    let outcome = pressed
        .key_down(
            0x4d,
            &definition,
            &mut state,
            &settings,
            DispatchContext::default(),
        )
        .unwrap();
    assert!(!outcome.consumed);
    assert!(pressed.is_empty());

    settings.tools.palette_style = PaletteStyle::Radial;
    let outcome = pressed
        .key_down(
            0x4d,
            &definition,
            &mut state,
            &settings,
            DispatchContext::default(),
        )
        .unwrap();
    assert!(outcome.consumed);
    assert_eq!(outcome.effects, vec![Effect::ToggleRadialPalette]);
    assert_eq!(pressed.len(), 1);
    let repeated = pressed
        .key_down(
            0x4d,
            &definition,
            &mut state,
            &settings,
            DispatchContext::default(),
        )
        .unwrap();
    assert!(repeated.consumed);
    assert!(repeated.effects.is_empty());
}

// Swift: AppCommandDispatcher.keyDown/keyUp and pressed definition lookup
#[test]
fn key_up_uses_the_definition_recorded_by_nonrepeat_key_down() {
    let settings = AppSettings::default();
    let mut state = AppState::new();
    state.set_annotation_mode_enabled(true);
    let mut pressed = PressedShortcuts::new();

    let pass_through = definition(ShortcutCommand::TogglePassThroughMode);
    let down = pressed
        .key_down(
            0x55,
            &pass_through,
            &mut state,
            &settings,
            DispatchContext {
                now: 2.0,
                ..DispatchContext::default()
            },
        )
        .unwrap();
    assert!(down.consumed);
    assert_eq!(
        state.pass_through_mode_source(),
        Some(PassThroughModeSource::Key(0x55))
    );
    let up = pressed.key_up(0x55, &mut state);
    assert_eq!(up.effects, vec![Effect::PassThroughModeChanged(false)]);
    assert!(!state.is_pass_through_mode_enabled());

    let help = definition(ShortcutCommand::ShowHelp);
    let down = pressed
        .key_down(
            0xbf,
            &help,
            &mut state,
            &settings,
            DispatchContext::default(),
        )
        .unwrap();
    assert_eq!(down.effects, vec![Effect::HelpShortcutPressed(0xbf)]);
    assert_eq!(
        pressed.key_up(0xbf, &mut state).effects,
        vec![Effect::HelpShortcutReleased]
    );

    let preview = definition(ShortcutCommand::BeginZoomPreview);
    let zoom_context = DispatchContext {
        scope: ShortcutScope::Zoom,
        ..DispatchContext::default()
    };
    let down = pressed
        .key_down(0x20, &preview, &mut state, &settings, zoom_context)
        .unwrap();
    assert_eq!(down.effects, vec![Effect::BeginZoomPreview]);
    assert_eq!(
        pressed.key_up(0x20, &mut state).effects,
        vec![Effect::FinishZoomPreview]
    );
}

// Swift: ShortcutTests.annotationCatalogCommandsAreSupported()
#[test]
fn every_catalog_command_is_supported() {
    for definition in ShortcutCatalog::definitions() {
        assert!(supports(&definition.command), "{}", definition.id);
    }
}
