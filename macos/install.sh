#!/bin/sh
# Development install: run the bundled input method's installer, which
# copies it into ~/Library/Input Methods, restarts it and registers it —
# the code path the app uses for users.
set -eu
. "$(dirname "$0")/common.sh"
[ -d "$IME" ] || { echo "run macos/build.sh first"; exit 1; }
"$IME/Contents/MacOS/LightBopomofo" install
