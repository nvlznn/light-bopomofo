#!/usr/bin/env python3
"""Write website/candidates.js: real engine candidates for the website demos.

Every candidate the site shows comes from lb-cli, so the demos match what the
input method really does. Run after the lexicon or the engine changes:

    cargo build --release -p lightbopomofo-cli
    scripts/gen-website-data.py
"""
import json
import os
import subprocess
import tempfile
from itertools import product

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = os.path.join(ROOT, "target", "release", "lb-cli")
OUT = os.path.join(ROOT, "website", "candidates.js")

# Symbols a syllable can start with (initials, medials, finals).
FIRST = "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ"

# Inputs the demos type; every prefix is included.
DEMO = [
    "ㄋㄏㄇ", "ㄉㄅㄑ", "ㄨㄅㄓㄉ", "ㄊㄏㄌ", "ㄒㄒㄋ", "ㄕㄖㄎㄌ",
    "ㄨㄅㄓㄉㄗㄇㄅ", "ㄊㄏㄌㄒㄒㄋ", "ㄒㄋㄎㄌㄕㄊㄐㄎ",
    "ㄉㄅㄑㄨㄅㄓㄉ", "ㄕㄖㄎㄌㄨㄞㄋ", "ㄇㄍㄒㄅㄎㄑ", "ㄅㄏㄧㄙㄨㄞㄋ",
    "ㄨㄒㄔㄈ", "ㄋㄗㄋㄌ",
]

# The MacBook demo types these quotes, in random order. Each piece is typed
# with the fewest keys whose candidates hold it: "keys=text" picks the first
# candidate, "keys=text#r" the r-th (Shift+r); punctuation stands alone.
# Found by searching per-character prefixes (± tone) against lb-cli; every
# piece is checked again below, so a lexicon change can't break the demo.
QUOTES = [
    "ㄓㄣㄒ=真相#3 ㄩㄩㄢㄓㄧㄧㄍ=永遠只有一個 ！",
    "ㄅㄋㄊㄠㄅ=不能逃避 ， ㄅㄋㄊㄠㄅ=不能逃避 ， ㄅㄋㄊㄠㄅ=不能逃避",
    "ㄢ=安 ㄒㄧˉㄐㄌㄧㄢ=西教練 ， ㄨ=我 ㄏㄠㄒ=好想#2 ㄉㄌㄑ=打籃球#3",
    "ㄒㄗㄈㄑㄉㄜ˙ㄏ=現在放棄的話 ， ㄅㄙㄐㄧㄡ=比賽就 ㄐㄕㄨㄌ=結束了 ㄧㄠˉ=喲#3",
    "ㄕㄉㄗㄉ=說到做到 ， ㄓㄐㄕˋㄨㄉㄖㄣˇ=這就是我的忍 ㄉㄠ=道#4",
    "ㄨㄧ=我要 ㄔㄥㄨㄒㄕㄐ=成為新世界 ㄉㄜㄕㄣ=的神",
    "ㄉㄕ=但是 ㄨㄐㄩㄐ=我拒絕",
    "ㄇㄟˊㄨㄊ=沒問題 ， ㄧㄨ=因為#2 ㄨㄕ=我是#2 ㄗ=最#6 ㄑㄧㄤ=強#2 ㄉ=的",
    "ㄨㄧ=我要 ㄅ=把#7 ㄐㄖㄣ=巨人#5 ㄘ=從#2 ㄓㄍ=這個#2 ㄕㄕㄤ=世上#5 ㄧㄓ=一隻#5 ㄅ=不 ㄕㄥ=剩#6 ㄉ=地#3 ㄑㄓㄨ=驅逐#4 ㄔㄑ=出去#2 ！",
    "ㄏ=好#6 ㄕˉㄌ=失禮#8 ㄚ=啊 ！ ㄨㄇㄎ=我們可 ㄕㄔㄨㄣㄞㄛˊ=是純愛哦 ！",
    "ㄌㄉ=勞動#7 ㄐㄕㄍㄡㄕ=就是狗屎",
    "ㄨ=我 ㄩˊㄔ=愚蠢#6 ㄉㄜㄡˉㄉㄉㄡˋㄚ=的歐豆豆啊",
    "ㄏㄞㄉㄜ˙ㄉㄨㄇ=海的對面 ， ㄕ=是 ㄉㄖ=敵人#5",
    "ㄌㄞˊ=萊#2 ㄋㄚ=納#3 ， ㄋ=你#3 ㄗㄨㄛ=坐#5 ㄚ=啊",
    "ㄕㄢㄌ=閃亮#4 ㄧㄠㄧㄢㄉㄜㄞㄓ=耀眼的愛之 ㄩˇ=雨#5 ， ㄧ=要#4 ㄅㄨㄧㄌㄞ=不要來 ㄕㄡˇ=首#3 ㄢㄎ=安可 ㄑㄩ=曲#5",
    "ㄧㄐ=已經#4 ㄇㄕˋ=沒事#3 ㄌ=了 ！ ㄧ=要#4 ㄨㄣㄨㄕㄇ=問為什麼 ， ㄧㄨㄟㄨ=因為我 ㄌㄌ=來了#8 ！",
    "ㄧ=一#6 ㄉㄞ=袋#6 ㄇㄧˇㄧㄠㄎㄤˊ=米要扛 ㄐㄧ=幾#9 ㄌ=樓#4",
    "ㄔㄅㄞ=崇拜 ㄕㄐㄩㄌㄌㄧˇㄐ=是距離理解 ㄗ=最#6 ㄧㄠㄩㄉㄜㄍㄢㄑ=遙遠的感情",
    "ㄓㄧㄧㄨㄗㄞ=只要有我在 ， ㄋㄧㄐㄕ=你就是 ㄗ=最#6 ㄑㄧㄤ=強#2 ㄉ=的",
    "ㄨㄎㄞˇㄊㄚㄔㄠ=我凱他超",
    "ㄕㄇ=什麼#2 ㄉㄡㄨㄈㄕㄜˇㄑㄉ=都無法捨棄的 ㄖㄣ=人 ， ㄐㄉ=絕對#9 ㄇㄟㄧㄋㄌ=沒有能力 ㄍㄅㄖㄏㄕ=改變任何事 ㄨ=物#8",
    "ㄅㄤㄨㄛ=幫我 ㄔㄥˉ=撐#2 ㄕˊㄇ=十秒#4",
    "ㄒㄔㄨˉ=獻出#7 ㄒㄗ=心臟#6 ㄅ=吧#9 ！",
    "ㄋ=你#3 ㄘ=從#2 ㄕㄇ=什麼#2 ㄕㄏ=時候#3 ㄎㄕㄔㄢㄕㄌ=開始產生了 ㄨㄇㄟ=我沒#2 ㄕㄩㄐㄏㄕㄩㄝ=使用鏡花水月 ㄉㄘㄨㄛㄐ=的錯覺",
]
PUNCT = "，。？！："

