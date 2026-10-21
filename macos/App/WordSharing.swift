import Foundation

/// 分享新詞: the words the user put together that the lexicon lacks (with the
/// symbols typed and how often), sent about once an hour by the background item
/// under a random device ID, for review on lightbopomofo.noky.dev/admin.html.
/// On unless turned off (the tutorial's last page, or 一般). See privacy.html.
enum WordSharing {
    /// LIGHTBOPOMOFO_WORDS_URL overrides it (testing against a local server).
    static var endpoint: URL {
        ProcessInfo.processInfo.environment["LIGHTBOPOMOFO_WORDS_URL"].flatMap(URL.init(string:))
            ?? URL(string: "https://lightbopomofo.noky.dev/api/words")!
    }
    static let privacyURL = URL(string: "https://lightbopomofo.noky.dev/privacy.html")!
    /// At most this often…
    private static let every: TimeInterval = 3600
    /// …and the most recent this many (the server takes no more).
    private static let maxWords = 500

    static var enabled: Bool {
        get { UserDefaults.standard.object(forKey: "ShareNewWords") as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: "ShareNewWords") }
    }

    /// Random, made on first use; ties nothing to the user.
    static var deviceID: String {
        if let id = UserDefaults.standard.string(forKey: "DeviceID") { return id }
        let id = UUID().uuidString.lowercased()
        UserDefaults.standard.set(id, forKey: "DeviceID")
        return id
    }

    struct Word: Encodable, Equatable {
        let text: String
        let typed: String
        let count: Int
    }

    /// The words to share from these history entries: those the server
    /// accepts (Han characters typed with bopomofo), most recent first.
    static func words(from entries: [HistoryEntry]) -> [Word] {
        let han = #/^\p{Script=Han}{1,20}$/#
        let bopomofo = #/^[ㄅ-ㄩˉˊˇˋ˙]{1,60}$/#
        return entries
            .sorted { $0.lastUsed > $1.lastUsed }
            .filter { $0.text.wholeMatch(of: han) != nil && $0.typed.wholeMatch(of: bopomofo) != nil }
            .prefix(maxWords)
            .map { Word(text: $0.text, typed: $0.typed, count: max(1, $0.count)) }
    }

    /// Send the new words if sharing is on and an hour has passed (`force`:
    /// regardless of when). Failures wait for the next try.
    static func shareIfDue(force: Bool) async {
        let defaults = UserDefaults.standard
        let now = Date().timeIntervalSince1970
        guard enabled, force || now >= defaults.double(forKey: "LastWordShare") + every else { return }
        let lexicon = AppModel.bundledInputMethod.appendingPathComponent("Contents/Resources/lexicon.bin").path
        guard let engine = Engine(lexiconPath: lexicon, historyPath: UserData.historyPath) else { return }
        let words = words(from: engine.newWords())
        // Empty uploads still record a successful sync on the server.
        do {
            struct Upload: Encodable {
                let device: String
                let app: String
                let words: [Word]
            }
            var request = URLRequest(url: endpoint, timeoutInterval: 30)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try? JSONEncoder().encode(Upload(device: deviceID, app: UpdateCheck.currentVersion, words: words))
            guard let (_, response) = try? await URLSession.shared.data(for: request),
                  (response as? HTTPURLResponse)?.statusCode == 204
            else { return }
        }
        defaults.set(now, forKey: "LastWordShare")
    }
}
