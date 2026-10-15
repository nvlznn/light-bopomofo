import SwiftUI

struct HistoryView: View {
    @EnvironmentObject private var model: AppModel
    @State private var selection = Set<HistoryEntry.ID>()
    @State private var search = ""
    @State private var sortOrder = [KeyPathComparator(\HistoryEntry.lastUsed, order: .reverse)]
    @State private var confirmingClear = false

    private var rows: [HistoryEntry] {
        let query = search.trimmingCharacters(in: .whitespaces)
        let filtered = query.isEmpty
            ? model.entries
            : model.entries.filter { $0.typed.contains(query) || $0.text.contains(query) }
        return filtered.sorted(using: sortOrder)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("輕注音會記住你選過的字，下次同樣的輸入就排在前面。選錯而一直排在前面的，可以在這裡刪掉。")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            HStack {
                TextField("搜尋注音或文字", text: $search)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 260)
                Spacer()
                Text("\(model.entries.count) 筆").foregroundStyle(.secondary)
            }

            if model.entries.isEmpty {
                ContentUnavailable()
            } else {
                Table(rows, selection: $selection, sortOrder: $sortOrder) {
                    TableColumn("注音", value: \.typed)
                    TableColumn("選的字", value: \.text)
                    TableColumn("次數", value: \.count) { Text("\($0.count)").monospacedDigit() }
                        .width(56)
                    TableColumn("最後使用", value: \.lastUsed) {
                        Text($0.lastUsed, format: .relative(presentation: .named))
                            .foregroundStyle(.secondary)
                    }
                    .width(110)
                }
                .contextMenu(forSelectionType: HistoryEntry.ID.self) { ids in
                    Button("刪除", role: .destructive) { delete(ids) }
                        .disabled(ids.isEmpty)
                }
                .onDeleteCommand { delete(selection) }
            }

            HStack {
                if let error = model.historyError {
                    Text(error).foregroundStyle(.red)
                }
                Spacer()
                Button("刪除所選") { delete(selection) }
                    .disabled(selection.isEmpty)
                Button("全部清除…", role: .destructive) { confirmingClear = true }
                    .disabled(model.entries.isEmpty)
            }
        }
        .confirmationDialog("清除全部選字紀錄？", isPresented: $confirmingClear) {
            Button("全部清除", role: .destructive) {
                model.clearHistory()
                selection.removeAll()
            }
        } message: {
            Text("輕注音會忘記所有你選過的字，排序回到預設。這個動作無法復原。")
        }
    }

    private func delete(_ ids: Set<HistoryEntry.ID>) {
        guard !ids.isEmpty else { return }
        model.forget(ids)
        selection.subtract(ids)
    }
}

private struct ContentUnavailable: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "clock").font(.system(size: 30)).foregroundStyle(.tertiary)
            Text("還沒有選字紀錄").font(.headline)
            Text("用輕注音打字後，你選過的字會出現在這裡。").foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
