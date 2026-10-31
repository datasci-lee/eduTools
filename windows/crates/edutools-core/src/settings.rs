use crate::display_preset::{DisplayPresetValidator, DisplaySettings};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::ops::RangeInclusive;
use std::sync::RwLock;

// Swift: SettingsColor
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SettingsColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

// Swift: GeneralSettings
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub launch_at_login: bool,
    pub clears_ink_when_annotation_mode_ends: bool,
    pub auto_archive_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_folder_path: Option<String>,
    pub display_disconnect_policy: String,
    pub automatically_restores_disconnected_displays: bool,
}

// Swift: GeneralSettings values in AppSettings.default
impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            clears_ink_when_annotation_mode_ends: false,
            auto_archive_enabled: true,
            archive_folder_path: None,
            display_disconnect_policy: "archive".to_owned(),
            automatically_restores_disconnected_displays: false,
        }
    }
}

// Swift: PaletteStyle
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PaletteStyle {
    Radial,
    Linear,
}

// Swift: RadialPaletteGlassStyle
pub struct RadialPaletteGlassStyle;

impl RadialPaletteGlassStyle {
    pub const DEFAULT_OPACITY: f64 = 0.86;
    pub const STORED_MINIMUM_OPACITY: f64 = 0.55;
    pub const MINIMUM_OPACITY: f64 = 0.75;
    pub const MAXIMUM_OPACITY: f64 = 0.90;

    // Swift: RadialPaletteGlassStyle.effectiveOpacity(stored:)
    pub fn effective_opacity(stored: f64) -> f64 {
        stored.max(Self::MINIMUM_OPACITY)
    }
}

// Swift: ToolMemorySetting
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolMemorySetting {
    pub color_index: i64,
    pub size: f64,
}

// Swift: PerToolMemorySettings
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PerToolMemorySettings {
    pub pen: ToolMemorySetting,
    pub highlighter: ToolMemorySetting,
    pub arrow: ToolMemorySetting,
    pub rectangle: ToolMemorySetting,
    pub ellipse: ToolMemorySetting,
    pub text: ToolMemorySetting,
}

// Swift: PerToolMemorySettings.default
impl Default for PerToolMemorySettings {
    fn default() -> Self {
        let pen = ToolMemorySetting {
            color_index: 0,
            size: 3.0,
        };
        Self {
            pen: pen.clone(),
            highlighter: ToolMemorySetting {
                color_index: 0,
                size: 18.0,
            },
            arrow: pen.clone(),
            rectangle: pen.clone(),
            ellipse: pen,
            text: ToolMemorySetting {
                color_index: 0,
                size: 24.0,
            },
        }
    }
}

impl PerToolMemorySettings {
    // Swift: PerToolMemorySettings.migratingLegacySizes
    pub fn migrating_legacy_sizes(
        pen_width: Option<f64>,
        highlighter_width: Option<f64>,
        text_font_size: Option<f64>,
    ) -> Self {
        let mut result = Self::default();
        if let Some(size) = pen_width {
            result.pen.size = size;
            result.arrow.size = size;
            result.rectangle.size = size;
            result.ellipse.size = size;
        }
        if let Some(size) = highlighter_width {
            result.highlighter.size = size;
        }
        if let Some(size) = text_font_size {
            result.text.size = size;
        }
        result
    }
}

// Swift: FavoritesDefaults
pub struct FavoritesDefaults;

impl FavoritesDefaults {
    pub const MAXIMUM_COUNT: usize = 8;
}

// Swift: FavoriteTool
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteTool {
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_index: Option<i64>,
    pub size: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eraser_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eraser_area_shape: Option<String>,
}

