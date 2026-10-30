use edutools_core::display_operations::Operation;
use edutools_core::display_preset::{DisplayPoint, OnlineDisplay};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayApplyState {
    Idle,
    Applying,
    AwaitingConfirmation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayApplySource {
    Manual,
    Automatic,
}

impl DisplayApplySource {
    #[cfg_attr(not(windows), allow(dead_code))]
    pub const fn log_name(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Automatic => "automatic",
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmationDecision {
    Keep,
    Revert,
    Expire,
    Terminate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayApplyStateMachine {
    state: DisplayApplyState,
}

impl Default for DisplayApplyStateMachine {
    fn default() -> Self {
        Self {
            state: DisplayApplyState::Idle,
        }
    }
}

impl DisplayApplyStateMachine {
    pub const fn state(&self) -> DisplayApplyState {
        self.state
    }

    pub fn begin_request(&mut self) -> Result<(), &'static str> {
        match self.state {
            DisplayApplyState::Idle => {
                self.state = DisplayApplyState::Applying;
                Ok(())
            }
            DisplayApplyState::Applying => Err("다른 화면 구성을 적용하는 중입니다"),
            DisplayApplyState::AwaitingConfirmation => {
                Err("현재 화면 구성을 먼저 유지하거나 되돌리세요")
            }
        }
    }

    pub fn applied(&mut self, source: DisplayApplySource) {
        debug_assert_eq!(self.state, DisplayApplyState::Applying);
        self.state = match source {
            DisplayApplySource::Manual => DisplayApplyState::AwaitingConfirmation,
            DisplayApplySource::Automatic => DisplayApplyState::Idle,
        };
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn failed(&mut self) {
        debug_assert_eq!(self.state, DisplayApplyState::Applying);
        self.state = DisplayApplyState::Idle;
    }

    pub fn resolve(&mut self, _decision: ConfirmationDecision) -> bool {
        if self.state != DisplayApplyState::AwaitingConfirmation {
            return false;
        }
        self.state = DisplayApplyState::Idle;
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SourceKey {
    adapter_low: u32,
    adapter_high: i32,
    source_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct TopologyPath {
    display_id: u32,
    target_id: u32,
    adapter_low: u32,
    adapter_high: i32,
    source_id: u32,
    width: u32,
    height: u32,
    origin: DisplayPoint,
}

impl TopologyPath {
    const fn source_key(&self) -> SourceKey {
        SourceKey {
            adapter_low: self.adapter_low,
            adapter_high: self.adapter_high,
            source_id: self.source_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DesiredPath {
    display_id: u32,
    source: SourceKey,
    mode_master_id: u32,
    width: u32,
    height: u32,
    origin: DisplayPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TopologyBuildError(String);

impl fmt::Display for TopologyBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn build_desired_paths(
    paths: &[TopologyPath],
    operations: &[Operation],
) -> Result<Vec<DesiredPath>, TopologyBuildError> {
    let mut used_sources = paths
        .iter()
        .map(TopologyPath::source_key)
        .collect::<HashSet<_>>();
    let mut first_for_source = HashSet::new();
    let mut own_sources = HashMap::new();

    for path in paths {
        let current = path.source_key();
        let own = if first_for_source.insert(current) {
            current
        } else {
            let mut candidate = SourceKey {
                adapter_low: path.adapter_low,
                adapter_high: path.adapter_high,
                source_id: path.target_id,
            };
            if used_sources.contains(&candidate) {
                let Some(source_id) = (0..=u16::MAX as u32).find(|source_id| {
                    !used_sources.contains(&SourceKey {
                        adapter_low: path.adapter_low,
                        adapter_high: path.adapter_high,
                        source_id: *source_id,
                    })
                }) else {
                    return Err(TopologyBuildError(format!(
                        "모니터 {}에 독립 소스를 배정할 수 없습니다",
                        path.display_id
                    )));
                };
                candidate.source_id = source_id;
            }
            used_sources.insert(candidate);
            candidate
        };
        own_sources.insert(path.display_id, own);
    }

    let mut desired_sources = own_sources.clone();
    let mut mode_masters = paths
        .iter()
        .map(|path| (path.display_id, path.display_id))
        .collect::<HashMap<_, _>>();
    let mut origins = paths
        .iter()
        .map(|path| (path.display_id, path.origin))
        .collect::<HashMap<_, _>>();
    let mut explicit_origins = HashSet::new();

    for operation in operations {
        match *operation {
            Operation::Unmirror { display_id } => {
                let own = own_sources.get(&display_id).copied().ok_or_else(|| {
                    TopologyBuildError(format!(
                        "연산 대상 모니터 {display_id}가 활성 경로에 없습니다"
                    ))
                })?;
                desired_sources.insert(display_id, own);
            }
            Operation::Mirror {
                display_id,
                master_id,
            } => {
                let slave = paths
                    .iter()
                    .find(|path| path.display_id == display_id)
                    .ok_or_else(|| {
                        TopologyBuildError(format!(
                            "복제 슬레이브 모니터 {display_id}가 활성 경로에 없습니다"
                        ))
                    })?;
                let master = paths
                    .iter()
                    .find(|path| path.display_id == master_id)
                    .ok_or_else(|| {
                        TopologyBuildError(format!(
                            "복제 마스터 모니터 {master_id}가 활성 경로에 없습니다"
                        ))
                    })?;
                if (slave.adapter_low, slave.adapter_high)
                    != (master.adapter_low, master.adapter_high)
                {
                    return Err(TopologyBuildError(format!(
                        "어댑터가 다른 모니터끼리는 복제할 수 없습니다: slave={display_id} master={master_id}"
                    )));
                }
                let source = desired_sources.get(&master_id).copied().ok_or_else(|| {
                    TopologyBuildError(format!("복제 마스터 {master_id}의 소스를 찾지 못했습니다"))
                })?;
                desired_sources.insert(display_id, source);
                let mode_master = mode_masters.get(&master_id).copied().ok_or_else(|| {
                    TopologyBuildError(format!(
                        "복제 마스터 {master_id}의 모드 기준을 찾지 못했습니다"
                    ))
                })?;
                mode_masters.insert(display_id, mode_master);
            }
            Operation::SetOrigin { display_id, origin } => {
                if !origins.contains_key(&display_id) {
                    return Err(TopologyBuildError(format!(
                        "원점 연산 대상 모니터 {display_id}가 활성 경로에 없습니다"
                    )));
                }
                origins.insert(display_id, origin);
                explicit_origins.insert(display_id);
            }
        }
    }

    let mut source_origins = HashMap::<SourceKey, DisplayPoint>::new();
    for path in paths
        .iter()
        .filter(|path| explicit_origins.contains(&path.display_id))
    {
        let source = desired_sources[&path.display_id];
        let origin = origins[&path.display_id];
        if let Some(existing) = source_origins.insert(source, origin) {
            if existing != origin {
                return Err(TopologyBuildError(format!(
                    "같은 복제 소스에 서로 다른 원점이 요청되었습니다: source={} ({},{}) vs ({},{})",
                    source.source_id, existing.x, existing.y, origin.x, origin.y
                )));
            }
        }
    }
    for path in paths {
        let source = desired_sources[&path.display_id];
        let origin = origins[&path.display_id];
        source_origins.entry(source).or_insert(origin);
    }

    Ok(paths
        .iter()
        .map(|path| {
            let source = desired_sources[&path.display_id];
            DesiredPath {
                display_id: path.display_id,
                source,
                mode_master_id: mode_masters[&path.display_id],
                width: path.width,
                height: path.height,
                origin: source_origins[&source],
            }
        })
        .collect())
}

pub fn confirmation_message(remaining_seconds: u64) -> String {
    format!("이 화면 구성을 유지하시겠습니까? {remaining_seconds}초 후 이전 구성으로 돌아갑니다")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingDisplayMarker {
    snapshot_id: String,
    created_unix_ms: u64,
}

impl PendingDisplayMarker {
    fn json(&self) -> String {
        format!(
            "{{\n  \"snapshotId\": \"{}\",\n  \"createdUnixMs\": {}\n}}\n",
            self.snapshot_id, self.created_unix_ms
        )
    }

    fn parse(value: &str) -> io::Result<Self> {
        let snapshot_id = json_string_field(value, "snapshotId")?;
        let created_unix_ms = json_u64_field(value, "createdUnixMs")?;
        Ok(Self {
            snapshot_id,
            created_unix_ms,
        })
    }
}

fn write_pending_marker(path: &Path, marker: &PendingDisplayMarker) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "미확정 화면 구성 마커가 이미 있습니다",
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, marker.json())?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

fn read_pending_marker(path: &Path) -> io::Result<PendingDisplayMarker> {
    PendingDisplayMarker::parse(&fs::read_to_string(path)?)
}

fn delete_pending_marker(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn json_string_field(value: &str, key: &str) -> io::Result<String> {
    let prefix = format!("\"{key}\"");
    let after_key = value
        .split_once(&prefix)
        .map(|(_, tail)| tail)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing {key}")))?;
    let after_colon = after_key
        .split_once(':')
        .map(|(_, tail)| tail.trim_start())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("invalid {key}")))?;
    let after_quote = after_colon
        .strip_prefix('"')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("invalid {key}")))?;
    let end = after_quote
        .find('"')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("invalid {key}")))?;
    Ok(after_quote[..end].to_owned())
}

fn json_u64_field(value: &str, key: &str) -> io::Result<u64> {
    let prefix = format!("\"{key}\"");
    let after_key = value
        .split_once(&prefix)
        .map(|(_, tail)| tail)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing {key}")))?;
    let digits = after_key
        .split_once(':')
        .map(|(_, tail)| tail.trim_start())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("invalid {key}")))?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    digits
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("invalid {key}")))
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub struct ExpectedDisplayConfiguration {
    displays: Vec<ExpectedDisplay>,
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
struct ExpectedDisplay {
    uuid: String,
    origin: DisplayPoint,
    mirror_master_uuid: Option<String>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl ExpectedDisplayConfiguration {
    pub fn capture(online: &[OnlineDisplay]) -> Self {
        let uuid_by_id = online
            .iter()
            .map(|display| (display.id, display.uuid.clone()))
            .collect::<HashMap<_, _>>();
        let mut displays = online
            .iter()
            .map(|display| ExpectedDisplay {
                uuid: display.uuid.clone(),
                origin: display.origin,
                mirror_master_uuid: display
                    .current_mirror_master_id
                    .and_then(|id| uuid_by_id.get(&id).cloned()),
            })
            .collect::<Vec<_>>();
        displays.sort_by(|left, right| left.uuid.cmp(&right.uuid));
        Self { displays }
    }

    pub fn matches(&self, online: &[OnlineDisplay]) -> bool {
        *self == Self::capture(online)
    }

    pub fn active_uuids(&self) -> HashSet<String> {
        self.displays
            .iter()
            .filter_map(|display| (!display.uuid.is_empty()).then(|| display.uuid.clone()))
            .collect()
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn selftest_failures() -> Vec<String> {
    let mut failures = Vec::new();
    let directory = std::env::temp_dir().join(format!(
        "edutools-display-selftest-{}-{}",
        std::process::id(),
        SystemTimeForSelftest::nonce()
    ));
    let marker_path = directory.join("display-pending.json");
    let marker = PendingDisplayMarker {
        snapshot_id: "selftest-snapshot".to_owned(),
        created_unix_ms: 42,
    };
    match write_pending_marker(&marker_path, &marker)
        .and_then(|()| read_pending_marker(&marker_path))
    {
        Ok(round_trip) if round_trip == marker => {}
        Ok(round_trip) => failures.push(format!("display marker 왕복 불일치: {round_trip:?}")),
        Err(error) => failures.push(format!("display marker 왕복 실패: {error}")),
    }
    let _ = delete_pending_marker(&marker_path);
    let _ = fs::remove_dir(&directory);

    let paths = vec![
        TopologyPath {
            display_id: 1,
            target_id: 1,
            adapter_low: 1,
            adapter_high: 0,
            source_id: 10,
            width: 1_920,
            height: 1_080,
            origin: DisplayPoint::zero(),
        },
        TopologyPath {
            display_id: 2,
            target_id: 2,
            adapter_low: 1,
            adapter_high: 0,
            source_id: 11,
            width: 1_280,
            height: 1_024,
            origin: DisplayPoint { x: 1_920.0, y: 0.0 },
        },
    ];
    let operations = [
        Operation::Unmirror { display_id: 1 },
        Operation::Unmirror { display_id: 2 },
        Operation::Mirror {
            display_id: 2,
            master_id: 1,
        },
    ];
    match build_desired_paths(&paths, &operations) {
        Ok(desired)
            if desired.len() == 2
                && desired[0].source == desired[1].source
                && desired[0].width == 1_920
                && desired[1].width == 1_280 => {}
        Ok(desired) => failures.push(format!("display 합성 경로 변환 불일치: {desired:?}")),
        Err(error) => failures.push(format!("display 합성 경로 변환 실패: {error}")),
    }
    failures
}

struct SystemTimeForSelftest;

impl SystemTimeForSelftest {
    fn nonce() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }
}

#[cfg(windows)]
mod windows_impl;

#[cfg(windows)]
pub use windows_impl::{
    decision_from_command, is_console_session, recover_pending_display_configuration,
    run_display_apply_dry_run, ConfirmationResolution, DisplayApplier, DisplayPresetMenuEntry,
    WM_APP_DISPLAY_DECISION, WM_APP_DISPLAY_EXPIRED,
};

#[cfg(test)]
mod tests {
    use super::{
        build_desired_paths, confirmation_message, delete_pending_marker, read_pending_marker,
        write_pending_marker, DisplayApplySource, DisplayApplyState, DisplayApplyStateMachine,
        PendingDisplayMarker, TopologyPath,
    };
    use edutools_core::display_operations::Operation;
    use edutools_core::display_preset::DisplayPoint;

    #[test]
    fn display_apply_state_machine_transition_table() {
        #[derive(Clone, Copy, Debug)]
        enum Event {
            Request,
            Decision,
            Expiry,
            Termination,
        }
        let cases = [
            (
                DisplayApplyState::Idle,
                Event::Request,
                true,
                DisplayApplyState::Applying,
            ),
            (
                DisplayApplyState::Idle,
                Event::Decision,
                false,
                DisplayApplyState::Idle,
            ),
            (
                DisplayApplyState::Idle,
                Event::Expiry,
                false,
                DisplayApplyState::Idle,
            ),
            (
                DisplayApplyState::Idle,
                Event::Termination,
                false,
                DisplayApplyState::Idle,
            ),
            (
                DisplayApplyState::Applying,
                Event::Request,
                false,
                DisplayApplyState::Applying,
            ),
            (
                DisplayApplyState::Applying,
                Event::Decision,
                false,
                DisplayApplyState::Applying,
            ),
            (
                DisplayApplyState::Applying,
                Event::Expiry,
                false,
                DisplayApplyState::Applying,
            ),
            (
                DisplayApplyState::Applying,
                Event::Termination,
                false,
                DisplayApplyState::Applying,
            ),
            (
                DisplayApplyState::AwaitingConfirmation,
                Event::Request,
                false,
                DisplayApplyState::AwaitingConfirmation,
            ),
            (
                DisplayApplyState::AwaitingConfirmation,
                Event::Decision,
                true,
                DisplayApplyState::Idle,
            ),
            (
                DisplayApplyState::AwaitingConfirmation,
                Event::Expiry,
                true,
                DisplayApplyState::Idle,
            ),
            (
                DisplayApplyState::AwaitingConfirmation,
                Event::Termination,
                true,
                DisplayApplyState::Idle,
            ),
        ];
        for (initial, event, accepted, expected) in cases {
            let mut machine = DisplayApplyStateMachine { state: initial };
            let actual = match event {
                Event::Request => machine.begin_request().is_ok(),
                Event::Decision => machine.resolve(super::ConfirmationDecision::Keep),
                Event::Expiry => machine.resolve(super::ConfirmationDecision::Expire),
                Event::Termination => machine.resolve(super::ConfirmationDecision::Terminate),
            };
            assert_eq!(actual, accepted, "{event:?} from {initial:?}");
            assert_eq!(machine.state(), expected, "{event:?} from {initial:?}");
        }

        for source in [DisplayApplySource::Manual, DisplayApplySource::Automatic] {
            let mut machine = DisplayApplyStateMachine::default();
            machine.begin_request().unwrap();
            machine.applied(source);
            assert_eq!(
                machine.state(),
                if source == DisplayApplySource::Manual {
                    DisplayApplyState::AwaitingConfirmation
                } else {
                    DisplayApplyState::Idle
                }
            );
        }
    }

    #[test]
    fn operation_list_to_paths_preserves_modes_for_extend_clone_and_outside() {
        let paths = vec![
            path(1, 1, 10, 1920, 1080, 0.0),
            path(2, 2, 11, 1280, 1024, 1920.0),
            path(3, 3, 12, 1024, 768, 3200.0),
            path(4, 4, 12, 1024, 768, 3200.0),
            path(5, 5, 13, 1600, 900, 4224.0),
        ];
        let operations = vec![
            Operation::Unmirror { display_id: 1 },
            Operation::Unmirror { display_id: 2 },
            Operation::Unmirror { display_id: 3 },
            Operation::Unmirror { display_id: 4 },
            Operation::Unmirror { display_id: 5 },
            Operation::Mirror {
                display_id: 4,
                master_id: 3,
            },
            Operation::SetOrigin {
                display_id: 1,
                origin: DisplayPoint { x: 0.0, y: 0.0 },
            },
            Operation::SetOrigin {
                display_id: 2,
                origin: DisplayPoint { x: 1920.0, y: 0.0 },
            },
            Operation::SetOrigin {
                display_id: 3,
                origin: DisplayPoint { x: -1024.0, y: 0.0 },
            },
            Operation::SetOrigin {
                display_id: 5,
                origin: DisplayPoint { x: 3200.0, y: 0.0 },
            },
        ];
        let desired = build_desired_paths(&paths, &operations).unwrap();
        assert_eq!(desired[3].source, desired[2].source, "clone shares source");
        assert_eq!(desired[3].origin, desired[2].origin, "clone shares origin");
        assert_eq!(desired[4].origin.x, 3200.0, "outside placement is retained");
        assert_eq!(
            desired
                .iter()
                .map(|path| (path.width, path.height))
                .collect::<Vec<_>>(),
            paths
                .iter()
                .map(|path| (path.width, path.height))
                .collect::<Vec<_>>(),
            "only topology and positions change"
        );
    }

    #[test]
    fn confirmation_countdown_uses_literal_product_copy() {
        assert_eq!(
            confirmation_message(15),
            "이 화면 구성을 유지하시겠습니까? 15초 후 이전 구성으로 돌아갑니다"
        );
        assert_eq!(
            confirmation_message(0),
            "이 화면 구성을 유지하시겠습니까? 0초 후 이전 구성으로 돌아갑니다"
        );
    }

    #[test]
    fn pending_marker_round_trip_uses_a_real_temporary_directory() {
        let directory = std::env::temp_dir().join(format!(
            "edutools-display-marker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("display-pending.json");
        let marker = PendingDisplayMarker {
            snapshot_id: "snapshot-deadbeef".to_owned(),
            created_unix_ms: 123_456,
        };
        write_pending_marker(&path, &marker).unwrap();
        assert_eq!(read_pending_marker(&path).unwrap(), marker);
        delete_pending_marker(&path).unwrap();
        assert!(!path.exists());
        std::fs::remove_dir(directory).unwrap();
    }

    fn path(
        display_id: u32,
        target_id: u32,
        source_id: u32,
        width: u32,
        height: u32,
        x: f64,
    ) -> TopologyPath {
        TopologyPath {
            display_id,
            target_id,
            adapter_low: 1,
            adapter_high: 0,
            source_id,
            width,
            height,
            origin: DisplayPoint { x, y: 0.0 },
        }
    }
}
