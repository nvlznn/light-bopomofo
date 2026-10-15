**輕注音 2.0**：Windows 的候選視窗換上和 macOS 一致的外觀。

## 下載
- **macOS 13 以上**（Apple Silicon 與 Intel）：`LightBopomofo.dmg`，把「輕注音」拖到「應用程式」並打開。
- **Windows 10／11**：`LightBopomofo-Setup.exe`。安裝檔還沒有程式碼簽署，Windows 會顯示「Windows 已保護您的電腦」，按「其他資訊」→「仍要執行」。裝完按 Windows 鍵 + 空白鍵切換到「輕注音」。

已經裝了 1.0 的話，會跳出更新通知，按「更新」就好。

## 打法（兩個平台相同）
- 每個字只打第一個注音：ㄋㄏㄇ → 你好嗎、ㄉㄅㄑ → 對不起
- 可以連打整句，也可以只選前段，剩下的繼續選
- 聲調可打可不打（空白鍵 3 4 6 7 是 ˉ ˇ ˋ ˊ ˙）
- Enter 送出，← → 或 Tab 選字，Shift + 1–9 直接選這一頁的第幾個，[ ] 或 PageUp／PageDown 換頁，Esc 清空
- Shift + , . / 1 ; 打出 ，。？！：
- Windows：單按 Shift 切換中／英；兩個平台都可以用 Caps Lock 暫時打英文（切換時還沒選的注音會清掉）
- 會記住你的選擇，紀錄只存在本機，可以在「輕注音」App 查看、刪除

## 這一版的改進
- Windows：候選視窗改用和 macOS 一致的配色，候選字改成半粗體，更好讀

## 已知限制
- Arm 版 Windows 上，以模擬方式執行的 x64 程式暫時還不能使用輕注音（Arm 原生程式可以）。

輸入法完全不連網，不傳送任何打字內容；只有「輕注音」App 每天向 GitHub 查一次有沒有新版本（不傳送任何資料，可以在 App 關閉「自動檢查更新」）。詞庫：新酷音（libchewing）內建詞庫，LGPL-2.1-or-later，詳見 DATA_LICENSES.md。
