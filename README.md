# 輕注音輸入法 Light Bopomofo

每個字只打第一個注音，就能打出整句。

```
ㄋㄏㄇ   → 你好嗎
ㄕㄖㄎㄌ → 生日快樂
```

## 下載

[lightbopomofo.noky.dev](https://lightbopomofo.noky.dev) 或 [GitHub Releases](https://github.com/nvlznn/light-bopomofo/releases/latest)。

- **macOS 13+**：`LightBopomofo.dmg`，拖到「應用程式」並打開。
- **Windows 10/11**：`LightBopomofo-Setup.exe`。尚未簽署，按「其他資訊」→「仍要執行」。

輸入法不連網。macOS App 會自動更新、預設分享新詞（[隱私權政策](https://lightbopomofo.noky.dev/privacy)）。

## 按鍵

| 按鍵 | 作用 |
|---|---|
| ← → Tab | 選字 |
| Shift + 1–9 | 直接選 |
| [ ] PageUp PageDown | 換頁 |
| ↓ | 展開更多候選（macOS） |

## 開發

```sh
scripts/fetch-lexicon.sh                                  # 下載詞庫到 data/
cargo test
cargo run --release -p lightbopomofo-cli -- ㄋㄏㄇ          # 看候選；--dachen 用大千鍵位
cargo run --release -p lightbopomofo-cli -- --eval eval/tatoeba.tsv   # 正確率
```

詞組資料（`word-pairs.tsv`）：`scripts/corpus-text.py` → `count-words` → `scripts/word-pairs.py`。

### macOS

```sh
macos/build.sh      # macos/build/輕注音.app
macos/install.sh    # 安裝輸入法到 ~/Library/Input Methods
macos/test.sh
```

App 在 `macos/App/`，輸入法在 `macos/IME/`，共用程式在 `macos/Shared/`。版本號只在 `macos/App/Info.plist`。
Log：`/usr/bin/log stream --info --predicate 'subsystem == "dev.noky.inputmethod.lightbopomofo"'`

### Windows

全部是 Rust：`crates/lightbopomofo-windows`（輸入法 DLL）、`crates/lightbopomofo-settings`（App）、`windows/installer.iss`。

```powershell
powershell -ExecutionPolicy Bypass -File windows\build.ps1
```

### 官網與新詞審核

`website/`，push 到 `main` 由 Vercel 部署。`website/api/` 是分享新詞的 API（Neon Postgres）。

```sh
scripts/gen-website-data.py                      # 引擎改了之後重跑
cd website && npm install && npm test
node test/dev-server.js                          # 本機審核頁，密碼 dev
LB_ADMIN_PASSWORD=… scripts/approved-words.py    # 通過的詞寫進詞庫
```

## 發布

1. 版本號升一個大版本：`macos/App/Info.plist`（`CFBundleShortVersionString` 如 `4.0`，`CFBundleVersion` +1）、`Cargo.toml`（`4.0.0`）。改 `macos/RELEASE_NOTES.md`。
2. push 後在「終端機」執行 `macos/release.sh`，再 `macos/release.sh --publish`。
3. Windows 安裝檔由 CI 自動附上；只修 Windows 時：`gh workflow run release.yml -f tag=v4.0`。

## 授權

程式碼 MIT。詞庫 [libchewing-data](https://github.com/chewing/libchewing-data)（LGPL-2.1+）；詞組 FineWeb-2（ODC-By）。見 [DATA_LICENSES.md](DATA_LICENSES.md)。

### 後台詞庫查詢與發布

後台新增詞前會比對內建詞庫索引與正式詞庫資料庫，查無結果才允許新增；相同詞即使讀音不同也不能重複新增。批次操作只影響目前清單勾選的項目，不會隱含選取其他篩選結果。

正式詞庫變更仍需同步到下一版 App。發布前依序執行：

```sh
python3 scripts/approved-words.py
python3 scripts/gen-admin-lexicon.py
npm --prefix website test
```

`gen-admin-lexicon.py` 使用 `data/tsi.csv` 與 `lexicon-adjustments.csv`，更新後台查詢用的內建詞庫索引；可用 `--source` 指定來源。索引沿用新酷音詞庫的授權，詳見 `DATA_LICENSES.md`。詞庫調整後未更新索引會讓測試失敗，避免查詢結果與發布內容不同步。
