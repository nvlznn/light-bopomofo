#!/bin/sh
# Build, sign, notarize and staple a release of 輕注音.
#
#   macos/release.sh            produce a notarized dist/LightBopomofo.dmg
#   macos/release.sh --publish  …and create GitHub release v<version> with it
#
# Needs a "Developer ID Application" certificate in the login keychain and a
# notarytool profile (default "lightbopomofo"; override with NOTARY_PROFILE).
# Run it from Terminal the first time so macOS can ask to allow codesign to
# use the key ("Always Allow").
set -eu
. "$(dirname "$0")/common.sh"
PROFILE="${NOTARY_PROFILE:-lightbopomofo}"
DMG="$ROOT/dist/LightBopomofo.dmg"
WORK="$ROOT/target/macos-release"

SIGN_IDENTITY="${SIGN_IDENTITY:-$(security find-identity -v -p codesigning \
    | sed -n 's/.*"\(Developer ID Application: .*\)"/\1/p' | head -1)}"
[ -n "$SIGN_IDENTITY" ] || { echo "no Developer ID Application certificate in keychain"; exit 1; }
if [ "${1:-}" = "--publish" ] && git -C "$ROOT" rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null; then
    echo "tag v$VERSION already exists; bump CFBundleShortVersionString in macos/Info.plist"
    exit 1
fi
echo "version $VERSION"
echo "signing $SIGN_IDENTITY"
export SIGN_IDENTITY

# 1. The app, notarized and stapled on its own, so the copy installed into
#    ~/Library/Input Methods carries its ticket even offline.
"$ROOT/macos/build.sh"
rm -rf "$WORK" && mkdir -p "$WORK"
ditto -c -k --keepParent "$APP" "$WORK/LightBopomofo.zip"
xcrun notarytool submit "$WORK/LightBopomofo.zip" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$APP"

# 2. The disk image around it.
"$ROOT/macos/package-dmg.sh"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$DMG"

spctl --assess --type open --context context:primary-signature -vv "$DMG"
spctl --assess --type execute -vv "$APP"
shasum -a 256 "$DMG"

if [ "${1:-}" = "--publish" ]; then
    gh release create "v$VERSION" "$DMG" --repo nvlznn/light-bopomofo \
        --title "輕注音 $VERSION" --notes-file "$ROOT/macos/RELEASE_NOTES.md"
fi
