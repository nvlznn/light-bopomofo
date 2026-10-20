import Foundation

/// One exercise of the tutorial: type `symbols`, then choose `pieces` one
/// after another (usually one piece: the whole `target`).
struct Lesson: Equatable {
    let symbols: String
    let target: String
    let pieces: [String]

    init(symbols: String, target: String, pieces: [String]? = nil) {
        self.symbols = symbols
        self.target = target
        self.pieces = pieces ?? [target]
    }
}

/// A lesson in progress on its own composer: what the user has typed so far
/// and which key comes next.
final class Practice {
    let lesson: Lesson
    let composer: Composer
    /// The keys that do the lesson from scratch, with the candidates as they
    /// are now (so the plan follows the lexicon and what was learned).
    let plan: [Key]
    /// Per piece: the bopomofo still composing when it is chosen, and where
    /// its moves start in `plan`.
    private let pieces: [(rest: String, start: Int)]
    /// Committed during this attempt.
    private(set) var committed = ""
    private(set) var isDone = false

    init(_ lesson: Lesson, engine: Engine) {
        self.lesson = lesson
        composer = Composer(engine: engine)
        var plan = lesson.symbols.map { Key.symbol($0) }
        var pieces: [(String, Int)] = []
        var rest = lesson.symbols
        for piece in lesson.pieces {
            let candidates = Composer.candidates(engine, rest).0
            let index = candidates.firstIndex { $0.text == piece } ?? 0
            let consumed = candidates.indices.contains(index) ? candidates[index].consumed : rest.count
            pieces.append((rest, plan.count))
            plan += Array(repeating: .pageNext, count: index / Composer.pageSize)
            plan += Array(repeating: .next, count: index % Composer.pageSize)
            plan.append(.commit)
            rest = String(rest.dropFirst(min(max(consumed, 1), rest.count)))
        }
        self.plan = plan
        self.pieces = pieces
    }

    /// Feeds a key to the composer. Returns whether it was used.
    @discardableResult
    func handle(_ key: Key) -> Bool {
        guard !isDone else { return false }
        let result = composer.handle(key)
        committed += result.commit
        if composer.isEmpty, committed == lesson.target {
            isDone = true
        }
        return result.handled || !result.commit.isEmpty
    }

    /// Committed something and stopped short of the target: start over.
    var wentWrong: Bool { composer.isEmpty && !committed.isEmpty && !isDone }

    func restart() {
        composer.reset()
        committed = ""
    }

    /// How many keys of `plan` are done, or nil when the user is off it
    /// (and should press Esc).
    var progress: Int? {
        if isDone { return plan.count }
        let typed = composer.composingText
        if committed.isEmpty {
            guard lesson.symbols.hasPrefix(typed) else { return nil }
            if typed.count < lesson.symbols.count { return typed.count }
        }
        // Which piece is being chosen: the ones before it are committed.
        var chosen = ""
        for (piece, (rest, start)) in zip(lesson.pieces, pieces) {
            if chosen == committed, typed == rest { return progress(from: start) }
            chosen += piece
        }
        return nil
    }

    /// Where the selection is along the moves starting at `start`.
    private func progress(from start: Int) -> Int? {
        var selected = 0
        var done = start
        if selected == composer.selected { return done }
        for key in plan[start...].prefix(while: { $0 != .commit }) {
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
    /// The tutorial's exercises, in order: type, choose, turn the page,
    /// choose in pieces when the whole comes out wrong (ㄇㄇㄒㄒㄋ is 密碼謝謝你
    /// at first), and see the pieces come back as one.
    static let tutorial = [
        Lesson(symbols: "ㄋㄏㄇ", target: "你好嗎"),
        Lesson(symbols: "ㄇㄊ", target: "明天"),
        Lesson(symbols: "ㄏㄏ", target: "還好"),
        Lesson(symbols: "ㄇㄇㄒㄒㄋ", target: "媽媽謝謝你", pieces: ["媽媽", "謝謝你"]),
        Lesson(symbols: "ㄇㄇㄒㄒㄋ", target: "媽媽謝謝你"),
    ]
}
