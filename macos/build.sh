#!/bin/sh
# Build macos/build/LightBopomofo.app — the settings app, with the input
# method inside it — as universal (arm64 + x86_64) binaries.
#
#   macos/build.sh                      ad-hoc signed, for local use
#   SIGN_IDENTITY="Developer ID Application: …" macos/build.sh
#                                       hardened runtime + timestamp, for release
set -eu
. "$(dirname "$0")/common.sh"
IDENTITY="${SIGN_IDENTITY:--}"
WORK="$ROOT/target/macos-app"
FRAMEWORKS="-framework Cocoa -framework Carbon"

rm -rf "$APP" "$WORK"
mkdir -p "$WORK"
for pair in arm64:aarch64-apple-darwin x86_64:x86_64-apple-darwin; do
    arch=${pair%%:*}
    triple=${pair#*:}
    build_rust "$triple"
    common="-O -target $arch-apple-macos$MACOSX_DEPLOYMENT_TARGET -import-objc-header $HEADER -L $ROOT/target/$triple/release -llightbopomofo_ffi $FRAMEWORKS"
    # shellcheck disable=SC2086
    swiftc $common -framework InputMethodKit \
        "$ROOT"/macos/IME/*.swift "$ROOT"/macos/Shared/*.swift -o "$WORK/ime-$arch"
    # shellcheck disable=SC2086
    swiftc $common -parse-as-library -framework SwiftUI \
        "$ROOT"/macos/App/*.swift "$ROOT"/macos/Shared/*.swift -o "$WORK/app-$arch"
done

licenses() {
    mkdir -p "$1/Contents/Resources/Licenses"
    cp "$ROOT/LICENSE" "$1/Contents/Resources/Licenses/LICENSE.txt"
    cp "$ROOT/DATA_LICENSES.md" "$ROOT/licenses/LGPL-2.1.txt" "$1/Contents/Resources/Licenses/"
}
set_version() {
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" \
        -c "Set :CFBundleVersion $BUILD_NUMBER" "$1/Contents/Info.plist"
}

# The input method.
mkdir -p "$IME/Contents/MacOS" "$IME/Contents/Resources"
lipo -create "$WORK/ime-arm64" "$WORK/ime-x86_64" -output "$IME/Contents/MacOS/LightBopomofo"
cp "$ROOT/macos/IME/Info.plist" "$IME/Contents/"
set_version "$IME"
cp -R "$ROOT/macos/IME/Resources/" "$IME/Contents/Resources/"
cp "$ROOT/data/tsi.csv" "$IME/Contents/Resources/"
licenses "$IME"

# The app around it.
mkdir -p "$APP/Contents/MacOS"
lipo -create "$WORK/app-arm64" "$WORK/app-x86_64" -output "$APP/Contents/MacOS/LightBopomofo"
cp "$ROOT/macos/App/Info.plist" "$APP/Contents/"
cp -R "$ROOT/macos/App/Resources/" "$APP/Contents/Resources/"
licenses "$APP"

# Inside out: the outer signature seals the inner bundle.
for bundle in "$IME" "$APP"; do
    if [ "$IDENTITY" = "-" ]; then
        codesign --force --sign - "$bundle"
    else
        codesign --force --options runtime --timestamp --sign "$IDENTITY" "$bundle"
    fi
done
codesign --verify --strict --deep "$APP"
echo "built $APP ($VERSION, $(lipo -archs "$APP/Contents/MacOS/LightBopomofo"))"
