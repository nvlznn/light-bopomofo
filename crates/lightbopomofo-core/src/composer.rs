//! The composition state machine: bopomofo typed so far, its candidates and
//! the selection, driven by platform-independent keys. Platform shells
//! translate OS key events into [`Key`]s and render the state.
//!
//! Mirrors macos/Shared/Composer.swift (same behaviour, same test scenarios).

use crate::bopomofo::FIRST_TONE;
use crate::dachen;
use crate::engine::{english_symbols, Candidate, Engine};

/// Platform-independent key, already translated from the OS event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// A bopomofo symbol or tone mark; Space is the first-tone mark ˉ.
    Symbol(char),
    /// Enter: commit the selected candidate.
    Commit,
    Backspace,
    Escape,
    /// ←, ↑, Shift+Tab.
    Previous,
    /// →, ↓, Tab.
    Next,
    /// Shift+1–9 while composing: commit that candidate of the current page.
    Select(usize),
    /// [, PageUp.
    PagePrevious,
    /// ], PageDown.
    PageNext,
    /// Full-width punctuation (Shift + , . / 1 ;).
    Punctuation(String),
    /// An uppercase letter (Shift + A–Z): starts English, which goes on
    /// until Space, Enter or punctuation (iPhone, API, 4G).
    Letter(char),
    /// Anything else: commit the composition, then let the app have the key.
    Other,
    /// Switched to English (Caps Lock): drop the composition, then let the
    /// app have the key.
    Abandon,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyResult {
    /// Whether the key was consumed (false: the app should process it).
    pub handled: bool,
    /// Text to insert into the document before anything else.
    pub commit: String,
}

impl KeyResult {
    fn pass() -> Self {
        Self::default()
    }

    fn handled(commit: String) -> Self {
        KeyResult {
            handled: true,
            commit,
        }
    }
}

pub const MAX_SYMBOLS: usize = 40;
/// Candidates per page of the candidate window, numbered 1–9.
pub const PAGE_SIZE: usize = 9;

#[derive(Default)]
pub struct Composer {
    symbols: Vec<char>,
    candidates: Vec<Candidate>,
    selected: usize,
    /// What this composition has committed so far, learned as a whole when
    /// it ends after several partial selections (工作 + 方法 → 工作方法).
    session_typed: String,
    session_text: String,
    session_pieces: usize,
    /// English typed since a Shift+letter, after the bopomofo in `symbols`;
    /// `None` while typing bopomofo.
    english: Option<String>,
    /// For each candidate while typing English: its English part.
    english_parts: Vec<String>,
    /// The candidate that types the keys themselves (app, ok), if offered.
    keys_at: Option<usize>,
    /// Moved among the candidates (Tab, arrows, pages) since the composition
    /// last changed: the number keys choose on the page shown and Space
    /// commits, as in a candidate window (唯音); other keys type as usual.
    choosing: bool,
}

/// Lowercase starts of names typed with a capital inside (iPhone, macOS):
/// the keys typed right before the capital that join the English. Only
/// ones that are rare as the end of 簡拼 (ㄛ, ㄩㄇㄏ).
const CAMEL_STARTS: [&str; 2] = ["mac", "i"];

/// The keys that type `symbols`.
fn keys(symbols: &[char]) -> String {
    symbols
        .iter()
        .map(|&s| dachen::key(s).unwrap_or(s))
        .collect()
}

/// Where English typed after `symbols` most likely starts: the lowercase
/// start of iPhone or macOS, and digits right before it (5G, 1080P). A tone
/// key counts as a digit only first or after another digit (4K, but
/// ㄋㄧˇ + API is 你API).
fn english_start(symbols: &[char]) -> usize {
    let typed: Vec<char> = keys(symbols).chars().collect();
    let mut start = typed.len();
    for camel in CAMEL_STARTS {
        let camel: Vec<char> = camel.chars().collect();
        if typed.ends_with(&camel) {
            start -= camel.len();
            break;
        }
    }
    while start > 0 && typed[start - 1].is_ascii_digit() {
        if is_tone(symbols[start - 1]) && start >= 2 && !typed[start - 2].is_ascii_digit() {
            break;
        }
        start -= 1;
    }
    start
}

/// The candidates for bopomofo being composed: the engine's, with the keys
/// themselves for English typed in lowercase (app, ok).
pub fn candidates(engine: &Engine, symbols: &[char]) -> Vec<Candidate> {
    let mut candidates = engine.query(symbols);
    offer_keys(symbols, &mut candidates);
    candidates
}

