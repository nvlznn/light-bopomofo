// Composer behaviour against the real engine and lexicon.
// Run with macos/test.sh.

import AppKit

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
    for key in [Key.commit, .backspace, .escape, .previous, .next, .other, .symbol(Composer.firstTone),
                .select(1), .pagePrevious, .pageNext, .abandon] {
        check(c.handle(key) == KeyResult(handled: false), "\(key) passes through when empty")
    }
    // A tone alone types its mark, as in the system 注音, once the next key
    // shows it is not the start of English (4G).
    for tone in "ˊˇˋ˙" {
        check(c.handle(.symbol(tone)) == KeyResult(handled: true), "\(tone) alone waits")
        check(c.composingText == String(tone), "\(tone) shown")
        check(c.handle(.symbol("ㄋ")) == KeyResult(handled: true, commit: String(tone)), "\(tone) typed by the next key")
        c.reset()
    }
    check(c.handle(.symbol("ˋ")) == KeyResult(handled: true), "ˋ waits")
    check(c.handle(.symbol(Composer.firstTone)) == KeyResult(handled: false, commit: "ˋ"), "then Space types ˋ and a space")
    check(c.isEmpty, "nothing left")
    _ = c.handle(.symbol("ˋ"))
    _ = c.handle(.next)
    check(c.handle(.commit) == KeyResult(handled: true, commit: "4"), "Tab picks the digit")
}

// Mixed English: a capital starts English, Space ends it.
do {
    let engine = Engine(lexiconPath: CommandLine.arguments[1], historyPath: nil)!
    let c = Composer(engine: engine)
    func keys(_ keys: String) {
        for k in keys {
            _ = c.handle(k.isUppercase ? .letter(k) : .symbol(Engine.bopomofo(forKey: k)!))
        }
    }
    func whole(_ input: String) -> String {
        engine.query(input).first { $0.consumed == input.count }!.text
    }
    keys("iPhone")
    check(c.composingText == "ㄛPhone", "shown as typed: \(c.composingText)")
    check(c.candidates.first?.text == "iPhone", "iPhone: \(c.candidates.prefix(3).map(\.text))")
    check(c.handle(.symbol(Composer.firstTone)) == KeyResult(handled: false, commit: "iPhone"), "space ends it")
    check(c.isEmpty, "empty after English")

    type(c, "ㄋㄏ")
    keys("API")
    check(c.candidates.first?.text == whole("ㄋㄏ") + "API", "你好API: \(c.candidates.prefix(3).map(\.text))")
    check(select(c, "scAPI"), "other starts a Tab away")
    check(c.handle(.commit) == KeyResult(handled: true, commit: "scAPI"), "commit scAPI")

    type(c, "ㄋㄏ")
    keys("iPad")
    check(c.candidates.first?.text == whole("ㄋㄏ") + "iPad", "takes the i: \(c.candidates.first?.text ?? "-")")
    c.reset()
    keys("5G")
    check(c.candidates.first?.text == "5G", "5G: \(c.candidates.first?.text ?? "-")")
    c.reset()
    keys("1080P")
    check(c.candidates.first?.text == "1080P", "1080P")
    c.reset()
    type(c, "ㄋㄧˇ")
    keys("API")
    check(c.candidates.first?.text == whole("ㄋㄧˇ") + "API", "a tone after bopomofo stays a tone")
    c.reset()

    // 4G: the tone alone waits, the capital makes it a digit.
    _ = c.handle(.symbol("ˋ"))
    keys("G")
    check(c.candidates.first?.text == "4G", "4G: \(c.candidates.first?.text ?? "-")")
    check(c.handle(.punctuation("，")) == KeyResult(handled: true, commit: "4G，"), "punctuation ends it")

    keys("Node")
    _ = c.handle(.symbol("ㄡ"))  // .
    keys("js")
    check(c.candidates.first?.text == "Node.js", "Node.js: \(c.candidates.first?.text ?? "-")")
    c.reset()

    type(c, "ㄋㄏ")
    keys("A")
    check(c.handle(.backspace).handled && c.composingText == "ㄋㄏ", "backspace back into bopomofo")
    check(c.candidates.first?.text == whole("ㄋㄏ"), "bopomofo candidates again")
    c.reset()

    // English words the engine knows come out typed in lowercase, no Shift.
    keys("google")
    check(c.candidates.first?.text == "Google", "google: \(c.candidates.first?.text ?? "-")")
    c.reset()
    type(c, "ㄋㄏ")
    keys("email")
    check(c.candidates.first?.text == whole("ㄋㄏ") + "email", "你好email: \(c.candidates.first?.text ?? "-")")
    c.reset()

    // Other lowercase English: the keys at the end of the first page, then learned.
    keys("kahoot")
    let at = c.candidates.firstIndex { $0.text == "kahoot" }
    check(at == min(c.candidates.count - 1, Composer.pageSize - 1), "kahoot at \(String(describing: at))")
    check(c.handle(.select((at ?? 0) + 1)) == KeyResult(handled: true, commit: "kahoot"), "shift+number types kahoot")
    keys("kahoot")
    check(c.candidates.first?.text == "kahoot", "kahoot learned: \(c.candidates.first?.text ?? "-")")
    c.reset()
    // Typed with Shift once: comes back in lowercase.
    keys("Figjam")
    _ = c.handle(.commit)
    keys("figjam")
    check(c.candidates.first?.text == "Figjam", "Figjam learned: \(c.candidates.first?.text ?? "-")")
    c.reset()
    check(engine.newWords().isEmpty, "English is never shared")
    // Not offered for initials only (簡拼).
    type(c, "ㄏㄏ")
    check(!c.candidates.contains { $0.text == "cc" }, "no cc for ㄏㄏ")
}

