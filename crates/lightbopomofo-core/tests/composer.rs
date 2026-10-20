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
fn bopomofo_itself_opens_the_third_page_and_is_not_learned() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    assert!(c.candidates().len() > 2 * PAGE_SIZE);
    assert_eq!(c.candidates()[2 * PAGE_SIZE].text, "ㄋㄏㄇ");
    let raw = c.candidates().iter().filter(|x| x.text == "ㄋㄏㄇ").count();
    assert_eq!(raw, 1);
    // Two pages on, Shift+1 types the symbols; they are not learned.
    c.handle(&mut e, Key::PageNext);
    c.handle(&mut e, Key::PageNext);
    assert_eq!(c.handle(&mut e, Key::Select(1)), handled("ㄋㄏㄇ"));
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
        assert_eq!(
            at,
            Some((candidates.len() - 1).min(2 * PAGE_SIZE)),
            "{input}"
        );
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

/// Type 大千 keys: lowercase as bopomofo, uppercase with Shift.
fn keys(c: &mut Composer, e: &mut Engine, keys: &str) {
    for k in keys.chars() {
        let key = if k.is_ascii_uppercase() {
            Key::Letter(k)
        } else {
            Key::Symbol(lightbopomofo_core::dachen::symbol(k).unwrap())
        };
        c.handle(e, key);
    }
}

/// The whole conversion of `input`, as the English candidates use it.
fn whole(e: &Engine, input: &str) -> String {
    let n = input.chars().count();
    e.query_str(input)
        .into_iter()
        .find(|x| x.consumed == n)
        .unwrap()
        .text
}

#[test]
fn a_capital_starts_english_and_space_ends_it() {
    let (mut e, mut c) = (engine(), Composer::new());
    keys(&mut c, &mut e, "iPhone");
    assert_eq!(c.composing_text(), "ㄛPhone");
    assert_eq!(first(&c), "iPhone");
    assert_eq!(
        c.handle(&mut e, Key::Symbol(FIRST_TONE)),
        KeyResult {
            handled: false,
            commit: "iPhone".into()
        }
    );
    assert!(c.is_empty());
    // Learned as its keys: in lowercase it comes back as typed; never shared.
    keys(&mut c, &mut e, "iphone");
    assert_eq!(first(&c), "iPhone");
    assert!(e.new_words().is_empty());
}

#[test]
fn english_typed_once_comes_back_in_lowercase() {
    let (mut e, mut c) = (engine(), Composer::new());
    keys(&mut c, &mut e, "kahoot");
    assert_ne!(first(&c), "Kahoot");
    c.reset();
    // Typed once with Shift, after Chinese: only the English part is learned.
    typing(&mut c, &mut e, "ㄋㄏ");
    keys(&mut c, &mut e, "Kahoot");
    c.handle(&mut e, Key::Commit);
    keys(&mut c, &mut e, "kahoot");
    assert_eq!(first(&c), "Kahoot");
    assert!(e.new_words().is_empty(), "English is never shared");
}

#[test]
fn english_after_bopomofo_keeps_the_chinese() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏ");
    keys(&mut c, &mut e, "API");
    assert_eq!(first(&c), format!("{}API", whole(&e, "ㄋㄏ")));
    // Other places it could start are a Tab away.
    assert!(c.candidates().iter().any(|x| x.text == "scAPI"));
    assert!(select(&mut c, &mut e, "scAPI"));
    assert_eq!(c.handle(&mut e, Key::Commit), handled("scAPI"));
    assert!(c.is_empty());
}

#[test]
fn english_takes_the_lowercase_start_and_digits_before_it() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏ");
    keys(&mut c, &mut e, "iPad");
    assert_eq!(first(&c), format!("{}iPad", whole(&e, "ㄋㄏ")));
    c.reset();
    keys(&mut c, &mut e, "5G");
    assert_eq!(first(&c), "5G");
    c.reset();
    keys(&mut c, &mut e, "1080P");
    assert_eq!(first(&c), "1080P");
    c.reset();
    // A tone after bopomofo stays a tone: 你 + API.
    typing(&mut c, &mut e, "ㄋㄧˇ");
    keys(&mut c, &mut e, "API");
    assert_eq!(first(&c), format!("{}API", whole(&e, "ㄋㄧˇ")));
}

#[test]
fn english_alone_and_punctuation_ending_it() {
    let (mut e, mut c) = (engine(), Composer::new());
    // 4G: the 4 alone went to the app, G starts English by itself.
    keys(&mut c, &mut e, "G");
    assert_eq!(c.composing_text(), "G");
    assert_eq!(first(&c), "G");
    c.reset();
    keys(&mut c, &mut e, "API");
    assert_eq!(
        c.handle(&mut e, Key::Punctuation("，".into())),
        handled("API，")
    );
    assert!(c.is_empty());
    // Keys that are bopomofo or tones type their letter or digit in English.
    keys(&mut c, &mut e, "Node");
    c.handle(&mut e, Key::Symbol('ㄡ')); // .
    keys(&mut c, &mut e, "js");
    assert_eq!(first(&c), "Node.js");
    c.reset();
    keys(&mut c, &mut e, "M");
    c.handle(&mut e, Key::Symbol('ˇ')); // 3
    assert_eq!(first(&c), "M3");
}

#[test]
fn backspace_back_into_bopomofo() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏ");
    keys(&mut c, &mut e, "A");
    assert!(c.handle(&mut e, Key::Backspace).handled);
    assert_eq!(c.composing_text(), "ㄋㄏ");
    assert_eq!(first(&c), whole(&e, "ㄋㄏ"));
    keys(&mut c, &mut e, "A");
    assert!(c.handle(&mut e, Key::Escape).handled);
    assert!(c.is_empty());
}