impl FavoriteTool {
    // Swift: FavoriteTool.isValid(colorPresetCount:)
    pub fn is_valid(&self, color_preset_count: usize) -> bool {
        let valid_color = self
            .color_index
            .and_then(|index| usize::try_from(index).ok())
            .is_some_and(|index| index < color_preset_count);
        match self.tool.as_str() {
            "eraser" => {
                self.color_index.is_none()
                    && SettingsRanges::ERASER_DIAMETER.contains(&self.size)
                    && self.arrow_style.is_none()
                    && self
                        .eraser_mode
                        .as_deref()
                        .is_some_and(|value| ERASER_MODES.contains(&value))
                    && self
                        .eraser_area_shape
                        .as_deref()
                        .is_some_and(|value| ERASER_AREA_SHAPES.contains(&value))
            }
            "laser" => {
                valid_color
                    && SettingsRanges::LASER_TRAIL_DURATION.contains(&self.size)
                    && self.arrow_style.is_none()
                    && self.eraser_mode.is_none()
                    && self.eraser_area_shape.is_none()
            }
            "arrow" => {
                valid_color
                    && SettingsRanges::TOOL_SIZE.contains(&self.size)
                    && self
                        .arrow_style
                        .as_deref()
                        .is_some_and(|value| ARROW_STYLES.contains(&value))
                    && self.eraser_mode.is_none()
                    && self.eraser_area_shape.is_none()
            }
            "pen" | "highlighter" | "rectangle" | "ellipse" => {
                valid_color
                    && SettingsRanges::TOOL_SIZE.contains(&self.size)
                    && self.arrow_style.is_none()
                    && self.eraser_mode.is_none()
                    && self.eraser_area_shape.is_none()
            }
            "text" => {
                valid_color
                    && SettingsRanges::TEXT_SIZE.contains(&self.size)
                    && self.arrow_style.is_none()
                    && self.eraser_mode.is_none()
                    && self.eraser_area_shape.is_none()
            }
            _ => false,
        }
    }
}

// Swift: NSLog hook used by ToolSettings.normalizedFavorites
pub type SettingsLogHook = fn(&str);

static SETTINGS_LOG_HOOK: RwLock<Option<SettingsLogHook>> = RwLock::new(None);

// Swift: ToolSettings.normalizedFavorites logging site
pub fn set_settings_log_hook(hook: Option<SettingsLogHook>) {
    if let Ok(mut current) = SETTINGS_LOG_HOOK.write() {
        *current = hook;
    }
}

// Swift: ToolSettings.normalizedFavorites NSLog call
fn log_ignored_favorites(count: usize) {
    let hook = SETTINGS_LOG_HOOK.read().ok().and_then(|current| *current);
    let message = format!("ignored {count} invalid, duplicate, or excess favorite tool settings");
    if let Some(hook) = hook {
        hook(&message);
    } else {
        eprintln!("EduTools: {message}");
    }
}

// Swift: ToolSettings
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSettings {
    pub colors: Vec<SettingsColor>,
    pub highlighter_opacity: f64,
    pub pressure_minimum_width_ratio: f64,
    pub laser_color: SettingsColor,
    pub laser_size: f64,
    pub laser_trail_duration: f64,
    pub per_tool_memory: PerToolMemorySettings,
    pub favorites: Vec<FavoriteTool>,
    pub shape_recognition_enabled: bool,
    pub palette_style: PaletteStyle,
    pub palette_glass_opacity: f64,
}

// Swift: ToolSettings CodingKeys and LegacySizeCodingKeys
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolSettingsWire {
    colors: Vec<SettingsColor>,
    highlighter_opacity: f64,
    pressure_minimum_width_ratio: Option<f64>,
    laser_color: SettingsColor,
    laser_size: f64,
    laser_trail_duration: f64,
    per_tool_memory: Option<PerToolMemorySettings>,
    favorites: Option<Vec<FavoriteTool>>,
    shape_recognition_enabled: bool,
    palette_style: Option<PaletteStyle>,
    palette_glass_opacity: Option<f64>,
    pen_width: Option<Value>,
    highlighter_width: Option<Value>,
    text_font_size: Option<Value>,
}

// Swift: ToolSettings.init(from:)
impl<'de> Deserialize<'de> for ToolSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ToolSettingsWire::deserialize(deserializer)?;
        let memory = match wire.per_tool_memory {
            Some(memory) => memory,
            None => PerToolMemorySettings::migrating_legacy_sizes(
                decode_legacy_size::<D::Error>(wire.pen_width)?,
                decode_legacy_size::<D::Error>(wire.highlighter_width)?,
                decode_legacy_size::<D::Error>(wire.text_font_size)?,
            ),
        };
        let favorites =
            normalized_favorites(wire.favorites.unwrap_or_else(Vec::new), wire.colors.len());
        Ok(Self {
            colors: wire.colors,
            highlighter_opacity: wire.highlighter_opacity,
            pressure_minimum_width_ratio: wire.pressure_minimum_width_ratio.unwrap_or(0.2),
            laser_color: wire.laser_color,
            laser_size: wire.laser_size,
            laser_trail_duration: wire.laser_trail_duration,
            per_tool_memory: memory,
            favorites,
            shape_recognition_enabled: wire.shape_recognition_enabled,
            palette_style: wire.palette_style.unwrap_or(PaletteStyle::Radial),
            palette_glass_opacity: wire
                .palette_glass_opacity
                .unwrap_or(RadialPaletteGlassStyle::DEFAULT_OPACITY),
        })
    }
}

