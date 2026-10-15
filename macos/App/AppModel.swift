import AppKit
import SwiftUI

/// State behind the settings window: install status of the bundled input
/// method and the user's history.
@MainActor
final class AppModel: ObservableObject {
    enum Status: Equatable {
        case checking
        case ready(version: String)
        case notEnabled(version: String)
        case newerInstalled(installed: String, bundled: String)
        case notInstalled
        case failed(String)
    }

    @Published private(set) var status: Status = .checking
    @Published private(set) var entries: [HistoryEntry] = []
    @Published var historyError: String?

    let installer: Installer
    private let history = History()

    /// The input method shipped inside this app.
    static var bundledInputMethod: URL {
        Bundle.main.bundleURL.appendingPathComponent("Contents/Library/Input Methods/LightBopomofo.app")
    }

    init() {
        installer = Installer(source: AppModel.bundledInputMethod)
    }

    var bundledVersion: String { installer.sourceVersion }

    /// On launch: install the bundled input method if it is missing or older.
    func setUp() {
        let installed = installer.installedVersion
        if installed == nil || bundledVersion.compare(installed!, options: .numeric) == .orderedDescending {
            install()
        } else {
            refreshStatus()
        }
        reloadHistory()
    }

    func install() {
        do {
            try installer.install()
            refreshStatus()
        } catch {
            status = .failed(error.localizedDescription)
        }
    }

    func uninstall() {
        do {
            try installer.uninstall()
            refreshStatus()
        } catch {
            status = .failed(error.localizedDescription)
        }
    }

    func refreshStatus() {
        guard let installed = installer.installedVersion else {
            status = .notInstalled
            return
        }
        if installed.compare(bundledVersion, options: .numeric) == .orderedDescending {
            status = .newerInstalled(installed: installed, bundled: bundledVersion)
        } else if installer.isEnabled {
            status = .ready(version: installed)
        } else {
            status = .notEnabled(version: installed)
        }
    }

    func openInputSourceSettings() {
        let url = URL(string: "x-apple.systempreferences:com.apple.Keyboard-Settings.extension")!
        NSWorkspace.shared.open(url)
    }

    // MARK: History

    func reloadHistory() {
        entries = history?.entries() ?? []
    }

    func forget(_ ids: Set<HistoryEntry.ID>) {
        guard let history else { return }
        let failed = entries.filter { ids.contains($0.id) }.filter { !history.forget($0) }
        historyError = failed.isEmpty ? nil : "有 \(failed.count) 筆紀錄無法刪除。"
        reloadHistory()
    }

    func clearHistory() {
        guard let history else { return }
        historyError = history.clear() ? nil : "無法清除選字紀錄。"
        reloadHistory()
    }
}
