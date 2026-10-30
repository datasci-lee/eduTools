use edutools_core::display_preset::{
    is_valid_name, unique_name, DisplayGrid, DisplayIdentity, DisplayPreset,
    DisplayPresetValidator, DisplaySettings, GridCell, PresetId, PresetMonitor, PresetScreen,
};
#[cfg(windows)]
use edutools_core::display_preset::{
    DisplayMatcher, DisplayPlanner, DisplayPresetDefaults, OnlineDisplay,
};
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
static PRESET_NONCE: AtomicU64 = AtomicU64::new(1);

#[cfg(windows)]
pub fn new_preset_id() -> PresetId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let nonce = u128::from(PRESET_NONCE.fetch_add(1, Ordering::Relaxed));
    let process = u128::from(std::process::id());
    let value = nanos ^ (process << 64) ^ nonce;
    let text = format!(
        "{:08X}-{:04X}-4{:03X}-A{:03X}-{:012X}",
        (value >> 96) as u32,
        ((value >> 80) & 0xffff) as u16,
        ((value >> 68) & 0x0fff) as u16,
        ((value >> 52) & 0x0fff) as u16,
        value & 0x000f_ffff_ffff_ffff,
    );
    PresetId::parse(&text).expect("generated preset id has UUID shape")
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresetScreenSlot {
    Main,
    Secondary1,
    Secondary2,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl PresetScreenSlot {
    const ALL: [Self; 3] = [Self::Main, Self::Secondary1, Self::Secondary2];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Main => "주 화면",
            Self::Secondary1 => "보조화면1",
            Self::Secondary2 => "보조화면2",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridCellState {
    Available,
    Selected,
    Main,
    Occupied,
    Disconnected,
}

pub fn grid_cell_state(
    preset: &DisplayPreset,
    slot: PresetScreenSlot,
    candidate: GridCell,
) -> GridCellState {
    let Some(selected) = screen(preset, slot) else {
        return GridCellState::Disconnected;
    };
    if selected.cell == candidate {
        return if slot == PresetScreenSlot::Main {
            GridCellState::Main
        } else {
            GridCellState::Selected
        };
    }
    if preset.occupied_cells().contains(&candidate) {
        return GridCellState::Occupied;
    }
    if slot == PresetScreenSlot::Main {
        return GridCellState::Disconnected;
    }
    let mut candidate_preset = preset.clone();
    if let Some(value) = screen_mut(&mut candidate_preset, slot) {
        value.cell = candidate;
    }
    if DisplayGrid::validation_error(&candidate_preset).is_some() {
        GridCellState::Disconnected
    } else {
        GridCellState::Available
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug)]
pub enum DisplayEdit {
    SetGridCell {
        preset: usize,
        slot: PresetScreenSlot,
        cell: GridCell,
    },
    AddScreen {
        preset: usize,
        slot: PresetScreenSlot,
    },
    RemoveScreen {
        preset: usize,
        slot: PresetScreenSlot,
    },
    AddMonitor {
        preset: usize,
        slot: PresetScreenSlot,
        identity: DisplayIdentity,
    },
    ReplaceMonitor {
        preset: usize,
        slot: PresetScreenSlot,
        monitor: usize,
        identity: DisplayIdentity,
    },
    SetAutoMatch {
        preset: usize,
        slot: PresetScreenSlot,
        monitor: usize,
        enabled: bool,
    },
    RemoveMonitor {
        preset: usize,
        slot: PresetScreenSlot,
        monitor: usize,
    },
    MoveMonitorWithin {
        preset: usize,
        slot: PresetScreenSlot,
        monitor: usize,
        offset: isize,
    },
    MoveMonitorBetween {
        preset: usize,
        source: PresetScreenSlot,
        monitor: usize,
        destination: PresetScreenSlot,
    },
}

pub fn apply_display_edit(
    display: &DisplaySettings,
    edit: &DisplayEdit,
) -> Result<DisplaySettings, String> {
    let mut candidate = display.clone();
    let preset_index = match edit {
        DisplayEdit::SetGridCell { preset, .. }
        | DisplayEdit::AddScreen { preset, .. }
        | DisplayEdit::RemoveScreen { preset, .. }
        | DisplayEdit::AddMonitor { preset, .. }
        | DisplayEdit::ReplaceMonitor { preset, .. }
        | DisplayEdit::SetAutoMatch { preset, .. }
        | DisplayEdit::RemoveMonitor { preset, .. }
        | DisplayEdit::MoveMonitorWithin { preset, .. }
        | DisplayEdit::MoveMonitorBetween { preset, .. } => *preset,
    };
    let preset = candidate
        .presets
        .get_mut(preset_index)
        .ok_or_else(|| "선택한 프리셋이 없습니다".to_owned())?;
    match edit {
        DisplayEdit::SetGridCell { slot, cell, .. } => {
            let value = screen_mut(preset, *slot)
                .ok_or_else(|| "선택한 화면 슬롯이 없습니다".to_owned())?;
            value.cell = *cell;
        }
        DisplayEdit::AddScreen { slot, .. } => match slot {
            PresetScreenSlot::Main => return Err("주 화면은 이미 있습니다".to_owned()),
            PresetScreenSlot::Secondary1 if preset.secondary1.is_none() => {
                let cell = next_available_cell(preset)
                    .ok_or_else(|| "연결 가능한 빈 격자 칸이 없습니다".to_owned())?;
                preset.secondary1 = Some(PresetScreen {
                    monitors: Vec::new(),
                    cell,
                });
            }
            PresetScreenSlot::Secondary2
                if preset.secondary1.is_some() && preset.secondary2.is_none() =>
            {
                let cell = next_available_cell(preset)
                    .ok_or_else(|| "연결 가능한 빈 격자 칸이 없습니다".to_owned())?;
                preset.secondary2 = Some(PresetScreen {
                    monitors: Vec::new(),
                    cell,
                });
            }
            PresetScreenSlot::Secondary2 => return Err("보조화면1을 먼저 추가하세요".to_owned()),
            _ => return Err("화면 슬롯이 이미 있습니다".to_owned()),
        },
        DisplayEdit::RemoveScreen { slot, .. } => match slot {
            PresetScreenSlot::Main => return Err("주 화면은 제거할 수 없습니다".to_owned()),
            PresetScreenSlot::Secondary1 if preset.secondary2.is_some() => {
                return Err("보조화면2를 먼저 제거하세요".to_owned())
            }
            PresetScreenSlot::Secondary1 => preset.secondary1 = None,
            PresetScreenSlot::Secondary2 => preset.secondary2 = None,
        },
        DisplayEdit::AddMonitor { slot, identity, .. } => {
            let value = screen_mut(preset, *slot)
                .ok_or_else(|| "선택한 화면 슬롯이 없습니다".to_owned())?;
            value.monitors.push(PresetMonitor {
                identity: identity.clone(),
                auto_match: false,
            });
        }
        DisplayEdit::ReplaceMonitor {
            slot,
            monitor,
            identity,
            ..
        } => {
            let value = screen_mut(preset, *slot)
                .ok_or_else(|| "선택한 화면 슬롯이 없습니다".to_owned())?;
            let row = value
                .monitors
                .get_mut(*monitor)
                .ok_or_else(|| "선택한 모니터 행이 없습니다".to_owned())?;
            row.identity = identity.clone();
        }
        DisplayEdit::SetAutoMatch {
            slot,
            monitor,
            enabled,
            ..
        } => {
            let value = screen_mut(preset, *slot)
                .ok_or_else(|| "선택한 화면 슬롯이 없습니다".to_owned())?;
            let row = value
                .monitors
                .get_mut(*monitor)
                .ok_or_else(|| "선택한 모니터 행이 없습니다".to_owned())?;
            row.auto_match = *enabled;
        }
        DisplayEdit::RemoveMonitor { slot, monitor, .. } => {
            let value = screen_mut(preset, *slot)
                .ok_or_else(|| "선택한 화면 슬롯이 없습니다".to_owned())?;
            if *slot == PresetScreenSlot::Main && value.monitors.len() <= 1 {
                return Err("주 화면의 마지막 모니터는 제거할 수 없습니다".to_owned());
            }
            if *monitor >= value.monitors.len() {
                return Err("선택한 모니터 행이 없습니다".to_owned());
            }
            value.monitors.remove(*monitor);
        }
        DisplayEdit::MoveMonitorWithin {
            slot,
            monitor,
            offset,
            ..
        } => {
            let value = screen_mut(preset, *slot)
                .ok_or_else(|| "선택한 화면 슬롯이 없습니다".to_owned())?;
            let destination = monitor
                .checked_add_signed(*offset)
                .ok_or_else(|| "모니터 이동 범위를 벗어났습니다".to_owned())?;
            if *monitor >= value.monitors.len() || destination >= value.monitors.len() {
                return Err("모니터 이동 범위를 벗어났습니다".to_owned());
            }
            value.monitors.swap(*monitor, destination);
        }
        DisplayEdit::MoveMonitorBetween {
            source,
            monitor,
            destination,
            ..
        } => {
            if source == destination {
                return Err("같은 화면 슬롯으로는 이동할 수 없습니다".to_owned());
            }
            let source_screen =
                screen(preset, *source).ok_or_else(|| "출발 화면 슬롯이 없습니다".to_owned())?;
            if *source == PresetScreenSlot::Main && source_screen.monitors.len() <= 1 {
                return Err("주 화면의 마지막 모니터는 이동할 수 없습니다".to_owned());
            }
            let monitor_value = source_screen
                .monitors
                .get(*monitor)
                .cloned()
                .ok_or_else(|| "선택한 모니터 행이 없습니다".to_owned())?;
            screen_mut(preset, *source)
                .expect("source was checked")
                .monitors
                .remove(*monitor);
            screen_mut(preset, *destination)
                .ok_or_else(|| "도착 화면 슬롯이 없습니다".to_owned())?
                .monitors
                .push(monitor_value);
        }
    }
    DisplayPresetValidator::validate(&candidate).map_err(|error| error.to_string())?;
    Ok(candidate)
}

fn next_available_cell(preset: &DisplayPreset) -> Option<GridCell> {
    let occupied = preset.occupied_cells();
    GridCell::placement_order().into_iter().find(|candidate| {
        !occupied.contains(candidate) && {
            let mut cells = occupied.clone();
            cells.push(*candidate);
            DisplayGrid::is_connected(&cells)
        }
    })
}

fn screen(preset: &DisplayPreset, slot: PresetScreenSlot) -> Option<&PresetScreen> {
    match slot {
        PresetScreenSlot::Main => Some(&preset.main),
        PresetScreenSlot::Secondary1 => preset.secondary1.as_ref(),
        PresetScreenSlot::Secondary2 => preset.secondary2.as_ref(),
    }
}

fn screen_mut(preset: &mut DisplayPreset, slot: PresetScreenSlot) -> Option<&mut PresetScreen> {
    match slot {
        PresetScreenSlot::Main => Some(&mut preset.main),
        PresetScreenSlot::Secondary1 => preset.secondary1.as_mut(),
        PresetScreenSlot::Secondary2 => preset.secondary2.as_mut(),
    }
}

pub fn synthetic_edit_round_trip_failure() -> Option<String> {
    let preset_id = match PresetId::parse("00000000-0000-4000-A000-000000000121") {
        Ok(value) => value,
        Err(error) => return Some(format!("화면 탭 합성 프리셋 ID 생성 실패: {error}")),
    };
    let monitor = |name: &str, serial: u32| PresetMonitor {
        identity: DisplayIdentity {
            uuid: format!("SELFTEST-{serial}"),
            vendor: 1,
            model: serial,
            serial,
            name: name.to_owned(),
            kind: edutools_core::display_preset::DisplayKind::External,
        },
        auto_match: false,
    };
    let original = DisplaySettings {
        auto_switch_enabled: false,
        presets: vec![DisplayPreset {
            id: preset_id,
            name: "selftest".to_owned(),
            main: PresetScreen {
                monitors: vec![monitor("main", 1)],
                cell: GridCell::center(),
            },
            secondary1: Some(PresetScreen {
                monitors: vec![monitor("secondary", 2)],
                cell: GridCell { x: 1, y: 0 },
            }),
            secondary2: None,
        }],
    };
    let moved = match apply_display_edit(
        &original,
        &DisplayEdit::MoveMonitorBetween {
            preset: 0,
            source: PresetScreenSlot::Secondary1,
            monitor: 0,
            destination: PresetScreenSlot::Main,
        },
    ) {
        Ok(value) => value,
        Err(error) => return Some(format!("화면 탭 합성 편집 이동 실패: {error}")),
    };
    let restored = match apply_display_edit(
        &moved,
        &DisplayEdit::MoveMonitorBetween {
            preset: 0,
            source: PresetScreenSlot::Main,
            monitor: 1,
            destination: PresetScreenSlot::Secondary1,
        },
    ) {
        Ok(value) => value,
        Err(error) => return Some(format!("화면 탭 합성 편집 복원 실패: {error}")),
    };
    (restored != original).then_some("화면 탭 합성 편집 왕복 결과가 원본과 다름".to_owned())
}

#[cfg(windows)]
#[derive(Clone, Debug)]
pub enum DisplayMutationKind {
    AutoSwitch,
    Rename {
        preset: usize,
    },
    Duplicate {
        source: usize,
        inserted: usize,
    },
    Delete {
        preset: usize,
    },
    MovePreset {
        source: usize,
        destination: usize,
    },
    Edit(DisplayEdit),
    Capture {
        preset: usize,
        warnings: Vec<String>,
    },
}

#[cfg(windows)]
#[derive(Clone, Debug)]
pub struct DisplayMutationRequest {
    pub display: DisplaySettings,
    pub kind: DisplayMutationKind,
}

#[cfg(windows)]
#[derive(Clone, Debug)]
pub enum DisplayCommand {
    ShowPlan(usize),
    Capture,
    Apply(usize),
}

#[cfg(windows)]
mod windows_ui {
    use super::*;
    use crate::monitor::{current_display_configuration, hash_prefix8};
    use crate::settings_ui::{dip, layout_metrics, wide, DisplayApplyAvailability};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicI32, Ordering};
    use windows::core::{w, Result, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, DrawFocusRect, DrawTextW, FillRect, FrameRect, GetSysColor,
        SetBkMode, SetTextColor, COLOR_BTNFACE, COLOR_BTNTEXT, COLOR_GRAYTEXT, COLOR_HIGHLIGHT,
        COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
        TRANSPARENT,
    };
    use windows::Win32::UI::Controls::{
        DRAWITEMSTRUCT, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVIF_STATE, LVIF_TEXT, LVIS_FOCUSED,
        LVIS_SELECTED, LVITEMW, LVNI_SELECTED, LVN_ITEMCHANGED, LVS_EX_FULLROWSELECT, LVS_REPORT,
        LVS_SHOWSELALWAYS, LVS_SINGLESEL, NMHDR, NMLISTVIEW, WC_LISTVIEWW,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetWindowTextLengthW, GetWindowTextW, IsWindow,
        SendMessageW, SetWindowPos, SetWindowTextW, BS_AUTOCHECKBOX, BS_OWNERDRAW, BS_PUSHBUTTON,
        CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, EN_KILLFOCUS,
        ES_AUTOHSCROLL, HMENU, SWP_NOACTIVATE, SWP_NOZORDER, WINDOW_EX_STYLE, WINDOW_STYLE,
        WM_SETFONT, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE,
    };

    const LVM_FIRST: u32 = 0x1000;
    const LVM_GETNEXTITEM: u32 = LVM_FIRST + 12;
    const LVM_DELETEALLITEMS: u32 = LVM_FIRST + 9;
    const LVM_SETITEMSTATE: u32 = LVM_FIRST + 43;
    const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = LVM_FIRST + 54;
    const LVM_INSERTITEMW: u32 = LVM_FIRST + 77;
    const LVM_SETITEMTEXTW: u32 = LVM_FIRST + 116;
    const LVM_INSERTCOLUMNW: u32 = LVM_FIRST + 97;
    const DISPLAY_CONTROL_BASE: i32 = 8_000;
    static NEXT_CONTROL_ID: AtomicI32 = AtomicI32::new(DISPLAY_CONTROL_BASE);

    #[derive(Default)]
    pub struct DisplayPageEvent {
        pub mutation: Option<DisplayMutationRequest>,
        pub command: Option<DisplayCommand>,
        pub error: Option<String>,
        pub changed_selection: bool,
    }

    pub struct DisplayPage {
        parent: HWND,
        hinstance: HINSTANCE,
        font: windows::Win32::Graphics::Gdi::HFONT,
        auto_switch: HWND,
        stability_help: HWND,
        priority_help: HWND,
        preset_list: HWND,
        capture: HWND,
        rename_edit: HWND,
        rename: HWND,
        duplicate: HWND,
        delete: HWND,
        move_up: HWND,
        move_down: HWND,
        plan: HWND,
        apply: HWND,
        refresh: HWND,
        capacity: HWND,
        apply_status: HWND,
        screens: Vec<ScreenUi>,
        selected_preset: Option<usize>,
        online: Vec<OnlineDisplay>,
        online_error: Option<String>,
        apply_availability: DisplayApplyAvailability,
    }

    struct ScreenUi {
        slot: PresetScreenSlot,
        title: HWND,
        add_screen: Option<HWND>,
        remove_screen: Option<HWND>,
        grid: Vec<GridUi>,
        add_combo: Option<HWND>,
        add_monitor: Option<HWND>,
        monitors: Vec<MonitorUi>,
    }

    struct GridUi {
        hwnd: HWND,
        cell: GridCell,
        state: GridCellState,
        label: &'static str,
    }

    struct MonitorUi {
        index: usize,
        label: HWND,
        status: HWND,
        auto_match: HWND,
        replacement: HWND,
        remove: HWND,
        up: HWND,
        down: HWND,
        destination: HWND,
        move_button: HWND,
    }

    impl DisplayPage {
        pub fn new(parent: HWND, hinstance: HINSTANCE) -> Result<Self> {
            let auto_switch = child(
                parent,
                hinstance,
                w!("BUTTON"),
                "연결된 모니터에 따라 자동 전환",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                WINDOW_EX_STYLE(0),
            )?;
            let stability_help = child(
                parent,
                hinstance,
                w!("STATIC"),
                "",
                WS_CHILD | WS_VISIBLE,
                WINDOW_EX_STYLE(0),
            )?;
            let priority_help = child(
                parent,
                hinstance,
                w!("STATIC"),
                "자동 집합이 같으면 목록의 위쪽 프리셋이 우선입니다.",
                WS_CHILD | WS_VISIBLE,
                WINDOW_EX_STYLE(0),
            )?;
            let preset_list = child_class(
                parent,
                hinstance,
                WC_LISTVIEWW,
                PCWSTR::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WINDOW_STYLE(LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS),
                WS_EX_CLIENTEDGE,
            )?;
            unsafe {
                SendMessageW(
                    preset_list,
                    LVM_SETEXTENDEDLISTVIEWSTYLE,
                    Some(WPARAM(LVS_EX_FULLROWSELECT as usize)),
                    Some(LPARAM(LVS_EX_FULLROWSELECT as isize)),
                );
            }
            insert_column(preset_list, 0, "번호", 70);
            insert_column(preset_list, 1, "이름", 360);
            insert_column(preset_list, 2, "구성", 120);

            let capture = button(parent, hinstance, "현재 구성 캡처로 추가")?;
            let rename_edit = child(
                parent,
                hinstance,
                w!("EDIT"),
                "",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                WS_EX_CLIENTEDGE,
            )?;
            let rename = button(parent, hinstance, "이름 변경")?;
            let duplicate = button(parent, hinstance, "복제")?;
            let delete = button(parent, hinstance, "삭제")?;
            let move_up = button(parent, hinstance, "위로")?;
            let move_down = button(parent, hinstance, "아래로")?;
            let plan = button(parent, hinstance, "계획 보기")?;
            let apply = button(parent, hinstance, "지금 적용")?;
            let refresh = button(parent, hinstance, "연결 상태 새로 고침")?;
            let capacity = static_text(parent, hinstance, "")?;
            let apply_status = static_text(parent, hinstance, "")?;
            Ok(Self {
                parent,
                hinstance,
                font: Default::default(),
                auto_switch,
                stability_help,
                priority_help,
                preset_list,
                capture,
                rename_edit,
                rename,
                duplicate,
                delete,
                move_up,
                move_down,
                plan,
                apply,
                refresh,
                capacity,
                apply_status,
                screens: Vec::new(),
                selected_preset: None,
                online: Vec::new(),
                online_error: None,
                apply_availability: DisplayApplyAvailability::available(),
            })
        }

        pub fn set_font(&mut self, font: windows::Win32::Graphics::Gdi::HFONT) {
            self.font = font;
            self.for_each_control(|hwnd| unsafe {
                SendMessageW(
                    hwnd,
                    WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
            });
        }

        pub fn refresh_online(&mut self, show_error: bool) -> Option<String> {
            match current_display_configuration() {
                Ok(configuration) => {
                    self.online = configuration.online;
                    self.online_error = None;
                    None
                }
                Err(error) => {
                    let reason = format!("모니터 목록을 읽을 수 없습니다: {error}");
                    self.online_error = Some(reason.clone());
                    show_error.then_some(reason)
                }
            }
        }

        pub fn select_preset(&mut self, index: usize) {
            self.selected_preset = Some(index);
        }

        pub fn refresh(
            &mut self,
            display: &DisplaySettings,
            availability: DisplayApplyAvailability,
        ) -> Result<()> {
            self.apply_availability = availability;
            set_check(self.auto_switch, display.auto_switch_enabled);
            set_text(
                self.stability_help,
                &format!(
                    "연결된 모니터가 프리셋의 자동 체크 모니터와 정확히 같으면 {:.0}초 안정된 뒤 자동으로 적용합니다. 적용 뒤 {:.0}초는 다시 바꾸지 않습니다.",
                    DisplayPresetDefaults::CONNECTION_STABILITY_INTERVAL,
                    DisplayPresetDefaults::AUTOMATIC_APPLY_COOLDOWN,
                ),
            );
            let match_description =
                DisplayPlanner::preset_for_connected_set(&self.online, &display.presets)
                    .and_then(|matched| {
                        display
                            .presets
                            .iter()
                            .position(|preset| preset.id == matched.id)
                            .map(|index| {
                                format!(
                                    "\r\n현재 연결 집합 일치: {}순위 {}",
                                    index + 1,
                                    matched.name
                                )
                            })
                    })
                    .unwrap_or_default();
            set_text(
                self.priority_help,
                &format!("자동 집합이 같으면 목록의 위쪽 프리셋이 우선입니다.{match_description}"),
            );
            unsafe {
                SendMessageW(self.preset_list, LVM_DELETEALLITEMS, None, None);
            }
            for (index, preset) in display.presets.iter().enumerate() {
                insert_list_item(self.preset_list, index, 0, &(index + 1).to_string());
                set_list_item(self.preset_list, index, 1, &preset.name);
                set_list_item(
                    self.preset_list,
                    index,
                    2,
                    &format!("모니터 {}대", preset.monitor_count()),
                );
            }
            self.selected_preset = match (self.selected_preset, display.presets.len()) {
                (_, 0) => None,
                (Some(index), length) if index < length => Some(index),
                (_, length) => Some(length - 1),
            };
            if let Some(index) = self.selected_preset {
                select_list_item(self.preset_list, index);
                set_text(self.rename_edit, &display.presets[index].name);
            } else {
                set_text(self.rename_edit, "");
            }
            set_text(
                self.capacity,
                &format!(
                    "프리셋 {}/{}개",
                    display.presets.len(),
                    DisplayPresetDefaults::MAXIMUM_PRESET_COUNT
                ),
            );
            self.update_buttons(display);
            self.rebuild_editor(display)
        }

        pub fn layout(&mut self, width: i32, dpi: u32, scroll: i32) -> i32 {
            let metrics = layout_metrics(dpi);
            let padding = metrics.page_padding;
            let gap = metrics.row_gap;
            let button_height = metrics.control_height;
            let usable = (width - padding * 2 - metrics.scrollbar_gutter).max(1);
            let mut y = padding - scroll;
            place(self.auto_switch, padding, y, usable, button_height);
            y += button_height + dip(3, dpi);
            place(self.stability_help, padding, y, usable, dip(22, dpi));
            y += dip(22, dpi);
            place(self.priority_help, padding, y, usable, dip(40, dpi));
            y += dip(44, dpi);
            place(self.preset_list, padding, y, usable, dip(150, dpi));
            y += dip(150, dpi) + gap;

            let small = dip(82, dpi);
            let capture_width = dip(178, dpi);
            place(self.capture, padding, y, capture_width, button_height);
            place(
                self.rename_edit,
                padding + capture_width + gap,
                y,
                (usable - capture_width - gap - small).max(1),
                button_height,
            );
            place(
                self.rename,
                padding + usable - small,
                y,
                small,
                button_height,
            );
            y += button_height + gap;
            let buttons = [
                self.duplicate,
                self.delete,
                self.move_up,
                self.move_down,
                self.plan,
                self.apply,
                self.refresh,
            ];
            let button_width = ((usable - gap * 6) / 7).max(dip(74, dpi));
            for (index, hwnd) in buttons.into_iter().enumerate() {
                place(
                    hwnd,
                    padding + (button_width + gap) * index as i32,
                    y,
                    button_width,
                    button_height,
                );
            }
            y += button_height + dip(4, dpi);
            place(self.capacity, padding, y, dip(180, dpi), dip(22, dpi));
            place(
                self.apply_status,
                padding + dip(190, dpi),
                y,
                (usable - dip(190, dpi)).max(1),
                dip(22, dpi),
            );
            y += dip(30, dpi);

            for screen in &self.screens {
                place(screen.title, padding, y, usable, dip(24, dpi));
                y += dip(28, dpi);
                if let Some(add) = screen.add_screen {
                    place(add, padding, y, dip(150, dpi), button_height);
                    y += button_height + gap;
                    continue;
                }
                if let Some(remove) = screen.remove_screen {
                    place(
                        remove,
                        padding + usable - dip(120, dpi),
                        y - dip(28, dpi),
                        dip(120, dpi),
                        button_height,
                    );
                }
                let grid_size = dip(42, dpi);
                for grid in &screen.grid {
                    let x = padding + (grid.cell.x as i32 + 1) * grid_size;
                    let grid_y = y + (grid.cell.y as i32 + 1) * grid_size;
                    place(grid.hwnd, x, grid_y, grid_size - 2, grid_size - 2);
                }
                let right_x = padding + grid_size * 3 + gap * 2;
                let right_width = (usable - (right_x - padding)).max(1);
                let mut row_y = y;
                for monitor in &screen.monitors {
                    place(monitor.label, right_x, row_y, right_width / 2, dip(20, dpi));
                    place(
                        monitor.status,
                        right_x + right_width / 2,
                        row_y,
                        right_width / 2,
                        dip(20, dpi),
                    );
                    place(
                        monitor.auto_match,
                        right_x + right_width - dip(72, dpi),
                        row_y,
                        dip(72, dpi),
                        dip(20, dpi),
                    );
                    row_y += dip(22, dpi);
                    place(
                        monitor.replacement,
                        right_x,
                        row_y,
                        (right_width / 3).max(1),
                        button_height,
                    );
                    let action_x = right_x + right_width / 3 + gap;
                    let action_width = dip(54, dpi);
                    place(monitor.up, action_x, row_y, action_width, button_height);
                    place(
                        monitor.down,
                        action_x + action_width + gap,
                        row_y,
                        action_width,
                        button_height,
                    );
                    place(
                        monitor.remove,
                        action_x + (action_width + gap) * 2,
                        row_y,
                        action_width,
                        button_height,
                    );
                    let destination_x = action_x + (action_width + gap) * 3;
                    let destination_width =
                        (right_x + right_width - destination_x - dip(58, dpi) - gap).max(1);
                    place(
                        monitor.destination,
                        destination_x,
                        row_y,
                        destination_width,
                        button_height,
                    );
                    place(
                        monitor.move_button,
                        destination_x + destination_width + gap,
                        row_y,
                        dip(58, dpi),
                        button_height,
                    );
                    row_y += button_height + gap;
                }
                if let (Some(combo), Some(add)) = (screen.add_combo, screen.add_monitor) {
                    place(
                        combo,
                        right_x,
                        row_y,
                        (right_width - dip(82, dpi) - gap).max(1),
                        button_height,
                    );
                    place(
                        add,
                        right_x + right_width - dip(82, dpi),
                        row_y,
                        dip(82, dpi),
                        button_height,
                    );
                    row_y += button_height + gap;
                }
                let grid_bottom = y + grid_size * 3;
                y = grid_bottom.max(row_y) + gap;
            }
            y + scroll + padding
        }

        pub fn handle_notify(
            &mut self,
            lparam: LPARAM,
            display: &DisplaySettings,
        ) -> Result<DisplayPageEvent> {
            let header = lparam.0 as *const NMHDR;
            if header.is_null() || unsafe { (*header).hwndFrom } != self.preset_list {
                return Ok(DisplayPageEvent::default());
            }
            if unsafe { (*header).code } != LVN_ITEMCHANGED {
                return Ok(DisplayPageEvent::default());
            }
            let detail = lparam.0 as *const NMLISTVIEW;
            if detail.is_null() {
                return Ok(DisplayPageEvent::default());
            }
            let next = selected_list_index(self.preset_list);
            if next == self.selected_preset {
                return Ok(DisplayPageEvent::default());
            }
            self.selected_preset = next.filter(|index| *index < display.presets.len());
            if let Some(index) = self.selected_preset {
                set_text(self.rename_edit, &display.presets[index].name);
            }
            self.update_buttons(display);
            self.rebuild_editor(display)?;
            Ok(DisplayPageEvent {
                changed_selection: true,
                ..Default::default()
            })
        }

        pub fn handle_command(
            &mut self,
            wparam: WPARAM,
            lparam: LPARAM,
            display: &DisplaySettings,
        ) -> Result<DisplayPageEvent> {
            let hwnd = HWND(lparam.0 as *mut c_void);
            let notification = ((wparam.0 >> 16) & 0xffff) as u16;
            if hwnd == self.auto_switch && notification == 0 {
                let mut candidate = display.clone();
                candidate.auto_switch_enabled = is_checked(self.auto_switch);
                return Ok(mutation(candidate, DisplayMutationKind::AutoSwitch));
            }
            if hwnd == self.capture && notification == 0 {
                return Ok(command(DisplayCommand::Capture));
            }
            if hwnd == self.refresh && notification == 0 {
                let error = self.refresh_online(true);
                self.rebuild_editor(display)?;
                self.update_buttons(display);
                return Ok(DisplayPageEvent {
                    error,
                    changed_selection: true,
                    ..Default::default()
                });
            }
            let Some(preset_index) = self.selected_preset else {
                return Ok(DisplayPageEvent::default());
            };
            if (hwnd == self.rename && notification == 0)
                || (hwnd == self.rename_edit && notification == EN_KILLFOCUS as u16)
            {
                let name = window_text(self.rename_edit).trim().to_owned();
                if !is_valid_name(&name) {
                    set_text(self.rename_edit, &display.presets[preset_index].name);
                    return Ok(error("프리셋 이름은 공백이 아닌 1~40자여야 합니다"));
                }
                let mut candidate = display.clone();
                candidate.presets[preset_index].name = name;
                return Ok(mutation(
                    candidate,
                    DisplayMutationKind::Rename {
                        preset: preset_index,
                    },
                ));
            }
            if hwnd == self.duplicate && notification == 0 {
                if display.presets.len() >= DisplayPresetDefaults::MAXIMUM_PRESET_COUNT {
                    return Ok(error("프리셋 최대 개수에 도달했습니다"));
                }
                let mut copy = display.presets[preset_index].clone();
                copy.id = new_preset_id();
                let requested = format!("{} 복사본", copy.name);
                copy.name = unique_name(
                    &requested,
                    display.presets.iter().map(|preset| preset.name.as_str()),
                );
                let inserted = preset_index + 1;
                let mut candidate = display.clone();
                candidate.presets.insert(inserted, copy);
                self.selected_preset = Some(inserted);
                return Ok(mutation(
                    candidate,
                    DisplayMutationKind::Duplicate {
                        source: preset_index,
                        inserted,
                    },
                ));
            }
            if hwnd == self.delete && notification == 0 {
                let mut candidate = display.clone();
                candidate.presets.remove(preset_index);
                self.selected_preset = if candidate.presets.is_empty() {
                    None
                } else {
                    Some(preset_index.min(candidate.presets.len() - 1))
                };
                return Ok(mutation(
                    candidate,
                    DisplayMutationKind::Delete {
                        preset: preset_index,
                    },
                ));
            }
            for (button, offset) in [(self.move_up, -1_isize), (self.move_down, 1)] {
                if hwnd == button && notification == 0 {
                    let Some(destination) = preset_index.checked_add_signed(offset) else {
                        return Ok(DisplayPageEvent::default());
                    };
                    if destination >= display.presets.len() {
                        return Ok(DisplayPageEvent::default());
                    }
                    let mut candidate = display.clone();
                    candidate.presets.swap(preset_index, destination);
                    self.selected_preset = Some(destination);
                    return Ok(mutation(
                        candidate,
                        DisplayMutationKind::MovePreset {
                            source: preset_index,
                            destination,
                        },
                    ));
                }
            }
            if hwnd == self.plan && notification == 0 {
                return Ok(command(DisplayCommand::ShowPlan(preset_index)));
            }
            if hwnd == self.apply && notification == 0 {
                return Ok(command(DisplayCommand::Apply(preset_index)));
            }

            for screen_ui in &self.screens {
                let slot = screen_ui.slot;
                if screen_ui.add_screen == Some(hwnd) && notification == 0 {
                    return Ok(self.edit(
                        display,
                        DisplayEdit::AddScreen {
                            preset: preset_index,
                            slot,
                        },
                    ));
                }
                if screen_ui.remove_screen == Some(hwnd) && notification == 0 {
                    return Ok(self.edit(
                        display,
                        DisplayEdit::RemoveScreen {
                            preset: preset_index,
                            slot,
                        },
                    ));
                }
                if let Some(grid) = screen_ui.grid.iter().find(|grid| grid.hwnd == hwnd) {
                    if notification == 0 && grid.state == GridCellState::Available {
                        return Ok(self.edit(
                            display,
                            DisplayEdit::SetGridCell {
                                preset: preset_index,
                                slot,
                                cell: grid.cell,
                            },
                        ));
                    }
                }
                if screen_ui.add_monitor == Some(hwnd) && notification == 0 {
                    let Some(combo) = screen_ui.add_combo else {
                        continue;
                    };
                    let Some(identity) = selected_online(combo, &self.online) else {
                        return Ok(error("추가할 연결 모니터를 선택하세요"));
                    };
                    return Ok(self.edit(
                        display,
                        DisplayEdit::AddMonitor {
                            preset: preset_index,
                            slot,
                            identity,
                        },
                    ));
                }
                for monitor in &screen_ui.monitors {
                    if hwnd == monitor.replacement && notification == CBN_SELCHANGE as u16 {
                        let selected = combo_index(monitor.replacement);
                        if selected > 0 {
                            if let Some(identity) =
                                self.online.get(selected - 1).map(OnlineDisplay::identity)
                            {
                                return Ok(self.edit(
                                    display,
                                    DisplayEdit::ReplaceMonitor {
                                        preset: preset_index,
                                        slot,
                                        monitor: monitor.index,
                                        identity,
                                    },
                                ));
                            }
                        }
                    }
                    if hwnd == monitor.auto_match && notification == 0 {
                        return Ok(self.edit(
                            display,
                            DisplayEdit::SetAutoMatch {
                                preset: preset_index,
                                slot,
                                monitor: monitor.index,
                                enabled: is_checked(monitor.auto_match),
                            },
                        ));
                    }
                    if hwnd == monitor.remove && notification == 0 {
                        return Ok(self.edit(
                            display,
                            DisplayEdit::RemoveMonitor {
                                preset: preset_index,
                                slot,
                                monitor: monitor.index,
                            },
                        ));
                    }
                    for (button, offset) in [(monitor.up, -1_isize), (monitor.down, 1)] {
                        if hwnd == button && notification == 0 {
                            return Ok(self.edit(
                                display,
                                DisplayEdit::MoveMonitorWithin {
                                    preset: preset_index,
                                    slot,
                                    monitor: monitor.index,
                                    offset,
                                },
                            ));
                        }
                    }
                    if hwnd == monitor.move_button && notification == 0 {
                        let choices = destination_slots(display, preset_index, slot);
                        let selected = combo_index(monitor.destination);
                        let Some(destination) = choices.get(selected).copied() else {
                            return Ok(error("이동할 화면 슬롯을 선택하세요"));
                        };
                        return Ok(self.edit(
                            display,
                            DisplayEdit::MoveMonitorBetween {
                                preset: preset_index,
                                source: slot,
                                monitor: monitor.index,
                                destination,
                            },
                        ));
                    }
                }
            }
            Ok(DisplayPageEvent::default())
        }

        fn edit(&self, display: &DisplaySettings, edit: DisplayEdit) -> DisplayPageEvent {
            match apply_display_edit(display, &edit) {
                Ok(candidate) => mutation(candidate, DisplayMutationKind::Edit(edit)),
                Err(reason) => error(&reason),
            }
        }

        pub fn draw_item(&self, lparam: LPARAM) -> bool {
            let item = lparam.0 as *const DRAWITEMSTRUCT;
            if item.is_null() {
                return false;
            }
            let item = unsafe { &*item };
            let Some(grid) = self
                .screens
                .iter()
                .flat_map(|screen| screen.grid.iter())
                .find(|grid| grid.hwnd == item.hwndItem)
            else {
                return false;
            };
            let (background, foreground) = match grid.state {
                GridCellState::Main => (
                    COLORREF(unsafe { GetSysColor(COLOR_HIGHLIGHT) }),
                    COLORREF(unsafe { GetSysColor(COLOR_HIGHLIGHTTEXT) }),
                ),
                GridCellState::Selected => (COLORREF(0x00d77800), COLORREF(0x00ffffff)),
                GridCellState::Occupied => (
                    COLORREF(0x00b0b0b0),
                    COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) }),
                ),
                GridCellState::Disconnected => (
                    COLORREF(unsafe { GetSysColor(COLOR_BTNFACE) }),
                    COLORREF(unsafe { GetSysColor(COLOR_GRAYTEXT) }),
                ),
                GridCellState::Available => (
                    COLORREF(unsafe { GetSysColor(COLOR_WINDOW) }),
                    COLORREF(unsafe { GetSysColor(COLOR_WINDOWTEXT) }),
                ),
            };
            let brush = unsafe { CreateSolidBrush(background) };
            unsafe { FillRect(item.hDC, &item.rcItem, brush) };
            let border = unsafe { CreateSolidBrush(COLORREF(GetSysColor(COLOR_BTNTEXT))) };
            unsafe { FrameRect(item.hDC, &item.rcItem, border) };
            let _ = unsafe { DeleteObject(brush.into()) };
            let _ = unsafe { DeleteObject(border.into()) };
            let mut text = wide(grid.label);
            let text_length = text.len().saturating_sub(1);
            let mut rect = item.rcItem;
            unsafe {
                SetBkMode(item.hDC, TRANSPARENT);
                SetTextColor(item.hDC, foreground);
                DrawTextW(
                    item.hDC,
                    &mut text[..text_length],
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            }
            if item.itemState.0 & 0x0010 != 0 {
                let _ = unsafe { DrawFocusRect(item.hDC, &item.rcItem) };
            }
            true
        }

        pub fn control_count(&self) -> usize {
            let mut count = 0;
            self.for_each_control(|hwnd| {
                if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
                    count += 1;
                }
            });
            count
        }

        pub fn for_each_control(&self, mut body: impl FnMut(HWND)) {
            for hwnd in [
                self.auto_switch,
                self.stability_help,
                self.priority_help,
                self.preset_list,
                self.capture,
                self.rename_edit,
                self.rename,
                self.duplicate,
                self.delete,
                self.move_up,
                self.move_down,
                self.plan,
                self.apply,
                self.refresh,
                self.capacity,
                self.apply_status,
            ] {
                body(hwnd);
            }
            for screen in &self.screens {
                body(screen.title);
                if let Some(hwnd) = screen.add_screen {
                    body(hwnd);
                }
                if let Some(hwnd) = screen.remove_screen {
                    body(hwnd);
                }
                for grid in &screen.grid {
                    body(grid.hwnd);
                }
                if let Some(hwnd) = screen.add_combo {
                    body(hwnd);
                }
                if let Some(hwnd) = screen.add_monitor {
                    body(hwnd);
                }
                for monitor in &screen.monitors {
                    for hwnd in [
                        monitor.label,
                        monitor.status,
                        monitor.auto_match,
                        monitor.replacement,
                        monitor.remove,
                        monitor.up,
                        monitor.down,
                        monitor.destination,
                        monitor.move_button,
                    ] {
                        body(hwnd);
                    }
                }
            }
        }

        fn rebuild_editor(&mut self, display: &DisplaySettings) -> Result<()> {
            self.destroy_editor();
            let Some(index) = self.selected_preset else {
                return Ok(());
            };
            let Some(preset) = display.presets.get(index) else {
                return Ok(());
            };
            for slot in PresetScreenSlot::ALL {
                let title = static_text(self.parent, self.hinstance, slot.display_name())?;
                let Some(screen_value) = screen(preset, slot) else {
                    let add_screen = Some(button(
                        self.parent,
                        self.hinstance,
                        &format!("{} 추가", slot.display_name()),
                    )?);
                    if let Some(add) = add_screen {
                        let enabled =
                            slot != PresetScreenSlot::Secondary2 || preset.secondary1.is_some();
                        let _ = unsafe { EnableWindow(add, enabled) };
                    }
                    self.screens.push(ScreenUi {
                        slot,
                        title,
                        add_screen,
                        remove_screen: None,
                        grid: Vec::new(),
                        add_combo: None,
                        add_monitor: None,
                        monitors: Vec::new(),
                    });
                    continue;
                };
                let remove_screen = (slot != PresetScreenSlot::Main)
                    .then(|| button(self.parent, self.hinstance, "화면 슬롯 제거"))
                    .transpose()?;
                if let Some(remove) = remove_screen {
                    let enabled =
                        slot != PresetScreenSlot::Secondary1 || preset.secondary2.is_none();
                    let _ = unsafe { EnableWindow(remove, enabled) };
                }
                let mut grid = Vec::new();
                for cell in GridCell::all() {
                    let state = grid_cell_state(preset, slot, cell);
                    let hwnd = child(
                        self.parent,
                        self.hinstance,
                        w!("BUTTON"),
                        "",
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
                        WINDOW_EX_STYLE(0),
                    )?;
                    let _ = unsafe { EnableWindow(hwnd, state == GridCellState::Available) };
                    let label = if preset.main.cell == cell {
                        "주"
                    } else if preset
                        .secondary1
                        .as_ref()
                        .is_some_and(|screen| screen.cell == cell)
                    {
                        "1"
                    } else if preset
                        .secondary2
                        .as_ref()
                        .is_some_and(|screen| screen.cell == cell)
                    {
                        "2"
                    } else {
                        "·"
                    };
                    grid.push(GridUi {
                        hwnd,
                        cell,
                        state,
                        label,
                    });
                }
                let add_combo = combo(self.parent, self.hinstance)?;
                add_combo_item(add_combo, "연결 모니터 선택…");
                for online in &self.online {
                    add_combo_item(add_combo, &display_label(online));
                }
                set_combo(add_combo, 0);
                let add_monitor = button(self.parent, self.hinstance, "추가")?;
                let mut monitors = Vec::new();
                for (monitor_index, monitor) in screen_value.monitors.iter().enumerate() {
                    let label = static_text(
                        self.parent,
                        self.hinstance,
                        &format!(
                            "{}{}",
                            if monitor_index == 0 { "기준 · " } else { "" },
                            identity_label(&monitor.identity)
                        ),
                    )?;
                    let matched = matched_online(&monitor.identity, &self.online);
                    let status = static_text(
                        self.parent,
                        self.hinstance,
                        matched
                            .map(|display| format!("연결됨 · {}", display.name))
                            .unwrap_or_else(|| "연결 안 됨".to_owned())
                            .as_str(),
                    )?;
                    let auto_match = child(
                        self.parent,
                        self.hinstance,
                        w!("BUTTON"),
                        "자동",
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                        WINDOW_EX_STYLE(0),
                    )?;
                    set_check(auto_match, monitor.auto_match);
                    let replacement = combo(self.parent, self.hinstance)?;
                    add_combo_item(replacement, "교체할 모니터…");
                    for online in &self.online {
                        add_combo_item(replacement, &display_label(online));
                    }
                    set_combo(replacement, 0);
                    let remove = button(self.parent, self.hinstance, "제거")?;
                    let up = button(self.parent, self.hinstance, "위")?;
                    let down = button(self.parent, self.hinstance, "아래")?;
                    let destination = combo(self.parent, self.hinstance)?;
                    let choices = destination_slots(display, index, slot);
                    for choice in &choices {
                        add_combo_item(destination, choice.display_name());
                    }
                    set_combo(destination, 0);
                    let move_button = button(self.parent, self.hinstance, "이동")?;
                    let _ = unsafe {
                        EnableWindow(
                            remove,
                            !(slot == PresetScreenSlot::Main && screen_value.monitors.len() <= 1),
                        )
                    };
                    let _ = unsafe { EnableWindow(up, monitor_index > 0) };
                    let _ = unsafe {
                        EnableWindow(down, monitor_index + 1 < screen_value.monitors.len())
                    };
                    let _ = unsafe { EnableWindow(destination, !choices.is_empty()) };
                    let _ = unsafe { EnableWindow(move_button, !choices.is_empty()) };
                    monitors.push(MonitorUi {
                        index: monitor_index,
                        label,
                        status,
                        auto_match,
                        replacement,
                        remove,
                        up,
                        down,
                        destination,
                        move_button,
                    });
                }
                self.screens.push(ScreenUi {
                    slot,
                    title,
                    add_screen: None,
                    remove_screen,
                    grid,
                    add_combo: Some(add_combo),
                    add_monitor: Some(add_monitor),
                    monitors,
                });
            }
            if !self.font.is_invalid() {
                let font = self.font;
                self.for_each_control(|hwnd| unsafe {
                    SendMessageW(
                        hwnd,
                        WM_SETFONT,
                        Some(WPARAM(font.0 as usize)),
                        Some(LPARAM(1)),
                    );
                });
            }
            Ok(())
        }

        fn destroy_editor(&mut self) {
            let mut handles = Vec::new();
            for screen in &self.screens {
                handles.push(screen.title);
                handles.extend(screen.add_screen);
                handles.extend(screen.remove_screen);
                handles.extend(screen.grid.iter().map(|grid| grid.hwnd));
                handles.extend(screen.add_combo);
                handles.extend(screen.add_monitor);
                for monitor in &screen.monitors {
                    handles.extend([
                        monitor.label,
                        monitor.status,
                        monitor.auto_match,
                        monitor.replacement,
                        monitor.remove,
                        monitor.up,
                        monitor.down,
                        monitor.destination,
                        monitor.move_button,
                    ]);
                }
            }
            self.screens.clear();
            for hwnd in handles {
                let _ = unsafe { DestroyWindow(hwnd) };
            }
        }

        fn update_buttons(&self, display: &DisplaySettings) {
            let selected = self.selected_preset;
            let has_selected = selected.is_some();
            let room = display.presets.len() < DisplayPresetDefaults::MAXIMUM_PRESET_COUNT;
            let _ = unsafe { EnableWindow(self.capture, room) };
            let _ = unsafe { EnableWindow(self.rename_edit, has_selected) };
            let _ = unsafe { EnableWindow(self.rename, has_selected) };
            let _ = unsafe { EnableWindow(self.duplicate, has_selected && room) };
            let _ = unsafe { EnableWindow(self.delete, has_selected) };
            let _ = unsafe { EnableWindow(self.move_up, selected.is_some_and(|index| index > 0)) };
            let _ = unsafe {
                EnableWindow(
                    self.move_down,
                    selected.is_some_and(|index| index + 1 < display.presets.len()),
                )
            };
            let _ = unsafe { EnableWindow(self.plan, has_selected) };

            let plan_reason = selected.and_then(|index| {
                let plan = DisplayPlanner::plan(&display.presets[index], &self.online);
                (!plan.is_applicable()).then(|| plan.summary())
            });
            let reason = self
                .apply_availability
                .reason
                .clone()
                .or_else(|| self.online_error.clone())
                .or(plan_reason);
            let enabled = has_selected && self.apply_availability.enabled && reason.is_none();
            let _ = unsafe { EnableWindow(self.apply, enabled) };
            set_text(
                self.apply_status,
                reason.as_deref().unwrap_or(if has_selected {
                    "지금 적용 가능"
                } else {
                    "프리셋을 선택하세요"
                }),
            );
        }
    }

    impl Drop for DisplayPage {
        fn drop(&mut self) {
            self.destroy_editor();
        }
    }

    fn destination_slots(
        display: &DisplaySettings,
        preset: usize,
        source: PresetScreenSlot,
    ) -> Vec<PresetScreenSlot> {
        let Some(preset) = display.presets.get(preset) else {
            return Vec::new();
        };
        PresetScreenSlot::ALL
            .into_iter()
            .filter(|slot| *slot != source && screen(preset, *slot).is_some())
            .collect()
    }

    fn matched_online<'a>(
        identity: &DisplayIdentity,
        online: &'a [OnlineDisplay],
    ) -> Option<&'a OnlineDisplay> {
        let monitor = PresetMonitor {
            identity: identity.clone(),
            auto_match: false,
        };
        DisplayMatcher::matches(&[monitor], online)
            .first()
            .and_then(|matched| online.get(matched.online_index))
    }

    fn identity_label(identity: &DisplayIdentity) -> String {
        let source = if identity.uuid.is_empty() {
            format!(
                "{}:{}:{}:{}",
                identity.vendor, identity.model, identity.serial, identity.name
            )
        } else {
            identity.uuid.clone()
        };
        format!(
            "{} · 모델 {} · #{}",
            identity.name,
            identity.model,
            hash_prefix8(&source)
        )
    }

    fn display_label(display: &OnlineDisplay) -> String {
        identity_label(&display.identity())
    }

    fn selected_online(combo: HWND, online: &[OnlineDisplay]) -> Option<DisplayIdentity> {
        let selected = combo_index(combo);
        selected
            .checked_sub(1)
            .and_then(|index| online.get(index))
            .map(OnlineDisplay::identity)
    }

    fn mutation(display: DisplaySettings, kind: DisplayMutationKind) -> DisplayPageEvent {
        DisplayPageEvent {
            mutation: Some(DisplayMutationRequest { display, kind }),
            ..Default::default()
        }
    }

    fn command(command: DisplayCommand) -> DisplayPageEvent {
        DisplayPageEvent {
            command: Some(command),
            ..Default::default()
        }
    }

    fn error(reason: &str) -> DisplayPageEvent {
        DisplayPageEvent {
            error: Some(reason.to_owned()),
            ..Default::default()
        }
    }

    fn next_id() -> i32 {
        NEXT_CONTROL_ID.fetch_add(1, Ordering::Relaxed)
    }

    fn child(
        parent: HWND,
        hinstance: HINSTANCE,
        class: PCWSTR,
        text: &str,
        style: WINDOW_STYLE,
        ex_style: WINDOW_EX_STYLE,
    ) -> Result<HWND> {
        let text = wide(text);
        child_class(
            parent,
            hinstance,
            class,
            PCWSTR(text.as_ptr()),
            style,
            ex_style,
        )
    }

    fn child_class(
        parent: HWND,
        hinstance: HINSTANCE,
        class: PCWSTR,
        text: PCWSTR,
        style: WINDOW_STYLE,
        ex_style: WINDOW_EX_STYLE,
    ) -> Result<HWND> {
        unsafe {
            CreateWindowExW(
                ex_style,
                class,
                text,
                style,
                0,
                0,
                0,
                0,
                Some(parent),
                Some(HMENU(next_id() as *mut c_void)),
                Some(hinstance),
                None,
            )
        }
    }

    fn button(parent: HWND, hinstance: HINSTANCE, text: &str) -> Result<HWND> {
        child(
            parent,
            hinstance,
            w!("BUTTON"),
            text,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            WINDOW_EX_STYLE(0),
        )
    }

    fn static_text(parent: HWND, hinstance: HINSTANCE, text: &str) -> Result<HWND> {
        child(
            parent,
            hinstance,
            w!("STATIC"),
            text,
            WS_CHILD | WS_VISIBLE,
            WINDOW_EX_STYLE(0),
        )
    }

    fn combo(parent: HWND, hinstance: HINSTANCE) -> Result<HWND> {
        child(
            parent,
            hinstance,
            w!("COMBOBOX"),
            "",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            WINDOW_EX_STYLE(0),
        )
    }

    fn place(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                None,
                x,
                y,
                width.max(1),
                height.max(1),
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }

    fn insert_column(list: HWND, index: usize, title: &str, width: i32) {
        let mut title = wide(title);
        let column = LVCOLUMNW {
            mask: LVCF_TEXT | LVCF_WIDTH,
            cx: width,
            pszText: PWSTR(title.as_mut_ptr()),
            ..Default::default()
        };
        unsafe {
            SendMessageW(
                list,
                LVM_INSERTCOLUMNW,
                Some(WPARAM(index)),
                Some(LPARAM((&column as *const LVCOLUMNW) as isize)),
            );
        }
    }

    fn insert_list_item(list: HWND, row: usize, column: i32, text: &str) {
        let mut text = wide(text);
        let item = LVITEMW {
            mask: LVIF_TEXT,
            iItem: row as i32,
            iSubItem: column,
            pszText: PWSTR(text.as_mut_ptr()),
            ..Default::default()
        };
        unsafe {
            SendMessageW(
                list,
                LVM_INSERTITEMW,
                None,
                Some(LPARAM((&item as *const LVITEMW) as isize)),
            );
        }
    }

    fn set_list_item(list: HWND, row: usize, column: i32, text: &str) {
        let mut text = wide(text);
        let item = LVITEMW {
            iItem: row as i32,
            iSubItem: column,
            pszText: PWSTR(text.as_mut_ptr()),
            ..Default::default()
        };
        unsafe {
            SendMessageW(
                list,
                LVM_SETITEMTEXTW,
                Some(WPARAM(row)),
                Some(LPARAM((&item as *const LVITEMW) as isize)),
            );
        }
    }

    fn select_list_item(list: HWND, row: usize) {
        let item = LVITEMW {
            mask: LVIF_STATE,
            state: windows::Win32::UI::Controls::LIST_VIEW_ITEM_STATE_FLAGS(
                LVIS_SELECTED.0 | LVIS_FOCUSED.0,
            ),
            stateMask: windows::Win32::UI::Controls::LIST_VIEW_ITEM_STATE_FLAGS(
                LVIS_SELECTED.0 | LVIS_FOCUSED.0,
            ),
            ..Default::default()
        };
        unsafe {
            SendMessageW(
                list,
                LVM_SETITEMSTATE,
                Some(WPARAM(row)),
                Some(LPARAM((&item as *const LVITEMW) as isize)),
            );
        }
    }

    fn selected_list_index(list: HWND) -> Option<usize> {
        let value = unsafe {
            SendMessageW(
                list,
                LVM_GETNEXTITEM,
                Some(WPARAM(usize::MAX)),
                Some(LPARAM(LVNI_SELECTED as isize)),
            )
        }
        .0;
        usize::try_from(value).ok()
    }

    fn add_combo_item(combo: HWND, value: &str) {
        let value = wide(value);
        unsafe {
            SendMessageW(
                combo,
                CB_ADDSTRING,
                None,
                Some(LPARAM(value.as_ptr() as isize)),
            );
        }
    }

    fn set_combo(combo: HWND, index: usize) {
        unsafe {
            SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(index)), None);
        }
    }

    fn combo_index(combo: HWND) -> usize {
        usize::try_from(unsafe { SendMessageW(combo, CB_GETCURSEL, None, None) }.0).unwrap_or(0)
    }

    fn set_text(hwnd: HWND, text: &str) {
        let text = wide(text);
        let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(text.as_ptr())) };
    }

    fn window_text(hwnd: HWND) -> String {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0_u16; length as usize + 1];
        let read = unsafe { GetWindowTextW(hwnd, &mut buffer) };
        String::from_utf16_lossy(&buffer[..read.max(0) as usize])
    }

    fn set_check(hwnd: HWND, checked: bool) {
        unsafe {
            SendMessageW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::BM_SETCHECK,
                Some(WPARAM(if checked { 1 } else { 0 })),
                None,
            );
        }
    }

    fn is_checked(hwnd: HWND) -> bool {
        unsafe {
            SendMessageW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::BM_GETCHECK,
                None,
                None,
            )
        }
        .0 == 1
    }

    pub use self::DisplayPageEvent as PlatformDisplayPageEvent;
    pub use DisplayPage as PlatformDisplayPage;
}