// Swift: LegacySizeCodingKeys decodeIfPresent(Double.self, forKey:)
fn decode_legacy_size<E>(value: Option<Value>) -> Result<Option<f64>, E>
where
    E: de::Error,
{
    value
        .map(|value| serde_json::from_value(value).map_err(E::custom))
        .transpose()
}

// Swift: ToolSettings.normalizedFavorites(_:colorPresetCount:)
fn normalized_favorites(
    favorites: Vec<FavoriteTool>,
    color_preset_count: usize,
) -> Vec<FavoriteTool> {
    let original_count = favorites.len();
    let mut result = Vec::new();
    for favorite in favorites {
        if favorite.is_valid(color_preset_count)
            && !result.contains(&favorite)
            && result.len() < FavoritesDefaults::MAXIMUM_COUNT
        {
            result.push(favorite);
        }
    }
    if result.len() != original_count {
        log_ignored_favorites(original_count - result.len());
    }
    result
}

// Swift: ToolSettings values in AppSettings.default
impl Default for ToolSettings {
    fn default() -> Self {
        let red = SettingsColor {
            red: 1.0,
            green: 0.231,
            blue: 0.188,
            alpha: 1.0,
        };
        Self {
            colors: vec![
                red.clone(),
                SettingsColor {
                    red: 1.0,
                    green: 0.584,
                    blue: 0.0,
                    alpha: 1.0,
                },
                SettingsColor {
                    red: 1.0,
                    green: 0.8,
                    blue: 0.0,
                    alpha: 1.0,
                },
                SettingsColor {
                    red: 0.204,
                    green: 0.78,
                    blue: 0.349,
                    alpha: 1.0,
                },
                SettingsColor {
                    red: 0.0,
                    green: 0.478,
                    blue: 1.0,
                    alpha: 1.0,
                },
                SettingsColor {
                    red: 1.0,
                    green: 1.0,
                    blue: 1.0,
                    alpha: 1.0,
                },
            ],
            highlighter_opacity: 0.4,
            pressure_minimum_width_ratio: 0.2,
            laser_color: red,
            laser_size: 8.0,
            laser_trail_duration: 1.0,
            per_tool_memory: PerToolMemorySettings::default(),
            favorites: Vec::new(),
            shape_recognition_enabled: true,
            palette_style: PaletteStyle::Radial,
            palette_glass_opacity: RadialPaletteGlassStyle::DEFAULT_OPACITY,
        }
    }
}

// Swift: EraserGroupSettings
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EraserGroupSettings {
    pub mode: String,
    pub diameter: f64,
    pub preset_diameters: Vec<f64>,
    pub hit_rule: String,
    pub target_filter: String,
    pub area_shape: String,
}

// Swift: EraserGroupSettings values in AppSettings.default
impl Default for EraserGroupSettings {
    fn default() -> Self {
        Self {
            mode: "partial".to_owned(),
            diameter: 24.0,
            preset_diameters: vec![12.0, 24.0, 60.0],
            hit_rule: "touching".to_owned(),
            target_filter: "all".to_owned(),
            area_shape: "rectangle".to_owned(),
        }
    }
}

// Swift: ZoomGroupSettings
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomGroupSettings {
    pub starting_magnification: f64,
    pub maximum_magnification: f64,
    pub magnification_step: f64,
    pub follow_mode: String,
    pub follow_smoothness: f64,
    pub live_by_default: bool,
    pub preview_dimming: f64,
}

// Swift: ZoomGroupSettings CodingKeys
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZoomGroupSettingsWire {
    starting_magnification: f64,
    maximum_magnification: f64,
    magnification_step: f64,
    follow_mode: String,
    follow_smoothness: f64,
    live_by_default: bool,
    preview_dimming: Option<f64>,
}

// Swift: ZoomGroupSettings.init(from:)
impl<'de> Deserialize<'de> for ZoomGroupSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ZoomGroupSettingsWire::deserialize(deserializer)?;
        Ok(Self {
            starting_magnification: wire.starting_magnification,
            maximum_magnification: wire.maximum_magnification,
            magnification_step: wire.magnification_step,
            follow_mode: wire.follow_mode,
            follow_smoothness: wire.follow_smoothness,
            live_by_default: wire.live_by_default,
            preview_dimming: wire.preview_dimming.unwrap_or(0.62),
        })
    }
}

// Swift: ZoomGroupSettings values in AppSettings.default
impl Default for ZoomGroupSettings {
    fn default() -> Self {
        Self {
            starting_magnification: 2.0,
            maximum_magnification: 8.0,
            magnification_step: 0.25,
            follow_mode: "edge".to_owned(),
            follow_smoothness: 0.22,
            live_by_default: true,
            preview_dimming: 0.62,
        }
    }
}

