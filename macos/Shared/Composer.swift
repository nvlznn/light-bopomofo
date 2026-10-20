import Foundation

/// Platform-independent key, already translated from the OS event.
enum Key: Equatable {
    case symbol(Character)  // bopomofo symbol or tone mark; Space is the first tone ˉ
    case commit  // Enter: commit the selected candidate
    case backspace
    case escape
    case previous  // ←, ↑, Shift+Tab
    case next  // →, ↓, Tab
    case select(Int)  // Shift+1–9 while composing: commit that candidate of the current page
    case pagePrevious  // [, PageUp
    case pageNext  // ], PageDown
    case punctuation(String)
    case letter(Character)  // Shift+A–Z: starts English, until Space, Enter or punctuation (iPhone, API, 4G)
    case other  // anything else: commit the composition, then let the app have it
    case abandon  // switched to English (Caps Lock): drop the composition, let the app have it
}

struct KeyResult: Equatable {
    /// Whether the key was consumed (false: the app should process it).
    var handled: Bool
    /// Text to insert into the document before anything else.
    var commit: String = ""
}

/// The composition state machine: bopomofo buffer, candidates, selection.
/// No InputMethodKit here, so it can be tested directly.
final class Composer {
    static let maxSymbols = 40
    /// Candidates per page of the candidate window, numbered 1–9.
    static let pageSize = 9
    static let firstTone: Character = "ˉ"
    static let tones: Set<Character> = [firstTone, "ˊ", "ˇ", "ˋ", "˙"]
    static let initials = Set("ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ")

    private let engine: Engine
    private(set) var symbols: [Character] = []
    private(set) var candidates: [Candidate] = []
    private(set) var selected = 0

    /// What this composition has committed so far, learned as a whole when it
    /// ends after several partial selections (工作 + 方法 → 工作方法).
    private var sessionTyped = ""
    private var sessionText = ""
    private var sessionPieces = 0

    /// English typed since a Shift+letter, after the bopomofo in `symbols`;
    /// nil while typing bopomofo.
    private(set) var english: String?
    /// A tone typed alone, waiting for the next key: its mark (as in the
    /// system 注音), or its digit when a capital follows (4G) or Tab picks it.
    private(set) var loneTone: Character?
    /// For each candidate while typing English: its English part.
    private var englishParts: [String] = []
    /// The candidate that types the keys themselves (app, ok), if offered.
    private var keysAt: Int?

    /// Lowercase starts of names typed with a capital inside (iPhone, macOS):
    /// the keys typed right before the capital that join the English. Only
    /// ones that are rare as the end of 簡拼 (ㄛ, ㄩㄇㄏ).
    static let camelStarts = ["mac", "i"]

    init(engine: Engine) {
        self.engine = engine
    }

    var isEmpty: Bool { symbols.isEmpty && english == nil && loneTone == nil }
    var composingText: String { loneTone.map { String($0) } ?? String(symbols) + (english ?? "") }
    private var length: Int { symbols.count + (english?.count ?? 0) }

