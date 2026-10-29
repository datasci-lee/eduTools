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
usage: scripts/test.sh (unit|integration|all) [...] [--filter <regex>] [--coverage] [--xunit <path>]
       scripts/test.sh gui|e2e

gui/e2e targets are not available yet (planned for S3/S4).
USAGE
}

unit_selected=false
integration_selected=false
coverage_enabled=false
filter_regex=""
xunit_path=""

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
        all)
            unit_selected=true
            integration_selected=true
            shift
            ;;
        gui|e2e)
            echo "EduTools tests: '$1' target is not available yet (gui=S3, e2e=S4)." >&2
            exit 4
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

if [[ "$unit_selected" == false && "$integration_selected" == false ]]; then
    usage
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

swift_test_args=(
    test
    --package-path "$ROOT_DIR"
    --scratch-path "$SCRATCH_PATH"
    --cache-path "$SWIFTPM_CACHE_PATH"
    --config-path "$SWIFTPM_CONFIG_PATH"
    --security-path "$SWIFTPM_SECURITY_PATH"
    --disable-sandbox
)

mkdir -p \
    "$SWIFTPM_CACHE_PATH" \
    "$SWIFTPM_CONFIG_PATH" \
    "$SWIFTPM_SECURITY_PATH" \
    "$MODULE_CACHE_PATH"
export CLANG_MODULE_CACHE_PATH="$MODULE_CACHE_PATH"
export SWIFTPM_MODULECACHE_OVERRIDE="$MODULE_CACHE_PATH"

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
        ;;
    *)
        toolchain_kind="Xcode"
        ;;
esac

echo "EduTools tests: toolchain=$toolchain_kind developer_dir=$developer_dir swift=$swift_bin"

if [[ "$unit_selected" == true && "$integration_selected" == true ]]; then
    target_module_regex='(UnitTests|IntegrationTests)'
    target_filter='^(UnitTests|IntegrationTests)\.'
elif [[ "$unit_selected" == true ]]; then
    target_module_regex='UnitTests'
    target_filter='^UnitTests\.'
else
    target_module_regex='IntegrationTests'
    target_filter='^IntegrationTests\.'
fi

if [[ -n "$filter_regex" ]]; then
    target_filter="^(?=${target_module_regex}\\.)(?=.*(${filter_regex})).*"
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
