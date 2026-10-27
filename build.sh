#!/bin/bash

set -euo pipefail

APP_NAME="EduTools"
BUNDLE_ID="com.jia.edutools"
MARKETING_VERSION="0.1.0"
BUILD_VERSION="1"
MIN_MACOS_VERSION="26.0"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$ROOT_DIR/dist"
BUILD_DIR="$DIST_DIR/build"
MODULE_CACHE_DIR="$DIST_DIR/module-cache"
APP_BUNDLE="$DIST_DIR/$APP_NAME.app"
EXECUTABLE_DIR="$APP_BUNDLE/Contents/MacOS"
RESOURCES_DIR="$APP_BUNDLE/Contents/Resources"
SDK_PATH="$(xcrun --show-sdk-path)"
SWIFTC="$(xcrun --find swiftc)"
LIPO="$(xcrun --find lipo)"
CODESIGN="$(xcrun --find codesign)"
ARCHITECTURES=(arm64 x86_64)

if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "test" ) ]]; then
    echo "usage: ./build.sh [test]" >&2
    exit 64
fi

rm -rf "$BUILD_DIR" "$MODULE_CACHE_DIR" "$APP_BUNDLE"
mkdir -p "$BUILD_DIR" "$MODULE_CACHE_DIR" "$EXECUTABLE_DIR" "$RESOURCES_DIR"

for arch in "${ARCHITECTURES[@]}"; do
    arch_module_cache="$MODULE_CACHE_DIR/$arch"
    arch_executable="$BUILD_DIR/$APP_NAME-$arch"
    mkdir -p "$arch_module_cache"

    "$SWIFTC" \
        -O \
        -swift-version 5 \
        -target "$arch-apple-macosx$MIN_MACOS_VERSION" \
        -sdk "$SDK_PATH" \
        -module-cache-path "$arch_module_cache" \
        "$ROOT_DIR"/Sources/EduTools/*.swift \
        -framework AppKit \
        -framework ApplicationServices \
        -framework Carbon \
        -framework ColorSync \
        -framework CoreAudio \
        -framework CoreGraphics \
        -framework CoreImage \
        -framework CoreMedia \
        -framework CoreVideo \
        -framework QuartzCore \
        -framework ScreenCaptureKit \
        -o "$arch_executable"
done

"$LIPO" -create \
    "$BUILD_DIR/$APP_NAME-arm64" \
    "$BUILD_DIR/$APP_NAME-x86_64" \
    -output "$EXECUTABLE_DIR/$APP_NAME"

cat > "$APP_BUNDLE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>ko</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$MARKETING_VERSION</string>
    <key>CFBundleVersion</key>
    <string>$BUILD_VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>$MIN_MACOS_VERSION</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSScreenCaptureUsageDescription</key>
    <string>현재 화면 배경과 판서를 함께 PNG로 저장하는 데 사용합니다.</string>
    <key>NSAppleEventsUsageDescription</key>
    <string>쉬는 시간이 끝나면 EduTools가 연 Google Chrome 음악 탭만 닫는 데 사용합니다.</string>
</dict>
</plist>
PLIST

designated_requirement="designated => identifier \"$BUNDLE_ID\""
"$CODESIGN" \
    --force \
    --sign - \
    --identifier "$BUNDLE_ID" \
    -r="$designated_requirement" \
    "$APP_BUNDLE"

echo "Built $APP_BUNDLE"

if [[ "${1:-}" == "test" ]]; then
    "$EXECUTABLE_DIR/$APP_NAME" --selftest
fi

