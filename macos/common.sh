# Shared by the macOS scripts.
ROOT=$(cd "$(dirname "$0")/.." && pwd)
export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/rustup/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET=13.0
HEADER="$ROOT/crates/lightbopomofo-ffi/include/lightbopomofo.h"
# The app users drag into /Applications, and the input method inside it.
APP="$ROOT/macos/build/LightBopomofo.app"
IME="$APP/Contents/Library/Input Methods/LightBopomofo.app"
# The single version number, set in the app's Info.plist.
VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$ROOT/macos/App/Info.plist")
BUILD_NUMBER=$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$ROOT/macos/App/Info.plist")

ensure_lexicon() {
    [ -f "$ROOT/data/tsi.csv" ] || "$ROOT/scripts/fetch-lexicon.sh"
}

# Build the engine static library for one target triple (host if omitted).
build_rust() {
    ensure_lexicon
    cargo build --release -p lightbopomofo-ffi --manifest-path "$ROOT/Cargo.toml" ${1:+--target "$1"}
}
