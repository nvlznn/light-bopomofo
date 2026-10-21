#!/usr/bin/env python3
"""Generate the admin lookup from the same source and adjustments as the app.
Run after changing the shipped dictionary; source defaults to data/tsi.csv.
The generated index contains dictionary words/readings only, never user data.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', type=Path, default=ROOT / 'data/tsi.csv')
args = parser.parse_args()
adjustments = ROOT / 'crates/lightbopomofo-core/lexicon-adjustments.csv'
source = args.source.read_text()
changes = adjustments.read_text()
removed = {line[1:].strip() for line in changes.splitlines() if line.startswith('-')}
words = {}
for row in csv.reader((source + '\n' + changes).splitlines()):
    if len(row) < 3 or not row[0] or row[0].startswith(('#', '-')) or row[0] in removed:
        continue
    reading = ' '.join(row[2].split())
    if not reading or len(reading.split()) != len(row[0]):
        continue
    words.setdefault(row[0], set()).add(reading)
payload = {'sourceSha256': hashlib.sha256(source.encode()).hexdigest(),
           'adjustmentsSha256': hashlib.sha256(changes.encode()).hexdigest(),
           'words': {word: sorted(readings) for word, readings in sorted(words.items())}}
output = ROOT / 'website/api/_data/lexicon.json'
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(payload, ensure_ascii=False, separators=(',', ':')) + '\n')
print(f'Indexed {len(words)} dictionary words in {output.name}')
