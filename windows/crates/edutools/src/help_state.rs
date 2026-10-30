//! HelpController.shortcutDidPress/Release와 같은 도움말 표시 상태기계.

pub const HOLD_THRESHOLD_SECONDS: f64 = 0.3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpMode {
    Closed,
    Held,
    Pinned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpTransition {
    None,
    ShowHeld,
    ShowPinned,
    Close,
    IgnoreRelease,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HelpState {
    mode: HelpMode,
    pressed_at: Option<f64>,
    ignores_next_release: bool,
}

impl Default for HelpState {
    fn default() -> Self {
        Self {
            mode: HelpMode::Closed,
            pressed_at: None,
            ignores_next_release: false,
        }
    }
}

impl HelpState {
    #[cfg(test)]
    pub fn mode(&self) -> HelpMode {
        self.mode
    }

    pub fn is_visible(&self) -> bool {
        self.mode != HelpMode::Closed
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn is_pinned(&self) -> bool {
        self.mode == HelpMode::Pinned
    }

    // Swift: HelpController.shortcutDidPress(keyCode:)
    pub fn shortcut_did_press(&mut self, now: f64) -> HelpTransition {
        if self.mode == HelpMode::Pinned {
            self.ignores_next_release = true;
            self.mode = HelpMode::Closed;
            self.pressed_at = None;
            return HelpTransition::Close;
        }
        if self.pressed_at.is_some() {
            return HelpTransition::None;
        }
        self.pressed_at = Some(now);
        self.mode = HelpMode::Held;
        HelpTransition::ShowHeld
    }

    // Swift: HelpController.shortcutDidRelease()
    pub fn shortcut_did_release(&mut self, now: f64) -> HelpTransition {
        if self.ignores_next_release {
            self.ignores_next_release = false;
            return HelpTransition::IgnoreRelease;
        }
        let Some(pressed_at) = self.pressed_at.take() else {
            return HelpTransition::None;
        };
        if now - pressed_at >= HOLD_THRESHOLD_SECONDS {
            self.mode = HelpMode::Closed;
            HelpTransition::Close
        } else {
            self.mode = HelpMode::Pinned;
            HelpTransition::ShowPinned
        }
    }

    // Swift: HelpController.showPinned()
    pub fn toggle_pinned(&mut self) -> HelpTransition {
        self.pressed_at = None;
        if self.is_visible() {
            self.mode = HelpMode::Closed;
            HelpTransition::Close
        } else {
            self.mode = HelpMode::Pinned;
            HelpTransition::ShowPinned
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn close(&mut self) -> HelpTransition {
        if !self.is_visible() {
            return HelpTransition::None;
        }
        self.mode = HelpMode::Closed;
        self.pressed_at = None;
        HelpTransition::Close
    }
}

#[cfg(test)]
mod tests {
    use super::{HelpMode, HelpState, HelpTransition};

    #[test]
    fn press_release_transition_table_matches_swift_boundary() {
        #[derive(Clone, Copy)]
        struct Case {
            name: &'static str,
            pinned_before_press: bool,
            elapsed: f64,
            expected_press: HelpTransition,
            expected_release: HelpTransition,
            expected_mode: HelpMode,
        }

        let cases = [
            Case {
                name: "closed short 0.29 pins",
                pinned_before_press: false,
                elapsed: 0.29,
                expected_press: HelpTransition::ShowHeld,
                expected_release: HelpTransition::ShowPinned,
                expected_mode: HelpMode::Pinned,
            },
            Case {
                name: "closed held 0.30 closes",
                pinned_before_press: false,
                elapsed: 0.30,
                expected_press: HelpTransition::ShowHeld,
                expected_release: HelpTransition::Close,
                expected_mode: HelpMode::Closed,
            },
            Case {
                name: "pinned short press closes and ignores release",
                pinned_before_press: true,
                elapsed: 0.29,
                expected_press: HelpTransition::Close,
                expected_release: HelpTransition::IgnoreRelease,
                expected_mode: HelpMode::Closed,
            },
            Case {
                name: "pinned held press closes and ignores release",
                pinned_before_press: true,
                elapsed: 0.30,
                expected_press: HelpTransition::Close,
                expected_release: HelpTransition::IgnoreRelease,
                expected_mode: HelpMode::Closed,
            },
        ];

        for case in cases {
            let mut state = HelpState::default();
            if case.pinned_before_press {
                assert_eq!(state.toggle_pinned(), HelpTransition::ShowPinned);
            }
            assert_eq!(
                state.shortcut_did_press(10.0),
                case.expected_press,
                "{} press",
                case.name
            );
            assert_eq!(
                state.shortcut_did_release(10.0 + case.elapsed),
                case.expected_release,
                "{} release",
                case.name
            );
            assert_eq!(state.mode(), case.expected_mode, "{} mode", case.name);
        }
    }
}
