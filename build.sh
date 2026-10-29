#!/bin/bash

set -euo pipefail

APP_NAME="EduTools"
BUNDLE_ID="kr.datasci.edutools"
APP_VERSION="0.1.0"
BUILD_VERSION="1"
MIN_MACOS_VERSION="26.0"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$ROOT_DIR/dist"
BUILD_DIR="$DIST_DIR/build"
MODULE_CACHE_DIR="$DIST_DIR/module-cache"
APP_BUNDLE="$DIST_DIR/$APP_NAME.app"
EXECUTABLE_DIR="$APP_BUNDLE/Contents/MacOS"
RESOURCES_DIR="$APP_BUNDLE/Contents/Resources"
ICON_ASSETS_DIR="$ROOT_DIR/assets/icons"
BUNDLED_ICONS_DIR="$RESOURCES_DIR/Icons"
ATTR_ICON_ASSETS_DIR="$ROOT_DIR/assets/symbols/attr"
BUNDLED_ATTR_ICONS_DIR="$BUNDLED_ICONS_DIR/Attr"
ICON_SOURCE="$BUILD_DIR/IconGenerator.swift"
ICON_GENERATOR="$BUILD_DIR/icon-generator"
ICONSET_DIR="$BUILD_DIR/$APP_NAME.iconset"
GENERATED_ICNS="$BUILD_DIR/$APP_NAME.generated.icns"
ENTITLEMENTS_PATH="$BUILD_DIR/$APP_NAME.entitlements"
DMG_STAGE_DIR="$BUILD_DIR/dmg-root"
DMG_PATH="$DIST_DIR/$APP_NAME-$APP_VERSION.dmg"
SDK_PATH="$(xcrun --show-sdk-path)"
SWIFTC="$(xcrun --find swiftc)"
LIPO="$(xcrun --find lipo)"
CODESIGN="$(xcrun --find codesign)"
ICONUTIL="$(xcrun --find iconutil)"
HDIUTIL="$(xcrun --find hdiutil)"
XCRUN="$(command -v xcrun)"
ARCHITECTURES=(arm64 x86_64)

if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "test" && "$1" != "dmg" ) ]]; then
    echo "usage: ./build.sh [test|dmg]" >&2
    exit 64
fi

COMMAND="${1:-build}"

rm -rf "$BUILD_DIR" "$MODULE_CACHE_DIR" "$APP_BUNDLE"
if [[ "$COMMAND" == "dmg" ]]; then
    rm -f "$DMG_PATH"
fi
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
        -framework Combine \
        -framework CoreAudio \
        -framework CoreGraphics \
        -framework CoreImage \
        -framework CoreMedia \
        -framework CoreVideo \
        -framework IOKit \
        -framework QuartzCore \
        -framework ScreenCaptureKit \
        -framework ServiceManagement \
        -framework SwiftUI \
        -framework UniformTypeIdentifiers \
        -o "$arch_executable"
done

"$LIPO" -create \
    "$BUILD_DIR/$APP_NAME-arm64" \
    "$BUILD_DIR/$APP_NAME-x86_64" \
    -output "$EXECUTABLE_DIR/$APP_NAME"

mkdir -p "$ICONSET_DIR" "$MODULE_CACHE_DIR/icon"
cat > "$ICON_SOURCE" <<'SWIFT'
import AppKit
import Foundation

enum IconGeneratorError: Error {
    case invalidArguments
    case bitmapCreationFailed(Int)
    case pngEncodingFailed(Int)
    case icnsTooLarge
}

guard CommandLine.arguments.count == 3 else {
    throw IconGeneratorError.invalidArguments
}

let outputDirectory = URL(
    fileURLWithPath: CommandLine.arguments[1],
    isDirectory: true
)
let icnsURL = URL(fileURLWithPath: CommandLine.arguments[2])

func point(_ x: CGFloat, _ y: CGFloat, size: CGFloat) -> NSPoint {
    NSPoint(x: x * size, y: y * size)
}

