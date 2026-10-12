#!/bin/sh
# Install into ~/Library/Input Methods, register and enable the input source.
# The running instance is stopped; macOS starts the new one on next use.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP="$ROOT/macos/build/LightBopomofo.app"
DEST="$HOME/Library/Input Methods"
[ -d "$APP" ] || { echo "run macos/build.sh first"; exit 1; }
mkdir -p "$DEST"
pkill -x LightBopomofo || true
rm -rf "$DEST/LightBopomofo.app"
cp -R "$APP" "$DEST/"
"$DEST/LightBopomofo.app/Contents/MacOS/LightBopomofo" install
