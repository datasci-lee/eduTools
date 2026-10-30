//! Board presentation, appearance, pages, and page-local history.

use crate::settings::{BoardGroupSettings, SettingsColor};
use serde::{Deserialize, Serialize};

// Swift: BoardKind
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BoardKind {
    White,
    Black,
}

// Swift: BlackboardBackground
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlackboardBackground {
    Black,
    ChalkGreen,
}

// Swift: BoardBackgroundPattern
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BoardBackgroundPattern {
    None,
    Grid,
    Lines,
}

impl BoardBackgroundPattern {
    // Swift: BoardBackgroundPattern.init(rawValue:)
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "grid" => Some(Self::Grid),
            "lines" => Some(Self::Lines),
            _ => None,
        }
    }
}

// Swift: BoardPresentation
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardPresentation {
    pub kind: BoardKind,
    pub display_id: String,
}

// Swift: BoardPagePosition
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardPagePosition {
    pub current: usize,
    pub total: usize,
}

// Swift: BoardAppearance color constants
struct BoardColors;

impl BoardColors {
    const WHITE: [f64; 4] = [1.0, 1.0, 1.0, 1.0];
    const BLACK: [f64; 4] = [0.0, 0.0, 0.0, 1.0];
    const CHALK_GREEN: [f64; 4] = [0.055, 0.18, 0.13, 1.0];
    const WHITE_PATTERN: [f64; 4] = [0.0, 0.0, 0.0, 0.12];
    const BLACK_PATTERN: [f64; 4] = [1.0, 1.0, 1.0, 0.18];
    const BLACK_COMPONENT_TOLERANCE: f64 = 0.001;
}

// Swift: BoardAppearance
pub struct BoardAppearance;

impl BoardAppearance {
    // Swift: BoardAppearance.backgroundColor(for:blackboardBackground:)
    pub fn background_color(
        kind: BoardKind,
        blackboard_background: BlackboardBackground,
    ) -> SettingsColor {
        match (kind, blackboard_background) {
            (BoardKind::White, _) => color(BoardColors::WHITE),
            (BoardKind::Black, BlackboardBackground::Black) => color(BoardColors::BLACK),
            (BoardKind::Black, BlackboardBackground::ChalkGreen) => color(BoardColors::CHALK_GREEN),
        }
    }

    // Swift: BoardAppearance.patternColor(for:)
    pub fn pattern_color(kind: BoardKind) -> SettingsColor {
        match kind {
            BoardKind::White => color(BoardColors::WHITE_PATTERN),
            BoardKind::Black => color(BoardColors::BLACK_PATTERN),
        }
    }

    // Swift: AppState.boardBackgroundColor(for:)
    pub fn settings_background_color(
        kind: BoardKind,
        settings: &BoardGroupSettings,
    ) -> SettingsColor {
        match kind {
            BoardKind::White => settings.whiteboard_color.clone(),
            BoardKind::Black => settings.blackboard_color.clone(),
        }
    }

    // Swift: BoardAppearance.displayColor(for:on:)
    pub fn display_color(stored_color: &SettingsColor, kind: Option<BoardKind>) -> SettingsColor {
        if kind == Some(BoardKind::Black) && Self::is_black(stored_color) {
            color(BoardColors::WHITE)
        } else {
            stored_color.clone()
        }
    }

    // Swift: BoardAppearance.isBlack(_:)
    pub fn is_black(value: &SettingsColor) -> bool {
        value.red <= BoardColors::BLACK_COMPONENT_TOLERANCE
            && value.green <= BoardColors::BLACK_COMPONENT_TOLERANCE
            && value.blue <= BoardColors::BLACK_COMPONENT_TOLERANCE
    }
}

// Swift: NSColor(deviceRed:green:blue:alpha:)
fn color(components: [f64; 4]) -> SettingsColor {
    SettingsColor {
        red: components[0],
        green: components[1],
        blue: components[2],
        alpha: components[3],
    }
}