// Swift: BoardGroupSettings
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardGroupSettings {
    pub whiteboard_color: SettingsColor,
    pub blackboard_color: SettingsColor,
    pub background_pattern: String,
}

// Swift: BoardGroupSettings values in AppSettings.default
impl Default for BoardGroupSettings {
    fn default() -> Self {
        Self {
            whiteboard_color: SettingsColor {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 1.0,
            },
            blackboard_color: SettingsColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            background_pattern: "none".to_owned(),
        }
    }
}

// Swift: TimerGroupSettings
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerGroupSettings {
    pub lesson_presets: Vec<i64>,
    pub lesson_default_minutes: i64,
    pub break_default_minutes: i64,
    pub notification_sound: String,
    pub break_message: String,
    pub break_background_color: SettingsColor,
    #[serde(rename = "playlistURLs")]
    pub playlist_urls: Vec<String>,
    pub default_playlist_index: i64,
    pub music_enabled: bool,
    pub music_browser: String,
    #[serde(default = "default_music_source")]
    pub music_source: String,
    #[serde(default)]
    pub music_folder_path: String,
    #[serde(default = "default_music_order")]
    pub music_order: String,
    #[serde(default = "default_music_repeat")]
    pub music_repeat: String,
    pub fade_enabled: bool,
    pub fade_duration: f64,
    pub auto_start_break: bool,
    pub lesson_badge_size: String,
    pub lesson_badge_opacity: f64,
    pub lesson_display_mode: String,
}

// Swift: TimerGroupSettings values in AppSettings.default
impl Default for TimerGroupSettings {
    fn default() -> Self {
        Self {
            lesson_presets: vec![25, 45, 50, 90],
            lesson_default_minutes: 50,
            break_default_minutes: 10,
            notification_sound: "Glass".to_owned(),
            break_message: "수업 재개 시각".to_owned(),
            break_background_color: SettingsColor {
                red: 0.12,
                green: 0.12,
                blue: 0.14,
                alpha: 1.0,
            },
            playlist_urls: Vec::new(),
            default_playlist_index: 0,
            music_enabled: false,
            music_browser: "chrome".to_owned(),
            music_source: default_music_source(),
            music_folder_path: String::new(),
            music_order: default_music_order(),
            music_repeat: default_music_repeat(),
            fade_enabled: true,
            fade_duration: 60.0,
            auto_start_break: false,
            lesson_badge_size: "medium".to_owned(),
            lesson_badge_opacity: 0.9,
            lesson_display_mode: "remaining".to_owned(),
        }
    }
}

fn default_music_source() -> String {
    "youtube".to_owned()
}

fn default_music_order() -> String {
    "name".to_owned()
}

fn default_music_repeat() -> String {
    "all".to_owned()
}

impl TimerGroupSettings {
    pub const MAXIMUM_PLAYLIST_COUNT: usize = 5;

    // Swift: BreakMusicSettings.replacePlaylistURLs(with:defaultIndex:)
    pub fn replace_playlist_urls(&mut self, values: Vec<String>, default_index: i64) -> bool {
        let index_is_valid = values.is_empty()
            || usize::try_from(default_index).is_ok_and(|index| index < values.len());
        if values.len() > Self::MAXIMUM_PLAYLIST_COUNT
            || !values.iter().all(|value| is_valid_music_url(value))
            || !index_is_valid
        {
            return false;
        }

        self.playlist_urls = values;
        self.default_playlist_index = if self.playlist_urls.is_empty() {
            0
        } else {
            default_index
        };
        true
    }

    // Swift: BreakMusicSettings.selectPlaylist(at:)
    pub fn select_playlist(&mut self, index: i64) -> bool {
        let Some(index) = usize::try_from(index).ok() else {
            return false;
        };
        if index >= self.playlist_urls.len() {
            return false;
        }
        self.default_playlist_index = index as i64;
        true
    }

    // Swift: BreakMusicSettings.setMusicEnabled(_:)
    pub fn set_music_enabled(&mut self, enabled: bool) {
        self.music_enabled = enabled;
    }

    // Swift: BreakMusicSettings.setBrowser(_:)
    pub fn set_browser(&mut self, browser: &str) -> bool {
        if !MUSIC_BROWSERS.contains(&browser) {
            return false;
        }
        self.music_browser = browser.to_owned();
        true
    }

    // Swift: BreakMusicSettings.setFade(enabled:duration:)
    pub fn set_fade(&mut self, enabled: bool, duration: f64) -> bool {
        if !SettingsRanges::FADE_DURATION.contains(&duration) {
            return false;
        }
        self.fade_enabled = enabled;
        self.fade_duration = duration;
        true
    }
}

