# 輕注音輸入法 Light Bopomofo

把 iPhone 注音的「簡拼」打法搬到電腦：每個字只打開頭的注音，就能選出整個詞。

```
ㄋㄏㄇ   → 你好嗎
ㄍㄗㄈㄈ → 工作方法
```

目前是 MVP 開發中，見 [docs/MVP.md](docs/MVP.md)。

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
macos/build.sh      # 編出 macos/build/LightBopomofo.app
macos/install.sh    # 安裝到 ~/Library/Input Methods 並啟用
macos/test.sh       # 輸入邏輯的測試（不需要安裝）
```

安裝後在選單列的輸入法選單選「輕注音」（系統語言為英文時顯示 Light Bopomofo）。
若沒有出現，登出再登入一次。除錯 log：

```sh
log stream --info --predicate 'subsystem == "dev.noky.inputmethod.lightbopomofo"'
```

使用者紀錄存在 `~/Library/Application Support/LightBopomofo/history.tsv`，只在本機。

## 授權

程式碼：MIT。詞庫：[libchewing-data](https://github.com/chewing/libchewing-data)（LGPL-2.1-or-later），不包含在本 repo 中。