// Swift: InkLayerState subset used by BoardDocument
#[derive(Clone, Debug, PartialEq)]
struct BoardPage<T> {
    items: Vec<T>,
    undo_history: Vec<Vec<T>>,
    redo_history: Vec<Vec<T>>,
}

impl<T> BoardPage<T>
where
    T: Clone + PartialEq,
{
    // Swift: InkLayerState.init()
    fn new() -> Self {
        Self {
            items: Vec::new(),
            undo_history: Vec::new(),
            redo_history: Vec::new(),
        }
    }

    // Swift: InkLayerState.execute(.replaceStrokes(before:after:))
    fn replace_items(&mut self, items: Vec<T>) -> bool {
        if self.items == items {
            return false;
        }
        self.undo_history
            .push(std::mem::replace(&mut self.items, items));
        self.redo_history.clear();
        true
    }

    // Swift: InkLayerState.undo()
    fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_history.pop() else {
            return false;
        };
        self.redo_history
            .push(std::mem::replace(&mut self.items, previous));
        true
    }

    // Swift: InkLayerState.redo()
    fn redo(&mut self) -> bool {
        let Some(next) = self.redo_history.pop() else {
            return false;
        };
        self.undo_history
            .push(std::mem::replace(&mut self.items, next));
        true
    }
}

// Swift: BoardDocument
#[derive(Clone, Debug, PartialEq)]
pub struct BoardDocument<T> {
    pages: Vec<BoardPage<T>>,
    current_page_index: usize,
}

impl<T> Default for BoardDocument<T>
where
    T: Clone + PartialEq,
{
    // Swift: BoardDocument.init()
    fn default() -> Self {
        Self::new()
    }
}

impl<T> BoardDocument<T>
where
    T: Clone + PartialEq,
{
    // Swift: BoardDocument.init()
    pub fn new() -> Self {
        Self {
            pages: vec![BoardPage::new()],
            current_page_index: 0,
        }
    }

    // Swift: BoardDocument.pagePosition
    pub fn page_position(&self) -> BoardPagePosition {
        BoardPagePosition {
            current: self.current_page_index + 1,
            total: self.pages.len(),
        }
    }

    // Swift: BoardDocument.strokes
    pub fn items(&self) -> &[T] {
        &self.pages[self.current_page_index].items
    }

    // Swift: BoardDocument.allPageStrokes
    pub fn all_page_items(&self) -> Vec<&[T]> {
        self.pages
            .iter()
            .map(|page| page.items.as_slice())
            .collect()
    }

    // Swift: BoardDocument.addStroke(_:)
    pub fn add_item(&mut self, item: T) {
        let mut items = self.items().to_vec();
        items.push(item);
        self.current_page_mut().replace_items(items);
    }

    // Swift: BoardDocument.replaceStrokes(with:)
    pub fn replace_items(&mut self, items: Vec<T>) -> bool {
        self.current_page_mut().replace_items(items)
    }

    // Swift: BoardDocument.clear()
    pub fn clear(&mut self) -> bool {
        self.replace_items(Vec::new())
    }

    // Swift: BoardDocument.undo()
    pub fn undo(&mut self) -> bool {
        self.current_page_mut().undo()
    }

    // Swift: BoardDocument.redo()
    pub fn redo(&mut self) -> bool {
        self.current_page_mut().redo()
    }

    // Swift: BoardDocument.moveToPreviousPage()
    pub fn move_to_previous_page(&mut self) -> bool {
        if self.current_page_index == 0 {
            return false;
        }
        self.current_page_index -= 1;
        true
    }

    // Swift: BoardDocument.moveToNextPage()
    pub fn move_to_next_page(&mut self) -> bool {
        if self.current_page_index + 1 == self.pages.len() {
            self.pages.push(BoardPage::new());
        }
        self.current_page_index += 1;
        true
    }

    // Swift: BoardDocument.currentPage
    fn current_page_mut(&mut self) -> &mut BoardPage<T> {
        &mut self.pages[self.current_page_index]
    }
}
