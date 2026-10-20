//! The tutorial's lessons against the real lexicon — the same scenarios as
//! macos/Tests/main.swift.

use std::sync::OnceLock;

use lightbopomofo_core::lexicon::{compile_with_pairs, Lexicon};
use lightbopomofo_core::tutorial::{Practice, LESSONS};
use lightbopomofo_core::{Engine, History, Key};

fn engine() -> Engine {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    let image = IMAGE.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tsi.csv");
        let pairs = concat!(env!("CARGO_MANIFEST_DIR"), "/word-pairs.tsv");
        compile_with_pairs(
            &std::fs::read_to_string(path).expect("run scripts/fetch-lexicon.sh"),
            &std::fs::read_to_string(pairs).unwrap(),
        )
    });
    Engine::new(Lexicon::from_image(image.clone()).unwrap(), History::in_memory())
}

fn keys(symbols: &str, then: &[Key]) -> Vec<Key> {
    symbols.chars().map(Key::Symbol).chain(then.iter().cloned()).collect()
}

#[test]
fn lessons_follow_their_plans_and_carry_learning_over() {
    use Key::{Commit, Next, PageNext};
    let mut e = engine();
    assert_eq!(e.query_str("ㄇㄇㄒㄒㄋ")[0].text, "密碼謝謝你", "the whole comes out wrong at first");
    let plans = [
        keys("ㄋㄏㄇ", &[Commit]),
        keys("ㄇㄊ", &[Next, Next, Commit]),
        keys("ㄏㄏ", &[PageNext, Next, Commit]),
        // 媽媽 is 4th; then ㄒㄒㄋ is 謝謝你 first.
        keys("ㄇㄇㄒㄒㄋ", &[Next, Next, Next, Commit, Commit]),
        // Learned from the pieces: first as a whole.
        keys("ㄇㄇㄒㄒㄋ", &[Commit]),
    ];
    for (lesson, expected) in LESSONS.iter().zip(plans) {
        let mut p = Practice::new(*lesson, &e);
        assert_eq!(p.plan(), expected, "{} plan", lesson.target);
        for (i, key) in expected.iter().enumerate() {
            assert_eq!(p.progress(), Some(i), "{} step {i}", lesson.target);
            assert_eq!(p.next_key().as_ref(), Some(key), "{} step {i}", lesson.target);
            p.handle(&mut e, key.clone());
        }
        assert!(p.is_done() && p.next_key().is_none(), "{} done", lesson.target);
    }
}

#[test]
fn off_the_plan() {
    let mut e = engine();
    let mut p = Practice::new(LESSONS[1], &e);
    p.handle(&mut e, Key::Symbol('ㄇ'));
    p.handle(&mut e, Key::Symbol('ㄋ'));
    assert_eq!(p.next_key(), Some(Key::Escape), "wrong symbol asks for Esc");
    p.handle(&mut e, Key::Escape);
    assert_eq!(p.next_key(), Some(Key::Symbol('ㄇ')), "after Esc, back to the start");
    p.handle(&mut e, Key::Symbol('ㄇ'));
    p.handle(&mut e, Key::Symbol('ㄊ'));
    p.handle(&mut e, Key::Commit);
    assert!(p.went_wrong() && !p.is_done(), "committed the wrong word");
    p.restart();
    assert_eq!(p.next_key(), Some(Key::Symbol('ㄇ')));
    assert!(!p.went_wrong());
}

#[test]
fn wrong_piece_asks_for_esc() {
    let mut e = engine();
    let mut p = Practice::new(LESSONS[3], &e);
    for s in "ㄇㄇㄒㄒㄋ".chars() {
        p.handle(&mut e, Key::Symbol(s));
    }
    let wrong = e.query_str("ㄇㄇㄒㄒㄋ").iter().position(|c| c.text == "密碼").unwrap();
    for _ in 0..wrong {
        p.handle(&mut e, Key::Next);
    }
    p.handle(&mut e, Key::Commit); // 密碼, not 媽媽
    assert_eq!(p.committed(), "密碼");
    assert_eq!(p.next_key(), Some(Key::Escape));
    p.handle(&mut e, Key::Escape);
    assert!(p.went_wrong(), "Esc leaves the wrong piece: start over");
}