    func handle(_ key: Key) -> KeyResult {
        if let tone = loneTone {
            switch key {
            case .letter(let c):
                loneTone = nil
                english = Composer.keys([tone]) + String(c)
                refresh()
                return KeyResult(handled: true)
            case .symbol, .punctuation, .other, .abandon:
                // Anything else: the tone is typed, then the key as usual.
                let typed = commitSelected()
                let result = handle(key)
                return KeyResult(handled: result.handled, commit: typed + result.commit)
            default: break
            }
        }
        if english != nil, let result = handleEnglish(key) {
            return result
        }
        switch key {
        case .letter(let c):
            let flushed = length >= Composer.maxSymbols ? commitAll() : ""
            english = (english ?? "") + String(c)
            refresh()
            return KeyResult(handled: true, commit: flushed)
        case .symbol(let s):
            // Alone, a tone waits for the next key; Space is still a space.
            if isEmpty && Composer.tones.contains(s) {
                guard s != Composer.firstTone else { return KeyResult(handled: false) }
                loneTone = s
                refresh()
                return KeyResult(handled: true)
            }
            // Space after a tone: the syllable is already finished.
            if s == Composer.firstTone, let last = symbols.last, Composer.tones.contains(last) {
                return KeyResult(handled: true)
            }
            let flushed = symbols.count >= Composer.maxSymbols ? commitAll() : ""
            symbols.append(s)
            refresh()
            return KeyResult(handled: true, commit: flushed)
        case .commit:
            guard !isEmpty else { return KeyResult(handled: false) }
            return KeyResult(handled: true, commit: commitSelected())
        case .backspace:
            guard !isEmpty else { return KeyResult(handled: false) }
            if loneTone != nil {
                loneTone = nil
            } else if var typed = english {
                typed.removeLast()
                english = typed.isEmpty ? nil : typed
            } else {
                symbols.removeLast()
            }
            if isEmpty { resetSession() }
            refresh()
            return KeyResult(handled: true)
        case .escape:
            guard !isEmpty else { return KeyResult(handled: false) }
            reset()
            return KeyResult(handled: true)
        case .previous:
            guard !isEmpty else { return KeyResult(handled: false) }
            selected = max(0, selected - 1)
            return KeyResult(handled: true)
        case .next:
            guard !isEmpty else { return KeyResult(handled: false) }
            selected = min(candidates.count - 1, selected + 1)
            return KeyResult(handled: true)
        case .select(let n):
            guard !isEmpty else { return KeyResult(handled: false) }
            let index = selected / Composer.pageSize * Composer.pageSize + n - 1
            guard (1...Composer.pageSize).contains(n), index < candidates.count else {
                return KeyResult(handled: true)
            }
            selected = index
            return KeyResult(handled: true, commit: commitSelected())
        case .pagePrevious, .pageNext:
            guard !isEmpty else { return KeyResult(handled: false) }
            let page = selected / Composer.pageSize
            let last = max(candidates.count - 1, 0) / Composer.pageSize
            selected = (key == .pageNext ? min(page + 1, last) : max(page - 1, 0)) * Composer.pageSize
            return KeyResult(handled: true)
        case .punctuation(let p):
            // While composing, punctuation is ignored (as in the system 注音).
            return KeyResult(handled: true, commit: isEmpty ? p : "")
        case .other:
            return KeyResult(handled: false, commit: commitAll())
        case .abandon:
            reset()
            return KeyResult(handled: false)
        }
    }

    /// Keys that end or edit English; nil for the ones handled as usual
    /// (choosing among the candidates, Backspace, Esc…).
    private func handleEnglish(_ key: Key) -> KeyResult? {
        switch key {
        case .symbol(Composer.firstTone):
            // Space ends the word and types itself.
            return KeyResult(handled: false, commit: commitAll())
        case .symbol(let s):
            let flushed = length >= Composer.maxSymbols ? commitAll() : ""
            english = (english ?? "") + Composer.keys([s])
            refresh()
            return KeyResult(handled: true, commit: flushed)
        case .punctuation(let p):
            return KeyResult(handled: true, commit: commitAll() + p)
        default:
            return nil
        }
    }

    /// Commit the selected candidate, then the best conversion of whatever
    /// remains, leaving the composition empty.
    func commitAll() -> String {
        guard !isEmpty else { return "" }
        var text = commitSelected()
        while !isEmpty {
            selected = 0
            text += commitSelected()
        }
        return text
    }

    func reset() {
        symbols.removeAll()
        english = nil
        loneTone = nil
        resetSession()
        refresh()
    }

    private func commitSelected() -> String {
        guard selected < candidates.count else {
            // No candidates (should not happen): give back the raw symbols.
            let raw = composingText
            reset()
            return raw
        }
        let choice = candidates[selected]
        if english != nil || loneTone != nil || keysAt == selected {
            // English covers the whole composition. It is learned as typed by
            // its keys, so typing it in lowercase brings it back (Figma). A
            // lone tone is not learned.
            if loneTone == nil {
                let part = english != nil && selected < englishParts.count ? englishParts[selected] : choice.text
                if let symbols = Composer.englishSymbols(part) {
                    engine.learn(symbols, text: part)
                }
            }
            reset()
            return choice.text
        }
        let consumed = min(max(choice.consumed, 1), symbols.count)
        engine.learn(composingText, text: choice.text)
        sessionTyped += String(symbols.prefix(consumed))
        sessionText += choice.text
        sessionPieces += 1
        symbols.removeFirst(consumed)
        if isEmpty {
            if sessionPieces > 1 {
                engine.learn(sessionTyped, text: sessionText)
            }
            resetSession()
        }
        refresh()
        return choice.text
    }

