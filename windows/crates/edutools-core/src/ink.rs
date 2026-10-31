//! 판서 모델과 OS 독립 기하.

use crate::settings::{PerToolMemorySettings, SettingsRanges, ToolMemorySetting};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::ops::RangeInclusive;

// Swift: InkDefaults.undoLimit
pub const UNDO_LIMIT: usize = 100;
// Swift: InkGeometry.smoothedPoints(_:subdivisions:)
pub const DEFAULT_SMOOTHING_SUBDIVISIONS: usize = 8;

/// Swift: CGPoint
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

/// Swift: CGSize
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// AnnotationTextView와 Windows EDIT가 공유하는 텍스트 입력 키 분류.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEntryKey {
    Enter,
    Escape,
    Other,
}

/// AnnotationTextView.keyDown(with:)의 텍스트 입력 처리 결과.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEntryAction {
    Commit,
    LineBreak,
    Cancel,
    Default,
}

/// Swift: commitTextEntry의 측정 결과 -> textSize 규칙.
pub fn text_stroke_size(measured: Size, font_size: f64) -> Size {
    Size {
        width: font_size.max(measured.width + 8.0),
        height: (font_size * 1.2).max(measured.height + 6.0),
    }
}

/// Swift: beginTextEntry의 편집 창 폭 clamp 규칙(DIP).
pub fn text_entry_width(monitor_width: f64, origin_x: f64) -> f64 {
    (monitor_width - origin_x - 12.0).min(320.0).max(80.0)
}

/// Swift: AnnotationTextView.keyDown(with:)의 Enter/Esc 처리 규칙.
pub fn text_entry_action(
    key: TextEntryKey,
    shift_pressed: bool,
    ime_composing: bool,
) -> TextEntryAction {
    match key {
        TextEntryKey::Escape => TextEntryAction::Cancel,
        TextEntryKey::Enter if shift_pressed => TextEntryAction::LineBreak,
        TextEntryKey::Enter if ime_composing => TextEntryAction::Default,
        TextEntryKey::Enter => TextEntryAction::Commit,
        TextEntryKey::Other => TextEntryAction::Default,
    }
}

/// Swift: CGRect
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Swift: InkTool
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InkTool {
    Pen,
    Highlighter,
    Laser,
    Eraser,
    Arrow,
    Rectangle,
    Ellipse,
    Text,
    Select,
    Cover,
}

/// Swift: ToolMemorySlot
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolMemorySlot {
    Pen,
    Highlighter,
    Arrow,
    Rectangle,
    Ellipse,
    Text,
}

impl ToolMemorySlot {
    /// Swift: ToolMemorySlot.keyPath
    pub fn setting(self, memory: &PerToolMemorySettings) -> &ToolMemorySetting {
        match self {
            Self::Pen => &memory.pen,
            Self::Highlighter => &memory.highlighter,
            Self::Arrow => &memory.arrow,
            Self::Rectangle => &memory.rectangle,
            Self::Ellipse => &memory.ellipse,
            Self::Text => &memory.text,
        }
    }

    /// Swift: ToolMemorySlot.keyPath
    pub fn setting_mut(self, memory: &mut PerToolMemorySettings) -> &mut ToolMemorySetting {
        match self {
            Self::Pen => &mut memory.pen,
            Self::Highlighter => &mut memory.highlighter,
            Self::Arrow => &mut memory.arrow,
            Self::Rectangle => &mut memory.rectangle,
            Self::Ellipse => &mut memory.ellipse,
            Self::Text => &mut memory.text,
        }
    }

    /// Swift: ToolMemorySlot.sizeRange
    pub fn size_range(self) -> RangeInclusive<f64> {
        match self {
            Self::Text => SettingsRanges::TEXT_SIZE,
            Self::Pen | Self::Highlighter | Self::Arrow | Self::Rectangle | Self::Ellipse => {
                SettingsRanges::TOOL_SIZE
            }
        }
    }
}

