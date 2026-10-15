#!/bin/sh
# Development install: run the freshly built app's own installer, which
# copies it into ~/Library/Input Methods, restarts it and registers it —
# the same code path users get from the disk image.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP="$ROOT/macos/build/LightBopomofo.app"
[ -d "$APP" ] || { echo "run macos/build.sh first"; exit 1; }
"$APP/Contents/MacOS/LightBopomofo" install
