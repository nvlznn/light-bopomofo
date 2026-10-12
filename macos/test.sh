#!/bin/sh
# Composer tests against the real engine (no InputMethodKit involved).
set -eu
. "$(dirname "$0")/common.sh"
OUT="$ROOT/target/macos-tests"

build_rust
mkdir -p "$(dirname "$OUT")"
# shellcheck disable=SC2086
swiftc $SWIFT_FLAGS \
    "$ROOT/macos/Sources/Engine.swift" "$ROOT/macos/Sources/Composer.swift" \
    "$ROOT/macos/Tests/main.swift" -o "$OUT"
"$OUT" "$ROOT/data/tsi.csv"
