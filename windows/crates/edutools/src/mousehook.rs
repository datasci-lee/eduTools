//! Shared, non-blocking low-level mouse observation for presenter effects and pass-through safety.

use std::cell::Cell;
use windows::core::{Error, Result};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

pub const WM_APP_PRESENTER_MOUSE: u32 = 0x8000 + 300;

const SUBSCRIBER_SPOTLIGHT: u8 = 1 << 0;
const SUBSCRIBER_CURSOR_HIGHLIGHT: u8 = 1 << 1;
const SUBSCRIBER_PASS_THROUGH: u8 = 1 << 2;
const SUBSCRIBER_GLOBAL_LASER: u8 = 1 << 3;
const MOVE_SUBSCRIBERS: u8 =
    SUBSCRIBER_SPOTLIGHT | SUBSCRIBER_CURSOR_HIGHLIGHT | SUBSCRIBER_GLOBAL_LASER;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseHookSubscriber {
    Spotlight,
    CursorHighlight,
    PassThrough,
    /// 판서 모드 밖 전역 레이저: 클릭은 아래 앱으로 통과시키고 커서 이동만 관찰한다.
    GlobalLaser,
}

impl MouseHookSubscriber {
    const fn bit(self) -> u8 {
        match self {
            Self::Spotlight => SUBSCRIBER_SPOTLIGHT,
            Self::CursorHighlight => SUBSCRIBER_CURSOR_HIGHLIGHT,
            Self::PassThrough => SUBSCRIBER_PASS_THROUGH,
            Self::GlobalLaser => SUBSCRIBER_GLOBAL_LASER,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseHookEventKind {
    Moved,
    ButtonDown(MouseButton),
    ButtonUp(MouseButton),
    Wheel { delta: i16, horizontal: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MouseHookEvent {
    pub kind: MouseHookEventKind,
    pub x: i32,
    pub y: i32,
}

impl MouseHookEvent {
    pub fn decode(wparam: WPARAM, lparam: LPARAM) -> Option<Self> {
        let code = (wparam.0 & 0xff) as u8;
        let detail = ((wparam.0 >> 16) & 0xffff) as u16;
        let packed = lparam.0 as u64;
        let x = packed as u32 as i32;
        let y = (packed >> 32) as u32 as i32;
        let kind = match code {
            1 => MouseHookEventKind::Moved,
            2..=5 => MouseHookEventKind::ButtonDown(decode_button(code, detail)?),
            6..=9 => MouseHookEventKind::ButtonUp(decode_button(code - 4, detail)?),
            10 => MouseHookEventKind::Wheel {
                delta: detail as i16,
                horizontal: false,
            },
            11 => MouseHookEventKind::Wheel {
                delta: detail as i16,
                horizontal: true,
            },
            _ => return None,
        };
        Some(Self { kind, x, y })
    }

    pub const fn is_interaction(self) -> bool {
        !matches!(self.kind, MouseHookEventKind::Moved)
    }
}

fn decode_button(code: u8, detail: u16) -> Option<MouseButton> {
    match code {
        2 => Some(MouseButton::Left),
        3 => Some(MouseButton::Right),
        4 => Some(MouseButton::Middle),
        5 => Some(MouseButton::Other(detail)),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct CallbackState {
    owner: HWND,
    subscribers: u8,
}

thread_local! {
    static CALLBACK_STATE: Cell<Option<CallbackState>> = const { Cell::new(None) };
}

pub struct MouseHook {
    owner: HWND,
    handle: Option<HHOOK>,
    subscribers: u8,
}

impl MouseHook {
    pub const fn new(owner: HWND) -> Self {
        Self {
            owner,
            handle: None,
            subscribers: 0,
        }
    }

    pub fn set_subscriber(
        &mut self,
        subscriber: MouseHookSubscriber,
        enabled: bool,
    ) -> Result<bool> {
        let previous = self.subscribers;
        if enabled {
            self.subscribers |= subscriber.bit();
        } else {
            self.subscribers &= !subscriber.bit();
        }
        if previous == self.subscribers {
            return Ok(false);
        }

        if previous == 0 && self.subscribers != 0 {
            if let Err(error) = self.install() {
                self.subscribers = previous;
                return Err(error);
            }
        } else if previous != 0 && self.subscribers == 0 {
            if let Err(error) = self.uninstall() {
                self.subscribers = previous;
                CALLBACK_STATE.with(|slot| {
                    slot.set(Some(CallbackState {
                        owner: self.owner,
                        subscribers: previous,
                    }));
                });
                return Err(error);
            }
        } else {
            self.update_callback_state();
        }
        Ok(true)
    }

    pub const fn subscriber_count(&self) -> u32 {
        self.subscribers.count_ones()
    }

    pub const fn is_installed(&self) -> bool {
        self.handle.is_some()
    }

    pub fn uninstall(&mut self) -> Result<()> {
        let Some(handle) = self.handle else {
            CALLBACK_STATE.with(|slot| slot.set(None));
            return Ok(());
        };
        unsafe { UnhookWindowsHookEx(handle) }?;
        self.handle = None;
        CALLBACK_STATE.with(|slot| slot.set(None));
        Ok(())
    }

    fn install(&mut self) -> Result<()> {
        if self.handle.is_some() {
            self.update_callback_state();
            return Ok(());
        }
        let module = unsafe { GetModuleHandleW(None) }?;
        self.update_callback_state();
        let installed = unsafe {
            SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_hook_proc),
                Some(HINSTANCE(module.0)),
                0,
            )
        };
        match installed {
            Ok(handle) => {
                self.handle = Some(handle);
                Ok(())
            }
            Err(error) => {
                CALLBACK_STATE.with(|slot| slot.set(None));
                Err(Error::new(
                    error.code(),
                    format!("WH_MOUSE_LL 설치 실패: {error}"),
                ))
            }
        }
    }

    fn update_callback_state(&self) {
        CALLBACK_STATE.with(|slot| {
            slot.set(Some(CallbackState {
                owner: self.owner,
                subscribers: self.subscribers,
            }));
        });
    }
}

fn encoded_event(message: u32, event: &MSLLHOOKSTRUCT) -> Option<(usize, isize)> {
    let detail = (event.mouseData >> 16) as u16;
    let code = match message {
        WM_MOUSEMOVE => 1,
        WM_LBUTTONDOWN => 2,
        WM_RBUTTONDOWN => 3,
        WM_MBUTTONDOWN => 4,
        WM_XBUTTONDOWN => 5,
        WM_LBUTTONUP => 6,
        WM_RBUTTONUP => 7,
        WM_MBUTTONUP => 8,
        WM_XBUTTONUP => 9,
        WM_MOUSEWHEEL => 10,
        WM_MOUSEHWHEEL => 11,
        _ => return None,
    };
    let wparam = code | (usize::from(detail) << 16);
    let packed = u64::from(event.pt.x as u32) | (u64::from(event.pt.y as u32) << 32);
    Some((wparam, packed as isize))
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        CALLBACK_STATE.with(|slot| {
            let Some(state) = slot.get() else { return };
            if message == WM_MOUSEMOVE && state.subscribers & MOVE_SUBSCRIBERS == 0 {
                return;
            }
            // SAFETY: WH_MOUSE_LL supplies MSLLHOOKSTRUCT for HC_ACTION notifications.
            let event = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
            if let Some((encoded, packed)) = encoded_event(message, event) {
                let _ = unsafe {
                    PostMessageW(
                        Some(state.owner),
                        WM_APP_PRESENTER_MOUSE,
                        WPARAM(encoded),
                        LPARAM(packed),
                    )
                };
            }
        });
    }
    // This observer never consumes input and performs no work beyond PostMessage.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
