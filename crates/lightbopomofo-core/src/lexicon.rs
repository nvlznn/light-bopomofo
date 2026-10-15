//! Lexicon: the chewing word list, compiled into a flat binary image that is
//! memory-mapped at run time.
//!
//! Source format (chewing `tsi.csv`): `詞,頻率,ㄍㄨㄥ ㄗㄨㄛˋ` — one reading per
//! character, first tone unmarked. Lines starting with `#` are metadata.
//!
//! The binary image (`lexicon.bin`, see [`compile`]) holds everything the
//! engine needs already built — entries, readings, the initials trie and the
//! syllable-prefix set — so opening it costs no parsing, and every process
//! that maps the same file shares its memory. That matters on Windows, where
//! the input method is loaded into every application.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use crate::bopomofo::{is_symbol, is_tone};

pub const MAX_SYLLABLE_LEN: usize = 3;
pub const ROOT: u32 = 0;

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

// ---------------------------------------------------------------------------
// Binary image layout (little endian). Every section starts at a multiple
// of 8; all sizes are counts of fixed-size records.
//
//   header    MAGIC, then u32 counts (entries, syllables, nodes, children,
//             node_entries, prefixes, text_bytes), then f64 total frequency
//   entries   [text_off u32, syl_start u32, freq u32, text_len u16, n_syl u16]
//   syllables [base0 u16, base1 u16, base2 u16, tone u16]   (0 = none)
//   nodes     [child_start u32, child_count u32, entry_start u32, entry_count u32]
//   children  [symbol u32, node u32]          sorted by symbol within a node
//   node_entries [entry u32]                  most frequent first
//   prefixes  [key u64]                       sorted; see `prefix_key`
//   text      UTF-8
// ---------------------------------------------------------------------------

const MAGIC: &[u8; 8] = b"LBLEX001";
const HEADER: usize = 48;
const ENTRY: usize = 16;
const SYLLABLE: usize = 8;
const NODE: usize = 16;
const CHILD: usize = 8;

fn align8(n: usize) -> usize {
    (n + 7) & !7
}

/// Pack a syllable base (≤ 3 symbols, all in the BMP) for the prefix table.
fn prefix_key(symbols: &[char]) -> Option<u64> {
    if symbols.is_empty() || symbols.len() > MAX_SYLLABLE_LEN {
        return None;
    }
    let mut key = 0u64;
    for i in 0..MAX_SYLLABLE_LEN {
        let c = symbols.get(i).map_or(0, |&c| c as u32);
        if c > 0xFFFF {
            return None;
        }
        key = key << 16 | c as u64;
    }
    Some(key)
}

enum Bytes {
    Owned(Vec<u8>),
    Mapped(memmap2::Mmap),
}

impl std::ops::Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Owned(v) => v,
            Bytes::Mapped(m) => m,
        }
    }
}

#[derive(Clone, Copy)]
struct Sections {
    entries: usize,
    syllables: usize,
    nodes: usize,
    children: usize,
    node_entries: usize,
    prefixes: usize,
    text: usize,
    n_entries: usize,
    n_syllables: usize,
    n_nodes: usize,
    n_children: usize,
    n_node_entries: usize,
    n_prefixes: usize,
    n_text: usize,
}

pub struct Lexicon {
    bytes: Bytes,
    s: Sections,
    total_freq: f64,
}

/// A word in the lexicon.
#[derive(Clone, Copy)]
pub struct EntryRef<'a> {
    lex: &'a Lexicon,
    /// Where the text is; decoded only when asked for (scoring never needs it).
    text: (usize, usize),
    syl_start: usize,
    n_syl: usize,
    freq: u32,
}

impl<'a> EntryRef<'a> {
    pub fn text(&self) -> &'a str {
        let (start, end) = self.text;
        let bytes = &self.lex.bytes[start..end];
        // SAFETY: every entry's text was checked to be UTF-8 when the image
        // was opened (`validate`), and the image is never modified after.
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    pub fn freq(&self) -> u32 {
        self.freq
    }

    pub fn syllable_count(&self) -> usize {
        self.n_syl
    }