// Swift: AppModifierSetting
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppModifierSetting {
    ControlOption,
    ControlOptionShift,
}

// Swift: ShortcutGroupSettings
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutGroupSettings {
    pub app_modifier: AppModifierSetting,
    pub overrides: BTreeMap<String, String>,
}

// Swift: ShortcutGroupSettings values in AppSettings.default
impl Default for ShortcutGroupSettings {
    fn default() -> Self {
        Self {
            app_modifier: AppModifierSetting::ControlOption,
            overrides: BTreeMap::new(),
        }
    }
}

// Swift: HelpGroupSettings
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpGroupSettings {
    pub shows_help_in_sharing: bool,
}

// Swift: HelpGroupSettings values in AppSettings.default
impl Default for HelpGroupSettings {
    fn default() -> Self {
        Self {
            shows_help_in_sharing: false,
        }
    }
}

// Swift: AppSettings
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AppSettings {
    pub general: GeneralSettings,
    pub tools: ToolSettings,
    pub eraser: EraserGroupSettings,
    pub zoom: ZoomGroupSettings,
    pub board: BoardGroupSettings,
    pub timer: TimerGroupSettings,
    pub display: DisplaySettings,
    pub shortcuts: ShortcutGroupSettings,
    pub help: HelpGroupSettings,
}

// Swift: AppSettings CodingKeys
#[derive(Deserialize)]
struct AppSettingsWire {
    general: GeneralSettings,
    tools: ToolSettings,
    eraser: EraserGroupSettings,
    zoom: ZoomGroupSettings,
    board: BoardGroupSettings,
    timer: TimerGroupSettings,
    display: Option<DisplaySettings>,
    shortcuts: ShortcutGroupSettings,
    help: HelpGroupSettings,
}

// Swift: AppSettings.init(from:)
impl<'de> Deserialize<'de> for AppSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AppSettingsWire::deserialize(deserializer)?;
        Ok(Self {
            general: wire.general,
            tools: wire.tools,
            eraser: wire.eraser,
            zoom: wire.zoom,
            board: wire.board,
            timer: wire.timer,
            display: wire.display.unwrap_or_else(DisplaySettings::default),
            shortcuts: wire.shortcuts,
            help: wire.help,
        })
    }
}

// Swift: AppSettings.default
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            tools: ToolSettings::default(),
            eraser: EraserGroupSettings::default(),
            zoom: ZoomGroupSettings::default(),
            board: BoardGroupSettings::default(),
            timer: TimerGroupSettings::default(),
            display: DisplaySettings::default(),
            shortcuts: ShortcutGroupSettings::default(),
            help: HelpGroupSettings::default(),
        }
    }
}

/// 설정 관찰자가 전체 문서를 값별로 기록하지 않고 변경 묶음만 식별할 때 쓴다.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsGroup {
    General,
    Tools,
    Eraser,
    Zoom,
    Board,
    Timer,
    Display,
    Shortcuts,
    Help,
}

impl SettingsGroup {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Tools => "tools",
            Self::Eraser => "eraser",
            Self::Zoom => "zoom",
            Self::Board => "board",
            Self::Timer => "timer",
            Self::Display => "display",
            Self::Shortcuts => "shortcuts",
            Self::Help => "help",
        }
    }
}

/// Swift `SettingsStore` 관찰자에 전달되는 이전/새 설정의 최상위 묶음 차이.
pub fn changed_settings_groups(
    previous: &AppSettings,
    current: &AppSettings,
) -> Vec<SettingsGroup> {
    let mut changed = Vec::new();
    if previous.general != current.general {
        changed.push(SettingsGroup::General);
    }
    if previous.tools != current.tools {
        changed.push(SettingsGroup::Tools);
    }
    if previous.eraser != current.eraser {
        changed.push(SettingsGroup::Eraser);
    }
    if previous.zoom != current.zoom {
        changed.push(SettingsGroup::Zoom);
    }
    if previous.board != current.board {
        changed.push(SettingsGroup::Board);
    }
    if previous.timer != current.timer {
        changed.push(SettingsGroup::Timer);
    }
    if previous.display != current.display {
        changed.push(SettingsGroup::Display);
    }
    if previous.shortcuts != current.shortcuts {
        changed.push(SettingsGroup::Shortcuts);
    }
    if previous.help != current.help {
        changed.push(SettingsGroup::Help);
    }
    changed
}

// Swift: SettingsRanges
pub struct SettingsRanges;

