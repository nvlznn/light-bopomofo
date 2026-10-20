import SwiftUI

struct GeneralView: View {
    @EnvironmentObject private var model: AppModel
    @State private var confirmingUninstall = false
    @AppStorage(OnboardingView.doneKey) private var tutorialDone = true

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            if model.update != .idle {
                UpdateBar()
            }
            StatusCard()

            GroupBox("打法") {
                VStack(alignment: .leading, spacing: 8) {
                    Text("每個字只打第一個注音：ㄋㄏㄇ → 你好嗎")
                    Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                        KeyRow(keys: "Enter", action: "送出")
                        KeyRow(keys: "← →", action: "選字")
                        KeyRow(keys: "Shift + 1–9", action: "直接選第幾個")
                        KeyRow(keys: "[ ]", action: "換頁")
                        KeyRow(keys: "Esc", action: "清空")
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(6)
            }

            Spacer()

            HStack {
                Toggle("自動更新", isOn: $model.autoCheck)
                    .help("有新版本時在背景下載並安裝，完成後通知你。只向 GitHub 查詢版本，不傳送任何資料。")
                Toggle("分享新詞", isOn: $model.shareWords)
                    .help("把你選過、詞庫沒有的詞（連同打的注音和次數）傳給我們，審核後加進詞庫。不傳送前後文。裝置編號：\(WordSharing.deviceID)")
                Spacer()
                Button("打字教學") { tutorialDone = false }
                    .buttonStyle(.link)
                Link("隱私權", destination: WordSharing.privacyURL)
                Link("官網", destination: URL(string: "https://lightbopomofo.noky.dev")!)
                Link("回報問題", destination: URL(string: "https://github.com/nvlznn/light-bopomofo/issues")!)
                Button("解除安裝…", role: .destructive) { confirmingUninstall = true }
                    .disabled(model.status == .notInstalled)
            }
            .font(.callout)
        }
        .confirmationDialog("解除安裝輕注音？", isPresented: $confirmingUninstall) {
            Button("解除安裝", role: .destructive) { model.uninstall() }
        } message: {
            Text("輸入法會被停用並移到垃圾桶，選字紀錄會保留。之後可以把這個 App 從「應用程式」移到垃圾桶。")
        }
    }
}

private struct UpdateBar: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: failed ? "exclamationmark.triangle.fill" : "arrow.down.circle.fill")
                .font(.system(size: 22))
                .foregroundStyle(failed ? Color.orange : Color.accentColor)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.headline)
                Text(detail)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
            switch model.update {
            case .updating:
                ProgressView().controlSize(.small)
            case .failed:
                Link("到官網下載", destination: URL(string: "https://lightbopomofo.noky.dev")!)
                Button("再試一次") { model.checkForUpdate(thenInstall: true) }
            case .available:
                Button("更新") { Task { await model.installUpdate() } }
                    .keyboardShortcut(.defaultAction)
            case .idle:
                EmptyView()
            }
        }
        .padding(12)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
    }

    private var failed: Bool {
        if case .failed = model.update { return true }
        return false
    }

    private var title: String {
        switch model.update {
        case .available(let v): return "有新版本：輕注音 \(v)"
        case .updating(let v): return v.isEmpty ? "正在下載新版本…" : "正在下載輕注音 \(v)…"
        case .failed: return "無法更新"
        case .idle: return ""
        }
    }

    private var detail: String {
        switch model.update {
        case .available: return "目前是 \(model.bundledVersion)。按「更新」下載並安裝，輸入法會一起更新。"
        case .updating: return "完成後輕注音會自動重新開啟。"
        case .failed(let message): return message
        case .idle: return ""
        }
    }
}

private struct StatusCard: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: symbol)
                .font(.system(size: 28))
                .foregroundStyle(color)
                .frame(width: 34)
            VStack(alignment: .leading, spacing: 6) {
                Text(title).font(.title3.weight(.semibold))
                Text(detail)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                HStack {
                    switch model.status {
                    case .notEnabled:
                        Button("打開輸入方式設定") { model.openInputSourceSettings() }
                            .keyboardShortcut(.defaultAction)
                    case .notInstalled, .failed:
                        Button("安裝") { model.install() }
                            .keyboardShortcut(.defaultAction)
                    case .ready, .newerInstalled, .checking:
                        Button("打開輸入方式設定") { model.openInputSourceSettings() }
                    }
                    if case .ready = model.status {
                        Button("重新安裝") { model.install() }
                    }
                }
                .padding(.top, 4)
            }
            Spacer(minLength: 0)
        }
        .padding(16)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
    }

    private var title: String {
        switch model.status {
        case .checking: return "正在檢查…"
        case .ready(let v): return "輕注音 \(v) 可以使用了"
        case .notEnabled(let v): return "輕注音 \(v) 已安裝，還沒加入輸入方式"
        case .newerInstalled(let installed, _): return "已安裝較新的版本 \(installed)"
        case .notInstalled: return "輸入法尚未安裝"
        case .failed: return "安裝失敗"
        }
    }

    private var detail: String {
        switch model.status {
        case .checking: return ""
        case .ready: return "在選單列的輸入法選單選「輕注音」就可以開始打字。"
        case .notEnabled: return "手動加入：① 打開「系統設定」→「鍵盤」，在「文字輸入」按「輸入方式」旁的「編輯…」 ② 按左下角「＋」，左邊選「中文（繁體）」 ③ 選「輕注音」按「加入」。加好這裡會自動更新。"
        case .newerInstalled(_, let bundled): return "這個 App 內含的是 \(bundled)，不會覆蓋較新的版本。請改用新版的 App。"
        case .notInstalled: return "按「安裝」把輸入法裝到你的使用者資料夾，不需要管理員密碼。"
        case .failed(let message): return message
        }
    }

    private var symbol: String {
        switch model.status {
        case .ready: return "checkmark.circle.fill"
        case .failed: return "xmark.octagon.fill"
        case .checking: return "hourglass"
        default: return "exclamationmark.circle.fill"
        }
    }

    private var color: Color {
        switch model.status {
        case .ready: return .green
        case .failed: return .red
        case .checking: return .secondary
        default: return .orange
        }
    }
}

private struct KeyRow: View {
    let keys: String
    let action: String

    var body: some View {
        GridRow {
            Text(keys).font(.callout.monospaced()).foregroundStyle(.secondary)
            Text(action)
        }
    }
}
