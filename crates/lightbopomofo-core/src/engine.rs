//! Query: bopomofo input → ranked candidates.
//!
//! The input is split into character slots; each slot must be a prefix of a
//! syllable, optionally ending with a tone mark (a tone always ends a slot).
//! Every lexicon word that fits a run of consecutive slots becomes an edge in
//! a lattice over input positions. Viterbi over the lattice gives the
//! whole-input conversion; edges starting at position 0 give the
//! prefix-only candidates.
//!
//! A slot cut short — the next symbol would continue it into a longer
//! syllable, as ㄧ does after ㄋ — is heavily penalized, so a fully typed
//! syllable stays one character (ㄋㄧˇ is 你, not 難以 from ㄋ|ㄧˇ).
//!
//! Candidates come in tiers, like the iPhone keyboard: the whole input
//! (conversion and common words covering it all), common words covering a
//! front part, common single characters for the first syllable, then
//! everything else by score.
//!
//! The bopomofo itself is always a candidate, last on the first page, to
//! type the symbols as they are; it is never learned.
//!
//! History: a word the user chose for exactly the same symbols wins its span
//! (it is lifted just above the span's best word) without making the lattice
//! prefer splitting into learned pieces. A whole input the user committed
//! before comes back first as-is.

use std::collections::HashMap;

use crate::bopomofo::{is_tone, FIRST_TONE};
use crate::history::{History, HistoryEntry};
use crate::lexicon::{Lexicon, MAX_SYLLABLE_LEN, ROOT};

// Ranking weights. Scores are roughly log-probabilities: the word's unigram
// log-probability plus a log-likelihood of how the user typed each slot.

/// Slot is the complete syllable (ㄍㄨㄥ for 工).
pub const SLOT_FULL: f64 = 0.0;
/// Slot is only the first symbol (ㄍ for 工) — the common 簡拼 case.
pub const SLOT_INITIAL: f64 = -0.5;
/// Slot is longer than the first symbol but not complete (ㄍㄨ for 工).
pub const SLOT_PARTIAL: f64 = -2.0;
/// Extra for an incomplete slot that ends in a tone: a tone usually means the
/// syllable is finished, so ㄧˋ should be 一 (ㄧˋ) before 頁 (ㄧㄝˋ).
pub const SLOT_TONED_INCOMPLETE: f64 = -3.0;
/// Slot ends where the next symbol would continue its syllable (ㄋ|ㄧ).
pub const SLOT_CUT: f64 = -6.0;
/// Added per word; negative values favor fewer, longer words.
pub const WORD_PENALTY: f64 = 0.0;
/// A learned word scores this much above the best word for the same symbols.
pub const HISTORY_LIFT: f64 = 0.5;
/// Plus this times ln(times chosen), so the most-chosen learned word wins.
pub const HISTORY_WEIGHT: f64 = 1.0;
/// Edge for an input symbol no word explains; shows the symbol itself.
pub const UNKNOWN_SYMBOL: f64 = -50.0;
/// Upper bound on returned candidates.
pub const MAX_CANDIDATES: usize = 100;
/// Where the bopomofo itself goes: last on the first page of the candidate
/// window, or last when there are fewer candidates.
pub const RAW_POSITION: usize = crate::composer::PAGE_SIZE - 1;
/// Tier limits: (score margin below the tier's best, maximum count).
/// Whole-input words.
pub const TIER_WHOLE: (f64, usize) = (3.0, 10);
/// Words covering a front part of the input.
pub const TIER_PREFIX: (f64, usize) = (3.0, 10);
/// Single characters for the first syllable.
pub const TIER_CHAR: (f64, usize) = (5.0, 20);

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub text: String,
    /// Number of input symbols this candidate covers, from the start.
    pub consumed: usize,
    pub score: f64,
    /// The lexicon words making up `text`, with the symbols typed for each.
    pub words: Vec<Word>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub typed: String,
    pub text: String,
}

#[derive(Clone, Copy)]
struct Edge {
    start: usize,
    end: usize,
    /// `None` is an unknown-symbol edge.
    entry: Option<u32>,
    score: f64,
    /// Some slot is cut short (see [`SLOT_CUT`]).
    cut: bool,
}

