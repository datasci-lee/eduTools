use edutools_core::{
    DisplayIdentity, DisplayKind, DisplayPreset, DisplayPresetValidationError,
    DisplayPresetValidator, DisplaySettings, GridCell, PresetId, PresetMonitor, PresetScreen,
};

// Swift: DisplayPlannerTests.displayFixture
fn monitor(index: u32) -> PresetMonitor {
    PresetMonitor {
        identity: DisplayIdentity {
            uuid: format!("unit-test-display-{index}"),
            vendor: 1_000 + index,
            model: 2_000 + index,
            serial: 3_000 + index,
            name: format!("Unit Test Display {index}"),
            kind: DisplayKind::External,
        },
        auto_match: false,
    }
}

// Swift: PresetScreen.init(monitors:cell:)
fn screen(monitors: Vec<PresetMonitor>, cell: GridCell) -> PresetScreen {
    PresetScreen { monitors, cell }
}

// Swift: DisplayPreset.init(id:name:main:secondary1:secondary2:)
fn preset(index: u64) -> DisplayPreset {
    DisplayPreset {
        id: PresetId::parse(&format!("00000000-0000-0000-0000-{index:012X}")).expect("test UUID"),
        name: format!("Preset {index}"),
        main: screen(vec![monitor(index as u32)], GridCell::center()),
        secondary1: None,
        secondary2: None,
    }
}

// Swift: DisplayPlannerTests.gridValidationRejectsDuplicateAndDiagonalCells
#[test]
fn display_validation_rejects_invalid_grids_and_preset_rules() {
    let mut duplicate_cell = preset(1);
    duplicate_cell.secondary1 = Some(screen(vec![monitor(2)], GridCell::center()));

    let mut diagonal = preset(1);
    diagonal.secondary1 = Some(screen(vec![monitor(2)], GridCell { x: 1, y: 1 }));

    let mut secondary_two_only = preset(1);
    secondary_two_only.secondary2 = Some(screen(vec![monitor(3)], GridCell { x: 1, y: 0 }));

    let mut long_name = preset(1);
    long_name.name = "a".repeat(41);

    let too_many = DisplaySettings {
        auto_switch_enabled: false,
        presets: (1..=9).map(preset).collect(),
    };

    let mut connected = preset(1);
    connected.secondary1 = Some(screen(vec![monitor(2)], GridCell { x: 1, y: 0 }));
    connected.secondary2 = Some(screen(vec![monitor(3)], GridCell { x: 1, y: 1 }));

    let mut duplicate_monitor = preset(1);
    duplicate_monitor.secondary1 = Some(screen(
        vec![duplicate_monitor.main.monitors[0].clone()],
        GridCell { x: 1, y: 0 },
    ));

    let mut empty_main = preset(1);
    empty_main.main.monitors.clear();

    let cases = [
        (
            "duplicate cell",
            DisplaySettings {
                auto_switch_enabled: false,
                presets: vec![duplicate_cell],
            },
            Some(DisplayPresetValidationError::DuplicateGridCell),
        ),
        (
            "diagonal only",
            DisplaySettings {
                auto_switch_enabled: false,
                presets: vec![diagonal],
            },
            Some(DisplayPresetValidationError::DisconnectedGrid),
        ),
        (
            "secondary2 without secondary1",
            DisplaySettings {
                auto_switch_enabled: false,
                presets: vec![secondary_two_only],
            },
            Some(DisplayPresetValidationError::SecondaryTwoRequiresSecondaryOne),
        ),
        (
            "name longer than forty scalars",
            DisplaySettings {
                auto_switch_enabled: false,
                presets: vec![long_name],
            },
            Some(DisplayPresetValidationError::InvalidName),
        ),
        (
            "nine presets",
            too_many,
            Some(DisplayPresetValidationError::TooManyPresets),
        ),
        (
            "duplicate monitor",
            DisplaySettings {
                auto_switch_enabled: false,
                presets: vec![duplicate_monitor],
            },
            Some(DisplayPresetValidationError::DuplicateMonitor(
                "Unit Test Display 1".to_owned(),
            )),
        ),
        (
            "empty main",
            DisplaySettings {
                auto_switch_enabled: false,
                presets: vec![empty_main],
            },
            Some(DisplayPresetValidationError::EmptyMainScreen),
        ),
        (
            "connected",
            DisplaySettings {
                auto_switch_enabled: false,
                presets: vec![connected],
            },
            None,
        ),
    ];

    for (name, settings, expected) in cases {
        assert_eq!(
            DisplayPresetValidator::validate(&settings).err(),
            expected,
            "case={name}"
        );
    }
}
