#!/usr/bin/env python3
"""Sync the backend's curated dictionary into the next app release.

    scripts/approved-words.py [--dry-run] [--site URL]

Run before building a release. Only the marked backend section is replaced;
handwritten adjustments remain unchanged. Deletions remove entries from that
section on the next sync. No guessing of full readings from submitted initials.
"""
import csv
import getpass
import http.cookiejar
import io
import json
import os
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ADJUSTMENTS = ROOT / "crates/lightbopomofo-core/lexicon-adjustments.csv"
EMAIL = "ino@noky.dev"
BEGIN = "# BEGIN backend dictionary"
END = "# END backend dictionary"

def fetch_approved(site, password):
    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    login = urllib.request.Request(
        f"{site}/api/admin/login", method="POST", headers={"Content-Type": "application/json"},
        data=json.dumps({"email": EMAIL, "password": password}).encode())
    opener.open(login)
    with opener.open(f"{site}/api/admin/export") as r:
        return json.load(r)["lexicon"]



def merge_dictionary(original, words):
    if original.count(BEGIN) != original.count(END) or original.count(BEGIN) > 1:
        raise ValueError("invalid backend dictionary markers")
    output = io.StringIO()
    writer = csv.writer(output, lineterminator="\n")
    for word in sorted(words, key=lambda w: (w["text"], w["reading"])):
        writer.writerow([word["text"], word["frequency"], word["reading"]])
    section = BEGIN + "\n" + output.getvalue() + END
    if BEGIN in original:
        start, end = original.index(BEGIN), original.index(END) + len(END)
        return original[:start] + section + original[end:]
    return original.rstrip() + "\n\n" + section + "\n"


def main():
    dry = "--dry-run" in sys.argv
    site = sys.argv[sys.argv.index("--site") + 1] if "--site" in sys.argv else "https://lightbopomofo.noky.dev"
    password = os.environ.get("LB_ADMIN_PASSWORD") or getpass.getpass(f"{EMAIL} password: ")
    words = fetch_approved(site, password)
    original = ADJUSTMENTS.read_text(encoding="utf-8")
    updated = merge_dictionary(original, words)
    if not dry and updated != original:
        ADJUSTMENTS.write_text(updated, encoding="utf-8")
    print(f"{'Would sync' if dry else 'Synced'} {len(words)} curated dictionary entries")


if __name__ == "__main__":
    main()
