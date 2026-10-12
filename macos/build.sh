#!/bin/sh
# Build macos/build/LightBopomofo.app (arm64, ad-hoc signed).
set -eu
. "$(dirname "$0")/common.sh"
APP="$ROOT/macos/build/LightBopomofo.app"

build_rust
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# shellcheck disable=SC2086
swiftc -O $SWIFT_FLAGS -framework Cocoa -framework InputMethodKit -framework Carbon \
    "$ROOT"/macos/Sources/*.swift -o "$APP/Contents/MacOS/LightBopomofo"
cp "$ROOT/macos/Info.plist" "$APP/Contents/"
cp -R "$ROOT/macos/Resources/" "$APP/Contents/Resources/"
cp "$ROOT/data/tsi.csv" "$APP/Contents/Resources/"
codesign --force --sign - "$APP"
echo "built $APP"
