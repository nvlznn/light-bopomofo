# Shared by build.sh and test.sh.
ROOT=$(cd "$(dirname "$0")/.." && pwd)
export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/rustup/bin:$PATH"
HEADER="$ROOT/crates/lightbopomofo-ffi/include/lightbopomofo.h"
SWIFT_FLAGS="-target arm64-apple-macos13.0 -import-objc-header $HEADER -L $ROOT/target/release -llightbopomofo_ffi"

build_rust() {
    [ -f "$ROOT/data/tsi.csv" ] || "$ROOT/scripts/fetch-lexicon.sh"
    cargo build --release -p lightbopomofo-ffi --manifest-path "$ROOT/Cargo.toml"
}
