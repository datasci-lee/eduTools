use edutools_core::eraser::EraserMode;
use edutools_core::ink::{InkTool, Point, Rect};
use edutools_core::radial::{
    group_definitions, hit_target, layout_cells, next_radial_palette_focus, primary_intent,
    secondary_intent, RadialPaletteCell, RadialPaletteCellAction, RadialPaletteEntry,
    RadialPaletteFocus, RadialPaletteGroupId, RadialPaletteHitTarget, RadialPaletteIntent,
    RadialPaletteNavigationKey, RadialPaletteOption, RadialPaletteOptionAction, RadialPaletteRing,
    RadialPaletteSectorGeometry, RadialPaletteState, ENTRY_INNER_RADIUS, ENTRY_OUTER_RADIUS,
    EXPANDED_DIAMETER, PROPERTY_GAP, SECTOR_GAP,
};
use edutools_core::settings::FavoriteTool;
use edutools_core::shortcut::ShortcutCommand;
use std::f64::consts::PI;

const EPSILON: f64 = 1e-9;

// Swift: RadialPaletteNavigationTests.navigationMatchesOriginalTable(_:)
#[test]
fn navigation_matches_original_table() {
    let state = RadialPaletteState::default();
    let definitions = group_definitions(&state);
    let pen = entry(&definitions, "tool.pen");
    let highlighter = entry(&definitions, "tool.highlighter");
    let laser = entry(&definitions, "tool.laser");
    let settings = entry(&definitions, "palette.settings");
    let group = group_cell();
    let option = option_cell("pen.color.1");

    assert_navigation(
        vec![
            group.clone(),
            entry_cell(pen.clone(), false),
            entry_cell(highlighter.clone(), true),
        ],
        RadialPaletteFocus::Cell(group.identifier.clone()),
        RadialPaletteNavigationKey::Deeper,
        None,
        "entry.tool.highlighter",
    );
    assert_navigation(
        vec![
            group.clone(),
            entry_cell(pen.clone(), false),
            entry_cell(highlighter, false),
        ],
        RadialPaletteFocus::Cell(group.identifier.clone()),
        RadialPaletteNavigationKey::Deeper,
        None,
        "entry.tool.pen",
    );
    assert_navigation(
        vec![
            group.clone(),
            entry_cell(laser, true),
            entry_cell(settings, false),
        ],
        RadialPaletteFocus::Cell(group.identifier.clone()),
        RadialPaletteNavigationKey::Deeper,
        Some("palette.settings"),
        "entry.palette.settings",
    );
    assert_navigation(
        vec![group.clone(), entry_cell(pen.clone(), true), option.clone()],
        RadialPaletteFocus::Cell("entry.tool.pen".to_owned()),
        RadialPaletteNavigationKey::Deeper,
        Some("tool.pen"),
        "option.pen.color.1",
    );
    assert_navigation(
        vec![group, entry_cell(pen, true), option],
        RadialPaletteFocus::Cell("option.pen.color.1".to_owned()),
        RadialPaletteNavigationKey::Shallower,
        Some("tool.pen"),
        "entry.tool.pen",
    );
}

// Swift: RadialPaletteTests.maximumFavoriteCellsDoNotOverlapAndHitTest()
#[test]
fn maximum_favorite_cells_do_not_overlap_and_hit_test() {
    let mut state = RadialPaletteState::default();
    state.is_expanded = true;
    state.selected_group = RadialPaletteGroupId::Favorites;
    state.selected_entry_identifier = None;
    state.favorites = (0..8)
        .map(|index| FavoriteTool {
            tool: "pen".to_owned(),
            color_index: Some(0),
            size: f64::from(index + 1),
            arrow_style: None,
            eraser_mode: None,
            eraser_area_shape: None,
        })
        .collect();

    let entries = layout_cells(&state)
        .into_iter()
        .filter(|cell| cell.ring == RadialPaletteRing::Entry)
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 9);
    for pair in entries.windows(2) {
        let gap = pair[1].geometry.start_angle - pair[0].geometry.end_angle;
        assert!(pair[0].geometry.end_angle <= pair[1].geometry.start_angle);
        assert!((gap - 2.0 * SECTOR_GAP).abs() <= EPSILON);
    }

    let target = entries.last().expect("favorite entry");
    let angle = (target.geometry.start_angle + target.geometry.end_angle) / 2.0;
    let radius = (ENTRY_INNER_RADIUS + ENTRY_OUTER_RADIUS) / 2.0;
    let bounds = expanded_bounds();
    let point = Point {
        x: bounds.x + bounds.width / 2.0 + angle.sin() * radius,
        y: bounds.y + bounds.height / 2.0 + angle.cos() * radius,
    };
    let hit = hit_target(&state, point, bounds).expect("favorite center must hit");
    assert_eq!(hit.identifier(), target.identifier);

    let center = hit_target(
        &state,
        Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        },
        bounds,
    )
    .expect("center must hit");
    assert_eq!(
        secondary_intent(&center),
        Some(RadialPaletteIntent::Execute(ShortcutCommand::StepBack))
    );
}

