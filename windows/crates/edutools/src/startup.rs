use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(windows)]
pub const COPYDATA_TAG: usize = 0x4554_5053;
pub const MAX_COPYDATA_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchRequest {
    App {
        import_path: Option<PathBuf>,
        warning: Option<ArgumentWarning>,
    },
    Selftest,
    SelftestIcons(PathBuf),
    PrintDisplays,
    DisplayApplyDryRun(usize),
    UsageError(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentWarning {
    pub file_name: String,
    pub reason: &'static str,
}

pub fn parse_arguments(arguments: &[OsString]) -> LaunchRequest {
    if arguments.is_empty() {
        return LaunchRequest::App {
            import_path: None,
            warning: None,
        };
    }

    if arguments[0] == "--selftest" {
        return if arguments.len() == 1 {
            LaunchRequest::Selftest
        } else {
            LaunchRequest::UsageError("사용법: edutools --selftest")
        };
    }
    if arguments[0] == "--selftest-icons" {
        return if arguments.len() == 2 {
            LaunchRequest::SelftestIcons(PathBuf::from(&arguments[1]))
        } else {
            LaunchRequest::UsageError("사용법: edutools --selftest-icons <출력 폴더>")
        };
    }
    if arguments[0] == "--print-displays" {
        return if arguments.len() == 1 {
            LaunchRequest::PrintDisplays
        } else {
            LaunchRequest::UsageError("사용법: edutools --print-displays")
        };
    }
    if arguments[0] == "--display-apply-dry-run" {
        if arguments.len() != 2 {
            return LaunchRequest::UsageError(
                "사용법: edutools --display-apply-dry-run <1-based-index>",
            );
        }
        return arguments[1]
            .to_string_lossy()
            .parse::<usize>()
            .ok()
            .filter(|index| *index > 0)
            .map(|index| LaunchRequest::DisplayApplyDryRun(index - 1))
            .unwrap_or(LaunchRequest::UsageError(
                "--display-apply-dry-run index는 1 이상의 정수여야 합니다",
            ));
    }

    if arguments.len() > 1 {
        return LaunchRequest::App {
            import_path: None,
            warning: Some(ArgumentWarning {
                file_name: file_name(&arguments[0]),
                reason: "파일 인자는 하나만 허용됩니다",
            }),
        };
    }

    let path = PathBuf::from(&arguments[0]);
    if has_pansetting_extension(&path) {
        LaunchRequest::App {
            import_path: Some(path),
            warning: None,
        }
    } else {
        LaunchRequest::App {
            import_path: None,
            warning: Some(ArgumentWarning {
                file_name: file_name(&arguments[0]),
                reason: ".pansetting 확장자가 아닙니다",
            }),
        }
    }
}

#[cfg(windows)]
pub fn validate_import_path(path: &Path) -> Result<PathBuf, &'static str> {
    if !has_pansetting_extension(path) {
        return Err(".pansetting 확장자가 아닙니다");
    }
    if !path.exists() {
        return Err("파일이 없습니다");
    }
    if !path.is_file() {
        return Err("일반 파일이 아닙니다");
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| "현재 폴더를 확인할 수 없습니다")?
            .join(path)
    };
    let absolute = absolute
        .canonicalize()
        .map_err(|_| "절대 경로를 확인할 수 없습니다")?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units = absolute.as_os_str().encode_wide().count();
        if (units + 1) * 2 > MAX_COPYDATA_BYTES {
            return Err("경로가 너무 깁니다");
        }
    }
    Ok(absolute)
}

pub fn validate_copydata_utf16(
    units: &[u16],
    declared_bytes: usize,
) -> Result<PathBuf, &'static str> {
    if declared_bytes < 2 || declared_bytes > MAX_COPYDATA_BYTES || declared_bytes % 2 != 0 {
        return Err("페이로드 길이가 허용 범위 밖입니다");
    }
    if units.len() * 2 != declared_bytes {
        return Err("페이로드 길이가 버퍼와 다릅니다");
    }
    if units.last() != Some(&0) {
        return Err("UTF-16 널 종료가 없습니다");
    }
    let content = &units[..units.len() - 1];
    if content.is_empty() || content.contains(&0) {
        return Err("비어 있거나 중간 널이 있는 경로입니다");
    }
    let text = String::from_utf16(content).map_err(|_| "유효하지 않은 UTF-16입니다")?;
    if !looks_like_pansetting_path(&text) {
        return Err(".pansetting 경로가 아닙니다");
    }
    Ok(PathBuf::from(text))
}

pub fn file_name_from_path(path: &Path) -> String {
    let display = path.as_os_str().to_string_lossy();
    display
        .rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| "설정 파일".to_owned())
}

fn has_pansetting_extension(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension
            .to_string_lossy()
            .eq_ignore_ascii_case("pansetting")
    })
}

fn looks_like_pansetting_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    (value.contains('\\') || value.contains('/'))
        && lower.ends_with(".pansetting")
        && !value.ends_with(['\\', '/'])
}

fn file_name(value: &OsString) -> String {
    file_name_from_path(Path::new(value))
}

#[cfg(test)]
mod tests {
    use super::{parse_arguments, validate_copydata_utf16, ArgumentWarning, LaunchRequest};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn command_line_parser_table() {
        let cases = [
            (
                vec![],
                LaunchRequest::App {
                    import_path: None,
                    warning: None,
                },
            ),
            (vec!["--selftest"], LaunchRequest::Selftest),
            (
                vec![r"C:\수업\기본.pansetting"],
                LaunchRequest::App {
                    import_path: Some(PathBuf::from(r"C:\수업\기본.pansetting")),
                    warning: None,
                },
            ),
            (
                vec![r"C:\수업\기본.json"],
                LaunchRequest::App {
                    import_path: None,
                    warning: Some(ArgumentWarning {
                        file_name: "기본.json".to_owned(),
                        reason: ".pansetting 확장자가 아닙니다",
                    }),
                },
            ),
            (
                vec![r"C:\수업\하나.pansetting", r"C:\수업\둘.pansetting"],
                LaunchRequest::App {
                    import_path: None,
                    warning: Some(ArgumentWarning {
                        file_name: "하나.pansetting".to_owned(),
                        reason: "파일 인자는 하나만 허용됩니다",
                    }),
                },
            ),
        ];
        for (input, expected) in cases {
            let input = input.into_iter().map(OsString::from).collect::<Vec<_>>();
            assert_eq!(parse_arguments(&input), expected, "input={input:?}");
        }
    }

    #[test]
    fn copydata_payload_validation_table() {
        fn wide(value: &str, nul: bool) -> Vec<u16> {
            value.encode_utf16().chain(nul.then_some(0)).collect()
        }

        let valid = wide(r"C:\수업\기본.pansetting", true);
        let too_long = vec![b'a' as u16; super::MAX_COPYDATA_BYTES / 2 + 1];
        let no_nul = wide(r"C:\수업\기본.pansetting", false);
        let not_path = wide("기본.pansetting", true);
        let cases = [
            ("valid", valid.clone(), valid.len() * 2, true),
            ("length-cap", too_long.clone(), too_long.len() * 2, false),
            ("nul-termination", no_nul.clone(), no_nul.len() * 2, false),
            ("not-a-path", not_path.clone(), not_path.len() * 2, false),
        ];
        for (label, payload, bytes, expected) in cases {
            assert_eq!(
                validate_copydata_utf16(&payload, bytes).is_ok(),
                expected,
                "{label}"
            );
        }
    }
}
