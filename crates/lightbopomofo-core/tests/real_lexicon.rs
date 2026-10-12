//! Behaviour against the real chewing lexicon (run scripts/fetch-lexicon.sh).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use lightbopomofo_core::{Candidate, Engine, History, Lexicon};

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tsi.csv");
        let lexicon =
            Lexicon::load(path).expect("data/tsi.csv missing: run scripts/fetch-lexicon.sh");
        Engine::new(lexicon, History::in_memory())
    })
}

fn texts(c: &[Candidate]) -> Vec<&str> {
    c.iter().map(|c| c.text.as_str()).collect()
}

fn top(input: &str, n: usize) -> Vec<String> {
    engine()
        .query_str(input)
        .into_iter()
        .take(n)
        .map(|c| c.text)
        .collect()
}

#[test]
fn abbreviated_greeting() {
    assert_eq!(top("ㄋㄏㄇ", 1), ["你好嗎"]);
}

#[test]
fn abbreviated_phrase_and_prefix() {
    let c = engine().query_str("ㄍㄗㄈㄈ");
    assert_eq!(c[0].text, "工作方法");
    let gz = c
        .iter()
        .find(|c| c.text == "工作")
        .expect("工作 offered as prefix");
    assert_eq!(gz.consumed, 2);
    assert!(
        c.iter().position(|c| c.text == "工作").unwrap() < 9,
        "{:?}",
        texts(&c)
    );
}

#[test]
fn mixed_full_and_abbreviated() {
    assert!(top("ㄋㄧˇㄏㄇ", 5).contains(&"你好嗎".to_string()));
    assert_eq!(top("ㄋㄧㄏㄠㄇㄚ", 1), ["你好嗎"]);
}

#[test]
fn polyphones_found_by_either_reading() {
    assert!(top("ㄒㄧㄥˊ", 20).contains(&"行".to_string()));
    assert!(top("ㄏㄤˊ", 20).contains(&"行".to_string()));
}

#[test]
fn tone_narrows_candidates() {
    // ㄇㄚˇ: 馬 (third tone) beats 媽 (first tone), which must not appear.
    let c = top("ㄇㄚˇ", 30);
    assert!(c.contains(&"馬".to_string()), "{c:?}");
    assert!(!c.contains(&"媽".to_string()), "{c:?}");
}

#[test]
fn long_input_is_fast() {
    // 40 symbols of mixed abbreviated input; well under the 10 ms budget.
    let input =
        "ㄐㄊㄊㄑㄏㄏㄨㄇㄑㄍㄩㄢˊㄗㄡˇㄗㄡˇㄨㄒㄓㄑㄩㄔㄈㄍㄗㄈㄈㄋㄏㄇㄊㄑㄏㄏㄨㄒㄓㄐㄊㄊ";
    assert!(input.chars().count() >= 40);
    engine().query_str(input); // warm up
    let mut worst = Duration::ZERO;
    for _ in 0..20 {
        let t = Instant::now();
        let c = engine().query_str(input);
        worst = worst.max(t.elapsed());
        assert_eq!(c[0].consumed, input.chars().count());
    }
    assert!(worst < Duration::from_millis(10), "worst query {worst:?}");
}