pub struct Engine {
    lexicon: Lexicon,
    history: History,
}

impl Engine {
    pub fn new(lexicon: Lexicon, history: History) -> Self {
        Engine { lexicon, history }
    }

    pub fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    pub fn query_str(&self, input: &str) -> Vec<Candidate> {
        self.query(&input.chars().collect::<Vec<_>>())
    }

    pub fn query(&self, input: &[char]) -> Vec<Candidate> {
        let n = input.len();
        if n == 0 {
            return Vec::new();
        }
        let spans = self.slot_spans(input);

        // best[i][j]: highest-scoring edge covering input[i..j], for Viterbi.
        let mut best: Vec<Vec<Option<Edge>>> = vec![vec![None; n + 1]; n];
        let mut from_zero: Vec<(Edge, u32)> = Vec::new();
        let mut learned: Vec<(Edge, u32)> = Vec::new();
        for start in 0..n {
            let mut slots = Vec::new();
            self.walk(
                input,
                &spans,
                start,
                start,
                ROOT,
                &mut slots,
                &mut |edge, used| {
                    let cell = &mut best[edge.start][edge.end];
                    if cell.is_none_or(|c| edge.score > c.score) {
                        *cell = Some(edge);
                    }
                    if edge.start == 0 {
                        from_zero.push((edge, used));
                    }
                    if used > 0 {
                        learned.push((edge, used));
                    }
                },
            );
            if best[start][start + 1].is_none() {
                best[start][start + 1] = Some(Edge {
                    start,
                    end: start + 1,
                    entry: None,
                    score: UNKNOWN_SYMBOL,
                    cut: false,
                });
            }
        }

        // Lift learned words above the best raw score of their span.
        let lift = |edge: &Edge, used: u32, best: &[Vec<Option<Edge>>]| {
            let top = best[edge.start][edge.end].map_or(edge.score, |c| c.score);
            top.max(edge.score) + HISTORY_LIFT + HISTORY_WEIGHT * (used as f64).ln()
        };
        let raw_best = best.clone();
        for (edge, used) in &learned {
            let lifted = Edge {
                score: lift(edge, *used, &raw_best),
                ..*edge
            };
            let cell = &mut best[edge.start][edge.end];
            if cell.is_none_or(|c| lifted.score > c.score) {
                *cell = Some(lifted);
            }
        }
        for (edge, used) in &mut from_zero {
            if *used > 0 {
                edge.score = lift(edge, *used, &raw_best);
            }
        }

        let mut candidates = Vec::new();
        let full = self.viterbi(input, &best);
        let full_text = full.text.clone();
        candidates.push(full);

        // Prefix candidates, one per text (the one covering the most input),
        // with whether they cut a syllable short.
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut prefix: Vec<(Candidate, bool)> = Vec::new();
        for (edge, _) in from_zero {
            let entry = self
                .lexicon
                .entry(edge.entry.expect("walk only yields words"));
            let text = entry.text().to_string();
            if text == full_text {
                continue;
            }
            let candidate = Candidate {
                text: text.clone(),
                consumed: edge.end,
                score: edge.score,
                words: vec![Word {
                    typed: input[..edge.end].iter().collect(),
                    text: text.clone(),
                }],
            };
            match seen.get(&text) {
                Some(&i) => {
                    let old = &prefix[i].0;
                    if (edge.end, edge.score) > (old.consumed, old.score) {
                        prefix[i] = (candidate, edge.cut);
                    }
                }
                None => {
                    seen.insert(text, prefix.len());
                    prefix.push((candidate, edge.cut));
                }
            }
        }
        prefix.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
        // Tiers: each takes the candidates that fit and cut no syllable,
        // within a margin of the best of them (or of `bar`, when higher), up
        // to a count. The rest follow by score, those cutting a syllable last.
        let mut taken = vec![false; prefix.len()];
        let mut tier =
            |fits: &dyn Fn(&Candidate) -> bool, bar: f64, (margin, max): (f64, usize)| {
                let mut picked = Vec::new();
                let fits = |(c, cut): &(Candidate, bool)| !cut && fits(c);
                let Some(best) = prefix.iter().find(|c| fits(c)).map(|c| c.0.score.max(bar)) else {
                    return picked;
                };
                for (i, c) in prefix.iter().enumerate() {
                    if picked.len() == max || c.0.score < best - margin {
                        break;
                    }
                    if !taken[i] && fits(c) {
                        taken[i] = true;
                        picked.push(c.0.clone());
                    }
                }
                picked
            };
        let single = |c: &Candidate| c.text.chars().count() == 1;
        let bar = candidates[0].score;
        let whole = tier(&|c| c.consumed == n, bar, TIER_WHOLE);
        let front = tier(&|c| c.consumed < n && !single(c), f64::MIN, TIER_PREFIX);
        let chars = tier(&single, f64::MIN, TIER_CHAR);
        let mut rest: Vec<(Candidate, bool)> = prefix
            .into_iter()
            .zip(taken)
            .filter_map(|(c, t)| (!t).then_some(c))
            .collect();
        rest.sort_by_key(|&(_, cut)| cut);
        let rest = rest.into_iter().map(|(c, _)| c);
        candidates.extend(whole.into_iter().chain(front).chain(chars).chain(rest));

        // A whole input committed before comes back first.
        let whole: String = input.iter().collect();
        if let Some(text) = self
            .history
            .learned(&whole)
            .iter()
            .max_by_key(|l| (l.count, l.last_used))
            .map(|l| &l.text)
        {
            candidates.retain(|c| c.text != *text);
            candidates.insert(
                0,
                Candidate {
                    text: text.clone(),
                    consumed: n,
                    score: f64::INFINITY,
                    words: vec![Word {
                        typed: whole.clone(),
                        text: text.clone(),
                    }],
                },
            );
        }
        // The symbols as typed, last on the first page (Shift+9).
        let raw: String = input.iter().collect();
        candidates.retain(|c| c.text != raw);
        candidates.truncate(MAX_CANDIDATES - 1);
        let at = candidates.len().min(RAW_POSITION);
        candidates.insert(
            at,
            Candidate {
                text: raw,
                consumed: n,
                score: f64::NEG_INFINITY,
                words: Vec::new(),
            },
        );
        candidates
    }