    private func resetSession() {
        sessionTyped = ""
        sessionText = ""
        sessionPieces = 0
    }

    private func refresh() {
        selected = 0
        keysAt = nil
        englishParts = []
        if let tone = loneTone {
            candidates = [String(tone), Composer.keys([tone])].map { Candidate(text: $0, consumed: 1) }
        } else if let english {
            candidates = englishCandidates(english)
        } else {
            (candidates, keysAt) = Composer.candidates(engine, String(symbols))
        }
    }

    /// The symbols the 大千 keys of English `text` type (case ignored), or nil
    /// when some character has no key.
    static func englishSymbols(_ text: String) -> String? {
        var symbols = ""
        for c in text.lowercased() {
            guard let symbol = Engine.bopomofo(forKey: c) else { return nil }
            symbols.append(symbol)
        }
        return symbols
    }

    /// The keys that type `symbols` (大千).
    static func keys(_ symbols: [Character]) -> String {
        String(symbols.map { $0 == firstTone ? " " : Engine.key(forBopomofo: $0) ?? $0 })
    }

    /// Where English typed after `symbols` most likely starts: the lowercase
    /// start of iPhone or macOS, and digits right before it (5G, 1080P). A
    /// tone key counts as a digit only first or after another digit (4K, but
    /// ㄋㄧˇ + API is 你API).
    static func englishStart(_ symbols: [Character]) -> Int {
        let typed = Array(keys(symbols))
        var start = typed.count
        if let camel = camelStarts.first(where: { typed.reversed().starts(with: $0.reversed()) }) {
            start -= camel.count
        }
        while start > 0, typed[start - 1].isASCII, typed[start - 1].isNumber {
            if tones.contains(symbols[start - 1]), start >= 2, !(typed[start - 2].isASCII && typed[start - 2].isNumber) {
                break
            }
            start -= 1
        }
        return start
    }

    /// While typing English: the bopomofo before it converted, then the
    /// English; first where it most likely starts, then the other places it
    /// could (Tab picks one), down to all of it as English.
    private func englishCandidates(_ english: String) -> [Candidate] {
        let n = symbols.count
        let options = [Composer.englishStart(symbols)] + Array((max(n - 3, 0)...n).reversed()) + [0]
        var result: [Candidate] = []
        for split in options {
            let before = Array(symbols.prefix(split))
            let chinese = split == 0 ? "" : engine.query(String(before)).first { $0.consumed == split }?.text ?? String(before)
            let part = Composer.keys(Array(symbols.dropFirst(split))) + english
            let text = chinese + part
            if !result.contains(where: { $0.text == text }) {
                result.append(Candidate(text: text, consumed: n))
                englishParts.append(part)
            }
        }
        return result
    }

    /// The candidates for bopomofo being composed: the engine's, with the
    /// keys themselves for English typed in lowercase (app, ok) at the end of
    /// the first page, keeping the bopomofo candidate where the engine put
    /// it. Also returns where the keys went. Only when some symbol is not an
    /// initial: 簡拼 is mostly initials (ㄏㄏ is not cc), while English keeps
    /// hitting the finals' keys (app is ㄇㄣㄣ).
    static func candidates(_ engine: Engine, _ symbols: String) -> ([Candidate], Int?) {
        var candidates = symbols.isEmpty ? [] : engine.query(symbols)
        let typed = keys(Array(symbols))
        guard !symbols.isEmpty, !symbols.contains(firstTone), !symbols.allSatisfy(initials.contains),
              typed.contains(where: { $0.isASCII && $0.isLetter }),
              !candidates.contains(where: { $0.text == typed })
        else { return (candidates, nil) }
        let bopomofoAt = candidates.firstIndex { $0.text == symbols }
        let at = min(candidates.count, pageSize - 1)
        candidates.insert(Candidate(text: typed, consumed: symbols.count), at: at)
        if let b = bopomofoAt, b > at {
            candidates.insert(candidates.remove(at: b + 1), at: b)
        }
        return (candidates, at)
    }
}
