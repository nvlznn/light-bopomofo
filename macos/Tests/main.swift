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

// Whole conversion on Enter.
do {
    let c = makeComposer()
    type(c, "ㄋㄏㄇ")
    check(c.candidates.first?.text == "你好嗎", "first candidate \(c.candidates.first?.text ?? "-")")
    check(c.handle(.commit) == KeyResult(handled: true, commit: "你好嗎"), "enter commits")
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
    for key in [Key.commit, .backspace, .escape, .previous, .next, .other, .symbol("ˇ"), .symbol(Composer.firstTone),
                .select(1), .pagePrevious, .pageNext, .abandon] {
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

// Space is the first tone: ㄊㄧㄢˉ → 天, never second-tone 田 for the whole input.
do {
    let c = makeComposer()
    type(c, "ㄊㄧㄢ")
    check(c.handle(.symbol(Composer.firstTone)) == KeyResult(handled: true), "space is handled")
    check(c.composingText == "ㄊㄧㄢˉ", "first tone shown")
    check(c.candidates.first?.text == "天", "ㄊㄧㄢˉ → \(c.candidates.first?.text ?? "-")")
    check(c.candidates.allSatisfy { $0.text != "田" || $0.consumed < 4 }, "田 filtered out")
    check(c.handle(.commit) == KeyResult(handled: true, commit: "天"), "enter commits 天")
}

// Space after a tone does nothing.
do {
    let c = makeComposer()
    type(c, "ㄇㄚˇ")
    check(c.handle(.symbol(Composer.firstTone)) == KeyResult(handled: true), "space after tone handled")
    check(c.composingText == "ㄇㄚˇ", "space after tone ignored")
}

// Shift+number commits that candidate of the page shown.
do {
    let c = makeComposer()
    type(c, "ㄍㄗㄈㄈ")
    let second = c.candidates[1]
    check(c.handle(.select(2)) == KeyResult(handled: true, commit: second.text), "shift+2 commits #2")
    check(c.symbols.count == 4 - second.consumed, "rest stays after shift+2")
    c.reset()
    type(c, "ㄍㄗㄈㄈ")
    check(c.handle(.select(0)) == KeyResult(handled: true) && c.composingText == "ㄍㄗㄈㄈ", "select 0 ignored")
}

// Pages of nine.
do {
    let c = makeComposer()
    type(c, "ㄕˋ")
    let count = c.candidates.count
    check(count > 2 * Composer.pageSize, "ㄕˋ has several pages")
    _ = c.handle(.next)
    check(c.handle(.pageNext).handled && c.selected == Composer.pageSize, "] goes to page 2")
    let third = c.candidates[Composer.pageSize + 2].text
    let d = makeComposer()
    type(d, "ㄕˋ")
    _ = d.handle(.pageNext)
    check(d.handle(.select(3)) == KeyResult(handled: true, commit: third), "shift+3 on page 2")
    _ = c.handle(.pagePrevious)
    _ = c.handle(.pagePrevious)
    check(c.selected == 0, "[ stops at page 1")
    for _ in 0..<50 { _ = c.handle(.pageNext) }
    check(c.selected == (count - 1) / Composer.pageSize * Composer.pageSize, "] stops at the last page")
}

// Switching to English drops the composition.
do {
    let c = makeComposer()
    type(c, "ㄋㄏㄇ")
    check(c.handle(.abandon) == KeyResult(handled: false), "abandon passes the key")
    check(c.isEmpty && c.candidates.isEmpty, "abandon drops the composition")
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

// Installer: copies into a directory, strips quarantine, replaces in place.
do {
    let fm = FileManager.default
    let tmp = fm.temporaryDirectory.appendingPathComponent("lb-installer-\(UUID().uuidString)")
    defer { try? fm.removeItem(at: tmp) }
    func makeBundle(_ version: String) -> URL {
        let app = tmp.appendingPathComponent("src-\(version)/LightBopomofo.app")
        let contents = app.appendingPathComponent("Contents")
        try! fm.createDirectory(at: contents.appendingPathComponent("MacOS"), withIntermediateDirectories: true)
        let plist: [String: Any] = [
            "CFBundleIdentifier": "test.lightbopomofo.installer",
            "CFBundleShortVersionString": version,
        ]
        (plist as NSDictionary).write(to: contents.appendingPathComponent("Info.plist"), atomically: true)
        try! "binary".write(to: contents.appendingPathComponent("MacOS/LightBopomofo"), atomically: true, encoding: .utf8)
        setxattr(app.path, "com.apple.quarantine", "0081;00000000;test;", 19, 0, 0)
        return app
    }
    let target = tmp.appendingPathComponent("Input Methods")

    let first = Installer(source: makeBundle("1.0"), directory: target, touchesSystem: false)
    check(first.installedVersion == nil, "nothing installed yet")
    do { try first.install() } catch { check(false, "install threw \(error)") }
    check(first.installedVersion == "1.0", "installed 1.0: \(first.installedVersion ?? "-")")
    check(getxattr(first.destination.path, "com.apple.quarantine", nil, 0, 0, 0) < 0, "quarantine removed")

    let second = Installer(source: makeBundle("1.1"), directory: target, touchesSystem: false)
    check(second.sourceVersion.compare("1.0", options: .numeric) == .orderedDescending, "1.1 is newer")
    do { try second.install() } catch { check(false, "reinstall threw \(error)") }
    check(second.installedVersion == "1.1", "replaced with 1.1: \(second.installedVersion ?? "-")")
    let leftovers = (try? fm.contentsOfDirectory(atPath: target.path)) ?? []
    check(leftovers == ["LightBopomofo.app"], "no staging leftovers: \(leftovers)")

    let home = fm.homeDirectoryForCurrentUser
    check(Installer.isInputMethodsCopy(home.appendingPathComponent("Library/Input Methods/LightBopomofo.app")), "user Input Methods copy")
    check(Installer.isInputMethodsCopy(URL(fileURLWithPath: "/Library/Input Methods/LightBopomofo.app")), "system Input Methods copy")
    check(!Installer.isInputMethodsCopy(URL(fileURLWithPath: "/Volumes/輕注音/LightBopomofo.app")), "disk image copy is the installer")
}

// The bopomofo itself opens the third page.
do {
    let c = makeComposer()
    type(c, "ㄋㄏㄇ")
    check(c.candidates.count > 18 && c.candidates[18].text == "ㄋㄏㄇ", "ㄋㄏㄇ is 19th: \(c.candidates.prefix(19).map(\.text))")
    _ = c.handle(.pageNext)
    _ = c.handle(.pageNext)
    check(c.handle(.select(1)) == KeyResult(handled: true, commit: "ㄋㄏㄇ"), "page 3, shift+1 types ㄋㄏㄇ")
    check(c.composingText.isEmpty, "nothing left composing")
}

// The tutorial: each lesson's keys, guidance, and learning carried over.
do {
    let lexicon = CommandLine.arguments[1]
    let engine = Engine(lexiconPath: lexicon, historyPath: nil)!
    let plans: [[Key]] = [
        [.symbol("ㄋ"), .symbol("ㄏ"), .symbol("ㄇ"), .commit],
        [.symbol("ㄇ"), .symbol("ㄊ"), .next, .next, .commit],
        [.symbol("ㄏ"), .symbol("ㄏ"), .pageNext, .next, .commit],
        [.symbol("ㄏ"), .symbol("ㄏ"), .commit],
    ]
    for (lesson, expected) in zip(Lesson.tutorial, plans) {
        let p = Practice(lesson, engine: engine)
        check(p.plan == expected, "\(lesson.target) plan \(p.plan)")
        for (i, key) in p.plan.enumerated() {
            check(p.progress == i && p.nextKey == key, "\(lesson.target) step \(i): \(String(describing: p.progress))")
            p.handle(key)
        }
        check(p.isDone && p.nextKey == nil, "\(lesson.target) done")
    }

    // Off the plan: Esc is the way back; a wrong commit starts over.
    let p = Practice(Lesson.tutorial[1], engine: Engine(lexiconPath: lexicon, historyPath: nil)!)
    p.handle(.symbol("ㄇ"))
    p.handle(.symbol("ㄋ"))
    check(p.nextKey == .escape, "wrong symbol asks for Esc")
    p.handle(.escape)
    check(p.nextKey == .symbol("ㄇ"), "after Esc, back to the start")
    p.handle(.symbol("ㄇ"))
    p.handle(.symbol("ㄊ"))
    p.handle(.commit)
    check(p.wentWrong && !p.isDone, "committed the wrong word")
    check(p.handle(.backspace) && p.committed.count == 1, "backspace deletes committed text")
    p.restart()
    check(p.nextKey == .symbol("ㄇ") && !p.wentWrong, "restart")
}

// Which release is newer (UpdateCheck).
do {
    let json = Data("""
    {"tag_name": "v2.0", "name": "輕注音 2.0",
     "assets": [{"name": "LightBopomofo-Setup.exe"}, {"name": "LightBopomofo.dmg", "size": 8329913}]}
    """.utf8)
    check(UpdateCheck.latestVersion(json: json) == "2.0", "reads v2.0")
    let windowsOnly = Data(#"{"tag_name": "v2.0", "assets": [{"name": "LightBopomofo-Setup.exe"}]}"#.utf8)
    check(UpdateCheck.latestVersion(json: windowsOnly) == nil, "waits for the dmg")
    check(UpdateCheck.latestVersion(json: Data("nope".utf8)) == nil, "not JSON")
    check(UpdateCheck.isNewer("2.0", than: "1.0"), "2.0 > 1.0")
    check(UpdateCheck.isNewer("1.0", than: "0.2.0"), "1.0 > 0.2.0")
    check(UpdateCheck.isNewer("10.0", than: "9.0"), "10.0 > 9.0")
    check(!UpdateCheck.isNewer("1.0", than: "1.0.0"), "1.0 = 1.0.0")
    check(!UpdateCheck.isNewer("0.1.0", than: "1.0"), "0.1.0 < 1.0")
}

print(failures == 0 ? "OK (\(String(format: "%.2f", Date().timeIntervalSince(started)))s)" : "\(failures) failure(s)")
exit(failures == 0 ? 0 : 1)