impl SettingsRanges {
    pub const HIGH_LIGHTER_OPACITY: RangeInclusive<f64> = 0.05..=1.0;
    pub const PRESSURE_MINIMUM_WIDTH_RATIO: RangeInclusive<f64> = 0.05..=1.0;
    pub const LASER_SIZE: RangeInclusive<f64> = 2.0..=50.0;
    pub const LASER_TRAIL_DURATION: RangeInclusive<f64> = 0.1..=10.0;
    pub const TOOL_SIZE: RangeInclusive<f64> = 1.0..=30.0;
    pub const TEXT_SIZE: RangeInclusive<f64> = 8.0..=96.0;
    pub const ERASER_DIAMETER: RangeInclusive<f64> = 8.0..=200.0;
    pub const ZOOM_MAGNIFICATION: RangeInclusive<f64> = 1.25..=8.0;
    pub const ZOOM_FOLLOW_SMOOTHNESS: RangeInclusive<f64> = 0.0..=1.0;
    pub const ZOOM_PREVIEW_DIMMING: RangeInclusive<f64> = 0.0..=1.0;
    pub const LESSON_MINUTES: RangeInclusive<i64> = 1..=240;
    pub const BREAK_MINUTES: RangeInclusive<i64> = 1..=180;
    pub const FADE_DURATION: RangeInclusive<f64> = 1.0..=600.0;
    pub const LESSON_BADGE_OPACITY: RangeInclusive<f64> = 0.4..=1.0;
}

// Swift: SettingsValidationError
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsError {
    InvalidValue(String),
    UnsupportedFormat(String),
    UnsupportedVersion(i64),
}

// Swift: SettingsValidationError.errorDescription
impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(value) => write!(formatter, "invalid settings value: {value}"),
            Self::UnsupportedFormat(value) => write!(formatter, "unsupported format: {value}"),
            Self::UnsupportedVersion(value) => write!(formatter, "unsupported version: {value}"),
        }
    }
}

impl Error for SettingsError {}

const DISPLAY_DISCONNECT_POLICIES: &[&str] = &["archive", "moveToPrimary", "delete"];
const ERASER_MODES: &[&str] = &["partial", "stroke", "area"];
const ERASER_HIT_RULES: &[&str] = &["touching", "contained"];
const ERASER_TARGET_FILTERS: &[&str] = &["all", "penOnly", "highlighterOnly"];
const ERASER_AREA_SHAPES: &[&str] = &["rectangle", "lasso"];
const ARROW_STYLES: &[&str] = &["single", "double"];
const ZOOM_FOLLOW_MODES: &[&str] = &["always", "edge", "fixed"];
const BOARD_BACKGROUND_PATTERNS: &[&str] = &["none", "grid", "lines"];
const MUSIC_BROWSERS: &[&str] = &["chrome", "youtubeMusicApp", "defaultBrowser"];
const MUSIC_SOURCES: &[&str] = &["youtube", "folder"];
const MUSIC_ORDERS: &[&str] = &["name", "shuffle"];
const MUSIC_REPEATS: &[&str] = &["all", "one"];
const LESSON_BADGE_SIZES: &[&str] = &["small", "medium", "large"];
const LESSON_DISPLAY_MODES: &[&str] = &["remaining", "elapsed"];

// Swift: SettingsValidator
pub struct SettingsValidator;

impl SettingsValidator {
    pub fn is_unsupported_music_folder_path(path: &str) -> bool {
        let bytes = path.trim().as_bytes();
        let is_separator = |byte| matches!(byte, b'\\' | b'/');
        (bytes.len() >= 2 && is_separator(bytes[0]) && is_separator(bytes[1]))
            || (bytes.len() >= 4
                && is_separator(bytes[0])
                && bytes[1] == b'?'
                && bytes[2] == b'?'
                && is_separator(bytes[3]))
    }

