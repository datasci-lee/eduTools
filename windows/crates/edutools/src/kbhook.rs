use crate::keymap;
use crate::text_entry;
use edutools_core::dispatch::should_consume;
use edutools_core::radial::{
    radial_palette_key_decision, radial_palette_keyboard_input, RadialPaletteKeyDecision,
    RadialPaletteKeyPassReason, RadialPaletteKeyboardInput, RadialPaletteKeyboardKey,
};
use edutools_core::settings::AppSettings;
use edutools_core::shortcut::{
    ShortcutCatalog, ShortcutCommand, ShortcutDefinition, ShortcutModifiers, ShortcutScope,
};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{Error, Result};
use windows::Win32::Foundation::{E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_DOWN, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_MENU, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE,
    VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_EXTENDED, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

pub const WM_APP_KEY: u32 = 0x8000 + 3;
pub const WM_APP_RADIAL_KEY: u32 = 0x8000 + 70;
pub const WM_APP_RADIAL_KEY_CAP: u32 = 0x8000 + 71;
const KEY_UP_FLAG: isize = 1;
const CONSUMED_FLAG: isize = 1 << 1;
const ZOOM_SCOPE_FLAG: isize = 1 << 2;
const REPEAT_FLAG: isize = 1 << 2;
const RADIAL_REASON_SHIFT: usize = 3;
const DEFINITION_SHIFT: usize = 3;
const SCAN_CODE_COUNT: usize = 512;
const VK_COUNT: usize = 256;

static TEXT_INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);
static PALETTE_EXPANDED: AtomicBool = AtomicBool::new(false);
static PALETTE_VISIBLE: AtomicBool = AtomicBool::new(false);

pub fn set_text_input_active(active: bool) {
    TEXT_INPUT_ACTIVE.store(active, Ordering::Release);
}

pub fn set_radial_palette_hook_state(expanded: bool, visible: bool) {
    PALETTE_EXPANDED.store(expanded, Ordering::Release);
    PALETTE_VISIBLE.store(visible, Ordering::Release);
}

struct CallbackState {
    hwnd: HWND,
    definitions: Vec<ShortcutDefinition>,
    settings: AppSettings,
    pressed: [Option<(usize, HookScope)>; SCAN_CODE_COUNT],
    radial_pressed: [bool; VK_COUNT],
    annotation_active: bool,
    zoom_active: bool,
}