// Swift: RadialPaletteTests.eraserModeEntriesSelectOneModeAndRetoggleAreaShape()
#[test]
fn eraser_mode_entries_select_one_mode_and_retoggle_area_shape() {
    let mut state = RadialPaletteState::default();
    state.is_expanded = true;
    state.selected_group = RadialPaletteGroupId::Erasing;
    state.selected_entry_identifier = Some("eraser.mode.area".to_owned());
    state.current_tool = InkTool::Eraser;
    state.eraser_mode = EraserMode::Area;

    let eraser_entries = layout_cells(&state)
        .into_iter()
        .filter(|cell| {
            matches!(
                &cell.action,
                RadialPaletteCellAction::Entry(entry) if entry.eraser_mode.is_some()
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(eraser_entries.len(), 3);
    let selected = eraser_entries
        .iter()
        .filter(|cell| cell.is_selected)
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].identifier, "entry.eraser.mode.area");

    let area_target = RadialPaletteHitTarget::Cell(selected[0].clone());
    assert_eq!(
        primary_intent(&area_target, &state),
        Some(RadialPaletteIntent::ToggleEraserAreaShape)
    );

    state.current_tool = InkTool::Pen;
    assert_eq!(
        primary_intent(&area_target, &state),
        Some(RadialPaletteIntent::SelectEraserMode(EraserMode::Area))
    );
}

// Swift: RadialPaletteTests.optionRingCapsSlicesAndAlignsSections()
#[test]
fn option_ring_caps_slices_and_aligns_sections() {
    let full_circle = 2.0 * PI;
    let group_slice = full_circle / RadialPaletteGroupId::all().len() as f64;
    let maximum_slice = 30.0_f64.to_radians();
    let maximum_cell_width = maximum_slice - SECTOR_GAP;

    let mut laser_state = RadialPaletteState::default();
    laser_state.is_expanded = true;
    laser_state.selected_group = RadialPaletteGroupId::Presenting;
    laser_state.selected_entry_identifier = Some("tool.laser".to_owned());
    laser_state.current_tool = InkTool::Laser;
    let laser_options = option_cells(&laser_state);
    assert_eq!(laser_options.len(), 6);
    assert!(laser_options.iter().all(|cell| {
        cell.geometry.end_angle - cell.geometry.start_angle <= maximum_slice + EPSILON
    }));
    assert!(laser_options.iter().all(|cell| {
        ((cell.geometry.end_angle - cell.geometry.start_angle) - maximum_cell_width).abs()
            <= EPSILON
    }));
    let first = laser_options.first().expect("laser first option");
    let last = laser_options.last().expect("laser last option");
    let arc_start = first.geometry.start_angle - SECTOR_GAP / 2.0;
    let arc_end = last.geometry.end_angle + SECTOR_GAP / 2.0;
    let arc_center = (arc_start + arc_end) / 2.0;
    let presenting_opposite = RadialPaletteGroupId::Presenting.index() as f64 * group_slice + PI;
    assert!(angular_distance(arc_center, presenting_opposite) <= EPSILON);

    let mut pen_state = RadialPaletteState::default();
    pen_state.is_expanded = true;
    pen_state.selected_group = RadialPaletteGroupId::Writing;
    pen_state.selected_entry_identifier = Some("tool.pen".to_owned());
    pen_state.current_tool = InkTool::Pen;
    let pen_options = option_cells(&pen_state);
    assert_eq!(pen_options.len(), 11);
    assert!(pen_options.iter().all(|cell| {
        cell.geometry.end_angle - cell.geometry.start_angle <= maximum_cell_width + EPSILON
    }));
    assert!(
        (pen_options[0].geometry.start_angle - SECTOR_GAP / 2.0 - PROPERTY_GAP / 2.0).abs()
            <= EPSILON
    );
    assert!(
        pen_options[6].geometry.start_angle - pen_options[5].geometry.end_angle >= PROPERTY_GAP
    );
}

// Swift: RadialPaletteNavigationTests.navigationInput(for:)
fn assert_navigation(
    cells: Vec<RadialPaletteCell>,
    current: RadialPaletteFocus,
    key: RadialPaletteNavigationKey,
    selected_entry_identifier: Option<&str>,
    expected: &str,
) {
    let result = next_radial_palette_focus(&cells, &current, key, selected_entry_identifier);
    assert_eq!(result.identifier(), expected);
}

// Swift: RadialPaletteNavigationTests.groupCell()
fn group_cell() -> RadialPaletteCell {
    RadialPaletteCell {
        identifier: "group.0".to_owned(),
        ring: RadialPaletteRing::Group,
        geometry: test_geometry(),
        name: "작성".to_owned(),
        icon_key: None,
        short_label: String::new(),
        shortcut: String::new(),
        color: None,
        preview: None,
        is_selected: true,
        is_enabled: true,
        action: RadialPaletteCellAction::Group(RadialPaletteGroupId::Writing),
    }
}

// Swift: RadialPaletteNavigationTests.entryCell(_:isSelected:)
fn entry_cell(entry: RadialPaletteEntry, is_selected: bool) -> RadialPaletteCell {
    RadialPaletteCell {
        identifier: format!("entry.{}", entry.identifier),
        ring: RadialPaletteRing::Entry,
        geometry: test_geometry(),
        name: entry.name.clone(),
        icon_key: Some(entry.icon_key.clone()),
        short_label: String::new(),
        shortcut: entry.shortcut.clone(),
        color: None,
        preview: None,
        is_selected,
        is_enabled: true,
        action: RadialPaletteCellAction::Entry(entry),
    }
}

// Swift: RadialPaletteNavigationTests.optionCell(identifier:)
fn option_cell(identifier: &str) -> RadialPaletteCell {
    let option = RadialPaletteOption {
        identifier: identifier.to_owned(),
        name: "색 1".to_owned(),
        short_label: "1".to_owned(),
        color: None,
        preview: None,
        shortcut: "1".to_owned(),
        is_selected: true,
        is_enabled: true,
        action: RadialPaletteOptionAction::SelectColor(0),
    };
    RadialPaletteCell {
        identifier: format!("option.{identifier}"),
        ring: RadialPaletteRing::Option,
        geometry: test_geometry(),
        name: option.name.clone(),
        icon_key: None,
        short_label: option.short_label.clone(),
        shortcut: option.shortcut.clone(),
        color: None,
        preview: None,
        is_selected: true,
        is_enabled: true,
        action: RadialPaletteCellAction::Option(option),
    }
}

// Swift: RadialPaletteNavigationTests.geometry
fn test_geometry() -> RadialPaletteSectorGeometry {
    RadialPaletteSectorGeometry {
        inner_radius: 0.0,
        outer_radius: 1.0,
        start_angle: 0.0,
        end_angle: 1.0,
    }
}

// Swift: RadialPaletteCatalog.definition(for:) 시험 헬퍼
fn entry(
    definitions: &[edutools_core::radial::RadialPaletteGroupDefinition],
    id: &str,
) -> RadialPaletteEntry {
    definitions
        .iter()
        .flat_map(|group| group.entries.iter())
        .find(|entry| entry.identifier == id)
        .expect("catalog entry")
        .clone()
}

// Swift: RadialPaletteMetrics.expandedDiameter
fn expanded_bounds() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: EXPANDED_DIAMETER,
        height: EXPANDED_DIAMETER,
    }
}

// Swift: RadialPaletteController.layoutCells() 옵션 필터
fn option_cells(state: &RadialPaletteState) -> Vec<RadialPaletteCell> {
    layout_cells(state)
        .into_iter()
        .filter(|cell| cell.ring == RadialPaletteRing::Option)
        .collect()
}

// Swift: RadialPaletteTests.optionRingCapsSlicesAndAlignsSections() 각도 비교
fn angular_distance(first: f64, second: f64) -> f64 {
    let difference = (first.rem_euclid(2.0 * PI) - second.rem_euclid(2.0 * PI)).abs();
    difference.min(2.0 * PI - difference)
}