    /// Record that the user committed `text` for the start of `input`: each
    /// word it is made of, and the whole text when it spans several words.
    /// `text` need not be a current candidate (e.g. a composition the user
    /// built from several partial selections); then it is recorded for all
    /// of `input`.
    pub fn learn(&mut self, input: &[char], text: &str) {
        let (typed, words) = match self.query(input).into_iter().find(|c| c.text == text) {
            Some(c) => (input[..c.consumed].iter().collect::<String>(), c.words),
            None => (input.iter().collect(), Vec::new()),
        };
        let mut choices: Vec<(&str, &str)> = words
            .iter()
            .map(|w| (w.typed.as_str(), w.text.as_str()))
            .collect();
        if words.len() != 1 {
            choices.push((&typed, text));
        }
        choices.retain(|(_, t)| !t.chars().any(crate::bopomofo::is_symbol));
        self.history.record(&choices);
    }

    pub fn learn_str(&mut self, input: &str, text: &str) {
        self.learn(&input.chars().collect::<Vec<_>>(), text);
    }

    /// Everything the engine has learned, most recent first.
    pub fn history_entries(&self) -> Vec<HistoryEntry> {
        self.history.entries()
    }

    /// Forget one learned choice; returns whether it existed.
    pub fn forget(&mut self, typed: &str, text: &str) -> std::io::Result<bool> {
        self.history.forget(typed, text)
    }

    pub fn clear_history(&mut self) -> std::io::Result<()> {
        self.history.clear()
    }

    /// Re-read the history file after another process changed it.
    pub fn reload_history(&mut self) {
        self.history.reload();
    }

    /// Re-read the history file only if it changed (cheap otherwise). Call
    /// before a new composition when other processes share the file.
    pub fn refresh_history(&mut self) -> bool {
        self.history.reload_if_changed()
    }

