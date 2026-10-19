#!/usr/bin/env python3
"""Turn FineWeb-2 Mandarin (cmn_Hani) parquet files into plain text for
counting words and word pairs (count-words): converted to Taiwan Traditional
with OpenCC, one run of two or more Han characters per line.

Needs opencc and pyarrow (pip install). The files are under
https://huggingface.co/datasets/HuggingFaceFW/fineweb-2/tree/main/data/cmn_Hani/train

    scripts/corpus-text.py data/corpus/*.parquet > data/corpus/text.txt
"""
import re
import sys
from multiprocessing import Pool

import opencc
import pyarrow.parquet as pq

HAN_RUNS = re.compile(r"[一-鿿]{2,}")
convert = None


def runs(texts):
    global convert
    if convert is None:
        convert = opencc.OpenCC("s2tw")
    out = []
    for text in texts:
        out.extend(HAN_RUNS.findall(convert.convert(text)))
    return "\n".join(out) + "\n" if out else ""


def batches(paths):
    for path in paths:
        for batch in pq.ParquetFile(path).iter_batches(batch_size=2000, columns=["text"]):
            yield batch.column("text").to_pylist()


if __name__ == "__main__":
    docs = 0
    with Pool() as pool:
        for chunk in pool.imap(runs, batches(sys.argv[1:]), chunksize=1):
            sys.stdout.write(chunk)
            docs += 2000
            if docs % 200000 == 0:
                print(f"{docs} documents", file=sys.stderr, flush=True)
