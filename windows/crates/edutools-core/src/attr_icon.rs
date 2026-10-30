use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

// Swift: AttrIconConstants
pub struct AttrIconConstants;

impl AttrIconConstants {
    pub const MINIMUM_CONTRAST_RATIO: f64 = 1.8;
    pub const DARK_HALO_ALPHA: f64 = 0.55;
    pub const LIGHT_HALO_ALPHA: f64 = 0.7;
    pub const LIGHT_BACKGROUND_LUMINANCE: f64 = 0.5;
    pub const RADIAL_BACKDROP_WHITE_COMPONENT: f64 = 0.07;
}

// Swift: AttrIconColor
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttrIconColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl AttrIconColor {
    // Swift: AttrIconColor.init(red:green:blue:alpha:)
    pub const fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

// Swift: AttrIconPure.ConditionError
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttrIconConditionError {
    Malformed(String),
    UnknownAttribute(String),
}

// Swift: AttrIconPure.ConditionError error description
impl fmt::Display for AttrIconConditionError {
    // Swift: AttrIconPure.ConditionError error description
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(expression) => write!(formatter, "malformed condition: {expression}"),
            Self::UnknownAttribute(attribute) => {
                write!(formatter, "unknown condition attribute: {attribute}")
            }
        }
    }
}

impl Error for AttrIconConditionError {}

// Swift: AttrIconToolCatalog.manifestKeyByTool
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AttrIconTool {
    Pen,
    Highlighter,
    Text,
    Arrow,
    Rectangle,
    Ellipse,
    Eraser,
    Laser,
}

impl AttrIconTool {
    // Swift: AttrIconToolCatalog.manifestKeyByTool
    pub const fn manifest_key(self) -> &'static str {
        match self {
            Self::Pen => "tool.pen",
            Self::Highlighter => "tool.highlighter",
            Self::Text => "tool.text",
            Self::Arrow => "tool.arrow",
            Self::Rectangle => "tool.rect",
            Self::Ellipse => "tool.circle",
            Self::Eraser => "tool.eraser",
            Self::Laser => "tool.laser",
        }
    }
}

// Swift: AttrIconConfiguration
#[derive(Clone, Debug, PartialEq)]
pub struct AttrIconConfiguration {
    pub tool_key: String,
    pub attributes: BTreeMap<String, String>,
    pub color: Option<AttrIconColor>,
}

// Swift: AttrIcon layer colorKind
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttrIconPaint {
    Ui,
    Attr,
}

// Swift: AttrIcon.image layers
#[derive(Clone, Debug, PartialEq)]
pub struct AttrIconLayer {
    pub name: String,
    pub paint: AttrIconPaint,
    pub halo: Option<AttrIconColor>,
}

// Swift: AttrIconColor, named for the cross-platform layer contract
pub type Rgba = AttrIconColor;

// Swift: AttrIcon layer colorKind, named for the cross-platform layer contract
pub type Paint = AttrIconPaint;

// Swift: AttrIcon.image layers, named for the cross-platform layer contract
pub type Layer = AttrIconLayer;

// Swift: AttrIcon manifest/render fallback reasons
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttrIconError {
    InvalidManifest(String),
    UnknownTool(String),
    MissingAttribute(String),
    InvalidCondition(AttrIconConditionError),
    UnresolvedLayer(String),
    UnknownPaint(String),
}

// Swift: AttrIcon logOnce fallback descriptions
impl fmt::Display for AttrIconError {
    // Swift: AttrIcon logOnce fallback descriptions
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(reason) => {
                write!(formatter, "invalid attr icon manifest: {reason}")
            }
            Self::UnknownTool(tool) => write!(formatter, "unknown attr icon tool: {tool}"),
            Self::MissingAttribute(attribute) => {
                write!(formatter, "missing attr icon attribute: {attribute}")
            }
            Self::InvalidCondition(error) => error.fmt(formatter),
            Self::UnresolvedLayer(template) => {
                write!(formatter, "unresolved attr icon layer: {template}")
            }
            Self::UnknownPaint(paint) => write!(formatter, "unknown attr icon paint: {paint}"),
        }
    }
}

impl Error for AttrIconError {}

// Swift: AttrIcon.Manifest
#[derive(Deserialize)]
struct Manifest {
    tools: BTreeMap<String, ToolDefinition>,
}

// Swift: AttrIcon.ToolDefinition
#[derive(Deserialize)]
struct ToolDefinition {
    attrs: Vec<String>,
    layers: Vec<LayerDefinition>,
}

