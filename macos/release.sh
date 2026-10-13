#!/bin/sh
# Build, sign, package, notarize and staple a release of 輕注音.
#
#   macos/release.sh            produce a notarized dist/LightBopomofo.pkg
#   macos/release.sh --publish  …and create GitHub release v<version> with it
#
# Needs in the login keychain: "Developer ID Application" and "Developer ID
# Installer" certificates, and a notarytool profile (default name
# "lightbopomofo"; override with NOTARY_PROFILE) created with
#   xcrun notarytool store-credentials lightbopomofo --apple-id … --team-id …
set -eu
. "$(dirname "$0")/common.sh"
PROFILE="${NOTARY_PROFILE:-lightbopomofo}"
PKG="$ROOT/dist/LightBopomofo.pkg"

find_identity() {
    security find-identity -v -p "$1" | sed -n "s/.*\"\($2: .*\)\"/\1/p" | head -1
}
SIGN_IDENTITY="${SIGN_IDENTITY:-$(find_identity codesigning "Developer ID Application")}"
INSTALLER_IDENTITY="${INSTALLER_IDENTITY:-$(find_identity basic "Developer ID Installer")}"
[ -n "$SIGN_IDENTITY" ] || { echo "no Developer ID Application certificate in keychain"; exit 1; }
[ -n "$INSTALLER_IDENTITY" ] || { echo "no Developer ID Installer certificate in keychain"; exit 1; }
if [ "${1:-}" = "--publish" ] && git -C "$ROOT" rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null; then
    echo "tag v$VERSION already exists; bump CFBundleShortVersionString in macos/Info.plist"
    exit 1
fi
echo "version   $VERSION"
echo "app       $SIGN_IDENTITY"
echo "installer $INSTALLER_IDENTITY"

SIGN_IDENTITY="$SIGN_IDENTITY" "$ROOT/macos/build.sh"
INSTALLER_IDENTITY="$INSTALLER_IDENTITY" "$ROOT/macos/package.sh"
xcrun notarytool submit "$PKG" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$PKG"
spctl --assess --type install -vv "$PKG"
shasum -a 256 "$PKG"

if [ "${1:-}" = "--publish" ]; then
    gh release create "v$VERSION" "$PKG" --repo nvlznn/light-bopomofo \
        --title "輕注音 $VERSION" --notes-file "$ROOT/macos/RELEASE_NOTES.md"
fi
