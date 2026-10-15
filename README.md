# 輕注音輸入法 Light Bopomofo

把 iPhone 注音的「簡拼」打法搬到電腦：每個字只打開頭的注音，就能選出整個詞。

```
ㄋㄏㄇ   → 你好嗎
ㄍㄗㄈㄈ → 工作方法
```

## 下載

**macOS 13 以上（Apple Silicon 與 Intel）**：到 [lightbopomofo.noky.dev](https://lightbopomofo.noky.dev) 或 [GitHub Releases](https://github.com/nvlznn/light-bopomofo/releases/latest) 下載 `LightBopomofo.dmg`，把「輕注音」拖到「應用程式」並打開，它會自動安裝並啟用輸入法。接著在選單列的輸入法選單選「輕注音」。

Windows 版開發中。

完全不連網，選字紀錄只存在本機，可以在「輕注音」App 裡查看或刪除。

## 開發

```sh
scripts/fetch-lexicon.sh          # 下載詞庫到 data/
cargo test
cargo run --release -p lightbopomofo-cli -- ㄋㄏㄇ ㄍㄗㄈㄈ
cargo run --release -p lightbopomofo-cli -- --dachen sca   # 用大千鍵位輸入
cargo run --release -p lightbopomofo-cli                   # 互動模式；以 : 開頭的行是大千鍵位
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
log stream --info --predicate 'subsystem == "dev.noky.inputmethod.lightbopomofo"'
```

使用者紀錄存在 `~/Library/Application Support/LightBopomofo/history.tsv`，只在本機；App 刪除紀錄後會通知輸入法重新載入。

## 發布新版（macOS）

1. 改 `macos/App/Info.plist` 的 `CFBundleShortVersionString`（輸入法會自動跟著）與 `CFBundleVersion`，更新 `macos/RELEASE_NOTES.md`。
2. 在「終端機」App 執行 `macos/release.sh`（第一次要對鑰匙圈的詢問按「永遠允許」）。它會簽署、公證、staple，產生 `dist/LightBopomofo.dmg`。
3. 用 git 建 tag 再建 Release（`gh` 沒有 `workflow` 權限時無法替含 CI 設定的 commit 建 tag）：
   ```sh
   git tag -a vX.Y.Z -m "輕注音 X.Y.Z" && git push origin vX.Y.Z
   gh release create vX.Y.Z dist/LightBopomofo.dmg --verify-tag --title "輕注音 X.Y.Z" --notes-file macos/RELEASE_NOTES.md
   ```
   檔名固定為 `LightBopomofo.dmg`，官網的 `releases/latest/download/LightBopomofo.dmg` 連結就不用改。

官網（`website/`）在 push 到 `main` 時由 Vercel 自動部署。

## 授權

程式碼：MIT。詞庫：[libchewing-data](https://github.com/chewing/libchewing-data)（LGPL-2.1-or-later），不包含在本 repo 中。
