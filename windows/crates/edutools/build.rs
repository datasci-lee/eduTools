use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TRAY_FILES: [&str; 4] = [
    "menubar.off.light-taskbar.ico",
    "menubar.on.light-taskbar.ico",
    "menubar.off.dark-taskbar.ico",
    "menubar.on.dark-taskbar.ico",
];

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required"));
    let repository_root = manifest_dir
        .ancestors()
        .nth(3)
        .expect("edutools must remain under windows/crates/edutools")
        .to_path_buf();
    let tray_dir = repository_root.join("assets/symbols/windows/tray");

    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("app.rc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("app.manifest").display()
    );
    for file in TRAY_FILES {
        println!("cargo:rerun-if-changed={}", tray_dir.join(file).display());
    }
    println!("cargo:rerun-if-env-changed=RC");

    let host = env::var("HOST").expect("HOST is required");
    if !host.contains("windows") {
        return;
    }

    let rc = find_resource_compiler().unwrap_or_else(|| {
        println!(
            "cargo:warning=rc.exe를 찾지 못했습니다. RC 환경 변수 또는 Windows 10 SDK를 확인하세요."
        );
        panic!("Windows 리소스 컴파일러 rc.exe가 필요합니다");
    });
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required")).join("edutools.res");
    let manifest = manifest_dir.join("app.manifest");
    let app_icon = tray_dir.join("menubar.on.dark-taskbar.ico");
    let tray_paths = TRAY_FILES.map(|file| tray_dir.join(file));
    let definitions = [
        ("EDUTOOLS_MANIFEST", manifest.as_path()),
        ("EDUTOOLS_APP_ICON", app_icon.as_path()),
        ("EDUTOOLS_TRAY_OFF_LIGHT", tray_paths[0].as_path()),
        ("EDUTOOLS_TRAY_ON_LIGHT", tray_paths[1].as_path()),
        ("EDUTOOLS_TRAY_OFF_DARK", tray_paths[2].as_path()),
        ("EDUTOOLS_TRAY_ON_DARK", tray_paths[3].as_path()),
    ];

    let mut command = Command::new(&rc);
    command
        .arg("/nologo")
        .arg(format!("/fo{}", output.display()));
    for (name, path) in definitions {
        command.arg(resource_definition(name, path));
    }
    command.arg(manifest_dir.join("app.rc"));

    let result = command.output().unwrap_or_else(|error| {
        println!(
            "cargo:warning=rc.exe 실행에 실패했습니다 ({}): {error}",
            rc.display()
        );
        panic!("rc.exe를 실행할 수 없습니다");
    });
    if !result.status.success() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        println!("cargo:warning=rc.exe stdout: {}", stdout.trim());
        println!("cargo:warning=rc.exe stderr: {}", stderr.trim());
        panic!("rc.exe가 EduTools 리소스 컴파일에 실패했습니다");
    }

    println!("cargo:rustc-link-arg-bins={}", output.display());
}

fn resource_definition(name: &str, path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    format!("/d{name}=\\\"{path}\\\"")
}

fn find_resource_compiler() -> Option<PathBuf> {
    if let Some(value) = env::var_os("RC") {
        let path = PathBuf::from(value);
        if path.as_os_str().is_empty() {
            println!("cargo:warning=RC 환경 변수가 비어 있습니다");
            return None;
        }
        return Some(path);
    }

    let program_files = env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
    let kits_bin = program_files.join("Windows Kits/10/bin");
    let mut versions = fs::read_dir(&kits_bin)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("10.0.").then_some((name, entry.path()))
        })
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| version_key(&left.0).cmp(&version_key(&right.0)));
    versions
        .into_iter()
        .rev()
        .map(|(_, directory)| directory.join("x64/rc.exe"))
        .find(|path| path.is_file())
}

fn version_key(value: &str) -> Vec<u32> {
    value
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}
