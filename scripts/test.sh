#!/bin/bash

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRATCH_PATH="$ROOT_DIR/.build"
SWIFTPM_CACHE_PATH="$SCRATCH_PATH/swiftpm-cache"
SWIFTPM_CONFIG_PATH="$SCRATCH_PATH/swiftpm-config"
SWIFTPM_SECURITY_PATH="$SCRATCH_PATH/swiftpm-security"
MODULE_CACHE_PATH="$SCRATCH_PATH/module-cache"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/test.sh (unit|integration|gui|all) [...] [--filter <regex>] [--coverage] [--xunit <path>]
       scripts/test.sh e2e [--app <EduTools.app>] [--filter <regex>] [--xunit <path>]

all runs unit+integration only. gui requires WindowServer. e2e additionally requires Accessibility and Screen Recording.
gui/e2e exit 3 with UNRUNNABLE when their environment is unavailable.
exit codes: 0=pass, 1=failure, 3=UNRUNNABLE, 4=usage error.
USAGE
}

unit_selected=false
integration_selected=false
gui_selected=false
e2e_selected=false
coverage_enabled=false
filter_regex=""
xunit_path=""
e2e_app_path=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        unit)
            unit_selected=true
            shift
            ;;
        integration)
            integration_selected=true
            shift
            ;;
        gui)
            gui_selected=true
            shift
            ;;
        all)
            unit_selected=true
            integration_selected=true
            shift
            ;;
        e2e)
            e2e_selected=true
            shift
            ;;
        --app)
            if [[ $# -lt 2 || -z "$2" ]]; then
                usage
                exit 4
            fi
            e2e_app_path="$2"
            shift 2
            ;;
        --filter)
            if [[ $# -lt 2 || -z "$2" ]]; then
                usage
                exit 4
            fi
            filter_regex="$2"
            shift 2
            ;;
        --coverage)
            coverage_enabled=true
            shift
            ;;
        --xunit)
            if [[ $# -lt 2 || -z "$2" ]]; then
                usage
                exit 4
            fi
            xunit_path="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 4
            ;;
        *)
            echo "EduTools tests: unknown argument '$1'." >&2
            usage
            exit 4
            ;;
    esac
done

if [[ "$unit_selected" == false && "$integration_selected" == false && "$gui_selected" == false && "$e2e_selected" == false ]]; then
    usage
    exit 4
fi

if [[ "$e2e_selected" == true && ( "$unit_selected" == true || "$integration_selected" == true || "$gui_selected" == true ) ]]; then
    echo "EduTools tests: e2e must run by itself so it cannot take over input during another tier." >&2
    exit 4
fi
if [[ -n "$e2e_app_path" && "$e2e_selected" == false ]]; then
    echo "EduTools tests: --app is only valid with e2e." >&2
    exit 4
fi
if [[ "$e2e_selected" == true && "$coverage_enabled" == true ]]; then
    echo "EduTools tests: --coverage is not supported for the external-process e2e tier." >&2
    exit 4
fi

if ! command -v xcode-select >/dev/null 2>&1 || ! command -v xcrun >/dev/null 2>&1; then
    echo "EduTools tests: UNRUNNABLE - Apple developer command-line tools are unavailable." >&2
    exit 3
fi

if ! developer_dir="$(xcode-select -p 2>/dev/null)" || [[ ! -d "$developer_dir" ]]; then
    echo "EduTools tests: UNRUNNABLE - xcode-select did not return an installed developer directory." >&2
    exit 3
fi

if ! swift_bin="$(xcrun --find swift 2>/dev/null)" || [[ ! -x "$swift_bin" ]]; then
    echo "EduTools tests: UNRUNNABLE - Swift could not be located with xcrun." >&2
    exit 3
fi
if ! swiftc_bin="$(xcrun --find swiftc 2>/dev/null)" || [[ ! -x "$swiftc_bin" ]]; then
    echo "EduTools tests: UNRUNNABLE - swiftc could not be located with xcrun." >&2
    exit 3
fi
if ! sdk_path="$(xcrun --show-sdk-path 2>/dev/null)" || [[ ! -d "$sdk_path" ]]; then
    echo "EduTools tests: UNRUNNABLE - the macOS SDK could not be located with xcrun." >&2
    exit 3
fi

swift_test_args=(
    test
    --package-path "$ROOT_DIR"
    --scratch-path "$SCRATCH_PATH"
    --cache-path "$SWIFTPM_CACHE_PATH"
    --config-path "$SWIFTPM_CONFIG_PATH"
    --security-path "$SWIFTPM_SECURITY_PATH"
    --disable-sandbox
)
swift_build_args=(
    build
    --package-path "$ROOT_DIR"
    --scratch-path "$SCRATCH_PATH"
    --cache-path "$SWIFTPM_CACHE_PATH"
    --config-path "$SWIFTPM_CONFIG_PATH"
    --security-path "$SWIFTPM_SECURITY_PATH"
    --disable-sandbox
    --target E2ETests
)

mkdir -p \
    "$SWIFTPM_CACHE_PATH" \
    "$SWIFTPM_CONFIG_PATH" \
    "$SWIFTPM_SECURITY_PATH" \
    "$MODULE_CACHE_PATH"
export CLANG_MODULE_CACHE_PATH="$MODULE_CACHE_PATH"
export SWIFTPM_MODULECACHE_OVERRIDE="$MODULE_CACHE_PATH"

if [[ "$gui_selected" == true ]]; then
    window_server_check="$SCRATCH_PATH/window-server-check"
    "$swiftc_bin" \
        -parse-as-library \
        -sdk "$sdk_path" \
        -module-cache-path "$MODULE_CACHE_PATH" \
        -DWINDOW_SERVER_CHECK_MAIN \
        "$ROOT_DIR/Tests/Support/WindowServer.swift" \
        -framework CoreGraphics \
        -o "$window_server_check"
    window_server_compile_status=$?
    if [[ $window_server_compile_status -ne 0 ]]; then
        echo "EduTools tests: WindowServer preflight could not be compiled or executed." >&2
        exit 1
    fi
    "$window_server_check"
    window_server_status=$?
    if [[ $window_server_status -ne 0 ]]; then
        [[ $window_server_status -eq 3 ]] && exit 3
        echo "EduTools tests: WindowServer preflight could not be executed." >&2
        exit 1
    fi
fi

case "$developer_dir" in
    */CommandLineTools)
        toolchain_kind="CLT"
        framework_dir="$developer_dir/Library/Developer/Frameworks"
        library_dir="$developer_dir/Library/Developer/usr/lib"
        if [[ ! -d "$framework_dir/Testing.framework" || ! -f "$library_dir/lib_TestingInterop.dylib" ]]; then
            echo "EduTools tests: UNRUNNABLE - Swift Testing runtime is missing from the selected CLT." >&2
            exit 3
        fi
        swift_test_args+=(
            -Xswiftc -F"$framework_dir"
            -Xlinker -F"$framework_dir"
            -Xlinker -rpath
            -Xlinker "$framework_dir"
            -Xlinker -rpath
            -Xlinker "$library_dir"
        )
        swift_build_args+=(
            -Xswiftc -F"$framework_dir"
            -Xlinker -F"$framework_dir"
            -Xlinker -rpath
            -Xlinker "$framework_dir"
            -Xlinker -rpath
            -Xlinker "$library_dir"
        )
        ;;
    *)
        toolchain_kind="Xcode"
        ;;