// Key events → keys: Shift+Option types what Shift does, but ” 《 》 on ' , .
do {
    func key(_ code: UInt16, _ flags: NSEvent.ModifierFlags, _ chars: String) -> Key {
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0,
                                     context: nil, characters: chars, charactersIgnoringModifiers: chars,
                                     isARepeat: false, keyCode: code)!
        return Key(event: event, composing: false)
    }
    let both: NSEvent.ModifierFlags = [.shift, .option]
    check(key(18, both, "1") == .punctuation("！"), "Shift+Option+1")
    check(key(33, both, "[") == .punctuation("『"), "Shift+Option+[")
    check(key(39, both, "'") == .punctuation("”"), "Shift+Option+'")
    check(key(43, both, ",") == .punctuation("《"), "Shift+Option+,")
    check(key(47, both, ".") == .punctuation("》"), "Shift+Option+.")
    check(key(47, .option, ".") == .punctuation("．"), "Option+.")
    check(key(0, both, "a") == .other, "Shift+Option+letter goes to the app")
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

// Punctuation types only when nothing is being composed; other keys commit first.
do {
    let c = makeComposer()
    check(c.handle(.punctuation("，")) == KeyResult(handled: true, commit: "，"), "punctuation alone")
    type(c, "ㄋㄏㄇ")
    check(c.handle(.punctuation("？")) == KeyResult(handled: true), "punctuation ignored while composing")
    check(!c.isEmpty, "composition kept after ignored punctuation")
    check(c.handle(.other) == KeyResult(handled: false, commit: "你好嗎"), "other key commits, passes")
    // Also after a partial selection.
    type(c, "ㄍㄗㄈㄈ")
    _ = select(c, "工作")
    check(c.handle(.punctuation("。")) == KeyResult(handled: true), "ignored after partial selection")
    check(!c.isEmpty, "still composing after partial selection")
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
    func keys(_ symbols: String, _ then: [Key]) -> [Key] { symbols.map { Key.symbol($0) } + then }
    check(engine.query("ㄇㄇㄒㄒㄋ").first?.text == "密碼謝謝你", "the whole comes out wrong at first")
    let plans: [[Key]] = [
        keys("ㄋㄏㄇ", [.commit]),
        keys("ㄇㄊ", [.next, .next, .commit]),
        keys("ㄏㄏ", [.pageNext, .next, .commit]),
        // 媽媽 is 4th; then ㄒㄒㄋ is 謝謝你 first.
        keys("ㄇㄇㄒㄒㄋ", [.next, .next, .next, .commit, .commit]),
        // Learned from the pieces: first as a whole.
        keys("ㄇㄇㄒㄒㄋ", [.commit]),
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
    p.restart()
    check(p.nextKey == .symbol("ㄇ") && !p.wentWrong, "restart")

    // A wrong piece: Esc, then start over.
    let q = Practice(Lesson.tutorial[3], engine: Engine(lexiconPath: lexicon, historyPath: nil)!)
    for s in "ㄇㄇㄒㄒㄋ" { q.handle(.symbol(s)) }
    let wrong = q.composer.candidates.firstIndex { $0.text == "密碼" } ?? 1
    for _ in 0..<wrong { q.handle(.next) }
    q.handle(.commit)
    check(q.committed == "密碼" && q.nextKey == .escape, "wrong piece asks for Esc: \(q.committed)")
    q.handle(.escape)
    check(q.wentWrong, "Esc leaves the wrong piece: start over")
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

// The version left for the input method's menu (PendingUpdate).
do {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("lb-pending-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: dir) }
    check(PendingUpdate.version(newerThan: "2.0", in: dir) == nil, "nothing recorded")
    PendingUpdate.record("3.0", in: dir)
    check(PendingUpdate.version(newerThan: "2.0", in: dir) == "3.0", "3.0 offered to 2.0")
    check(PendingUpdate.version(newerThan: "3.0", in: dir) == nil, "not offered once updated")
    check(PendingUpdate.version(newerThan: "10.0", in: dir) == nil, "never an older version")
    PendingUpdate.record(nil, in: dir)
    check(PendingUpdate.version(newerThan: "2.0", in: dir) == nil, "forgotten")
}

print(failures == 0 ? "OK (\(String(format: "%.2f", Date().timeIntervalSince(started)))s)" : "\(failures) failure(s)")
exit(failures == 0 ? 0 : 1)
