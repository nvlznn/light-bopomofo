#!/usr/bin/env python3
"""Generate eval/mixed.tsv: phrases of eval/tatoeba.tsv with an English word
of crates/lightbopomofo-core/english.csv before or after them, typed in
lowercase as its 大千 keys (app is ㄇㄣㄣ). Same columns as tatoeba.tsv
(text, full bopomofo, first symbols), so `lb-cli --eval eval/mixed.tsv`
measures how often the English comes out right inside Chinese.

    scripts/gen-mixed-eval.py [COUNT]   (default 1000; fixed seed)
"""
import random
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KEYS = "1qaz2wsx3edc4rfv5tgb6yhn7ujm8ik,9ol.0p;/-"
SYMBOLS = "ㄅㄆㄇㄈㄉㄊㄋㄌˇㄍㄎㄏˋㄐㄑㄒㄓㄔㄕㄖˊㄗㄘㄙ˙ㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ"
DACHEN = dict(zip(KEYS, SYMBOLS))


def typed(word):
    return "".join(DACHEN[c] for c in word.lower())


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 1000
    words = []
    for line in (ROOT / "crates/lightbopomofo-core/english.csv").read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "," not in line:
            continue
        text, weight = line.split(",", 1)
        words.append((text.strip(), float(weight)))
    phrases = [l.split("\t") for l in (ROOT / "eval/tatoeba.tsv").read_text().splitlines()]
    phrases = [p for p in phrases if len(p) >= 3]
    rng = random.Random(20261020)
    texts, weights = zip(*words)
    out = []
    for text, full, first in rng.sample(phrases, count):
        word = rng.choices(texts, weights)[0]
        keys = typed(word)
        if rng.random() < 0.5:
            out.append(f"{word}{text}\t{keys}{full}\t{keys}{first}")
        else:
            out.append(f"{text}{word}\t{full}{keys}\t{first}{keys}")
    (ROOT / "eval/mixed.tsv").write_text("\n".join(out) + "\n")
    print(f"eval/mixed.tsv: {len(out)} phrases")


if __name__ == "__main__":
    main()
