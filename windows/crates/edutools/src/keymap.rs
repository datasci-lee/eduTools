// Shortcut.swift의 휴대용 키 토큰과 Windows VK 사이의 최소 정본.

use edutools_core::settings::AppModifierSetting;
use edutools_core::shortcut::{PortableModifier, PortableShortcut, ShortcutScope};
use std::collections::BTreeSet;

pub const MODIFIER_CONTROL: u8 = 1 << 0;
pub const MODIFIER_ALT: u8 = 1 << 1;
pub const MODIFIER_SHIFT: u8 = 1 << 2;
pub const MODIFIER_WINDOWS: u8 = 1 << 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecorderOutcome {
    Cancel,
    Clear,
    Ignore,
    Reject,
    Record(String),
}

/// Swift `ShortcutRecorderButton.keyDown`의 Windows 입력 규칙.
pub fn recorder_outcome(
    vk: u32,
    modifier_bits: u8,
    scope: ShortcutScope,
    app_modifier: AppModifierSetting,
) -> RecorderOutcome {
    if vk == 0x1B {
        return RecorderOutcome::Cancel;
    }
    if matches!(vk, 0x08 | 0x2E) {
        return RecorderOutcome::Clear;
    }
    if matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C) {
        return RecorderOutcome::Ignore;
    }
    let Some(token) = vk_to_token(vk) else {
        return RecorderOutcome::Reject;
    };
    if modifier_bits & MODIFIER_WINDOWS != 0 {
        return RecorderOutcome::Reject;
    }

    if scope == ShortcutScope::Global {
        let expected = MODIFIER_CONTROL
            | MODIFIER_ALT
            | if app_modifier == AppModifierSetting::ControlOptionShift {
                MODIFIER_SHIFT
            } else {
                0
            };
        return if modifier_bits == expected {
            RecorderOutcome::Record(format!("AppMod+{token}"))
        } else {
            RecorderOutcome::Reject
        };
    }

    let mut modifiers = BTreeSet::new();
    if modifier_bits & MODIFIER_ALT != 0 {
        modifiers.insert(PortableModifier::Option);
    }
    if modifier_bits & MODIFIER_SHIFT != 0 {
        modifiers.insert(PortableModifier::Shift);
    }
    if modifier_bits & MODIFIER_CONTROL != 0 {
        // Windows의 Ctrl은 portable `Primary`로 기록한다(Windows plan §2.2).
        modifiers.insert(PortableModifier::Primary);
    }
    RecorderOutcome::Record(
        PortableShortcut {
            key: token.to_owned(),
            uses_app_modifier: false,
            modifiers,
        }
        .serialized(),
    )
}

pub fn token_to_vk(token: &str) -> Option<u32> {
    let byte = token.as_bytes();
    if byte.len() == 1 && (byte[0].is_ascii_uppercase() || byte[0].is_ascii_digit()) {
        return Some(u32::from(byte[0]));
    }
    match token {
        "/" => Some(0xBF),
        "[" => Some(0xDB),
        "]" => Some(0xDD),
        "=" => Some(0xBB),
        "-" => Some(0xBD),
        "Left" => Some(0x25),
        "Up" => Some(0x26),
        "Right" => Some(0x27),
        "Down" => Some(0x28),
        // PortableShortcut의 Delete는 macOS ⌫, Windows VK_BACK에 대응한다.
        "Delete" => Some(0x08),
        "Tab" => Some(0x09),
        "Return" => Some(0x0D),
        "Escape" => Some(0x1B),
        "Space" => Some(0x20),
        _ => None,
    }
}

pub fn vk_to_token(vk: u32) -> Option<&'static str> {
    match vk {
        0x41..=0x5A => Some(LETTERS[(vk - 0x41) as usize]),
        0x30..=0x39 => Some(DIGITS[(vk - 0x30) as usize]),
        0xBF => Some("/"),
        0xDB => Some("["),
        0xDD => Some("]"),
        0xBB => Some("="),
        0xBD => Some("-"),
        0x25 => Some("Left"),
        0x26 => Some("Up"),
        0x27 => Some("Right"),
        0x28 => Some("Down"),
        0x08 => Some("Delete"),
        0x09 => Some("Tab"),
        0x0D => Some("Return"),
        0x1B => Some("Escape"),
        0x20 => Some("Space"),
        _ => None,
    }
}

const LETTERS: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];
const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

#[cfg(test)]
mod tests {
    use super::{
        recorder_outcome, token_to_vk, vk_to_token, RecorderOutcome, MODIFIER_ALT,
        MODIFIER_CONTROL, MODIFIER_SHIFT,
    };
    use edutools_core::settings::AppModifierSetting;
    use edutools_core::shortcut::ShortcutScope;

    #[test]
    fn portable_tokens_round_trip_through_vk_table() {
        let cases = [
            ("P", 0x50),
            ("0", 0x30),
            ("9", 0x39),
            ("/", 0xBF),
            ("[", 0xDB),
            ("]", 0xDD),
            ("=", 0xBB),
            ("-", 0xBD),
            ("Left", 0x25),
            ("Up", 0x26),
            ("Right", 0x27),
            ("Down", 0x28),
            ("Delete", 0x08),
            ("Tab", 0x09),
            ("Escape", 0x1B),
            ("Space", 0x20),
            ("Return", 0x0D),
        ];
        for (token, vk) in cases {
            assert_eq!(token_to_vk(token), Some(vk), "token {token}");
            assert_eq!(vk_to_token(vk), Some(token), "VK {vk:#x}");
        }
    }

    #[test]
    fn rejects_tokens_and_virtual_keys_outside_the_portable_table() {
        for token in ["", "p", "AA", "F1", "Backspace"] {
            assert_eq!(token_to_vk(token), None, "token {token}");
        }
        for vk in [0, 0x2E, 0x70, 0xBA] {
            assert_eq!(vk_to_token(vk), None, "VK {vk:#x}");
        }
    }

    #[test]
    fn recorder_input_rules_table() {
        let cases = [
            (
                0x50,
                MODIFIER_CONTROL | MODIFIER_ALT,
                ShortcutScope::Global,
                AppModifierSetting::ControlOption,
                RecorderOutcome::Record("AppMod+P".to_owned()),
            ),
            (
                0x5A,
                MODIFIER_CONTROL | MODIFIER_SHIFT,
                ShortcutScope::Annotation,
                AppModifierSetting::ControlOption,
                RecorderOutcome::Record("Shift+Primary+Z".to_owned()),
            ),
            (
                0x1B,
                0,
                ShortcutScope::Annotation,
                AppModifierSetting::ControlOption,
                RecorderOutcome::Cancel,
            ),
            (
                0x08,
                0,
                ShortcutScope::Annotation,
                AppModifierSetting::ControlOption,
                RecorderOutcome::Clear,
            ),
            (
                0x2E,
                0,
                ShortcutScope::Annotation,
                AppModifierSetting::ControlOption,
                RecorderOutcome::Clear,
            ),
            (
                0x11,
                MODIFIER_CONTROL,
                ShortcutScope::Annotation,
                AppModifierSetting::ControlOption,
                RecorderOutcome::Ignore,
            ),
        ];
        for (vk, modifiers, scope, app_modifier, expected) in cases {
            assert_eq!(
                recorder_outcome(vk, modifiers, scope, app_modifier),
                expected,
                "vk={vk:#x} modifiers={modifiers:#x}"
            );
        }
    }
}
