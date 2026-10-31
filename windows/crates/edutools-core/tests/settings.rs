use edutools_core::{
    AppSettings, BoardGroupSettings, DisplayPreset, EraserGroupSettings, GeneralSettings,
    HelpGroupSettings, SettingsValidator, ShortcutGroupSettings, TimerGroupSettings, ToolSettings,
    ZoomGroupSettings,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;

// Swift: SettingsDocument subset used by SettingsTests.exportIncludesPresetsAndExcludesComputerSpecificValues
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureDocument {
    included_bundles: Vec<String>,
    payload: Value,
}

// Swift: DisplayPresetBundle
#[derive(Deserialize)]
struct FixtureDisplayBundle {
    presets: Vec<DisplayPreset>,
}

// Swift: Tests fixture lookup through TestSupport.Fixtures
fn repository_fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

// Swift: JSONEncoder().encode(AppSettings.default)
fn default_json() -> Value {
    serde_json::to_value(AppSettings::default()).expect("default settings serialize")
}

// Swift: DisplayPreset Codable input used by SettingsTests
fn preset_json() -> Value {
    json!({
        "id": "11111111-2222-3333-4444-555555555555",
        "name": "Classroom",
        "main": {
            "monitors": [{
                "identity": {
                    "uuid": "display-test-uuid",
                    "vendor": 1,
                    "model": 2,
                    "serial": 3,
                    "name": "Classroom Display",
                    "kind": "external"
                },
                "autoMatch": true
            }],
            "cell": {"x": 0, "y": 0}
        }
    })
}

// Swift: SettingsTests JSON mutation helpers
fn object_at_mut<'a>(value: &'a mut Value, key: &str) -> &'a mut Map<String, Value> {
    value
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{key} object"))
}

// Swift: SettingsGoldenIntegrationTests.everyVersionedFixtureRoundTripsThroughSettingsStore
#[test]
fn golden_settings_decode_validate_and_round_trip() {
    let data = fs::read_to_string(repository_fixture(
        "Tests/Fixtures/settings/v0.1.0/settings.json",
    ))
    .expect("read golden settings");
    let decoded: AppSettings = serde_json::from_str(&data).expect("decode golden settings");

    SettingsValidator::validate(&decoded).expect("golden settings validate");
    let encoded = serde_json::to_string(&decoded).expect("serialize golden settings");
    let round_tripped: AppSettings =
        serde_json::from_str(&encoded).expect("decode serialized golden settings");

    assert_eq!(round_tripped, decoded);
    assert_eq!(decoded.tools.palette_glass_opacity, 0.62);
}

// Swift: SettingsTests.exportIncludesPresetsAndExcludesComputerSpecificValues
#[test]
fn macos_pansetting_all_bundles_apply_to_default() {
    let data = fs::read_to_string(repository_fixture(
        "fixtures/pansetting/macos-v1.pansetting",
    ))
    .expect("read macOS pansetting fixture");
    let document: FixtureDocument = serde_json::from_str(&data).expect("decode fixture document");
    let expected_bundles = [
        "general",
        "tools",
        "eraser",
        "zoom",
        "board",
        "timer",
        "display",
        "shortcuts",
        "help",
    ];
    assert_eq!(document.included_bundles, expected_bundles);

    let payload = document.payload.as_object().expect("payload object");
    let mut applied = AppSettings::default();
    applied.general = serde_json::from_value::<GeneralSettings>(payload["general"].clone())
        .expect("general payload");
    applied.tools =
        serde_json::from_value::<ToolSettings>(payload["tools"].clone()).expect("tools payload");
    applied.eraser = serde_json::from_value::<EraserGroupSettings>(payload["eraser"].clone())
        .expect("eraser payload");
    applied.zoom =
        serde_json::from_value::<ZoomGroupSettings>(payload["zoom"].clone()).expect("zoom payload");
    applied.board = serde_json::from_value::<BoardGroupSettings>(payload["board"].clone())
        .expect("board payload");
    applied.timer = serde_json::from_value::<TimerGroupSettings>(payload["timer"].clone())
        .expect("timer payload");
    applied.display.presets =
        serde_json::from_value::<FixtureDisplayBundle>(payload["display"].clone())
            .expect("display payload")
            .presets;
    applied.shortcuts =
        serde_json::from_value::<ShortcutGroupSettings>(payload["shortcuts"].clone())
            .expect("shortcuts payload");
    applied.help =
        serde_json::from_value::<HelpGroupSettings>(payload["help"].clone()).expect("help payload");

    SettingsValidator::validate(&applied).expect("applied fixture validates");
    assert_eq!(applied, AppSettings::default());
}

