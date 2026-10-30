// Shortcut.swift의 휴대용 키 토큰과 Windows VK 사이의 최소 정본.

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
    use super::{token_to_vk, vk_to_token};

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
}