impl InkTool {
    /// Swift: InkTool.memorySlot
    pub fn memory_slot(self) -> Option<ToolMemorySlot> {
        match self {
            Self::Pen => Some(ToolMemorySlot::Pen),
            Self::Highlighter => Some(ToolMemorySlot::Highlighter),
            Self::Arrow => Some(ToolMemorySlot::Arrow),
            Self::Rectangle => Some(ToolMemorySlot::Rectangle),
            Self::Ellipse => Some(ToolMemorySlot::Ellipse),
            Self::Text => Some(ToolMemorySlot::Text),
            Self::Laser | Self::Eraser | Self::Select | Self::Cover => None,
        }
    }
}

/// Swift: InkStrokeTool
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InkStrokeTool {
    Pen,
    Highlighter,
    Shape,
    Text,
    Cover,
}

/// Swift: ArrowStyle
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArrowStyle {
    Single,
    Double,
}

/// Swift: InkShapeKind
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InkShapeKind {
    Line,
    Arrow(ArrowStyle),
    Rectangle,
    Ellipse,
}

/// Swift: InkTextContent
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InkTextContent {
    pub string: String,
    pub font_size: f64,
}

/// Swift: InkStrokeContent
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InkStrokeContent {
    Freehand,
    Shape(InkShapeKind),
    Text(InkTextContent),
    Cover,
}

impl Default for InkStrokeContent {
    /// Swift: InkStroke.init(...content: .freehand)
    fn default() -> Self {
        Self::Freehand
    }
}

/// Swift: InkPoint
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InkPoint {
    pub location: Point,
    pub pressure: f64,
    pub timestamp: f64,
}

impl InkPoint {
    /// Swift: InkPoint.init(location:pressure:timestamp:)
    pub fn new(location: Point, pressure: f64, timestamp: f64) -> Self {
        Self {
            location: Point {
                x: location.x.clamp(0.0, 1.0),
                y: location.y.clamp(0.0, 1.0),
            },
            pressure: pressure.clamp(0.0, 1.0),
            timestamp,
        }
    }
}

/// Swift: InkStroke
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InkStroke {
    pub identifier: String,
    pub tool: InkStrokeTool,
    pub color: [f64; 4],
    pub width: f64,
    pub uses_pressure: bool,
    pub points: Vec<InkPoint>,
    pub content: InkStrokeContent,
}

impl InkStroke {
    /// Swift: InkStroke.init(identifier:tool:color:width:usesPressure:points:content:)
    pub fn new(
        identifier: impl Into<String>,
        tool: InkStrokeTool,
        color: [f64; 4],
        width: f64,
        uses_pressure: bool,
        points: Vec<InkPoint>,
        content: InkStrokeContent,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            tool,
            color,
            width,
            uses_pressure,
            points,
            content,
        }
    }

    /// Swift: InkStroke.effectiveWidth(pressure:minimumRatio:)
    pub fn effective_width(&self, pressure: f64, minimum_ratio: f64) -> f64 {
        effective_width(self.width, pressure, minimum_ratio, self.uses_pressure)
    }
}

/// Swift: InkStroke.effectiveWidth(pressure:minimumRatio:)
pub fn effective_width(width: f64, pressure: f64, minimum_ratio: f64, uses_pressure: bool) -> f64 {
    if uses_pressure {
        width * (minimum_ratio + (1.0 - minimum_ratio) * pressure)
    } else {
        width
    }
}

/// Swift: InkCommand
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "payload")]
pub enum InkCommand {
    AddStroke(InkStroke),
    ReplaceStrokes {
        before: Vec<InkStroke>,
        after: Vec<InkStroke>,
    },
    AddDisplayStroke {
        display_id: String,
        stroke: InkStroke,
    },
    ReplaceDisplayStrokes {
        display_id: String,
        before: Vec<InkStroke>,
        after: Vec<InkStroke>,
    },
    ReplaceAllDisplayStrokes {
        before: BTreeMap<String, Vec<InkStroke>>,
        after: BTreeMap<String, Vec<InkStroke>>,
    },
}