/// Offer the keys themselves at the end of the first page, keeping the
/// bopomofo candidate where the engine put it. Returns where they went.
/// Only when some symbol is not an initial: 簡拼 is mostly initials (ㄏㄏ is
/// not cc), while English keeps hitting the finals' keys (app is ㄇㄣㄣ).
fn offer_keys(symbols: &[char], candidates: &mut Vec<Candidate>) -> Option<usize> {
    let initial = |&s: &char| crate::bopomofo::kind(s) == Some(crate::bopomofo::Kind::Initial);
    if symbols.is_empty() || symbols.contains(&FIRST_TONE) || symbols.iter().all(initial) {
        return None;
    }
    let text = keys(symbols);
    if !text.chars().any(|c| c.is_ascii_alphabetic()) || candidates.iter().any(|c| c.text == text) {
        return None;
    }
    let bopomofo: String = symbols.iter().collect();
    let bopomofo_at = candidates.iter().position(|c| c.text == bopomofo);
    let at = candidates.len().min(PAGE_SIZE - 1);
    candidates.insert(at, typed_candidate(text, symbols.len()));
    if let Some(b) = bopomofo_at.filter(|&b| b > at) {
        let moved = candidates.remove(b + 1);
        candidates.insert(b, moved);
    }
    Some(at)
}

fn typed_candidate(text: String, consumed: usize) -> Candidate {
    Candidate {
        text,
        consumed,
        score: 0.0,
        words: Vec::new(),
    }
}

