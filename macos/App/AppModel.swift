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

    enum Update: Equatable {
        case idle
        case available(String)
        /// Looking up the newest release, then downloading it ("" until known).
        case updating(String)
        case failed(String)
    }

    @Published private(set) var status: Status = .checking
    @Published private(set) var update: Update = .idle
    @Published var autoCheck = Updater.autoCheck {
        didSet {
            Updater.autoCheck = autoCheck
            Updater.scheduleChecks()
            if autoCheck { Updater.requestPermission() }
        }
    }
    /// 分享新詞 (see WordSharing).
    @Published var shareWords = WordSharing.enabled {
        didSet {
            WordSharing.enabled = shareWords
            Updater.scheduleChecks()
        }
    }
    @Published private(set) var entries: [HistoryEntry] = []
    @Published var historyError: String?

    let installer: Installer
    private let history = History()

    /// The input method shipped inside this app.
    nonisolated static var bundledInputMethod: URL {
        Bundle.main.bundleURL.appendingPathComponent("Contents/Library/Input Methods/LightBopomofo.app")
    }

    init() {
        installer = Installer(source: AppModel.bundledInputMethod)
    }

    var bundledVersion: String { installer.sourceVersion }

    /// On launch: install the bundled input method if it is missing or older,
    /// and look for updates.
    func setUp() {
        let installed = installer.installedVersion
        if installed == nil || bundledVersion.compare(installed!, options: .numeric) == .orderedDescending {
            install()
        } else {
            refreshStatus()
        }
        reloadHistory()
        Updater.scheduleChecks()
        if autoCheck { Updater.requestPermission() }
        if AppModel.updatePending {
            AppModel.updatePending = false
            checkForUpdate(thenInstall: true)
        } else if autoCheck {
            checkForUpdate()
        }
    }

    // MARK: Updates

    static let updateRequested = Notification.Name("LightBopomofoUpdateRequested")
    /// Set when the notification launched the app before the window existed.
    private static var updatePending = false

    /// The update notification was chosen.
    static func requestUpdate() {
        updatePending = true
        NotificationCenter.default.post(name: updateRequested, object: nil)
    }

    func checkForUpdate(thenInstall: Bool = false) {
        AppModel.updatePending = false
        if case .updating = update { return }
        if thenInstall { update = .updating("") }
        Task {
            do {
                let version = try await UpdateCheck.available()
                PendingUpdate.record(version)
                guard let version else {
                    update = thenInstall ? .failed("已經是最新版本") : .idle
                    return
                }
                update = .available(version)
                if thenInstall { await installUpdate() }
            } catch {
                // A failed check in passing is not worth a word.
                update = thenInstall ? .failed(error.localizedDescription) : .idle
            }
        }
    }

    func installUpdate() async {
        guard case .available(let version) = update else {
            checkForUpdate(thenInstall: true)
            return
        }
        update = .updating(version)
        do {
            try await Updater.install(version)
            Updater.relaunch()
        } catch {
            update = .failed(error.localizedDescription)
        }
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
