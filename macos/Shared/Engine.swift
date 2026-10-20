import Foundation

/// Where the user's data lives, shared by the input method and the app.
enum UserData {
    static var directory: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("LightBopomofo")
    }

    static var historyPath: String { directory.appendingPathComponent("history.tsv").path }

    /// Posted (machine-local) by the app after it edits the history, so a
    /// running input method reloads it.
    static let historyChanged = Notification.Name("dev.noky.lightbopomofo.historyChanged")
}

struct Candidate: Decodable, Equatable {
    let text: String
    /// Number of input symbols this candidate covers, from the start.
    let consumed: Int
}

/// Swift wrapper over the Rust engine's C ABI (lightbopomofo.h).
/// Use from one thread only (IMK calls arrive on the main thread).
final class Engine {
    private let handle: OpaquePointer

    init?(lexiconPath: String, historyPath: String?) {
        let created: OpaquePointer? = lexiconPath.withCString { lexicon in
            guard let historyPath else { return lb_engine_new(lexicon, nil) }
            return historyPath.withCString { lb_engine_new(lexicon, $0) }
        }
        guard let created else { return nil }
        handle = created
    }

    deinit {
        lb_engine_free(handle)
    }

    func query(_ symbols: String) -> [Candidate] {
        guard let json = lb_query(handle, symbols) else { return [] }
        defer { lb_string_free(json) }
        let data = Data(bytes: json, count: strlen(json))
        return (try? JSONDecoder().decode([Candidate].self, from: data)) ?? []
    }

    func learn(_ symbols: String, text: String) {
        lb_learn(handle, symbols, text)
    }

    func reloadHistory() {
        lb_reload_history(handle)
    }

    /// Learned choices whose text the lexicon lacks (分享新詞).
    func newWords() -> [HistoryEntry] {
        guard let json = lb_new_words(handle) else { return [] }
        defer { lb_string_free(json) }
        let data = Data(bytes: json, count: strlen(json))
        return (try? JSONDecoder().decode([HistoryEntry].self, from: data)) ?? []
    }

    /// 大千 key (unshifted US-layout character) → bopomofo symbol.
    static func bopomofo(forKey key: Character) -> Character? {
        guard key.unicodeScalars.count == 1, let scalar = key.unicodeScalars.first else { return nil }
        let code = lb_dachen(scalar.value)
        guard code != 0, let symbol = Unicode.Scalar(code) else { return nil }
        return Character(symbol)
    }
}

struct HistoryEntry: Decodable, Equatable, Identifiable, Hashable {
    let typed: String
    let text: String
    let count: Int
    let lastUsed: Date

    var id: String { typed + "\t" + text }

    private enum CodingKeys: String, CodingKey {
        case typed, text, count
        case lastUsed = "last_used"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        typed = try c.decode(String.self, forKey: .typed)
        text = try c.decode(String.self, forKey: .text)
        count = try c.decode(Int.self, forKey: .count)
        lastUsed = Date(timeIntervalSince1970: try c.decode(Double.self, forKey: .lastUsed))
    }
}

/// The history file without the lexicon, for viewing and editing in the app.
final class History {
    private let handle: OpaquePointer

    init?(path: String = UserData.historyPath) {
        guard let created = lb_history_open(path) else { return nil }
        handle = created
    }

    deinit {
        lb_history_free(handle)
    }

    /// Re-reads the file; most recent first.
    func entries() -> [HistoryEntry] {
        guard let json = lb_history_json(handle) else { return [] }
        defer { lb_string_free(json) }
        let data = Data(bytes: json, count: strlen(json))
        return (try? JSONDecoder().decode([HistoryEntry].self, from: data)) ?? []
    }

    @discardableResult
    func forget(_ entry: HistoryEntry) -> Bool {
        defer { History.notifyChanged() }
        return lb_history_forget(handle, entry.typed, entry.text) == 1
    }

    @discardableResult
    func clear() -> Bool {
        defer { History.notifyChanged() }
        return lb_history_clear(handle) == 0
    }

    static func notifyChanged() {
        DistributedNotificationCenter.default().postNotificationName(
            UserData.historyChanged, object: nil, userInfo: nil, deliverImmediately: true)
    }
}
