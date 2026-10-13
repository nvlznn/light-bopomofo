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
| 本專案的使用方式 | 原檔**不修改**，以獨立檔案的形式放在 App 內（`LightBopomofo.app/Contents/Resources/tsi.csv`）。引擎在啟動時讀取，並在記憶體中重新排序同音單字。使用者可以自行換成其他版本的 `tsi.csv` |
| 標示方式 | 安裝檔內附本文件、LGPL-2.1 全文與原始出處；官網與 README 也註明詞庫來源 |

### 為什麼可以和 MIT 程式碼一起發布
- 詞庫是獨立的資料檔，引擎在執行時才讀取。它沒有被編譯進程式，也沒有和 MIT 程式碼合併成同一個作品。
- 散布時一併附上 LGPL 全文、著作權聲明和原始出處（上表的 repo 與 commit）。詞庫沒有修改，原始形式就是使用者拿到的那個檔案。
- 使用者可以直接替換 `tsi.csv`，這符合 LGPL 允許使用者修改、替換函式庫部分的精神。

## 不隨安裝檔散布的資料
- repo 不包含 `tsi.csv`。建置時由 `scripts/fetch-lexicon.sh` 從上述固定的 commit 下載。
