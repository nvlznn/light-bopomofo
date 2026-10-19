import Foundation

/// The newer release the app's background check found and did not install,
/// left for the input method (which never goes online) to offer in its menu.
enum PendingUpdate {
    /// Opens the app and starts the update (macOS and Windows alike).
    static let url = URL(string: "lightbopomofo:update")!

    static func file(in directory: URL = UserData.directory) -> URL {
        directory.appendingPathComponent("update-available")
    }

    /// Remember `version` as available, or forget it (`nil`).
    static func record(_ version: String?, in directory: URL = UserData.directory) {
        let file = file(in: directory)
        guard let version else {
            try? FileManager.default.removeItem(at: file)
            return
        }
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try? Data(version.utf8).write(to: file, options: .atomic)
    }

    /// The recorded version, if it is newer than `current`.
    static func version(newerThan current: String, in directory: URL = UserData.directory) -> String? {
        guard let data = try? Data(contentsOf: file(in: directory)),
              let version = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
              version.compare(current, options: .numeric) == .orderedDescending
        else { return nil }
        return version
    }
}
