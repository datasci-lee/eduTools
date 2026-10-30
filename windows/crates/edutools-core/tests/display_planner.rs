use edutools_core::display_operations::{build as build_operations, Operation};
use edutools_core::display_preset::{
    DisplayAutoSwitchDecision, DisplayAutoSwitchPolicy, DisplayGrid, DisplayGridMetric,
    DisplayIdentity, DisplayKind, DisplayMatch, DisplayMatcher, DisplayMirrorPlan,
    DisplayOutsidePlacement, DisplayPlan, DisplayPlanner, DisplayPoint, DisplayPreset,
    DisplayPresetDefaults, DisplaySize, GridCell, OnlineDisplay, PresetId, PresetMonitor,
    PresetScreen,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

// Swift: DisplayGridVectorFile
#[derive(Deserialize)]
struct GridVectorFile {
    version: u32,
    cases: Vec<GridVectorCase>,
}

// Swift: DisplayGridVectorCase
#[derive(Deserialize)]
struct GridVectorCase {
    name: String,
    input: GridInput,
    expected: GridExpected,
}

// Swift: DisplayGridVectorCase.Input
#[derive(Deserialize)]
struct GridInput {
    metrics: Vec<GridMetric>,
}

// Swift: DisplayGridVectorCase.Metric
#[derive(Deserialize)]
struct GridMetric {
    cell: GridCell,
    size: DisplaySize,
}

// Swift: DisplayGridVectorCase.Expected
#[derive(Deserialize)]
struct GridExpected {
    origins: Vec<GridOrigin>,
}

// Swift: DisplayGridVectorCase.Origin
#[derive(Deserialize)]
struct GridOrigin {
    cell: GridCell,
    origin: DisplayPoint,
}

// Swift: DisplayPlannerTests.displayFixture
#[derive(Clone)]
struct DisplayFixture {
    online: OnlineDisplay,
    monitor: PresetMonitor,
}

// Swift: DisplayPlannerTests.displayFixture(id:size:kind:)
fn display_fixture(id: u32, size: DisplaySize, kind: DisplayKind) -> DisplayFixture {
    let online = OnlineDisplay {
        id,
        uuid: format!("unit-test-display-{id}"),
        vendor: 1_000 + id,
        model: 2_000 + id,
        serial: 3_000 + id,
        name: format!("Unit Test Display {id}"),
        kind,
        size,
        origin: DisplayPoint::zero(),
        current_mirror_master_id: None,
    };
    DisplayFixture {
        monitor: PresetMonitor {
            identity: online.identity(),
            auto_match: false,
        },
        online,
    }
}

// Swift: UUID(uuidString:) test fixture
fn preset_id(index: u64) -> PresetId {
    PresetId::parse(&format!("00000000-0000-0000-0000-{index:012X}")).expect("valid preset id")
}

// Swift: DisplayPreset.init(id:name:main:secondary1:secondary2:)
fn preset(
    index: u64,
    name: &str,
    main: PresetScreen,
    secondary1: Option<PresetScreen>,
    secondary2: Option<PresetScreen>,
) -> DisplayPreset {
    DisplayPreset {
        id: preset_id(index),
        name: name.to_owned(),
        main,
        secondary1,
        secondary2,
    }
}

// Swift: PresetScreen.init(monitors:cell:)
fn screen(monitors: Vec<PresetMonitor>, cell: GridCell) -> PresetScreen {
    PresetScreen { monitors, cell }
}

// Swift: TestSupport.Fixtures URL lookup
fn repository_fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

// Swift: DisplayPlannerTests.gridLayoutsFollowVector(_:)
#[test]
fn grid_layouts_follow_vector() {
    let data = fs::read_to_string(repository_fixture(
        "fixtures/vectors/display-grid-layouts.json",
    ))
    .expect("read display grid vectors");
    let fixture: GridVectorFile = serde_json::from_str(&data).expect("decode display grid vectors");
    assert_eq!(fixture.version, 1);

    for (case_index, case) in fixture.cases.into_iter().enumerate() {
        let metrics = case
            .input
            .metrics
            .iter()
            .map(|metric| DisplayGridMetric {
                cell: metric.cell,
                size: metric.size,
            })
            .collect::<Vec<_>>();
        let displays = metrics
            .iter()
            .enumerate()
            .map(|(index, metric)| {
                display_fixture((index + 1) as u32, metric.size, DisplayKind::External)
            })
            .collect::<Vec<_>>();
        let screens = displays
            .iter()
            .zip(metrics.iter())
            .map(|(display, metric)| screen(vec![display.monitor.clone()], metric.cell))
            .collect::<Vec<_>>();
        let plan = DisplayPlanner::plan(
            &preset(
                case_index as u64 + 1,
                &case.name,
                screens[0].clone(),
                screens.get(1).cloned(),
                screens.get(2).cloned(),
            ),
            &displays
                .iter()
                .map(|display| display.online.clone())
                .collect::<Vec<_>>(),
        );
        let origins = plan
            .screens
            .iter()
            .map(|planned| (planned.cell, planned.origin))
            .collect::<HashMap<_, _>>();

        assert_eq!(origins.len(), metrics.len(), "case={}", case.name);
        for expected in case.expected.origins {
            assert_eq!(
                origins.get(&expected.cell),
                Some(&expected.origin),
                "case={}, cell=({},{})",
                case.name,
                expected.cell.x,
                expected.cell.y
            );
        }
    }
}

// Swift: DisplayPlannerTests.connectedMainAnchorsAtZeroAndOverlapIsRejected()
#[test]
fn connected_main_anchors_at_zero_and_overlap_is_rejected() {
    let absent_main = display_fixture(
        21,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let connected_left = display_fixture(
        22,
        DisplaySize {
            width: 2560.0,
            height: 1440.0,
        },
        DisplayKind::External,
    );
    let connected_right = display_fixture(
        23,
        DisplaySize {
            width: 1280.0,
            height: 720.0,
        },
        DisplayKind::External,
    );
    let missing_main_plan = DisplayPlanner::plan(
        &preset(
            1,
            "Missing main",
            screen(vec![absent_main.monitor], GridCell::center()),
            Some(screen(
                vec![connected_left.monitor.clone()],
                GridCell { x: -1, y: 0 },
            )),
            Some(screen(
                vec![connected_right.monitor.clone()],
                GridCell { x: 1, y: 0 },
            )),
        ),
        &[
            connected_left.online.clone(),
            connected_right.online.clone(),
        ],
    );
    let overlap_main = display_fixture(
        24,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let overlap_right = display_fixture(
        25,
        DisplaySize {
            width: 1280.0,
            height: 1440.0,
        },
        DisplayKind::External,
    );
    let overlap_below = display_fixture(
        26,
        DisplaySize {
            width: 3000.0,
            height: 720.0,
        },
        DisplayKind::External,
    );
    let overlap_plan = DisplayPlanner::plan(
        &preset(
            2,
            "Overlap",
            screen(vec![overlap_main.monitor], GridCell::center()),
            Some(screen(vec![overlap_right.monitor], GridCell { x: 1, y: 0 })),
            Some(screen(vec![overlap_below.monitor], GridCell { x: 0, y: 1 })),
        ),
        &[
            overlap_main.online,
            overlap_right.online,
            overlap_below.online,
        ],
    );

    assert_eq!(
        missing_main_plan.main_display_id,
        Some(connected_left.online.id)
    );
    assert_eq!(
        missing_main_plan
            .origins_by_master
            .get(&connected_left.online.id),
        Some(&DisplayPoint::zero())
    );
    assert_eq!(
        missing_main_plan
            .origins_by_master
            .get(&connected_right.online.id),
        Some(&DisplayPoint { x: 2560.0, y: 0.0 })
    );
    assert!(!overlap_plan.layout_issues.is_empty());
    assert!(!overlap_plan.is_applicable());
}

// Swift: DisplayPlannerTests.mirrorSuccessionMissingAndOutsideRulesArePreserved()
#[test]
fn mirror_succession_missing_and_outside_rules_are_preserved() {
    let display_one = display_fixture(
        1,
        DisplaySize {
            width: 2560.0,
            height: 1440.0,
        },
        DisplayKind::External,
    );
    let display_two = display_fixture(
        2,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let display_three = display_fixture(
        3,
        DisplaySize {
            width: 1280.0,
            height: 720.0,
        },
        DisplayKind::External,
    );
    let online = [
        display_one.online.clone(),
        display_two.online.clone(),
        display_three.online.clone(),
    ];
    let mirror_plan = DisplayPlanner::plan(
        &preset(
            1,
            "Mirrored main",
            screen(
                vec![
                    display_one.monitor.clone(),
                    display_two.monitor.clone(),
                    display_three.monitor.clone(),
                ],
                GridCell::center(),
            ),
            None,
            None,
        ),
        &online,
    );
    let extended_mirror_plan = DisplayPlanner::plan(
        &preset(
            2,
            "Mirrored secondary",
            screen(vec![display_one.monitor.clone()], GridCell::center()),
            Some(screen(
                vec![display_two.monitor.clone(), display_three.monitor.clone()],
                GridCell { x: 1, y: 0 },
            )),
            None,
        ),
        &online,
    );
    let missing_first = display_fixture(
        31,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let succeeding_master = display_fixture(
        32,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let succession_plan = DisplayPlanner::plan(
        &preset(
            3,
            "Master succession",
            screen(
                vec![
                    missing_first.monitor.clone(),
                    succeeding_master.monitor.clone(),
                ],
                GridCell::center(),
            ),
            None,
            None,
        ),
        std::slice::from_ref(&succeeding_master.online),
    );
    let outside_display = display_fixture(
        33,
        DisplaySize {
            width: 1024.0,
            height: 768.0,
        },
        DisplayKind::Virtual,
    );
    let outside_plan = DisplayPlanner::plan(
        &preset(
            4,
            "Outside display",
            screen(vec![display_one.monitor.clone()], GridCell::center()),
            None,
            None,
        ),
        &[display_one.online.clone(), outside_display.online.clone()],
    );

    assert_eq!(mirror_plan.main_display_id, Some(display_one.online.id));
    assert_eq!(
        mirror_plan.mirrors,
        vec![
            DisplayMirrorPlan {
                slave_id: display_two.online.id,
                master_id: display_one.online.id,
            },
            DisplayMirrorPlan {
                slave_id: display_three.online.id,
                master_id: display_one.online.id,
            },
        ]
    );
    assert_eq!(
        extended_mirror_plan.mirrors,
        vec![DisplayMirrorPlan {
            slave_id: display_three.online.id,
            master_id: display_two.online.id,
        }]
    );
    assert_eq!(
        extended_mirror_plan
            .origins_by_master
            .get(&display_two.online.id),
        Some(&DisplayPoint { x: 2560.0, y: 0.0 })
    );
    assert_eq!(
        succession_plan.main_display_id,
        Some(succeeding_master.online.id)
    );
    assert_eq!(
        succession_plan.missing_preset_monitors,
        vec![missing_first.monitor.identity]
    );
    assert_eq!(outside_plan.outside_placements.len(), 1);
    assert_eq!(
        outside_plan.outside_placements[0].display_id,
        outside_display.online.id
    );
    assert!(outside_plan.outside_placements[0].origin.x >= 2560.0);
}

// Swift: DisplayPlannerTests.gridValidationRejectsDuplicateAndDiagonalCells()
#[test]
fn grid_validation_rejects_duplicate_and_diagonal_cells() {
    let displays = (1..=3)
        .map(|id| {
            display_fixture(
                id,
                DisplaySize {
                    width: 2560.0 - f64::from(id - 1) * 640.0,
                    height: 1440.0 - f64::from(id - 1) * 360.0,
                },
                DisplayKind::External,
            )
        })
        .collect::<Vec<_>>();
    let duplicate = preset(
        1,
        "Duplicate cell",
        screen(vec![displays[0].monitor.clone()], GridCell::center()),
        Some(screen(
            vec![displays[1].monitor.clone()],
            GridCell::center(),
        )),
        None,
    );
    let diagonal = preset(
        2,
        "Diagonal",
        screen(vec![displays[0].monitor.clone()], GridCell::center()),
        Some(screen(
            vec![displays[1].monitor.clone()],
            GridCell { x: 1, y: 1 },
        )),
        None,
    );
    let connected = preset(
        3,
        "Connected",
        screen(vec![displays[0].monitor.clone()], GridCell::center()),
        Some(screen(
            vec![displays[1].monitor.clone()],
            GridCell { x: 1, y: 0 },
        )),
        Some(screen(
            vec![displays[2].monitor.clone()],
            GridCell { x: 1, y: 1 },
        )),
    );

    assert_eq!(
        DisplayGrid::validation_error(&duplicate),
        Some(edutools_core::display_preset::DisplayPresetValidationError::DuplicateGridCell)
    );
    assert_eq!(
        DisplayGrid::validation_error(&diagonal),
        Some(edutools_core::display_preset::DisplayPresetValidationError::DisconnectedGrid)
    );
    assert_eq!(DisplayGrid::validation_error(&connected), None);
}

// Swift: DisplayPlannerTests.automaticSelectionRequiresExactSetAndPreservesPriority()
#[test]
fn automatic_selection_requires_exact_set_and_preserves_priority() {
    let displays = (1..=3)
        .map(|id| {
            display_fixture(
                id,
                DisplaySize {
                    width: 2560.0 - f64::from(id - 1) * 640.0,
                    height: 1440.0 - f64::from(id - 1) * 360.0,
                },
                DisplayKind::External,
            )
        })
        .collect::<Vec<_>>();
    let mut monitor_one = displays[0].monitor.clone();
    monitor_one.auto_match = true;
    let mut monitor_two = displays[1].monitor.clone();
    monitor_two.auto_match = true;
    let preset_a = preset(
        10,
        "Automatic A",
        screen(
            vec![monitor_one.clone(), monitor_two.clone()],
            GridCell::center(),
        ),
        None,
        None,
    );
    let preset_b = preset(
        11,
        "Automatic B",
        screen(vec![monitor_one, monitor_two], GridCell::center()),
        None,
        None,
    );
    let manual = preset(
        12,
        "No automatic monitors",
        screen(vec![displays[0].monitor.clone()], GridCell::center()),
        None,
        None,
    );
    let first_two = [displays[0].online.clone(), displays[1].online.clone()];

    assert_eq!(
        DisplayPlanner::preset_for_connected_set(&first_two, &[preset_a.clone(), preset_b.clone()])
            .map(|value| value.id.as_str()),
        Some(preset_a.id.as_str())
    );
    assert_eq!(
        DisplayPlanner::preset_for_connected_set(&first_two, &[preset_b.clone(), preset_a.clone()])
            .map(|value| value.id.as_str()),
        Some(preset_b.id.as_str())
    );
    assert!(DisplayPlanner::preset_for_connected_set(
        &[
            displays[0].online.clone(),
            displays[1].online.clone(),
            displays[2].online.clone(),
        ],
        std::slice::from_ref(&preset_a),
    )
    .is_none());
    assert!(DisplayPlanner::preset_for_connected_set(
        std::slice::from_ref(&displays[0].online),
        std::slice::from_ref(&preset_a),
    )
    .is_none());
    assert!(DisplayPlanner::preset_for_connected_set(
        std::slice::from_ref(&displays[0].online),
        std::slice::from_ref(&manual),
    )
    .is_none());
}

// Swift: DisplayAutoSwitchBoundary
#[derive(Clone, Copy, Debug)]
enum AutoSwitchBoundary {
    UnchangedConnectionSet,
    Disabled,
    NoMatchingPreset,
    OneExtraOnlineDisplay,
    OneMissingOnlineDisplay,
    NoAutomaticMonitors,
    FirstMatchingPresetWins,
    CurrentLayoutAlreadyMatches,
    InsideCooldown,
    CooldownElapsed,
}

// Swift: DisplayPlannerTests.automaticSwitchPolicyPreservesSafetyBoundaries(_:)
#[test]
fn automatic_switch_policy_preserves_safety_boundaries() {
    let display_one = display_fixture(
        101,
        DisplaySize {
            width: 2560.0,
            height: 1440.0,
        },
        DisplayKind::External,
    );
    let display_two = display_fixture(
        102,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let display_three = display_fixture(
        103,
        DisplaySize {
            width: 1280.0,
            height: 720.0,
        },
        DisplayKind::External,
    );

    // Swift: DisplayPlannerTests.automaticSwitchPolicyPreservesSafetyBoundaries.preset
    let automatic_preset = |index: u64, name: &str, automatic: bool| {
        let mut monitor_one = display_one.monitor.clone();
        monitor_one.auto_match = automatic;
        let mut monitor_two = display_two.monitor.clone();
        monitor_two.auto_match = automatic;
        preset(
            index,
            name,
            screen(vec![monitor_one, monitor_two], GridCell::center()),
            None,
            None,
        )
    };
    let first_preset = automatic_preset(1, "First", true);
    let second_preset = automatic_preset(2, "Second", true);
    let boundaries = [
        AutoSwitchBoundary::UnchangedConnectionSet,
        AutoSwitchBoundary::Disabled,
        AutoSwitchBoundary::NoMatchingPreset,
        AutoSwitchBoundary::OneExtraOnlineDisplay,
        AutoSwitchBoundary::OneMissingOnlineDisplay,
        AutoSwitchBoundary::NoAutomaticMonitors,
        AutoSwitchBoundary::FirstMatchingPresetWins,
        AutoSwitchBoundary::CurrentLayoutAlreadyMatches,
        AutoSwitchBoundary::InsideCooldown,
        AutoSwitchBoundary::CooldownElapsed,
    ];

    for boundary in boundaries {
        let mut online = vec![display_one.online.clone(), display_two.online.clone()];
        let mut presets = vec![first_preset.clone()];
        let mut enabled = true;
        let mut last_judged_uuids = HashSet::from(["previous-display".to_owned()]);
        let mut last_auto_apply_at = None;
        let now = 100.0;
        let mut expected = DisplayAutoSwitchDecision::Ignore;

        match boundary {
            AutoSwitchBoundary::UnchangedConnectionSet => {
                last_judged_uuids = online.iter().map(|display| display.uuid.clone()).collect();
            }
            AutoSwitchBoundary::Disabled => enabled = false,
            AutoSwitchBoundary::NoMatchingPreset => {
                online = vec![display_one.online.clone(), display_three.online.clone()]
            }
            AutoSwitchBoundary::OneExtraOnlineDisplay => online.push(display_three.online.clone()),
            AutoSwitchBoundary::OneMissingOnlineDisplay => {
                online = vec![display_one.online.clone()]
            }
            AutoSwitchBoundary::NoAutomaticMonitors => {
                presets = vec![automatic_preset(3, "Manual only", false)]
            }
            AutoSwitchBoundary::FirstMatchingPresetWins => {
                presets = vec![first_preset.clone(), second_preset.clone()];
                expected = DisplayAutoSwitchDecision::Apply(0);
            }
            AutoSwitchBoundary::CurrentLayoutAlreadyMatches => {
                online[1].current_mirror_master_id = Some(display_one.online.id)
            }
            AutoSwitchBoundary::InsideCooldown => {
                last_auto_apply_at = Some(90.0);
                expected = DisplayAutoSwitchDecision::Retry(20.0);
            }
            AutoSwitchBoundary::CooldownElapsed => {
                last_auto_apply_at = Some(70.0);
                expected = DisplayAutoSwitchDecision::Apply(0);
            }
        }

        assert_eq!(
            DisplayAutoSwitchPolicy::decide(
                &online,
                &presets,
                enabled,
                Some(&last_judged_uuids),
                last_auto_apply_at,
                now,
                DisplayPresetDefaults::AUTOMATIC_APPLY_COOLDOWN,
            ),
            expected,
            "boundary={boundary:?}"
        );
    }
}

// Swift: DisplayPlannerTests.matchingUsesValidHardwareIdentityWithoutReuse()
#[test]
fn matching_uses_valid_hardware_identity_without_reuse() {
    let hardware = display_fixture(
        41,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let mut hardware_online = hardware.online.clone();
    hardware_online.uuid = "different-hardware-uuid".to_owned();
    let hardware_matches =
        DisplayMatcher::matches(std::slice::from_ref(&hardware.monitor), &[hardware_online]);
    let zero_serial = display_fixture(
        42,
        DisplaySize {
            width: 1920.0,
            height: 1080.0,
        },
        DisplayKind::External,
    );
    let mut zero_serial_monitor = zero_serial.monitor.clone();
    zero_serial_monitor.identity.uuid = "preset-zero-serial".to_owned();
    zero_serial_monitor.identity.serial = 0;
    let mut zero_serial_online = zero_serial.online;
    zero_serial_online.uuid = "online-zero-serial".to_owned();
    zero_serial_online.serial = 0;
    let zero_serial_matches =
        DisplayMatcher::matches(&[zero_serial_monitor], &[zero_serial_online]);
    let duplicate_online_matches = DisplayMatcher::matches(
        &[hardware.monitor.clone(), hardware.monitor.clone()],
        std::slice::from_ref(&hardware.online),
    );

    assert_eq!(
        hardware_matches,
        vec![DisplayMatch {
            preset_index: 0,
            online_index: 0,
        }]
    );
    assert!(zero_serial_matches.is_empty());
    assert_eq!(duplicate_online_matches.len(), 1);
    assert_eq!(
        duplicate_online_matches
            .iter()
            .map(|matched| matched.online_index)
            .collect::<HashSet<_>>()
            .len(),
        duplicate_online_matches.len()
    );
}

// Swift: DisplayConfigurationOperations.build(from:)
#[test]
fn display_configuration_operations_preserve_phase_and_sort_order() {
    let plan = DisplayPlan {
        main_display_id: Some(1),
        origins_by_master: HashMap::from([
            (7, DisplayPoint { x: 700.0, y: 0.0 }),
            (5, DisplayPoint { x: 500.0, y: 0.0 }),
            (1, DisplayPoint::zero()),
        ]),
        mirrors: vec![
            DisplayMirrorPlan {
                slave_id: 8,
                master_id: 4,
            },
            DisplayMirrorPlan {
                slave_id: 3,
                master_id: 1,
            },
            DisplayMirrorPlan {
                slave_id: 3,
                master_id: 0,
            },
        ],
        unmirror_display_ids: vec![9, 2, 9],
        outside_placements: vec![DisplayOutsidePlacement {
            display_id: 7,
            name: "Outside".to_owned(),
            origin: DisplayPoint { x: 700.0, y: 0.0 },
        }],
        missing_preset_monitors: Vec::<DisplayIdentity>::new(),
        screens: Vec::new(),
        layout_issues: Vec::new(),
    };

    assert_eq!(
        build_operations(&plan),
        vec![
            Operation::Unmirror { display_id: 2 },
            Operation::Unmirror { display_id: 9 },
            Operation::Mirror {
                display_id: 3,
                master_id: 0,
            },
            Operation::Mirror {
                display_id: 3,
                master_id: 1,
            },
            Operation::Mirror {
                display_id: 8,
                master_id: 4,
            },
            Operation::SetOrigin {
                display_id: 1,
                origin: DisplayPoint::zero(),
            },
            Operation::SetOrigin {
                display_id: 5,
                origin: DisplayPoint { x: 500.0, y: 0.0 },
            },
            Operation::SetOrigin {
                display_id: 7,
                origin: DisplayPoint { x: 700.0, y: 0.0 },
            },
        ]
    );
}
