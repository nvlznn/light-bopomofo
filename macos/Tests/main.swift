// Composer behaviour against the real engine and lexicon.
// Run with macos/test.sh.

import Foundation

var failures = 0

func check(_ condition: @autoclosure () -> Bool, _ message: String, line: Int = #line) {
    if !condition() {
        failures += 1
        print("FAIL line \(line): \(message)")
    }
}

func makeComposer() -> Composer {
    let lexicon = CommandLine.arguments[1]
    guard let engine = Engine(lexiconPath: lexicon, historyPath: nil) else {
        fatalError("cannot load \(lexicon)")
    }
    return Composer(engine: engine)
}

func type(_ c: Composer, _ symbols: String) {
    for s in symbols {
        _ = c.handle(.symbol(s))
    }
}

func select(_ c: Composer, _ text: String) -> Bool {
    while c.selected < c.candidates.count - 1, c.candidates[c.selected].text != text {
        _ = c.handle(.next)
    }
    return c.candidates[c.selected].text == text
}

let started = Date()

// Whole conversion on Space.
do {
    let c = makeComposer()
    type(c, "ㄋㄏㄇ")
    check(c.candidates.first?.text == "你好嗎", "first candidate \(c.candidates.first?.text ?? "-")")
    check(c.handle(.commit) == KeyResult(handled: true, commit: "你好嗎"), "space commits")
    check(c.isEmpty && c.candidates.isEmpty, "empty after commit")
}

// Partial selection keeps the rest and reconverts it.
do {
    let c = makeComposer()
    type(c, "ㄍㄗㄈㄈ")
    check(select(c, "工作"), "工作 offered")
    check(c.handle(.commit).commit == "工作", "commit 工作")
    check(c.composingText == "ㄈㄈ", "rest stays: \(c.composingText)")
    check(c.candidates.first?.text == "方法", "rest reconverted: \(c.candidates.first?.text ?? "-")")
    check(c.handle(.commit).commit == "方法", "commit 方法")
    check(c.isEmpty, "empty")
}

// A composition built from partial selections is remembered as a whole.
do {
    let c = makeComposer()
    type(c, "ㄊㄑㄏㄏ")
    let first = c.candidates.first?.text
    check(select(c, "天氣"), "天氣 offered")
    _ = c.handle(.commit)
    let rest = c.handle(.commit).commit
    type(c, "ㄊㄑㄏㄏ")
    check(c.candidates.first?.text == "天氣" + rest, "recalled \(c.candidates.first?.text ?? "-") (was \(first ?? "-"))")
    _ = c.handle(.escape)
}

// Selection movement is clamped.
do {
    let c = makeComposer()
    type(c, "ㄋ")
    check(c.handle(.previous).handled && c.selected == 0, "previous at 0")
    _ = c.handle(.next)
    check(c.selected == 1, "next")
}

// Keys with an empty composition go to the app.
do {
    let c = makeComposer()
    for key in [Key.commit, .backspace, .escape, .previous, .next, .other, .symbol("ˇ")] {
        check(c.handle(key) == KeyResult(handled: false), "\(key) passes through when empty")
    }
}

// Backspace and Escape.
do {
    let c = makeComposer()
    type(c, "ㄋㄏ")
    check(c.handle(.backspace).handled && c.composingText == "ㄋ", "backspace removes last")
    _ = c.handle(.backspace)
    check(c.isEmpty && c.candidates.isEmpty, "backspace to empty")
    type(c, "ㄋㄏ")
    check(c.handle(.escape).handled && c.isEmpty, "escape clears")
}

// Punctuation and other keys commit first.
do {
    let c = makeComposer()
    check(c.handle(.punctuation("，")) == KeyResult(handled: true, commit: "，"), "punctuation alone")
    type(c, "ㄋㄏㄇ")
    check(c.handle(.punctuation("？")).commit == "你好嗎？", "commit then punctuation")
    type(c, "ㄋㄏㄇ")
    check(c.handle(.other) == KeyResult(handled: false, commit: "你好嗎"), "other key commits, passes")
    // After a partial selection, the rest is committed with its best conversion.
    type(c, "ㄍㄗㄈㄈ")
    _ = select(c, "工作")
    check(c.handle(.punctuation("。")).commit == "工作方法。", "commitAll after partial selection")
}

// A tone after symbols is part of the composition.
do {
    let c = makeComposer()
    type(c, "ㄇㄚˇ")
    check(c.composingText == "ㄇㄚˇ", "tone kept")
    check(c.candidates.first?.text == "馬", "ㄇㄚˇ → \(c.candidates.first?.text ?? "-")")
}

// Very long input flushes instead of growing without bound.
do {
    let c = makeComposer()
    var flushed = ""
    for _ in 0..<(Composer.maxSymbols + 1) {
        flushed += c.handle(.symbol("ㄋ")).commit
    }
    check(!flushed.isEmpty && c.composingText == "ㄋ", "flush at max length")
}

print(failures == 0 ? "OK (\(String(format: "%.2f", Date().timeIntervalSince(started)))s)" : "\(failures) failure(s)")
exit(failures == 0 ? 0 : 1)
