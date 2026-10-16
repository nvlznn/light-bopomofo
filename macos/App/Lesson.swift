import Foundation

/// One exercise of the tutorial: type `symbols`, end up with `target`.
struct Lesson: Equatable {
    let symbols: String
    let target: String
}

/// A lesson in progress on its own composer: what the user has typed so far
/// and which key comes next.
final class Practice {
    let lesson: Lesson
    let composer: Composer
    /// The keys that do the lesson from scratch, with the candidates as they
    /// are now (so the plan follows the lexicon and what was learned).
    let plan: [Key]
    /// Committed during this attempt.
    private(set) var committed = ""
    private(set) var isDone = false

    init(_ lesson: Lesson, engine: Engine) {
        self.lesson = lesson
        composer = Composer(engine: engine)
        let index = engine.query(lesson.symbols).firstIndex { $0.text == lesson.target } ?? 0
        let page = index / Composer.pageSize
        plan = lesson.symbols.map { Key.symbol($0) }
            + Array(repeating: .pageNext, count: page)
            + Array(repeating: .next, count: index % Composer.pageSize)
            + [.commit]
    }

    /// Feeds a key to the composer. Returns whether it was used.
    @discardableResult
    func handle(_ key: Key) -> Bool {
        guard !isDone else { return false }
        if key == .backspace, composer.isEmpty, !committed.isEmpty {
            committed.removeLast()
            return true
        }
        let result = composer.handle(key)
        committed += result.commit
        if composer.isEmpty, committed == lesson.target {
            isDone = true
        }
        return result.handled || !result.commit.isEmpty
    }

    /// Committed something other than the target: start over.
    var wentWrong: Bool { composer.isEmpty && !committed.isEmpty && !isDone }

    func restart() {
        composer.reset()
        committed = ""
    }

    /// How many keys of `plan` are done, or nil when the user is off it
    /// (and should press Esc).
    var progress: Int? {
        if isDone { return plan.count }
        guard committed.isEmpty else { return nil }
        let typed = composer.composingText
        let symbols = lesson.symbols
        guard symbols.hasPrefix(typed) else { return nil }
        if typed.count < symbols.count { return typed.count }
        // All symbols typed: where the selection is along the plan's moves.
        var selected = 0
        var done = symbols.count
        if selected == composer.selected { return done }
        for key in plan.dropFirst(symbols.count) where key != .commit {
            selected = key == .pageNext
                ? (selected / Composer.pageSize + 1) * Composer.pageSize
                : selected + 1
            done += 1
            if selected == composer.selected { return done }
        }
        return nil
    }

    /// The key to press now.
    var nextKey: Key? {
        guard !isDone else { return nil }
        guard let progress else { return .escape }
        return plan[progress]
    }
}

extension Lesson {
    /// The tutorial's exercises, in order: type, choose, turn the page, and
    /// see the choice come back first.
    static let tutorial = [
        Lesson(symbols: "ㄋㄏㄇ", target: "你好嗎"),
        Lesson(symbols: "ㄇㄊ", target: "明天"),
        Lesson(symbols: "ㄏㄏ", target: "還好"),
        Lesson(symbols: "ㄏㄏ", target: "還好"),
    ]
}
