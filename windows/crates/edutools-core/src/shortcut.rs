use crate::settings::{AppModifierSetting, ShortcutGroupSettings};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::fmt;

// Swift: PortableShortcut modifier tokens
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PortableModifier {
    Control,
    Option,
    Shift,
    Primary,
}

impl PortableModifier {
    // Swift: PortableShortcut.init(serialized:)
    fn parse(token: &str) -> Option<Self> {
        match token {
            "Control" => Some(Self::Control),
            "Option" => Some(Self::Option),
            "Shift" => Some(Self::Shift),
            "Primary" => Some(Self::Primary),
            _ => None,
        }
    }

    // Swift: PortableShortcut.serialized
    fn token(self) -> &'static str {
        match self {
            Self::Control => "Control",
            Self::Option => "Option",
            Self::Shift => "Shift",
            Self::Primary => "Primary",
        }
    }
}

// Swift: NSEvent.ModifierFlags used by PortableShortcut and ShortcutBinding
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ShortcutModifiers(u8);

impl ShortcutModifiers {
    pub const CONTROL: Self = Self(1 << 0);
    pub const ALT: Self = Self(1 << 1);
    pub const SHIFT: Self = Self(1 << 2);
    pub const WINDOWS: Self = Self(1 << 3);

    // Swift: NSEvent.ModifierFlags []
    pub const fn empty() -> Self {
        Self(0)
    }

    // Swift: NSEvent.ModifierFlags.contains(_:)
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    // Swift: NSEvent.ModifierFlags.insert(_:)
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    // Swift: NSEvent.ModifierFlags.rawValue
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for ShortcutModifiers {
    type Output = Self;

    // Swift: NSEvent.ModifierFlags union
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ShortcutModifiers {
    // Swift: NSEvent.ModifierFlags.insert(_:)
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// Swift: PortableShortcut.init(serialized:) rejection paths
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum PortableShortcutError {
    InvalidKey(String),
    UnknownModifier(String),
    DuplicateModifier(String),
    AppModifierMustBeAlone,
}

impl fmt::Display for PortableShortcutError {
    // Swift: PortableShortcut.init(serialized:)
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(key) => write!(formatter, "invalid shortcut key token: {key}"),
            Self::UnknownModifier(modifier) => {
                write!(formatter, "unknown shortcut modifier token: {modifier}")
            }
            Self::DuplicateModifier(modifier) => {
                write!(formatter, "duplicate shortcut modifier token: {modifier}")
            }
            Self::AppModifierMustBeAlone => write!(formatter, "AppMod cannot be combined"),
        }
    }
}

impl Error for PortableShortcutError {}

// Swift: PortableShortcut
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortableShortcut {
    pub key: String,
    pub uses_app_modifier: bool,
    pub modifiers: BTreeSet<PortableModifier>,
}

impl PortableShortcut {
    // Swift: PortableShortcut.init(serialized:)
    pub fn parse(serialized: &str) -> Result<Self, PortableShortcutError> {
        let components: Vec<&str> = serialized.split('+').collect();
        let key = components.last().copied().unwrap_or("");
        if !is_valid_key_token(key) {
            return Err(PortableShortcutError::InvalidKey(key.to_owned()));
        }

        let mut seen_tokens = HashSet::new();
        let mut modifiers = BTreeSet::new();
        let mut uses_app_modifier = false;
        for token in &components[..components.len() - 1] {
            if !seen_tokens.insert(*token) {
                return Err(PortableShortcutError::DuplicateModifier(
                    (*token).to_owned(),
                ));
            }
            if *token == "AppMod" {
                uses_app_modifier = true;
            } else if let Some(modifier) = PortableModifier::parse(token) {
                modifiers.insert(modifier);
            } else {
                return Err(PortableShortcutError::UnknownModifier((*token).to_owned()));
            }
        }
        if uses_app_modifier && components.len() != 2 {
            return Err(PortableShortcutError::AppModifierMustBeAlone);
        }

        Ok(Self {
            key: key.to_owned(),
            uses_app_modifier,
            modifiers,
        })
    }

    // Swift: PortableShortcut.serialized
    pub fn serialized(&self) -> String {
        let mut tokens = Vec::new();
        if self.uses_app_modifier {
            tokens.push("AppMod".to_owned());
        } else {
            tokens.extend(
                self.modifiers
                    .iter()
                    .copied()
                    .map(PortableModifier::token)
                    .map(|token| token.to_owned()),
            );
        }
        tokens.push(self.key.clone());
        tokens.join("+")
    }

