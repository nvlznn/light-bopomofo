//! The first-launch tutorial's exercises: what to type, which key comes
//! next, and whether the user got there. Platform apps draw it.
//!
//! Mirrors macos/App/Lesson.swift (same lessons, same test scenarios).

use crate::composer::{Composer, Key, PAGE_SIZE};
use crate::engine::Engine;

/// One exercise: type `symbols`, then choose `pieces` one after another
/// (usually one piece: the whole `target`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lesson {
    pub symbols: &'static str,
    pub target: &'static str,
    pub pieces: &'static [&'static str],
}

impl Lesson {
    const fn new(symbols: &'static str, target: &'static str, pieces: &'static [&'static str]) -> Self {
        Lesson { symbols, target, pieces }
    }
}

/// The exercises, in order: type, choose, turn the page, choose in pieces
/// when the whole comes out wrong (ㄇㄇㄒㄒㄋ is 密碼謝謝你 at first), and see
/// the pieces come back as one.
pub const LESSONS: [Lesson; 5] = [
    Lesson::new("ㄋㄏㄇ", "你好嗎", &["你好嗎"]),
    Lesson::new("ㄇㄊ", "明天", &["明天"]),
    Lesson::new("ㄏㄏ", "還好", &["還好"]),
    Lesson::new("ㄇㄇㄒㄒㄋ", "媽媽謝謝你", &["媽媽", "謝謝你"]),
    Lesson::new("ㄇㄇㄒㄒㄋ", "媽媽謝謝你", &["媽媽謝謝你"]),
];

/// The moves that choose one piece, from the first candidate.
fn moves(index: usize) -> Vec<Key> {
    let mut keys = vec![Key::PageNext; index / PAGE_SIZE];
    keys.extend(std::iter::repeat(Key::Next).take(index % PAGE_SIZE));
    keys
}

/// A lesson in progress on its own composer.
pub struct Practice {
    lesson: Lesson,
    composer: Composer,
    /// The keys that do the lesson from scratch, with the candidates as they
    /// are now (so the plan follows the lexicon and what was learned).
    plan: Vec<Key>,
    /// Per piece: the bopomofo still composing when it is chosen, and where
    /// its moves start in `plan`.
    pieces: Vec<(String, usize)>,
    /// Committed during this attempt.
    committed: String,
    done: bool,
}

impl Practice {
    pub fn new(lesson: Lesson, engine: &Engine) -> Self {
        let mut plan: Vec<Key> = lesson.symbols.chars().map(Key::Symbol).collect();
        let mut pieces = Vec::new();
        let mut rest: Vec<char> = lesson.symbols.chars().collect();
        for piece in lesson.pieces {
            let candidates = engine.query(&rest);
            let (index, consumed) = candidates
                .iter()
                .enumerate()
                .find(|(_, c)| c.text == *piece)
                .map_or((0, rest.len()), |(i, c)| (i, c.consumed));
            pieces.push((rest.iter().collect(), plan.len()));
            plan.extend(moves(index));
            plan.push(Key::Commit);
            rest.drain(..consumed.clamp(1, rest.len()));
        }
        Practice {
            lesson,
            composer: Composer::new(),
            plan,
            pieces,
            committed: String::new(),
            done: false,
        }
    }

    pub fn lesson(&self) -> Lesson {
        self.lesson
    }

    pub fn composer(&self) -> &Composer {
        &self.composer
    }

    pub fn plan(&self) -> &[Key] {
        &self.plan
    }

    pub fn committed(&self) -> &str {
        &self.committed
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Feeds a key to the composer. Returns whether it was used.
    pub fn handle(&mut self, engine: &mut Engine, key: Key) -> bool {
        if self.done {
            return false;
        }
        let result = self.composer.handle(engine, key);
        self.committed += &result.commit;
        if self.composer.is_empty() && self.committed == self.lesson.target {
            self.done = true;
        }
        result.handled || !result.commit.is_empty()
    }

    /// Committed something and stopped short of the target: start over.
    pub fn went_wrong(&self) -> bool {
        self.composer.is_empty() && !self.committed.is_empty() && !self.done
    }

    pub fn restart(&mut self) {
        self.composer.reset();
        self.committed.clear();
    }

    /// How many keys of the plan are done, or `None` when the user is off it
    /// (and should press Esc).
    pub fn progress(&self) -> Option<usize> {
        if self.done {
            return Some(self.plan.len());
        }
        let typed = self.composer.composing_text();
        if self.committed.is_empty() {
            let symbols = self.lesson.symbols;
            if !symbols.starts_with(&typed) {
                return None;
            }
            if typed.chars().count() < symbols.chars().count() {
                return Some(typed.chars().count());
            }
        }
        // Which piece is being chosen: the ones before it are committed.
        let mut chosen = String::new();
        for (piece, (rest, start)) in self.lesson.pieces.iter().zip(&self.pieces) {
            if chosen == self.committed && typed == *rest {
                return self.progress_in_piece(*start);
            }
            chosen += piece;
        }
        None
    }

    /// Where the selection is along the moves starting at `start`.
    fn progress_in_piece(&self, start: usize) -> Option<usize> {
        let mut selected = 0;
        let mut done = start;
        if selected == self.composer.selected() {
            return Some(done);
        }
        for key in self.plan[start..].iter().take_while(|k| **k != Key::Commit) {
            selected = if *key == Key::PageNext {
                (selected / PAGE_SIZE + 1) * PAGE_SIZE
            } else {
                selected + 1
            };
            done += 1;
            if selected == self.composer.selected() {
                return Some(done);
            }
        }
        None
    }

    /// The key to press now.
    pub fn next_key(&self) -> Option<Key> {
        if self.done {
            return None;
        }
        match self.progress() {
            Some(i) => Some(self.plan[i].clone()),
            None => Some(Key::Escape),
        }
    }
}