// Swift: SettingsTests.decodingIgnoresUnknownKeysAndDefaultsMissingValues
#[test]
fn compatibility_decoding_defaults_migrates_and_filters() {
    let mut compatible = default_json();
    compatible["futureSetting"] = json!({"enabled": true});
    compatible
        .as_object_mut()
        .expect("settings object")
        .remove("display");
    object_at_mut(&mut compatible, "zoom").remove("previewDimming");
    for key in [
        "musicSource",
        "musicFolderPath",
        "musicOrder",
        "musicRepeat",
    ] {
        object_at_mut(&mut compatible, "timer").remove(key);
    }
    let tools = object_at_mut(&mut compatible, "tools");
    tools.insert("penWidth".to_owned(), json!(3));
    tools.insert("highlighterWidth".to_owned(), json!(18));
    tools.insert("textFontSize".to_owned(), json!(24));
    for key in [
        "perToolMemory",
        "favorites",
        "paletteGlassOpacity",
        "pressureMinimumWidthRatio",
    ] {
        tools.remove(key);
    }
    let decoded: AppSettings = serde_json::from_value(compatible).expect("compatible settings");
    assert_eq!(decoded, AppSettings::default());
    assert_eq!(decoded.timer.music_source, "youtube");
    assert_eq!(decoded.timer.music_folder_path, "");
    assert_eq!(decoded.timer.music_order, "name");
    assert_eq!(decoded.timer.music_repeat, "all");

    let mut local_music = AppSettings::default();
    local_music.timer.music_source = "folder".to_owned();
    local_music.timer.music_folder_path = "C:\\Class Music".to_owned();
    local_music.timer.music_order = "shuffle".to_owned();
    local_music.timer.music_repeat = "one".to_owned();
    let encoded = serde_json::to_value(&local_music).expect("local music settings encode");
    assert_eq!(encoded["timer"]["musicSource"], json!("folder"));
    assert_eq!(
        encoded["timer"]["musicFolderPath"],
        json!("C:\\Class Music")
    );
    let round_tripped: AppSettings =
        serde_json::from_value(encoded).expect("local music settings round trip");
    assert_eq!(round_tripped, local_music);
    SettingsValidator::validate(&round_tripped).expect("local music settings validate");

    let mut migrated = default_json();
    let tools = object_at_mut(&mut migrated, "tools");
    tools.remove("perToolMemory");
    tools.insert("penWidth".to_owned(), json!(5));
    tools.insert("highlighterWidth".to_owned(), json!(20));
    tools.insert("textFontSize".to_owned(), json!(30));
    let migrated: AppSettings = serde_json::from_value(migrated).expect("migrated settings");
    assert_eq!(migrated.tools.per_tool_memory.pen.size, 5.0);
    assert_eq!(migrated.tools.per_tool_memory.arrow.size, 5.0);
    assert_eq!(migrated.tools.per_tool_memory.rectangle.size, 5.0);
    assert_eq!(migrated.tools.per_tool_memory.ellipse.size, 5.0);
    assert_eq!(migrated.tools.per_tool_memory.highlighter.size, 20.0);
    assert_eq!(migrated.tools.per_tool_memory.text.size, 30.0);
    SettingsValidator::validate(&migrated).expect("migrated settings validate");

    let mut current_wins = default_json();
    object_at_mut(&mut current_wins, "tools").insert("penWidth".to_owned(), json!("bad"));
    let decoded: AppSettings =
        serde_json::from_value(current_wins).expect("perToolMemory wins over old key");
    assert_eq!(decoded, AppSettings::default());

    let valid = json!({"tool": "pen", "colorIndex": 0, "size": 3});
    let mut favorites = vec![valid.clone(), valid, json!({"tool": "future", "size": 3})];
    for size in 4..=11 {
        favorites.push(json!({"tool": "pen", "colorIndex": 0, "size": size}));
    }
    let mut filtered = default_json();
    object_at_mut(&mut filtered, "tools").insert("favorites".to_owned(), Value::Array(favorites));
    let filtered: AppSettings = serde_json::from_value(filtered).expect("favorite filtering");
    assert_eq!(filtered.tools.favorites.len(), 8);
    assert!(filtered
        .tools
        .favorites
        .iter()
        .all(|favorite| favorite.tool == "pen"));
    SettingsValidator::validate(&filtered).expect("filtered favorites validate");
}

