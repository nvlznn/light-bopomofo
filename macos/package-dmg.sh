#!/bin/sh
# Put macos/build/輕注音.app into dist/LightBopomofo.dmg: the app on the
# left, an 應用程式 (/Applications) link on the right, over a colourful
# background with an arrow between them.
#
#   SIGN_IDENTITY="Developer ID Application: …" macos/package-dmg.sh   signed
#   macos/package-dmg.sh                                               unsigned (testing)
#
# The window layout (.DS_Store) is written by macos/dmg/generate_dsstore.py,
# the way Chromium does it: no Finder scripting, reproducible, works in CI.
# The volume name stays ASCII because the background reference stores it in
# a field that cannot hold Chinese.
set -eu
. "$(dirname "$0")/common.sh"
VOLUME="Light Bopomofo"
LINK="應用程式"
OUT="$ROOT/dist/LightBopomofo.dmg"
WORK="$ROOT/target/macos-dmg"
VENV="$ROOT/target/dmg-venv"
[ -d "$APP" ] || { echo "run macos/build.sh first"; exit 1; }

if ! "$VENV/bin/python" -c "import ds_store, mac_alias" 2>/dev/null; then
    python3 -m venv "$VENV"
    "$VENV/bin/pip" install --quiet --disable-pip-version-check "ds_store==1.3.3" "mac_alias==2.2.3"
fi

rm -rf "$WORK" "$OUT"
mkdir -p "$WORK/stage/.background" "$WORK/mnt" "$(dirname "$OUT")"
STAGE="$WORK/stage"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/$LINK"
# Drawn at build time (deterministic) rather than kept in git: the canvas is
# large so it covers big windows.
swift "$ROOT/scripts/make-dmg-background.swift" "$STAGE/.background/background.jpg"
cp "$ROOT/macos/App/Resources/AppIcon.icns" "$STAGE/.VolumeIcon.icns"
"$VENV/bin/python" -I "$ROOT/macos/dmg/generate_dsstore.py" "$STAGE/.DS_Store" \
    --volume-name "$VOLUME" --app-name "$(basename "$APP")" --link-name "$LINK"

# Writable image first, to set the hidden and custom-icon flags, then
# compress it into the read-only image users download.
hdiutil create -quiet -volname "$VOLUME" -srcfolder "$STAGE" -fs HFS+ -format UDRW \
    -ov "$WORK/rw.dmg"
hdiutil attach -quiet -nobrowse -noautoopen -mountpoint "$WORK/mnt" "$WORK/rw.dmg"
SetFile -a V "$WORK/mnt/.background" "$WORK/mnt/.VolumeIcon.icns"
SetFile -a E "$WORK/mnt/$(basename "$APP")"
SetFile -a C "$WORK/mnt"
hdiutil detach -quiet "$WORK/mnt"
hdiutil convert -quiet "$WORK/rw.dmg" -format UDZO -imagekey zlib-level=9 -o "$OUT"

if [ -n "${SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$SIGN_IDENTITY" "$OUT"
fi
echo "packaged $OUT"
