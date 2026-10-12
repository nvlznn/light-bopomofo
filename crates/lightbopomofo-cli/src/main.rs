//! lb-cli: type bopomofo (or 大千 keys) and see the candidates.
//!
//!   lb-cli ㄋㄏㄇ ㄍㄗㄈㄈ          query each argument
//!   lb-cli --dachen sca             arguments are 大千 keys
//!   lb-cli                          interactive; lines starting with `:` are 大千 keys
//!
//! Options: --lexicon PATH (default data/tsi.csv), --history PATH, -n COUNT

use std::io::{BufRead, Write};
use std::time::Instant;

use lightbopomofo_core::{dachen, Engine, History, Lexicon};

fn main() {
    let mut lexicon_path = default_lexicon();
    let mut history_path: Option<String> = None;
    let mut count = 9;
    let mut dachen_keys = false;
    let mut inputs = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = args.next().expect("--lexicon PATH"),
            "--history" => history_path = Some(args.next().expect("--history PATH")),
            "-n" => count = args.next().and_then(|n| n.parse().ok()).expect("-n COUNT"),
            "--dachen" => dachen_keys = true,
            "-h" | "--help" => {
                eprintln!("usage: lb-cli [--dachen] [--lexicon PATH] [--history PATH] [-n COUNT] [INPUT...]");
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
    let engine = Engine::new(lexicon, history);
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

fn default_lexicon() -> String {
    let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tsi.csv");
    if std::path::Path::new(repo).exists() {
        repo.to_string()
    } else {
        "data/tsi.csv".to_string()
    }
}
