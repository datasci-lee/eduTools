use edutools_core::attr_icon::{AttrIconColor, AttrIconConditionError, AttrIconPure};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

// Swift: AttrIconWidthVectorFile
#[derive(Deserialize)]
struct WidthVectorFile {
    version: u32,
    cases: Vec<WidthVectorCase>,
}

// Swift: AttrIconWidthVectorCase
#[derive(Deserialize)]
struct WidthVectorCase {
    name: String,
    input: WidthInput,
    expected: i64,
}

// Swift: AttrIconWidthVectorCase.Input
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WidthInput {
    value: f64,
    lower_bound: f64,
    upper_bound: f64,
}

// Swift: AttrIconConditionVectorFile
#[derive(Deserialize)]
struct ConditionVectorFile {
    version: u32,
    cases: Vec<ConditionVectorCase>,
}

// Swift: AttrIconConditionVectorCase
#[derive(Deserialize)]
struct ConditionVectorCase {
    name: String,
    input: ConditionInput,
    expected: ConditionExpected,
}

// Swift: AttrIconConditionVectorCase.Input
#[derive(Deserialize)]
struct ConditionInput {
    expression: String,
    attributes: BTreeMap<String, String>,
}

// Swift: AttrIconConditionVectorCase.Expected
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConditionExpected {
    matches: Option<bool>,
    unknown_attribute: Option<String>,
}

// Swift: AttrIconContrastVectorFile
#[derive(Deserialize)]
struct ContrastVectorFile {
    version: u32,
    cases: Vec<ContrastVectorCase>,
}

// Swift: AttrIconContrastVectorCase
#[derive(Deserialize)]
struct ContrastVectorCase {
    name: String,
    input: ContrastInput,
    expected: ContrastExpected,
}

// Swift: AttrIconContrastVectorCase.Input
#[derive(Deserialize)]
struct ContrastInput {
    foreground: Color,
    background: Color,
}

// Swift: AttrIconContrastVectorCase.Color
#[derive(Clone, Copy, Deserialize)]
struct Color {
    red: f64,
    green: f64,
    blue: f64,
}

impl Color {
    // Swift: AttrIconContrastVectorCase.Color.attrIconColor
    fn attr_icon_color(self) -> AttrIconColor {
        AttrIconColor::new(self.red, self.green, self.blue, 1.0)
    }
}

// Swift: AttrIconContrastVectorCase.Expected
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContrastExpected {
    ratio: f64,
    tolerance: f64,
    needs_halo: bool,
}

// Swift: TestSupport.Fixtures URL lookup
fn repository_fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

// Swift: TestSupport.Fixtures.decode(_:from:)
fn decode_fixture<T: for<'de> Deserialize<'de>>(relative: &str) -> T {
    let data = fs::read_to_string(repository_fixture(relative)).expect("read vector fixture");
    serde_json::from_str(&data).expect("decode vector fixture")
}

// Swift: AttrIconTests.widthLevelsFollowVector(_:)
#[test]
fn width_levels_follow_vector() {
    let fixture: WidthVectorFile = decode_fixture("fixtures/vectors/attr-icon-width-levels.json");
    assert_eq!(fixture.version, 1);

    for case in fixture.cases {
        assert_eq!(
            AttrIconPure::width_level(
                case.input.value,
                case.input.lower_bound,
                case.input.upper_bound,
            ),
            case.expected,
            "case={}",
            case.name
        );
    }
}

// Swift: AttrIconTests.conditionsFollowVector(_:)
#[test]
fn conditions_follow_vector() {
    let fixture: ConditionVectorFile = decode_fixture("fixtures/vectors/attr-icon-conditions.json");
    assert_eq!(fixture.version, 1);

    for case in fixture.cases {
        let actual =
            AttrIconPure::condition_matches(&case.input.expression, &case.input.attributes);
        if let Some(expected) = case.expected.matches {
            assert_eq!(case.expected.unknown_attribute, None, "case={}", case.name);
            assert_eq!(actual, Ok(expected), "case={}", case.name);
        } else {
            let attribute = case
                .expected
                .unknown_attribute
                .expect("unknown attribute expectation");
            assert_eq!(
                actual,
                Err(AttrIconConditionError::UnknownAttribute(attribute)),
                "case={}",
                case.name
            );
        }
    }
}

// Swift: AttrIconTests.contrastFollowsVector(_:)
#[test]
fn contrast_follows_vector() {
    let fixture: ContrastVectorFile = decode_fixture("fixtures/vectors/attr-icon-contrast.json");
    assert_eq!(fixture.version, 1);

    for case in fixture.cases {
        let foreground = case.input.foreground.attr_icon_color();
        let background = case.input.background.attr_icon_color();
        let ratio = AttrIconPure::contrast_ratio(foreground, background);
        assert!(
            (ratio - case.expected.ratio).abs() < case.expected.tolerance,
            "case={}, actual={ratio}",
            case.name
        );
        assert_eq!(
            AttrIconPure::needs_halo(foreground, background),
            case.expected.needs_halo,
            "case={}",
            case.name
        );
    }
}
