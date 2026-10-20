# 輕注音輸入法 Light Bopomofo

把 iPhone 注音的「簡拼」打法搬到電腦：每個字只打開頭的注音，就能選出整個詞、整句話。

```
ㄋㄏㄇ   → 你好嗎
ㄉㄅㄑ   → 對不起
ㄕㄖㄎㄌ → 生日快樂
```

想打得更準時，多打幾個注音或加上聲調，候選就會縮小。輸入法會記住你的選擇，常打的字詞越用越前面。

### 中英混打

不用切換輸入法：

- **常見的英文詞直接打**：`google`、`iphone`、`email` 照英文的鍵打，就會出現 Google、iPhone、email，也能接在句子裡（ㄋㄏ + `app` → 你好app）。英文的鍵常落在韻母上（app 是 ㄇㄣㄣ），簡拼幾乎都是聲母，所以兩者很少互相搶。詞表在 `crates/lightbopomofo-core/english.csv`。
- **大寫字母開始英文**：Shift+字母開始一段英文，直到空白、Enter 或標點：`iPhone`、`API`、`5G`、`Node.js`。英文前面的注音照樣轉換（ㄋㄏ + `API` → 你好API）；英文從哪裡開始猜錯時，按 Tab 換。
- **其他小寫英文**：打的鍵本身是第一頁最後一個候選。
- **一次選好**：英文把句子切開時，第一頁會有換掉中文那段的整句（ㄋㄏ + `app` 有 女孩app，也有 你好app），選一次就好，選過的每一段都會記住。
- **會記住**：用過的英文下次直接用小寫打就會出現（打過一次 `Figma`，之後 `figma` 就是 Figma）。英文只存在本機，不會被「分享新詞」傳出去。

和簡拼分不開的英文（yes 是 ㄗㄍㄋ、bug 是 ㄖㄧㄕ）不會自動出現，用 Shift 打過一次之後，單獨打才會出現。

## 下載