#[cfg(windows)]
pub use windows_ui::{
    PlatformDisplayPage as DisplayPage, PlatformDisplayPageEvent as DisplayPageEvent,
};

#[cfg(test)]
mod tests {
    use super::*;
    use edutools_core::display_preset::{DisplayKind, DisplayPresetDefaults};

    fn identity(name: &str, serial: u32) -> DisplayIdentity {
        DisplayIdentity {
            uuid: format!("DISPLAY-{serial}"),
            vendor: 1,
            model: serial,
            serial,
            name: name.to_owned(),
            kind: DisplayKind::External,
        }
    }

    fn preset() -> DisplayPreset {
        DisplayPreset {
            id: PresetId::parse("00000000-0000-4000-A000-000000000001").unwrap(),
            name: "교실".to_owned(),
            main: PresetScreen {
                monitors: vec![PresetMonitor {
                    identity: identity("주", 1),
                    auto_match: false,
                }],
                cell: GridCell::center(),
            },
            secondary1: Some(PresetScreen {
                monitors: vec![PresetMonitor {
                    identity: identity("보조", 2),
                    auto_match: false,
                }],
                cell: GridCell { x: 1, y: 0 },
            }),
            secondary2: None,
        }
    }

    #[test]
    fn grid_cell_state_table_covers_occupied_selected_main_and_r1_adjacency() {
        let value = preset();
        let cases = [
            (
                PresetScreenSlot::Main,
                GridCell::center(),
                GridCellState::Main,
            ),
            (
                PresetScreenSlot::Secondary1,
                GridCell { x: 1, y: 0 },
                GridCellState::Selected,
            ),
            (
                PresetScreenSlot::Secondary1,
                GridCell::center(),
                GridCellState::Occupied,
            ),
            (
                PresetScreenSlot::Secondary1,
                GridCell { x: -1, y: 0 },
                GridCellState::Available,
            ),
            (
                PresetScreenSlot::Secondary1,
                GridCell { x: 1, y: 1 },
                GridCellState::Disconnected,
            ),
        ];
        for (slot, cell, expected) in cases {
            assert_eq!(
                grid_cell_state(&value, slot, cell),
                expected,
                "{slot:?} {cell:?}"
            );
        }
    }