#[test]
fn lowercase_english_is_on_the_first_page_and_learned() {
    let (mut e, mut c) = (engine(), Composer::new());
    // Not an English word the engine knows: the keys themselves are offered.
    keys(&mut c, &mut e, "kahoot");
    let at = c.candidates().iter().position(|x| x.text == "kahoot");
    assert_eq!(at, Some((c.candidates().len() - 1).min(PAGE_SIZE - 1)));
    assert_eq!(
        c.handle(&mut e, Key::Select(at.unwrap() + 1)),
        handled("kahoot")
    );
    assert!(c.is_empty());
    keys(&mut c, &mut e, "kahoot");
    assert_eq!(first(&c), "kahoot", "learned");
    assert!(e.new_words().is_empty(), "English is never shared");
    c.reset();
    // Not offered for keys with Space in them, or for initials only (簡拼).
    typing(&mut c, &mut e, "ㄊㄧㄢˉ");
    assert!(c.candidates().iter().all(|x| x.text != "wu0 "));
    c.reset();
    typing(&mut c, &mut e, "ㄏㄏ");
    assert!(c.candidates().iter().all(|x| x.text != "cc"));
}

#[test]
fn english_flushes_at_max_length() {
    let (mut e, mut c) = (engine(), Composer::new());
    let mut flushed = String::new();
    for _ in 0..=MAX_SYMBOLS {
        flushed += &c.handle(&mut e, Key::Letter('A')).commit;
    }
    assert_eq!(flushed, "A".repeat(MAX_SYMBOLS));
    assert_eq!(c.composing_text(), "A");
}

#[test]
fn english_words_come_out_typed_in_lowercase() {
    let (mut e, mut c) = (engine(), Composer::new());
    // Built-in English words, alone and inside Chinese; no Shift.
    keys(&mut c, &mut e, "google");
    assert_eq!(first(&c), "Google");
    c.reset();
    keys(&mut c, &mut e, "iphone");
    assert_eq!(first(&c), "iPhone");
    c.reset();
    typing(&mut c, &mut e, "ㄋㄏ");
    keys(&mut c, &mut e, "email");
    assert_eq!(first(&c), format!("{}email", whole(&e, "ㄋㄏ")));
    c.reset();
    // Keys that are everyday 簡拼 stay Chinese: yes is ㄗㄍㄋ.
    typing(&mut c, &mut e, "ㄒㄗㄍㄋㄌ");
    assert!(!first(&c).contains("yes"), "{}", first(&c));
}

#[test]
fn learned_english_that_looks_like_jianpin_comes_only_alone() {
    let (mut e, mut c) = (engine(), Composer::new());
    keys(&mut c, &mut e, "Yes");
    c.handle(&mut e, Key::Commit);
    keys(&mut c, &mut e, "yes");
    assert_eq!(first(&c), "Yes", "typed alone");
    c.reset();
    let before = whole(&engine(), "ㄒㄗㄍㄋㄌ");
    typing(&mut c, &mut e, "ㄒㄗㄍㄋㄌ");
    assert_eq!(first(&c), before, "not inside a sentence");
}

#[test]
fn number_keys_choose_once_moving_among_candidates() {
    let (mut e, mut c) = (engine(), Composer::new());
    // Not moved yet: 5 is ㄓ, typed as usual.
    typing(&mut c, &mut e, "ㄍㄗㄈㄈ");
    c.handle(&mut e, Key::Symbol('ㄓ'));
    assert_eq!(c.composing_text(), "ㄍㄗㄈㄈㄓ");
    c.handle(&mut e, Key::Backspace);
    // After Tab: 3 (ˇ on 大千) chooses the third of the page.
    c.handle(&mut e, Key::Next);
    assert!(c.is_choosing());
    let third = c.candidates()[2].text.clone();
    assert_eq!(c.handle(&mut e, Key::Symbol('ˇ')), handled(&third));
    // Space commits the one highlighted.
    c.reset();
    typing(&mut c, &mut e, "ㄋㄏㄇ");
    c.handle(&mut e, Key::Next);
    let second = c.candidates()[1].text.clone();
    assert_eq!(c.handle(&mut e, Key::Symbol(FIRST_TONE)), handled(&second));
    c.reset();
    // Any other key goes back to typing.
    typing(&mut c, &mut e, "ㄋㄏ");
    c.handle(&mut e, Key::Next);
    c.handle(&mut e, Key::Symbol('ㄇ'));
    assert!(!c.is_choosing());
    assert_eq!(c.composing_text(), "ㄋㄏㄇ");
}

#[test]
fn readings_around_english_are_one_choice_on_the_first_page() {
    let (mut e, mut c) = (engine(), Composer::new());
    typing(&mut c, &mut e, "ㄋㄏ");
    keys(&mut c, &mut e, "app");
    let at = c.candidates().iter().position(|x| x.text == "你好app");
    assert!(
        at.is_some_and(|i| i < PAGE_SIZE),
        "{:?}",
        c.candidates().iter().take(PAGE_SIZE).map(|x| &x.text).collect::<Vec<_>>()
    );
    assert_eq!(
        c.handle(&mut e, Key::Select(at.unwrap() + 1)),
        handled("你好app")
    );
    // Learned piece by piece: ㄋㄏ is 你好 from now on, also alone.
    typing(&mut c, &mut e, "ㄋㄏ");
    assert_eq!(first(&c), "你好");
}
