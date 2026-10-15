//! Composer behaviour against the real lexicon — the same scenarios as
//! macos/Tests/main.swift, so both platforms behave alike.

use std::sync::OnceLock;

use lightbopomofo_core::bopomofo::FIRST_TONE;
use lightbopomofo_core::composer::{Composer, Key, KeyResult, MAX_SYMBOLS};
use lightbopomofo_core::lexicon::{compile, Lexicon};
use lightbopomofo_core::{Engine, History};

fn engine() -> Engine {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    let image = IMAGE.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tsi.csv");
        compile(&std::fs::read_to_string(path).expect("run scripts/fetch-lexicon.sh"))
    });
    Engine::new(
        Lexicon::from_image(image.clone()).unwrap(),
        History::in_memory(),
    )
}

fn typing(c: &mut Composer, e: &mut Engine, symbols: &str) {
    for s in symbols.chars() {
        c.handle(e, Key::Symbol(s));
    }
}

fn select(c: &mut Composer, e: &mut Engine, text: &str) -> bool {
    while c.selected() + 1 < c.candidates().len() && c.candidates()[c.selected()].text != text {
        c.handle(e, Key::Next);
    }
    c.candidates()[c.selected()].text == text
}

fn first(c: &Composer) -> &str {
    c.candidates().first().map_or("-", |x| x.text.as_str())
}

fn handled(commit: &str) -> KeyResult {
    KeyResult {
        handled: true,
        commit: commit.into(),
    }
}

#[test]
fn whole_conversion_on_commit() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert_eq!(first(&c), "你好嗎");
    assert_eq!(c.handle(&mut e, Key::Commit), handled("你好嗎"));
    assert!(c.is_empty() && c.candidates().is_empty());
}

#[test]
fn partial_selection_keeps_the_rest() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄍㄗㄈㄈ");
    assert!(select(&mut c, &mut e, "工作"));
    assert_eq!(c.handle(&mut e, Key::Commit).commit, "工作");
    assert_eq!(c.composing_text(), "ㄈㄈ");
    assert_eq!(first(&c), "方法");
    assert_eq!(c.handle(&mut e, Key::Commit).commit, "方法");
    assert!(c.is_empty());
}

#[test]
fn composition_built_from_pieces_is_remembered() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄊㄑㄏㄏ");
    assert!(select(&mut c, &mut e, "天氣"));
    c.handle(&mut e, Key::Commit);
    let rest = c.handle(&mut e, Key::Commit).commit;
    typing(&mut c, &mut e, "ㄊㄑㄏㄏ");
    assert_eq!(first(&c), format!("天氣{rest}"));
}

#[test]
fn selection_is_clamped() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋ");
    assert!(c.handle(&mut e, Key::Previous).handled);
    assert_eq!(c.selected(), 0);
    c.handle(&mut e, Key::Next);
    assert_eq!(c.selected(), 1);
    for _ in 0..500 {
        c.handle(&mut e, Key::Next);
    }
    assert_eq!(c.selected(), c.candidates().len() - 1);
}

#[test]
fn keys_pass_through_when_empty() {
    let (mut e, mut c) = (engine(), Composer::new());
    for key in [
        Key::Commit,
        Key::Backspace,
        Key::Escape,
        Key::Previous,
        Key::Next,
        Key::Other,
        Key::Symbol('ˇ'),
        Key::Symbol(FIRST_TONE), // Space
    ] {
        assert_eq!(
            c.handle(&mut e, key.clone()),
            KeyResult::default(),
            "{key:?}"
        );
    }
}

#[test]
fn backspace_and_escape() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏ");
    assert!(c.handle(&mut e, Key::Backspace).handled);
    assert_eq!(c.composing_text(), "ㄋ");
    c.handle(&mut e, Key::Backspace);
    assert!(c.is_empty() && c.candidates().is_empty());
    typing(&mut c, &mut e, "ㄋㄏ");
    assert!(c.handle(&mut e, Key::Escape).handled);
    assert!(c.is_empty());
}

#[test]
fn punctuation_and_other_keys_commit_first() {
    let (mut e, mut c) = (engine(), Composer::new());
    assert_eq!(
        c.handle(&mut e, Key::Punctuation("，".into())),
        handled("，")
    );
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert_eq!(
        c.handle(&mut e, Key::Punctuation("？".into())).commit,
        "你好嗎？"
    );
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert_eq!(
        c.handle(&mut e, Key::Other),
        KeyResult {
            handled: false,
            commit: "你好嗎".into()
        }
    );
    typing(&mut c, &mut e, "ㄍㄗㄈㄈ");
    select(&mut c, &mut e, "工作");
    assert_eq!(
        c.handle(&mut e, Key::Punctuation("。".into())).commit,
        "工作方法。"
    );
}

#[test]
fn tone_is_part_of_the_composition() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄇㄚˇ");
    assert_eq!(c.composing_text(), "ㄇㄚˇ");
    assert_eq!(first(&c), "馬");
}

#[test]
fn long_input_flushes() {
    let (mut e, mut c) = (engine(), Composer::new());
    let mut flushed = String::new();
    for _ in 0..=MAX_SYMBOLS {
        flushed += &c.handle(&mut e, Key::Symbol('ㄋ')).commit;
    }
    assert!(!flushed.is_empty());
    assert_eq!(c.composing_text(), "ㄋ");
}

#[test]
fn space_is_first_tone() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄊㄧㄢ");
    assert_eq!(c.handle(&mut e, Key::Symbol(FIRST_TONE)), handled(""));
    assert_eq!(c.composing_text(), "ㄊㄧㄢˉ");
    assert_eq!(first(&c), "天");
    // 田 is second tone.
    assert!(c
        .candidates()
        .iter()
        .all(|x| x.text != "田" || x.consumed < 4));
    assert_eq!(c.handle(&mut e, Key::Commit), handled("天"));
}

#[test]
fn space_after_a_tone_does_nothing() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄇㄚˇ");
    assert_eq!(c.handle(&mut e, Key::Symbol(FIRST_TONE)), handled(""));
    assert_eq!(c.composing_text(), "ㄇㄚˇ");
    assert_eq!(first(&c), "馬");
}
