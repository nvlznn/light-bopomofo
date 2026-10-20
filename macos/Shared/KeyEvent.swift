import AppKit

/// AppKit key events → composer `Key`s, shared by the input method and the
/// app's tutorial.
extension Key {
    /// What each punctuation key types as plain, with Shift and with Option,
    /// by key code, as in the system 注音. `nil`: the key is a bopomofo symbol
    /// (or nothing) in that state.
    private static let punctuation: [UInt16: (plain: String?, shift: String, option: String)] = [
        50: ("·", "～", "｀"),  // `
        18: (nil, "！", "１"), 19: (nil, "＠", "２"), 20: (nil, "＃", "３"),
        21: (nil, "＄", "４"), 23: (nil, "％", "５"), 22: (nil, "＾", "６"),
        26: (nil, "＆", "７"), 28: (nil, "＊", "８"), 25: (nil, "（", "９"),
        29: (nil, "）", "０"),
        27: (nil, "＿", "—"),  // -
        24: ("＝", "＋", "＝"),  // =
        33: ("「", "『", "「"),  // [
        30: ("」", "』", "」"),  // ]
        42: ("、", "｜", "、"),  // \
        41: (nil, "：", "；"),  // ;
        39: ("‘", "“", "’"),  // '
        43: (nil, "，", "，"),  // ,
        47: (nil, "。", "．"),  // .
        44: (nil, "？", "／"),  // /
    ]
    /// Shift+Option types what Shift does, except on these keys.
    private static let shiftOption: [UInt16: String] = [
        39: "”", 43: "《", 47: "》",  // ' , .
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
        if !flags.isDisjoint(with: [.command, .control])
            || (flags.contains(.option) && punctuation[event.keyCode] == nil)
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
        case 123: return .previous  // ←
        case 124: return .next  // →
        case 126: return .up  // ↑
        case 125: return .down  // ↓: the grid
        case 48: return flags.contains(.shift) ? .previous : .next  // tab
        case 116: return .pagePrevious  // PageUp
        case 121: return .pageNext  // PageDown
        case 33 where composing && flags.isEmpty: return .pagePrevious  // [
        case 30 where composing && flags.isEmpty: return .pageNext  // ]
        default: break
        }
        if flags.contains(.shift), composing, let n = digits[event.keyCode] { return .select(n) }
        if let p = punctuation[event.keyCode] {
            if flags.isSuperset(of: [.shift, .option]) {
                return .punctuation(shiftOption[event.keyCode] ?? p.shift)
            }
            if flags.contains(.shift) { return .punctuation(p.shift) }
            if flags.contains(.option) { return .punctuation(p.option) }
            if let plain = p.plain { return .punctuation(plain) }
        } else if !flags.isDisjoint(with: [.shift, .option]) {
            return .other  // Shift+letter: an uppercase letter; Option+letter: its own
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