fn is_tone(c: char) -> bool {
    crate::bopomofo::is_tone(c)
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty() && self.english.is_none()
    }

    /// The bopomofo (and English) shown inline while composing.
    pub fn composing_text(&self) -> String {
        let mut text: String = self.symbols.iter().collect();
        text.push_str(self.english.as_deref().unwrap_or(""));
        text
    }

    fn len(&self) -> usize {
        self.symbols.len() + self.english.as_ref().map_or(0, |e| e.chars().count())
    }

    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Whether the number keys choose (see `choosing`).
    pub fn is_choosing(&self) -> bool {
        self.choosing
    }

    pub fn handle(&mut self, engine: &mut Engine, key: Key) -> KeyResult {
        let key = match key {
            // 1–9 are bopomofo and tones on 大千; while choosing, they choose.
            Key::Symbol(s) if self.choosing => match dachen::key(s) {
                Some(' ') => Key::Commit,
                Some(d @ '1'..='9') => Key::Select(d as usize - '0' as usize),
                _ => Key::Symbol(s),
            },
            key => key,
        };
        if self.english.is_some() {
            if let Some(result) = self.handle_english(engine, &key) {
                return result;
            }
        }
        match key {
            Key::Letter(c) => {
                if self.is_empty() {
                    engine.refresh_history();
                }
                let flushed = if self.len() >= MAX_SYMBOLS {
                    self.commit_all(engine)
                } else {
                    String::new()
                };
                self.english.get_or_insert_with(String::new).push(c);
                self.refresh(engine);
                KeyResult::handled(flushed)
            }
            Key::Symbol(s) => {
                if self.is_empty() {
                    if is_tone(s) {
                        return KeyResult::pass();
                    }
                    // A new composition: pick up what other processes learned
                    // or forgot (one metadata call when nothing changed).
                    engine.refresh_history();
                } else if s == FIRST_TONE && self.symbols.last().is_some_and(|&c| is_tone(c)) {
                    // Space after a tone: the syllable is already finished.
                    return KeyResult::handled(String::new());
                }
                let flushed = if self.symbols.len() >= MAX_SYMBOLS {
                    self.commit_all(engine)
                } else {
                    String::new()
                };
                self.symbols.push(s);
                self.refresh(engine);
                KeyResult::handled(flushed)
            }
            Key::Commit => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                let text = self.commit_selected(engine);
                KeyResult::handled(text)
            }
            Key::Backspace => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                if let Some(english) = &mut self.english {
                    english.pop();
                    if english.is_empty() {
                        self.english = None;
                    }
                } else {
                    self.symbols.pop();
                }
                if self.is_empty() {
                    self.reset_session();
                }
                self.refresh(engine);
                KeyResult::handled(String::new())
            }
            Key::Escape => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                self.reset();
                KeyResult::handled(String::new())
            }
            Key::Previous => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                self.choosing = true;
                self.selected = self.selected.saturating_sub(1);
                KeyResult::handled(String::new())
            }
            Key::Next => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                self.choosing = true;
                self.selected = (self.selected + 1).min(self.candidates.len().saturating_sub(1));
                KeyResult::handled(String::new())
            }
            Key::Select(n) => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                let index = self.selected / PAGE_SIZE * PAGE_SIZE + n.saturating_sub(1);
                if n == 0 || n > PAGE_SIZE || index >= self.candidates.len() {
                    return KeyResult::handled(String::new());
                }
                self.selected = index;
                let text = self.commit_selected(engine);
                KeyResult::handled(text)
            }
            Key::PagePrevious | Key::PageNext => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                self.choosing = true;
                let page = self.selected / PAGE_SIZE;
                let last = self.candidates.len().saturating_sub(1) / PAGE_SIZE;
                let page = if key == Key::PageNext {
                    (page + 1).min(last)
                } else {
                    page.saturating_sub(1)
                };
                self.selected = page * PAGE_SIZE;
                KeyResult::handled(String::new())
            }
            Key::Punctuation(p) => {
                // While composing, punctuation is ignored (as in the system 注音).
                KeyResult::handled(if self.is_empty() { p } else { String::new() })
            }
            Key::Other => KeyResult {
                handled: false,
                commit: self.commit_all(engine),
            },
            Key::Abandon => {
                self.reset();
                KeyResult::pass()
            }
        }
    }

    /// Commit the selected candidate, then the best conversion of whatever
    /// remains, leaving the composition empty.
    pub fn commit_all(&mut self, engine: &mut Engine) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut text = self.commit_selected(engine);
        while !self.is_empty() {
            self.selected = 0;
            text += &self.commit_selected(engine);
        }
        text
    }

    /// Drop the composition without committing.
    pub fn reset(&mut self) {
        self.symbols.clear();
        self.english = None;
        self.candidates.clear();
        self.english_parts.clear();
        self.keys_at = None;
        self.selected = 0;
        self.choosing = false;
        self.reset_session();
    }

    /// Keys that end or edit English; `None` for the ones handled as usual
    /// (choosing among the candidates, Backspace, Esc…).
    fn handle_english(&mut self, engine: &mut Engine, key: &Key) -> Option<KeyResult> {
        let result = match key {
            // Space ends the word and types itself.
            Key::Symbol(FIRST_TONE) => KeyResult {
                handled: false,
                commit: self.commit_all(engine),
            },
            Key::Symbol(s) => {
                let flushed = if self.len() >= MAX_SYMBOLS {
                    self.commit_all(engine)
                } else {
                    String::new()
                };
                let c = dachen::key(*s).unwrap_or(*s);
                self.english.get_or_insert_with(String::new).push(c);
                self.refresh(engine);
                KeyResult::handled(flushed)
            }
            Key::Punctuation(p) => {
                let text = self.commit_all(engine);
                KeyResult::handled(text + p)
            }
            _ => return None,
        };
        Some(result)
    }

    fn commit_selected(&mut self, engine: &mut Engine) -> String {
        let Some(choice) = self.candidates.get(self.selected).cloned() else {
            // No candidates (should not happen): give back the raw symbols.
            let raw = self.composing_text();
            self.reset();
            return raw;
        };
        if self.english.is_some() || self.keys_at == Some(self.selected) {
            // English covers the whole composition. It is learned as typed by
            // its keys, so typing it in lowercase brings it back (Figma).
            let english = match self.english_parts.get(self.selected) {
                Some(part) => part.clone(),
                None => choice.text.clone(),
            };
            if let Some(symbols) = english_symbols(&english) {
                engine.learn(&symbols, &english);
            }
            self.reset();
            return choice.text;
        }
        let consumed = choice.consumed.clamp(1, self.symbols.len());
        engine.learn(&self.symbols, &choice.text);
        self.session_typed.extend(&self.symbols[..consumed]);
        self.session_text.push_str(&choice.text);
        self.session_pieces += 1;
        self.symbols.drain(..consumed);
        if self.is_empty() {
            if self.session_pieces > 1 {
                let typed: Vec<char> = self.session_typed.chars().collect();
                engine.learn(&typed, &self.session_text);
            }
            self.reset_session();
        }
        self.refresh(engine);
        choice.text
    }

    fn reset_session(&mut self) {
        self.session_typed.clear();
        self.session_text.clear();
        self.session_pieces = 0;
    }

    fn refresh(&mut self, engine: &mut Engine) {
        self.selected = 0;
        self.choosing = false;
        self.keys_at = None;
        self.english_parts.clear();
        self.candidates = if let Some(english) = self.english.clone() {
            self.english_candidates(engine, &english)
        } else if self.symbols.is_empty() {
            Vec::new()
        } else {
            let mut candidates = engine.query(&self.symbols);
            self.keys_at = offer_keys(&self.symbols, &mut candidates);
            candidates
        };
    }

    /// While typing English: the bopomofo before it converted, then the
    /// English; first where it most likely starts, then the other places
    /// it could (Tab picks one), down to all of it as English.
    fn english_candidates(&mut self, engine: &Engine, english: &str) -> Vec<Candidate> {
        let n = self.symbols.len();
        let mut splits = vec![english_start(&self.symbols)];
        splits.extend((n.saturating_sub(3)..=n).rev());
        splits.push(0);
        let mut candidates: Vec<Candidate> = Vec::new();
        for split in splits {
            let chinese = if split == 0 {
                String::new()
            } else {
                let before = &self.symbols[..split];
                engine
                    .query(before)
                    .into_iter()
                    .find(|c| c.consumed == split)
                    .map_or_else(|| before.iter().collect(), |c| c.text)
            };
            let part = keys(&self.symbols[split..]) + english;
            let text = chinese + &part;
            if !candidates.iter().any(|c| c.text == text) {
                candidates.push(typed_candidate(text, n));
                self.english_parts.push(part);
            }
        }
        candidates
    }
}
