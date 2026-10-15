import AppKit
import Security
import ServiceManagement
import UserNotifications

/// Updating in place: a background item runs this app with --check-update
/// every few hours; when GitHub has a newer release it posts a notification.
/// Choosing it (or 更新 in the app) downloads the dmg, checks that it is
/// signed by us, replaces this app and reopens it; the new app then updates
/// the input method as on any first launch.
enum Updater {
    static let teamID = "6KB6SVY565"
    static let category = "update"
    private static let agent = SMAppService.agent(plistName: "dev.noky.lightbopomofo.update-check.plist")
    /// The background check asks at most this often…
    private static let checkEvery: TimeInterval = 12 * 3600
    /// …and reminds of an update it already announced this often.
    private static let remindEvery: TimeInterval = 7 * 24 * 3600

    /// Whether to check in the background (on unless turned off).
    static var autoCheck: Bool {
        get { UserDefaults.standard.object(forKey: "CheckForUpdates") as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: "CheckForUpdates") }
    }

    /// Register or remove the background item.
    static func scheduleChecks(_ on: Bool) {
        do {
            if on {
                if agent.status != .enabled { try agent.register() }
            } else if agent.status != .notRegistered {
                try agent.unregister()
            }
        } catch {
            NSLog("輕注音: background update check: \(error)")
        }
    }

    // MARK: Notification

    static func requestPermission() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    static func notify(_ version: String) async {
        let center = UNUserNotificationCenter.current()
        let install = UNNotificationAction(identifier: "install", title: "更新", options: [.foreground])
        center.setNotificationCategories([
            UNNotificationCategory(identifier: category, actions: [install], intentIdentifiers: []),
        ])
        let content = UNMutableNotificationContent()
        content.title = "輕注音 \(version) 可以更新"
        content.body = "按「更新」下載並安裝，輸入法會一起更新。"
        content.categoryIdentifier = category
        try? await center.add(UNNotificationRequest(identifier: category, content: content, trigger: nil))
    }

    /// `--check-update` (the background item): 0 nothing to do, 10 an update
    /// is available, 1 the check failed. `force` (`--now`) skips the setting
    /// and the once-in-a-while limits.
    static func checkInBackground(force: Bool) async -> Int32 {
        let defaults = UserDefaults.standard
        let now = Date().timeIntervalSince1970
        if !force, !autoCheck || now < defaults.double(forKey: "LastUpdateCheck") + checkEvery {
            return 0
        }
        defaults.set(now, forKey: "LastUpdateCheck")
        do {
            guard let version = try await UpdateCheck.available() else { return 0 }
            let announced = defaults.string(forKey: "NotifiedVersion") == version
                && now < defaults.double(forKey: "NotifiedAt") + remindEvery
            if force || !announced {
                await notify(version)
                defaults.set(version, forKey: "NotifiedVersion")
                defaults.set(now, forKey: "NotifiedAt")
            }
            return 10
        } catch {
            return 1
        }
    }

    // MARK: Install

    /// Download `version`, check its signature and replace this app with it.
    static func install(_ version: String) async throws {
        let fm = FileManager.default
        let current = Bundle.main.bundleURL
        let (download, response) = try await URLSession.shared.download(from: UpdateCheck.dmgURL(version))
        guard (response as? HTTPURLResponse)?.statusCode == 200 else { throw UpdateError.download }
        // On the app's volume, so the final move is a rename.
        let work = try fm.url(for: .itemReplacementDirectory, in: .userDomainMask, appropriateFor: current, create: true)
        defer { try? fm.removeItem(at: work) }
        let dmg = work.appendingPathComponent(UpdateCheck.asset)
        try fm.moveItem(at: download, to: dmg)

        // Not inside `work`: macOS denies reading a volume mounted under TemporaryItems.
        let mount = fm.temporaryDirectory.appendingPathComponent("LightBopomofo-\(UUID().uuidString)")
        try run("/usr/bin/hdiutil", "attach", dmg.path, "-nobrowse", "-readonly", "-noautoopen", "-mountpoint", mount.path)
        defer {
            try? run("/usr/bin/hdiutil", "detach", mount.path, "-force")
            try? fm.removeItem(at: mount)
        }
        let items = (try? fm.contentsOfDirectory(at: mount, includingPropertiesForKeys: nil)) ?? []
        guard let app = items.first(where: { $0.pathExtension == "app" }) else { throw UpdateError.damaged }
        try verify(app)
        guard let new = Installer.version(of: app), UpdateCheck.isNewer(new, than: UpdateCheck.currentVersion) else {
            throw UpdateError.notNewer
        }

        let staged = work.appendingPathComponent(current.lastPathComponent)
        try run("/usr/bin/ditto", app.path, staged.path)
        _ = try fm.replaceItemAt(current, withItemAt: staged)
    }

    /// Signed with our Developer ID, as this app, intact.
    static func verify(_ app: URL) throws {
        var code: SecStaticCode?
        var requirement: SecRequirement?
        let text = "anchor apple generic and identifier \"dev.noky.lightbopomofo\" and certificate leaf[subject.OU] = \"\(teamID)\""
        guard SecStaticCodeCreateWithPath(app as CFURL, [], &code) == errSecSuccess, let code,
              SecRequirementCreateWithString(text as CFString, [], &requirement) == errSecSuccess,
              SecStaticCodeCheckValidity(code, SecCSFlags(rawValue: kSecCSCheckAllArchitectures | kSecCSCheckNestedCode | kSecCSStrictValidate), requirement) == errSecSuccess
        else { throw UpdateError.signature }
    }

    /// Open the (new) app at this app's place and quit this one.
    @MainActor
    static func relaunch() {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.createsNewApplicationInstance = true
        NSWorkspace.shared.openApplication(at: Bundle.main.bundleURL, configuration: configuration) { _, _ in
            DispatchQueue.main.async { NSApp.terminate(nil) }
        }
    }

    private static func run(_ tool: String, _ arguments: String...) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: tool)
        process.arguments = arguments
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw UpdateError.tool(URL(fileURLWithPath: tool).lastPathComponent) }
    }
}
