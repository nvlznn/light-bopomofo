#!/bin/sh
# Put macos/build/LightBopomofo.app into dist/LightBopomofo.dmg: the app on
# the left, an Applications link on the right, an arrow between them.
#
#   SIGN_IDENTITY="Developer ID Application: …" macos/package-dmg.sh   signed
#   macos/package-dmg.sh                                               unsigned (testing)
#
# Uses dmgbuild (writes the Finder layout directly, no Finder scripting),
# installed into target/dmg-venv on first use. The volume name stays ASCII:
# the background is found through a classic alias, whose volume-name field
# cannot hold Chinese, and Finder then shows no background at all.
set -eu
. "$(dirname "$0")/common.sh"
OUT="$ROOT/dist/LightBopomofo.dmg"
VENV="$ROOT/target/dmg-venv"
[ -d "$APP" ] || { echo "run macos/build.sh first"; exit 1; }

if [ ! -x "$VENV/bin/dmgbuild" ]; then
    python3 -m venv "$VENV"
    "$VENV/bin/pip" install --quiet --disable-pip-version-check "dmgbuild==1.6.5"
fi

rm -f "$OUT"
mkdir -p "$(dirname "$OUT")"
"$VENV/bin/dmgbuild" -s "$ROOT/macos/dmg/settings.py" \
    -D app="$APP" \
    -D background="$ROOT/macos/dmg/background.tiff" \
    -D icon="$ROOT/macos/App/Resources/AppIcon.icns" \
    "Light Bopomofo" "$OUT" >/dev/null
if [ -n "${SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$SIGN_IDENTITY" "$OUT"
fi
echo "packaged $OUT"
