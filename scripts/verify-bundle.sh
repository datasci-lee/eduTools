#!/bin/bash

set -uo pipefail

usage() {
    echo "usage: scripts/verify-bundle.sh [--app <path>] [--dmg <path>]" >&2
    exit 4
}

PASS_COUNT=0
FAIL_COUNT=0
INFO_COUNT=0

pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    echo "PASS: $1"
}

fail() {
    FAIL_COUNT=$((FAIL_COUNT + 1))
    echo "FAIL: $1"
}

info() {
    INFO_COUNT=$((INFO_COUNT + 1))
    echo "INFO: $1"
}

finish_unrunnable() {
    info "UNRUNNABLE: $1"
    echo "SUMMARY: PASS=$PASS_COUNT FAIL=$FAIL_COUNT INFO=$INFO_COUNT RESULT=UNRUNNABLE"
    exit 3
}

finish() {
    if [[ $FAIL_COUNT -gt 0 ]]; then
        echo "SUMMARY: PASS=$PASS_COUNT FAIL=$FAIL_COUNT INFO=$INFO_COUNT RESULT=FAIL"
        exit 1
    fi
    echo "SUMMARY: PASS=$PASS_COUNT FAIL=$FAIL_COUNT INFO=$INFO_COUNT RESULT=PASS"
    exit 0
}

CALLER_DIR="$PWD"
SCRIPT_DIR="${BASH_SOURCE[0]%/*}"
if [[ "$SCRIPT_DIR" == "${BASH_SOURCE[0]}" ]]; then
    SCRIPT_DIR="."
fi
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)" || finish_unrunnable "저장소 경로를 확인할 수 없습니다"

APP_PATH="$ROOT_DIR/dist/EduTools.app"
DMG_PATH=""
APP_WAS_SET=0
DMG_WAS_SET=0

