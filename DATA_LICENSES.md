# 資料來源與授權

輕注音的**程式碼**採用 MIT 授權（見 [LICENSE](LICENSE)）。
輸入法安裝檔另外**附帶**下面這份資料。它保有自己的授權，不受 MIT 授權約束。

> 這份整理是工程上的紀錄，不構成法律意見。

## 詞庫：新酷音（libchewing）內建詞庫 `tsi.csv`

| 項目 | 內容 |
|---|---|
| 來源 | https://github.com/chewing/libchewing-data — `dict/chewing/tsi.csv` |
| 版本 | commit `ea74f76dd2548b1d65d0ff70c3ae66057a6ad97d`（檔頭 `dc:identifier` 2025.8.19） |
| 著作權 | Copyright (c) 2025 libchewing Core Team（檔頭 `dc:rights`） |
| 授權 | **GNU Lesser General Public License v2.1 or later**（檔頭 `dc:license`）。repo 本身沒有授權檔，授權全文取自 libchewing 的 [COPYING](https://github.com/chewing/libchewing/blob/master/COPYING)，收錄於 [licenses/LGPL-2.1.txt](licenses/LGPL-2.1.txt) |
| 可否修改、再散布 | 可以，須遵守 LGPL-2.1 的條件 |
| 本專案的使用方式 | 建置時由 `build-lexicon` 把原檔編譯成二進位索引 `lexicon.bin`（含同音單字的重新排序），引擎以記憶體映射讀取。編譯時套用輕注音的少量調整 [`lexicon-adjustments.csv`](crates/lightbopomofo-core/lexicon-adjustments.csv)（加入或刪除個別詞，例如加入「什麼意思」、刪除「史密斯」），所以 `lexicon.bin` 是**修改過的版本**。**原檔不修改，與調整檔一併隨安裝檔散布**（macOS：輸入法內的 `Contents/Resources/Licenses/`；Windows：安裝資料夾的 `licenses\`）。使用者可以用 `build-lexicon` 從任何版本的 `tsi.csv` 重新產生 `lexicon.bin` 並替換 |
| 標示方式 | 安裝檔內附本文件、LGPL-2.1 全文與原始出處；官網與 README 也註明詞庫來源 |

### 為什麼可以和 MIT 程式碼一起發布
- 詞庫是獨立的資料檔，引擎在執行時才讀取。它沒有被編譯進程式，也沒有和 MIT 程式碼合併成同一個作品。
- 散布時一併附上 LGPL 全文、著作權聲明、原始出處（上表的 repo 與 commit）、**未修改的原始資料檔 `tsi.csv`**，以及記錄所有修改的 `lexicon-adjustments.csv`；`lexicon.bin` 是從這兩個檔案產生的索引形式，產生工具（`crates/build-lexicon`）也開源。
- 使用者可以自行重新產生並替換 `lexicon.bin`，這符合 LGPL 允許使用者修改、替換函式庫部分的精神。

## 詞組與語料詞頻：`word-pairs.tsv`

| 項目 | 內容 |
|---|---|
| 來源 | FineWeb-2 國語語料 `cmn_Hani`（https://huggingface.co/datasets/HuggingFaceFW/fineweb-2）其中兩個檔，以 OpenCC 轉為台灣繁體 |
| 授權 | 資料集本身：**Open Data Commons Attribution License (ODC-By) v1.0**，要求標示出處；原始網頁另受 Common Crawl 使用條款約束 |
| 本專案的使用方式 | 只散布**統計數字**，不散布任何原文：以新酷音詞庫斷詞後，每個詞的出現次數，以及常一起出現的前後詞與其點互資訊（前 200 萬組）。產生方式見 `scripts/corpus-text.py`、`count-words`、`scripts/word-pairs.py`。`build-lexicon` 把它編進 `lexicon.bin`：詞頻與新酷音的頻率各半混合，詞組用來替整句轉換加分 |
| 標示方式 | 本文件；官網與 README |

## 不隨安裝檔散布的資料
- repo 不包含 `tsi.csv`。建置時由 `scripts/fetch-lexicon.sh` 從上述固定的 commit 下載。
- `eval/tatoeba.tsv`：評估用的日常短句，取自 Tatoeba（https://tatoeba.org，CC BY 2.0 FR），由 `scripts/gen-eval.py` 產生，只用於 `lb-cli --eval`。
