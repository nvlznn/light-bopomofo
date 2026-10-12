import Foundation

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

    /// 大千 key (unshifted US-layout character) → bopomofo symbol.
    static func bopomofo(forKey key: Character) -> Character? {
        guard key.unicodeScalars.count == 1, let scalar = key.unicodeScalars.first else { return nil }
        let code = lb_dachen(scalar.value)
        guard code != 0, let symbol = Unicode.Scalar(code) else { return nil }
        return Character(symbol)
    }
}