func drawIcon(pixelSize: Int, pointSize: Int) throws -> Data {
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: pixelSize,
        pixelsHigh: pixelSize,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    ), let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
        throw IconGeneratorError.bitmapCreationFailed(pixelSize)
    }

    let size = CGFloat(pointSize)
    bitmap.size = NSSize(width: size, height: size)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.setAllowsAntialiasing(true)
    context.cgContext.setShouldAntialias(true)
    let scale = CGFloat(pixelSize) / size
    context.cgContext.scaleBy(x: scale, y: scale)

    NSColor.clear.setFill()
    NSRect(x: 0, y: 0, width: size, height: size).fill()

    let backgroundRect = NSRect(
        x: size * 0.045,
        y: size * 0.045,
        width: size * 0.91,
        height: size * 0.91
    )
    let background = NSBezierPath(
        roundedRect: backgroundRect,
        xRadius: size * 0.22,
        yRadius: size * 0.22
    )
    let gradient = NSGradient(
        starting: NSColor(calibratedRed: 0.16, green: 0.38, blue: 0.92, alpha: 1),
        ending: NSColor(calibratedRed: 0.38, green: 0.16, blue: 0.72, alpha: 1)
    )
    gradient?.draw(in: background, angle: -48)

    let boardRect = NSRect(
        x: size * 0.18,
        y: size * 0.22,
        width: size * 0.64,
        height: size * 0.57
    )
    let board = NSBezierPath(
        roundedRect: boardRect,
        xRadius: size * 0.065,
        yRadius: size * 0.065
    )
    NSColor(calibratedWhite: 1, alpha: 0.94).setFill()
    board.fill()

    let inkColor = NSColor(calibratedRed: 0.16, green: 0.35, blue: 0.78, alpha: 0.82)
    for y in [CGFloat(0.62), 0.51, 0.40] {
        let line = NSBezierPath()
        line.move(to: point(0.28, y, size: size))
        line.line(to: point(0.63, y, size: size))
        line.lineWidth = max(1, size * 0.035)
        line.lineCapStyle = .round
        inkColor.setStroke()
        line.stroke()
    }

    let pencilOutline = NSBezierPath()
    pencilOutline.move(to: point(0.29, 0.24, size: size))
    pencilOutline.line(to: point(0.76, 0.71, size: size))
    pencilOutline.lineWidth = max(2, size * 0.15)
    pencilOutline.lineCapStyle = .round
    NSColor(calibratedRed: 0.12, green: 0.16, blue: 0.30, alpha: 0.92).setStroke()
    pencilOutline.stroke()

    let pencil = NSBezierPath()
    pencil.move(to: point(0.29, 0.24, size: size))
    pencil.line(to: point(0.76, 0.71, size: size))
    pencil.lineWidth = max(1, size * 0.095)
    pencil.lineCapStyle = .round
    NSColor(calibratedRed: 1, green: 0.72, blue: 0.14, alpha: 1).setStroke()
    pencil.stroke()

    let nib = NSBezierPath()
    nib.move(to: point(0.22, 0.17, size: size))
    nib.line(to: point(0.34, 0.22, size: size))
    nib.line(to: point(0.27, 0.29, size: size))
    nib.close()
    NSColor(calibratedWhite: 0.13, alpha: 1).setFill()
    nib.fill()

    let eraser = NSBezierPath()
    eraser.move(to: point(0.72, 0.75, size: size))
    eraser.line(to: point(0.79, 0.68, size: size))
    eraser.lineWidth = max(1, size * 0.10)
    eraser.lineCapStyle = .round
    NSColor(calibratedRed: 1, green: 0.48, blue: 0.55, alpha: 1).setStroke()
    eraser.stroke()

    NSGraphicsContext.restoreGraphicsState()

    guard let png = bitmap.representation(using: .png, properties: [:]) else {
        throw IconGeneratorError.pngEncodingFailed(pixelSize)
    }
    return png
}