**macOS 13 以上（Apple Silicon 與 Intel）**：到 [lightbopomofo.noky.dev](https://lightbopomofo.noky.dev) 或 [GitHub Releases](https://github.com/nvlznn/light-bopomofo/releases/latest) 下載 `LightBopomofo.dmg`，把「輕注音」拖到「應用程式」並打開，它會自動安裝並啟用輸入法。接著在選單列的輸入法選單選「輕注音」。

**Windows 10/11**：同樣從官網或 [GitHub Releases](https://github.com/nvlznn/light-bopomofo/releases/latest) 下載 `LightBopomofo-Setup.exe` 並執行。安裝檔還沒有程式碼簽署，看到「Windows 已保護您的電腦」時按「其他資訊」→「仍要執行」。裝完按 Windows 鍵 + 空白鍵切換到「輕注音」；單按 Shift 切換中／英。

輸入法本身不連網，選字紀錄存在本機，可以在「輕注音」App 裡查看或刪除。macOS 的「輕注音」App 會：
- 在背景向 GitHub 查新版本並自動安裝（可以關閉「自動更新」）；
- 預設「分享新詞」：把你選過、詞庫沒有的詞連同打的注音和次數，以隨機裝置編號傳給我們，審核後加進詞庫（可以在打字教學最後一頁或 App 關閉）。詳見[隱私權政策](https://lightbopomofo.noky.dev/privacy.html)。

Windows 有新版本時跳出通知，按一下就更新。

## 按鍵

| 按鍵 | 作用 |
|---|---|
| ← → Tab | 選候選；可以只選前段，剩下的接著選 |
| Shift + 1–9 | 直接送出這一頁的第幾個 |
| 按過 Tab／方向鍵／換頁之後的 1–9 | 直接送出這一頁的第幾個（像唯音的選字窗）；空白鍵送出選中的；打其他注音就回到打字 |
| [ ] PageUp PageDown | 換頁 |
| Shift + 字母 | 開始一段英文，空白、Enter 或標點結束 |

## 開發

```sh
scripts/fetch-lexicon.sh          # 下載詞庫到 data/
cargo test
cargo run --release -p lightbopomofo-cli -- ㄋㄏㄇ ㄍㄗㄈㄈ
cargo run --release -p lightbopomofo-cli -- --dachen sca   # 用大千鍵位輸入
cargo run --release -p lightbopomofo-cli                   # 互動模式；以 : 開頭的行是大千鍵位
cargo run --release -p lightbopomofo-cli -- --eval eval/tatoeba.tsv   # 正確率（--no-pairs 比較不用詞組）
scripts/gen-mixed-eval.py                                              # 產生 eval/mixed.tsv（句子前後接英文詞）和 eval/mixed-middle.tsv（英文在句子中間）
cargo run --release -p lightbopomofo-cli -- --eval eval/mixed.tsv      # 中英混打正確率
```

詞組與語料詞頻（`crates/lightbopomofo-core/word-pairs.tsv`）的產生方式見 `scripts/corpus-text.py`、`count-words`、`scripts/word-pairs.py`。

### 網站與新詞審核

`website/` 是官網（Vercel，Root Directory 為 `website`），`website/api/` 是「分享新詞」的 API，資料存在 Neon Postgres。

```sh
cd website && npm install && npm test        # API 測試（PGlite，不需要資料庫）
node test/dev-server.js                      # 本機 http://localhost:3333/admin.html，密碼 dev
node scripts/admin-password.mjs              # 產生新密碼與 ADMIN_PASSWORD_HASH
LB_ADMIN_PASSWORD=… scripts/approved-words.py   # 審核通過的詞寫進 lexicon-adjustments.csv
```

## 安裝到 macOS（開發版）

```sh
macos/build.sh        # 編出 macos/build/輕注音.app（通用版、本機簽署）
macos/install.sh      # 只安裝 App 內含的輸入法到 ~/Library/Input Methods 並啟用
macos/test.sh         # 輸入邏輯與安裝程式的測試（不會動到系統）
macos/package-dmg.sh  # 產生 dist/LightBopomofo.dmg
macos/release.sh      # 簽署、公證、staple；加 --publish 建立 GitHub Release
```

使用者拿到的是「輕注音」App（`macos/App/`，SwiftUI），放在「應用程式」。輸入法本體（`macos/IME/`）包在 App 裡的 `Contents/Library/Input Methods/`；App 打開時會把它安裝到 `~/Library/Input Methods`，也可以查看、刪除選字紀錄。兩者共用 `macos/Shared/`（引擎包裝、安裝程式）。版本號只在 `macos/App/Info.plist` 設定。

安裝後在選單列的輸入法選單選「輕注音」（系統語言為英文時顯示 Light Bopomofo）。
若沒有出現，登出再登入一次。除錯 log：

```sh
/usr/bin/log stream --info --predicate 'subsystem == "dev.noky.inputmethod.lightbopomofo"'
```

使用者紀錄存在 `~/Library/Application Support/LightBopomofo/history.tsv`，只在本機；App 刪除紀錄後會通知輸入法重新載入。

## Windows 版（開發）

Windows 版全部是 Rust，實作計畫見 [docs/WINDOWS_PLAN.md](docs/WINDOWS_PLAN.md)：

- `crates/lightbopomofo-windows`：輸入法 DLL（TSF text service），編成 `lightbopomofo.dll`
- `crates/lightbopomofo-settings`：App（`LightBopomofo.exe`），狀態與選字紀錄
- `windows/installer.iss`：Inno Setup 安裝程式；`windows/build.ps1` 建置全部並產生 `dist\LightBopomofo-Setup.exe`

在 Windows 電腦上（需要 Rust 與 Visual Studio 的 C++ 建置工具，含 ARM64）：

```powershell
powershell -ExecutionPolicy Bypass -File windows\build.ps1
```

在 Mac 上可以做型別檢查：`cargo check -p lightbopomofo-windows -p lightbopomofo-settings --target x86_64-pc-windows-msvc`。

工作列上的「輕」是語言列的輸入模式按鈕（`langbar.rs`），依工作列深淺色顯示白字 `windows/lightbopomofo-input.ico` 或黑字 `windows/lightbopomofo-input-light.ico`，兩者和 App 圖示都由 `scripts/make-icon.swift` 產生。

## 發布新版（macOS）

1. 改版本號：每次發佈升一個大版本（1.0 → 2.0 → 3.0）。`macos/App/Info.plist` 的 `CFBundleShortVersionString` 寫 `2.0`（輸入法會自動跟著，標籤就是 `v2.0`），`CFBundleVersion` 加 1；`Cargo.toml` 的 `version` 寫 `2.0.0`（Windows 版用它，Cargo 規定要三段）。更新 `macos/RELEASE_NOTES.md`。
2. 在「終端機」App 執行 `macos/release.sh`（第一次要對鑰匙圈的詢問按「永遠允許」）。它會簽署、公證、staple，產生 `dist/LightBopomofo.dmg`。
3. commit 並 push 後，在「終端機」執行 `macos/release.sh --publish`：它會用 git 建立並推送 tag（`gh` 沒有 `workflow` 權限時無法替含 CI 設定的 commit 建 tag），再建立 GitHub Release 並附上 dmg。
   檔名固定為 `LightBopomofo.dmg`，官網的 `releases/latest/download/LightBopomofo.dmg` 連結就不用改。

Release 發布後，`.github/workflows/release.yml` 會建置 Windows 安裝檔並附加到同一個 Release（`LightBopomofo-Setup.exe`；GitHub 專案變數 `WINDOWS_RELEASE` 已設為 `true`）。只有 Windows 版需要修正時，不必重發 macOS：把 tag 移到修正後的 commit，再執行 `gh workflow run release.yml -f tag=v2.0` 重建安裝檔。

## 官網

官網（`website/`）是靜態頁加上「分享新詞」的 API（`website/api/`，見上方「網站與新詞審核」），push 到 `main` 時由 Vercel 自動部署。

```sh
scripts/serve-website.py          # 本機預覽 http://127.0.0.1:8765/，存檔後自動重新整理
scripts/gen-website-data.py       # 詞庫或引擎改了之後重跑，產生 website/candidates.js
```

網頁上的示範候選全部來自真引擎：`gen-website-data.py` 呼叫 `target/release/lb-cli`（先 `cargo build --release -p lightbopomofo-cli`），把結果寫進 `website/candidates.js`。MacBook 示範打的名言連同每段的打法也寫在這支腳本裡，產生時會逐段驗證；詞庫改變後某段打不出來，它會直接報錯。

## 授權

程式碼：MIT。詞庫：[libchewing-data](https://github.com/chewing/libchewing-data)（LGPL-2.1-or-later），不包含在本 repo 中。
