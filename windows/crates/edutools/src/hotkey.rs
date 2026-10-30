use crate::keymap;
use crate::log::Logger;
use crate::tray::Tray;
use edutools_core::shortcut::{
    ShortcutCatalog, ShortcutDefinition, ShortcutModifiers, ShortcutScope,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};

#[derive(Clone)]
struct RegisteredHotkey {
    id: i32,
    definition: ShortcutDefinition,
}

pub struct HotkeyCenter {
    hwnd: HWND,
    registered: Vec<RegisteredHotkey>,
    logger: Logger,
}

impl HotkeyCenter {
    pub fn register_defaults(hwnd: HWND, tray: &mut Tray, logger: Logger) -> Self {
        let definitions = ShortcutCatalog::definitions()
            .into_iter()
            .filter(|definition| definition.binding(ShortcutScope::Global).is_some());
        let mut registered = Vec::new();
        for (index, definition) in definitions.enumerate() {
            let id = i32::try_from(index + 1).expect("global shortcut count must fit in i32");
            let binding = definition
                .binding(ShortcutScope::Global)
                .expect("filtered global binding must exist");
            let Some(vk) = keymap::token_to_vk(&binding.key) else {
                let message = format!(
                    "전역 키 등록 실패: {} ({}) - 지원하지 않는 키 토큰 {}",
                    definition.id, binding.portable_string, binding.key
                );
                logger.event(&message);
                tray.show_hotkey_failure(&definition.name, &binding.key_cap);
                continue;
            };
            let modifiers = windows_modifiers(binding.modifiers);
            // SAFETY: hwnd belongs to the UI thread and id is unique within this process.
            let result = unsafe { RegisterHotKey(Some(hwnd), id, modifiers, vk) };
            match result {
                Ok(()) => {
                    logger.event(&format!(
                        "전역 키 등록 성공: {} ({})",
                        definition.id, binding.portable_string
                    ));
                    registered.push(RegisteredHotkey { id, definition });
                }
                Err(error) => {
                    logger.event(&format!(
                        "전역 키 등록 선점/실패: {} ({}) - {error}",
                        definition.id, binding.portable_string
                    ));
                    tray.show_hotkey_failure(&definition.name, &binding.key_cap);
                }
            }
        }
        Self {
            hwnd,
            registered,
            logger,
        }
    }

    pub fn definition(&self, id: i32) -> Option<ShortcutDefinition> {
        self.registered
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.definition.clone())
    }

    pub fn shutdown(&mut self) {
        for entry in self.registered.drain(..) {
            // SAFETY: each id was registered for this hwnd and is unregistered once.
            if let Err(error) = unsafe { UnregisterHotKey(Some(self.hwnd), entry.id) } {
                self.logger.event(&format!(
                    "전역 키 등록 해제 실패: {} - {error}",
                    entry.definition.id
                ));
            }
        }
    }
}

fn windows_modifiers(modifiers: ShortcutModifiers) -> HOT_KEY_MODIFIERS {
    let mut bits = MOD_NOREPEAT.0;
    if modifiers.contains(ShortcutModifiers::CONTROL) {
        bits |= MOD_CONTROL.0;
    }
    if modifiers.contains(ShortcutModifiers::ALT) {
        bits |= MOD_ALT.0;
    }
    if modifiers.contains(ShortcutModifiers::SHIFT) {
        bits |= MOD_SHIFT.0;
    }
    if modifiers.contains(ShortcutModifiers::WINDOWS) {
        bits |= MOD_WIN.0;
    }
    HOT_KEY_MODIFIERS(bits)
}
