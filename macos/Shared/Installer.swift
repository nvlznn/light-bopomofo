import AppKit
import Carbon

/// Installs, updates and uninstalls the input method in a user's
/// `~/Library/Input Methods` — no administrator password, and updatable
/// in place. Used by the installer window and by `LightBopomofo install`.
struct Installer {
    static let bundleName = "LightBopomofo.app"

    /// Where input methods live; a copy running from one of these is the
    /// input method itself rather than the installer.
    static var inputMethodsDirectories: [URL] {
        [
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Input Methods"),
            URL(fileURLWithPath: "/Library/Input Methods"),
        ]
    }

    static func isInputMethodsCopy(_ bundle: URL = Bundle.main.bundleURL) -> Bool {
        let parent = bundle.deletingLastPathComponent().standardizedFileURL.resolvingSymlinksInPath()
        return inputMethodsDirectories.contains { $0.standardizedFileURL.resolvingSymlinksInPath() == parent }
    }

    let source: URL
    let directory: URL
    /// Register with Text Input Sources and stop running copies. Off in tests.
    var touchesSystem = true

    init(
        source: URL = Bundle.main.bundleURL,
        directory: URL = Installer.inputMethodsDirectories[0],
        touchesSystem: Bool = true
    ) {
        self.source = source
        self.directory = directory
        self.touchesSystem = touchesSystem
    }

    var destination: URL { directory.appendingPathComponent(Installer.bundleName) }

    /// Read straight from Info.plist: `Bundle` caches by path, so after an
    /// update in place it would keep reporting the old version.
    static func info(_ key: String, of bundle: URL) -> String? {
        let plist = bundle.appendingPathComponent("Contents/Info.plist")
        return NSDictionary(contentsOf: plist)?[key] as? String
    }

    static func version(of bundle: URL) -> String? {
        info("CFBundleShortVersionString", of: bundle)
    }

    var installedVersion: String? {
        FileManager.default.fileExists(atPath: destination.path) ? Installer.version(of: destination) : nil
    }

    var sourceVersion: String { Installer.version(of: source) ?? "0" }

    /// Copy the source bundle into place (replacing any existing copy), then
    /// register and enable it.
    func install() throws {
        let fm = FileManager.default
        try fm.createDirectory(at: directory, withIntermediateDirectories: true)
        if source.standardizedFileURL != destination.standardizedFileURL {
            if touchesSystem { stopRunningCopies() }
            // Copy beside the destination first so a failure never leaves a
            // half-written bundle where the system expects one.
            let staging = directory.appendingPathComponent(".\(Installer.bundleName).\(UUID().uuidString)")
            try fm.copyItem(at: source, to: staging)
            removeQuarantine(staging)
            if fm.fileExists(atPath: destination.path) {
                _ = try fm.replaceItemAt(destination, withItemAt: staging)
            } else {
                try fm.moveItem(at: staging, to: destination)
            }
        }
        if touchesSystem { try register() }
    }

    /// Disable the input source, stop it and move the bundle to the Trash.
    /// The user's history is kept.
    func uninstall() throws {
        if touchesSystem {
            for source in inputSources() { TISDisableInputSource(source) }
            stopRunningCopies()
        }
        if FileManager.default.fileExists(atPath: destination.path) {
            try FileManager.default.trashItem(at: destination, resultingItemURL: nil)
        }
    }

    /// Whether the input source is registered and switched on in the
    /// user's input sources (selectable from the menu bar).
    var isEnabled: Bool {
        inputSources().contains { source in
            func flag(_ key: CFString) -> Bool {
                guard let raw = TISGetInputSourceProperty(source, key) else { return false }
                return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(raw).takeUnretainedValue())
            }
            return flag(kTISPropertyInputSourceIsSelectCapable) && flag(kTISPropertyInputSourceIsEnabled)
        }
    }

    // MARK: - System

    private func register() throws {
        let status = TISRegisterInputSource(destination as CFURL)
        guard status == noErr else { throw InstallError.register(status) }
        let sources = inputSources()
        guard !sources.isEmpty else { throw InstallError.notFound }
        for source in sources { TISEnableInputSource(source) }
    }

    private func inputSources() -> [TISInputSource] {
        let filter = [kTISPropertyBundleID as String: bundleID] as CFDictionary
        return TISCreateInputSourceList(filter, true)?.takeRetainedValue() as? [TISInputSource] ?? []
    }

    private var bundleID: String {
        Installer.info("CFBundleIdentifier", of: source) ?? "dev.noky.inputmethod.lightbopomofo"
    }

    /// Stop other running copies so the system starts the new one on next use.
    private func stopRunningCopies() {
        let me = ProcessInfo.processInfo.processIdentifier
        let others = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID)
            .filter { $0.processIdentifier != me }
        others.forEach { $0.terminate() }
        let deadline = Date().addingTimeInterval(2)
        while others.contains(where: { !$0.isTerminated }) && Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        }
        others.filter { !$0.isTerminated }.forEach { $0.forceTerminate() }
    }

    private func removeQuarantine(_ url: URL) {
        let fm = FileManager.default
        var paths = [url.path]
        if let walker = fm.enumerator(atPath: url.path) {
            paths += walker.compactMap { $0 as? String }.map { url.appendingPathComponent($0).path }
        }
        for path in paths {
            removexattr(path, "com.apple.quarantine", XATTR_NOFOLLOW)
        }
    }
}

enum InstallError: LocalizedError {
    case register(OSStatus)
    case notFound

    var errorDescription: String? {
        switch self {
        case .register(let status): return "無法向系統註冊輸入法（錯誤 \(status)）。"
        case .notFound: return "已註冊，但系統找不到輕注音的輸入方式。"
        }
    }
}
