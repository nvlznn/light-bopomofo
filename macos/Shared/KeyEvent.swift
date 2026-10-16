import AppKit

/// AppKit key events → composer `Key`s, shared by the input method and the
/// app's tutorial.
extension Key {
    private static let punctuation: [String: String] = [
        "<": "，", ">": "。", "?": "？", "!": "！", ":": "：",
    ]

    /// Number keys 1–9 by key code (the keypad's are separate keys).
    private static let digits: [UInt16: Int] = [
        18: 1, 19: 2, 20: 3, 21: 4, 23: 5, 22: 6, 26: 7, 28: 8, 25: 9,
    ]

    init(event: NSEvent, composing: Bool) {
        self = Key.translate(event, composing: composing)
    }

    private static func translate(_ event: NSEvent, composing: Bool) -> Key {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // Shortcuts and keypad numbers go to the app. Arrows, PageUp and
        // PageDown carry .numericPad too, so they are exempt from that one.
        let isNavigation = (123...126).contains(event.keyCode) || [116, 121].contains(event.keyCode)
        if !flags.isDisjoint(with: [.command, .control, .option])
            || (flags.contains(.numericPad) && !isNavigation)
        {
            return .other
        }
        if flags.contains(.capsLock) { return .abandon }  // Caps Lock types English
        switch event.keyCode {
        case 49: return .symbol(Composer.firstTone)  // space
        case 36, 76: return .commit  // return, keypad enter
        case 51: return .backspace
        case 53: return .escape
        case 123, 126: return .previous  // ←, ↑
        case 124, 125: return .next  // →, ↓
        case 48: return flags.contains(.shift) ? .previous : .next  // tab
        case 116: return .pagePrevious  // PageUp
        case 121: return .pageNext  // PageDown
        case 33 where !flags.contains(.shift): return .pagePrevious  // [
        case 30 where !flags.contains(.shift): return .pageNext  // ]
        default: break
        }
        if flags.contains(.shift) {
            if composing, let n = digits[event.keyCode] { return .select(n) }
            if let p = punctuation[event.characters ?? ""] { return .punctuation(p) }
            return .other
        }
        if let ch = event.charactersIgnoringModifiers?.lowercased().first,
           let symbol = Engine.bopomofo(forKey: ch)
        {
            return .symbol(symbol)
        }
        return .other
    }
}

extension Engine {
    /// The 大千 key that types `symbol` (the inverse of `bopomofo(forKey:)`).
    static func key(forBopomofo symbol: Character) -> Character? {
        "1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-7634".first { bopomofo(forKey: $0) == symbol }
    }
}
