#!/usr/bin/env python3
"""Write eval/tatoeba.tsv: everyday Mandarin phrases with the bopomofo a
user would type for them, to measure the engine (lb-cli --eval).

Tatoeba's Mandarin sentences (CC BY 2.0 FR), converted to Taiwan Traditional,
split at punctuation into phrases of 2–10 Han characters. Each line:

    text <TAB> full bopomofo, no tones <TAB> first symbol of each syllable

Phrases with a reading the lexicon doesn't have for that character are left
out, as no engine could type them. Needs opencc and pypinyin (pip install):

    curl -fsSLO https://downloads.tatoeba.org/exports/per_language/cmn/cmn_sentences.tsv.bz2
    bunzip2 cmn_sentences.tsv.bz2
    scripts/gen-eval.py cmn_sentences.tsv
"""
import csv
import os
import random
import re
import sys

import opencc
from pypinyin import Style, pinyin

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "eval", "tatoeba.tsv")
COUNT = 5000
TONES = "ˉˊˇˋ˙"

# Every toneless reading each character has in the lexicon.
readings = {}
for row in csv.reader(open(os.path.join(ROOT, "data", "tsi.csv"), encoding="utf-8")):
    if len(row) >= 3 and len(row[0]) == 1 and not row[0].startswith("#"):
        readings.setdefault(row[0], set()).add(row[2].strip().strip(TONES))

convert = opencc.OpenCC("s2tw")
phrases = set()
for line in open(sys.argv[1], encoding="utf-8"):
    parts = line.rstrip("\n").split("\t")
    if len(parts) < 3:
        continue
    for phrase in re.split(r"[^一-鿿]+", convert.convert(parts[2])):
        if 2 <= len(phrase) <= 10:
            phrases.add(phrase)

rows = []
for phrase in sorted(phrases):
    syllables = [p[0].strip(TONES) for p in pinyin(phrase, style=Style.BOPOMOFO)]
    if len(syllables) != len(phrase) or any(s not in readings.get(c, ()) for c, s in zip(phrase, syllables)):
        continue
    rows.append((phrase, "".join(syllables), "".join(s[0] for s in syllables)))

random.Random(1).shuffle(rows)
os.makedirs(os.path.dirname(OUT), exist_ok=True)
with open(OUT, "w", encoding="utf-8") as f:
    for row in sorted(rows[:COUNT], key=lambda r: (len(r[0]), r[0])):
        f.write("\t".join(row) + "\n")
print(f"{OUT}: {min(COUNT, len(rows))} of {len(rows)} phrases ({len(phrases)} before the reading check)")
