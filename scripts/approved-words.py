#!/usr/bin/env python3
"""Add the words approved on lightbopomofo.noky.dev/admin.html (分享新詞) to
crates/lightbopomofo-core/lexicon-adjustments.csv, for the next release.

Each word gets a full reading: for every character, the most frequent of its
tsi.csv readings that agrees with what users typed (ㄍㄨㄊ or ㄍㄨㄥㄨㄢㄊ for
貢丸湯 → ㄍㄨㄥˋ ㄨㄢˊ ㄊㄤ). Words already in the adjustments are skipped.

    LB_ADMIN_PASSWORD=… scripts/approved-words.py [--dry-run] [--site URL]
"""
import csv
import getpass
import http.cookiejar
import json
import os
import sys
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADJUSTMENTS = os.path.join(ROOT, "crates", "lightbopomofo-core", "lexicon-adjustments.csv")
TSI = os.path.join(ROOT, "data", "tsi.csv")
EMAIL = "ino@noky.dev"
FREQ = 2000  # as 什麼意思
TONES = "ˉˊˇˋ˙"


def fetch_approved(site, password):
    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    login = urllib.request.Request(
        f"{site}/api/admin/login", method="POST", headers={"Content-Type": "application/json"},
        data=json.dumps({"email": EMAIL, "password": password}).encode())
    opener.open(login)
    with opener.open(f"{site}/api/admin/export") as r:
        return json.load(r)["words"]


def char_readings():
    """Each character's readings, most frequent first."""
    readings = {}
    for row in csv.reader(open(TSI, encoding="utf-8")):
        if len(row) >= 3 and len(row[0]) == 1 and not row[0].startswith("#"):
            readings.setdefault(row[0], []).append((int(row[1]) if row[1].isdigit() else 0, row[2].strip()))
    return {c: [r for _, r in sorted(rs, reverse=True)] for c, rs in readings.items()}


def fits(typed, readings):
    """Whether `typed` splits into one slot per reading, each the start of
    its syllable (with the tone, if typed, matching)."""
    def go(t, i):
        if i == len(readings):
            return t == ""
        base, tone = readings[i].rstrip(TONES), readings[i][len(readings[i].rstrip(TONES)):] or "ˉ"
        for n in range(1, len(base) + 1):
            slot = t[:n]
            if not base.startswith(slot) or len(slot) < n:
                break
            if t[n:n + 1] in TONES and t[n:n + 1] != "":
                if t[n] == tone and go(t[n + 1:], i + 1):
                    return True
            elif go(t[n:], i + 1):
                return True
        return False
    return go(typed, 0)


def reading(text, typings, readings):
    """The most frequent readings of the characters that fit the typing typed
    most often, or None."""
    options = [readings.get(c, []) for c in text]
    if not all(options):
        return None
    def search(i, chosen):
        if i == len(text):
            return chosen if any(fits(t, chosen) for t in typings) else None
        for r in options[i]:
            found = search(i + 1, chosen + [r])
            if found:
                return found
        return None
    return search(0, [])


def main():
    dry = "--dry-run" in sys.argv
    site = sys.argv[sys.argv.index("--site") + 1] if "--site" in sys.argv else "https://lightbopomofo.noky.dev"
    password = os.environ.get("LB_ADMIN_PASSWORD") or getpass.getpass(f"{EMAIL} password: ")
    words = {}
    for w in fetch_approved(site, password):
        words.setdefault(w["text"], []).append(w["typed"])
    existing = {line.split(",")[0].lstrip("-") for line in open(ADJUSTMENTS, encoding="utf-8") if not line.startswith("#")}
    readings = char_readings()
    lines, skipped = [], []
    for text, typings in sorted(words.items()):
        if text in existing:
            continue
        r = reading(text, typings, readings)
        if r is None:
            skipped.append(text)
            continue
        lines.append(f"{text},{FREQ},{' '.join(r)}")
    if lines and not dry:
        with open(ADJUSTMENTS, "a", encoding="utf-8") as f:
            f.write("# 分享新詞審核通過（scripts/approved-words.py）\n" + "\n".join(lines) + "\n")
    for line in lines:
        print(("would add " if dry else "added ") + line)
    for text in skipped:
        print(f"no reading fits what was typed: {text} ({', '.join(words[text])})", file=sys.stderr)


if __name__ == "__main__":
    main()
