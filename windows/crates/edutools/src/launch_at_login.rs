use std::path::Path;

#[cfg(windows)]
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
pub const RUN_VALUE_NAME: &str = "EduTools";

pub trait RunRegistry {
    fn read(&self) -> Result<Option<String>, String>;
    fn write(&mut self, value: &str) -> Result<(), String>;
    fn delete(&mut self) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncOutcome {
    Unchanged,
    Enabled,
    Disabled,
    UpdatedPath,
}

pub fn run_value(executable: &Path) -> Result<String, &'static str> {
    let value = executable.to_string_lossy();
    if !executable.is_absolute() && !looks_like_windows_absolute_path(&value) {
        return Err("실행 파일 경로가 절대 경로가 아닙니다");
    }
    if value.contains('"') || value.is_empty() {
        return Err("실행 파일 경로를 Run 값으로 인용할 수 없습니다");
    }
    Ok(format!("\"{value}\""))
}

fn looks_like_windows_absolute_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/'))
        || value.starts_with(r"\\")
}

pub fn synchronize<R: RunRegistry>(
    registry: &mut R,
    enabled: bool,
    executable: &Path,
) -> Result<SyncOutcome, String> {
    let current = registry.read()?;
    if !enabled {
        return if current.is_some() {
            registry.delete()?;
            Ok(SyncOutcome::Disabled)
        } else {
            Ok(SyncOutcome::Unchanged)
        };
    }

    let expected = run_value(executable).map_err(str::to_owned)?;
    match current {
        Some(current) if current == expected => Ok(SyncOutcome::Unchanged),
        Some(_) => {
            registry.write(&expected)?;
            Ok(SyncOutcome::UpdatedPath)
        }
        None => {
            registry.write(&expected)?;
            Ok(SyncOutcome::Enabled)
        }
    }
}

#[cfg(windows)]
pub fn selftest_failures() -> Vec<String> {
    #[derive(Default)]
    struct FakeRegistry {
        value: Option<String>,
    }

    impl RunRegistry for FakeRegistry {
        fn read(&self) -> Result<Option<String>, String> {
            Ok(self.value.clone())
        }

        fn write(&mut self, value: &str) -> Result<(), String> {
            self.value = Some(value.to_owned());
            Ok(())
        }

        fn delete(&mut self) -> Result<(), String> {
            self.value = None;
            Ok(())
        }
    }

    let mut failures = Vec::new();
    let executable = Path::new(r"C:\Program Files\EduTools\edutools.exe");
    let expected = r#""C:\Program Files\EduTools\edutools.exe""#;
    let mut registry = FakeRegistry::default();
    if synchronize(&mut registry, true, executable) != Ok(SyncOutcome::Enabled)
        || registry.value.as_deref() != Some(expected)
    {
        failures.push("자동 실행: 가짜 레지스트리 활성화 왕복 실패".to_owned());
    }
    if synchronize(&mut registry, false, executable) != Ok(SyncOutcome::Disabled)
        || registry.value.is_some()
    {
        failures.push("자동 실행: 가짜 레지스트리 비활성화 왕복 실패".to_owned());
    }
    failures
}

#[cfg(windows)]
pub struct WindowsRunRegistry;

#[cfg(windows)]
impl RunRegistry for WindowsRunRegistry {
    fn read(&self) -> Result<Option<String>, String> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

        let key = wide(RUN_KEY);
        let name = wide(RUN_VALUE_NAME);
        let mut byte_count = 0u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut byte_count),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(format!("Run 값 크기 조회 실패: Win32 {}", status.0));
        }
        if byte_count < 2 || byte_count % 2 != 0 {
            return Err("Run 값의 UTF-16 길이가 잘못되었습니다".to_owned());
        }
        let mut value = vec![0u16; (byte_count as usize).div_ceil(2)];
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(value.as_mut_ptr().cast()),
                Some(&mut byte_count),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("Run 값 읽기 실패: Win32 {}", status.0));
        }
        let end = value
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(value.len());
        String::from_utf16(&value[..end])
            .map(Some)
            .map_err(|_| "Run 값이 유효한 UTF-16이 아닙니다".to_owned())
    }

    fn write(&mut self, value: &str) -> Result<(), String> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::ERROR_SUCCESS;
        use windows::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
            REG_OPTION_NON_VOLATILE, REG_SZ,
        };

        let key = wide(RUN_KEY);
        let name = wide(RUN_VALUE_NAME);
        let value = value
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let bytes =
            unsafe { std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), value.len() * 2) };
        let mut handle = HKEY::default();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut handle,
                None,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("Run 키 열기 실패: Win32 {}", status.0));
        }
        let status =
            unsafe { RegSetValueExW(handle, PCWSTR(name.as_ptr()), None, REG_SZ, Some(bytes)) };
        let close_status = unsafe { RegCloseKey(handle) };
        if status != ERROR_SUCCESS {
            return Err(format!("Run 값 쓰기 실패: Win32 {}", status.0));
        }
        if close_status != ERROR_SUCCESS {
            return Err(format!("Run 키 닫기 실패: Win32 {}", close_status.0));
        }
        Ok(())
    }

    fn delete(&mut self) -> Result<(), String> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
        use windows::Win32::System::Registry::{RegDeleteKeyValueW, HKEY_CURRENT_USER};

        let key = wide(RUN_KEY);
        let name = wide(RUN_VALUE_NAME);
        let status = unsafe {
            RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
            )
        };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("Run 값 삭제 실패: Win32 {}", status.0))
        }
    }
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{run_value, synchronize, RunRegistry, SyncOutcome};
    use std::path::Path;

    #[derive(Default)]
    struct FakeRegistry {
        value: Option<String>,
    }

    impl RunRegistry for FakeRegistry {
        fn read(&self) -> Result<Option<String>, String> {
            Ok(self.value.clone())
        }

        fn write(&mut self, value: &str) -> Result<(), String> {
            self.value = Some(value.to_owned());
            Ok(())
        }

        fn delete(&mut self) -> Result<(), String> {
            self.value = None;
            Ok(())
        }
    }

    #[test]
    fn run_value_quotes_spaces_and_unicode_table() {
        let cases = [
            (r"C:\EduTools\edutools.exe", r#""C:\EduTools\edutools.exe""#),
            (
                r"C:\수업 도구\edutools.exe",
                r#""C:\수업 도구\edutools.exe""#,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                run_value(Path::new(input)),
                Ok(expected.to_owned()),
                "{input}"
            );
        }
    }

    #[test]
    fn synchronization_uses_fake_registry_round_trip() {
        let executable = Path::new(r"C:\수업 도구\edutools.exe");
        let expected = r#""C:\수업 도구\edutools.exe""#.to_owned();
        let mut registry = FakeRegistry::default();
        assert_eq!(
            synchronize(&mut registry, true, executable),
            Ok(SyncOutcome::Enabled)
        );
        assert_eq!(registry.value, Some(expected));
        assert_eq!(
            synchronize(&mut registry, false, executable),
            Ok(SyncOutcome::Disabled)
        );
        assert_eq!(registry.value, None);
    }
}
