import Foundation

/// Which release is newest, from GitHub's "latest release" — the same rules
/// as lightbopomofo-core's release.rs on Windows. Only this app goes online,
/// to read the public release list; the input method never does.
enum UpdateCheck {
    static let asset = "LightBopomofo.dmg"
    static let api = URL(string: "https://api.github.com/repos/nvlznn/light-bopomofo/releases/latest")!

    private struct Release: Decodable {
        struct Asset: Decodable {
            let name: String
        }

        let tag_name: String
        let assets: [Asset]
    }

    /// The latest release's version ("2.0"), if it already carries the dmg.
    static func latestVersion(json: Data) -> String? {
        guard let release = try? JSONDecoder().decode(Release.self, from: json),
              release.assets.contains(where: { $0.name == asset })
        else { return nil }
        return String(release.tag_name.drop(while: { $0 == "v" }))
    }

    /// Numbers compared part by part, missing parts are 0:
    /// "2.0" > "1.0" > "0.2.0", "1.0" = "1.0.0".
    static func isNewer(_ candidate: String, than current: String) -> Bool {
        func parts(_ v: String) -> [Int] {
            v.drop(while: { $0 == "v" }).split(separator: ".").map { Int($0) ?? 0 }
        }
        let (a, b) = (parts(candidate), parts(current))
        for i in 0..<max(a.count, b.count) {
            let (x, y) = (i < a.count ? a[i] : 0, i < b.count ? b[i] : 0)
            if x != y { return x > y }
        }
        return false
    }

    /// This app's version, read from disk (an update in place changes it).
    /// LIGHTBOPOMOFO_PRETEND_VERSION overrides it, to try updating without
    /// publishing a release.
    static var currentVersion: String {
        ProcessInfo.processInfo.environment["LIGHTBOPOMOFO_PRETEND_VERSION"]
            ?? NSDictionary(contentsOf: Bundle.main.bundleURL.appendingPathComponent("Contents/Info.plist"))?["CFBundleShortVersionString"] as? String
            ?? "0"
    }

    /// A newer release than this app, if there is one.
    /// LIGHTBOPOMOFO_PRETEND_LATEST stands in for GitHub (screenshots, tests).
    static func available() async throws -> String? {
        if let latest = ProcessInfo.processInfo.environment["LIGHTBOPOMOFO_PRETEND_LATEST"] {
            return isNewer(latest, than: currentVersion) ? latest : nil
        }
        var request = URLRequest(url: api, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 30)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        let (data, response) = try await URLSession.shared.data(for: request)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else { throw UpdateError.server }
        guard let latest = latestVersion(json: data), isNewer(latest, than: currentVersion) else { return nil }
        return latest
    }

    static func dmgURL(_ version: String) -> URL {
        URL(string: "https://github.com/nvlznn/light-bopomofo/releases/download/v\(version)/\(asset)")!
    }
}

enum UpdateError: LocalizedError {
    case server, download, damaged, signature, notNewer, tool(String)

    var errorDescription: String? {
        switch self {
        case .server: return "無法向 GitHub 查詢新版本"
        case .download: return "下載失敗"
        case .damaged: return "下載的檔案不完整"
        case .signature: return "新版的簽章不正確，已停止更新"
        case .notNewer: return "下載的版本沒有比較新"
        case .tool(let name): return "無法處理下載的檔案（\(name)）"
        }
    }
}