// Swift: AttrIcon.LayerDefinition
struct LayerDefinition {
    name_template: String,
    color_kind: String,
    condition: Option<String>,
}

// Swift: AttrIcon.LayerDefinition.init(from:)
impl<'de> Deserialize<'de> for LayerDefinition {
    // Swift: AttrIcon.LayerDefinition.init(from:)
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Swift: AttrIcon.LayerDefinition unkeyed manifest visitor
        struct LayerVisitor;

        impl<'de> Visitor<'de> for LayerVisitor {
            type Value = LayerDefinition;

            // Swift: AttrIcon.LayerDefinition decode error description
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an attr icon layer with two or three string values")
            }

            // Swift: AttrIcon.LayerDefinition.init(from:)
            fn visit_seq<A>(self, mut values: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let name_template = values
                    .next_element::<String>()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let color_kind = values
                    .next_element::<String>()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let condition = values.next_element::<String>()?;
                if values.next_element::<String>()?.is_some() {
                    return Err(de::Error::invalid_length(4, &self));
                }
                Ok(LayerDefinition {
                    name_template,
                    color_kind,
                    condition,
                })
            }
        }

        deserializer.deserialize_seq(LayerVisitor)
    }
}

// Swift: AttrIconPure
pub struct AttrIconPure;

impl AttrIconPure {
    // Swift: AttrIconPure.widthLevel(value:lowerBound:upperBound:)
    pub fn width_level(value: f64, lower_bound: f64, upper_bound: f64) -> i64 {
        if upper_bound <= lower_bound {
            return 2;
        }
        let normalized = ((value - lower_bound) / (upper_bound - lower_bound)).clamp(0.0, 1.0);
        1 + ((4.0 * normalized).floor() as i64).min(3)
    }

    // Swift: AttrIconPure.conditionMatches(_:attributes:)
    pub fn condition_matches(
        expression: &str,
        attributes: &BTreeMap<String, String>,
    ) -> Result<bool, AttrIconConditionError> {
        let condition = expression.trim();
        if condition.is_empty() {
            return Err(AttrIconConditionError::Malformed(expression.to_owned()));
        }

        if let Some((name, expected)) = condition.split_once("!=") {
            Self::validate_comparison(expression, name, expected)?;
            return attributes
                .get(name)
                .map(|actual| actual != expected)
                .ok_or_else(|| AttrIconConditionError::UnknownAttribute(name.to_owned()));
        }
        if let Some((name, expected)) = condition.split_once('=') {
            Self::validate_comparison(expression, name, expected)?;
            return attributes
                .get(name)
                .map(|actual| actual == expected)
                .ok_or_else(|| AttrIconConditionError::UnknownAttribute(name.to_owned()));
        }
        if let Some(name) = condition.strip_prefix('!') {
            if !Self::is_identifier(name) {
                return Err(AttrIconConditionError::Malformed(expression.to_owned()));
            }
            return attributes
                .get(name)
                .map(|actual| actual == "false")
                .ok_or_else(|| AttrIconConditionError::UnknownAttribute(name.to_owned()));
        }
        if !Self::is_identifier(condition) {
            return Err(AttrIconConditionError::Malformed(expression.to_owned()));
        }
        attributes
            .get(condition)
            .map(|actual| actual == "true")
            .ok_or_else(|| AttrIconConditionError::UnknownAttribute(condition.to_owned()))
    }

