//! Query speed on long input: cargo run --release --example bench -- LEXICON
//! (a lexicon.bin or a tsi.csv). Prints median and p95 of 200 queries.

use std::time::{Duration, Instant};

use lightbopomofo_core::{Engine, History, Lexicon};

const INPUTS: [&str; 3] = [
    "ㄐㄊㄊㄑㄏㄏㄨㄇㄑㄍㄩㄢˊㄗㄡˇㄗㄡˇㄨㄒㄓㄑㄩㄔㄈㄍㄗㄈㄈㄋㄏㄇㄊㄑㄏㄏㄨㄒㄓㄐㄊㄊ",
    "ㄋㄏㄇ",
    "ㄍㄗㄈㄈ",
];

fn main() {
    let path = std::env::args().nth(1).expect("usage: bench LEXICON");
    let t = Instant::now();
    let engine = Engine::new(Lexicon::load(&path).unwrap(), History::in_memory());
    println!("open {:.1?}", t.elapsed());
    for input in INPUTS {
        let chars: Vec<char> = input.chars().collect();
        engine.query(&chars);
        let mut times: Vec<Duration> = (0..200)
            .map(|_| {
                let t = Instant::now();
                std::hint::black_box(engine.query(&chars));
                t.elapsed()
            })
            .collect();
        times.sort();
        println!(
            "{:>2} symbols: median {:.2?}, p95 {:.2?}",
            chars.len(),
            times[100],
            times[190]
        );
    }
}