let iconFiles: [(pixels: Int, points: Int, name: String, icnsType: String)] = [
    (16, 16, "icon_16x16.png", "icp4"),
    (32, 16, "icon_16x16@2x.png", "ic11"),
    (32, 32, "icon_32x32.png", "icp5"),
    (64, 32, "icon_32x32@2x.png", "ic12"),
    (128, 128, "icon_128x128.png", "ic07"),
    (256, 128, "icon_128x128@2x.png", "ic13"),
    (256, 256, "icon_256x256.png", "ic08"),
    (512, 256, "icon_256x256@2x.png", "ic14"),
    (512, 512, "icon_512x512.png", "ic09"),
    (1024, 512, "icon_512x512@2x.png", "ic10"),
]

var iconRepresentations: [(type: String, data: Data)] = []
for iconFile in iconFiles {
    let data = try drawIcon(
        pixelSize: iconFile.pixels,
        pointSize: iconFile.points
    )
    try data.write(
        to: outputDirectory.appendingPathComponent(iconFile.name),
        options: .atomic
    )
    iconRepresentations.append((type: iconFile.icnsType, data: data))
}

func bigEndianData(_ value: UInt32) -> Data {
    var encoded = value.bigEndian
    return withUnsafeBytes(of: &encoded) { Data($0) }
}

let totalSize = 8 + iconRepresentations.reduce(0) { partial, representation in
    partial + 8 + representation.data.count
}
guard let totalLength = UInt32(exactly: totalSize) else {
    throw IconGeneratorError.icnsTooLarge
}

var icns = Data("icns".utf8)
icns.append(bigEndianData(totalLength))
for representation in iconRepresentations {
    icns.append(Data(representation.type.utf8))
    icns.append(bigEndianData(UInt32(representation.data.count + 8)))
    icns.append(representation.data)
}
try icns.write(to: icnsURL, options: .atomic)
SWIFT

"$SWIFTC" \
    -O \
    -swift-version 5 \
    -sdk "$SDK_PATH" \
    -module-cache-path "$MODULE_CACHE_DIR/icon" \
    "$ICON_SOURCE" \
    -framework AppKit \
    -o "$ICON_GENERATOR"
"$ICON_GENERATOR" "$ICONSET_DIR" "$GENERATED_ICNS"
if ! "$ICONUTIL" -c icns "$ICONSET_DIR" -o "$RESOURCES_DIR/$APP_NAME.icns"; then
    echo "EduTools: iconutil 변환 실패; Swift 생성기의 표준 ICNS 컨테이너를 사용합니다." >&2
    /bin/cp "$GENERATED_ICNS" "$RESOURCES_DIR/$APP_NAME.icns"
fi
test -s "$RESOURCES_DIR/$APP_NAME.icns"

