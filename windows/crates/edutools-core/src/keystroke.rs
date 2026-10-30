//! Pure filtering and Windows presentation for displayed keystrokes.

// Swift: NSEvent.ModifierFlags portable subset
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Modifier {
    Control,
    Alt,
    Shift,
    Meta,
}

impl Modifier {
    // Swift: NSEvent.ModifierFlags raw bit corresponding to a relevant modifier
    const fn bit(self) -> u8 {
        match self {
            Self::Control => 1 << 0,
            Self::Alt => 1 << 1,
            Self::Shift => 1 << 2,
            Self::Meta => 1 << 3,
        }
    }
}

// Swift: NSEvent.ModifierFlags relevant modifier intersection
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModifierSet(u8);

impl ModifierSet {
    // Swift: NSEvent.ModifierFlags.init(rawValue:)
    pub const fn empty() -> Self {
        Self(0)
    }

    // Swift: NSEvent.ModifierFlags.contains(_:)
    pub const fn contains(self, modifier: Modifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    // Swift: NSEvent.ModifierFlags.insert(_:)
    pub fn insert(&mut self, modifier: Modifier) {
        self.0 |= modifier.bit();
    }

    // Swift: NSEvent.ModifierFlags union construction
    pub fn with(mut self, modifier: Modifier) -> Self {
        self.insert(modifier);
        self
    }
}

// Swift: KeystrokeDisplayFilter
pub struct KeystrokeDisplayFilter;

impl KeystrokeDisplayFilter {
    // Swift: KeystrokeDisplayFilter.displayString(keyCode:charactersIgnoringModifiers:modifierFlags:)
    pub fn display_string(modifiers: ModifierSet, key_name: &str) -> Option<String> {
        let has_display_modifier = modifiers.contains(Modifier::Control)
            || modifiers.contains(Modifier::Alt)
            || modifiers.contains(Modifier::Meta);
        if !has_display_modifier
            || key_name.is_empty()
            || key_name
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
        {
            return None;
        }

        let mut parts = Vec::with_capacity(5);
        if modifiers.contains(Modifier::Control) {
            parts.push("Ctrl");
        }
        if modifiers.contains(Modifier::Alt) {
            parts.push("Alt");
        }
        if modifiers.contains(Modifier::Shift) {
            parts.push("Shift");
        }
        if modifiers.contains(Modifier::Meta) {
            parts.push("Win");
        }
        parts.push(key_name);
        Some(parts.join("+"))
    }
}
