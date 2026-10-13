#!/bin/sh
# Composer tests against the real engine (no InputMethodKit involved).
set -eu
. "$(dirname "$0")/common.sh"
OUT="$ROOT/target/macos-tests"

build_rust
swiftc -import-objc-header "$HEADER" -L "$ROOT/target/release" -llightbopomofo_ffi \
    "$ROOT/macos/Sources/Engine.swift" "$ROOT/macos/Sources/Composer.swift" \
    "$ROOT/macos/Tests/main.swift" -o "$OUT"
"$OUT" "$ROOT/data/tsi.csv"