mkdir -p "$BUNDLED_ICONS_DIR"
icon_asset_count=0
shopt -s nullglob
for icon_asset in "$ICON_ASSETS_DIR"/*.svg "$ICON_ASSETS_DIR"/*.pdf; do
    /bin/cp "$icon_asset" "$BUNDLED_ICONS_DIR/"
    icon_asset_count=$((icon_asset_count + 1))
done
shopt -u nullglob
echo "Copied $icon_asset_count icon asset(s) to $BUNDLED_ICONS_DIR"

mkdir -p "$BUNDLED_ATTR_ICONS_DIR"
attr_icon_asset_count=0
shopt -s nullglob
for attr_icon_asset in "$ATTR_ICON_ASSETS_DIR"/svg/*.svg; do
    /bin/cp "$attr_icon_asset" "$BUNDLED_ATTR_ICONS_DIR/"
    attr_icon_asset_count=$((attr_icon_asset_count + 1))
done
shopt -u nullglob
attr_manifest_copied=0
if [[ -f "$ATTR_ICON_ASSETS_DIR/manifest.json" ]]; then
    /bin/cp "$ATTR_ICON_ASSETS_DIR/manifest.json" "$BUNDLED_ATTR_ICONS_DIR/manifest.json"
    attr_manifest_copied=1
fi
echo "Copied $attr_icon_asset_count attr icon layer(s) and $attr_manifest_copied manifest to $BUNDLED_ATTR_ICONS_DIR"

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
    <key>CFBundleIconFile</key>
    <string>$APP_NAME.icns</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$APP_VERSION</string>
    <key>CFBundleVersion</key>
    <string>$BUILD_VERSION</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>EduTools Settings</string>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
            <key>LSHandlerRank</key>
            <string>Owner</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>kr.datasci.edutools.pansetting</string>
            </array>
        </dict>
    </array>
    <key>UTExportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key>
            <string>kr.datasci.edutools.pansetting</string>
            <key>UTTypeDescription</key>
            <string>EduTools Settings</string>
            <key>UTTypeConformsTo</key>
            <array>
                <string>public.json</string>
                <string>public.data</string>
            </array>
            <key>UTTypeTagSpecification</key>
            <dict>
                <key>public.filename-extension</key>
                <array>
                    <string>pansetting</string>
                </array>
                <key>public.mime-type</key>
                <string>application/vnd.jia.edutools-settings+json</string>
            </dict>
        </dict>
    </array>
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

SIGN_IDENTITY="${EDUTOOLS_SIGN_IDENTITY:-}"
NOTARY_PROFILE="${EDUTOOLS_NOTARY_PROFILE:-}"

if [[ -n "$SIGN_IDENTITY" ]]; then
    cat > "$ENTITLEMENTS_PATH" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.automation.apple-events</key>
    <true/>
</dict>
</plist>
PLIST
    "$CODESIGN" \
        --force \
        --timestamp \
        --options runtime \
        --entitlements "$ENTITLEMENTS_PATH" \
        --sign "$SIGN_IDENTITY" \
        "$APP_BUNDLE"
else
    designated_requirement="designated => identifier \"$BUNDLE_ID\""
    "$CODESIGN" \
        --force \
        --sign - \
        --identifier "$BUNDLE_ID" \
        -r="$designated_requirement" \
        "$APP_BUNDLE"
fi

"$CODESIGN" --verify --strict "$APP_BUNDLE"

echo "Built $APP_BUNDLE"

if [[ "$COMMAND" == "test" ]]; then
    "$EXECUTABLE_DIR/$APP_NAME" --selftest
    "$ROOT_DIR/scripts/test.sh" unit integration
fi

if [[ "$COMMAND" == "dmg" ]]; then
    mkdir -p "$DMG_STAGE_DIR"
    /usr/bin/ditto "$APP_BUNDLE" "$DMG_STAGE_DIR/$APP_NAME.app"
    ln -s /Applications "$DMG_STAGE_DIR/Applications"

    "$HDIUTIL" create \
        -volname "$APP_NAME $APP_VERSION" \
        -srcfolder "$DMG_STAGE_DIR" \
        -ov \
        -format UDZO \
        "$DMG_PATH"

    if [[ -n "$SIGN_IDENTITY" && -n "$NOTARY_PROFILE" ]]; then
        "$XCRUN" notarytool submit \
            "$DMG_PATH" \
            --keychain-profile "$NOTARY_PROFILE" \
            --wait
        "$XCRUN" stapler staple "$DMG_PATH"
        "$XCRUN" stapler validate "$DMG_PATH"
    elif [[ -n "$SIGN_IDENTITY" ]]; then
        echo "경고: EDUTOOLS_NOTARY_PROFILE이 없어 DMG가 공증되지 않았습니다." >&2
    elif [[ -n "$NOTARY_PROFILE" ]]; then
        echo "경고: EDUTOOLS_SIGN_IDENTITY가 없어 ad-hoc 서명했습니다. EDUTOOLS_NOTARY_PROFILE은 사용하지 않았으며 DMG가 공증되지 않았습니다." >&2
    else
        echo "경고: EDUTOOLS_SIGN_IDENTITY가 없어 ad-hoc 서명했으며 DMG가 공증되지 않았습니다." >&2
    fi

    echo "Created $DMG_PATH"
fi