    // Swift: SettingsValidator.validate(_:)
    pub fn validate(settings: &AppSettings) -> Result<(), SettingsError> {
        if !DISPLAY_DISCONNECT_POLICIES
            .contains(&settings.general.display_disconnect_policy.as_str())
        {
            return invalid("display disconnect policy");
        }
        if settings
            .general
            .archive_folder_path
            .as_ref()
            .is_some_and(String::is_empty)
        {
            return invalid("archive folder path");
        }

        if settings.tools.colors.len() != 6 {
            return invalid("colors must contain six presets");
        }
        for color in &settings.tools.colors {
            validate_color(color)?;
        }
        validate_color(&settings.tools.laser_color)?;
        if !SettingsRanges::HIGH_LIGHTER_OPACITY.contains(&settings.tools.highlighter_opacity)
            || !SettingsRanges::PRESSURE_MINIMUM_WIDTH_RATIO
                .contains(&settings.tools.pressure_minimum_width_ratio)
            || !SettingsRanges::LASER_SIZE.contains(&settings.tools.laser_size)
            || !SettingsRanges::LASER_TRAIL_DURATION.contains(&settings.tools.laser_trail_duration)
            || !(RadialPaletteGlassStyle::STORED_MINIMUM_OPACITY
                ..=RadialPaletteGlassStyle::MAXIMUM_OPACITY)
                .contains(&settings.tools.palette_glass_opacity)
        {
            return invalid("tool range");
        }
        validate_memory(
            "pen",
            &settings.tools.per_tool_memory.pen,
            &SettingsRanges::TOOL_SIZE,
        )?;
        validate_memory(
            "highlighter",
            &settings.tools.per_tool_memory.highlighter,
            &SettingsRanges::TOOL_SIZE,
        )?;
        validate_memory(
            "arrow",
            &settings.tools.per_tool_memory.arrow,
            &SettingsRanges::TOOL_SIZE,
        )?;
        validate_memory(
            "rectangle",
            &settings.tools.per_tool_memory.rectangle,
            &SettingsRanges::TOOL_SIZE,
        )?;
        validate_memory(
            "ellipse",
            &settings.tools.per_tool_memory.ellipse,
            &SettingsRanges::TOOL_SIZE,
        )?;
        validate_memory(
            "text",
            &settings.tools.per_tool_memory.text,
            &SettingsRanges::TEXT_SIZE,
        )?;
        if settings.tools.favorites.len() > FavoritesDefaults::MAXIMUM_COUNT
            || !settings
                .tools
                .favorites
                .iter()
                .all(|favorite| favorite.is_valid(settings.tools.colors.len()))
            || settings
                .tools
                .favorites
                .iter()
                .enumerate()
                .any(|(index, favorite)| settings.tools.favorites[..index].contains(favorite))
        {
            return invalid("favorite tools");
        }

        if !ERASER_MODES.contains(&settings.eraser.mode.as_str())
            || !SettingsRanges::ERASER_DIAMETER.contains(&settings.eraser.diameter)
            || settings.eraser.preset_diameters.len() != 3
            || !settings
                .eraser
                .preset_diameters
                .iter()
                .all(|value| SettingsRanges::ERASER_DIAMETER.contains(value))
            || !ERASER_HIT_RULES.contains(&settings.eraser.hit_rule.as_str())
            || !ERASER_TARGET_FILTERS.contains(&settings.eraser.target_filter.as_str())
            || !ERASER_AREA_SHAPES.contains(&settings.eraser.area_shape.as_str())
        {
            return invalid("eraser range");
        }

        if !SettingsRanges::ZOOM_MAGNIFICATION.contains(&settings.zoom.starting_magnification) {
            return invalid("starting magnification");
        }
        if !SettingsRanges::ZOOM_MAGNIFICATION.contains(&settings.zoom.maximum_magnification) {
            return invalid("maximum magnification");
        }
        if settings.zoom.starting_magnification > settings.zoom.maximum_magnification {
            return invalid("maximum magnification below starting magnification");
        }
        if settings.zoom.magnification_step != 0.25 && settings.zoom.magnification_step != 0.5 {
            return invalid("magnification step");
        }
        if !ZOOM_FOLLOW_MODES.contains(&settings.zoom.follow_mode.as_str()) {
            return invalid("zoom follow mode");
        }
        if !SettingsRanges::ZOOM_FOLLOW_SMOOTHNESS.contains(&settings.zoom.follow_smoothness) {
            return invalid("zoom follow smoothness");
        }
        if !SettingsRanges::ZOOM_PREVIEW_DIMMING.contains(&settings.zoom.preview_dimming) {
            return invalid("zoom preview dimming");
        }

        validate_color(&settings.board.whiteboard_color)?;
        validate_color(&settings.board.blackboard_color)?;
        if !BOARD_BACKGROUND_PATTERNS.contains(&settings.board.background_pattern.as_str()) {
            return invalid("board background pattern");
        }

        validate_color(&settings.timer.break_background_color)?;
        if settings.timer.lesson_presets.is_empty()
            || !settings
                .timer
                .lesson_presets
                .iter()
                .all(|value| SettingsRanges::LESSON_MINUTES.contains(value))
            || !SettingsRanges::LESSON_MINUTES.contains(&settings.timer.lesson_default_minutes)
            || !SettingsRanges::BREAK_MINUTES.contains(&settings.timer.break_default_minutes)
            || settings.timer.playlist_urls.len() > 5
            || !settings
                .timer
                .playlist_urls
                .iter()
                .all(|value| is_valid_music_url(value))
            || (!settings.timer.playlist_urls.is_empty()
                && usize::try_from(settings.timer.default_playlist_index)
                    .map_or(true, |index| index >= settings.timer.playlist_urls.len()))
            || !MUSIC_BROWSERS.contains(&settings.timer.music_browser.as_str())
            || !MUSIC_SOURCES.contains(&settings.timer.music_source.as_str())
            || settings
                .timer
                .music_folder_path
                .chars()
                .any(char::is_control)
            || Self::is_unsupported_music_folder_path(&settings.timer.music_folder_path)
            || !MUSIC_ORDERS.contains(&settings.timer.music_order.as_str())
            || !MUSIC_REPEATS.contains(&settings.timer.music_repeat.as_str())
            || !SettingsRanges::FADE_DURATION.contains(&settings.timer.fade_duration)
            || !LESSON_BADGE_SIZES.contains(&settings.timer.lesson_badge_size.as_str())
            || !SettingsRanges::LESSON_BADGE_OPACITY.contains(&settings.timer.lesson_badge_opacity)
            || !LESSON_DISPLAY_MODES.contains(&settings.timer.lesson_display_mode.as_str())
        {
            return invalid("timer range");
        }

        if settings.shortcuts.overrides.keys().any(String::is_empty) {
            return invalid("shortcut identifier");
        }

        DisplayPresetValidator::validate(&settings.display)
            .map_err(|error| SettingsError::InvalidValue(error.to_string()))?;
        Ok(())
    }
}

