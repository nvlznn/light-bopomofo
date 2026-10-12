//! Lexicon loading and the initials index.
//!
//! Source format (chewing `tsi.csv`): `詞,頻率,ㄍㄨㄥ ㄗㄨㄛˋ` — one reading per
//! character, first tone unmarked. Lines starting with `#` are metadata.

use std::collections::{HashMap, HashSet};
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
        rank_homophones(&mut lex.entries);
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

/// Two single-character frequencies within this ratio are treated as tied.
const TIE_RATIO: f64 = 0.99;

/// chewing flattens single-character frequencies into tiers: 的/得/地 are
/// 147270/147269/147269 and 事 ties with 氏 and 嗜. Within each tier of
/// homophones (same reading), reorder by how often the character occurs in
/// multi-character words, reusing the tier's frequency values so overall
/// scores keep their scale. Distinct tiers (嗎 26437 vs 媽 1 for ㄇㄚ˙) stay.
fn rank_homophones(entries: &mut [Entry]) {
    let mut in_words: HashMap<(char, Syllable), u64> = HashMap::new();
    let mut groups: HashMap<Syllable, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.syllables.len() == 1 {
            groups.entry(e.syllables[0]).or_default().push(i);
        } else {
            for (c, syl) in e.text.chars().zip(e.syllables.iter()) {
                *in_words.entry((c, *syl)).or_insert(0) += e.freq as u64;
            }
        }
    }
    let usage = |e: &Entry| {
        let c = e.text.chars().next().unwrap_or_default();
        in_words.get(&(c, e.syllables[0])).copied().unwrap_or(0)
    };
    for mut group in groups.into_values() {
        group.sort_by(|&a, &b| entries[b].freq.cmp(&entries[a].freq));
        let mut start = 0;
        while start < group.len() {
            let top = entries[group[start]].freq as f64;
            let mut end = start + 1;
            while end < group.len() && entries[group[end]].freq as f64 >= top * TIE_RATIO {
                end += 1;
            }
            let tier = &mut group[start..end];
            let values: Vec<u32> = tier.iter().map(|&i| entries[i].freq).collect();
            tier.sort_by_key(|&i| std::cmp::Reverse(usage(&entries[i])));
            // Keep the new order strict even where values were equal.
            let mut prev = u32::MAX;
            for (&i, v) in tier.iter().zip(values) {
                let v = v.min(prev.saturating_sub(1));
                entries[i].freq = v;
                prev = v;
            }
            start = end;
        }
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
    fn homophones_ranked_by_use_in_words() {
        // 氏 and 事 tie; 事 appears in more words. 是 is a higher tier and stays.
        let lex = Lexicon::parse(
            "是,182405,ㄕˋ\n氏,52404,ㄕˋ\n事,52404,ㄕˋ\n故事,5000,ㄍㄨˋ ㄕˋ\n姓氏,100,ㄒㄧㄥˋ ㄕˋ\n",
        );
        let sh = lex.child(ROOT, 'ㄕ').unwrap();
        let order: Vec<&str> = lex
            .entries_at(sh)
            .iter()
            .map(|&i| &*lex.entry(i).text)
            .collect();
        assert_eq!(order, ["是", "事", "氏"]);
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