#[test]
fn break_music_playlist_mutations_enforce_limits_and_indices() {
    let valid_a = "https://music.youtube.com/playlist?list=a";
    let valid_b = "https://music.youtube.com/playlist?list=b";
    let cases = [
        ("empty", Vec::new(), 7, true, Vec::new(), 0),
        (
            "valid selection",
            vec![valid_a.to_owned(), valid_b.to_owned()],
            1,
            true,
            vec![valid_a.to_owned(), valid_b.to_owned()],
            1,
        ),
        (
            "exactly five",
            (0..5)
                .map(|index| format!("https://music.youtube.com/playlist?list={index}"))
                .collect(),
            4,
            true,
            (0..5)
                .map(|index| format!("https://music.youtube.com/playlist?list={index}"))
                .collect(),
            4,
        ),
        (
            "index out of range",
            vec![valid_a.to_owned()],
            1,
            false,
            Vec::new(),
            0,
        ),
        (
            "more than five",
            (0..6)
                .map(|index| format!("https://music.youtube.com/playlist?list={index}"))
                .collect(),
            0,
            false,
            Vec::new(),
            0,
        ),
        (
            "invalid url",
            vec!["https://example.com/not-music".to_owned()],
            0,
            false,
            Vec::new(),
            0,
        ),
    ];

    for (name, values, default_index, expected_result, expected_urls, expected_index) in cases {
        let mut timer = TimerGroupSettings::default();
        assert_eq!(
            timer.replace_playlist_urls(values, default_index),
            expected_result,
            "case={name}"
        );
        assert_eq!(timer.playlist_urls, expected_urls, "case={name}");
        assert_eq!(timer.default_playlist_index, expected_index, "case={name}");
    }

    let mut timer = TimerGroupSettings::default();
    assert!(timer.replace_playlist_urls(vec![valid_a.to_owned(), valid_b.to_owned()], 0));
    for (name, index, expected_result, expected_index) in [
        ("negative", -1, false, 0),
        ("valid", 1, true, 1),
        ("too large", 2, false, 1),
    ] {
        assert_eq!(timer.select_playlist(index), expected_result, "case={name}");
        assert_eq!(timer.default_playlist_index, expected_index, "case={name}");
    }
}

