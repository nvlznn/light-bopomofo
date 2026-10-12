#!/bin/sh
# Download the chewing lexicon (LGPL-2.1-or-later) into data/.
# Pinned to a fixed libchewing-data commit so results are reproducible.
set -eu
COMMIT=ea74f76dd2548b1d65d0ff70c3ae66057a6ad97d
ROOT=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$ROOT/data"
curl -fsSL -o "$ROOT/data/tsi.csv" \
  "https://raw.githubusercontent.com/chewing/libchewing-data/$COMMIT/dict/chewing/tsi.csv"
echo "data/tsi.csv: $(wc -l < "$ROOT/data/tsi.csv") lines"
