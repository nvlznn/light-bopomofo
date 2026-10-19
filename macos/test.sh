#!/bin/sh
# Composer and installer tests against the real engine (no InputMethodKit,
# nothing installed).
set -eu
. "$(dirname "$0")/common.sh"
OUT="$ROOT/target/macos-tests"

build_rust
swiftc -import-objc-header "$HEADER" -L "$ROOT/target/release" -llightbopomofo_ffi \
    "$ROOT/macos/Shared/Engine.swift" "$ROOT/macos/Shared/Installer.swift" \
    "$ROOT/macos/Shared/Composer.swift" "$ROOT/macos/Shared/PendingUpdate.swift" \
    "$ROOT/macos/App/UpdateCheck.swift" \
    "$ROOT/macos/App/Lesson.swift" \
    "$ROOT/macos/Tests/main.swift" -o "$OUT"
"$OUT" "$ROOT/data/tsi.csv"