impl InkCommand {
    /// Swift: InkCommand.apply(to:)
    fn apply(&self, state: &mut InkLayerState) {
        match self {
            Self::AddStroke(stroke) => state.strokes.push(stroke.clone()),
            Self::ReplaceStrokes { after, .. } => state.strokes = after.clone(),
            Self::AddDisplayStroke { display_id, stroke } => state
                .display_strokes
                .entry(display_id.clone())
                .or_default()
                .push(stroke.clone()),
            Self::ReplaceDisplayStrokes {
                display_id, after, ..
            } => {
                state
                    .display_strokes
                    .insert(display_id.clone(), after.clone());
            }
            Self::ReplaceAllDisplayStrokes { after, .. } => {
                state.display_strokes = after.clone();
            }
        }
    }

    /// Swift: InkCommand.revert(on:)
    fn revert(&self, state: &mut InkLayerState) {
        match self {
            Self::AddStroke(stroke) => {
                if let Some(index) = state
                    .strokes
                    .iter()
                    .rposition(|candidate| candidate.identifier == stroke.identifier)
                {
                    state.strokes.remove(index);
                }
            }
            Self::ReplaceStrokes { before, .. } => state.strokes = before.clone(),
            Self::AddDisplayStroke { display_id, stroke } => {
                if let Some(strokes) = state.display_strokes.get_mut(display_id) {
                    if let Some(index) = strokes
                        .iter()
                        .rposition(|candidate| candidate.identifier == stroke.identifier)
                    {
                        strokes.remove(index);
                    }
                }
            }
            Self::ReplaceDisplayStrokes {
                display_id, before, ..
            } => {
                state
                    .display_strokes
                    .insert(display_id.clone(), before.clone());
            }
            Self::ReplaceAllDisplayStrokes { before, .. } => {
                state.display_strokes = before.clone();
            }
        }
    }

    /// Swift: InkCommand.command(for:)
    fn for_display(&self, display_id: &str) -> Option<Self> {
        match self {
            Self::AddStroke(_) | Self::ReplaceStrokes { .. } => None,
            Self::AddDisplayStroke {
                display_id: command_display_id,
                stroke,
            } if command_display_id == display_id => Some(Self::AddDisplayStroke {
                display_id: display_id.to_owned(),
                stroke: stroke.clone(),
            }),
            Self::ReplaceDisplayStrokes {
                display_id: command_display_id,
                before,
                after,
            } if command_display_id == display_id => Some(Self::ReplaceDisplayStrokes {
                display_id: display_id.to_owned(),
                before: before.clone(),
                after: after.clone(),
            }),
            Self::ReplaceAllDisplayStrokes { before, after } => {
                let display_before = before.get(display_id).map_or_else(Vec::new, Clone::clone);
                let display_after = after.get(display_id).map_or_else(Vec::new, Clone::clone);
                (display_before != display_after).then(|| Self::ReplaceDisplayStrokes {
                    display_id: display_id.to_owned(),
                    before: display_before,
                    after: display_after,
                })
            }
            _ => None,
        }
    }

    /// Swift: InkCommand.excluding(displayUUID:)
    fn excluding_display(&self, display_id: &str) -> Option<Self> {
        match self {
            Self::AddStroke(_) | Self::ReplaceStrokes { .. } => Some(self.clone()),
            Self::AddDisplayStroke {
                display_id: command_display_id,
                ..
            }
            | Self::ReplaceDisplayStrokes {
                display_id: command_display_id,
                ..
            } => (command_display_id != display_id).then(|| self.clone()),
            Self::ReplaceAllDisplayStrokes { before, after } => {
                let mut filtered_before = before.clone();
                let mut filtered_after = after.clone();
                filtered_before.remove(display_id);
                filtered_after.remove(display_id);
                (filtered_before != filtered_after).then(|| Self::ReplaceAllDisplayStrokes {
                    before: filtered_before,
                    after: filtered_after,
                })
            }
        }
    }
}

/// Swift: InkLayerState
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InkLayerState {
    strokes: Vec<InkStroke>,
    display_strokes: BTreeMap<String, Vec<InkStroke>>,
    undo_commands: Vec<InkCommand>,
    redo_commands: Vec<InkCommand>,
    undo_limit: usize,
}

