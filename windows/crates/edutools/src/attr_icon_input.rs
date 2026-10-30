#![cfg_attr(not(windows), allow(dead_code))]

use edutools_core::attr_icon::{AttrIconColor, AttrIconConfiguration, AttrIconPure, AttrIconTool};
use edutools_core::eraser::{EraserAreaShape, EraserMode};
use edutools_core::ink::{ArrowStyle, InkTool};
use edutools_core::radial::{RadialColor, RadialPaletteState};
use edutools_core::settings::{AppSettings, FavoriteTool, SettingsColor};
use edutools_core::state::AppState;
use std::collections::BTreeMap;

const WRITING_RANGE: (f64, f64) = (1.0, 12.0);
const HIGHLIGHTER_RANGE: (f64, f64) = (6.0, 30.0);
const TEXT_RANGE: (f64, f64) = (16.0, 72.0);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AttrIconVisualState {
    pub is_selected: bool,
    pub is_enabled: bool,
    pub is_hovered: bool,
    pub is_pass_through: bool,
    pub is_high_contrast: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttrIconSurface {
    Radial,
    Linear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemAppearance {
    Light,
    Dark,
}

#[derive(Clone, Copy)]
struct AttrIconSnapshot {
    tool: AttrIconTool,
    color: Option<AttrIconColor>,
    size: f64,
    arrow_style: Option<ArrowStyle>,
    eraser_mode: Option<EraserMode>,
    eraser_area_shape: Option<EraserAreaShape>,
}

pub fn configuration_for_tool(
    tool: InkTool,
    state: &AppState,
    settings: &AppSettings,
) -> Option<AttrIconConfiguration> {
    let attr_tool = attr_tool(tool)?;
    let snapshot = match tool {
        InkTool::Pen
        | InkTool::Highlighter
        | InkTool::Arrow
        | InkTool::Rectangle
        | InkTool::Ellipse
        | InkTool::Text => {
            let memory = tool.memory_slot()?.setting(&settings.tools.per_tool_memory);
            let color_index = if tool == state.current_tool() {
                state.selected_color_index(settings)
            } else {
                memory.color_index
            };
            AttrIconSnapshot {
                tool: attr_tool,
                color: settings_color(
                    settings
                        .tools
                        .colors
                        .get(usize::try_from(color_index).ok()?)?,
                ),
                size: state.width_for(tool, settings)?,
                arrow_style: (tool == InkTool::Arrow).then(|| state.arrow_style()),
                eraser_mode: None,
                eraser_area_shape: None,
            }
        }
        InkTool::Eraser => AttrIconSnapshot {
            tool: attr_tool,
            color: None,
            size: settings.eraser.diameter,
            arrow_style: None,
            eraser_mode: parse_eraser_mode(&settings.eraser.mode),
            eraser_area_shape: parse_eraser_area_shape(&settings.eraser.area_shape),
        },
        InkTool::Laser => AttrIconSnapshot {
            tool: attr_tool,
            color: settings_color(&settings.tools.laser_color),
            size: settings.tools.laser_trail_duration,
            arrow_style: None,
            eraser_mode: None,
            eraser_area_shape: None,
        },
        InkTool::Select | InkTool::Cover => return None,
    };
    configuration(snapshot, &settings.eraser.preset_diameters)
}

pub fn configuration_for_radial_tool(
    tool: InkTool,
    state: &RadialPaletteState,
    arrow_style: ArrowStyle,
) -> Option<AttrIconConfiguration> {
    let attr_tool = attr_tool(tool)?;
    let snapshot = match tool {
        InkTool::Pen
        | InkTool::Highlighter
        | InkTool::Arrow
        | InkTool::Rectangle
        | InkTool::Ellipse
        | InkTool::Text => {
            let memory = tool.memory_slot()?.setting(&state.per_tool_memory);
            let color_index = if tool == state.current_tool {
                state.selected_color_index
            } else {
                usize::try_from(memory.color_index).ok()?
            };
            AttrIconSnapshot {
                tool: attr_tool,
                color: radial_color(*state.color_presets.get(color_index)?),
                size: state.remembered_width(tool)?,
                arrow_style: (tool == InkTool::Arrow).then_some(arrow_style),
                eraser_mode: None,
                eraser_area_shape: None,
            }
        }
        InkTool::Eraser => AttrIconSnapshot {
            tool: attr_tool,
            color: None,
            size: state.eraser_diameter,
            arrow_style: None,
            eraser_mode: Some(state.eraser_mode),
            eraser_area_shape: Some(state.eraser_area_shape),
        },
        InkTool::Laser => AttrIconSnapshot {
            tool: attr_tool,
            color: radial_color(state.laser_color),
            size: state.laser_size,
            arrow_style: None,
            eraser_mode: None,
            eraser_area_shape: None,
        },
        InkTool::Select | InkTool::Cover => return None,
    };
    configuration(snapshot, &state.eraser_preset_diameters)
}

pub fn configuration_for_radial_favorite(
    favorite: &FavoriteTool,
    state: &RadialPaletteState,
) -> Option<AttrIconConfiguration> {
    let colors = state
        .color_presets
        .iter()
        .copied()
        .map(radial_color)
        .collect::<Option<Vec<_>>>()?;
    configuration_for_favorite_colors(favorite, &colors, &state.eraser_preset_diameters)
}

fn configuration_for_favorite_colors(
    favorite: &FavoriteTool,
    colors: &[AttrIconColor],
    eraser_presets: &[f64],
) -> Option<AttrIconConfiguration> {
    let tool = favorite_attr_tool(&favorite.tool)?;
    let snapshot = AttrIconSnapshot {
        tool,
        color: favorite
            .color_index
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| colors.get(index).copied()),
        size: favorite.size,
        arrow_style: favorite.arrow_style.as_deref().and_then(parse_arrow_style),
        eraser_mode: favorite.eraser_mode.as_deref().and_then(parse_eraser_mode),
        eraser_area_shape: favorite
            .eraser_area_shape
            .as_deref()
            .and_then(parse_eraser_area_shape),
    };
    configuration(snapshot, eraser_presets)
}

fn configuration(
    snapshot: AttrIconSnapshot,
    eraser_presets: &[f64],
) -> Option<AttrIconConfiguration> {
    let mut attributes = BTreeMap::new();
    let color = snapshot.color;
    match snapshot.tool {
        AttrIconTool::Pen
        | AttrIconTool::Arrow
        | AttrIconTool::Rectangle
        | AttrIconTool::Ellipse => {
            color?;
            attributes.insert(
                "width".to_owned(),
                width_level(snapshot.size, WRITING_RANGE).to_string(),
            );
            match snapshot.tool {
                AttrIconTool::Arrow => {
                    attributes.insert(
                        "heads".to_owned(),
                        match snapshot.arrow_style? {
                            ArrowStyle::Single => "single",
                            ArrowStyle::Double => "double",
                        }
                        .to_owned(),
                    );
                }
                AttrIconTool::Rectangle | AttrIconTool::Ellipse => {
                    attributes.insert("fill".to_owned(), "false".to_owned());
                }
                _ => {}
            }
        }
        AttrIconTool::Highlighter => {
            color?;
            attributes.insert(
                "width".to_owned(),
                width_level(snapshot.size, HIGHLIGHTER_RANGE).to_string(),
            );
        }
        AttrIconTool::Text => {
            color?;
            attributes.insert(
                "width".to_owned(),
                width_level(snapshot.size, TEXT_RANGE).to_string(),
            );
        }
        AttrIconTool::Eraser => {
            let lower = eraser_presets.iter().copied().reduce(f64::min)?;
            let upper = eraser_presets.iter().copied().reduce(f64::max)?;
            attributes.insert(
                "width".to_owned(),
                AttrIconPure::width_level(snapshot.size, lower, upper).to_string(),
            );
            let mode = match snapshot.eraser_mode? {
                EraserMode::Partial => "brush",
                EraserMode::Stroke => "stroke",
                EraserMode::Area => match snapshot.eraser_area_shape? {
                    EraserAreaShape::Rectangle => "rect",
                    EraserAreaShape::Lasso => "lasso",
                },
            };
            attributes.insert("mode".to_owned(), mode.to_owned());
        }
        AttrIconTool::Laser => {
            color?;
            attributes.insert("trail".to_owned(), "true".to_owned());
        }
    }
    Some(AttrIconConfiguration {
        tool_key: snapshot.tool.manifest_key().to_owned(),
        attributes,
        color,
    })
}

pub fn estimated_background(
    surface: AttrIconSurface,
    selected: bool,
    appearance: SystemAppearance,
    accent: AttrIconColor,
) -> AttrIconColor {
    if selected {
        return accent;
    }
    let white = match (surface, appearance) {
        (AttrIconSurface::Radial, _) => 0.07,
        (AttrIconSurface::Linear, SystemAppearance::Light) => 0.93,
        (AttrIconSurface::Linear, SystemAppearance::Dark) => 0.18,
    };
    AttrIconColor::new(white, white, white, 1.0)
}

fn width_level(value: f64, range: (f64, f64)) -> i64 {
    AttrIconPure::width_level(value, range.0, range.1)
}

fn attr_tool(tool: InkTool) -> Option<AttrIconTool> {
    match tool {
        InkTool::Pen => Some(AttrIconTool::Pen),
        InkTool::Highlighter => Some(AttrIconTool::Highlighter),
        InkTool::Laser => Some(AttrIconTool::Laser),
        InkTool::Eraser => Some(AttrIconTool::Eraser),
        InkTool::Arrow => Some(AttrIconTool::Arrow),
        InkTool::Rectangle => Some(AttrIconTool::Rectangle),
        InkTool::Ellipse => Some(AttrIconTool::Ellipse),
        InkTool::Text => Some(AttrIconTool::Text),
        InkTool::Select | InkTool::Cover => None,
    }
}

fn favorite_attr_tool(tool: &str) -> Option<AttrIconTool> {
    match tool {
        "pen" => Some(AttrIconTool::Pen),
        "highlighter" => Some(AttrIconTool::Highlighter),
        "laser" => Some(AttrIconTool::Laser),
        "eraser" => Some(AttrIconTool::Eraser),
        "arrow" => Some(AttrIconTool::Arrow),
        "rectangle" => Some(AttrIconTool::Rectangle),
        "ellipse" => Some(AttrIconTool::Ellipse),
        "text" => Some(AttrIconTool::Text),
        "select" | "cover" => None,
        _ => None,
    }
}

fn parse_arrow_style(value: &str) -> Option<ArrowStyle> {
    match value {
        "single" => Some(ArrowStyle::Single),
        "double" => Some(ArrowStyle::Double),
        _ => None,
    }
}

fn parse_eraser_mode(value: &str) -> Option<EraserMode> {
    match value {
        "partial" => Some(EraserMode::Partial),
        "stroke" => Some(EraserMode::Stroke),
        "area" => Some(EraserMode::Area),
        _ => None,
    }
}

fn parse_eraser_area_shape(value: &str) -> Option<EraserAreaShape> {
    match value {
        "rectangle" => Some(EraserAreaShape::Rectangle),
        "lasso" => Some(EraserAreaShape::Lasso),
        _ => None,
    }
}

fn settings_color(color: &SettingsColor) -> Option<AttrIconColor> {
    color_components(color.red, color.green, color.blue, color.alpha)
}

fn radial_color(color: RadialColor) -> Option<AttrIconColor> {
    color_components(color[0], color[1], color[2], color[3])
}

fn color_components(red: f64, green: f64, blue: f64, alpha: f64) -> Option<AttrIconColor> {
    [red, green, blue, alpha]
        .iter()
        .all(|component| component.is_finite() && (0.0..=1.0).contains(component))
        .then(|| AttrIconColor::new(red, green, blue, alpha))
}

#[cfg(test)]
mod tests {
    use super::{
        configuration, estimated_background, AttrIconSnapshot, AttrIconSurface, SystemAppearance,
    };
    use edutools_core::attr_icon::{AttrIconColor, AttrIconTool};
    use edutools_core::eraser::{EraserAreaShape, EraserMode};
    use edutools_core::ink::ArrowStyle;
    use std::collections::BTreeMap;

    trait AttrIconArrowStyleTestExt {
        fn expected(self) -> &'static str;
    }

    impl AttrIconArrowStyleTestExt for ArrowStyle {
        fn expected(self) -> &'static str {
            match self {
                Self::Single => "single",
                Self::Double => "double",
            }
        }
    }

    #[test]
    fn input_configuration_table_covers_modes_heads_and_size_boundaries() {
        let red = AttrIconColor::new(1.0, 0.0, 0.0, 1.0);
        let presets = [12.0, 24.0, 60.0];
        let cases = [
            (
                "pen-min",
                AttrIconTool::Pen,
                1.0,
                None,
                None,
                None,
                "width",
                "1",
            ),
            (
                "pen-max",
                AttrIconTool::Pen,
                12.0,
                None,
                None,
                None,
                "width",
                "4",
            ),
            (
                "highlighter-min",
                AttrIconTool::Highlighter,
                6.0,
                None,
                None,
                None,
                "width",
                "1",
            ),
            (
                "highlighter-max",
                AttrIconTool::Highlighter,
                30.0,
                None,
                None,
                None,
                "width",
                "4",
            ),
            (
                "text-min",
                AttrIconTool::Text,
                16.0,
                None,
                None,
                None,
                "width",
                "1",
            ),
            (
                "text-max",
                AttrIconTool::Text,
                72.0,
                None,
                None,
                None,
                "width",
                "4",
            ),
            (
                "arrow-single",
                AttrIconTool::Arrow,
                1.0,
                Some(ArrowStyle::Single),
                None,
                None,
                "heads",
                ArrowStyle::Single.expected(),
            ),
            (
                "arrow-double",
                AttrIconTool::Arrow,
                12.0,
                Some(ArrowStyle::Double),
                None,
                None,
                "heads",
                ArrowStyle::Double.expected(),
            ),
            (
                "eraser-partial-min",
                AttrIconTool::Eraser,
                12.0,
                None,
                Some(EraserMode::Partial),
                Some(EraserAreaShape::Rectangle),
                "mode",
                "brush",
            ),
            (
                "eraser-stroke-max",
                AttrIconTool::Eraser,
                60.0,
                None,
                Some(EraserMode::Stroke),
                Some(EraserAreaShape::Rectangle),
                "mode",
                "stroke",
            ),
            (
                "eraser-area-rect",
                AttrIconTool::Eraser,
                24.0,
                None,
                Some(EraserMode::Area),
                Some(EraserAreaShape::Rectangle),
                "mode",
                "rect",
            ),
            (
                "eraser-area-lasso",
                AttrIconTool::Eraser,
                24.0,
                None,
                Some(EraserMode::Area),
                Some(EraserAreaShape::Lasso),
                "mode",
                "lasso",
            ),
        ];
        for (name, tool, size, arrow_style, eraser_mode, eraser_area_shape, key, expected) in cases
        {
            let result = configuration(
                AttrIconSnapshot {
                    tool,
                    color: (tool != AttrIconTool::Eraser).then_some(red),
                    size,
                    arrow_style,
                    eraser_mode,
                    eraser_area_shape,
                },
                &presets,
            )
            .unwrap_or_else(|| panic!("{name} configuration"));
            assert_eq!(
                result.attributes.get(key).map(String::as_str),
                Some(expected),
                "{name}"
            );
            if name.ends_with("-min") {
                assert_eq!(
                    result.attributes.get("width").map(String::as_str),
                    Some("1"),
                    "{name}"
                );
            }
            if name.ends_with("-max") {
                assert_eq!(
                    result.attributes.get("width").map(String::as_str),
                    Some("4"),
                    "{name}"
                );
            }
        }

        let shape = configuration(
            AttrIconSnapshot {
                tool: AttrIconTool::Rectangle,
                color: Some(red),
                size: 3.0,
                arrow_style: None,
                eraser_mode: None,
                eraser_area_shape: None,
            },
            &presets,
        )
        .expect("rectangle configuration");
        assert_eq!(
            shape.attributes,
            BTreeMap::from([
                ("fill".to_owned(), "false".to_owned()),
                ("width".to_owned(), "1".to_owned()),
            ])
        );
    }

    #[test]
    fn background_estimate_table_covers_surfaces_selection_and_appearance() {
        let accent = AttrIconColor::new(0.0, 0.48, 1.0, 1.0);
        let cases = [
            (
                AttrIconSurface::Radial,
                false,
                SystemAppearance::Light,
                0.07,
            ),
            (AttrIconSurface::Radial, false, SystemAppearance::Dark, 0.07),
            (
                AttrIconSurface::Linear,
                false,
                SystemAppearance::Light,
                0.93,
            ),
            (AttrIconSurface::Linear, false, SystemAppearance::Dark, 0.18),
        ];
        for (surface, selected, appearance, white) in cases {
            assert_eq!(
                estimated_background(surface, selected, appearance, accent),
                AttrIconColor::new(white, white, white, 1.0)
            );
            assert_eq!(
                estimated_background(surface, true, appearance, accent),
                accent
            );
        }
    }
}
