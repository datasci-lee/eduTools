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
    let app_icon = repository_root.join("assets/app-icon/edutools.ico");

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
    println!("cargo:rerun-if-changed={}", app_icon.display());
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
    let package_version = env!("CARGO_PKG_VERSION");
    let file_version = numeric_file_version(package_version);
    resource_script = resource_script
        .replace("{{EDUTOOLS_FILE_VERSION}}", &file_version)
        .replace("{{EDUTOOLS_PRODUCT_VERSION}}", package_version);
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

fn numeric_file_version(version: &str) -> String {
    let numeric = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = numeric.split('.').map(|part| {
        part.parse::<u16>()
            .unwrap_or_else(|_| panic!("Cargo 패키지 버전 숫자 구성요소가 아닙니다: {version}"))
    });
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    assert!(
        parts.next().is_none(),
        "Cargo 패키지 버전 구성요소가 너무 많습니다: {version}"
    );
    format!("{major},{minor},{patch},0")
}

fn generate_icon_table(repository_root: &Path, manifest_dir: &Path) {
    let icons_dir = repository_root.join("assets/icons");
    let attr_dir = repository_root.join("assets/symbols/attr");
    let attr_svg_dir = attr_dir.join("svg");
    let attr_manifest_path = attr_dir.join("manifest.json");
    let symbol_map_path = repository_root.join("assets/symbol-map.tsv");
    let os_map_path = repository_root.join("windows/icon-os.tsv");
    let validator_path = manifest_dir.join("src/icon_assets.rs");
    println!("cargo:rerun-if-changed={}", icons_dir.display());
    println!("cargo:rerun-if-changed={}", attr_manifest_path.display());
    println!("cargo:rerun-if-changed={}", attr_svg_dir.display());
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

    let attr_manifest = fs::read_to_string(&attr_manifest_path).unwrap_or_else(|error| {
        panic!(
            "{} 읽기에 실패했습니다: {error}",
            attr_manifest_path.display()
        )
    });
    let (referenced_attr_layers, attr_tool_count) = attr_manifest_inventory(&attr_manifest)
        .unwrap_or_else(|error| panic!("{}: {error}", attr_manifest_path.display()));
    let mut attr_icons = fs::read_dir(&attr_svg_dir)
        .unwrap_or_else(|error| panic!("{} 읽기에 실패했습니다: {error}", attr_svg_dir.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|value| value == "svg"))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    attr_icons.sort();
    let attr_icon_names = attr_icons
        .iter()
        .map(|path| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .expect("속성 아이콘 파일명은 UTF-8이어야 합니다")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    let missing = referenced_attr_layers
        .difference(&attr_icon_names)
        .cloned()
        .collect::<Vec<_>>();
    let unused = attr_icon_names
        .difference(&referenced_attr_layers)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty() && unused.is_empty(),
        "속성 아이콘 manifest/파일 불일치: missing=[{}], unused=[{}]",
        missing.join(", "),
        unused.join(", ")
    );
    for path in &attr_icons {
        validate_svg_file(path);
    }
    println!(
        "cargo:warning=속성 아이콘 임베드 검증: 레이어 파일 {}개, 도구 {}개",
        attr_icons.len(),
        attr_tool_count
    );

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
    output.push_str("];\npub(crate) static EMBEDDED_ATTR_ICONS: &[(&str, &[u8], &str)] = &[\n");
    for path in attr_icons {
        let key = path
            .file_stem()
            .and_then(|value| value.to_str())
            .expect("속성 아이콘 파일명은 UTF-8이어야 합니다");
        let absolute = fs::canonicalize(&path)
            .unwrap_or_else(|error| panic!("{} 경로 확인 실패: {error}", path.display()));
        output.push_str(&format!(
            "    ({key:?}, include_bytes!({path:?}), {source:?}),\n",
            path = absolute.to_string_lossy(),
            source = path.display().to_string(),
        ));
    }
    let absolute_manifest = fs::canonicalize(&attr_manifest_path)
        .unwrap_or_else(|error| panic!("{} 경로 확인 실패: {error}", attr_manifest_path.display()));
    output.push_str(&format!(
        "];\npub(crate) static ATTR_ICON_MANIFEST: &str = include_str!({:?});\n",
        absolute_manifest.to_string_lossy()
    ));
    fs::write(&output_path, output)
        .unwrap_or_else(|error| panic!("{} 생성에 실패했습니다: {error}", output_path.display()));
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JsonToken {
    String(String),
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
}

fn attr_manifest_inventory(source: &str) -> Result<(BTreeSet<String>, usize), String> {
    let tokens = json_tokens(source)?;
    let tools_key = tokens
        .windows(3)
        .position(|window| {
            matches!(&window[0], JsonToken::String(value) if value == "tools")
                && window[1] == JsonToken::Colon
                && window[2] == JsonToken::LeftBrace
        })
        .ok_or_else(|| "tools 객체가 없습니다".to_owned())?;
    let tools_start = tools_key + 2;
    let tools_end = matching_token(&tokens, tools_start)?;
    let mut layers = BTreeSet::new();
    let mut tool_count = 0usize;
    let mut cursor = tools_start + 1;
    while cursor < tools_end {
        if tokens[cursor] == JsonToken::Comma {
            cursor += 1;
            continue;
        }
        let JsonToken::String(tool_key) = &tokens[cursor] else {
            return Err(format!(
                "tools 객체의 키 위치가 잘못되었습니다: token {cursor}"
            ));
        };
        if !tool_key.starts_with("tool.")
            || tokens.get(cursor + 1) != Some(&JsonToken::Colon)
            || tokens.get(cursor + 2) != Some(&JsonToken::LeftBrace)
        {
            return Err(format!(
                "속성 아이콘 도구 정의가 잘못되었습니다: {tool_key}"
            ));
        }
        let tool_start = cursor + 2;
        let tool_end = matching_token(&tokens, tool_start)?;
        tool_count += 1;
        let layers_key = (tool_start + 1..tool_end)
            .find(|index| {
                matches!(&tokens[*index], JsonToken::String(value) if value == "layers")
                    && tokens.get(*index + 1) == Some(&JsonToken::Colon)
                    && tokens.get(*index + 2) == Some(&JsonToken::LeftBracket)
            })
            .ok_or_else(|| format!("{tool_key}: layers 배열이 없습니다"))?;
        let layers_start = layers_key + 2;
        let layers_end = matching_token(&tokens, layers_start)?;
        let mut layer_cursor = layers_start + 1;
        while layer_cursor < layers_end {
            if tokens[layer_cursor] == JsonToken::Comma {
                layer_cursor += 1;
                continue;
            }
            if tokens[layer_cursor] != JsonToken::LeftBracket {
                return Err(format!("{tool_key}: layer 행이 배열이 아닙니다"));
            }
            let row_end = matching_token(&tokens, layer_cursor)?;
            let template = match tokens.get(layer_cursor + 1) {
                Some(JsonToken::String(value)) => value,
                _ => return Err(format!("{tool_key}: layer 이름이 문자열이 아닙니다")),
            };
            for name in expanded_attr_layer_names(template)? {
                layers.insert(name);
            }
            layer_cursor = row_end + 1;
        }
        cursor = tool_end + 1;
    }
    if tool_count == 0 || layers.is_empty() {
        return Err("도구 또는 레이어가 비어 있습니다".to_owned());
    }
    Ok((layers, tool_count))
}

fn expanded_attr_layer_names(template: &str) -> Result<Vec<String>, String> {
    let widths: &[&str] = if template.contains("{width}") {
        &["1", "2", "3", "4"]
    } else {
        &[""]
    };
    let heads: &[&str] = if template.contains("{heads}") {
        &["single", "double"]
    } else {
        &[""]
    };
    let mut names = Vec::new();
    for width in widths {
        for head in heads {
            let name = template.replace("{width}", width).replace("{heads}", head);
            if name.contains('{') || name.contains('}') {
                return Err(format!("지원하지 않는 layer 자리표시자: {template}"));
            }
            names.push(name);
        }
    }
    Ok(names)
}

fn matching_token(tokens: &[JsonToken], start: usize) -> Result<usize, String> {
    let (open, close) = match tokens.get(start) {
        Some(JsonToken::LeftBrace) => (JsonToken::LeftBrace, JsonToken::RightBrace),
        Some(JsonToken::LeftBracket) => (JsonToken::LeftBracket, JsonToken::RightBracket),
        _ => return Err(format!("token {start}: 여는 구분자가 아닙니다")),
    };
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if *token == open {
            depth += 1;
        } else if *token == close {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| format!("token {index}: 닫는 구분자가 너무 많습니다"))?;
            if depth == 0 {
                return Ok(index);
            }
        }
    }
    Err(format!("token {start}: 닫는 구분자가 없습니다"))
}

