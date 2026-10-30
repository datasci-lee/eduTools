use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use windows::core::PCWSTR;
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::System::SystemInformation::GetLocalTime;

#[derive(Clone)]
pub struct Logger {
    file: Arc<Mutex<Option<File>>>,
}

impl Logger {
    pub fn open() -> Self {
        let file = match log_path().and_then(|path| {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            OpenOptions::new().create(true).append(true).open(path)
        }) {
            Ok(file) => Some(file),
            Err(error) => {
                eprintln!("EduTools: 로그 파일을 열 수 없습니다: {error}");
                None
            }
        };
        Self {
            file: Arc::new(Mutex::new(file)),
        }
    }

    pub fn event(&self, event: &str) {
        // SAFETY: GetLocalTime fills and returns an owned SYSTEMTIME value.
        let time = unsafe { GetLocalTime() };
        let line = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {event}\n",
            time.wYear,
            time.wMonth,
            time.wDay,
            time.wHour,
            time.wMinute,
            time.wSecond,
            time.wMilliseconds
        );
        if let Ok(mut slot) = self.file.lock() {
            let failure = slot.as_mut().and_then(|file| {
                file.write_all(line.as_bytes())
                    .and_then(|_| file.flush())
                    .err()
            });
            if let Some(error) = failure {
                eprintln!("EduTools: 로그 파일에 쓸 수 없습니다: {error}");
                *slot = None;
            }
        } else {
            eprintln!("EduTools: 로그 잠금을 얻을 수 없습니다");
        }

        let wide = line
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: wide is NUL-terminated and remains alive for the duration of the call.
        unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
    }
}

fn log_path() -> std::io::Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "LOCALAPPDATA 환경 변수가 없습니다",
        )
    })?;
    Ok(PathBuf::from(local).join("EduTools/edutools.log"))
}
