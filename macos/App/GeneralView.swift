import SwiftUI

struct GeneralView: View {
    @EnvironmentObject private var model: AppModel
    @State private var confirmingUninstall = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            StatusCard()

            GroupBox("打法") {
                VStack(alignment: .leading, spacing: 8) {
                    Text("每個字只打第一個注音，例如 ㄋㄏㄇ → 你好嗎、ㄍㄗㄈㄈ → 工作方法。也可以打完整注音或加上聲調來縮小範圍。")
                        .fixedSize(horizontal: false, vertical: true)
                    Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                        KeyRow(keys: "Enter", action: "送出選中的候選")
                        KeyRow(keys: "← →／Tab", action: "選擇候選；可以只選前段，剩下的接著選")
                        KeyRow(keys: "Shift + 1–9", action: "直接送出這一頁的第幾個候選")
                        KeyRow(keys: "[ ]／PageUp PageDown", action: "上一頁／下一頁候選")
                        KeyRow(keys: "空白 3 4 6 7", action: "聲調 ˉ ˇ ˋ ˊ ˙（可省略）")
                        KeyRow(keys: "Backspace／Esc", action: "刪除一個注音／清空")
                        KeyRow(keys: "Shift + , . / 1 ;", action: "，。？！：（組字中 Shift + 1 是選字）")
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(6)
            }

            Spacer()

            HStack {
                Text("不連網，選字紀錄只存在這台 Mac。")
                    .foregroundStyle(.secondary)
                Spacer()
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
        case .notEnabled: return "到「系統設定 → 鍵盤 → 輸入方式」按「＋」，在「繁體中文」底下加入「輕注音」。"
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
