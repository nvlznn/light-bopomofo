#!/usr/bin/env python3
"""Write crates/lightbopomofo-core/word-pairs.tsv, the word pairs the engine
weighs whole-input conversions with, from count-words' counts:

    scripts/corpus-text.py data/corpus/*.parquet > data/corpus/text.txt
    count-words data/tsi.csv data/corpus < data/corpus/text.txt
    scripts/word-pairs.py data/corpus

Keeps the PAIRS most frequent pairs whose words go together more often than
chance, each with its pointwise mutual information (natural log): how much
likelier the second word is right after the first than anywhere.
"""
import math
import os
import sys

PAIRS = 2_000_000
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "crates", "lightbopomofo-core", "word-pairs.tsv")

counts_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "data", "corpus")
words = {}
for line in open(os.path.join(counts_dir, "words.tsv"), encoding="utf-8"):
    word, count = line.rstrip("\n").split("\t")
    words[word] = int(count)
total = sum(words.values())

pairs = {}
kept = 0
# pairs.tsv is sorted by count, most frequent first.
for line in open(os.path.join(counts_dir, "pairs.tsv"), encoding="utf-8"):
    a, b, count = line.rstrip("\n").split("\t")
    pmi = math.log(int(count) * total / (words[a] * words[b]))
    if pmi < 0.05:
        continue
    pairs.setdefault(a, []).append((b, pmi))
    kept += 1
    if kept == PAIRS:
        break

with open(OUT, "w", encoding="utf-8") as out:
    out.write("# 每個詞一行：詞、在語料中出現的次數，接著「後一個詞 加分」（加分 = 點互資訊，自然對數），以 tab 分隔。\n")
    out.write("# FineWeb-2 國語語料（cmn_Hani，ODC-By 1.0）轉為繁體後統計；由 scripts/word-pairs.py 產生。\n")
    for a in sorted(words, key=lambda w: w.encode()):
        nexts = sorted(pairs.get(a, []), key=lambda p: p[0].encode())
        out.write("\t".join([a, str(words[a])] + [f"{b} {pmi:.1f}" for b, pmi in nexts]) + "\n")
print(f"{OUT}: {kept} pairs after {len(pairs)} words")
