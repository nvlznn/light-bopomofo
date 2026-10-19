//! lb-cli: type bopomofo (or 大千 keys) and see the candidates.
//!
//!   lb-cli ㄋㄏㄇ ㄍㄗㄈㄈ          query each argument
//!   lb-cli --dachen sca             arguments are 大千 keys
//!   lb-cli                          interactive; lines starting with `:` are 大千 keys
//!
//!   lb-cli --eval eval/tatoeba.tsv  measure: phrases typed in full and by
//!                                   first symbols (-n: misses to show)
//!
//! Options: --lexicon PATH (default data/tsi.csv), --history PATH, -n COUNT,
//! --pairs DIR (words.tsv and pairs.tsv from count-words), --pair-weight W

use std::io::{BufRead, Write};
use std::time::Instant;

use lightbopomofo_core::{dachen, Engine, History, Lexicon};

fn main() {
    let mut lexicon_path = default_lexicon();
    let mut history_path: Option<String> = None;
    let mut count = 9;
    let mut dachen_keys = false;
    let mut eval_path: Option<String> = None;
    let mut pairs_dir: Option<String> = None;
    let mut pair_weight = 1.0;
    let mut inputs = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = args.next().expect("--lexicon PATH"),
            "--history" => history_path = Some(args.next().expect("--history PATH")),
            "-n" => count = args.next().and_then(|n| n.parse().ok()).expect("-n COUNT"),
            "--dachen" => dachen_keys = true,
            "--eval" => eval_path = Some(args.next().expect("--eval FILE")),
            "--pairs" => pairs_dir = Some(args.next().expect("--pairs DIR")),
            "--pair-weight" => {
                pair_weight = args
                    .next()
                    .and_then(|w| w.parse().ok())
                    .expect("--pair-weight W")
            }
            "-h" | "--help" => {
                eprintln!("usage: lb-cli [--dachen] [--lexicon PATH] [--history PATH] [-n COUNT] [--eval FILE | INPUT...]");
                return;
            }
            _ => inputs.push(arg),
        }
    }

    let t = Instant::now();
    let lexicon = Lexicon::load(&lexicon_path).unwrap_or_else(|e| {
        eprintln!("cannot read {lexicon_path}: {e}\nrun scripts/fetch-lexicon.sh first");
        std::process::exit(1);
    });
    let history = history_path.map_or_else(History::in_memory, History::load);
    let mut engine = Engine::new(lexicon, history);
    if let Some(dir) = &pairs_dir {
        let read = |name: &str| {
            std::fs::read_to_string(format!("{dir}/{name}")).unwrap_or_else(|e| {
                eprintln!("cannot read {dir}/{name}: {e}");
                std::process::exit(1);
            })
        };
        engine.load_pairs(&read("words.tsv"), &read("pairs.tsv"), pair_weight);
    }
    eprintln!(
        "loaded {} entries in {:.0?}",
        engine.lexicon().len(),
        t.elapsed()
    );

    let show = |line: &str, keys: bool| {
        let symbols = if keys {
            dachen::convert(line)
        } else {
            line.trim().to_string()
        };
        if symbols.is_empty() {
            return;
        }
        let t = Instant::now();
        let candidates = engine.query_str(&symbols);
        let elapsed = t.elapsed();
        let total = symbols.chars().count();
        println!(
            "{symbols}  ({} candidates, {:.2?})",
            candidates.len(),
            elapsed
        );
        for (i, c) in candidates.iter().take(count).enumerate() {
            let rest: String = symbols.chars().skip(c.consumed).collect();
            let rest = if c.consumed < total {
                format!("  + {rest}")
            } else {
                String::new()
            };
            println!("  {}. {}{}   [{:.2}]", i + 1, c.text, rest, c.score);
        }
    };

    if let Some(path) = &eval_path {
        evaluate(&engine, path, count);
        return;
    }
    if !inputs.is_empty() {
        for input in &inputs {
            show(input, dachen_keys);
        }
        return;
    }
    let stdin = std::io::stdin();
    loop {
        print!("> ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end_matches(['\n', '\r']);
        match line.strip_prefix(':') {
            Some(keys) => show(keys, true),
            None => show(line, dachen_keys),
        }
    }
}

/// `--eval FILE`: lines of text, its full bopomofo and its first symbols
/// (scripts/gen-eval.py). For each way of typing it: how often the first
/// candidate is the text, how many of its characters are right, and how
/// often the text is among the first five. Prints up to `misses` misses.
fn evaluate(engine: &Engine, path: &str, misses: usize) {
    let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1);
    });
    let cases: Vec<Vec<&str>> = source
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|f| f.len() >= 3)
        .collect();
    for (column, name) in [(1, "full"), (2, "first symbols")] {
        let (mut sentences, mut in_five, mut chars, mut chars_right) = (0, 0, 0, 0);
        let mut shown = 0;
        let t = Instant::now();
        for case in &cases {
            let (text, typed) = (case[0], case[column]);
            let candidates = engine.query_str(typed);
            let first = candidates.first().map_or("", |c| c.text.as_str());
            chars += text.chars().count();
            chars_right += text
                .chars()
                .zip(first.chars())
                .filter(|(a, b)| a == b)
                .count();
            if first == text {
                sentences += 1;
            } else if shown < misses {
                println!("  {typed}  {first}  (want {text})");
                shown += 1;
            }
            if candidates.iter().take(5).any(|c| c.text == text) {
                in_five += 1;
            }
        }
        let pct = |n: usize, of: usize| 100.0 * n as f64 / of as f64;
        println!(
            "{name}: sentence {:.1}%, characters {:.1}%, in first five {:.1}%  ({} phrases, {:.2?})",
            pct(sentences, cases.len()),
            pct(chars_right, chars),
            pct(in_five, cases.len()),
            cases.len(),
            t.elapsed()
        );
    }
}

fn default_lexicon() -> String {
    let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tsi.csv");
    if std::path::Path::new(repo).exists() {
        repo.to_string()
    } else {
        "data/tsi.csv".to_string()
    }
}