// Swift: SettingsTests decoding and SettingsValidator rejection table
#[test]
fn rejects_bad_enums_integers_required_keys_and_ranges() {
    let mut failures: Vec<(&str, Value)> = Vec::new();

    let mut value = default_json();
    object_at_mut(&mut value, "tools").insert("paletteStyle".to_owned(), json!("x"));
    failures.push(("unknown palette style", value));

    let mut value = default_json();
    object_at_mut(&mut value, "shortcuts").insert("appModifier".to_owned(), json!("x"));
    failures.push(("unknown app modifier", value));

    for (name, vendor) in [
        ("negative vendor", json!(-1)),
        ("overflow vendor", json!(4_294_967_296_u64)),
    ] {
        let mut value = default_json();
        value["display"]["presets"] = Value::Array(vec![preset_json()]);
        value["display"]["presets"][0]["main"]["monitors"][0]["identity"]["vendor"] = vendor;
        failures.push((name, value));
    }

    let mut value = default_json();
    value["display"]["presets"] = Value::Array(vec![preset_json()]);
    value["display"]["presets"][0]["main"]["monitors"][0]["identity"]["kind"] = json!("x");
    failures.push(("unknown display kind", value));

    let mut value = default_json();
    value["display"]["presets"] = Value::Array(vec![preset_json()]);
    value["display"]["presets"][0]["id"] = json!("not-a-uuid");
    failures.push(("invalid preset id", value));

    let mut value = default_json();
    value
        .as_object_mut()
        .expect("settings object")
        .remove("general");
    failures.push(("missing required top-level group", value));

    let mut value = default_json();
    object_at_mut(&mut value, "tools").remove("colors");
    failures.push(("missing ordinary required field", value));

    let mut value = default_json();
    object_at_mut(&mut value, "tools")
        .get_mut("perToolMemory")
        .and_then(Value::as_object_mut)
        .expect("memory object")
        .remove("text");
    failures.push(("partial per-tool memory", value));

    let mut value = default_json();
    object_at_mut(&mut value, "display").remove("presets");
    failures.push(("partial display group", value));

    let mut value = default_json();
    value["display"]["presets"] = Value::Array(vec![preset_json()]);
    value["display"]["presets"][0]["main"]
        .as_object_mut()
        .expect("main screen")
        .remove("cell");
    failures.push(("missing screen cell", value));

    for (name, value) in failures {
        assert!(
            serde_json::from_value::<AppSettings>(value).is_err(),
            "case={name}"
        );
    }

    for pointer in [
        "/tools/pressureMinimumWidthRatio",
        "/tools/perToolMemory",
        "/tools/favorites",
        "/tools/paletteStyle",
        "/tools/paletteGlassOpacity",
        "/zoom/previewDimming",
        "/display",
    ] {
        let mut value = default_json();
        *value.pointer_mut(pointer).expect("defaultable field") = Value::Null;
        let decoded: AppSettings = serde_json::from_value(value)
            .unwrap_or_else(|error| panic!("null default failed for {pointer}: {error}"));
        assert_eq!(decoded, AppSettings::default(), "pointer={pointer}");
    }

    let mut out_of_range = AppSettings::default();
    out_of_range.zoom.maximum_magnification = 9.0;
    assert!(SettingsValidator::validate(&out_of_range).is_err());

    for (name, key, invalid_value) in [
        ("unknown music source", "musicSource", "stream"),
        ("unknown music order", "musicOrder", "random"),
        ("unknown music repeat", "musicRepeat", "none"),
        (
            "control in music folder",
            "musicFolderPath",
            "C:\\Music\nOther",
        ),
        ("network music folder", "musicFolderPath", r"\\host\share"),
        (
            "mixed network music folder",
            "musicFolderPath",
            r"/\host\share",
        ),
        ("NT device music folder", "musicFolderPath", r"\??\UNC\h\s"),
    ] {
        let mut value = default_json();
        object_at_mut(&mut value, "timer").insert(key.to_owned(), json!(invalid_value));
        let decoded: AppSettings = serde_json::from_value(value).expect("music setting decode");
        assert!(
            SettingsValidator::validate(&decoded).is_err(),
            "case={name}"
        );
    }

    let mut lowercase_id = default_json();
    let mut preset = preset_json();
    preset["id"] = json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    lowercase_id["display"]["presets"] = Value::Array(vec![preset]);
    let decoded: AppSettings = serde_json::from_value(lowercase_id).expect("lowercase UUID");
    SettingsValidator::validate(&decoded).expect("lowercase UUID settings validate");
    let encoded = serde_json::to_value(decoded).expect("serialize normalized UUID");
    assert_eq!(
        encoded["display"]["presets"][0]["id"],
        json!("AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE")
    );
}
