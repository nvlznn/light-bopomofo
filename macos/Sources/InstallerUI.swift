import AppKit

/// What the user sees when opening LightBopomofo.app from the disk image:
/// standard alerts to install, update, reinstall or uninstall.
enum InstallerUI {
    static func run() -> Never {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        app.finishLaunching()
        app.activate(ignoringOtherApps: true)

        let installer = Installer()
        let new = installer.sourceVersion
        guard let old = installer.installedVersion else {
            if ask("安裝輕注音？", "輕注音 \(new) 會安裝在你的使用者資料夾，不需要管理員密碼。", ["安裝", "取消"]) == 0 {
                perform("安裝完成", installer.install)
            }
            exit(0)
        }
        switch new.compare(old, options: .numeric) {
        case .orderedDescending:
            if ask("更新輕注音？", "已安裝 \(old)，要更新到 \(new) 嗎？", ["更新", "取消"]) == 0 {
                perform("已更新到 \(new)", installer.install)
            }
        case .orderedSame:
            switch ask("輕注音 \(new) 已經安裝", "要重新安裝或解除安裝嗎？", ["好", "重新安裝", "解除安裝…"]) {
            case 1: perform("重新安裝完成", installer.install)
            case 2: confirmUninstall(installer)
            default: break
            }
        case .orderedAscending:
            switch ask("已安裝較新的版本", "已安裝 \(old)，這份是 \(new)。建議保留目前的版本。", ["保留", "解除安裝…"]) {
            case 1: confirmUninstall(installer)
            default: break
            }
        }
        exit(0)
    }

    private static func confirmUninstall(_ installer: Installer) {
        let history = "~/Library/Application Support/LightBopomofo"
        guard ask("解除安裝輕注音？", "輸入法會被停用並移到垃圾桶。你的選字紀錄保留在 \(history)，不需要時可以自行刪除。", ["解除安裝", "取消"], destructive: 0) == 0 else { return }
        do {
            try installer.uninstall()
            _ = ask("已解除安裝", "輕注音已移到垃圾桶。若選單列仍看得到，登出再登入一次即可。", ["好"])
        } catch {
            _ = ask("解除安裝失敗", error.localizedDescription, ["好"], style: .critical)
        }
    }

    private static func perform(_ done: String, _ action: () throws -> Void) {
        do {
            try action()
            _ = ask(done, """
                在選單列的輸入法選單選「輕注音」就可以開始打字。
                如果沒看到，到「系統設定 → 鍵盤 → 輸入方式」按「＋」，在「繁體中文」底下加入「輕注音」。

                打法：每個字只打第一個注音，例如 ㄋㄏㄇ → 你好嗎。空白鍵送出，← → 選字。
                """, ["好"])
        } catch {
            _ = ask("安裝失敗", error.localizedDescription, ["好"], style: .critical)
        }
    }

    /// Show an alert; returns the index of the button pressed.
    private static func ask(
        _ title: String, _ message: String, _ buttons: [String],
        style: NSAlert.Style = .informational, destructive: Int? = nil
    ) -> Int {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = style
        for (i, title) in buttons.enumerated() {
            let button = alert.addButton(withTitle: title)
            if i == destructive { button.hasDestructiveAction = true }
        }
        NSApp.activate(ignoringOtherApps: true)
        let response = alert.runModal()
        return response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
    }
}
