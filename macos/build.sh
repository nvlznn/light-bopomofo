#!/bin/sh
# Build macos/build/LightBopomofo.app as a universal (arm64 + x86_64) binary.
#
#   macos/build.sh                      ad-hoc signed, for local use
#   SIGN_IDENTITY="Developer ID Application: …" macos/build.sh
#                                       hardened runtime + timestamp, for release
set -eu
. "$(dirname "$0")/common.sh"
IDENTITY="${SIGN_IDENTITY:--}"
WORK="$ROOT/target/macos-app"

rm -rf "$APP" "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$WORK"
for pair in arm64:aarch64-apple-darwin x86_64:x86_64-apple-darwin; do
    arch=${pair%%:*}
    triple=${pair#*:}
    build_rust "$triple"
    swiftc -O -target "$arch-apple-macos$MACOSX_DEPLOYMENT_TARGET" \
        -import-objc-header "$HEADER" \
        -L "$ROOT/target/$triple/release" -llightbopomofo_ffi \
        -framework Cocoa -framework InputMethodKit -framework Carbon \
        "$ROOT"/macos/Sources/*.swift -o "$WORK/LightBopomofo-$arch"
done
lipo -create "$WORK"/LightBopomofo-arm64 "$WORK"/LightBopomofo-x86_64 \
    -output "$APP/Contents/MacOS/LightBopomofo"

cp "$ROOT/macos/Info.plist" "$APP/Contents/"
cp -R "$ROOT/macos/Resources/" "$APP/Contents/Resources/"
cp "$ROOT/data/tsi.csv" "$APP/Contents/Resources/"
mkdir -p "$APP/Contents/Resources/Licenses"
cp "$ROOT/LICENSE" "$APP/Contents/Resources/Licenses/LICENSE.txt"
cp "$ROOT/DATA_LICENSES.md" "$APP/Contents/Resources/Licenses/"
cp "$ROOT/licenses/LGPL-2.1.txt" "$APP/Contents/Resources/Licenses/"

if [ "$IDENTITY" = "-" ]; then
    codesign --force --sign - "$APP"
else
    codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
fi
codesign --verify --strict "$APP"
echo "built $APP ($VERSION, $(lipo -archs "$APP/Contents/MacOS/LightBopomofo"))"