    /// For each position, the end positions of valid slots starting there.
    fn slot_spans(&self, input: &[char]) -> Vec<Vec<usize>> {
        (0..input.len())
            .map(|start| {
                let mut ends = Vec::new();
                for len in 1..=MAX_SYLLABLE_LEN {
                    let Some(prefix) = input.get(start..start + len) else {
                        break;
                    };
                    if prefix.iter().any(|&c| is_tone(c))
                        || !self.lexicon.is_syllable_prefix(prefix)
                    {
                        break;
                    }
                    // A following tone belongs to this slot and ends it.
                    if input.get(start + len).is_some_and(|&c| is_tone(c)) {
                        ends.push(start + len + 1);
                        break;
                    }
                    ends.push(start + len);
                }
                ends
            })
            .collect()
    }

    /// Depth-first over slot sequences starting at `start`, following the
    /// initials trie in lockstep, emitting every word that fits.
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        input: &[char],
        spans: &[Vec<usize>],
        start: usize,
        pos: usize,
        node: u32,
        slots: &mut Vec<(usize, usize)>,
        emit: &mut dyn FnMut(Edge, u32),
    ) {
        if !slots.is_empty() {
            let learned = if self.history.is_empty() {
                &[]
            } else {
                self.history
                    .learned(&input[start..pos].iter().collect::<String>())
            };
            let cuts = slots
                .iter()
                .filter(|&&(a, b)| spans[a].last().is_some_and(|&end| end > b))
                .count();
            for id in self.lexicon.entries_at(node) {
                if let Some(score) = self.score_entry(input, slots, id) {
                    // Only look at the text when this span has learned choices.
                    let used = if learned.is_empty() {
                        0
                    } else {
                        let text = self.lexicon.entry(id).text();
                        learned
                            .iter()
                            .find(|l| l.text == text)
                            .map_or(0, |l| l.count)
                    };
                    let edge = Edge {
                        start,
                        end: pos,
                        entry: Some(id),
                        score: score + SLOT_CUT * cuts as f64,
                        cut: cuts > 0,
                    };
                    emit(edge, used);
                }
            }
        }
        let Some(&first) = input.get(pos) else { return };
        let Some(child) = self.lexicon.child(node, first) else {
            return;
        };
        for &end in &spans[pos] {
            slots.push((pos, end));
            self.walk(input, spans, start, end, child, slots, emit);
            slots.pop();
        }
    }

    /// Score of word `id` typed as `slots`, or `None` if it doesn't fit.
    fn score_entry(&self, input: &[char], slots: &[(usize, usize)], id: u32) -> Option<f64> {
        // Hot path: most entries fail on the first symbol, so compare the
        // stored code points directly and compute the log only on a match.
        let entry = self.lexicon.entry(id);
        let mut score = 0.0;
        for (k, &(a, b)) in slots.iter().enumerate() {
            let syl = entry.raw_syllable(k);
            let mut typed = &input[a..b];
            let mut toned = false;
            if let Some((&last, rest)) = typed.split_last() {
                if is_tone(last) {
                    toned = true;
                    // The lexicon stores first tone as 0.
                    let want = if last == FIRST_TONE { 0 } else { last as u32 };
                    if syl.tone as u32 != want {
                        return None;
                    }
                    typed = rest;
                }
            }
            let base_len = syl.len();
            if typed.len() > base_len
                || typed
                    .iter()
                    .zip(&syl.base)
                    .any(|(&c, &s)| c as u32 != s as u32)
            {
                return None;
            }
            score += if typed.len() == base_len {
                SLOT_FULL
            } else if typed.len() == 1 {
                SLOT_INITIAL
            } else {
                SLOT_PARTIAL
            };
            if toned && typed.len() < base_len {
                score += SLOT_TONED_INCOMPLETE;
            }
        }
        Some(score + self.lexicon.log_prob_of(entry.freq()) + WORD_PENALTY)
    }

    fn viterbi(&self, input: &[char], best: &[Vec<Option<Edge>>]) -> Candidate {
        let n = input.len();
        let mut score = vec![f64::NEG_INFINITY; n + 1];
        let mut back: Vec<Option<Edge>> = vec![None; n + 1];
        score[0] = 0.0;
        for end in 1..=n {
            for start in 0..end {
                if let Some(edge) = best[start][end] {
                    let s = score[start] + edge.score;
                    if s > score[end] {
                        score[end] = s;
                        back[end] = Some(edge);
                    }
                }
            }
        }
        let mut path = Vec::new();
        let mut pos = n;
        while pos > 0 {
            let edge = back[pos].expect("unknown-symbol edges keep the lattice connected");
            path.push(edge);
            pos = edge.start;
        }
        path.reverse();
        let mut text = String::new();
        let mut words = Vec::new();
        for edge in &path {
            match edge.entry {
                Some(id) => {
                    let t = self.lexicon.entry(id).text();
                    text.push_str(t);
                    words.push(Word {
                        typed: input[edge.start..edge.end].iter().collect(),
                        text: t.to_string(),
                    });
                }
                None => text.extend(&input[edge.start..edge.end]),
            }
        }
        Candidate {
            text,
            consumed: n,
            score: score[n],
            words,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = "\
古,5991,ㄍㄨˇ
姑,2000,ㄍㄨ
公,8000,ㄍㄨㄥ
公務,1018,ㄍㄨㄥ ㄨˋ
感悟,10,ㄍㄢˇ ㄨˋ
務,3000,ㄨˋ
屋,2500,ㄨ
工作,34279,ㄍㄨㄥ ㄗㄨㄛˋ
方法,20000,ㄈㄤ ㄈㄚˇ
工,19861,ㄍㄨㄥ
作,9000,ㄗㄨㄛˋ
";

    fn engine() -> Engine {
        Engine::new(Lexicon::parse(MINI), History::in_memory())
    }

    fn texts(c: &[Candidate]) -> Vec<&str> {
        c.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn gu_is_one_or_two_characters() {
        let c = engine().query_str("ㄍㄨ");
        let t = texts(&c);
        assert!(t.contains(&"古"), "{t:?}");
        assert!(t.contains(&"公務"), "{t:?}");
        assert!(t.contains(&"感悟"), "{t:?}");
        assert!(c.iter().all(|c| c.consumed == 2 || c.consumed == 1));
    }

    #[test]
    fn tone_ends_slot_and_filters() {
        // ㄍㄨˇ: only 古 has third tone; 公務/姑 must not appear as full matches.
        let c = engine().query_str("ㄍㄨˇ");
        assert_eq!(c[0].text, "古");
        // No candidate may end right before the tone (ㄍㄨ|ˇ).
        assert!(c.iter().all(|c| c.consumed != 2), "{:?}", texts(&c));
        assert!(!texts(&c).contains(&"公務"));
    }

    #[test]
    fn tone_after_initial_filters_by_tone() {
        // ㄍˇ ㄨ: first char starts with ㄍ in third tone → 感悟, not 公務.
        let c = engine().query_str("ㄍˇㄨ");
        let t = texts(&c);
        assert!(t.contains(&"感悟"), "{t:?}");
        assert!(!t.contains(&"公務"), "{t:?}");
        assert!(!t.contains(&"工"), "{t:?}");
    }

    #[test]
    fn first_tone_mark_matches_unmarked_syllables() {
        // ㄍㄨˉ: 姑 is first tone; 古 (ˇ) must not appear.
        let c = engine().query_str("ㄍㄨˉ");
        assert_eq!(c[0].text, "姑");
        // Only as a prefix candidate for ㄍ alone.
        assert!(c.iter().all(|c| c.text != "古" || c.consumed == 1));
        // ㄍˉㄨ: first char in first tone → 公務 (ㄍㄨㄥ), not 感悟 (ㄍㄢˇ).
        let t = engine().query_str("ㄍˉㄨ");
        let t = texts(&t);
        assert!(t.contains(&"公務"), "{t:?}");
        assert!(!t.contains(&"感悟"), "{t:?}");
    }

    #[test]
    fn whole_input_and_prefix_candidates() {
        let c = engine().query_str("ㄍㄗㄈㄈ");
        assert_eq!(c[0].text, "工作方法");
        assert_eq!(c[0].consumed, 4);
        let words: Vec<(&str, &str)> = c[0]
            .words
            .iter()
            .map(|w| (w.typed.as_str(), w.text.as_str()))
            .collect();
        assert_eq!(words, vec![("ㄍㄗ", "工作"), ("ㄈㄈ", "方法")]);
        let gz = c.iter().find(|c| c.text == "工作").unwrap();
        assert_eq!(gz.consumed, 2);
        // Longer prefix words come before single characters.
        let pos = |s: &str| c.iter().position(|c| c.text == s).unwrap();
        assert!(pos("工作") < pos("工"));
    }

    #[test]
    fn unknown_input_does_not_fail() {
        let e = engine();
        for input in ["ˇ", "ㄦㄦ", "abc", "ㄍxㄗ", "ˇˇˇㄍ"] {
            let c = e.query_str(input);
            assert_eq!(c[0].consumed, input.chars().count(), "{input}");
        }
        assert!(e.query_str("").is_empty());
    }

    #[test]
    fn history_promotes_chosen_word() {
        let mut e = engine();
        let before = e.query_str("ㄍㄨ");
        let rank = |c: &[Candidate]| c.iter().position(|c| c.text == "感悟").unwrap();
        let r0 = rank(&before);
        for _ in 0..3 {
            e.learn_str("ㄍㄨ", "感悟");
        }
        let after = e.query_str("ㄍㄨ");
        assert!(rank(&after) < r0, "{:?}", texts(&after));
        assert_eq!(after[0].text, "感悟");
        // Learned for ㄍㄨ only; other input is unaffected.
        assert_ne!(e.query_str("ㄍㄨㄥㄨ")[0].text, "感悟");
    }

    #[test]
    fn candidates_are_unique_by_text() {
        let c = engine().query_str("ㄍㄨ");
        let mut t = texts(&c);
        let len = t.len();
        t.sort();
        t.dedup();
        assert_eq!(t.len(), len);
    }

    #[test]
    fn learning_whole_conversion_records_each_word() {
        let mut e = engine();
        e.learn_str("ㄍㄗㄈㄈ", "工作方法");
        assert_eq!(e.history.count("ㄍㄗ", "工作"), 1);
        assert_eq!(e.history.count("ㄈㄈ", "方法"), 1);
        assert_eq!(e.history.count("ㄍㄗㄈㄈ", "工作方法"), 1);
    }

    #[test]
    fn learned_composition_comes_back_first() {
        let mut e = engine();
        assert_ne!(e.query_str("ㄍㄨㄈㄈ")[0].text, "感悟方法");
        // Built from two partial selections, then learned as a whole.
        e.learn_str("ㄍㄨㄈㄈ", "感悟");
        e.learn_str("ㄈㄈ", "方法");
        e.learn_str("ㄍㄨㄈㄈ", "感悟方法");
        let c = e.query_str("ㄍㄨㄈㄈ");
        assert_eq!(c[0].text, "感悟方法");
        assert_eq!(c[0].consumed, 4);
        assert_eq!(c.iter().filter(|c| c.text == "感悟方法").count(), 1);
    }

    #[test]
    fn forgetting_undoes_learning() {
        let mut e = engine();
        e.learn_str("ㄍㄨ", "感悟");
        assert_eq!(e.query_str("ㄍㄨ")[0].text, "感悟");
        assert!(e
            .history_entries()
            .iter()
            .any(|h| h.typed == "ㄍㄨ" && h.text == "感悟"));
        assert!(e.forget("ㄍㄨ", "感悟").unwrap());
        assert_ne!(e.query_str("ㄍㄨ")[0].text, "感悟");
        e.learn_str("ㄍㄨ", "感悟");
        e.clear_history().unwrap();
        assert!(e.history_entries().is_empty());
    }

    #[test]
    fn learned_single_character_does_not_hijack_other_input() {
        let mut e = engine();
        for _ in 0..5 {
            e.learn_str("ㄍ", "古");
        }
        assert_eq!(e.query_str("ㄍ")[0].text, "古");
        assert_eq!(e.query_str("ㄍㄗㄈㄈ")[0].text, "工作方法");
    }
}
