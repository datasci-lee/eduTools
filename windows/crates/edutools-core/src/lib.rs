//! EduTools 핵심 로직 — OS 의존 0 (docs/windows-plan.md §2.1).

pub mod attr_icon;
pub mod board;
pub mod dispatch;
pub mod display_operations;
pub mod display_preset;
pub mod eraser;
pub mod ink;
pub mod keystroke;
pub mod laser;
pub mod music;
pub mod radial;
pub mod screen_position;
pub mod settings;
pub mod settings_store;
pub mod settings_transfer;
pub mod shape;
pub mod shortcut;
pub mod state;
pub mod timer;
pub mod zoom;

pub use display_preset::*;
pub use settings::*;