# The learning demo: what the user picks the first time, piece by piece.
LEARN = [
    ("ㄨㄒ", "我想"), ("ㄔㄈ", "吃飯"), ("ㄨㄒㄔㄈ", "我想吃飯"),
    ("ㄋㄗ", "你在"), ("ㄋㄌ", "哪裡"), ("ㄋㄗㄋㄌ", "你在哪裡"),
]


def query(inputs, history=None):
    args = [CLI, "-n", "9"]
    if history:
        args += ["--history", history]
    out = subprocess.run(args + inputs, capture_output=True, text=True,
                         check=True, cwd=ROOT).stdout
    result, current = {}, None
    for line in out.splitlines():
        if not line.startswith("  "):
            current = line.split()[0]
            result[current] = []
        else:
            text = line.strip().split(". ", 1)[1].split()[0]
            result[current].append(text)
    return {k: " ".join(v) for k, v in result.items()}


def parse_quote(quote):
    steps = []
    for token in quote.split():
        if token in PUNCT:
            steps.append(token)
            continue
        keys, text = token.split("=")
        text, _, rank = text.partition("#")
        steps.append({"k": keys, "t": text, **({"r": int(rank)} if rank else {})})
    return steps


def check_quotes(quotes):
    """Every piece must be exactly its candidate (not a partial pick)."""
    pieces = [s for q in quotes for s in q if isinstance(s, dict)]
    out = subprocess.run([CLI, "-n", "9"] + sorted({p["k"] for p in pieces}),
                         capture_output=True, text=True, check=True, cwd=ROOT).stdout
    full, current = {}, None
    for line in out.splitlines():
        if not line.startswith("  "):
            current = line.split()[0]
            full[current] = []
        else:
            full[current].append(line.strip().split(". ", 1)[1].rsplit("[", 1)[0].strip())
    bad = [p for p in pieces if full.get(p["k"], [])[p.get("r", 1) - 1:p.get("r", 1)] != [p["t"]]]
    for p in bad:
        print(f"quote piece no longer works: {p['k']} -> {full.get(p['k'])}, want #{p.get('r', 1)} {p['t']}")
    if bad:
        raise SystemExit(1)


def main():
    quotes = [parse_quote(q) for q in QUOTES]
    check_quotes(quotes)
    keys = set(FIRST)
    keys |= {a + b for a, b in product(FIRST, repeat=2)}
    typed = DEMO + [s["k"] for q in quotes for s in q if isinstance(s, dict)]
    for d in typed:
        keys |= {d[:i] for i in range(1, len(d) + 1)}
    base = query(sorted(keys))

    with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as f:
        for typed, text in LEARN:
            f.write(f"1\t{typed}\t{text}\n")
    learned = query([t for t, _ in LEARN], history=f.name)
    os.unlink(f.name)

    with open(OUT, "w") as f:
        f.write("// Generated by scripts/gen-website-data.py from the real engine. Do not edit.\n")
        f.write("window.LB_CANDIDATES = ")
        json.dump(base, f, ensure_ascii=False, separators=(",", ":"))
        f.write(";\nwindow.LB_LEARNED = ")
        json.dump(learned, f, ensure_ascii=False, separators=(",", ":"))
        f.write(";\nwindow.LB_QUOTES = ")
        json.dump(quotes, f, ensure_ascii=False, separators=(",", ":"))
        f.write(";\n")
    print(f"{OUT}: {len(base)} inputs, {os.path.getsize(OUT)} bytes")


if __name__ == "__main__":
    main()