/// Swift: InkLayerState.CodingKeys
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InkLayerStateWire {
    strokes: Vec<InkStroke>,
    display_strokes: BTreeMap<String, Vec<InkStroke>>,
    undo_commands: Vec<InkCommand>,
    redo_commands: Vec<InkCommand>,
    undo_limit: usize,
}

impl<'de> Deserialize<'de> for InkLayerState {
    /// Swift: InkLayerState.init(from:)
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = InkLayerStateWire::deserialize(deserializer)?;
        if wire.undo_limit == 0 {
            return Err(de::Error::custom("undo limit must be greater than zero"));
        }
        let mut state = Self {
            strokes: wire.strokes,
            display_strokes: wire.display_strokes,
            undo_commands: wire.undo_commands,
            redo_commands: wire.redo_commands,
            undo_limit: wire.undo_limit,
        };
        state.trim_commands_to_limit();
        Ok(state)
    }
}

impl Default for InkLayerState {
    /// Swift: InkLayerState.init(undoLimit:)
    fn default() -> Self {
        Self::new()
    }
}

impl InkLayerState {
    /// Swift: InkLayerState.init(undoLimit:)
    pub fn new() -> Self {
        Self {
            strokes: Vec::new(),
            display_strokes: BTreeMap::new(),
            undo_commands: Vec::new(),
            redo_commands: Vec::new(),
            undo_limit: UNDO_LIMIT,
        }
    }