fn json_tokens(source: &str) -> Result<Vec<JsonToken>, String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' => tokens.push(JsonToken::LeftBrace),
            b'}' => tokens.push(JsonToken::RightBrace),
            b'[' => tokens.push(JsonToken::LeftBracket),
            b']' => tokens.push(JsonToken::RightBracket),
            b':' => tokens.push(JsonToken::Colon),
            b',' => tokens.push(JsonToken::Comma),
            b'"' => {
                cursor += 1;
                let mut value = String::new();
                while cursor < bytes.len() && bytes[cursor] != b'"' {
                    if bytes[cursor] == b'\\' {
                        cursor += 1;
                        let escaped = *bytes
                            .get(cursor)
                            .ok_or_else(|| "문자열 escape가 닫히지 않았습니다".to_owned())?;
                        value.push(match escaped {
                            b'"' => '"',
                            b'\\' => '\\',
                            b'/' => '/',
                            b'b' => '\u{0008}',
                            b'f' => '\u{000c}',
                            b'n' => '\n',
                            b'r' => '\r',
                            b't' => '\t',
                            _ => return Err("지원하지 않는 JSON 문자열 escape입니다".to_owned()),
                        });
                    } else {
                        let rest = std::str::from_utf8(&bytes[cursor..])
                            .map_err(|error| format!("manifest가 UTF-8이 아닙니다: {error}"))?;
                        let character = rest
                            .chars()
                            .next()
                            .ok_or_else(|| "문자열이 닫히지 않았습니다".to_owned())?;
                        value.push(character);
                        cursor += character.len_utf8() - 1;
                    }
                    cursor += 1;
                }
                if cursor >= bytes.len() {
                    return Err("문자열이 닫히지 않았습니다".to_owned());
                }
                tokens.push(JsonToken::String(value));
            }
            _ => {}
        }
        cursor += 1;
    }
    Ok(tokens)
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