    // Swift: PortableShortcut.eventModifiers(appModifier:)
    pub fn resolved_modifiers(&self, app_modifier: AppModifierSetting) -> ShortcutModifiers {
        if self.uses_app_modifier {
            let mut result = ShortcutModifiers::CONTROL | ShortcutModifiers::ALT;
            if app_modifier == AppModifierSetting::ControlOptionShift {
                result |= ShortcutModifiers::SHIFT;
            }
            return result;
        }

        let mut result = ShortcutModifiers::empty();
        for modifier in &self.modifiers {
            match modifier {
                PortableModifier::Control | PortableModifier::Primary => {
                    result.insert(ShortcutModifiers::CONTROL);
                }
                PortableModifier::Option => result.insert(ShortcutModifiers::ALT),
                PortableModifier::Shift => result.insert(ShortcutModifiers::SHIFT),
            }
        }
        result
    }

    // Swift: PortableShortcut.displayString(appModifier:)
    pub fn display_string(&self, app_modifier: AppModifierSetting) -> String {
        display_string(&self.key, self.resolved_modifiers(app_modifier))
    }
}

// Swift: PortableShortcut keys
fn is_valid_key_token(token: &str) -> bool {
    (token.len() == 1
        && token
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()))
        || matches!(
            token,
            "/" | "["
                | "]"
                | "="
                | "-"
                | "Left"
                | "Right"
                | "Up"
                | "Down"
                | "Delete"
                | "Tab"
                | "Escape"
                | "Space"
                | "Return"
        )
}

// Swift: PortableShortcut.displayString(appModifier:)
fn display_string(key: &str, modifiers: ShortcutModifiers) -> String {
    let mut tokens = Vec::new();
    if modifiers.contains(ShortcutModifiers::CONTROL) {
        tokens.push("Ctrl");
    }
    if modifiers.contains(ShortcutModifiers::ALT) {
        tokens.push("Alt");
    }
    if modifiers.contains(ShortcutModifiers::SHIFT) {
        tokens.push("Shift");
    }
    if modifiers.contains(ShortcutModifiers::WINDOWS) {
        tokens.push("Win");
    }
    tokens.push(key_cap(key));
    tokens.join("+")
}

// Swift: PortableShortcut.KeyDescriptor.keyCap
fn key_cap(key: &str) -> &str {
    match key {
        "Delete" => "Backspace",
        "Escape" => "Esc",
        _ => key,
    }
}

// Swift: AppMode
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AppMode {
    Global,
    Annotation,
    Zoom,
}

// Swift: ShortcutScope
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ShortcutScope {
    Global,
    Annotation,
    Zoom,
}

impl ShortcutScope {
    // Swift: ShortcutScope.rawValue
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Annotation => "annotation",
            Self::Zoom => "zoom",
        }
    }

    // Swift: ShortcutScope.init(rawValue:)
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "global" => Some(Self::Global),
            "annotation" => Some(Self::Annotation),
            "zoom" => Some(Self::Zoom),
            _ => None,
        }
    }
}

// Swift: ShortcutBinding.CreationError
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutBindingError {
    InvalidPortableString(PortableShortcutError),
    GlobalRequiresAppModifier,
    LocalForbidsAppModifier,
}

impl fmt::Display for ShortcutBindingError {
    // Swift: ShortcutBinding.CreationError.description
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPortableString(error) => write!(formatter, "{error}"),
            Self::GlobalRequiresAppModifier => {
                write!(formatter, "global scope requires AppMod")
            }
            Self::LocalForbidsAppModifier => {
                write!(formatter, "non-global scope does not allow AppMod")
            }
        }
    }
}

impl Error for ShortcutBindingError {}

// Swift: ShortcutBinding.uniquenessKey
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ShortcutUniquenessKey {
    pub scope: ShortcutScope,
    pub key: String,
    pub modifiers: ShortcutModifiers,
}

// Swift: ShortcutBinding
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutBinding {
    pub scope: ShortcutScope,
    pub key: String,
    pub modifiers: ShortcutModifiers,
    pub key_cap: String,
    pub serialized_global_shortcut: Option<String>,
    pub portable_string: String,
}

impl ShortcutBinding {
    // Swift: ShortcutBinding.make(scope:portableString:appModifier:)
    pub fn make(
        scope: ShortcutScope,
        portable_string: &str,
        app_modifier: AppModifierSetting,
    ) -> Result<Self, ShortcutBindingError> {
        let portable = PortableShortcut::parse(portable_string)
            .map_err(ShortcutBindingError::InvalidPortableString)?;
        if scope == ShortcutScope::Global && !portable.uses_app_modifier {
            return Err(ShortcutBindingError::GlobalRequiresAppModifier);
        }
        if scope != ShortcutScope::Global && portable.uses_app_modifier {
            return Err(ShortcutBindingError::LocalForbidsAppModifier);
        }

        let canonical = portable.serialized();
        Ok(Self {
            scope,
            key: portable.key.clone(),
            modifiers: portable.resolved_modifiers(app_modifier),
            key_cap: portable.display_string(app_modifier),
            serialized_global_shortcut: (scope == ShortcutScope::Global).then(|| canonical.clone()),
            portable_string: canonical,
        })
    }

