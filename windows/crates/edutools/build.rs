use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/icon_assets.rs"]
mod icon_assets;

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

    generate_icon_table(&repository_root, &manifest_dir);

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
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let output = out_dir.join("edutools.res");
    let generated_rc = out_dir.join("app.generated.rc");
    let template_path = manifest_dir.join("app.rc");
    let manifest = manifest_dir.join("app.manifest");
    let app_icon = tray_dir.join("menubar.on.dark-taskbar.ico");
    let tray_paths = TRAY_FILES.map(|file| tray_dir.join(file));
    let resources = [
        ("{{EDUTOOLS_MANIFEST}}", manifest.as_path()),
        ("{{EDUTOOLS_APP_ICON}}", app_icon.as_path()),
        ("{{EDUTOOLS_TRAY_OFF_LIGHT}}", tray_paths[0].as_path()),
        ("{{EDUTOOLS_TRAY_ON_LIGHT}}", tray_paths[1].as_path()),
        ("{{EDUTOOLS_TRAY_OFF_DARK}}", tray_paths[2].as_path()),
        ("{{EDUTOOLS_TRAY_ON_DARK}}", tray_paths[3].as_path()),
    ];
    let mut resource_script = fs::read_to_string(&template_path)
        .unwrap_or_else(|error| panic!("{} 읽기에 실패했습니다: {error}", template_path.display()));
    for (placeholder, path) in resources {
        resource_script = resource_script.replace(placeholder, &resource_path_literal(path));
    }
    fs::write(&generated_rc, resource_script)
        .unwrap_or_else(|error| panic!("{} 생성에 실패했습니다: {error}", generated_rc.display()));

    let mut command = Command::new(&rc);
    command
        .arg("/nologo")
        .arg(format!("/fo{}", output.display()))
        .arg(&generated_rc);

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

fn generate_icon_table(repository_root: &Path, manifest_dir: &Path) {
    let icons_dir = repository_root.join("assets/icons");
    let symbol_map_path = repository_root.join("assets/symbol-map.tsv");
    let os_map_path = repository_root.join("windows/icon-os.tsv");
    let validator_path = manifest_dir.join("src/icon_assets.rs");
    println!("cargo:rerun-if-changed={}", icons_dir.display());
    println!("cargo:rerun-if-changed={}", symbol_map_path.display());
    println!("cargo:rerun-if-changed={}", os_map_path.display());
    println!("cargo:rerun-if-changed={}", validator_path.display());

    let symbol_map = read_tsv(&symbol_map_path);
    let known_keys = symbol_map
        .iter()
        .map(|(_, app_key)| app_key.clone())
        .collect::<BTreeSet<_>>();
    let os_overrides = read_tsv(&os_map_path)
        .into_iter()
        .collect::<BTreeMap<_, _>>();

    let mut icons = fs::read_dir(&icons_dir)
        .unwrap_or_else(|error| panic!("{} 읽기에 실패했습니다: {error}", icons_dir.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|value| value == "svg"))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    icons.sort();
    assert_eq!(
        icons.len(),
        57,
        "assets/icons에는 앱 SVG 57개가 있어야 합니다"
    );

    let mut generated_icons = Vec::with_capacity(icons.len());
    let icon_keys = icons
        .iter()
        .map(|path| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .expect("아이콘 파일명은 UTF-8이어야 합니다")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    for path in icons {
        let key = path
            .file_stem()
            .and_then(|value| value.to_str())
            .expect("아이콘 파일명은 UTF-8이어야 합니다")
            .to_owned();
        let regular_key = key.strip_suffix(".selected").unwrap_or(&key);
        assert!(
            known_keys.contains(regular_key),
            "알 수 없는 앱 아이콘 파일명: {}",
            path.display()
        );
        if key.ends_with(".selected") {
            assert!(
                icon_keys.contains(regular_key),
                "일반 아이콘 짝이 없는 selected 아이콘: {}",
                path.display()
            );
        }

        validate_svg_file(&path);

        let (source_path, source_name, is_os_override) =
            if let Some(source) = os_overrides.get(&key) {
                let source_path = repository_root
                    .join("assets/symbols/svg")
                    .join(format!("{source}.svg"));
                assert!(
                    source_path.is_file(),
                    "OS 아이콘 예외 원본이 없습니다: {}",
                    source_path.display()
                );
                (source_path, source.clone(), true)
            } else {
                (path.clone(), key.clone(), false)
            };
        if is_os_override {
            validate_svg_file(&source_path);
        }
        generated_icons.push((key, source_name, source_path, is_os_override));
    }

    for key in os_overrides.keys() {
        assert!(
            icon_keys.contains(key),
            "icon-os.tsv 키가 앱 SVG 키에 없습니다: {key}"
        );
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let output_path = out_dir.join("icons_embed.rs");
    let mut output =
        String::from("pub(crate) static EMBEDDED_ICONS: &[(&str, &[u8], &str, bool)] = &[\n");
    for (key, source, path, is_os_override) in generated_icons {
        let absolute = fs::canonicalize(&path)
            .unwrap_or_else(|error| panic!("{} 경로 확인 실패: {error}", path.display()));
        output.push_str(&format!(
            "    ({key:?}, include_bytes!({path:?}), {source:?}, {is_os_override}),\n",
            path = absolute.to_string_lossy(),
        ));
    }
    output.push_str(
        "];
pub(crate) static SYMBOL_MAP: &[(&str, &str)] = &[\n",
    );
    for (design_name, app_key) in &symbol_map {
        output.push_str(&format!("    ({design_name:?}, {app_key:?}),\n"));
    }
    output.push_str(
        "];
pub(crate) static OS_ICON_OVERRIDES: &[(&str, &str)] = &[\n",
    );
    for (app_key, source) in &os_overrides {
        output.push_str(&format!("    ({app_key:?}, {source:?}),\n"));
    }
    output.push_str("];\n");
    fs::write(&output_path, output)
        .unwrap_or_else(|error| panic!("{} 생성에 실패했습니다: {error}", output_path.display()));
}

fn read_tsv(path: &Path) -> Vec<(String, String)> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} 읽기에 실패했습니다: {error}", path.display()));
    source
        .lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (left, right) = line.split_once('\t').unwrap_or_else(|| {
                panic!(
                    "{}:{}: 탭으로 나뉜 두 열이 필요합니다",
                    path.display(),
                    index + 1
                )
            });
            assert!(
                !left.trim().is_empty() && !right.trim().is_empty() && !right.contains('\t'),
                "{}:{}: 비어 있지 않은 두 열만 허용합니다",
                path.display(),
                index + 1
            );
            Some((left.trim().to_owned(), right.trim().to_owned()))
        })
        .collect()
}

fn validate_svg_file(path: &Path) {
    let bytes = fs::read(path)
        .unwrap_or_else(|error| panic!("{} 읽기에 실패했습니다: {error}", path.display()));
    icon_assets::validate_svg(&path.display().to_string(), &bytes)
        .unwrap_or_else(|error| panic!("{error}"));
}

fn resource_path_literal(path: &Path) -> String {
    assert!(path.is_absolute(), "리소스 경로는 절대 경로여야 합니다");
    let path = path.to_string_lossy().replace('\\', "/");
    assert!(
        !path.contains('"'),
        "리소스 경로에 큰따옴표를 사용할 수 없습니다"
    );
    format!("\"{path}\"")
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