    pub fn syllables(&self) -> impl ExactSizeIterator<Item = Syllable> + 'a {
        let lex = self.lex;
        (self.syl_start..self.syl_start + self.n_syl).map(move |i| lex.syllable(i))
    }

    /// The `k`-th syllable as stored, for matching without decoding.
    /// `k` must be below [`syllable_count`](Self::syllable_count).
    pub fn raw_syllable(&self, k: usize) -> RawSyllable {
        let o = self.lex.s.syllables + (self.syl_start + k) * SYLLABLE;
        let b = &*self.lex.bytes;
        RawSyllable {
            base: [u16_at(b, o), u16_at(b, o + 2), u16_at(b, o + 4)],
            tone: u16_at(b, o + 6),
        }
    }
}

/// A syllable as stored: symbol code points (0 past the end) and the tone
/// mark's code point (0 for first tone). Every symbol is in the BMP.
#[derive(Clone, Copy, Debug)]
pub struct RawSyllable {
    pub base: [u16; MAX_SYLLABLE_LEN],
    pub tone: u16,
}

impl RawSyllable {
    pub fn len(&self) -> usize {
        self.base.iter().take_while(|&&c| c != 0).count()
    }

    pub fn is_empty(&self) -> bool {
        self.base[0] == 0
    }
}

fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("lexicon: {what}"))
}

