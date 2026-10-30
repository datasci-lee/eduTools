#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl MonitorRect {
    pub const fn width(self) -> i32 {
        self.right - self.left
    }

    pub const fn height(self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonitorSnapshot {
    pub session_key: String,
    pub rect: MonitorRect,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct MonitorSyncPlan {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub moved: Vec<String>,
    pub unchanged: Vec<String>,
}

pub fn plan_monitor_sync(
    current: &[MonitorSnapshot],
    discovered: &[MonitorSnapshot],
) -> MonitorSyncPlan {
    use std::collections::HashMap;

    let current_by_key = current
        .iter()
        .map(|monitor| (monitor.session_key.as_str(), monitor.rect))
        .collect::<HashMap<_, _>>();
    let discovered_by_key = discovered
        .iter()
        .map(|monitor| (monitor.session_key.as_str(), monitor.rect))
        .collect::<HashMap<_, _>>();
    let mut plan = MonitorSyncPlan::default();

    for monitor in discovered {
        match current_by_key.get(monitor.session_key.as_str()) {
            None => plan.added.push(monitor.session_key.clone()),
            Some(rect) if *rect != monitor.rect => plan.moved.push(monitor.session_key.clone()),
            Some(_) => plan.unchanged.push(monitor.session_key.clone()),
        }
    }
    for monitor in current {
        if !discovered_by_key.contains_key(monitor.session_key.as_str()) {
            plan.removed.push(monitor.session_key.clone());
        }
    }
    plan.added.sort();
    plan.removed.sort();
    plan.moved.sort();
    plan.unchanged.sort();
    plan
}

#[cfg(windows)]
mod windows_impl {
    use super::{MonitorRect, MonitorSnapshot};
    use std::mem::size_of;
    use windows::core::{Error, Result, BOOL};
    use windows::Win32::Foundation::{LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFOEXW,
        MONITOR_DEFAULTTONULL,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    #[derive(Clone, Debug, PartialEq)]
    pub struct Monitor {
        pub handle: HMONITOR,
        pub rect: MonitorRect,
        pub work_rect: MonitorRect,
        pub dpi: (u32, u32),
        pub is_primary: bool,
        pub session_key: String,
    }

    impl Monitor {
        pub fn snapshot(&self) -> MonitorSnapshot {
            MonitorSnapshot {
                session_key: self.session_key.clone(),
                rect: self.rect,
            }
        }

        pub const fn win32_rect(&self) -> RECT {
            RECT {
                left: self.rect.left,
                top: self.rect.top,
                right: self.rect.right,
                bottom: self.rect.bottom,
            }
        }
    }

    struct EnumerationContext {
        monitors: Vec<Monitor>,
        error: Option<Error>,
    }

    pub fn enumerate_monitors() -> Result<Vec<Monitor>> {
        let mut context = EnumerationContext {
            monitors: Vec::new(),
            error: None,
        };
        // SAFETY: the callback is synchronous and dwData points to `context` until this call returns.
        let completed = unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(enum_monitor),
                LPARAM((&mut context as *mut EnumerationContext) as isize),
            )
        };
        if let Some(error) = context.error {
            return Err(error);
        }
        if !completed.as_bool() {
            return Err(Error::from_thread());
        }
        Ok(context.monitors)
    }

    unsafe extern "system" fn enum_monitor(
        handle: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: EnumDisplayMonitors passes back the live context pointer supplied by
        // `enumerate_monitors`; the callback is not retained.
        let context = unsafe { &mut *(data.0 as *mut EnumerationContext) };
        match unsafe { monitor_from_handle(handle) } {
            Ok(monitor) => {
                context.monitors.push(monitor);
                BOOL(1)
            }
            Err(error) => {
                context.error = Some(error);
                BOOL(0)
            }
        }
    }

    unsafe fn monitor_from_handle(handle: HMONITOR) -> Result<Monitor> {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize =
            u32::try_from(size_of::<MONITORINFOEXW>()).expect("MONITORINFOEXW size fits u32");
        // SAFETY: MONITORINFOEXW starts with MONITORINFO and cbSize advertises the full struct.
        if !unsafe { GetMonitorInfoW(handle, &mut info.monitorInfo) }.as_bool() {
            return Err(Error::from_thread());
        }
        let mut dpi_x = 0;
        let mut dpi_y = 0;
        // SAFETY: handle came from EnumDisplayMonitors and both output pointers are writable.
        unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }?;
        let device_len = info
            .szDevice
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(info.szDevice.len());
        let session_key = String::from_utf16_lossy(&info.szDevice[..device_len]);
        let rect = info.monitorInfo.rcMonitor;
        let work_rect = info.monitorInfo.rcWork;
        Ok(Monitor {
            handle,
            rect: MonitorRect {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            },
            work_rect: MonitorRect {
                left: work_rect.left,
                top: work_rect.top,
                right: work_rect.right,
                bottom: work_rect.bottom,
            },
            dpi: (dpi_x, dpi_y),
            is_primary: info.monitorInfo.dwFlags & 1 != 0,
            // `szDevice` is only a session-local reconciliation key. W10 DisplayIdentity owns
            // the persistent identity because DISPLAY1-style names can change.
            session_key,
        })
    }

    pub fn monitor_by_key<'a>(monitors: &'a [Monitor], key: &str) -> Option<&'a Monitor> {
        monitors.iter().find(|monitor| monitor.session_key == key)
    }

    #[allow(dead_code)]
    pub fn monitor_at_point(monitors: &[Monitor], point: POINT) -> Option<&Monitor> {
        // SAFETY: point is a value and MONITOR_DEFAULTTONULL avoids inventing a nearest screen.
        let handle = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONULL) };
        monitors.iter().find(|monitor| monitor.handle == handle)
    }
}

#[cfg(windows)]
#[allow(unused_imports)]
pub use windows_impl::monitor_at_point;
#[cfg(windows)]
pub use windows_impl::{enumerate_monitors, monitor_by_key, Monitor};

#[cfg(test)]
mod tests {
    use super::{plan_monitor_sync, MonitorRect, MonitorSnapshot, MonitorSyncPlan};

    fn monitor(key: &str, left: i32) -> MonitorSnapshot {
        MonitorSnapshot {
            session_key: key.to_owned(),
            rect: MonitorRect {
                left,
                top: 0,
                right: left + 100,
                bottom: 100,
            },
        }
    }

    #[test]
    fn monitor_diff_table_covers_add_remove_move_and_no_change() {
        let cases = [
            (
                "added",
                vec![monitor("A", 0)],
                vec![monitor("A", 0), monitor("B", 100)],
                MonitorSyncPlan {
                    added: vec!["B".to_owned()],
                    unchanged: vec!["A".to_owned()],
                    ..Default::default()
                },
            ),
            (
                "removed",
                vec![monitor("A", 0), monitor("B", 100)],
                vec![monitor("A", 0)],
                MonitorSyncPlan {
                    removed: vec!["B".to_owned()],
                    unchanged: vec!["A".to_owned()],
                    ..Default::default()
                },
            ),
            (
                "rect changed",
                vec![monitor("A", 0)],
                vec![monitor("A", -100)],
                MonitorSyncPlan {
                    moved: vec!["A".to_owned()],
                    ..Default::default()
                },
            ),
            (
                "unchanged",
                vec![monitor("A", 0)],
                vec![monitor("A", 0)],
                MonitorSyncPlan {
                    unchanged: vec!["A".to_owned()],
                    ..Default::default()
                },
            ),
        ];

        for (name, current, discovered, expected) in cases {
            assert_eq!(plan_monitor_sync(&current, &discovered), expected, "{name}");
        }
    }
}