esac

if [[ "$e2e_selected" == true ]]; then
    "$swift_bin" "${swift_build_args[@]}"
    e2e_build_status=$?
    if [[ $e2e_build_status -ne 0 ]]; then
        echo "EduTools tests: E2ETests target did not compile." >&2
        exit 1
    fi

    e2e_environment_check="$SCRATCH_PATH/e2e-environment-check"
    "$swiftc_bin" \
        -parse-as-library \
        -sdk "$sdk_path" \
        -module-cache-path "$MODULE_CACHE_PATH" \
        -DE2E_ENVIRONMENT_CHECK_MAIN \
        "$ROOT_DIR/Tests/Support/WindowServer.swift" \
        "$ROOT_DIR/Tests/Support/TempDir.swift" \
        "$ROOT_DIR/Tests/Support/E2EHarness.swift" \
        -framework AppKit \
        -framework ApplicationServices \
        -framework CoreGraphics \
        -framework ImageIO \
        -o "$e2e_environment_check"
    e2e_environment_compile_status=$?
    if [[ $e2e_environment_compile_status -ne 0 ]]; then
        echo "EduTools tests: E2E environment preflight could not be compiled." >&2
        exit 1
    fi
    if [[ -n "$e2e_app_path" ]]; then
        export EDUTOOLS_E2E_APP="$e2e_app_path"
    fi
    "$e2e_environment_check"
    e2e_environment_status=$?
    if [[ $e2e_environment_status -ne 0 ]]; then
        [[ $e2e_environment_status -eq 3 ]] && exit 3
        echo "EduTools tests: E2E environment preflight could not be executed." >&2
        exit 1
    fi
    export EDUTOOLS_E2E_NO_COUNTDOWN=1
    export EDUTOOLS_E2E_PREFLIGHTED=1