    // Swift: AttrIconPure.relativeLuminance(_:)
    pub fn relative_luminance(color: AttrIconColor) -> f64 {
        // Swift: AttrIconPure.relativeLuminance(_:).linearized
        fn linearized(component: f64) -> f64 {
            let clamped = component.clamp(0.0, 1.0);
            if clamped <= 0.03928 {
                clamped / 12.92
            } else {
                ((clamped + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * linearized(color.red)
            + 0.7152 * linearized(color.green)
            + 0.0722 * linearized(color.blue)
    }

    // Swift: AttrIconPure.contrastRatio(_:_:)
    pub fn contrast_ratio(first: AttrIconColor, second: AttrIconColor) -> f64 {
        let first_luminance = Self::relative_luminance(first);
        let second_luminance = Self::relative_luminance(second);
        let lighter = first_luminance.max(second_luminance);
        let darker = first_luminance.min(second_luminance);
        (lighter + 0.05) / (darker + 0.05)
    }

    // Swift: AttrIconPure.needsHalo(foreground:background:)
    pub fn needs_halo(foreground: AttrIconColor, background: AttrIconColor) -> bool {
        Self::contrast_ratio(foreground, background) < AttrIconConstants::MINIMUM_CONTRAST_RATIO
    }

    // Swift: RadialPaletteArcStyle.estimatedBackdropColor(selected: false)
    pub const fn estimated_radial_backdrop_color() -> AttrIconColor {
        let value = AttrIconConstants::RADIAL_BACKDROP_WHITE_COMPONENT;
        AttrIconColor::new(value, value, value, 1.0)
    }

    // Swift: AttrIcon.image(configuration:pointSize:uiColor:backgroundColor:visualState:accessibilityDescription:)
    pub fn layers(
        manifest_json: &str,
        configuration: &AttrIconConfiguration,
        ui_color: AttrIconColor,
        background_color: AttrIconColor,
    ) -> Result<Vec<AttrIconLayer>, AttrIconError> {
        let manifest: Manifest = serde_json::from_str(manifest_json)
            .map_err(|error| AttrIconError::InvalidManifest(error.to_string()))?;
        let tool = manifest
            .tools
            .get(&configuration.tool_key)
            .ok_or_else(|| AttrIconError::UnknownTool(configuration.tool_key.clone()))?;

        for required in &tool.attrs {
            let present = if required == "color" {
                configuration.color.is_some()
            } else {
                configuration.attributes.contains_key(required)
            };
            if !present {
                return Err(AttrIconError::MissingAttribute(required.clone()));
            }
        }

        let mut result = Vec::new();
        for layer in &tool.layers {
            if let Some(condition) = layer.condition.as_deref() {
                let matches = Self::condition_matches(condition, &configuration.attributes)
                    .map_err(AttrIconError::InvalidCondition)?;
                if !matches {
                    continue;
                }
            }
            let name = Self::resolved_layer_name(&layer.name_template, &configuration.attributes)?;
            let (paint, color) = match layer.color_kind.as_str() {
                "ui" => (AttrIconPaint::Ui, ui_color),
                "attr" => (
                    AttrIconPaint::Attr,
                    configuration
                        .color
                        .ok_or_else(|| AttrIconError::MissingAttribute("color".to_owned()))?,
                ),
                other => return Err(AttrIconError::UnknownPaint(other.to_owned())),
            };
            let halo = (paint == AttrIconPaint::Attr
                && !name.ends_with(".fill")
                && Self::needs_halo(color, background_color))
            .then(|| Self::halo_color(background_color));
            result.push(AttrIconLayer { name, paint, halo });
        }
        Ok(result)
    }

    // Swift: AttrIcon.conditionMatches comparison validation
    fn validate_comparison(
        expression: &str,
        name: &str,
        expected: &str,
    ) -> Result<(), AttrIconConditionError> {
        if Self::is_identifier(name) && Self::is_identifier(expected) {
            Ok(())
        } else {
            Err(AttrIconConditionError::Malformed(expression.to_owned()))
        }
    }

    // Swift: AttrIconPure.isIdentifier(_:)
    fn is_identifier(value: &str) -> bool {
        let mut characters = value.chars();
        let Some(first) = characters.next() else {
            return false;
        };
        (first.is_alphabetic() || first == '_')
            && characters.all(|character| {
                character.is_alphanumeric() || character == '_' || character == '-'
            })
    }

    // Swift: AttrIcon.resolvedLayerName(template:attributes:)
    fn resolved_layer_name(
        template: &str,
        attributes: &BTreeMap<String, String>,
    ) -> Result<String, AttrIconError> {
        let mut result = template.to_owned();
        for (name, value) in attributes {
            result = result.replace(&format!("{{{name}}}"), value);
        }
        if result.contains('{') || result.contains('}') {
            Err(AttrIconError::UnresolvedLayer(template.to_owned()))
        } else {
            Ok(result)
        }
    }

    // Swift: AttrIcon.haloColor(for:)
    fn halo_color(background: AttrIconColor) -> AttrIconColor {
        if Self::relative_luminance(background) >= AttrIconConstants::LIGHT_BACKGROUND_LUMINANCE {
            AttrIconColor::new(0.0, 0.0, 0.0, AttrIconConstants::DARK_HALO_ALPHA)
        } else {
            AttrIconColor::new(1.0, 1.0, 1.0, AttrIconConstants::LIGHT_HALO_ALPHA)
        }
    }
}