    /// Swift: InkLayerState.init(undoLimit:)
    pub fn with_undo_limit(undo_limit: usize) -> Result<Self, &'static str> {
        if undo_limit == 0 {
            return Err("undo limit must be greater than zero");
        }
        Ok(Self {
            undo_limit,
            ..Self::new()
        })
    }

    /// Swift: InkLayerState.strokes
    pub fn strokes(&self) -> &[InkStroke] {
        &self.strokes
    }

    /// 화면별 레이어가 현재 획 또는 실행 취소/다시 실행 기록을 보관하는지 판정한다.
    pub fn has_unscoped_restorable_state(&self) -> bool {
        !self.strokes.is_empty() || !self.undo_commands.is_empty() || !self.redo_commands.is_empty()
    }

    /// Swift: InkLayerState.allDisplayStrokes
    pub fn all_display_strokes(&self) -> &BTreeMap<String, Vec<InkStroke>> {
        &self.display_strokes
    }

    /// Swift: InkLayerState.execute(_:)
    pub fn execute(&mut self, command: InkCommand) {
        command.apply(self);
        self.undo_commands.push(command);
        self.trim_commands_to_limit();
        self.redo_commands.clear();
    }

    /// Swift: InkLayerState.trimCommandsToLimit()
    fn trim_commands_to_limit(&mut self) {
        if self.undo_commands.len() > self.undo_limit {
            self.undo_commands
                .drain(0..self.undo_commands.len() - self.undo_limit);
        }
        if self.redo_commands.len() > self.undo_limit {
            self.redo_commands
                .drain(0..self.redo_commands.len() - self.undo_limit);
        }
    }

    /// Swift: InkLayerState.undo()
    pub fn undo(&mut self) -> bool {
        let Some(command) = self.undo_commands.pop() else {
            return false;
        };
        command.revert(self);
        self.redo_commands.push(command);
        true
    }

    /// Swift: InkLayerState.redo()
    pub fn redo(&mut self) -> bool {
        let Some(command) = self.redo_commands.pop() else {
            return false;
        };
        command.apply(self);
        self.undo_commands.push(command);
        true
    }

    /// Swift: InkLayerState.canRedo
    pub fn can_redo(&self) -> bool {
        !self.redo_commands.is_empty()
    }

    /// Swift: InkLayerState.strokes(for:)
    pub fn strokes_for(&self, display_id: &str) -> &[InkStroke] {
        self.display_strokes
            .get(display_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Swift: InkLayerState.hasRestorableState(for:)
    pub fn has_restorable_state(&self, display_id: &str) -> bool {
        !self.strokes_for(display_id).is_empty()
            || self
                .undo_commands
                .iter()
                .any(|command| command.for_display(display_id).is_some())
            || self
                .redo_commands
                .iter()
                .any(|command| command.for_display(display_id).is_some())
    }

    /// Swift: InkLayerState.state(for:)
    pub fn state_for(&self, display_id: &str) -> Self {
        let mut result = Self {
            strokes: Vec::new(),
            display_strokes: BTreeMap::new(),
            undo_commands: Vec::new(),
            redo_commands: Vec::new(),
            undo_limit: self.undo_limit,
        };
        if let Some(strokes) = self.display_strokes.get(display_id) {
            result
                .display_strokes
                .insert(display_id.to_owned(), strokes.clone());
        }
        result.undo_commands = self
            .undo_commands
            .iter()
            .filter_map(|command| command.for_display(display_id))
            .collect();
        result.redo_commands = self
            .redo_commands
            .iter()
            .filter_map(|command| command.for_display(display_id))
            .collect();
        result
    }

    /// Swift: InkLayerState.mergeDisplayState(_:displayUUID:)
    pub fn merge_display_state(&mut self, archived_state: &Self, display_id: &str) {
        self.display_strokes.insert(
            display_id.to_owned(),
            archived_state.strokes_for(display_id).to_vec(),
        );
        self.undo_commands = self
            .undo_commands
            .iter()
            .filter_map(|command| command.excluding_display(display_id))
            .chain(
                archived_state
                    .undo_commands
                    .iter()
                    .filter_map(|command| command.for_display(display_id)),
            )
            .collect();
        self.redo_commands = self
            .redo_commands
            .iter()
            .filter_map(|command| command.excluding_display(display_id))
            .chain(
                archived_state
                    .redo_commands
                    .iter()
                    .filter_map(|command| command.for_display(display_id)),
            )
            .collect();
        self.trim_commands_to_limit();
    }

    /// Swift: InkLayerState.removeDisplay(_:)
    pub fn remove_display(&mut self, display_id: &str) {
        self.display_strokes.remove(display_id);
        self.undo_commands = self
            .undo_commands
            .iter()
            .filter_map(|command| command.excluding_display(display_id))
            .collect();
        self.redo_commands = self
            .redo_commands
            .iter()
            .filter_map(|command| command.excluding_display(display_id))
            .collect();
    }
}

/// Windows 보관 문서의 첫 버전. 새 선택 필드는 이 값을 유지한다.
pub const WINDOWS_ARCHIVE_FORMAT_VERSION: u64 = 1;

/// `InkLayerState`의 기존 serde 형식을 평탄하게 유지하면서 Windows 메타데이터를 붙인다.
pub fn encode_windows_archive(state: &InkLayerState, resolution: Size) -> Result<Vec<u8>, String> {
    if !resolution.width.is_finite()
        || !resolution.height.is_finite()
        || resolution.width <= 0.0
        || resolution.height <= 0.0
    {
        return Err("archive resolution must be finite and positive".to_owned());
    }
    let mut value = serde_json::to_value(state).map_err(|error| error.to_string())?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "ink layer archive must encode as an object".to_owned())?;
    object.insert(
        "formatVersion".to_owned(),
        serde_json::Value::from(WINDOWS_ARCHIVE_FORMAT_VERSION),
    );
    object.insert(
        "resolution".to_owned(),
        serde_json::to_value(resolution).map_err(|error| error.to_string())?,
    );
    serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())
}

/// Windows 보관 문서의 버전과 해상도를 검증한 뒤 기존 판서 상태를 복원한다.
pub fn decode_windows_archive(data: &[u8]) -> Result<(InkLayerState, Size), String> {
    let value: serde_json::Value =
        serde_json::from_slice(data).map_err(|error| error.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "archive document must be an object".to_owned())?;
    let version = object
        .get("formatVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "archive formatVersion is missing".to_owned())?;
    if version != WINDOWS_ARCHIVE_FORMAT_VERSION {
        return Err(format!("unsupported archive formatVersion {version}"));
    }
    let resolution: Size = serde_json::from_value(
        object
            .get("resolution")
            .cloned()
            .ok_or_else(|| "archive resolution is missing".to_owned())?,
    )
    .map_err(|error| error.to_string())?;
    if !resolution.width.is_finite()
        || !resolution.height.is_finite()
        || resolution.width <= 0.0
        || resolution.height <= 0.0
    {
        return Err("archive resolution must be finite and positive".to_owned());
    }
    let state = serde_json::from_value(value).map_err(|error| error.to_string())?;
    Ok((state, resolution))
}

/// Swift: InkGeometry.laserTrailPath(_:)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCommand {
    MoveTo(Point),
    LineTo(Point),
    QuadTo { control: Point, to: Point },
}