fi

echo "EduTools tests: toolchain=$toolchain_kind developer_dir=$developer_dir swift=$swift_bin"

selected_modules=()
if [[ "$unit_selected" == true ]]; then
    selected_modules+=(UnitTests)
fi
if [[ "$integration_selected" == true ]]; then
    selected_modules+=(IntegrationTests)
fi
if [[ "$gui_selected" == true ]]; then
    selected_modules+=(GUITests)
fi
if [[ "$e2e_selected" == true ]]; then
    selected_modules+=(E2ETests)
fi
target_module_regex="$(IFS='|'; echo "${selected_modules[*]}")"
target_filter="^(${target_module_regex})\."

if [[ -n "$filter_regex" ]]; then
    target_filter="^(?=(${target_module_regex})\\.)(?=.*(${filter_regex})).*"
fi
swift_test_args+=(--filter "$target_filter")

if [[ "$coverage_enabled" == true ]]; then
    swift_test_args+=(--enable-code-coverage)
fi
if [[ -n "$xunit_path" ]]; then
    swift_test_args+=(--xunit-output "$xunit_path")
fi

test_log="$(mktemp -t edutools-tests)"
cleanup_test_log() {
    rm -f "$test_log"
}
trap cleanup_test_log EXIT

"$swift_bin" "${swift_test_args[@]}" 2>&1 | tee "$test_log"
test_status=${PIPESTATUS[0]}
if [[ $test_status -ne 0 ]]; then
    if rg -qi 'operation not permitted|sandbox[^:]*denied|Library not loaded:.*TestingInterop|no such module .Testing.|failed to initialize.*Testing' "$test_log"; then
        echo "EduTools tests: UNRUNNABLE - the selected environment cannot load or execute Swift Testing." >&2
        exit 3
    fi
    exit 1
fi

if [[ "$coverage_enabled" == true ]]; then
    shopt -s nullglob
    profdata_candidates=(
        "$SCRATCH_PATH"/debug/codecov/default.profdata
        "$SCRATCH_PATH"/*/debug/codecov/default.profdata
    )
    coverage_binary_candidates=(
        "$SCRATCH_PATH"/debug/*.xctest/Contents/MacOS/*
        "$SCRATCH_PATH"/*/debug/*.xctest/Contents/MacOS/*
    )
    shopt -u nullglob

    if [[ ${#profdata_candidates[@]} -eq 0 || ${#coverage_binary_candidates[@]} -eq 0 ]]; then
        echo "EduTools tests: coverage data or test executable was not produced." >&2
        exit 1
    fi
    if ! llvm_cov="$(xcrun --find llvm-cov 2>/dev/null)" || [[ ! -x "$llvm_cov" ]]; then
        echo "EduTools tests: UNRUNNABLE - llvm-cov is unavailable in the selected toolchain." >&2
        exit 3
    fi

    "$llvm_cov" report \
        "${coverage_binary_candidates[0]}" \
        -instr-profile="${profdata_candidates[0]}" \
        -ignore-filename-regex='(^|/)(Tests|\.build)/'
    coverage_status=$?
    if [[ $coverage_status -ne 0 ]]; then
        exit 1
    fi
fi

exit 0
