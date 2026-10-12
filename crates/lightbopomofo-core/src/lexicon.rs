//! Lexicon loading and the initials index.
//!
//! Source format (chewing `tsi.csv`): `詞,頻率,ㄍㄨㄥ ㄗㄨㄛˋ` — one reading per
//! character, first tone unmarked. Lines starting with `#` are metadata.

use std::collections::HashSet;
use std::path::Path;

use crate::bopomofo::{is_symbol, is_tone};

pub const MAX_SYLLABLE_LEN: usize = 3;

/// One syllable of a reading: up to three symbols plus an optional tone mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Syllable {
    base: [char; MAX_SYLLABLE_LEN],
    len: u8,
    tone: Option<char>,
}

impl Syllable {
    pub fn parse(s: &str) -> Option<Self> {
        let mut chars: Vec<char> = s.chars().collect();
        let tone = match chars.last() {
            Some(&c) if is_tone(c) => {
                chars.pop();
                Some(c)
            }
            _ => None,
        };
        if chars.is_empty()
            || chars.len() > MAX_SYLLABLE_LEN
            || !chars.iter().all(|&c| is_symbol(c) && !is_tone(c))
        {
            return None;
        }
        let mut base = ['\0'; MAX_SYLLABLE_LEN];
        base[..chars.len()].copy_from_slice(&chars);
        Some(Syllable {
            base,
            len: chars.len() as u8,
            tone,
        })
    }

    pub fn base(&self) -> &[char] {
        &self.base[..self.len as usize]
    }

    /// `None` means first tone.
    pub fn tone(&self) -> Option<char> {
        self.tone
    }
}

pub struct Entry {
    pub text: Box<str>,
    pub syllables: Box<[Syllable]>,
    pub freq: u32,
}

/// Trie keyed by the first symbol of each syllable ("工作" → ㄍ, ㄗ).
struct Node {
    children: Vec<(char, u32)>,
    /// Entries whose initials sequence ends here, sorted by frequency (desc).
    entries: Vec<u32>,
}

pub struct Lexicon {
    entries: Vec<Entry>,
    nodes: Vec<Node>,
    /// Every non-empty prefix of every syllable base in the lexicon.
    prefixes: HashSet<Vec<char>>,
    total_freq: f64,
}

pub const ROOT: u32 = 0;

impl Lexicon {
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    pub fn parse(source: &str) -> Self {
        let mut lex = Lexicon {
            entries: Vec::new(),
            nodes: vec![Node {
                children: Vec::new(),
                entries: Vec::new(),
            }],
            prefixes: HashSet::new(),
            total_freq: 0.0,
        };
        for line in source.lines() {
            if let Some(entry) = parse_line(line) {
                lex.insert(entry);
            }
        }
        let entries = &lex.entries;
        for node in &mut lex.nodes {
            node.entries
                .sort_by(|&a, &b| entries[b as usize].freq.cmp(&entries[a as usize].freq));
        }
        lex.total_freq = lex.entries.iter().map(|e| e.freq as f64 + 1.0).sum();
        lex
    }

    fn insert(&mut self, entry: Entry) {
        let id = self.entries.len() as u32;
        let mut node = ROOT;
        for syl in entry.syllables.iter() {
            let base = syl.base();
            for n in 1..=base.len() {
                if !self.prefixes.contains(&base[..n]) {
                    self.prefixes.insert(base[..n].to_vec());
                }
            }
            node = match self.child(node, base[0]) {
                Some(next) => next,
                None => {
                    let next = self.nodes.len() as u32;
                    self.nodes.push(Node {
                        children: Vec::new(),
                        entries: Vec::new(),
                    });
                    self.nodes[node as usize].children.push((base[0], next));
                    next
                }
            };
        }
        self.nodes[node as usize].entries.push(id);
        self.entries.push(entry);
    }

    pub fn child(&self, node: u32, initial: char) -> Option<u32> {
        self.nodes[node as usize]
            .children
            .iter()
            .find(|&&(c, _)| c == initial)
            .map(|&(_, n)| n)
    }

    pub fn entries_at(&self, node: u32) -> &[u32] {
        &self.nodes[node as usize].entries
    }

    pub fn entry(&self, id: u32) -> &Entry {
        &self.entries[id as usize]
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Is `symbols` (no tone) the start of some syllable?
    pub fn is_syllable_prefix(&self, symbols: &[char]) -> bool {
        self.prefixes.contains(symbols)
    }

    /// Natural log of the entry's unigram probability.
    pub fn log_prob(&self, id: u32) -> f64 {
        ((self.entry(id).freq as f64 + 1.0) / self.total_freq).ln()
    }
}

fn parse_line(line: &str) -> Option<Entry> {
    if line.starts_with('#') {
        return None;
    }
    let mut parts = line.splitn(3, ',');
    let text = parts.next()?.trim();
    let freq: u32 = parts.next()?.trim().parse().ok()?;
    let reading = parts.next()?;
    // Skip entries that are themselves bopomofo symbols (ㄅ, ˇ, …).
    if text.is_empty() || text.chars().any(is_symbol) {
        return None;
    }
    let syllables: Option<Vec<Syllable>> =
        reading.split_whitespace().map(Syllable::parse).collect();
    let syllables = syllables?;
    if syllables.len() != text.chars().count() {
        return None;
    }
    Some(Entry {
        text: text.into(),
        syllables: syllables.into(),
        freq,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = "# dc:title,test,\n工作,34279,ㄍㄨㄥ ㄗㄨㄛˋ\n工,19861,ㄍㄨㄥ\n嗎,26437,ㄇㄚ˙\nㄅ,0,ㄅ\n壞行,1,ㄏㄨㄞˋ\n";

    #[test]
    fn parses_and_indexes() {
        let lex = Lexicon::parse(MINI);
        assert_eq!(lex.len(), 3); // bopomofo entry and malformed line skipped
        let g = lex.child(ROOT, 'ㄍ').unwrap();
        let gz = lex.child(g, 'ㄗ').unwrap();
        assert_eq!(&*lex.entry(lex.entries_at(g)[0]).text, "工");
        assert_eq!(&*lex.entry(lex.entries_at(gz)[0]).text, "工作");
        assert!(lex.child(ROOT, 'ㄗ').is_none());
    }

    #[test]
    fn syllables() {
        let s = Syllable::parse("ㄗㄨㄛˋ").unwrap();
        assert_eq!(s.base(), &['ㄗ', 'ㄨ', 'ㄛ']);
        assert_eq!(s.tone(), Some('ˋ'));
        assert_eq!(Syllable::parse("ㄇㄚ˙").unwrap().tone(), Some('˙'));
        assert_eq!(Syllable::parse("ㄍㄨㄥ").unwrap().tone(), None);
        assert!(Syllable::parse("ˋ").is_none());
        assert!(Syllable::parse("ㄍㄨㄥㄥ").is_none());
    }

    #[test]
    fn prefixes() {
        let lex = Lexicon::parse(MINI);
        assert!(lex.is_syllable_prefix(&['ㄍ']));
        assert!(lex.is_syllable_prefix(&['ㄍ', 'ㄨ']));
        assert!(lex.is_syllable_prefix(&['ㄍ', 'ㄨ', 'ㄥ']));
        assert!(!lex.is_syllable_prefix(&['ㄨ', 'ㄥ']));
        assert!(!lex.is_syllable_prefix(&['ㄍ', 'ㄗ']));
    }
}
