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
if [ "${1:-}" = "--publish" ]; then
    # The release must point at code that is committed and on GitHub.
    if [ -n "$(git -C "$ROOT" status --porcelain)" ]; then
        echo "uncommitted changes; commit them before publishing"
        exit 1
    fi
    git -C "$ROOT" fetch -q origin
    if [ "$(git -C "$ROOT" rev-parse HEAD)" != "$(git -C "$ROOT" rev-parse origin/main)" ]; then
        echo "HEAD is not origin/main; push (or pull) first"
        exit 1
    fi
    TAG_COMMIT=$(git -C "$ROOT" rev-parse -q --verify "refs/tags/v$VERSION^{commit}" || true)
    if [ -n "$TAG_COMMIT" ] && [ "$TAG_COMMIT" != "$(git -C "$ROOT" rev-parse HEAD)" ]; then
        echo "tag v$VERSION exists on another commit; bump the version in macos/App/Info.plist"
        exit 1
    fi
fi
echo "version $VERSION"
echo "signing $SIGN_IDENTITY"
export SIGN_IDENTITY

# 1. The app (with the input method inside), notarized and stapled on its
#    own, so it carries its ticket even when the disk image is opened offline.
"$ROOT/macos/build.sh"
rm -rf "$WORK" && mkdir -p "$WORK"
ditto -c -k --keepParent "$APP" "$WORK/LightBopomofo.zip"
xcrun notarytool submit "$WORK/LightBopomofo.zip" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$APP"
# Don't staple the inner input method: that writes a ticket file into it
# after the outer signature sealed it, which breaks the app's signature.
# It was notarized as part of the app, and the installed copy has its
# quarantine cleared, so it needs no ticket of its own.
codesign --verify --deep --strict "$APP"

# 2. The disk image around it.
"$ROOT/macos/package-dmg.sh"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$DMG"

spctl --assess --type open --context context:primary-signature -vv "$DMG"
spctl --assess --type execute -vv "$APP"
shasum -a 256 "$DMG"

if [ "${1:-}" = "--publish" ]; then
    # Tag with git over SSH: `gh release create` cannot create a tag on a
    # commit that contains workflow files without the `workflow` scope.
    if ! git -C "$ROOT" rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null; then
        git -C "$ROOT" tag -a "v$VERSION" -m "輕注音 $VERSION"
    fi
    git -C "$ROOT" push origin "v$VERSION"
    RELEASE_DATE=$(TZ=Asia/Taipei date +%y.%m.%d)
    gh release create "v$VERSION" "$DMG" --repo nvlznn/light-bopomofo --verify-tag \
        --title "$VERSION ($RELEASE_DATE)" --notes-file "$ROOT/macos/RELEASE_NOTES.md"
    # The Windows installer is attached by .github/workflows/release.yml.
fi
