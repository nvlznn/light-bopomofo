//! Composer behaviour against the real lexicon — the same scenarios as
//! macos/Tests/main.swift, so both platforms behave alike.

use std::sync::OnceLock;

use lightbopomofo_core::bopomofo::FIRST_TONE;
use lightbopomofo_core::composer::{Composer, Key, KeyResult, MAX_SYMBOLS, PAGE_SIZE};
use lightbopomofo_core::lexicon::{compile_with_pairs, Lexicon};
use lightbopomofo_core::{Engine, History};

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
        Key::Select(1),
        Key::PagePrevious,
        Key::PageNext,
        Key::Abandon,
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
fn punctuation_types_only_when_not_composing() {
    let (mut e, mut c) = (engine(), Composer::new());
    assert_eq!(
        c.handle(&mut e, Key::Punctuation("，".into())),
        handled("，")
    );
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert_eq!(
        c.handle(&mut e, Key::Punctuation("？".into())),
        handled("")
    );
    assert!(!c.is_empty());
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
        c.handle(&mut e, Key::Punctuation("。".into())),
        handled("")
    );
    assert!(!c.is_empty());
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

#[test]
fn shift_number_commits_that_candidate() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄍㄗㄈㄈ");
    let second = c.candidates()[1].clone();
    assert_eq!(c.handle(&mut e, Key::Select(2)), handled(&second.text));
    assert_eq!(c.composing_text().chars().count(), 4 - second.consumed);
    // Not a number on the page: nothing happens.
    c.reset();
    typing(&mut c, &mut e, "ㄍㄗㄈㄈ");
    assert_eq!(c.handle(&mut e, Key::Select(0)), handled(""));
    assert_eq!(c.composing_text(), "ㄍㄗㄈㄈ");
}

#[test]
fn pages_of_nine() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄕˋ");
    let count = c.candidates().len();
    assert!(count > 2 * PAGE_SIZE);
    c.handle(&mut e, Key::Next);
    assert!(c.handle(&mut e, Key::PageNext).handled);
    assert_eq!(c.selected(), PAGE_SIZE);
    // Shift+number counts within the page shown.
    let third = c.candidates()[PAGE_SIZE + 2].text.clone();
    let mut d = Composer::new();
    typing(&mut d, &mut e, "ㄕˋ");
    d.handle(&mut e, Key::PageNext);
    assert_eq!(d.handle(&mut e, Key::Select(3)), handled(&third));
    // Back, and no further than the first page.
    c.handle(&mut e, Key::PagePrevious);
    assert_eq!(c.selected(), 0);
    c.handle(&mut e, Key::PagePrevious);
    assert_eq!(c.selected(), 0);
    // No further than the last page.
    for _ in 0..50 {
        c.handle(&mut e, Key::PageNext);
    }
    assert_eq!(c.selected(), (count - 1) / PAGE_SIZE * PAGE_SIZE);
}

#[test]
fn switching_language_drops_the_composition() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert_eq!(c.handle(&mut e, Key::Abandon), KeyResult::default());
    assert!(c.is_empty() && c.candidates().is_empty());
}

#[test]
fn bopomofo_itself_ends_the_second_page_and_is_not_learned() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert!(c.candidates().len() > 2 * PAGE_SIZE);
    assert_eq!(c.candidates()[2 * PAGE_SIZE - 1].text, "ㄋㄏㄇ");
    let raw = c.candidates().iter().filter(|x| x.text == "ㄋㄏㄇ").count();
    assert_eq!(raw, 1);
    // A page on, Shift+9 types the symbols; they are not learned.
    c.handle(&mut e, Key::PageNext);
    assert_eq!(c.handle(&mut e, Key::Select(PAGE_SIZE)), handled("ㄋㄏㄇ"));
    assert!(c.is_empty());
    assert!(e.history_entries().is_empty());
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert_eq!(first(&c), "你好嗎");
}

#[test]
fn bopomofo_itself_is_last_when_there_are_few() {
    let e = engine();
    for input in ["ㄋ", "ㄦˋ", "ㄋㄏㄇ", "ㄍㄗㄈㄈ", "ㄖㄖㄖㄖㄖㄖㄖㄖ"] {
        let candidates = e.query_str(input);
        let at = candidates.iter().position(|x| x.text == input);
        let last = candidates.len() - 1;
        // Last of all after three symbols, else last on the second page.
        let want = if input.chars().count() > 3 { last } else { last.min(2 * PAGE_SIZE - 1) };
        assert_eq!(at, Some(want), "{input}");
    }
}

#[test]
fn only_the_bopomofo_candidate_shows_symbols() {
    let e = engine();
    for input in [
        "ㄕㄇㄧㄙ",
        "ㄋㄏㄇ",
        "ㄉㄚ",
        "ㄦㄦㄦㄦ",
        "ㄖㄖㄖㄖㄖㄖㄖㄖ",
        "ㄍㄗㄈㄈㄓㄓ",
    ] {
        let candidates = e.query_str(input);
        let mixed: Vec<&str> = candidates
            .iter()
            .map(|c| c.text.as_str())
            .filter(|t| *t != input && t.chars().any(lightbopomofo_core::bopomofo::is_symbol))
            .collect();
        assert!(mixed.is_empty(), "{input}: {mixed:?}");
        assert!(candidates.iter().any(|c| c.text == input), "{input}");
    }
}
