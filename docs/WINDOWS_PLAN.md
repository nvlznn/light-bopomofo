# 輕注音 Windows 版實作計畫

> 目標：Windows 10/11 使用者從 [lightbopomofo.noky.dev](https://lightbopomofo.noky.dev) 下載安裝檔，裝完就能用和 macOS 版一樣的打法（簡拼、整段轉換、選前段、聲調可省略、會學習），也有管理選字紀錄的 App。

## 已確定的決定（沿用原計畫與 macOS 版）

| 項目 | 決定 |
|---|---|
| 語言 | **全部 Rust**：輸入法 DLL、設定 App、安裝流程輔助程式 |
| Windows API | 微軟官方 [`windows` crate](https://github.com/microsoft/windows-rs)，實作 TSF（Text Services Framework）的 COM 介面 |
| 引擎 | 直接以 Rust crate 依賴 `lightbopomofo-core`，**不經過 C ABI** |
| 簽署 | 第一版**不簽署**；官網說明如何略過 SmartScreen 警告。有使用者之後再買憑證 |
| 測試機 | Ino 的實體 Windows 電腦（x64） |
| 資料 | 完全本機、不連網；選字紀錄格式與 macOS 相同（`history.tsv`） |

## 和 macOS 版最大的不同：DLL 會被載入到每個程式

Windows 輸入法（TSF 的 Text Input Processor）是一個 COM DLL，**每個打字的程式都會載入一份**。這帶來三個 macOS 沒有的要求：

1. **詞庫不能每個程式各解析一次。** 現在的引擎啟動時解析 5 MB 文字詞庫（約 0.1 秒、約 50 MB 記憶體）。開十個程式就是十份。→ 改成**預先建好的二進位詞庫，用記憶體映射（mmap）開啟**：不用解析、作業系統讓所有程式共用同一份記憶體分頁。
2. **選字紀錄由很多程式同時讀寫。** 在記事本學到的字，切到瀏覽器也要有效；設定 App 刪掉的紀錄，所有程式都要忘記。→ 每次開始組字時檢查紀錄檔的修改時間，有變才重讀；寫入用附加模式；整檔重寫要處理 Windows 的檔案共用鎖。
3. **任何錯誤都會拖垮別人的程式。** panic 跨出 DLL 等於讓使用者的 Word 當掉。→ 每個 COM 方法都用 `catch_unwind` 包住；按鍵處理中不做檔案寫入以外的耗時工作，寫入失敗一律忽略。

## 架構

```
crates/
├── lightbopomofo-core/        // 既有：加上二進位詞庫、Composer（見 W0）
├── lightbopomofo-ffi/         // 既有：只給 macOS
├── lightbopomofo-windows/     // 新：輸入法 DLL（cdylib）
│   └── src/
│       ├── lib.rs             // DllGetClassObject、DllCanUnloadNow、DllRegisterServer…
│       ├── register.rs        // COM 與 TSF profile／category 註冊
│       ├── tip.rs             // ITfTextInputProcessorEx：Activate／Deactivate
│       ├── keys.rs            // ITfKeyEventSink：虛擬鍵碼 → Composer 的 Key
│       ├── edit.rs            // ITfEditSession、組字（composition）與送出文字
│       ├── candidates.rs      // 候選視窗（Win32 分層視窗 + DirectWrite）
│       └── log.rs             // %LOCALAPPDATA%\LightBopomofo\logs
├── lightbopomofo-settings/    // 新：設定 App（輕注音.exe）
└── build-lexicon/             // 新：tsi.csv → 二進位詞庫（macOS 也改用）
windows/
├── installer.iss              // Inno Setup 腳本
└── build.ps1                  // 本機建置（Ino 的電腦上用）
```

### 輸入法 DLL 要實作的 TSF 介面

以微軟官方的 [SampleIME](https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/IME)（C++）為主要參考，並對照 [小狼毫 Weasel](https://github.com/rime/weasel)（C++，Rime 的 Windows 版）處理各種程式相容性的做法。

| 介面 | 用途 |
|---|---|
| `ITfTextInputProcessorEx` | 輸入法被切換到／切換走 |
| `ITfKeyEventSink` | 攔截按鍵：`OnTestKeyDown` 決定要不要處理，`OnKeyDown` 實際處理 |
| `ITfThreadMgrEventSink` | 使用者切換視窗或輸入框時，送出或清空組字 |
| `ITfCompositionSink` | 程式自己結束組字時（例如點到別處）同步狀態 |
| `ITfEditSession` + `ITfContextComposition` | 在游標處顯示注音（組字中、加底線），送出文字 |
| `ITfDisplayAttributeProvider` | 組字文字的底線樣式 |
| `ITfContextView::GetTextExt` | 取得游標位置，放候選視窗 |

註冊時要加上 `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT`、`GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT` 等類別，否則開始功能表搜尋、設定、Microsoft Store 程式裡不能用。DLL 要放在 `Program Files`，讓這些沙箱化的程式讀得到。

### 架構與位元數

同一份程式碼編三種 DLL，安裝程式依電腦全部裝上：

- **x64**：64 位元程式（大部分）
- **x86**：仍是 32 位元的程式（一些舊軟體、部分通訊軟體），沒有它這些程式裡打不了字
- **ARM64**：Windows on ARM 筆電（Snapdragon 等）

## 按鍵行為

和 macOS 版相同（空白／Enter 送出、← → Tab 選字、Backspace、Esc、Shift + , . / 1 ; 打全形標點、選前段後剩下的繼續轉換）。Windows 特有的部分：

- **中英切換**：Windows 使用者習慣**按一下 Shift** 切換中英（微軟注音、新酷音都是），加上 Caps Lock。→ 兩者都支援；Shift 單獨按下又放開才切換，Shift + 其他鍵照常。
- **大千鍵位用虛擬鍵碼（VK_*）判斷**，不依賴目前的鍵盤配置字元，換鍵盤配置也一樣是實體位置。
- 數字鍵盤的數字、Ctrl／Alt／Win 組合鍵交還程式（有組字時先送出）。

## 設定 App（輕注音.exe）

功能對齊 macOS 版：

- **一般**：輸入法是否已安裝、已加入、版本；按鈕打開「設定 → 時間與語言 → 語言與地區」；按鍵說明；官網與回報問題連結
- **選字紀錄**：列表、搜尋、排序、刪除單筆／多筆、全部清除；刪除後通知所有程式重讀

用 Rust + Win32 原生控制項（ListView 等）實作，不額外帶執行環境（例如 WebView、.NET）。自動支援高 DPI 與深色模式。

## 安裝與解除安裝

用 **Inno Setup** 產生 `LightBopomofo-Setup.exe`：

- 安裝到 `C:\Program Files\LightBopomofo\`（需要管理員權限，TSF 註冊與沙箱化程式的存取都需要）
- 依序註冊 x64／x86／ARM64 DLL，並把輕注音加入目前使用者的輸入法清單
- 開始功能表捷徑「輕注音」（設定 App）
- **更新時 DLL 正在被使用中**（每個開著的程式都載入了它）：舊 DLL 改名、新 DLL 放上、舊檔排程在重新開機時刪除，使用者不用先關掉所有程式
- 解除安裝：取消註冊、移除檔案；選字紀錄（`%APPDATA%\LightBopomofo\`）保留，並在畫面上說明位置

檔名固定為 `LightBopomofo-Setup.exe`，官網用 `releases/latest/download/LightBopomofo-Setup.exe`。

## 開發與測試流程

我在 Mac 上寫程式，Windows 的實際測試由 Ino 在自己的電腦上做：

1. 我 push 到 GitHub → GitHub Actions 在 Windows 上建置、跑測試、**產生安裝檔當作 CI 產物**
2. Ino 從該次 CI 頁面下載安裝檔、安裝、照測試清單操作
3. 有問題時，Ino 把 `%LOCALAPPDATA%\LightBopomofo\logs\` 的 log 傳給我（log 不記錄打字內容，只記錄事件與錯誤）

在 Mac 上我也能用 `cargo-xwin` 交叉編譯 Windows DLL 做快速檢查，但真正的驗證以 CI 與 Ino 的電腦為準。

## 目前進度

| | 狀態 |
|---|---|
| W0 引擎準備 | ✅ 完成（二進位詞庫、Rust Composer、多程式共用紀錄），macOS 也已改用 |
| W1+W2 輸入法 DLL | 程式碼完成，x64／x86／ARM64 都能編譯；**尚未在 Windows 上實測** |
| W3 安裝程式與 CI | `windows/installer.iss`、`windows/build.ps1`；CI 的 `windows-package` 產生安裝檔 |
| W4 設定 App | 程式碼完成、能編譯；**尚未在 Windows 上實測** |
| W5 相容性測試 | 等 Ino 在 Windows 上試用 |
| W6 發布 | W5 通過後 |

**已知限制（第一版）**：ARM64 電腦上，x64 程式（以模擬方式執行）和 ARM64 程式共用同一份 64 位元 COM 註冊，只能指向一個 DLL；第一版註冊 ARM64 版，所以 ARM 電腦上模擬執行的 x64 程式暫時無法使用輕注音（ARM64 原生程式與 32 位元程式可以）。要同時支援需改用 ARM64X 二進位，之後再評估。

## 里程碑

| | 內容 | 在哪裡驗證 | 預估 |
|---|---|---|---|
| **W0** | **引擎準備**（不需要 Windows）：① `build-lexicon` 產生二進位詞庫，引擎改用 mmap 讀取（macOS 也一起換，啟動更快）② 把按鍵狀態機 Composer 從 Swift 移到 Rust core（Windows 直接用；macOS 之後也可改用，兩邊行為一定一致）③ 紀錄檔的多程式讀寫 | Mac 上的 `cargo test`、CI | 2–3 天 |
| **W1** | **輸入法骨架**：DLL 能註冊、出現在 Windows 輸入法清單；在記事本打注音、空白鍵送出第一個候選；log | Ino 的電腦：記事本 | 3–5 天 |
| **W2** | **候選視窗與完整按鍵**：候選視窗（位置、高 DPI、深色模式）、選前段、標點、中英切換 | 記事本、Word、Chrome、Edge | 3–5 天 |
| **W3** | **安裝程式**：x64／x86／ARM64、更新時 DLL 使用中、解除安裝 | 乾淨的 Windows 帳號上安裝／更新／移除 | 2–3 天 |
| **W4** | **設定 App**：一般、選字紀錄 | Ino 的電腦 | 2–3 天 |
| **W5** | **相容性測試**（見下表），修各程式的問題 | Ino 的電腦 | 3 天以上 |
| **W6** | **發布**：CI 產生安裝檔、GitHub Release、官網 Windows 按鈕與 SmartScreen 說明 | 官網下載實測 | 1 天 |

合計約 **3–4 週**，主要時間在 W1–W5 的「改 → 裝 → 試」來回。

### W5 相容性測試清單

記事本、Word、Excel、Chrome、Edge、Firefox、VS Code、Windows Terminal、檔案總管的重新命名與搜尋、**開始功能表搜尋**、**設定 App**、Microsoft Store 程式、LINE、Discord、Teams、Slack、以系統管理員身分執行的程式、密碼欄位（系統會停用輸入法，確認不出錯）。

每一項確認：能打字、候選視窗位置正確、空白鍵送出、選前段、Backspace、中英切換。

## 風險

1. **TSF 是整個專案最難的部分**，Rust 的 TSF 範例很少。→ 嚴格照 SampleIME 的結構寫，每個介面有疑問就對照 C++ 原始碼。
2. **候選視窗位置**在 Electron 程式（VS Code、Discord、Slack）、終端機裡常算錯。→ W5 逐一修，參考小狼毫的處理方式。
3. **未簽署的 DLL**：SmartScreen 會警告安裝檔；少數開啟嚴格程式碼完整性保護的程式（例如部分遊戲的防作弊）會拒絕載入未簽署的 DLL，輕注音在那些程式裡無法使用。→ 官網說明；有使用者後買憑證。
4. **更新時 DLL 被鎖住**：處理不好就要求重新開機。→ W3 專門處理並測試。
5. **一個 bug 影響所有程式**：例如死結會讓輸入框卡住。→ 按鍵處理中不等待任何鎖或檔案；log 記錄每次例外。

## W0 細節（下一步要做的）

1. **二進位詞庫**
   - `build-lexicon` 讀 `tsi.csv`，輸出 `lexicon.bin`：詞條表、字首 trie、音節前綴表、同音字重排都在建置時做完
   - 引擎用 `memmap2` 開啟，查詢直接讀映射的位元組，不建任何 HashMap
   - 目標：開啟 < 5 ms、每個程式額外記憶體 < 5 MB；查詢速度不變（p95 < 10 ms）；所有現有測試照常通過
   - macOS 的 App 改帶 `lexicon.bin`，啟動也跟著變快
2. **Rust Composer**
   - 把 `macos/IME/Composer.swift` 的狀態機（`Key` → 組字、候選、送出、整段學習）移到 `lightbopomofo-core/src/composer.rs`
   - 移植 `macos/Tests/main.swift` 的所有情境成 Rust 測試
   - macOS 暫時維持 Swift 版；Windows 穩定後再評估讓 macOS 也改用 Rust 版（透過 C ABI），兩個平台就只剩一份按鍵邏輯
3. **多程式共用紀錄**
   - 組字開始時檢查 `history.tsv` 的修改時間，有變才重讀
   - Windows 上重寫檔案時處理其他程式正開著該檔的情況（共用模式、失敗重試）
   - 測試：兩個 Engine 同時寫同一個檔、一個刪除另一個看得到