// Swift: SettingsValidator.validateColor(_:)
fn validate_color(color: &SettingsColor) -> Result<(), SettingsError> {
    if [color.red, color.green, color.blue, color.alpha]
        .into_iter()
        .all(|value| (0.0..=1.0).contains(&value))
    {
        Ok(())
    } else {
        invalid("color component")
    }
}

// Swift: SettingsValidator per-tool memory loop
fn validate_memory(
    name: &str,
    memory: &ToolMemorySetting,
    size_range: &RangeInclusive<f64>,
) -> Result<(), SettingsError> {
    if (0..6).contains(&memory.color_index) && size_range.contains(&memory.size) {
        Ok(())
    } else {
        invalid(&format!("{name} last setting"))
    }
}

// Swift: BreakMusicURL.validated(_:)
fn is_valid_music_url(value: &str) -> bool {
    const PREFIX: &str = "https://music.youtube.com/";
    if !value.starts_with(PREFIX) || value.chars().any(char::is_control) {
        return false;
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

// Swift: SettingsValidationError.invalidValue
fn invalid<T>(detail: &str) -> Result<T, SettingsError> {
    Err(SettingsError::InvalidValue(detail.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{changed_settings_groups, AppSettings, SettingsGroup};

    #[test]
    fn changed_settings_groups_table() {
        let cases: Vec<(&str, fn(&mut AppSettings), SettingsGroup)> = vec![
            (
                "general",
                |value| value.general.launch_at_login = true,
                SettingsGroup::General,
            ),
            (
                "tools",
                |value| value.tools.shape_recognition_enabled = false,
                SettingsGroup::Tools,
            ),
            (
                "eraser",
                |value| value.eraser.diameter = 25.0,
                SettingsGroup::Eraser,
            ),
            (
                "zoom",
                |value| value.zoom.preview_dimming = 0.5,
                SettingsGroup::Zoom,
            ),
            (
                "board",
                |value| value.board.background_pattern = "grid".to_owned(),
                SettingsGroup::Board,
            ),
            (
                "timer",
                |value| value.timer.lesson_default_minutes = 45,
                SettingsGroup::Timer,
            ),
            (
                "display",
                |value| value.display.auto_switch_enabled = true,
                SettingsGroup::Display,
            ),
            (
                "shortcuts",
                |value| {
                    value
                        .shortcuts
                        .overrides
                        .insert("test|global".to_owned(), "AppMod+P".to_owned());
                },
                SettingsGroup::Shortcuts,
            ),
            (
                "help",
                |value| value.help.shows_help_in_sharing = true,
                SettingsGroup::Help,
            ),
        ];

        let previous = AppSettings::default();
        assert!(changed_settings_groups(&previous, &previous).is_empty());
        for (label, mutate, expected) in cases {
            let mut current = previous.clone();
            mutate(&mut current);
            assert_eq!(
                changed_settings_groups(&previous, &current),
                vec![expected],
                "{label}"
            );
        }
    }
}
