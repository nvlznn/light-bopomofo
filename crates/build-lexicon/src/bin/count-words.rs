//! count-words: how often each lexicon word, and each pair of words in a
//! row, appears in plain text (scripts/corpus-text.py), for the word-pair
//! statistics.
//!
//!   count-words data/tsi.csv data/corpus < data/corpus/text.txt
//!
//! Each line is split into the lexicon's words, the most likely split by
//! the words' frequencies; a character the lexicon lacks breaks the line.
//! Writes OUT_DIR/words.tsv (word, count) and OUT_DIR/pairs.tsv (word, next
//! word, count) for pairs seen at least MIN_PAIR times.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::io::{BufRead, BufWriter, Write};

use lightbopomofo_core::Lexicon;

/// Pairs kept in the output.
const MIN_PAIR: u32 = 3;
/// Above this many distinct pairs, those seen once are dropped (memory).
const MAX_PAIRS: usize = 80_000_000;

/// FxHash: much faster than SipHash for integer keys.
#[derive(Default)]
struct Fx(u64);
impl Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}
type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (lexicon_path, out_dir) = match &args[..] {
        [_, l, o] => (l, o),
        _ => {
            eprintln!("usage: count-words LEXICON OUT_DIR < TEXT");
            std::process::exit(2);
        }
    };
    let lexicon = Lexicon::load(lexicon_path).expect("lexicon");

    // Words by text, with their total frequency over readings.
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut words: Vec<String> = Vec::new();
    let mut freq: Vec<f64> = Vec::new();
    for id in 0..lexicon.len() as u32 {
        let entry = lexicon.entry(id);
        let i = *ids.entry(entry.text().to_string()).or_insert_with(|| {
            words.push(entry.text().to_string());
            freq.push(0.0);
            (words.len() - 1) as u32
        });
        freq[i as usize] += entry.freq() as f64 + 1.0;
    }
    let total: f64 = freq.iter().sum();
    let cost: Vec<f64> = freq.iter().map(|f| -(f / total).ln()).collect();
    let longest = words.iter().map(|w| w.chars().count()).max().unwrap_or(1);

    let mut word_count = vec![0u64; words.len()];
    let mut pairs: FxMap<u64, u32> = FxMap::default();
    let (mut lines, mut chars) = (0u64, 0u64);
    let stdin = std::io::stdin();
    let mut line = String::new();
    let mut lock = stdin.lock();
    let mut offsets = Vec::new();
    let mut best: Vec<(f64, usize, Option<u32>)> = Vec::new();
    let mut split = Vec::new();
    loop {
        line.clear();
        if lock.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let text = line.trim_end();
        offsets.clear();
        offsets.extend(text.char_indices().map(|(i, _)| i));
        offsets.push(text.len());
        let n = offsets.len() - 1;
        lines += 1;
        chars += n as u64;

        // best[j]: cheapest split of text[..j]; a character the lexicon
        // lacks costs a lot and has no word.
        best.clear();
        best.resize(n + 1, (f64::INFINITY, 0, None));
        best[0].0 = 0.0;
        for i in 0..n {
            let base = best[i].0;
            if base == f64::INFINITY {
                continue;
            }
            let unknown = (base + 30.0, i, None);
            if unknown.0 < best[i + 1].0 {
                best[i + 1] = unknown;
            }
            for len in 1..=longest.min(n - i) {
                if let Some(&id) = ids.get(&text[offsets[i]..offsets[i + len]]) {
                    let c = base + cost[id as usize];
                    if c < best[i + len].0 {
                        best[i + len] = (c, i, Some(id));
                    }
                }
            }
        }
        split.clear();
        let mut j = n;
        while j > 0 {
            split.push(best[j].2);
            j = best[j].1;
        }
        split.reverse();

        let mut prev: Option<u32> = None;
        for &word in &split {
            if let Some(w) = word {
                word_count[w as usize] += 1;
                if let Some(p) = prev {
                    *pairs.entry((p as u64) << 32 | w as u64).or_insert(0) += 1;
                }
            }
            prev = word;
        }
        if pairs.len() > MAX_PAIRS {
            let before = pairs.len();
            pairs.retain(|_, c| *c > 1);
            eprintln!("pruned pairs seen once: {before} → {}", pairs.len());
        }
        if lines % 10_000_000 == 0 {
            eprintln!("{lines} lines, {chars} characters, {} pairs", pairs.len());
        }
    }
    eprintln!("{lines} lines, {chars} characters, {} pairs", pairs.len());

    let mut out =
        BufWriter::new(std::fs::File::create(format!("{out_dir}/words.tsv")).expect("words.tsv"));
    for (w, &c) in words.iter().zip(&word_count) {
        if c > 0 {
            writeln!(out, "{w}\t{c}").unwrap();
        }
    }
    let mut kept: Vec<(u64, u32)> = pairs.into_iter().filter(|&(_, c)| c >= MIN_PAIR).collect();
    kept.sort_unstable_by_key(|&(_, c)| std::cmp::Reverse(c));
    let mut out =
        BufWriter::new(std::fs::File::create(format!("{out_dir}/pairs.tsv")).expect("pairs.tsv"));
    for (key, c) in &kept {
        let (a, b) = ((key >> 32) as usize, (key & 0xffff_ffff) as usize);
        writeln!(out, "{}\t{}\t{c}", words[a], words[b]).unwrap();
    }
    eprintln!("{} pairs seen at least {MIN_PAIR} times", kept.len());
}
