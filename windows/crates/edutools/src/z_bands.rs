//! EduTools 최상위 창의 상대 z 대역 정본.

use std::cell::RefCell;
use windows::core::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    IsWindow, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Band {
    Overlay,
    Effect,
    Tool,
    Guidance,
    Dialog,
}

const BAND_ORDER: [Band; 5] = [
    Band::Overlay,
    Band::Effect,
    Band::Tool,
    Band::Guidance,
    Band::Dialog,
];

#[derive(Default)]
struct Registry {
    windows: Vec<(Band, HWND)>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

pub struct BandRegistration {
    hwnd: HWND,
}

impl BandRegistration {
    pub fn new(hwnd: HWND, band: Band) -> Self {
        REGISTRY.with(|registry| {
            let mut registry = registry.borrow_mut();
            if !registry
                .windows
                .iter()
                .any(|(_, candidate)| *candidate == hwnd)
            {
                registry.windows.push((band, hwnd));
            }
        });
        Self { hwnd }
    }
}

impl Drop for BandRegistration {
    fn drop(&mut self) {
        REGISTRY.with(|registry| {
            registry
                .borrow_mut()
                .windows
                .retain(|(_, candidate)| *candidate != self.hwnd);
        });
    }
}

/// 낮은 대역부터 TOPMOST 그룹의 맨 위로 옮겨 마지막 대역이 최상위가 되게 한다.
pub fn raise_band_order() -> Result<()> {
    let windows = REGISTRY.with(|registry| registry.borrow().windows.clone());
    for band in BAND_ORDER {
        for (_, hwnd) in windows.iter().filter(|(candidate, _)| *candidate == band) {
            // A destroyed HWND can remain only while its Rust owner is unwinding. Ignore it here;
            // the registration guard removes it as that owner drops.
            if !unsafe { IsWindow(Some(*hwnd)) }.as_bool() {
                continue;
            }
            unsafe {
                SetWindowPos(
                    *hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
            }?;
        }
    }
    Ok(())
}
