#!/bin/sh
# Composer and installer tests against the real engine (no InputMethodKit,
# nothing installed).
set -eu
. "$(dirname "$0")/common.sh"
OUT="$ROOT/target/macos-tests"

build_rust
swiftc -import-objc-header "$HEADER" -L "$ROOT/target/release" -llightbopomofo_ffi \
    "$ROOT/macos/Shared/Engine.swift" "$ROOT/macos/Shared/Installer.swift" \
    "$ROOT/macos/Shared/Composer.swift" "$ROOT/macos/Shared/KeyEvent.swift" \
    "$ROOT/macos/Shared/PendingUpdate.swift" \
    "$ROOT/macos/App/UpdateCheck.swift" \
    "$ROOT/macos/App/Lesson.swift" \
    "$ROOT/macos/Tests/main.swift" -o "$OUT"
# The lexicon as shipped: compiled, with the word pairs.
cargo build --release -p build-lexicon --manifest-path "$ROOT/Cargo.toml"
"$ROOT/target/release/build-lexicon" "$ROOT/data/tsi.csv" "$OUT-lexicon.bin"
"$OUT" "$OUT-lexicon.bin"

# Service lifecycle tests use a fake OS boundary; they never register a real job.
swiftc -parse-as-library "$ROOT/macos/App/BackgroundRegistration.swift" \
    "$ROOT/macos/Tests/BackgroundService/main.swift" -o "$OUT-background"
"$OUT-background"