while [[ $# -gt 0 ]]; do
    case "$1" in
    --app)
        [[ $APP_WAS_SET -eq 0 && $# -ge 2 ]] || usage
        APP_PATH="$2"
        APP_WAS_SET=1
        shift 2
        ;;
    --dmg)
        [[ $DMG_WAS_SET -eq 0 && $# -ge 2 ]] || usage
        DMG_PATH="$2"
        DMG_WAS_SET=1
        shift 2
        ;;
    *)
        usage
        ;;
    esac
done

if [[ "$APP_PATH" != /* ]]; then
    APP_PATH="$CALLER_DIR/$APP_PATH"
fi
if [[ -n "$DMG_PATH" && "$DMG_PATH" != /* ]]; then
    DMG_PATH="$CALLER_DIR/$DMG_PATH"
fi

[[ -d "$APP_PATH" ]] || finish_unrunnable "앱 번들이 없습니다: $APP_PATH"
if [[ -n "$DMG_PATH" && ! -f "$DMG_PATH" ]]; then
    finish_unrunnable "DMG 파일이 없습니다: $DMG_PATH"
fi

missing_tools=()
for tool_name in lipo codesign nm plutil strings; do
    if ! command -v "$tool_name" >/dev/null 2>&1; then
        missing_tools+=("$tool_name")
    fi
done
if [[ -n "$DMG_PATH" ]] && ! command -v hdiutil >/dev/null 2>&1; then
    missing_tools+=("hdiutil")
fi
if [[ ! -x /usr/bin/python3 ]]; then
    missing_tools+=("/usr/bin/python3")
fi
if [[ ${#missing_tools[@]} -gt 0 ]]; then
    finish_unrunnable "필수 도구가 없습니다: ${missing_tools[*]}"
fi

LIPO="$(command -v lipo)"
CODESIGN="$(command -v codesign)"
NM="$(command -v nm)"
PLUTIL="$(command -v plutil)"
STRINGS="$(command -v strings)"
PYTHON=/usr/bin/python3
HDIUTIL=""
if [[ -n "$DMG_PATH" ]]; then
    HDIUTIL="$(command -v hdiutil)"
fi

BUILD_FILE="$ROOT_DIR/build.sh"
SOURCE_ICON_DIR="$ROOT_DIR/assets/icons"
SOURCE_ATTR_DIR="$ROOT_DIR/assets/symbols/attr/svg"
SOURCE_MANIFEST="$ROOT_DIR/assets/symbols/attr/manifest.json"
[[ -f "$BUILD_FILE" ]] || finish_unrunnable "build.sh를 읽을 수 없습니다"
[[ -d "$SOURCE_ICON_DIR" && -d "$SOURCE_ATTR_DIR" && -f "$SOURCE_MANIFEST" ]] \
    || finish_unrunnable "원본 아이콘 자산을 읽을 수 없습니다"

build_values=""
if ! build_values="$($PYTHON - "$BUILD_FILE" <<'PY'
import re
import sys

path = sys.argv[1]
wanted = ("APP_NAME", "BUNDLE_ID", "MIN_MACOS_VERSION")
values = {}
assignment = re.compile(r'^([A-Z_]+)="([^"]*)"$')
with open(path, encoding="utf-8") as handle:
    for raw_line in handle:
        match = assignment.match(raw_line.strip())
        if match and match.group(1) in wanted:
            values[match.group(1)] = match.group(2)
missing = [name for name in wanted if not values.get(name)]
if missing:
    print("missing build value: " + ", ".join(missing), file=sys.stderr)
    raise SystemExit(1)
print("\t".join(values[name] for name in wanted))
PY
)"; then
    finish_unrunnable "build.sh의 번들 기대값을 읽지 못했습니다"
fi
IFS=$'\t' read -r EXPECTED_APP_NAME EXPECTED_BUNDLE_ID EXPECTED_MINIMUM_OS <<< "$build_values"

PLIST_PATH="$APP_PATH/Contents/Info.plist"
BUNDLE_ICON_DIR="$APP_PATH/Contents/Resources/Icons"
BUNDLE_ATTR_DIR="$BUNDLE_ICON_DIR/Attr"
BUNDLE_MANIFEST="$BUNDLE_ATTR_DIR/manifest.json"

[[ -f "$PLIST_PATH" ]] || fail "Contents/Info.plist가 없습니다"

executable_name=""
if [[ -f "$PLIST_PATH" ]]; then
    executable_name="$($PLUTIL -extract CFBundleExecutable raw -o - "$PLIST_PATH" 2>/dev/null || true)"
fi
EXECUTABLE_PATH="$APP_PATH/Contents/MacOS/$executable_name"

if [[ -n "$executable_name" && "$executable_name" != */* && -f "$EXECUTABLE_PATH" ]]; then
    arch_output=""
    if arch_output="$($LIPO -archs "$EXECUTABLE_PATH" 2>&1)"; then
        missing_arches=()
        case " $arch_output " in *" arm64 "*) ;; *) missing_arches+=("arm64") ;; esac
        case " $arch_output " in *" x86_64 "*) ;; *) missing_arches+=("x86_64") ;; esac
        if [[ ${#missing_arches[@]} -eq 0 ]]; then
            pass "실행 파일 아키텍처에 arm64와 x86_64가 모두 있습니다 ($arch_output)"
        else
            fail "실행 파일 아키텍처가 부족합니다: ${missing_arches[*]} (현재: $arch_output)"
        fi
    else
        arch_output="${arch_output//$'\n'/; }"
        fail "실행 파일 아키텍처를 읽지 못했습니다: $arch_output"
    fi
else
    fail "CFBundleExecutable에 해당하는 실행 파일이 Contents/MacOS에 없습니다"
fi

signature_output=""
if signature_output="$($CODESIGN --verify --deep --strict "$APP_PATH" 2>&1)"; then
    pass "codesign --verify --deep --strict 검증을 통과했습니다"
    signature_details="$($CODESIGN -dv --verbose=4 "$APP_PATH" 2>&1 || true)"
    if [[ "$signature_details" == *"Signature=adhoc"* ]]; then
        info "ad-hoc 서명(공증 전)"
    else
        team_identifier=""
        while IFS= read -r signature_line; do
            case "$signature_line" in
            TeamIdentifier=*) team_identifier="${signature_line#TeamIdentifier=}" ;;
            esac
        done <<< "$signature_details"
        if [[ -n "$team_identifier" && "$team_identifier" != "not set" ]]; then
            info "Developer ID 서명 팀 ID: $team_identifier"
        else
            info "서명은 유효하지만 TeamIdentifier를 확인하지 못했습니다"
        fi
    fi
else
    signature_output="${signature_output//$'\n'/; }"
    fail "코드 서명 검증에 실패했습니다: $signature_output"
fi

if [[ -f "$PLIST_PATH" ]]; then
    plist_lint_output=""
    if plist_lint_output="$($PLUTIL -lint "$PLIST_PATH" 2>&1)"; then
        pass "Info.plist 문법이 유효합니다"
    else
        plist_lint_output="${plist_lint_output//$'\n'/; }"
        fail "Info.plist 문법이 유효하지 않습니다: $plist_lint_output"
    fi

    actual_bundle_id="$($PLUTIL -extract CFBundleIdentifier raw -o - "$PLIST_PATH" 2>/dev/null || true)"
    if [[ "$actual_bundle_id" == "$EXPECTED_BUNDLE_ID" ]]; then
        pass "CFBundleIdentifier가 build.sh와 일치합니다 ($actual_bundle_id)"
    else
        fail "CFBundleIdentifier가 build.sh와 다릅니다 (기대: $EXPECTED_BUNDLE_ID, 실제: ${actual_bundle_id:-없음})"
    fi

    if [[ -n "$executable_name" && "$executable_name" != */* \
          && -f "$EXECUTABLE_PATH" && -x "$EXECUTABLE_PATH" ]]; then
        pass "CFBundleExecutable이 Contents/MacOS에 있고 실행 가능합니다 ($executable_name)"
    else
        fail "CFBundleExecutable이 없거나 실행 가능하지 않습니다 (${executable_name:-없음})"
    fi

    lsui_element="$($PLUTIL -extract LSUIElement raw -o - "$PLIST_PATH" 2>/dev/null || true)"
    if [[ "$lsui_element" == "true" ]]; then
        pass "LSUIElement가 true입니다"
    else
        fail "LSUIElement가 true가 아닙니다 (실제: ${lsui_element:-없음})"
    fi

    minimum_os="$($PLUTIL -extract LSMinimumSystemVersion raw -o - "$PLIST_PATH" 2>/dev/null || true)"
    minimum_os_major="${minimum_os%%.*}"
    expected_minimum_major="${EXPECTED_MINIMUM_OS%%.*}"
    if [[ "$minimum_os_major" =~ ^[0-9]+$ && "$expected_minimum_major" =~ ^[0-9]+$ \
          && $minimum_os_major -ge $expected_minimum_major ]]; then
        pass "LSMinimumSystemVersion의 주 버전이 build.sh 기준 이상입니다 ($minimum_os)"
    else
        fail "LSMinimumSystemVersion이 build.sh 기준보다 낮거나 유효하지 않습니다 (기대 주 버전 >= $expected_minimum_major, 실제: ${minimum_os:-없음})"
    fi

    short_version="$($PLUTIL -extract CFBundleShortVersionString raw -o - "$PLIST_PATH" 2>/dev/null || true)"
    if [[ -n "$short_version" ]]; then
        pass "CFBundleShortVersionString이 있습니다 ($short_version)"
    else
        fail "CFBundleShortVersionString이 없습니다"
    fi
fi

resource_counts=""
if resource_counts="$($PYTHON - "$SOURCE_ICON_DIR" "$BUNDLE_ICON_DIR" "$SOURCE_ATTR_DIR" "$BUNDLE_ATTR_DIR" <<'PY'
from pathlib import Path
import sys

def count(directory, suffixes):
    path = Path(directory)
    if not path.is_dir():
        raise FileNotFoundError(str(path))
    return sum(
        1 for item in path.iterdir()
        if item.is_file() and item.suffix.lower() in suffixes
    )

values = (
    count(sys.argv[1], {".svg", ".pdf"}),
    count(sys.argv[2], {".svg", ".pdf"}),
    count(sys.argv[3], {".svg"}),
    count(sys.argv[4], {".svg"}),
)
print("\t".join(map(str, values)))
PY
)"; then
    IFS=$'\t' read -r SOURCE_ICON_COUNT BUNDLE_ICON_COUNT SOURCE_ATTR_COUNT BUNDLE_ATTR_COUNT <<< "$resource_counts"
    if [[ $SOURCE_ICON_COUNT -gt 0 && $SOURCE_ICON_COUNT -eq $BUNDLE_ICON_COUNT ]]; then
        pass "일반 아이콘 개수가 원본과 같습니다 ($BUNDLE_ICON_COUNT)"
    else
        fail "일반 아이콘 개수가 원본과 다릅니다 (원본: $SOURCE_ICON_COUNT, 번들: $BUNDLE_ICON_COUNT)"
    fi
    if [[ $SOURCE_ATTR_COUNT -gt 0 && $SOURCE_ATTR_COUNT -eq $BUNDLE_ATTR_COUNT ]]; then
        pass "속성 레이어 SVG 개수가 원본과 같습니다 ($BUNDLE_ATTR_COUNT)"
    else
        fail "속성 레이어 SVG 개수가 원본과 다릅니다 (원본: $SOURCE_ATTR_COUNT, 번들: $BUNDLE_ATTR_COUNT)"
    fi
else
    fail "원본 또는 번들 리소스 디렉터리를 읽지 못했습니다"
fi

manifest_result=""
if manifest_result="$($PYTHON - "$SOURCE_MANIFEST" "$BUNDLE_MANIFEST" <<'PY'
import json
from pathlib import Path
import sys

source = Path(sys.argv[1])
bundled = Path(sys.argv[2])
if not bundled.is_file():
    print("번들 manifest.json 없음")
    raise SystemExit(1)
try:
    json.loads(source.read_text(encoding="utf-8"))
    json.loads(bundled.read_text(encoding="utf-8"))
except (OSError, UnicodeError, json.JSONDecodeError) as error:
    print(f"JSON 오류: {error}")
    raise SystemExit(1)
if source.read_bytes() != bundled.read_bytes():
    print("번들 파일이 원본과 바이트 단위로 다름")
    raise SystemExit(1)
print("유효한 JSON이며 원본과 동일")
PY
)"; then
    pass "속성 manifest.json이 존재하고 $manifest_result"
else
    manifest_result="${manifest_result//$'\n'/; }"
    fail "속성 manifest.json 검증에 실패했습니다: $manifest_result"
fi

layer_audit_result=""
if layer_audit_result="$($PYTHON - "$BUNDLE_MANIFEST" "$BUNDLE_ATTR_DIR" <<'PY'
import json
import re
from pathlib import Path
import sys

manifest_path = Path(sys.argv[1])
layer_dir = Path(sys.argv[2])
try:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
except (OSError, UnicodeError, json.JSONDecodeError) as error:
    print(f"매니페스트를 읽지 못함: {error}")
    raise SystemExit(1)

width_levels = manifest.get("widthLevels")
tools = manifest.get("tools")
if not isinstance(width_levels, dict) or not width_levels or not isinstance(tools, dict):
    print("widthLevels 또는 tools 형식이 유효하지 않음")
    raise SystemExit(1)

substitutions = {
    "width": sorted(width_levels.keys()),
    "heads": ["single", "double"],
}
placeholder = re.compile(r"\{([^{}]+)\}")

def expand(template):
    results = [template]
    while True:
        unresolved = next((value for value in results if "{" in value or "}" in value), None)
        if unresolved is None:
            return results
        match = placeholder.search(unresolved)
        if match is None:
            raise ValueError(f"잘못된 자리표시자: {template}")
        name = match.group(1)
        values = substitutions.get(name)
        if not values:
            raise ValueError(f"알 수 없는 자리표시자 {{{name}}}: {template}")
        index = results.index(unresolved)
        results[index:index + 1] = [
            unresolved[:match.start()] + value + unresolved[match.end():]
            for value in values
        ]

expected = set()
try:
    for tool_name, tool in tools.items():
        layers = tool.get("layers") if isinstance(tool, dict) else None
        if not isinstance(layers, list):
            raise ValueError(f"{tool_name}의 layers 형식이 유효하지 않음")
        for layer in layers:
            if not isinstance(layer, list) or len(layer) not in (2, 3) or not isinstance(layer[0], str):
                raise ValueError(f"{tool_name}에 잘못된 레이어 정의가 있음")
            expected.update(expand(layer[0]))
except ValueError as error:
    print(error)
    raise SystemExit(1)

try:
    available = {
        path.stem for path in layer_dir.iterdir()
        if path.is_file() and path.suffix.lower() == ".svg"
    }
except OSError as error:
    print(f"레이어 디렉터리를 읽지 못함: {error}")
    raise SystemExit(1)

missing = sorted(expected - available)
unused = sorted(available - expected)
if missing or unused:
    details = []
    if missing:
        details.append("누락=" + ",".join(missing))
    if unused:
        details.append("미사용=" + ",".join(unused))
    print("; ".join(details))
    raise SystemExit(1)
print(f"기대 레이어 {len(expected)}개와 번들 SVG가 정확히 일치")
PY
)"; then
    pass "속성 레이어 정합성이 맞습니다 ($layer_audit_result)"
else
    layer_audit_result="${layer_audit_result//$'\n'/; }"
    fail "속성 레이어 정합성이 맞지 않습니다: $layer_audit_result"
fi

check_selftest_code() {
    local executable_path="$1"
    local nm_output=""
    local strings_output=""

    if ! nm_output="$($NM "$executable_path" 2>&1)"; then
        info "심볼을 읽을 수 없어 시험 코드 부재를 판정할 수 없음"
        return
    fi

    # runSelfTests와 무관한 앱 심볼을 양성 대조로 삼아 nm이 앱 심볼을 읽었는지 먼저 확인한다.
    if [[ "$nm_output" != *"AppDelegate"* ]]; then
        info "심볼을 읽을 수 없어 시험 코드 부재를 판정할 수 없음"
        return
    fi

    if ! strings_output="$($STRINGS "$executable_path" 2>/dev/null)"; then
        if [[ "$nm_output" == *"runSelfTests"* ]]; then
            info "자체 시험 코드 포함(이관 전) — S1b/c 완료 후 FAIL 로 승격"
        else
            info "실행 파일 문자열을 읽을 수 없어 시험 코드 부재를 판정할 수 없음"
        fi
        return
    fi

    if [[ "$nm_output" == *"runSelfTests"* || "$strings_output" == *"SelfTest"* ]]; then
        # S1b/c가 시험 코드를 번들 밖으로 이관하면 이 INFO를 FAIL로 승격한다.
        info "자체 시험 코드 포함(이관 전) — S1b/c 완료 후 FAIL 로 승격"
    else
        pass "실행 파일에 자체 시험 코드가 없습니다"
    fi
}

if [[ -f "$EXECUTABLE_PATH" ]]; then
    check_selftest_code "$EXECUTABLE_PATH"
fi

# 실제 설정과 실행 중 앱을 격리할 이음새가 없으므로 여기서는 앱을 실행하지 않는다. E2E S4가 담당한다.
info "스모크 실행은 생략했습니다(E2E S4 담당)"

MOUNT_POINT=""
MOUNT_ATTACHED=0
cleanup_mount() {
    if [[ $MOUNT_ATTACHED -eq 1 && -n "$MOUNT_POINT" ]]; then
        if ! "$HDIUTIL" detach "$MOUNT_POINT" >/dev/null 2>&1; then
            info "일반 DMG 분리에 실패해 강제 분리를 시도합니다"
            if ! "$HDIUTIL" detach -force "$MOUNT_POINT" >/dev/null 2>&1; then
                fail "트랩에서 DMG를 분리하지 못했습니다: $MOUNT_POINT"
            fi
        fi
        MOUNT_ATTACHED=0
    fi
    if [[ -n "$MOUNT_POINT" && -d "$MOUNT_POINT" ]]; then
        if ! /bin/rmdir "$MOUNT_POINT" >/dev/null 2>&1; then
            info "DMG 임시 마운트 디렉터리를 제거하지 못했습니다: $MOUNT_POINT"
        fi
    fi
}

if [[ -n "$DMG_PATH" ]]; then
    if ! MOUNT_POINT="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/edutools-verify.XXXXXX")"; then
        finish_unrunnable "DMG 마운트 임시 디렉터리를 만들지 못했습니다"
    fi
    trap cleanup_mount EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    attach_output=""
    # attach 도중 중단되어도 EXIT 트랩이 이 마운트 지점의 분리를 시도하게 한다.
    MOUNT_ATTACHED=1
    if attach_output="$($HDIUTIL attach -nobrowse -readonly -mountpoint "$MOUNT_POINT" "$DMG_PATH" 2>&1)"; then
        pass "DMG를 읽기 전용으로 마운트했습니다"
        MOUNTED_APP="$MOUNT_POINT/$EXPECTED_APP_NAME.app"
        if [[ -d "$MOUNTED_APP" ]]; then
            pass "DMG에 $EXPECTED_APP_NAME.app이 있습니다"
            mounted_plist="$MOUNTED_APP/Contents/Info.plist"
            mounted_bundle_id="$($PLUTIL -extract CFBundleIdentifier raw -o - "$mounted_plist" 2>/dev/null || true)"
            if [[ "$mounted_bundle_id" == "$EXPECTED_BUNDLE_ID" ]]; then
                pass "DMG 앱의 CFBundleIdentifier가 build.sh와 일치합니다 ($mounted_bundle_id)"
            else
                fail "DMG 앱의 CFBundleIdentifier가 다릅니다 (기대: $EXPECTED_BUNDLE_ID, 실제: ${mounted_bundle_id:-없음})"
            fi
        else
            fail "DMG에 $EXPECTED_APP_NAME.app이 없습니다"
        fi
        if [[ -L "$MOUNT_POINT/Applications" ]]; then
            info "DMG에 Applications 심볼릭 링크가 있습니다"
        else
            info "DMG에 Applications 심볼릭 링크가 없습니다"
        fi

        detach_output=""
        if detach_output="$($HDIUTIL detach "$MOUNT_POINT" 2>&1)"; then
            MOUNT_ATTACHED=0
            pass "DMG를 분리했습니다"
        else
            detach_output="${detach_output//$'\n'/; }"
            fail "DMG 분리에 실패했습니다: $detach_output"
        fi
    else
        MOUNT_ATTACHED=0
        attach_output="${attach_output//$'\n'/; }"
        fail "DMG 마운트에 실패했습니다: $attach_output"
    fi

    cleanup_mount
    trap - EXIT HUP INT TERM
    MOUNT_POINT=""
fi

finish
