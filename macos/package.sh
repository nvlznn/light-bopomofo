#!/bin/sh
# Wrap macos/build/LightBopomofo.app into dist/LightBopomofo.pkg, which
# installs into /Library/Input Methods and registers the input source.
#
#   INSTALLER_IDENTITY="Developer ID Installer: …" macos/package.sh   signed
#   macos/package.sh                                                  unsigned (testing)
set -eu
. "$(dirname "$0")/common.sh"
WORK="$ROOT/target/macos-pkg"
STAGE="$WORK/root"
OUT="$ROOT/dist/LightBopomofo.pkg"
[ -d "$APP" ] || { echo "run macos/build.sh first"; exit 1; }

rm -rf "$WORK"
mkdir -p "$STAGE/Library/Input Methods" "$(dirname "$OUT")"
cp -R "$APP" "$STAGE/Library/Input Methods/"

pkgbuild --analyze --root "$STAGE" "$WORK/component.plist" >/dev/null
# Never "relocate" onto another copy with the same bundle ID.
plutil -replace 0.BundleIsRelocatable -bool NO "$WORK/component.plist"
pkgbuild --root "$STAGE" --component-plist "$WORK/component.plist" \
    --identifier dev.noky.inputmethod.lightbopomofo.pkg --version "$VERSION" \
    --install-location / --scripts "$ROOT/macos/pkg/scripts" \
    "$WORK/component.pkg" >/dev/null

sed "s/__VERSION__/$VERSION/" "$ROOT/macos/pkg/distribution.xml" > "$WORK/distribution.xml"
set -- --distribution "$WORK/distribution.xml" --package-path "$WORK" \
    --resources "$ROOT/macos/pkg/resources"
if [ -n "${INSTALLER_IDENTITY:-}" ]; then
    set -- "$@" --sign "$INSTALLER_IDENTITY" --timestamp
fi
productbuild "$@" "$OUT" >/dev/null
echo "packaged $OUT"
