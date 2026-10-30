use edutools_core::display_preset::{DisplayKind, DisplayPoint, DisplaySize, OnlineDisplay};
use std::collections::{HashMap, HashSet};

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

#[cfg_attr(not(test), allow(dead_code))]
pub fn monitor_index_at_point(rects: &[MonitorRect], x: i32, y: i32) -> Option<usize> {
    rects
        .iter()
        .position(|rect| x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom)
}

pub const fn local_point_in_monitor(rect: MonitorRect, x: i32, y: i32) -> (i32, i32) {
    (x - rect.left, y - rect.top)
}

const OUTPUT_TECHNOLOGY_INTERNAL: i32 = i32::MIN;
const OUTPUT_TECHNOLOGY_LVDS: i32 = 6;
const OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED: i32 = 11;
const OUTPUT_TECHNOLOGY_UDI_EMBEDDED: i32 = 13;
const OUTPUT_TECHNOLOGY_MIRACAST: i32 = 15;
const OUTPUT_TECHNOLOGY_INDIRECT_WIRED: i32 = 16;
const OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL: i32 = 17;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DisplaySourceKey {
    pub adapter_low: u32,
    pub adapter_high: i32,
    pub source_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DisplayTargetKey {
    pub adapter_low: u32,
    pub adapter_high: i32,
    pub target_id: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayPathRecord {
    pub source: DisplaySourceKey,
    pub target: DisplayTargetKey,
    pub target_available: bool,
    pub output_technology: i32,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub device_path: String,
    pub vendor: u32,
    pub model: u32,
    pub serial: u32,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IdentityStage {
    DevicePath,
    EdidSerial,
    VirtualName,
    Unmatchable,
}

impl IdentityStage {
    #[cfg_attr(not(windows), allow(dead_code))]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DevicePath => "devicePath",
            Self::EdidSerial => "edidSerial",
            Self::VirtualName => "virtualName",
            Self::Unmatchable => "unmatchable",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConvertedOnlineDisplay {
    pub display: OnlineDisplay,
    pub target: DisplayTargetKey,
    pub identity_stage: IdentityStage,
    pub output_technology: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OnlineDisplayConversion {
    pub displays: Vec<ConvertedOnlineDisplay>,
    pub target_table: Vec<(u32, DisplayTargetKey)>,
    pub unavailable_target_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionSetOutcome {
    Stale,
    Unchanged,
    Changed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayChangeState {
    generation: u64,
    pending_generation: Option<u64>,
    last_connection_uuids: Option<HashSet<String>>,
}

impl DisplayChangeState {
    pub fn initialize(&mut self, uuids: HashSet<String>) {
        self.last_connection_uuids = Some(uuids);
    }

    pub fn notify(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending_generation = Some(self.generation);
        self.generation
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub const fn pending_generation(&self) -> Option<u64> {
        self.pending_generation
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn cancel(&mut self, generation: u64) {
        if self.pending_generation == Some(generation) {
            self.pending_generation = None;
        }
    }

    pub fn settle(
        &mut self,
        generation: u64,
        current_uuids: &HashSet<String>,
    ) -> ConnectionSetOutcome {
        if self.pending_generation != Some(generation) {
            return ConnectionSetOutcome::Stale;
        }
        self.pending_generation = None;
        if self.last_connection_uuids.as_ref() == Some(current_uuids) {
            ConnectionSetOutcome::Unchanged
        } else {
            self.last_connection_uuids = Some(current_uuids.clone());
            ConnectionSetOutcome::Changed
        }
    }
}

pub const fn display_kind_for_output_technology(output_technology: i32) -> DisplayKind {
    match output_technology {
        OUTPUT_TECHNOLOGY_INTERNAL
        | OUTPUT_TECHNOLOGY_LVDS
        | OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED
        | OUTPUT_TECHNOLOGY_UDI_EMBEDDED => DisplayKind::BuiltIn,
        OUTPUT_TECHNOLOGY_MIRACAST
        | OUTPUT_TECHNOLOGY_INDIRECT_WIRED
        | OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL => DisplayKind::Virtual,
        _ => DisplayKind::External,
    }
}

pub fn identity_stage(path: &DisplayPathRecord) -> IdentityStage {
    if !path.device_path.is_empty() {
        IdentityStage::DevicePath
    } else if path.serial != 0 {
        IdentityStage::EdidSerial
    } else if display_kind_for_output_technology(path.output_technology) == DisplayKind::Virtual {
        IdentityStage::VirtualName
    } else {
        IdentityStage::Unmatchable
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn is_primary_display(display: &OnlineDisplay) -> bool {
    display.origin == DisplayPoint::zero()
}

pub fn online_displays_from_paths(
    paths: &[DisplayPathRecord],
) -> Result<OnlineDisplayConversion, String> {
    let mut conversion = OnlineDisplayConversion::default();
    let mut mirror_masters = HashMap::<DisplaySourceKey, u32>::new();

    for path in paths {
        if !path.target_available {
            conversion.unavailable_target_count += 1;
            continue;
        }
        if path.width == 0 || path.height == 0 {
            return Err(format!(
                "CCD source mode has an empty size: target={}",
                path.target.target_id
            ));
        }
        let id = u32::try_from(conversion.displays.len() + 1)
            .map_err(|_| "too many active display targets".to_owned())?;
        let current_mirror_master_id = mirror_masters.get(&path.source).copied();
        mirror_masters.entry(path.source).or_insert(id);
        let uuid = path.device_path.to_ascii_lowercase();
        conversion.target_table.push((id, path.target));
        conversion.displays.push(ConvertedOnlineDisplay {
            display: OnlineDisplay {
                id,
                uuid,
                vendor: path.vendor,
                model: path.model,
                serial: path.serial,
                name: path.name.clone(),
                kind: display_kind_for_output_technology(path.output_technology),
                size: DisplaySize {
                    width: f64::from(path.width),
                    height: f64::from(path.height),
                },
                origin: DisplayPoint {
                    x: f64::from(path.x),
                    y: f64::from(path.y),
                },
                current_mirror_master_id,
            },
            target: path.target,
            identity_stage: identity_stage(path),
            output_technology: path.output_technology,
        });
    }
    Ok(conversion)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn hash_prefix8(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", (hash >> 32) as u32)
}

pub fn edid_identity(edid: &[u8]) -> Option<(u32, u32, u32)> {
    (edid.len() >= 16).then(|| {
        let vendor = u32::from(u16::from_be_bytes([edid[8], edid[9]]));
        let model = u32::from(u16::from_le_bytes([edid[10], edid[11]]));
        let serial = u32::from_le_bytes([edid[12], edid[13], edid[14], edid[15]]);
        (vendor, model, serial)
    })
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn online_display_selftest_failures() -> Vec<String> {
    let source = DisplaySourceKey {
        adapter_low: 1,
        adapter_high: 0,
        source_id: 7,
    };
    let path = DisplayPathRecord {
        source,
        target: DisplayTargetKey {
            adapter_low: 1,
            adapter_high: 0,
            target_id: 9,
        },
        target_available: true,
        output_technology: 5,
        width: 1_920,
        height: 1_080,
        x: 0,
        y: 0,
        device_path: r"\\?\DISPLAY#SELFTEST".to_owned(),
        vendor: 0x1234,
        model: 0x5678,
        serial: 42,
        name: "selftest".to_owned(),
    };
    match online_displays_from_paths(&[path]) {
        Ok(result)
            if result.displays.len() == 1
                && result.displays[0].display.id == 1
                && result.displays[0].display.size.width == 1_920.0
                && result.displays[0].display.origin == DisplayPoint::zero()
                && result.displays[0]
                    .display
                    .current_mirror_master_id
                    .is_none()
                && result.target_table.len() == 1 =>
        {
            Vec::new()
        }
        Ok(result) => vec![format!(
            "OnlineDisplay 합성 경로 왕복 결과 불일치: {result:?}"
        )],
        Err(error) => vec![format!("OnlineDisplay 합성 경로 왕복 실패: {error}")],
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        edid_identity, hash_prefix8, online_displays_from_paths, DisplayPathRecord,
        DisplaySourceKey, DisplayTargetKey, IdentityStage, MonitorRect, MonitorSnapshot,
    };
    use edutools_core::display_preset::{DisplayKind, OnlineDisplay};
    use std::collections::{HashMap, HashSet};
    use std::mem::size_of;
    use std::ptr::addr_of;
    use windows::core::{w, Error, Result, BOOL, PCWSTR};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW, SetupDiOpenDevRegKey, DICS_FLAG_GLOBAL,
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, DIREG_DEV, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
        SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
    };
    use windows::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO,
        DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_MODE,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, GUID_DEVINTERFACE_MONITOR, QDC_ONLY_ACTIVE_PATHS,
        QDC_VIRTUAL_MODE_AWARE,
    };
    use windows::Win32::Foundation::{
        ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, E_FAIL, LPARAM, LUID, POINT,
        RECT, WIN32_ERROR,
    };
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFOEXW,
        MONITOR_DEFAULTTONULL,
    };
    use windows::Win32::System::Registry::{
        RegCloseKey, RegQueryValueExW, HKEY, KEY_READ, REG_BINARY, REG_VALUE_TYPE,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    const DISPLAY_CONFIG_RETRY_LIMIT: usize = 8;

    #[derive(Clone, Debug)]
    pub struct DisplayConfiguration {
        pub online: Vec<OnlineDisplay>,
        pub target_table: Vec<(u32, DisplayTargetKey)>,
        pub connected_uuids: HashSet<String>,
        pub active_count: usize,
        pub inactive_count: usize,
        pub mirror_group_count: usize,
        pub unavailable_target_count: usize,
        diagnostics: Vec<DisplayDiagnostic>,
    }

    #[derive(Clone, Debug)]
    struct DisplayDiagnostic {
        id: u32,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
        mirror_master: Option<u32>,
        identity_stage: IdentityStage,
        identity_hash: String,
        output_technology: i32,
    }

    impl DisplayConfiguration {
        pub fn diagnostic_line(&self) -> String {
            let mut stages = [0_usize; 4];
            for diagnostic in &self.diagnostics {
                stages[match diagnostic.identity_stage {
                    IdentityStage::DevicePath => 0,
                    IdentityStage::EdidSerial => 1,
                    IdentityStage::VirtualName => 2,
                    IdentityStage::Unmatchable => 3,
                }] += 1;
            }
            let monitors = self
                .diagnostics
                .iter()
                .map(|display| {
                    format!(
                        "id={},size={}x{},origin={},{} primary={} mirrorMaster={} identityStage={} identityHash={} outputTechnology={}",
                        display.id,
                        display.width,
                        display.height,
                        display.x,
                        display.y,
                        display.x == 0 && display.y == 0,
                        display
                            .mirror_master
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "null".to_owned()),
                        display.identity_stage.as_str(),
                        display.identity_hash,
                        display.output_technology,
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            format!(
                "display-query total={} active={} inactive={} mirrorGroups={} unavailableTargets={} identityStages=devicePath:{},edidSerial:{},virtualName:{},unmatchable:{} monitors=[{}]",
                self.connected_uuids.len(),
                self.active_count,
                self.inactive_count,
                self.mirror_group_count,
                self.unavailable_target_count,
                stages[0],
                stages[1],
                stages[2],
                stages[3],
                monitors,
            )
        }
    }

    #[derive(Debug)]
    struct PresentMonitor {
        device_path: String,
        edid: Option<Vec<u8>>,
    }

    struct DeviceInfoSet(HDEVINFO);

    impl Drop for DeviceInfoSet {
        fn drop(&mut self) {
            // SAFETY: this guard owns the SetupAPI device information set.
            let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
        }
    }

    struct RegistryKey(HKEY);

    impl Drop for RegistryKey {
        fn drop(&mut self) {
            // SAFETY: this guard owns the registry key returned by SetupDiOpenDevRegKey.
            let _ = unsafe { RegCloseKey(self.0) };
        }
    }

    pub fn current_display_configuration() -> Result<DisplayConfiguration> {
        let present = enumerate_present_monitors()?;
        let present_by_path = present
            .iter()
            .map(|monitor| (monitor.device_path.as_str(), monitor))
            .collect::<HashMap<_, _>>();
        let (paths, modes) = query_active_display_config()?;
        let mut records = Vec::with_capacity(paths.len());
        for path in &paths {
            let target_available = path.targetInfo.targetAvailable.as_bool();
            if !target_available {
                records.push(DisplayPathRecord {
                    source: source_key(path.sourceInfo.adapterId, path.sourceInfo.id),
                    target: target_key(path.targetInfo.adapterId, path.targetInfo.id),
                    target_available: false,
                    output_technology: path.targetInfo.outputTechnology.0,
                    width: 0,
                    height: 0,
                    x: 0,
                    y: 0,
                    device_path: String::new(),
                    vendor: 0,
                    model: 0,
                    serial: 0,
                    name: String::new(),
                });
                continue;
            }
            let source_mode = source_mode(path, &modes)?;
            let target_name = target_device_name(path)?;
            let device_path = wide_string(&target_name.monitorDevicePath).to_ascii_lowercase();
            let friendly_name = wide_string(&target_name.monitorFriendlyDeviceName);
            let parsed_edid = present_by_path
                .get(device_path.as_str())
                .and_then(|monitor| monitor.edid.as_deref())
                .and_then(edid_identity);
            let edid_ids_valid = unsafe { target_name.flags.Anonymous.value } & 0x4 != 0;
            let (vendor, model, serial) = parsed_edid.unwrap_or_else(|| {
                if edid_ids_valid {
                    (
                        u32::from(target_name.edidManufactureId.swap_bytes()),
                        u32::from(target_name.edidProductCodeId),
                        0,
                    )
                } else {
                    (0, 0, 0)
                }
            });
            let output_technology = path.targetInfo.outputTechnology.0;
            let kind = super::display_kind_for_output_technology(output_technology);
            let name = if friendly_name.trim().is_empty() {
                fallback_display_name(kind, model)
            } else {
                friendly_name
            };
            // CCD source position/size and the EnumDisplayMonitors rectangles below are both
            // PMv2 physical-pixel virtual-screen coordinates. The overlay keeps its GDI source
            // key, while OnlineDisplay deliberately keeps the per-target opaque table instead.
            records.push(DisplayPathRecord {
                source: source_key(path.sourceInfo.adapterId, path.sourceInfo.id),
                target: target_key(path.targetInfo.adapterId, path.targetInfo.id),
                target_available,
                output_technology,
                width: source_mode.width,
                height: source_mode.height,
                x: source_mode.position.x,
                y: source_mode.position.y,
                device_path,
                vendor,
                model,
                serial,
                name,
            });
        }

        let mut source_counts = HashMap::<DisplaySourceKey, usize>::new();
        for record in records.iter().filter(|record| record.target_available) {
            *source_counts.entry(record.source).or_default() += 1;
        }
        let mirror_group_count = source_counts.values().filter(|count| **count > 1).count();
        let conversion =
            online_displays_from_paths(&records).map_err(|detail| Error::new(E_FAIL, detail))?;
        let active_uuids = conversion
            .displays
            .iter()
            .filter_map(|record| {
                (!record.display.uuid.is_empty()).then(|| record.display.uuid.clone())
            })
            .collect::<HashSet<_>>();
        let mut connected_uuids = present
            .iter()
            .filter_map(|monitor| {
                (!monitor.device_path.is_empty()).then(|| monitor.device_path.clone())
            })
            .collect::<HashSet<_>>();
        connected_uuids.extend(active_uuids.iter().cloned());
        let inactive_count = connected_uuids.difference(&active_uuids).count();
        let diagnostics = conversion
            .displays
            .iter()
            .map(|record| {
                let display = &record.display;
                let identity_source = if display.uuid.is_empty() {
                    format!(
                        "{}:{}:{}:{}:{:?}",
                        display.vendor, display.model, display.serial, display.name, display.kind
                    )
                } else {
                    display.uuid.clone()
                };
                DisplayDiagnostic {
                    id: display.id,
                    width: display.size.width as u32,
                    height: display.size.height as u32,
                    x: display.origin.x as i32,
                    y: display.origin.y as i32,
                    mirror_master: display.current_mirror_master_id,
                    identity_stage: record.identity_stage,
                    identity_hash: hash_prefix8(&identity_source),
                    output_technology: record.output_technology,
                }
            })
            .collect::<Vec<_>>();
        let online = conversion
            .displays
            .into_iter()
            .map(|record| record.display)
            .collect::<Vec<_>>();
        Ok(DisplayConfiguration {
            active_count: online.len(),
            online,
            target_table: conversion.target_table,
            connected_uuids,
            inactive_count,
            mirror_group_count,
            unavailable_target_count: conversion.unavailable_target_count,
            diagnostics,
        })
    }

    fn query_active_display_config(
    ) -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>)> {
        let flags = QDC_ONLY_ACTIVE_PATHS | QDC_VIRTUAL_MODE_AWARE;
        for _ in 0..DISPLAY_CONFIG_RETRY_LIMIT {
            let mut path_count = 0;
            let mut mode_count = 0;
            // SAFETY: both count pointers are valid writable u32 values.
            let status =
                unsafe { GetDisplayConfigBufferSizes(flags, &mut path_count, &mut mode_count) };
            if status != ERROR_SUCCESS {
                return Err(win32_error("GetDisplayConfigBufferSizes", status));
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
            // SAFETY: vectors are sized from GetDisplayConfigBufferSizes and counts are writable.
            let status = unsafe {
                QueryDisplayConfig(
                    flags,
                    &mut path_count,
                    paths.as_mut_ptr(),
                    &mut mode_count,
                    modes.as_mut_ptr(),
                    None,
                )
            };
            if status == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if status != ERROR_SUCCESS {
                return Err(win32_error("QueryDisplayConfig", status));
            }
            paths.truncate(path_count as usize);
            modes.truncate(mode_count as usize);
            return Ok((paths, modes));
        }
        Err(Error::new(
            E_FAIL,
            "QueryDisplayConfig buffer changed too many times",
        ))
    }

    fn source_mode(
        path: &DISPLAYCONFIG_PATH_INFO,
        modes: &[DISPLAYCONFIG_MODE_INFO],
    ) -> Result<DISPLAYCONFIG_SOURCE_MODE> {
        let packed_index = unsafe { path.sourceInfo.Anonymous.modeInfoIdx };
        let virtual_aware_index = (packed_index >> 16) & 0xffff;
        if virtual_aware_index == 0xffff {
            return Err(Error::new(E_FAIL, "active CCD path has no source mode"));
        }
        let mode = modes
            .get(virtual_aware_index as usize)
            .ok_or_else(|| Error::new(E_FAIL, "active CCD source mode index is out of bounds"))?;
        if mode.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE
            || mode.id != path.sourceInfo.id
            || mode.adapterId != path.sourceInfo.adapterId
        {
            return Err(Error::new(
                E_FAIL,
                "active CCD source mode does not match its path",
            ));
        }
        Ok(unsafe { mode.Anonymous.sourceMode })
    }

    fn target_device_name(
        path: &DISPLAYCONFIG_PATH_INFO,
    ) -> Result<DISPLAYCONFIG_TARGET_DEVICE_NAME> {
        let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        name.header.size = u32::try_from(size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>())
            .expect("DISPLAYCONFIG_TARGET_DEVICE_NAME size fits u32");
        name.header.adapterId = path.targetInfo.adapterId;
        name.header.id = path.targetInfo.id;
        // SAFETY: header is the first field and size identifies the full writable packet.
        let status = unsafe { DisplayConfigGetDeviceInfo(&mut name.header) };
        if status != 0 {
            return Err(win32_error(
                "DisplayConfigGetDeviceInfo(GET_TARGET_NAME)",
                WIN32_ERROR(status as u32),
            ));
        }
        Ok(name)
    }

    fn enumerate_present_monitors() -> Result<Vec<PresentMonitor>> {
        // SAFETY: the class GUID is static and no enumerator or parent window is supplied.
        let set = unsafe {
            SetupDiGetClassDevsW(
                Some(&GUID_DEVINTERFACE_MONITOR),
                PCWSTR::null(),
                None,
                DIGCF_DEVICEINTERFACE | DIGCF_PRESENT,
            )
        }?;
        let set = DeviceInfoSet(set);
        let mut monitors = Vec::new();
        let mut index = 0;
        loop {
            let mut interface_data = SP_DEVICE_INTERFACE_DATA {
                cbSize: u32::try_from(size_of::<SP_DEVICE_INTERFACE_DATA>())
                    .expect("SP_DEVICE_INTERFACE_DATA size fits u32"),
                ..Default::default()
            };
            // SAFETY: structures are initialized and the device set remains alive in this scope.
            match unsafe {
                SetupDiEnumDeviceInterfaces(
                    set.0,
                    None,
                    &GUID_DEVINTERFACE_MONITOR,
                    index,
                    &mut interface_data,
                )
            } {
                Ok(()) => {}
                Err(error) if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
                Err(error) => return Err(error),
            }
            index += 1;
            let mut required_size = 0;
            // SAFETY: this size-only call intentionally supplies no detail buffer.
            let _ = unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface_data,
                    None,
                    0,
                    Some(&mut required_size),
                    None,
                )
            };
            if required_size < size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 {
                return Err(Error::new(
                    E_FAIL,
                    "SetupDiGetDeviceInterfaceDetailW returned an invalid size",
                ));
            }
            let word_size = size_of::<usize>();
            let mut buffer = vec![0_usize; (required_size as usize).div_ceil(word_size)];
            let buffer_start = buffer.as_mut_ptr().cast::<u8>();
            let detail = buffer_start.cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            let mut device_info = SP_DEVINFO_DATA {
                cbSize: u32::try_from(size_of::<SP_DEVINFO_DATA>())
                    .expect("SP_DEVINFO_DATA size fits u32"),
                ..Default::default()
            };
            // SAFETY: detail points into an aligned-enough byte allocation of required_size bytes.
            unsafe {
                (*detail).cbSize = u32::try_from(size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>())
                    .expect("SP_DEVICE_INTERFACE_DETAIL_DATA_W size fits u32");
                SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface_data,
                    Some(detail),
                    required_size,
                    None,
                    Some(&mut device_info),
                )?;
            }
            // SAFETY: detail was populated above and DevicePath starts a NUL-terminated tail array.
            let device_path = unsafe {
                let start = addr_of!((*detail).DevicePath).cast::<u16>();
                let offset = start as usize - buffer_start as usize;
                let units = (required_size as usize - offset) / size_of::<u16>();
                wide_string_from_ptr(start, units)
            }
            .to_ascii_lowercase();
            let edid = read_monitor_edid(set.0, &device_info)?;
            monitors.push(PresentMonitor { device_path, edid });
        }
        Ok(monitors)
    }

    fn read_monitor_edid(set: HDEVINFO, device_info: &SP_DEVINFO_DATA) -> Result<Option<Vec<u8>>> {
        // SAFETY: device_info belongs to set and KEY_READ requests a read-only device registry key.
        let key = unsafe {
            SetupDiOpenDevRegKey(
                set,
                device_info,
                DICS_FLAG_GLOBAL.0,
                0,
                DIREG_DEV,
                KEY_READ.0,
            )
        }?;
        let key = RegistryKey(key);
        let mut value_type = REG_VALUE_TYPE::default();
        let mut size = 0;
        // SAFETY: key is valid and this first call only requests the byte count and type.
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                w!("EDID"),
                None,
                Some(&mut value_type),
                None,
                Some(&mut size),
            )
        };
        if status != ERROR_SUCCESS {
            return Ok(None);
        }
        if value_type != REG_BINARY || size == 0 {
            return Ok(None);
        }
        let mut bytes = vec![0_u8; size as usize];
        // SAFETY: bytes has the size reported by the registry and remains writable for the call.
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                w!("EDID"),
                None,
                Some(&mut value_type),
                Some(bytes.as_mut_ptr()),
                Some(&mut size),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(win32_error("RegQueryValueExW(EDID)", status));
        }
        bytes.truncate(size as usize);
        Ok(Some(bytes))
    }

    fn source_key(adapter: LUID, source_id: u32) -> DisplaySourceKey {
        DisplaySourceKey {
            adapter_low: adapter.LowPart,
            adapter_high: adapter.HighPart,
            source_id,
        }
    }

    fn target_key(adapter: LUID, target_id: u32) -> DisplayTargetKey {
        DisplayTargetKey {
            adapter_low: adapter.LowPart,
            adapter_high: adapter.HighPart,
            target_id,
        }
    }

    fn fallback_display_name(kind: DisplayKind, model: u32) -> String {
        match kind {
            DisplayKind::BuiltIn => "내장 디스플레이".to_owned(),
            DisplayKind::External => format!("외장 디스플레이 {model:08X}"),
            DisplayKind::Virtual => format!("가상 디스플레이 {model:08X}"),
        }
    }

    fn wide_string(buffer: &[u16]) -> String {
        let length = buffer
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..length])
    }

    unsafe fn wide_string_from_ptr(pointer: *const u16, maximum: usize) -> String {
        let mut length = 0;
        while length < maximum && unsafe { *pointer.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: length was bounded by maximum and pointer addresses the SetupAPI output buffer.
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(pointer, length) })
    }

    fn win32_error(context: &str, status: WIN32_ERROR) -> Error {
        Error::new(status.to_hresult(), context.to_owned())
    }

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
pub use windows_impl::{
    current_display_configuration, enumerate_monitors, monitor_by_key, DisplayConfiguration,
    Monitor,
};

#[cfg(test)]
mod tests {
    use super::{
        edid_identity, identity_stage, is_primary_display, local_point_in_monitor,
        monitor_index_at_point, online_displays_from_paths, plan_monitor_sync,
        ConnectionSetOutcome, DisplayChangeState, DisplayPathRecord, DisplaySourceKey,
        DisplayTargetKey, IdentityStage, MonitorRect, MonitorSnapshot, MonitorSyncPlan,
    };
    use std::collections::HashSet;

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

    fn display_path(
        source_id: u32,
        target_id: u32,
        x: i32,
        device_path: &str,
    ) -> DisplayPathRecord {
        DisplayPathRecord {
            source: DisplaySourceKey {
                adapter_low: 1,
                adapter_high: 0,
                source_id,
            },
            target: DisplayTargetKey {
                adapter_low: 1,
                adapter_high: 0,
                target_id,
            },
            target_available: true,
            output_technology: 5,
            width: 1_920,
            height: 1_080,
            x,
            y: 0,
            device_path: device_path.to_owned(),
            vendor: 0x1234,
            model: 0x5678,
            serial: target_id,
            name: format!("monitor-{target_id}"),
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

    #[test]
    fn zoom_start_monitor_and_local_center_table_handles_boundaries_and_negative_origins() {
        let rects = [
            MonitorRect {
                left: -1_280,
                top: -200,
                right: 0,
                bottom: 824,
            },
            MonitorRect {
                left: 0,
                top: 0,
                right: 1_920,
                bottom: 1_080,
            },
        ];
        let cases = [
            ((-1, 10), Some((0, (1_279, 210)))),
            ((0, 10), Some((1, (0, 10)))),
            ((-1_280, -200), Some((0, (0, 0)))),
            ((1_920, 10), None),
        ];
        for ((x, y), expected) in cases {
            let actual = monitor_index_at_point(&rects, x, y)
                .map(|index| (index, local_point_in_monitor(rects[index], x, y)));
            assert_eq!(actual, expected, "point=({x},{y})");
        }
    }

    #[test]
    fn ccd_path_conversion_table_covers_single_extended_clone_primary_and_master_rule() {
        let cases = [
            (
                "single",
                vec![display_path(10, 100, 0, "PATH-A")],
                vec![(1, 0, None, true)],
            ),
            (
                "extended two",
                vec![
                    display_path(10, 100, 0, "PATH-A"),
                    display_path(11, 101, 1_920, "PATH-B"),
                ],
                vec![(1, 0, None, true), (2, 1_920, None, false)],
            ),
            (
                "cloned two",
                vec![
                    display_path(10, 100, 0, "PATH-A"),
                    display_path(10, 101, 0, "PATH-B"),
                ],
                vec![(1, 0, None, true), (2, 0, Some(1), true)],
            ),
        ];

        for (name, paths, expected) in cases {
            let converted =
                online_displays_from_paths(&paths).expect("synthetic CCD paths convert");
            let actual = converted
                .displays
                .iter()
                .map(|record| {
                    (
                        record.display.id,
                        record.display.origin.x as i32,
                        record.display.current_mirror_master_id,
                        is_primary_display(&record.display),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{name}");
        }
    }

    #[test]
    fn identity_stage_table_covers_device_path_zero_serial_and_virtual_monitor() {
        let mut no_path_with_serial = display_path(10, 100, 0, "");
        no_path_with_serial.serial = 77;
        let mut no_path_no_serial = no_path_with_serial.clone();
        no_path_no_serial.serial = 0;
        let mut virtual_monitor = no_path_no_serial.clone();
        virtual_monitor.output_technology = 15;
        let cases = [
            (
                display_path(10, 100, 0, "PATH-A"),
                IdentityStage::DevicePath,
            ),
            (no_path_with_serial, IdentityStage::EdidSerial),
            (no_path_no_serial, IdentityStage::Unmatchable),
            (virtual_monitor, IdentityStage::VirtualName),
        ];
        for (path, expected) in cases {
            assert_eq!(identity_stage(&path), expected);
        }
        assert_eq!(
            edid_identity(&[0, 0, 0, 0, 0, 0, 0, 0, 0x12, 0x34, 0x78, 0x56, 0x2a, 0, 0, 0,]),
            Some((0x1234, 0x5678, 42))
        );
    }

    #[test]
    fn display_change_state_debounces_repeated_notifications_to_one_judgment() {
        let mut state = DisplayChangeState::default();
        state.initialize(HashSet::from(["a".to_owned()]));
        let stale_generation = state.notify();
        let current_generation = state.notify();
        let changed = HashSet::from(["a".to_owned(), "b".to_owned()]);
        assert_eq!(
            state.settle(stale_generation, &changed),
            ConnectionSetOutcome::Stale
        );
        assert_eq!(
            state.settle(current_generation, &changed),
            ConnectionSetOutcome::Changed
        );
        assert_eq!(
            state.settle(current_generation, &changed),
            ConnectionSetOutcome::Stale
        );
    }

    #[test]
    fn display_change_state_ignores_same_set_and_topology_only_notifications() {
        let mut state = DisplayChangeState::default();
        let connected = HashSet::from(["a".to_owned(), "b".to_owned()]);
        state.initialize(connected.clone());
        for reason in ["same-set notification", "Win+P topology only"] {
            let generation = state.notify();
            assert_eq!(
                state.settle(generation, &connected),
                ConnectionSetOutcome::Unchanged,
                "{reason}"
            );
        }
    }
}
