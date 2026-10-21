import AppKit
import Security
import CryptoKit
import ServiceManagement
import UserNotifications

/// Updating in place: a background item runs this app with --check-update
/// every hour; when GitHub has a newer release it downloads the dmg,
/// checks that it is signed by us and replaces this app, then puts the new
/// input method in place once the user stops typing, and says so. When that
/// cannot happen (the window is open, the download failed) it posts a
/// notification instead and leaves the version for the input method's menu;
/// choosing either (or 更新 in the app) does the same and reopens the app,
/// which updates the input method as on any first launch.
enum Updater {
    static let teamID = "6KB6SVY565"
    static let category = "update"
    private static let agent = SMAppService.agent(plistName: "dev.noky.lightbopomofo.background-service.plist")
    /// The background check asks at most this often…
    private static let checkEvery: TimeInterval = 3600
    /// …and reminds of an update it already announced this often.
    private static let remindEvery: TimeInterval = 7 * 24 * 3600

    /// How long the user must have left the keyboard and mouse alone before
    /// the input method is restarted with the new version…
    private static let quietFor: TimeInterval = 30
    /// …and how long to wait for that before leaving it to the next check.
    private static let waitForQuiet: TimeInterval = 15 * 60

    /// Whether to update in the background (on unless turned off).
    static var autoCheck: Bool {
        get { UserDefaults.standard.object(forKey: "CheckForUpdates") as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: "CheckForUpdates") }
    }

    private static let legacyAgent = SMAppService.agent(plistName: "dev.noky.lightbopomofo.update-check.plist")
    @MainActor private static var legacyMigration: Task<Void, Error>?

    @MainActor
    private static func removeLegacyRegistration() async throws {
        if let migration = legacyMigration { return try await migration.value }
        let migration = Task { @MainActor in
            // Preserve a refusal of background execution across the migration.
            if legacyAgent.status == .requiresApproval { throw RegistrationError.approval }
            if legacyAgent.status == .enabled { try await legacyAgent.unregister() }
        }
        legacyMigration = migration
        do { try await migration.value }
        catch { legacyMigration = nil; throw error }
    }

    @MainActor private static let registration = BackgroundRegistration(
        defaults: .standard, status: { agent.status },
        unregister: { try await agent.unregister() }, register: { try agent.register() }
    )

    /// Development copies must never replace the installed app's registration.
    /// Hash the executable as well as the plist: rebuilding the same version also
    /// changes the code that ServiceManagement must authorize.
    private static func registrationRevision() throws -> String {
        let app = Bundle.main.bundleURL.standardizedFileURL.resolvingSymlinksInPath()
        let directories = [URL(fileURLWithPath: "/Applications"),
                           FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications")]
        guard directories.contains(app.deletingLastPathComponent()),
              let executable = Bundle.main.executableURL else { throw RegistrationError.developmentCopy }
        do { try verify(app) } catch { throw RegistrationError.developmentCopy }
        let plist = app.appendingPathComponent("Contents/Library/LaunchAgents/dev.noky.lightbopomofo.background-service.plist")
        var hash = SHA256()
        hash.update(data: try Data(contentsOf: executable, options: .mappedIfSafe))
        hash.update(data: try Data(contentsOf: plist))
        return "1:" + hash.finalize().map { String(format: "%02x", $0) }.joined()
    }

    @MainActor
    static func refreshRegistration(force: Bool = false) async throws {
        let revision = try registrationRevision()
        try await removeLegacyRegistration()
        try await registration.reconcile(enabled: autoCheck || WordSharing.enabled,
                                         revision: revision, force: force)
    }

    @MainActor
    static func scheduleChecks() {
        Task {
            do { try await refreshRegistration() }
            catch { NSLog("輕注音: background registration: \(error.localizedDescription)") }
        }
    }

    /// Launch Services gives the new app its own lifecycle, so unregistering
    /// the old background job cannot kill the process doing the repair.
    @MainActor
    static func handOffRegistration() async throws {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.createsNewApplicationInstance = true
        configuration.activates = false
        configuration.arguments = ["--refresh-background-service", String(ProcessInfo.processInfo.processIdentifier)]
        _ = try await NSWorkspace.shared.openApplication(at: Bundle.main.bundleURL, configuration: configuration)
    }

    @MainActor
    static func finishRegistration(after pid: Int32) async throws {
        // The updating process must finish using the old executable before its
        // launchd registration is removed. Never unregister our own job.
        guard ProcessInfo.processInfo.environment["XPC_SERVICE_NAME"] != "dev.noky.lightbopomofo.background-service" else {
            throw RegistrationError.handoffTimeout
        }
        let deadline = Date().addingTimeInterval(60)
        while kill(pid, 0) == 0 {
            guard Date() < deadline else { throw RegistrationError.handoffTimeout }
            try await Task.sleep(nanoseconds: 100_000_000)
        }
        try await refreshRegistration()
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

    static func notifyInstalled(_ version: String) async {
        let center = UNUserNotificationCenter.current()
        center.removeDeliveredNotifications(withIdentifiers: [category])
        let content = UNMutableNotificationContent()
        content.title = "輕注音已更新到 \(version)"
        content.body = "新版本已經裝好，可以繼續打字。"
        try? await center.add(UNNotificationRequest(identifier: "updated", content: content, trigger: nil))
    }

    /// `--check-update` (the background item): 0 nothing to do, 10 an update
    /// is waiting for the user, 20 updated, 1 the check failed. `force`
    /// (`--now`) skips the setting and the once-in-a-while limits.
    static func checkInBackground(force: Bool) async -> Int32 {
        await WordSharing.shareIfDue(force: force)
        await updateInputMethodWhenQuiet()  // left waiting by an earlier update
        let defaults = UserDefaults.standard
        let now = Date().timeIntervalSince1970
        if !force, !autoCheck || now < defaults.double(forKey: "LastUpdateCheck") + checkEvery {
            return 0
        }
        defaults.set(now, forKey: "LastUpdateCheck")
        do {
            guard let version = try await UpdateCheck.available() else {
                PendingUpdate.record(nil)
                return 0
            }
            if !windowIsOpen {
                do {
                    try await install(version)
                    PendingUpdate.record(nil)
                    await updateInputMethodWhenQuiet()
                    await notifyInstalled(version)
                    do { try await handOffRegistration() }
                    catch { NSLog("輕注音: background registration handoff: \(error.localizedDescription)") }
                    return 20
                } catch {
                    NSLog("輕注音: background update to \(version): \(error)")
                }
            }
            PendingUpdate.record(version)
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

    /// The app (with its window) is running: leave the update to it.
    private static var windowIsOpen: Bool {
        let me = ProcessInfo.processInfo.processIdentifier
        return NSRunningApplication.runningApplications(withBundleIdentifier: Bundle.main.bundleIdentifier ?? "")
            .contains { $0.processIdentifier != me }
    }

    /// Seconds since the last key press, click or mouse move anywhere.
    private static var idleTime: TimeInterval {
        CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: CGEventType(rawValue: ~0)!)
    }

    /// When this app carries a newer input method than the one installed
    /// (it was updated in the background), put it in place once the user has
    /// stopped typing for a while, since that restarts the input method.
    static func updateInputMethodWhenQuiet() async {
        let installer = Installer(source: AppModel.bundledInputMethod)
        guard let installed = installer.installedVersion,
              UpdateCheck.isNewer(installer.sourceVersion, than: installed)
        else { return }
        let deadline = Date().addingTimeInterval(waitForQuiet)
        while idleTime < quietFor {
            guard Date() < deadline else { return }
            try? await Task.sleep(nanoseconds: 5_000_000_000)
        }
        // On the main thread, which can wait for the old input method to quit.
        await MainActor.run {
            do {
                try installer.install()
            } catch {
                NSLog("輕注音: updating the input method: \(error)")
            }
        }
    }

    // MARK: Install

    /// Download `version`, check its signature and replace this app with it.
    /// Set when this app replaced itself with a newer one (see AppModel).
    static let updatedInPlaceKey = "UpdatedInPlace"

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
        // Not a download by hand: the new app should not open on the tutorial.
        UserDefaults.standard.set(true, forKey: updatedInPlaceKey)
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
