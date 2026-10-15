#!/bin/sh
# Put macos/build/LightBopomofo.app into dist/LightBopomofo.dmg next to a
# link to /Applications, for drag-and-drop install.
#
#   SIGN_IDENTITY="Developer ID Application: …" macos/package-dmg.sh   signed
#   macos/package-dmg.sh                                               unsigned (testing)
set -eu
. "$(dirname "$0")/common.sh"
WORK="$ROOT/target/macos-dmg"
OUT="$ROOT/dist/LightBopomofo.dmg"
[ -d "$APP" ] || { echo "run macos/build.sh first"; exit 1; }

rm -rf "$WORK" "$OUT"
mkdir -p "$WORK/stage" "$(dirname "$OUT")"
cp -R "$APP" "$WORK/stage/"
ln -s /Applications "$WORK/stage/Applications"
hdiutil create -quiet -volname "輕注音" -srcfolder "$WORK/stage" \
    -fs HFS+ -format UDZO -ov "$OUT"
if [ -n "${SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$SIGN_IDENTITY" "$OUT"
fi
echo "packaged $OUT"