    // Swift: ShortcutBinding.replacing(portableString:appModifier:)
    pub fn replacing(
        &self,
        portable_string: &str,
        app_modifier: AppModifierSetting,
    ) -> Result<Self, ShortcutBindingError> {
        Self::make(self.scope, portable_string, app_modifier)
    }

    // Swift: ShortcutBinding.uniquenessKey
    pub fn uniqueness_key(&self) -> ShortcutUniquenessKey {
        ShortcutUniquenessKey {
            scope: self.scope,
            key: self.key.clone(),
            modifiers: self.modifiers,
        }
    }

    // Swift: ShortcutBinding.matches(_:)
    pub fn matches(&self, key: &str, modifiers: ShortcutModifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

// Swift: ShortcutGroup
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutGroup {
    Tools,
    Editing,
    ScreenBoardZoom,
    TimerAndOther,
}

// Swift: InkTool values carried by AppCommand.selectTool
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutTool {
    Pen,
    Highlighter,
    Arrow,
    Rectangle,
    Ellipse,
    Text,
    Select,
    Cover,
}

// Swift: BoardKind values carried by AppCommand.toggleBoard
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutBoard {
    White,
    Black,
}

// Swift: AppCommand
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutCommand {
    ToggleAnnotationMode,
    TogglePassThroughMode,
    ToggleZoom,
    ToggleSpotlight,
    ToggleCursorHighlight,
    ToggleKeystrokeDisplay,
    ToggleHelpSharing,
    ApplyDisplayPreset(usize),
    ToggleLessonTimer,
    ShowBreakTimerSetup,
    ToggleBoard(ShortcutBoard),
    PreviousBoardPage,
    NextBoardPage,
    SelectTool(ShortcutTool),
    UseLaser,
    SelectOrCycleEraser,
    ToggleEraserTool,
    ToggleEraserAreaShape,
    ToggleShapeRecognition,
    SelectColor(usize),
    AdjustWidth(i32),
    Undo,
    Redo,
    ClearAll,
    DeleteSelection,
    SaveCurrentContent,
    CopyCurrentImage,
    TogglePalette,
    ToggleRadialPalette,
    StepBack,
    ShowHelp,
    AdjustZoom(i32),
    ResetZoom,
    ToggleZoomFreeze,
    StopZoom,
    BeginZoomPreview,
    CycleZoomFollowMode,
}

impl ShortcutCommand {
    // Swift: AppCommand.isBoardPageNavigation
    pub const fn is_board_page_navigation(&self) -> bool {
        matches!(self, Self::PreviousBoardPage | Self::NextBoardPage)
    }
}

// Swift: ShortcutDefinition
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutDefinition {
    pub id: String,
    pub name: String,
    pub icon_key: String,
    pub group: ShortcutGroup,
    pub modes: Vec<AppMode>,
    pub command: ShortcutCommand,
    pub bindings: Vec<ShortcutBinding>,
}

impl ShortcutDefinition {
    // Swift: ShortcutDefinition.assignableScopes
    pub fn assignable_scopes(&self) -> Vec<ShortcutScope> {
        if !self.bindings.is_empty() {
            return Vec::new();
        }
        [
            (AppMode::Global, ShortcutScope::Global),
            (AppMode::Annotation, ShortcutScope::Annotation),
            (AppMode::Zoom, ShortcutScope::Zoom),
        ]
        .into_iter()
        .filter_map(|(mode, scope)| self.modes.contains(&mode).then_some(scope))
        .collect()
    }

    // Swift: ShortcutDefinition.displayKeys
    pub fn display_keys(&self) -> String {
        let mut seen = HashSet::new();
        self.bindings
            .iter()
            .filter_map(|binding| {
                seen.insert(binding.key_cap.clone())
                    .then_some(binding.key_cap.as_str())
            })
            .collect::<Vec<_>>()
            .join(" / ")
    }

    // Swift: ShortcutDefinition.binding(in:)
    pub fn binding(&self, scope: ShortcutScope) -> Option<&ShortcutBinding> {
        self.bindings.iter().find(|binding| binding.scope == scope)
    }
}

// Swift: ShortcutCatalog.InvalidOverrideLog.Reason
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutOverrideIssueReason {
    InvalidKeyFormat,
    UnknownDefinition(String),
    UnknownScope(String),
    DisallowedScope {
        scope: String,
        definition_id: String,
    },
    BindingCreationFailed(ShortcutBindingError),
}

// Swift: ShortcutCatalog.InvalidOverrideLog
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ShortcutOverrideIssue {
    pub key: String,
    pub reason: ShortcutOverrideIssueReason,
}

// Swift: ShortcutCatalog.loggedInvalidOverrides
#[derive(Debug, Default)]
pub struct ShortcutOverrideLogger {
    seen: HashSet<ShortcutOverrideIssue>,
}

impl ShortcutOverrideLogger {
    // Swift: ShortcutCatalog.logOnce(_:)
    fn log_once(
        &mut self,
        issue: ShortcutOverrideIssue,
        hook: &mut dyn FnMut(&ShortcutOverrideIssue),
    ) {
        if self.seen.insert(issue.clone()) {
            hook(&issue);
        }
    }
}

// Swift: ShortcutCatalog
pub struct ShortcutCatalog;

impl ShortcutCatalog {
    // Swift: ShortcutCatalog.overrideKey(definitionID:scope:)
    pub fn override_key(definition_id: &str, scope: ShortcutScope) -> String {
        format!("{definition_id}|{}", scope.as_str())
    }

    // Swift: ShortcutCatalog.definitions
    pub fn definitions() -> Vec<ShortcutDefinition> {
        let app_modifier = AppModifierSetting::ControlOption;
        let global =
            |portable: &str| builtin_binding(ShortcutScope::Global, portable, app_modifier);
        let annotation =
            |portable: &str| builtin_binding(ShortcutScope::Annotation, portable, app_modifier);
        let zoom = |portable: &str| builtin_binding(ShortcutScope::Zoom, portable, app_modifier);

        macro_rules! d {
            ($id:expr, $name:expr, $icon:expr, $group:ident, [$($mode:ident),*], $command:expr, [$($binding:expr),* $(,)?]) => {
                definition(
                    $id,
                    $name,
                    $icon,
                    ShortcutGroup::$group,
                    vec![$(AppMode::$mode),*],
                    $command,
                    vec![$($binding),*],
                )
            };
        }

        let mut result = vec![
            d!(
                "annotation.toggle",
                "판서 모드 켜기/끄기",
                "pencil",
                ScreenBoardZoom,
                [Global],
                ShortcutCommand::ToggleAnnotationMode,
                [global("AppMod+P")]
            ),
            d!(
                "annotation.pass-through",
                "아래 화면 클릭(통과 모드)",
                "cursorarrow.click",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::TogglePassThroughMode,
                [annotation("U")]
            ),
            d!(
                "zoom.toggle",
                "화면 줌 켜기/끄기",
                "plus.magnifyingglass",
                ScreenBoardZoom,
                [Global],
                ShortcutCommand::ToggleZoom,
                [global("AppMod+Z")]
            ),
            d!(
                "spotlight.toggle",
                "스포트라이트 켜기/끄기",
                "light.beacon.max",
                ScreenBoardZoom,
                [Global, Annotation],
                ShortcutCommand::ToggleSpotlight,
                [global("AppMod+S")]
            ),
            d!(
                "cursor-highlight.toggle",
                "커서 강조 켜기/끄기",
                "cursorarrow.rays",
                ScreenBoardZoom,
                [Global],
                ShortcutCommand::ToggleCursorHighlight,
                []
            ),
            d!(
                "keystroke-display.toggle",
                "키 입력 표시 켜기/끄기",
                "command",
                ScreenBoardZoom,
                [Global],
                ShortcutCommand::ToggleKeystrokeDisplay,
                []
            ),
            d!(
                "timer.lesson.toggle",
                "수업 타이머 시작/일시정지",
                "timer",
                TimerAndOther,
                [Global],
                ShortcutCommand::ToggleLessonTimer,
                [global("AppMod+M")]
            ),
            d!(
                "timer.break.open",
                "쉬는 시간 타이머 열기",
                "cup.and.saucer",
                TimerAndOther,
                [Global],
                ShortcutCommand::ShowBreakTimerSetup,
                [global("AppMod+B")]
            ),
            d!(
                "tool.pen",
                "펜",
                "pencil.tip",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Pen),
                [annotation("P")]
            ),
            d!(
                "tool.highlighter",
                "형광펜",
                "highlighter",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Highlighter),
                [annotation("H")]
            ),
            d!(
                "tool.laser",
                "레이저",
                "laser.burst",
                Tools,
                [Global, Annotation],
                ShortcutCommand::UseLaser,
                [global("AppMod+L"), annotation("L")]
            ),
            d!(
                "tool.eraser",
                "지우개",
                "eraser",
                Tools,
                [Annotation],
                ShortcutCommand::SelectOrCycleEraser,
                [annotation("E")]
            ),
            d!(
                "tool.eraser-toggle",
                "지우개 ↔ 이전 도구",
                "eraser",
                Tools,
                [Annotation],
                ShortcutCommand::ToggleEraserTool,
                []
            ),
            d!(
                "tool.arrow",
                "화살표 (재입력: 한쪽/양쪽)",
                "arrow.right",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Arrow),
                [annotation("A")]
            ),
            d!(
                "tool.rectangle",
                "사각형",
                "rectangle",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Rectangle),
                [annotation("R")]
            ),
            d!(
                "tool.ellipse",
                "원",
                "circle",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Ellipse),
                [annotation("O")]
            ),
            d!(
                "tool.text",
                "텍스트",
                "textformat",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Text),
                [annotation("T")]
            ),
            d!(
                "tool.select",
                "올가미 선택·이동",
                "lasso",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Select),
                [annotation("V")]
            ),
            d!(
                "tool.cover",
                "화면 가리개",
                "rectangle.fill",
                Tools,
                [Annotation],
                ShortcutCommand::SelectTool(ShortcutTool::Cover),
                []
            ),
            d!(
                "tool.shape-recognition",
                "손그림 도형 자동 보정",
                "wand.and.stars",
                Tools,
                [Annotation],
                ShortcutCommand::ToggleShapeRecognition,
                []
            ),
            d!(
                "eraser.area-shape",
                "영역 지우개 사각형/올가미",
                "rectangle.dashed",
                Tools,
                [Annotation],
                ShortcutCommand::ToggleEraserAreaShape,
                []
            ),
            d!(
                "board.white",
                "화이트보드",
                "rectangle.fill",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::ToggleBoard(ShortcutBoard::White),
                [annotation("W")]
            ),
            d!(
                "board.black",
                "블랙보드",
                "rectangle.inset.filled",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::ToggleBoard(ShortcutBoard::Black),
                [annotation("B")]
            ),
            d!(
                "board.page.previous",
                "이전 보드 페이지",
                "chevron.left",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::PreviousBoardPage,
                [annotation("Left")]
            ),
            d!(
                "board.page.next",
                "다음 보드 페이지",
                "chevron.right",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::NextBoardPage,
                [annotation("Right")]
            ),
            d!(
                "tool.width.decrease",
                "현재 도구 크기 줄이기",
                "minus",
                Tools,
                [Annotation],
                ShortcutCommand::AdjustWidth(-1),
                [annotation("[")]
            ),
            d!(
                "tool.width.increase",
                "현재 도구 크기 키우기",
                "plus",
                Tools,
                [Annotation],
                ShortcutCommand::AdjustWidth(1),
                [annotation("]")]
            ),
            d!(
                "edit.undo",
                "실행 취소",
                "arrow.uturn.backward",
                Editing,
                [Annotation],
                ShortcutCommand::Undo,
                [annotation("Primary+Z")]
            ),
            d!(
                "edit.redo",
                "다시 실행",
                "arrow.uturn.forward",
                Editing,
                [Annotation],
                ShortcutCommand::Redo,
                [annotation("Shift+Primary+Z")]
            ),
            d!(
                "edit.clear-all",
                "전체 지우기",
                "trash",
                Editing,
                [Global, Annotation],
                ShortcutCommand::ClearAll,
                [global("AppMod+E"), annotation("Primary+Delete")]
            ),
            d!(
                "edit.delete-selection",
                "선택한 항목 삭제",
                "delete.left",
                Editing,
                [Annotation],
                ShortcutCommand::DeleteSelection,
                [annotation("Delete")]
            ),
            d!(
                "export.save",
                "PNG 저장 / 보드 PDF 저장",
                "square.and.arrow.down",
                Editing,
                [Annotation],
                ShortcutCommand::SaveCurrentContent,
                [annotation("Primary+S")]
            ),
            d!(
                "export.copy",
                "클립보드 복사",
                "doc.on.doc",
                Editing,
                [Annotation],
                ShortcutCommand::CopyCurrentImage,
                [annotation("Primary+C")]
            ),
            d!(
                "palette.toggle",
                "도구 팔레트 숨기기/보이기",
                "rectangle.on.rectangle",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::TogglePalette,
                [annotation("Tab")]
            ),
            d!(
                "palette.radial.toggle",
                "원형 팔레트 열기/접기",
                "circle.grid.cross",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::ToggleRadialPalette,
                [annotation("M")]
            ),
            d!(
                "annotation.step-back",
                "한 단계 나가기",
                "escape",
                ScreenBoardZoom,
                [Annotation],
                ShortcutCommand::StepBack,
                [annotation("Escape")]
            ),
            d!(
                "zoom.increase",
                "줌 배율 올리기",
                "plus.magnifyingglass",
                ScreenBoardZoom,
                [Zoom],
                ShortcutCommand::AdjustZoom(1),
                [zoom("Shift+=")]
            ),
            d!(
                "zoom.decrease",
                "줌 배율 내리기",
                "minus.magnifyingglass",
                ScreenBoardZoom,
                [Zoom],
                ShortcutCommand::AdjustZoom(-1),
                [zoom("-")]
            ),
            d!(
                "zoom.reset",
                "시작 배율로 되돌리기",
                "1.magnifyingglass",
                ScreenBoardZoom,
                [Zoom],
                ShortcutCommand::ResetZoom,
                [zoom("0")]
            ),
            d!(
                "zoom.freeze",
                "화면 멈추고 판서",
                "pause.circle",
                ScreenBoardZoom,
                [Zoom],
                ShortcutCommand::ToggleZoomFreeze,
                [zoom("P")]
            ),
            d!(
                "zoom.preview",
                "확대 위치 미리보기",
                "rectangle.dashed",
                ScreenBoardZoom,
                [Zoom],
                ShortcutCommand::BeginZoomPreview,
                [zoom("Space")]
            ),
            d!(
                "zoom.follow",
                "따라가기 방식 바꾸기",
                "cursorarrow.motionlines",
                ScreenBoardZoom,
                [Zoom],
                ShortcutCommand::CycleZoomFollowMode,
                [zoom("F")]
            ),
            d!(
                "zoom.stop",
                "줌 종료",
                "escape",
                ScreenBoardZoom,
                [Zoom],
                ShortcutCommand::StopZoom,
                [zoom("Escape")]
            ),
            d!(
                "help.show",
                "도움말",
                "questionmark.circle",
                TimerAndOther,
                [Global, Annotation, Zoom],
                ShortcutCommand::ShowHelp,
                [global("AppMod+/"), annotation("Shift+/"), zoom("Shift+/")]
            ),
        ];

        // Swift: DisplayPresetDefaults.maximumPresetCount
        for index in 0..8 {
            result.push(definition(
                &format!("display.preset.{}", index + 1),
                &format!("화면 구성 {} 적용", index + 1),
                "display.2",
                ShortcutGroup::ScreenBoardZoom,
                vec![AppMode::Global],
                ShortcutCommand::ApplyDisplayPreset(index),
                vec![],
            ));
        }

        // Swift: ShortcutCatalog colorKeyCodes
        for index in 0..6 {
            let key = (index + 1).to_string();
            result.push(definition(
                &format!("color.{}", index + 1),
                &format!("색상 {}", index + 1),
                "circle.fill",
                ShortcutGroup::Tools,
                vec![AppMode::Annotation],
                ShortcutCommand::SelectColor(index),
                vec![annotation(&key)],
            ));
        }
        result
    }

    // Swift: ShortcutCatalog.resolvedDefinitions(for:)
    pub fn resolved_definitions(settings: &ShortcutGroupSettings) -> Vec<ShortcutDefinition> {
        let mut logger = ShortcutOverrideLogger::default();
        Self::resolved_definitions_with_log(settings, &mut logger, |_| {})
    }

    // Swift: ShortcutCatalog.resolvedDefinitions(for:) and logOnce(_:)
    pub fn resolved_definitions_with_log(
        settings: &ShortcutGroupSettings,
        logger: &mut ShortcutOverrideLogger,
        mut hook: impl FnMut(&ShortcutOverrideIssue),
    ) -> Vec<ShortcutDefinition> {
        let definitions = Self::definitions();
        validate_override_keys(&definitions, settings, logger, &mut hook);

        definitions
            .into_iter()
            .map(|definition| {
                let mut bindings = definition
                    .bindings
                    .iter()
                    .map(|binding| {
                        let key = Self::override_key(&definition.id, binding.scope);
                        if let Some(portable) = settings.overrides.get(&key) {
                            if let Some(resolved) = resolve_override_binding(
                                binding.scope,
                                portable,
                                settings.app_modifier,
                                &key,
                                logger,
                                &mut hook,
                            ) {
                                return resolved;
                            }
                        }
                        binding
                            .replacing(&binding.portable_string, settings.app_modifier)
                            .expect("built-in shortcut must remain valid")
                    })
                    .collect::<Vec<_>>();

                for scope in definition.assignable_scopes() {
                    let key = Self::override_key(&definition.id, scope);
                    if let Some(portable) = settings.overrides.get(&key) {
                        if let Some(resolved) = resolve_override_binding(
                            scope,
                            portable,
                            settings.app_modifier,
                            &key,
                            logger,
                            &mut hook,
                        ) {
                            bindings.push(resolved);
                        }
                    }
                }

                ShortcutDefinition {
                    bindings,
                    ..definition
                }
            })
            .collect()
    }

    // Swift: ShortcutCatalog.filtered(_:query:)
    pub fn filtered(definitions: &[ShortcutDefinition], query: &str) -> Vec<ShortcutDefinition> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return definitions.to_vec();
        }
        definitions
            .iter()
            .filter(|definition| {
                definition.name.to_lowercase().contains(&query)
                    || definition
                        .bindings
                        .iter()
                        .any(|binding| binding.key_cap.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }
}

// Swift: ShortcutCatalog.definitions construction helper
fn definition(
    id: &str,
    name: &str,
    icon_key: &str,
    group: ShortcutGroup,
    modes: Vec<AppMode>,
    command: ShortcutCommand,
    bindings: Vec<ShortcutBinding>,
) -> ShortcutDefinition {
    ShortcutDefinition {
        id: id.to_owned(),
        name: name.to_owned(),
        icon_key: icon_key.to_owned(),
        group,
        modes,
        command,
        bindings,
    }
}

// Swift: ShortcutCatalog.definitions global/local binding helpers
fn builtin_binding(
    scope: ShortcutScope,
    portable: &str,
    app_modifier: AppModifierSetting,
) -> ShortcutBinding {
    ShortcutBinding::make(scope, portable, app_modifier)
        .expect("built-in shortcut must satisfy its scope")
}

// Swift: ShortcutCatalog.resolvedOverrideBinding(scope:portableString:appModifier:key:)
fn resolve_override_binding(
    scope: ShortcutScope,
    portable_string: &str,
    app_modifier: AppModifierSetting,
    key: &str,
    logger: &mut ShortcutOverrideLogger,
    hook: &mut dyn FnMut(&ShortcutOverrideIssue),
) -> Option<ShortcutBinding> {
    match ShortcutBinding::make(scope, portable_string, app_modifier) {
        Ok(binding) => Some(binding),
        Err(error) => {
            logger.log_once(
                ShortcutOverrideIssue {
                    key: key.to_owned(),
                    reason: ShortcutOverrideIssueReason::BindingCreationFailed(error),
                },
                hook,
            );
            None
        }
    }
}

// Swift: ShortcutCatalog.logInvalidOverrideKeys(in:)
fn validate_override_keys(
    definitions: &[ShortcutDefinition],
    settings: &ShortcutGroupSettings,
    logger: &mut ShortcutOverrideLogger,
    hook: &mut dyn FnMut(&ShortcutOverrideIssue),
) {
    for key in settings.overrides.keys() {
        let components: Vec<&str> = key.split('|').collect();
        if components.len() != 2 {
            logger.log_once(
                ShortcutOverrideIssue {
                    key: key.clone(),
                    reason: ShortcutOverrideIssueReason::InvalidKeyFormat,
                },
                hook,
            );
            continue;
        }

        let definition_id = components[0];
        let Some(definition) = definitions
            .iter()
            .find(|definition| definition.id.as_str() == definition_id)
        else {
            logger.log_once(
                ShortcutOverrideIssue {
                    key: key.clone(),
                    reason: ShortcutOverrideIssueReason::UnknownDefinition(
                        definition_id.to_owned(),
                    ),
                },
                hook,
            );
            continue;
        };

        let scope_value = components[1];
        let Some(scope) = ShortcutScope::parse(scope_value) else {
            logger.log_once(
                ShortcutOverrideIssue {
                    key: key.clone(),
                    reason: ShortcutOverrideIssueReason::UnknownScope(scope_value.to_owned()),
                },
                hook,
            );
            continue;
        };

        let has_default_binding = definition.binding(scope).is_some();
        if !has_default_binding && !definition.assignable_scopes().contains(&scope) {
            logger.log_once(
                ShortcutOverrideIssue {
                    key: key.clone(),
                    reason: ShortcutOverrideIssueReason::DisallowedScope {
                        scope: scope_value.to_owned(),
                        definition_id: definition_id.to_owned(),
                    },
                },
                hook,
            );
        }
    }
}

// Swift: ShortcutConflictDetector avoided shortcut data
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutConflictTable {
    pub avoided_app_modifier_keys: HashSet<String>,
    pub avoided_portable_shortcuts: HashSet<String>,
    pub avoids_control_digits: bool,
    pub avoids_control_shift_letters: bool,
}

impl ShortcutConflictTable {
    // Swift: ShortcutConflictDetector avoidedAppModifierKeys and avoidedPortableShortcuts
    pub fn new(avoided_app_modifier_keys: &[&str], avoided_portable_shortcuts: &[&str]) -> Self {
        Self {
            avoided_app_modifier_keys: avoided_app_modifier_keys
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            avoided_portable_shortcuts: avoided_portable_shortcuts
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            avoids_control_digits: false,
            avoids_control_shift_letters: false,
        }
    }

    // Swift: ShortcutConflictDetector macOS avoided tables
    pub fn macos_compatibility() -> Self {
        let mut table = Self::new(
            &[
                "D", "F", "G", "E", "T", "U", "I", "J", "K", "C", "Left", "Right", "Up", "Down",
                "Return", "Delete",
            ],
            &[
                "Option+Primary+8",
                "Option+Primary+=",
                "Option+Primary+-",
                "Control+Space",
                "Control+Primary+Space",
                "Shift+Primary+3",
                "Shift+Primary+4",
                "Shift+Primary+5",
                "Control+Left",
                "Control+Right",
                "Control+Up",
                "Control+Down",
                "Control+Option+Delete",
                "Control+Option+Tab",
                "Control+Option+Left",
                "Control+Option+Right",
                "Control+Option+Up",
                "Control+Option+Down",
            ],
        );
        table.avoids_control_digits = true;
        table.avoids_control_shift_letters = true;
        table
    }
}

// Swift: ShortcutConflictDetector
pub struct ShortcutConflictDetector;

impl ShortcutConflictDetector {
    // Swift: ShortcutConflictDetector.warnings(for:)
    pub fn warnings(
        settings: &ShortcutGroupSettings,
        table: &ShortcutConflictTable,
    ) -> BTreeMap<String, String> {
        let definitions = ShortcutCatalog::resolved_definitions(settings);
        let mut warnings = BTreeMap::new();
        let mut by_uniqueness: HashMap<ShortcutUniquenessKey, Vec<(String, ShortcutScope)>> =
            HashMap::new();

        for definition in &definitions {
            for binding in &definition.bindings {
                let key = ShortcutCatalog::override_key(&definition.id, binding.scope);
                by_uniqueness
                    .entry(binding.uniqueness_key())
                    .or_default()
                    .push((definition.id.clone(), binding.scope));
                if Self::has_avoided_conflict(&binding.portable_string, table) {
                    warnings.insert(
                        key,
                        format!(
                            "알려진 시스템·앱 단축키와 겹칠 수 있습니다. 대안: {}",
                            suggestion(&definitions)
                        ),
                    );
                }
            }
        }

        for duplicates in by_uniqueness.values().filter(|values| values.len() > 1) {
            for (definition_id, scope) in duplicates {
                warnings.insert(
                    ShortcutCatalog::override_key(definition_id, *scope),
                    format!(
                        "같은 범위의 다른 기능과 중복됩니다. 대안: {}",
                        suggestion(&definitions)
                    ),
                );
            }
        }
        warnings
    }

    // Swift: ShortcutConflictDetector.hasAvoidedConflict(_:)
    pub fn has_avoided_conflict(serialized: &str, table: &ShortcutConflictTable) -> bool {
        let Ok(portable) = PortableShortcut::parse(serialized) else {
            return false;
        };
        if portable.uses_app_modifier {
            return table.avoided_app_modifier_keys.contains(&portable.key);
        }
        if table
            .avoided_portable_shortcuts
            .contains(&portable.serialized())
        {
            return true;
        }

        let only_control = BTreeSet::from([PortableModifier::Control]);
        if table.avoids_control_digits
            && portable.modifiers == only_control
            && portable
                .key
                .chars()
                .next()
                .is_some_and(|value| value.is_ascii_digit())
        {
            return true;
        }
        let control_shift = BTreeSet::from([PortableModifier::Control, PortableModifier::Shift]);
        table.avoids_control_shift_letters
            && portable.modifiers == control_shift
            && portable.key.len() == 1
            && portable
                .key
                .chars()
                .next()
                .is_some_and(|value| value.is_ascii_uppercase())
    }
}

// Swift: ShortcutConflictDetector.suggestion(for:)
fn suggestion(definitions: &[ShortcutDefinition]) -> String {
    let occupied = definitions
        .iter()
        .flat_map(|definition| &definition.bindings)
        .filter(|binding| binding.scope == ShortcutScope::Global)
        .map(|binding| binding.portable_string.as_str())
        .collect::<HashSet<_>>();
    [
        "AppMod+P", "AppMod+Z", "AppMod+L", "AppMod+S", "AppMod+M", "AppMod+B", "AppMod+/",
    ]
    .into_iter()
    .find(|candidate| !occupied.contains(*candidate))
    .unwrap_or("사용하지 않는 AppMod+문자")
    .to_owned()
}