impl Lexicon {
    /// Open a lexicon: a compiled image (memory-mapped) or a `tsi.csv`.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        use std::io::Read;
        let mut file = std::fs::File::open(path.as_ref())?;
        let mut magic = [0u8; 8];
        let is_image = file.read_exact(&mut magic).is_ok() && &magic == MAGIC;
        if is_image {
            // SAFETY: only read; an installed, read-only data file that is not
            // modified while mapped. (Text sources are read instead: they are
            // for development and may be rewritten.)
            let map = unsafe { memmap2::Mmap::map(&file)? };
            Self::from_bytes(Bytes::Mapped(map))
        } else {
            Ok(Self::parse(&std::fs::read_to_string(path)?))
        }
    }

    /// Build from `tsi.csv` text (compiles in memory).
    pub fn parse(source: &str) -> Self {
        Self::from_bytes(Bytes::Owned(compile(source))).expect("a freshly compiled image is valid")
    }

    /// Open a compiled image held in memory.
    pub fn from_image(image: Vec<u8>) -> io::Result<Self> {
        Self::from_bytes(Bytes::Owned(image))
    }

    fn from_bytes(bytes: Bytes) -> io::Result<Self> {
        let b: &[u8] = &bytes;
        if b.len() < HEADER || !b.starts_with(MAGIC) {
            return Err(invalid("bad header"));
        }
        let count = |i: usize| u32_at(b, 8 + 4 * i) as usize;
        let (n_entries, n_syllables, n_nodes, n_children, n_node_entries, n_prefixes, n_text) = (
            count(0),
            count(1),
            count(2),
            count(3),
            count(4),
            count(5),
            count(6),
        );
        let total_freq = f64::from_le_bytes(b[36..44].try_into().unwrap());
        let mut at = HEADER;
        let mut section = |records: usize, size: usize| {
            let start = at;
            at = align8(at + records * size);
            start
        };
        let s = Sections {
            entries: section(n_entries, ENTRY),
            syllables: section(n_syllables, SYLLABLE),
            nodes: section(n_nodes, NODE),
            children: section(n_children, CHILD),
            node_entries: section(n_node_entries, 4),
            prefixes: section(n_prefixes, 8),
            text: section(n_text, 1),
            n_entries,
            n_syllables,
            n_nodes,
            n_children,
            n_node_entries,
            n_prefixes,
            n_text,
        };
        if at > b.len() || n_nodes == 0 || !(total_freq.is_finite() && total_freq >= 0.0) {
            return Err(invalid("truncated or empty"));
        }
        let lex = Lexicon {
            bytes,
            s,
            total_freq,
        };
        lex.validate()?;
        Ok(lex)
    }

    /// Check every reference once, so lookups can never go out of bounds.
    fn validate(&self) -> io::Result<()> {
        let (b, s) = (&*self.bytes, self.s);
        for i in 0..s.n_entries {
            let o = s.entries + i * ENTRY;
            let (text_off, syl_start) = (u32_at(b, o) as usize, u32_at(b, o + 4) as usize);
            let (text_len, n_syl) = (u16_at(b, o + 12) as usize, u16_at(b, o + 14) as usize);
            if text_off + text_len > s.n_text || syl_start + n_syl > s.n_syllables || n_syl == 0 {
                return Err(invalid("entry out of range"));
            }
            let text = &b[s.text + text_off..s.text + text_off + text_len];
            if std::str::from_utf8(text).is_err() {
                return Err(invalid("entry text not UTF-8"));
            }
        }
        for i in 0..s.n_syllables {
            let o = s.syllables + i * SYLLABLE;
            let c0 = u16_at(b, o) as u32;
            let tone = u16_at(b, o + 6) as u32;
            if char::from_u32(c0).is_none_or(|c| c == '\0') || char::from_u32(tone).is_none() {
                return Err(invalid("bad syllable"));
            }
        }
        for i in 0..s.n_nodes {
            let o = s.nodes + i * NODE;
            let (cs, cc) = (u32_at(b, o) as usize, u32_at(b, o + 4) as usize);
            let (es, ec) = (u32_at(b, o + 8) as usize, u32_at(b, o + 12) as usize);
            if cs + cc > s.n_children || es + ec > s.n_node_entries {
                return Err(invalid("node out of range"));
            }
        }
        for i in 0..s.n_children {
            let o = s.children + i * CHILD;
            if char::from_u32(u32_at(b, o)).is_none() || u32_at(b, o + 4) as usize >= s.n_nodes {
                return Err(invalid("child out of range"));
            }
        }
        for i in 0..s.n_node_entries {
            if u32_at(b, s.node_entries + i * 4) as usize >= s.n_entries {
                return Err(invalid("node entry out of range"));
            }
        }
        Ok(())
    }

    fn syllable(&self, i: usize) -> Syllable {
        let o = self.s.syllables + i * SYLLABLE;
        let b = &*self.bytes;
        let mut base = ['\0'; MAX_SYLLABLE_LEN];
        let mut len = 0;
        for (k, slot) in base.iter_mut().enumerate() {
            let c = u16_at(b, o + 2 * k) as u32;
            if c == 0 {
                break;
            }
            *slot = char::from_u32(c).unwrap_or('\0');
            len += 1;
        }
        let tone = u16_at(b, o + 6) as u32;
        Syllable {
            base,
            len,
            tone: (tone != 0).then(|| char::from_u32(tone).unwrap_or('\0')),
        }
    }

    pub fn child(&self, node: u32, initial: char) -> Option<u32> {
        let (b, s) = (&*self.bytes, self.s);
        let o = s.nodes + node as usize * NODE;
        let (start, count) = (u32_at(b, o) as usize, u32_at(b, o + 4) as usize);
        let (mut lo, mut hi) = (start, start + count);
        let want = initial as u32;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let c = u32_at(b, s.children + mid * CHILD);
            match c.cmp(&want) {
                std::cmp::Ordering::Equal => return Some(u32_at(b, s.children + mid * CHILD + 4)),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
    }

    /// Entries whose initials sequence ends at `node`, most frequent first.
    pub fn entries_at(&self, node: u32) -> impl Iterator<Item = u32> + '_ {
        let (b, s) = (&*self.bytes, self.s);
        let o = s.nodes + node as usize * NODE;
        let (start, count) = (u32_at(b, o + 8) as usize, u32_at(b, o + 12) as usize);
        (start..start + count).map(move |i| u32_at(b, s.node_entries + i * 4))
    }

    pub fn entry(&self, id: u32) -> EntryRef<'_> {
        let (b, s) = (&*self.bytes, self.s);
        let o = s.entries + id as usize * ENTRY;
        let text_off = s.text + u32_at(b, o) as usize;
        let text_len = u16_at(b, o + 12) as usize;
        EntryRef {
            lex: self,
            text: (text_off, text_off + text_len),
            syl_start: u32_at(b, o + 4) as usize,
            n_syl: u16_at(b, o + 14) as usize,
            freq: u32_at(b, o + 8),
        }
    }

    pub fn len(&self) -> usize {
        self.s.n_entries
    }

    pub fn is_empty(&self) -> bool {
        self.s.n_entries == 0
    }

    /// Is `symbols` (no tone) the start of some syllable?
    pub fn is_syllable_prefix(&self, symbols: &[char]) -> bool {
        let Some(key) = prefix_key(symbols) else {
            return false;
        };
        let (b, s) = (&*self.bytes, self.s);
        let (mut lo, mut hi) = (0, s.n_prefixes);
        while lo < hi {
            let mid = (lo + hi) / 2;
            match u64_at(b, s.prefixes + mid * 8).cmp(&key) {
                std::cmp::Ordering::Equal => return true,
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        false
    }

    /// Natural log of the entry's unigram probability.
    pub fn log_prob(&self, id: u32) -> f64 {
        self.log_prob_of(self.entry(id).freq())
    }

    /// Natural log of the unigram probability of a word with frequency `freq`.
    pub fn log_prob_of(&self, freq: u32) -> f64 {
        ((freq as f64 + 1.0) / self.total_freq).ln()
    }
}

// ---------------------------------------------------------------------------
// Compiling tsi.csv into an image.
// ---------------------------------------------------------------------------

struct Entry {
    text: Box<str>,
    syllables: Box<[Syllable]>,
    freq: u32,
}

struct Node {
    children: Vec<(char, u32)>,
    entries: Vec<u32>,
}

/// Compile `tsi.csv` text into a binary lexicon image.
pub fn compile(source: &str) -> Vec<u8> {
    let mut entries: Vec<Entry> = source.lines().filter_map(parse_line).collect();
    rank_homophones(&mut entries);

    let mut nodes = vec![Node {
        children: Vec::new(),
        entries: Vec::new(),
    }];
    let mut prefixes: HashSet<u64> = HashSet::new();
    for (id, entry) in entries.iter().enumerate() {
        let mut node = 0usize;
        for syl in entry.syllables.iter() {
            let base = syl.base();
            for n in 1..=base.len() {
                prefixes.extend(prefix_key(&base[..n]));
            }
            node = match nodes[node].children.iter().find(|&&(c, _)| c == base[0]) {
                Some(&(_, next)) => next as usize,
                None => {
                    let next = nodes.len();
                    nodes.push(Node {
                        children: Vec::new(),
                        entries: Vec::new(),
                    });
                    nodes[node].children.push((base[0], next as u32));
                    next
                }
            };
        }
        nodes[node].entries.push(id as u32);
    }
    for node in &mut nodes {
        node.children.sort_by_key(|&(c, _)| c);
        // Stable: equal frequencies keep their source order.
        node.entries
            .sort_by(|&a, &b| entries[b as usize].freq.cmp(&entries[a as usize].freq));
    }
    let mut prefixes: Vec<u64> = prefixes.into_iter().collect();
    prefixes.sort_unstable();
    let total_freq: f64 = entries.iter().map(|e| e.freq as f64 + 1.0).sum();

    // Serialize.
    let mut text = Vec::new();
    let mut entry_bytes = Vec::with_capacity(entries.len() * ENTRY);
    let mut syl_bytes = Vec::new();
    let mut n_syllables = 0u32;
    for e in &entries {
        entry_bytes.extend((text.len() as u32).to_le_bytes());
        entry_bytes.extend(n_syllables.to_le_bytes());
        entry_bytes.extend(e.freq.to_le_bytes());
        entry_bytes.extend((e.text.len() as u16).to_le_bytes());
        entry_bytes.extend((e.syllables.len() as u16).to_le_bytes());
        text.extend(e.text.as_bytes());
        for syl in e.syllables.iter() {
            for k in 0..MAX_SYLLABLE_LEN {
                let c = syl.base().get(k).map_or(0, |&c| c as u32) as u16;
                syl_bytes.extend(c.to_le_bytes());
            }
            syl_bytes.extend((syl.tone().map_or(0, |c| c as u32) as u16).to_le_bytes());
            n_syllables += 1;
        }
    }
    let mut node_bytes = Vec::new();
    let mut child_bytes = Vec::new();
    let mut node_entry_bytes = Vec::new();
    let (mut n_children, mut n_node_entries) = (0u32, 0u32);
    for node in &nodes {
        node_bytes.extend(n_children.to_le_bytes());
        node_bytes.extend((node.children.len() as u32).to_le_bytes());
        node_bytes.extend(n_node_entries.to_le_bytes());
        node_bytes.extend((node.entries.len() as u32).to_le_bytes());
        for &(c, next) in &node.children {
            child_bytes.extend((c as u32).to_le_bytes());
            child_bytes.extend(next.to_le_bytes());
        }
        for &id in &node.entries {
            node_entry_bytes.extend(id.to_le_bytes());
        }
        n_children += node.children.len() as u32;
        n_node_entries += node.entries.len() as u32;
    }
    let prefix_bytes: Vec<u8> = prefixes.iter().flat_map(|k| k.to_le_bytes()).collect();

    let mut out = Vec::new();
    out.extend(MAGIC);
    for n in [
        entries.len() as u32,
        n_syllables,
        nodes.len() as u32,
        n_children,
        n_node_entries,
        prefixes.len() as u32,
        text.len() as u32,
    ] {
        out.extend(n.to_le_bytes());
    }
    out.extend(total_freq.to_le_bytes());
    out.resize(HEADER, 0);
    for section in [
        &entry_bytes,
        &syl_bytes,
        &node_bytes,
        &child_bytes,
        &node_entry_bytes,
        &prefix_bytes,
        &text,
    ] {
        out.extend(section.iter());
        out.resize(align8(out.len()), 0);
    }
    out
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
    if text.is_empty() || text.len() > u16::MAX as usize || text.chars().any(is_symbol) {
        return None;
    }
    let syllables: Option<Vec<Syllable>> =
        reading.split_whitespace().map(Syllable::parse).collect();
    let syllables = syllables?;
    if syllables.len() != text.chars().count() {
        return None;
    }
    // The image stores symbols as u16; every bopomofo symbol and tone is in the BMP.
    if syllables.iter().any(|s| {
        s.base()
            .iter()
            .chain(s.tone().iter())
            .any(|&c| c as u32 > 0xFFFF)
    }) {
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
        assert_eq!(lex.entry(lex.entries_at(g).next().unwrap()).text(), "工");
        assert_eq!(lex.entry(lex.entries_at(gz).next().unwrap()).text(), "工作");
        assert!(lex.child(ROOT, 'ㄗ').is_none());
        let work = lex.entry(lex.entries_at(gz).next().unwrap());
        let syls: Vec<Syllable> = work.syllables().collect();
        assert_eq!(
            syls,
            [
                Syllable::parse("ㄍㄨㄥ").unwrap(),
                Syllable::parse("ㄗㄨㄛˋ").unwrap()
            ]
        );
        assert_eq!(work.freq(), 34279);
    }

    #[test]
    fn homophones_ranked_by_use_in_words() {
        // 氏 and 事 tie; 事 appears in more words. 是 is a higher tier and stays.
        let lex = Lexicon::parse(
            "是,182405,ㄕˋ\n氏,52404,ㄕˋ\n事,52404,ㄕˋ\n故事,5000,ㄍㄨˋ ㄕˋ\n姓氏,100,ㄒㄧㄥˋ ㄕˋ\n",
        );
        let sh = lex.child(ROOT, 'ㄕ').unwrap();
        let order: Vec<&str> = lex.entries_at(sh).map(|i| lex.entry(i).text()).collect();
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
        assert!(!lex.is_syllable_prefix(&[]));
        assert!(!lex.is_syllable_prefix(&['ㄍ', 'ㄨ', 'ㄥ', 'ㄥ']));
    }

    #[test]
    fn image_round_trips_through_a_file() {
        let path = std::env::temp_dir().join(format!("lb-lexicon-{}.bin", std::process::id()));
        std::fs::write(&path, compile(MINI)).unwrap();
        let lex = Lexicon::load(&path).unwrap();
        assert_eq!(lex.len(), 3);
        assert!(lex.is_syllable_prefix(&['ㄇ', 'ㄚ']));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_lexicon_is_valid() {
        let lex = Lexicon::parse("# nothing here\n");
        assert!(lex.is_empty());
        assert!(lex.child(ROOT, 'ㄍ').is_none());
        assert!(!lex.is_syllable_prefix(&['ㄍ']));
    }

    #[test]
    fn rejects_corrupt_images() {
        let good = compile(MINI);
        assert!(Lexicon::from_image(good.clone()).is_ok());
        assert!(Lexicon::from_image(good[..HEADER - 1].to_vec()).is_err());
        assert!(Lexicon::from_image(good[..good.len() / 2].to_vec()).is_err());
        let mut bad_magic = good.clone();
        bad_magic[0] = b'X';
        assert!(Lexicon::from_image(bad_magic).is_err());
        // Point the first child at a node that does not exist.
        let mut bad_child = good.clone();
        let s = Lexicon::from_image(good).unwrap().s;
        bad_child[s.children + 4..s.children + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Lexicon::from_image(bad_child).is_err());
    }
}