    #[test]
    fn name_rule_table_covers_duplicate_length_and_whitespace() {
        let long = "가".repeat(DisplayPresetDefaults::MAXIMUM_NAME_LENGTH + 3);
        let cases = [
            ("교실", vec!["교실"], "교실 2".to_owned(), true),
            (
                long.as_str(),
                Vec::new(),
                "가".repeat(DisplayPresetDefaults::MAXIMUM_NAME_LENGTH),
                false,
            ),
            ("   ", Vec::new(), "   ".to_owned(), false),
        ];
        for (requested, existing, expected_unique, expected_valid) in cases {
            assert_eq!(
                unique_name(requested, existing.iter().copied()),
                expected_unique
            );
            assert_eq!(is_valid_name(requested), expected_valid);
            assert!(
                edutools_core::display_preset::limited_name(requested)
                    .chars()
                    .count()
                    <= DisplayPresetDefaults::MAXIMUM_NAME_LENGTH
            );
        }
    }

    #[test]
    fn monitor_edit_table_crosses_validator_accept_and_reject_boundaries() {
        let display = DisplaySettings {
            auto_switch_enabled: false,
            presets: vec![preset()],
        };
        let cases = [
            (
                "move",
                DisplayEdit::MoveMonitorBetween {
                    preset: 0,
                    source: PresetScreenSlot::Secondary1,
                    monitor: 0,
                    destination: PresetScreenSlot::Main,
                },
                true,
            ),
            (
                "replace duplicate",
                DisplayEdit::ReplaceMonitor {
                    preset: 0,
                    slot: PresetScreenSlot::Secondary1,
                    monitor: 0,
                    identity: identity("주 복제", 1),
                },
                false,
            ),
            (
                "remove last main",
                DisplayEdit::RemoveMonitor {
                    preset: 0,
                    slot: PresetScreenSlot::Main,
                    monitor: 0,
                },
                false,
            ),
        ];
        for (name, edit, accepted) in cases {
            let result = apply_display_edit(&display, &edit);
            assert_eq!(result.is_ok(), accepted, "{name}: {result:?}");
            if let Ok(candidate) = result {
                DisplayPresetValidator::validate(&candidate).unwrap();
            }
        }
    }
}