thread_local! {
    static CALLBACK_STATE: RefCell<Option<CallbackState>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookScope {
    Annotation,
    Zoom,
}

impl HookScope {
    pub const fn shortcut_scope(self) -> ShortcutScope {
        match self {
            Self::Annotation => ShortcutScope::Annotation,
            Self::Zoom => ShortcutScope::Zoom,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookKeyEvent {
    pub key_code: u32,
    pub definition_index: usize,
    pub is_key_up: bool,
    pub consumed: bool,
    pub scope: HookScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadialPaletteKeyEvent {
    pub key_code: u32,
    pub is_key_up: bool,
    pub consumed: bool,
    pub is_repeat: bool,
    pub pass_reason: Option<RadialPaletteKeyPassReason>,
}

impl RadialPaletteKeyEvent {
    pub fn decode(wparam: WPARAM, lparam: LPARAM) -> Self {
        let payload = lparam.0;
        Self {
            key_code: wparam.0 as u32,
            is_key_up: payload & KEY_UP_FLAG != 0,
            consumed: payload & CONSUMED_FLAG != 0,
            is_repeat: payload & REPEAT_FLAG != 0,
            pass_reason: decode_pass_reason((payload as usize) >> RADIAL_REASON_SHIFT),
        }
    }

    pub fn input(self) -> Option<RadialPaletteKeyboardInput> {
        radial_palette_keyboard_input(radial_key(self.key_code))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadialKeyCapHookEvent {
    ControlAloneDown,
    OtherKeyDown,
    ModifierAdded,
    ControlReleased,
}

impl RadialKeyCapHookEvent {
    pub fn decode(wparam: WPARAM) -> Option<Self> {
        match wparam.0 {
            1 => Some(Self::ControlAloneDown),
            2 => Some(Self::OtherKeyDown),
            3 => Some(Self::ModifierAdded),
            4 => Some(Self::ControlReleased),
            _ => None,
        }
    }
}

impl HookKeyEvent {
    pub fn decode(wparam: WPARAM, lparam: LPARAM) -> Self {
        let payload = lparam.0 as usize;
        Self {
            key_code: wparam.0 as u32,
            definition_index: payload >> DEFINITION_SHIFT,
            is_key_up: lparam.0 & KEY_UP_FLAG != 0,
            consumed: lparam.0 & CONSUMED_FLAG != 0,
            scope: if lparam.0 & ZOOM_SCOPE_FLAG != 0 {
                HookScope::Zoom
            } else {
                HookScope::Annotation
            },
        }
    }
}

pub struct KeyboardHook {
    handle: Option<HHOOK>,
    definitions: Vec<ShortcutDefinition>,
}

impl KeyboardHook {
    pub fn install(
        hwnd: HWND,
        settings: &AppSettings,
        annotation_active: bool,
        zoom_active: bool,
    ) -> Result<Self> {
        let definitions = ShortcutCatalog::resolved_definitions(&settings.shortcuts);
        // SAFETY: None requests the current executable module that contains the callback.
        let module = unsafe { GetModuleHandleW(None) }?;
        let already_installed = CALLBACK_STATE.with(|slot| slot.borrow().is_some());
        if already_installed {
            return Err(Error::new(E_FAIL, "keyboard hook is already installed"));
        }

        CALLBACK_STATE.with(|slot| {
            *slot.borrow_mut() = Some(CallbackState {
                hwnd,
                definitions: definitions.clone(),
                settings: settings.clone(),
                pressed: [None; SCAN_CODE_COUNT],
                radial_pressed: [false; VK_COUNT],
                annotation_active,
                zoom_active,
            });
        });

        // SAFETY: the callback has process lifetime, the module contains it, and thread id zero
        // requests the documented low-level global keyboard hook on this UI thread.
        let installed = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                Some(HINSTANCE(module.0)),
                0,
            )
        };
        match installed {
            Ok(handle) => Ok(Self {
                handle: Some(handle),
                definitions,
            }),
            Err(error) => {
                CALLBACK_STATE.with(|slot| *slot.borrow_mut() = None);
                Err(error)
            }
        }
    }

    pub fn definition(&self, index: usize) -> Option<&ShortcutDefinition> {
        self.definitions.get(index)
    }

    pub fn set_scopes(&self, annotation_active: bool, zoom_active: bool) {
        CALLBACK_STATE.with(|slot| {
            if let Some(state) = slot.borrow_mut().as_mut() {
                state.annotation_active = annotation_active;
                state.zoom_active = zoom_active;
            }
        });
    }

    pub fn has_pressed_keys(&self) -> bool {
        CALLBACK_STATE.with(|slot| {
            slot.borrow().as_ref().is_some_and(|state| {
                state.pressed.iter().any(Option::is_some)
                    || state.radial_pressed.iter().any(|pressed| *pressed)
            })
        })
    }

    pub fn uninstall(&mut self) -> Result<()> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        // SAFETY: handle was returned by SetWindowsHookExW and is unhooked at most once here.
        match unsafe { UnhookWindowsHookEx(handle) } {
            Ok(()) => {
                CALLBACK_STATE.with(|slot| *slot.borrow_mut() = None);
                Ok(())
            }
            Err(error) => {
                self.handle = Some(handle);
                Err(error)
            }
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        let _ = self.uninstall();
    }
}

pub fn unmappable_annotation_bindings(settings: &AppSettings) -> Vec<String> {
    ShortcutCatalog::resolved_definitions(&settings.shortcuts)
        .into_iter()
        .filter_map(|definition| {
            let binding = definition.binding(ShortcutScope::Annotation)?;
            keymap::token_to_vk(&binding.key)
                .is_none()
                .then(|| format!("{} ({})", definition.id, binding.key))
        })
        .collect()
}

pub fn unmappable_zoom_bindings(settings: &AppSettings) -> Vec<String> {
    ShortcutCatalog::resolved_definitions(&settings.shortcuts)
        .into_iter()
        .filter_map(|definition| {
            let binding = definition.binding(ShortcutScope::Zoom)?;
            keymap::token_to_vk(&binding.key)
                .is_none()
                .then(|| format!("{} ({})", definition.id, binding.key))
        })
        .collect()
}

#[cfg(test)]
fn matching_annotation_definition(
    definitions: &[ShortcutDefinition],
    key_code: u32,
    modifiers: ShortcutModifiers,
) -> Option<usize> {
    let key = keymap::vk_to_token(key_code)?;
    definitions.iter().position(|definition| {
        definition
            .binding(ShortcutScope::Annotation)
            .is_some_and(|binding| binding.matches(key, modifiers))
    })
}

fn matching_mode_definition(
    definitions: &[ShortcutDefinition],
    key_code: u32,
    modifiers: ShortcutModifiers,
    annotation_active: bool,
    zoom_active: bool,
) -> Option<(usize, HookScope)> {
    let key = keymap::vk_to_token(key_code)?;
    if zoom_active {
        if let Some(index) = definitions.iter().position(|definition| {
            definition
                .binding(ShortcutScope::Zoom)
                .is_some_and(|binding| binding.matches(key, modifiers))
        }) {
            return Some((index, HookScope::Zoom));
        }
    }
    if annotation_active {
        return definitions
            .iter()
            .position(|definition| {
                definition
                    .binding(ShortcutScope::Annotation)
                    .is_some_and(|binding| binding.matches(key, modifiers))
            })
            .map(|index| (index, HookScope::Annotation));
    }
    None
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        // SAFETY: forwarding untouched hook data is required for negative hook codes.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let message = wparam.0 as u32;
    let is_key_down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    let is_key_up = message == WM_KEYUP || message == WM_SYSKEYUP;
    if !is_key_down && !is_key_up {
        // SAFETY: this callback does not handle other keyboard hook notifications.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    // SAFETY: WH_KEYBOARD_LL supplies a live KBDLLHOOKSTRUCT pointer for HC_ACTION callbacks.
    let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    // EduTools never injects keyboard input, so injected events cannot form a self-triggering loop.

    let key_cap_event = if is_control_key(event.vkCode) {
        if is_key_up {
            Some(RadialKeyCapHookEvent::ControlReleased)
        } else {
            let modifiers = current_modifiers();
            let other_modifiers = modifiers.bits() & !ShortcutModifiers::CONTROL.bits();
            if PALETTE_VISIBLE.load(Ordering::Acquire) && other_modifiers == 0 {
                Some(RadialKeyCapHookEvent::ControlAloneDown)
            } else {
                Some(RadialKeyCapHookEvent::ModifierAdded)
            }
        }
    } else if is_key_down {
        Some(if is_modifier_key(event.vkCode) {
            RadialKeyCapHookEvent::ModifierAdded
        } else {
            RadialKeyCapHookEvent::OtherKeyDown
        })
    } else {
        None
    };
    if let Some(key_cap_event) = key_cap_event {
        CALLBACK_STATE.with(|slot| {
            if let Some(state) = slot.borrow().as_ref() {
                post_key_cap_event(state.hwnd, key_cap_event);
            }
        });
    }

    let radial_key_index = usize::try_from(event.vkCode)
        .ok()
        .filter(|index| *index < VK_COUNT);
    if is_key_up {
        let consumed = CALLBACK_STATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let state = slot.as_mut()?;
            let index = radial_key_index?;
            if !state.radial_pressed[index] {
                return None;
            }
            state.radial_pressed[index] = false;
            let _ = post_radial_key_event(state.hwnd, event.vkCode, true, true, false, None);
            Some(true)
        });
        if consumed == Some(true) {
            return LRESULT(1);
        }
    }

    let text_input = text_entry::text_input_active() || TEXT_INPUT_ACTIVE.load(Ordering::Acquire);
    if is_key_down && radial_palette_keyboard_input(radial_key(event.vkCode)).is_some() {
        let modifiers = current_modifiers();
        let decision = radial_palette_key_decision(
            PALETTE_EXPANDED.load(Ordering::Acquire),
            PALETTE_VISIBLE.load(Ordering::Acquire),
            text_input,
            modifiers.bits() != 0,
            radial_key(event.vkCode),
        );
        let consumed = CALLBACK_STATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let state = slot.as_mut()?;
            if !state.annotation_active {
                return None;
            }
            let zoom_has_priority = matching_mode_definition(
                &state.definitions,
                event.vkCode,
                modifiers,
                false,
                state.zoom_active,
            )
            .is_some_and(|(_, scope)| scope == HookScope::Zoom);
            if zoom_has_priority {
                return None;
            }
            let index = radial_key_index?;
            if state.radial_pressed[index] {
                let input = radial_palette_keyboard_input(radial_key(event.vkCode))?;
                if matches!(input, RadialPaletteKeyboardInput::Navigate(_)) {
                    let _ =
                        post_radial_key_event(state.hwnd, event.vkCode, false, true, true, None);
                }
                return Some(true);
            }
            match decision {
                RadialPaletteKeyDecision::Consume(_) => {
                    let posted =
                        post_radial_key_event(state.hwnd, event.vkCode, false, true, false, None);
                    if posted {
                        state.radial_pressed[index] = true;
                    }
                    Some(posted)
                }
                RadialPaletteKeyDecision::Pass(reason) => {
                    let _ = post_radial_key_event(
                        state.hwnd,
                        event.vkCode,
                        false,
                        false,
                        false,
                        Some(reason),
                    );
                    None
                }
            }
        });
        if consumed == Some(true) {
            return LRESULT(1);
        }
    }

    if text_entry::text_input_active() {
        if is_key_up {
            CALLBACK_STATE.with(|slot| {
                if let Some((key_index, _)) = physical_key(event) {
                    if let Some(state) = slot.borrow_mut().as_mut() {
                        state.pressed[key_index] = None;
                    }
                }
            });
        }
        // 네이티브 EDIT/IME가 모든 키를 직접 처리해야 하므로 판서 훅은 전달만 한다.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    let decision = CALLBACK_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut()?;
        let (key_index, physical_key_code) = physical_key(event)?;

        if is_key_up {
            let (definition_index, scope) = state.pressed[key_index].take()?;
            let posted = post_hook_event(
                state.hwnd,
                physical_key_code,
                definition_index,
                scope,
                true,
                true,
            );
            return Some(posted);
        }

        if state.pressed[key_index].is_some() {
            // Auto-repeat does not dispatch again, but remains consumed with its original keyDown.
            return Some(true);
        }

        let modifiers = current_modifiers();
        let (definition_index, scope) = matching_mode_definition(
            &state.definitions,
            event.vkCode,
            modifiers,
            state.annotation_active,
            state.zoom_active,
        )?;
        if TEXT_INPUT_ACTIVE.load(Ordering::Acquire)
            && !matches!(
                state.definitions[definition_index].command,
                ShortcutCommand::StepBack | ShortcutCommand::ShowHelp
            )
        {
            // 도움말 EDIT가 IME/문자 입력을 받을 때 닫기·도움말 토글 외 키는 전경 창으로 보낸다.
            return None;
        }
        let consumed = should_consume(
            &state.definitions[definition_index].command,
            &state.settings,
        );
        let posted = post_hook_event(
            state.hwnd,
            physical_key_code,
            definition_index,
            scope,
            false,
            consumed,
        );
        if consumed && posted {
            state.pressed[key_index] = Some((definition_index, scope));
        }
        Some(consumed && posted)
    });

    if decision == Some(true) {
        LRESULT(1)
    } else {
        // Unbound, non-consumed, and failed-to-post keys pass to the foreground application.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

fn physical_key(event: &KBDLLHOOKSTRUCT) -> Option<(usize, u32)> {
    let scan_code = usize::try_from(event.scanCode)
        .ok()
        .filter(|code| *code < 256)?;
    let extended = event.flags & LLKHF_EXTENDED == LLKHF_EXTENDED;
    let index = scan_code + if extended { 256 } else { 0 };
    let key_code = event.scanCode | if extended { 1 << 16 } else { 0 };
    Some((index, key_code))
}

fn post_hook_event(
    hwnd: HWND,
    key_code: u32,
    definition_index: usize,
    scope: HookScope,
    is_key_up: bool,
    consumed: bool,
) -> bool {
    let mut payload = (definition_index << DEFINITION_SHIFT) as isize;
    if is_key_up {
        payload |= KEY_UP_FLAG;
    }
    if consumed {
        payload |= CONSUMED_FLAG;
    }
    if scope == HookScope::Zoom {
        payload |= ZOOM_SCOPE_FLAG;
    }
    // SAFETY: WM_APP_KEY carries only integer values and hwnd belongs to the UI thread.
    unsafe {
        PostMessageW(
            Some(hwnd),
            WM_APP_KEY,
            WPARAM(key_code as usize),
            LPARAM(payload),
        )
        .is_ok()
    }
}

fn radial_key(key_code: u32) -> RadialPaletteKeyboardKey {
    match key_code {
        value if value == u32::from(VK_UP.0) => RadialPaletteKeyboardKey::Up,
        value if value == u32::from(VK_DOWN.0) => RadialPaletteKeyboardKey::Down,
        value if value == u32::from(VK_LEFT.0) => RadialPaletteKeyboardKey::Left,
        value if value == u32::from(VK_RIGHT.0) => RadialPaletteKeyboardKey::Right,
        value if value == u32::from(VK_RETURN.0) => RadialPaletteKeyboardKey::Enter,
        value if value == u32::from(VK_SPACE.0) => RadialPaletteKeyboardKey::Space,
        _ => RadialPaletteKeyboardKey::Other,
    }
}

fn pass_reason_code(reason: RadialPaletteKeyPassReason) -> usize {
    match reason {
        RadialPaletteKeyPassReason::Hidden => 1,
        RadialPaletteKeyPassReason::Collapsed => 2,
        RadialPaletteKeyPassReason::TextInput => 3,
        RadialPaletteKeyPassReason::Modifiers => 4,
        RadialPaletteKeyPassReason::Unsupported => 5,
    }
}

fn decode_pass_reason(code: usize) -> Option<RadialPaletteKeyPassReason> {
    match code {
        1 => Some(RadialPaletteKeyPassReason::Hidden),
        2 => Some(RadialPaletteKeyPassReason::Collapsed),
        3 => Some(RadialPaletteKeyPassReason::TextInput),
        4 => Some(RadialPaletteKeyPassReason::Modifiers),
        5 => Some(RadialPaletteKeyPassReason::Unsupported),
        _ => None,
    }
}

fn post_radial_key_event(
    hwnd: HWND,
    key_code: u32,
    is_key_up: bool,
    consumed: bool,
    is_repeat: bool,
    pass_reason: Option<RadialPaletteKeyPassReason>,
) -> bool {
    let mut payload = 0_isize;
    if is_key_up {
        payload |= KEY_UP_FLAG;
    }
    if consumed {
        payload |= CONSUMED_FLAG;
    }
    if is_repeat {
        payload |= REPEAT_FLAG;
    }
    if let Some(reason) = pass_reason {
        payload |= (pass_reason_code(reason) << RADIAL_REASON_SHIFT) as isize;
    }
    unsafe {
        PostMessageW(
            Some(hwnd),
            WM_APP_RADIAL_KEY,
            WPARAM(key_code as usize),
            LPARAM(payload),
        )
        .is_ok()
    }
}

fn post_key_cap_event(hwnd: HWND, event: RadialKeyCapHookEvent) {
    let value = match event {
        RadialKeyCapHookEvent::ControlAloneDown => 1,
        RadialKeyCapHookEvent::OtherKeyDown => 2,
        RadialKeyCapHookEvent::ModifierAdded => 3,
        RadialKeyCapHookEvent::ControlReleased => 4,
    };
    let _ = unsafe { PostMessageW(Some(hwnd), WM_APP_RADIAL_KEY_CAP, WPARAM(value), LPARAM(0)) };
}

fn is_control_key(key_code: u32) -> bool {
    key_code == u32::from(VK_CONTROL.0)
        || key_code == u32::from(VK_LCONTROL.0)
        || key_code == u32::from(VK_RCONTROL.0)
}

fn is_modifier_key(key_code: u32) -> bool {
    is_control_key(key_code)
        || key_code == u32::from(VK_MENU.0)
        || key_code == u32::from(VK_LMENU.0)
        || key_code == u32::from(VK_RMENU.0)
        || key_code == u32::from(VK_SHIFT.0)
        || key_code == u32::from(VK_LSHIFT.0)
        || key_code == u32::from(VK_RSHIFT.0)
        || key_code == u32::from(VK_LWIN.0)
        || key_code == u32::from(VK_RWIN.0)
}

pub(crate) fn current_modifiers() -> ShortcutModifiers {
    let mut modifiers = ShortcutModifiers::empty();
    if key_is_down(VK_CONTROL.0) {
        modifiers.insert(ShortcutModifiers::CONTROL);
    }
    if key_is_down(VK_MENU.0) {
        modifiers.insert(ShortcutModifiers::ALT);
    }
    if key_is_down(VK_SHIFT.0) {
        modifiers.insert(ShortcutModifiers::SHIFT);
    }
    if key_is_down(VK_LWIN.0) || key_is_down(VK_RWIN.0) {
        modifiers.insert(ShortcutModifiers::WINDOWS);
    }
    modifiers
}

fn key_is_down(key: u16) -> bool {
    // SAFETY: GetAsyncKeyState accepts all virtual-key values; the high bit is the down state.
    unsafe { GetAsyncKeyState(i32::from(key)) as u16 & 0x8000 != 0 }
}

#[cfg(test)]
mod tests {
    use super::{matching_annotation_definition, matching_mode_definition, HookScope};
    use crate::render::px_per_dip;
    use edutools_core::settings::AppSettings;
    use edutools_core::shortcut::{ShortcutCatalog, ShortcutModifiers};

    #[test]
    fn annotation_vk_resolution_and_dip_conversion_table() {
        let definitions = ShortcutCatalog::resolved_definitions(&AppSettings::default().shortcuts);
        let ctrl = ShortcutModifiers::CONTROL;
        let shift = ShortcutModifiers::SHIFT;
        let cases = [
            (0x50, ShortcutModifiers::empty(), Some("tool.pen")),
            (0xBF, shift, Some("help.show")),
            (0x5A, ctrl, Some("edit.undo")),
            (0x5A, ctrl | shift, Some("edit.redo")),
            (0x31, ShortcutModifiers::empty(), Some("color.1")),
            (0x50, shift, None),
            (0x70, ShortcutModifiers::empty(), None),
        ];
        for (vk, modifiers, expected) in cases {
            let actual = matching_annotation_definition(&definitions, vk, modifiers)
                .map(|index| definitions[index].id.as_str());
            assert_eq!(
                actual,
                expected,
                "VK {vk:#x}, modifiers={}",
                modifiers.bits()
            );
        }
        assert!((3.0 * px_per_dip(144) - 4.5).abs() < f64::EPSILON);
    }

    #[test]
    fn zoom_scope_precedes_annotation_scope_table() {
        let definitions = ShortcutCatalog::resolved_definitions(&AppSettings::default().shortcuts);
        let cases = [
            (
                0x20,
                ShortcutModifiers::empty(),
                true,
                true,
                Some(HookScope::Zoom),
            ),
            (
                0x50,
                ShortcutModifiers::empty(),
                true,
                true,
                Some(HookScope::Zoom),
            ),
            (
                0x42,
                ShortcutModifiers::empty(),
                true,
                true,
                Some(HookScope::Annotation),
            ),
            (0x42, ShortcutModifiers::empty(), false, true, None),
        ];
        for (vk, modifiers, annotation, zoom, expected_scope) in cases {
            let actual = matching_mode_definition(&definitions, vk, modifiers, annotation, zoom)
                .map(|(_, scope)| scope);
            assert_eq!(actual, expected_scope, "VK {vk:#x}");
        }
    }
}
