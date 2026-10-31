use edutools_core::settings::{AppModifierSetting, ShortcutGroupSettings};
use edutools_core::shortcut::{
    PortableShortcut, ShortcutBinding, ShortcutBindingError, ShortcutCatalog,
    ShortcutConflictDetector, ShortcutConflictTable, ShortcutModifiers, ShortcutScope,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

// Swift: ShortcutDefaultsFixture
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ShortcutDefaultsFixture {
    format_version: u32,
    bindings: BTreeMap<String, String>,
    unbound: Vec<String>,
}

// Swift: ShortcutTests.appModPContract, appModSlashContract, invalidSerializedValueIsRejected
#[test]
fn portable_tokens_and_scope_rules_match_swift_contracts() {
    let app_mod_p = PortableShortcut::parse("AppMod+P").expect("AppMod+P must parse");
    assert_eq!(app_mod_p.serialized(), "AppMod+P");
    assert_eq!(
        app_mod_p.display_string(AppModifierSetting::ControlOption),
        "Ctrl+Alt+P"
    );

    let app_mod_slash = PortableShortcut::parse("AppMod+/").expect("AppMod+/ must parse");
    assert_eq!(app_mod_slash.serialized(), "AppMod+/");

    for invalid in [
        "",
        "P",
        "AppMod+",
        "AppMod+p",
        "AppMod+PP",
        "AppMod+P+Z",
        "Control+Option+P",
    ] {
        assert!(
            ShortcutBinding::make(
                ShortcutScope::Global,
                invalid,
                AppModifierSetting::ControlOption
            )
            .is_err(),
            "global shortcut must reject {invalid:?}"
        );
    }

    assert!(ShortcutBinding::make(
        ShortcutScope::Annotation,
        "P",
        AppModifierSetting::ControlOption
    )
    .is_ok());
    assert_eq!(
        ShortcutBinding::make(
            ShortcutScope::Global,
            "P",
            AppModifierSetting::ControlOption
        ),
        Err(ShortcutBindingError::GlobalRequiresAppModifier)
    );

    let reordered = PortableShortcut::parse("Primary+Shift+Option+Control+P")
        .expect("modifier input order must not matter");
    assert_eq!(reordered.serialized(), "Control+Option+Shift+Primary+P");
}

// Swift: ShortcutTests.catalogBindingsAreUniqueWithinScope
#[test]
fn catalog_matches_shared_fixture_and_is_unique_within_scope() {
    let definitions = ShortcutCatalog::definitions();
    let bindings = definitions
        .iter()
        .flat_map(|definition| {
            definition.bindings.iter().map(|binding| {
                (
                    ShortcutCatalog::override_key(&definition.id, binding.scope),
                    binding.portable_string.clone(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut unbound = definitions
        .iter()
        .filter(|definition| definition.bindings.is_empty())
        .map(|definition| definition.id.clone())
        .collect::<Vec<_>>();
    unbound.sort();

    let fixture: ShortcutDefaultsFixture =
        serde_json::from_str(include_str!("../../../../fixtures/shortcuts-default.json"))
            .expect("shared shortcut fixture must decode");
    let uniqueness = definitions
        .iter()
        .flat_map(|definition| {
            definition
                .bindings
                .iter()
                .map(ShortcutBinding::uniqueness_key)
        })
        .collect::<Vec<_>>();

    assert_eq!(definitions.len(), 58);
    assert_eq!(uniqueness.len(), 48);
    assert_eq!(
        uniqueness.iter().collect::<HashSet<_>>().len(),
        uniqueness.len()
    );
    assert_eq!(fixture.format_version, 1);
    assert_eq!(bindings, fixture.bindings);
    assert_eq!(unbound.len(), 14);
    assert_eq!(unbound, fixture.unbound);
}

// Swift: ShortcutPresentationTests.helpFilterMatchesKoreanFeatureName and helpFilterMatchesDisplayedKey
#[test]
fn help_filter_matches_korean_name_and_displayed_key() {
    let definitions = ShortcutCatalog::definitions();
    let korean = ShortcutCatalog::filtered(&definitions, "형광펜");
    let displayed_key = ShortcutCatalog::filtered(&definitions, "[");

    assert_eq!(
        korean
            .iter()
            .map(|definition| definition.id.as_str())
            .collect::<Vec<_>>(),
        ["tool.highlighter"]
    );
    assert_eq!(
        displayed_key
            .iter()
            .map(|definition| definition.id.as_str())
            .collect::<Vec<_>>(),
        ["tool.width.decrease"]
    );
}

// Swift: SettingsTests.avoidedGlobalShortcutIsReportedAsConflict
#[test]
fn avoided_global_shortcut_uses_replaceable_platform_table() {
    let macos = ShortcutConflictTable::macos_compatibility();
    assert!(ShortcutConflictDetector::has_avoided_conflict(
        "AppMod+D", &macos
    ));

    let windows = ShortcutConflictTable::new(&["Q"], &["Primary+Tab"]);
    assert!(ShortcutConflictDetector::has_avoided_conflict(
        "AppMod+Q", &windows
    ));
    assert!(!ShortcutConflictDetector::has_avoided_conflict(
        "AppMod+D", &windows
    ));

    let annotation_p = ShortcutBinding::make(
        ShortcutScope::Annotation,
        "P",
        AppModifierSetting::ControlOption,
    )
    .expect("annotation P must be valid");
    assert!(annotation_p.matches("P", ShortcutModifiers::empty()));
    assert!(!annotation_p.matches("P", ShortcutModifiers::SHIFT));

    let mut settings = ShortcutGroupSettings::default();
    settings
        .overrides
        .insert("annotation.toggle|global".to_owned(), "AppMod+D".to_owned());
    let warnings = ShortcutConflictDetector::warnings(&settings, &macos);
    assert!(warnings.contains_key("annotation.toggle|global"));
}