/// Swift: InkGeometry
pub struct InkGeometry;

impl InkGeometry {
    /// Swift: InkView.variableWidthPath(for:samples:)
    pub fn variable_width_path(
        stroke: &InkStroke,
        samples: &[(Point, f64)],
        minimum_ratio: f64,
    ) -> Vec<PathCommand> {
        if samples.len() <= 1 {
            return Vec::new();
        }

        let mut left_edge = Vec::with_capacity(samples.len());
        let mut right_edge = Vec::with_capacity(samples.len());
        for index in 0..samples.len() {
            let previous = samples[index.saturating_sub(1)].0;
            let next = samples[(index + 1).min(samples.len() - 1)].0;
            let tangent = Point {
                x: next.x - previous.x,
                y: next.y - previous.y,
            };
            let length = tangent.x.hypot(tangent.y);
            let normal = if length > 0.0 {
                Point {
                    x: -tangent.y / length,
                    y: tangent.x / length,
                }
            } else {
                Point::ZERO
            };
            let half_width = stroke.effective_width(samples[index].1, minimum_ratio) / 2.0;
            left_edge.push(Point {
                x: samples[index].0.x + normal.x * half_width,
                y: samples[index].0.y + normal.y * half_width,
            });
            right_edge.push(Point {
                x: samples[index].0.x - normal.x * half_width,
                y: samples[index].0.y - normal.y * half_width,
            });
        }

        let mut path = Vec::with_capacity(samples.len() * 2 + 11);
        path.push(PathCommand::MoveTo(left_edge[0]));
        path.extend(left_edge.iter().skip(1).copied().map(PathCommand::LineTo));
        path.extend(right_edge.iter().rev().copied().map(PathCommand::LineTo));
        path.push(PathCommand::LineTo(left_edge[0]));
        for sample in [samples[0], samples[samples.len() - 1]] {
            let center = sample.0;
            let radius = stroke.effective_width(sample.1, minimum_ratio) / 2.0;
            path.push(PathCommand::MoveTo(Point {
                x: center.x + radius,
                y: center.y,
            }));
            for (control, to) in [
                (
                    Point {
                        x: center.x + radius,
                        y: center.y + radius,
                    },
                    Point {
                        x: center.x,
                        y: center.y + radius,
                    },
                ),
                (
                    Point {
                        x: center.x - radius,
                        y: center.y + radius,
                    },
                    Point {
                        x: center.x - radius,
                        y: center.y,
                    },
                ),
                (
                    Point {
                        x: center.x - radius,
                        y: center.y - radius,
                    },
                    Point {
                        x: center.x,
                        y: center.y - radius,
                    },
                ),
                (
                    Point {
                        x: center.x + radius,
                        y: center.y - radius,
                    },
                    Point {
                        x: center.x + radius,
                        y: center.y,
                    },
                ),
            ] {
                path.push(PathCommand::QuadTo { control, to });
            }
        }
        path
    }

    /// Swift: InkGeometry.laserTrailPath(_:)
    pub fn laser_trail_path(points: &[Point]) -> Vec<PathCommand> {
        if points.len() <= 1 {
            return Vec::new();
        }
        let mut path = vec![PathCommand::MoveTo(points[0])];
        if points.len() == 2 {
            path.push(PathCommand::LineTo(points[1]));
            return path;
        }
        for index in 1..points.len() - 1 {
            let point = points[index];
            let next = points[index + 1];
            path.push(PathCommand::QuadTo {
                control: point,
                to: Point {
                    x: (point.x + next.x) / 2.0,
                    y: (point.y + next.y) / 2.0,
                },
            });
        }
        path.push(PathCommand::LineTo(points[points.len() - 1]));
        path
    }

