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

    private let engine: Engine
    private(set) var symbols: [Character] = []
    private(set) var candidates: [Candidate] = []
    private(set) var selected = 0

    /// What this composition has committed so far, learned as a whole when it
    /// ends after several partial selections (工作 + 方法 → 工作方法).
    private var sessionTyped = ""
    private var sessionText = ""
    private var sessionPieces = 0

    init(engine: Engine) {
        self.engine = engine
    }

    var isEmpty: Bool { symbols.isEmpty }
    var composingText: String { String(symbols) }

    func handle(_ key: Key) -> KeyResult {
        switch key {
        case .symbol(let s):
            if isEmpty && Composer.tones.contains(s) {
                return KeyResult(handled: false)
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
            symbols.removeLast()
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
            return KeyResult(handled: true, commit: commitAll() + p)
        case .other:
            return KeyResult(handled: false, commit: commitAll())
        case .abandon:
            reset()
            return KeyResult(handled: false)
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
        candidates = isEmpty ? [] : engine.query(composingText)
        selected = 0
    }
}
