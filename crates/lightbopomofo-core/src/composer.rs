//! The composition state machine: bopomofo typed so far, its candidates and
//! the selection, driven by platform-independent keys. Platform shells
//! translate OS key events into [`Key`]s and render the state.
//!
//! Mirrors macos/IME/Composer.swift (same behaviour, same test scenarios).

use crate::engine::{Candidate, Engine};

/// Platform-independent key, already translated from the OS event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// A bopomofo symbol or tone mark.
    Symbol(char),
    /// Space, Enter.
    Commit,
    Backspace,
    Escape,
    /// ←, ↑, Shift+Tab.
    Previous,
    /// →, ↓, Tab.
    Next,
    /// Full-width punctuation (Shift + , . / 1 ;).
    Punctuation(String),
    /// Anything else: commit the composition, then let the app have the key.
    Other,
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
}

fn is_tone(c: char) -> bool {
    crate::bopomofo::is_tone(c)
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// The bopomofo shown inline while composing.
    pub fn composing_text(&self) -> String {
        self.symbols.iter().collect()
    }

    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn handle(&mut self, engine: &mut Engine, key: Key) -> KeyResult {
        match key {
            Key::Symbol(s) => {
                if self.is_empty() {
                    if is_tone(s) {
                        return KeyResult::pass();
                    }
                    // A new composition: pick up what other processes learned
                    // or forgot (one metadata call when nothing changed).
                    engine.refresh_history();
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
                self.symbols.pop();
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
                self.selected = self.selected.saturating_sub(1);
                KeyResult::handled(String::new())
            }
            Key::Next => {
                if self.is_empty() {
                    return KeyResult::pass();
                }
                self.selected = (self.selected + 1).min(self.candidates.len().saturating_sub(1));
                KeyResult::handled(String::new())
            }
            Key::Punctuation(p) => {
                let mut text = self.commit_all(engine);
                text.push_str(&p);
                KeyResult::handled(text)
            }
            Key::Other => KeyResult {
                handled: false,
                commit: self.commit_all(engine),
            },
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
        self.candidates.clear();
        self.selected = 0;
        self.reset_session();
    }

    fn commit_selected(&mut self, engine: &mut Engine) -> String {
        let Some(choice) = self.candidates.get(self.selected).cloned() else {
            // No candidates (should not happen): give back the raw symbols.
            let raw = self.composing_text();
            self.reset();
            return raw;
        };
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
        self.candidates = if self.is_empty() {
            Vec::new()
        } else {
            engine.query(&self.symbols)
        };
        self.selected = 0;
    }
}