    /// Swift: InkGeometry.normalizedPoint(from:in:)
    pub fn normalized_point(point: Point, size: Size) -> Point {
        if size.width <= 0.0 || size.height <= 0.0 {
            return Point::ZERO;
        }
        Point {
            x: point.x / size.width,
            y: point.y / size.height,
        }
    }

    /// Swift: InkGeometry.screenPoint(from:in:)
    pub fn screen_point(point: Point, size: Size) -> Point {
        Point {
            x: point.x * size.width,
            y: point.y * size.height,
        }
    }

    /// Swift: InkGeometry.smoothedPoints(_:subdivisions:)
    pub fn smoothed_points(points: &[InkPoint]) -> Vec<InkPoint> {
        Self::smoothed_points_with_subdivisions(points, DEFAULT_SMOOTHING_SUBDIVISIONS)
    }

    /// Swift: InkGeometry.smoothedPoints(_:subdivisions:)
    pub fn smoothed_points_with_subdivisions(
        points: &[InkPoint],
        subdivisions: usize,
    ) -> Vec<InkPoint> {
        if points.len() <= 1 || subdivisions == 0 {
            return points.to_vec();
        }

        let mut result = Vec::with_capacity((points.len() - 1) * subdivisions + 1);
        for index in 0..points.len() - 1 {
            let p0 = &points[index.saturating_sub(1)];
            let p1 = &points[index];
            let p2 = &points[index + 1];
            let p3 = &points[(index + 2).min(points.len() - 1)];

            for step in 0..subdivisions {
                let t = step as f64 / subdivisions as f64;
                let t2 = t * t;
                let t3 = t2 * t;
                let x = 0.5
                    * (2.0 * p1.location.x
                        + (-p0.location.x + p2.location.x) * t
                        + (2.0 * p0.location.x - 5.0 * p1.location.x + 4.0 * p2.location.x
                            - p3.location.x)
                            * t2
                        + (-p0.location.x + 3.0 * p1.location.x - 3.0 * p2.location.x
                            + p3.location.x)
                            * t3);
                let y = 0.5
                    * (2.0 * p1.location.y
                        + (-p0.location.y + p2.location.y) * t
                        + (2.0 * p0.location.y - 5.0 * p1.location.y + 4.0 * p2.location.y
                            - p3.location.y)
                            * t2
                        + (-p0.location.y + 3.0 * p1.location.y - 3.0 * p2.location.y
                            + p3.location.y)
                            * t3);
                result.push(InkPoint::new(
                    Point { x, y },
                    p1.pressure + (p2.pressure - p1.pressure) * t,
                    p1.timestamp + (p2.timestamp - p1.timestamp) * t,
                ));
            }
        }
        result.push(points[points.len() - 1].clone());
        result
    }

    /// Swift: InkGeometry.snappedPenEndpoint(from:to:)
    pub fn snapped_pen_endpoint(start: Point, proposed: Point) -> Point {
        let delta_x = proposed.x - start.x;
        let delta_y = proposed.y - start.y;
        let distance = delta_x.hypot(delta_y);
        if distance <= 0.0 {
            return start;
        }
        let step = PI / 4.0;
        let angle = (delta_y.atan2(delta_x) / step).round() * step;
        Point {
            x: start.x + angle.cos() * distance,
            y: start.y + angle.sin() * distance,
        }
    }

    /// Swift: InkGeometry.snappedHighlighterEndpoint(from:to:)
    pub fn snapped_highlighter_endpoint(start: Point, proposed: Point) -> Point {
        Point {
            x: proposed.x,
            y: start.y,
        }
    }

    /// Swift: InkGeometry.snappedSquareEndpoint(from:to:)
    pub fn snapped_square_endpoint(start: Point, proposed: Point) -> Point {
        let delta_x = proposed.x - start.x;
        let delta_y = proposed.y - start.y;
        let side = delta_x.abs().max(delta_y.abs());
        Point {
            x: start.x + if delta_x < 0.0 { -side } else { side },
            y: start.y + if delta_y < 0.0 { -side } else { side },
        }
    }
}
