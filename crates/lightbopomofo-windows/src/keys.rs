//! Windows virtual-key codes → platform-independent [`Key`]s.
//!
//! Pure functions with no Windows API, so they are unit-tested everywhere.
//! 大千 uses physical positions, so keys are read as virtual-key codes (the
//! US-layout key at that position), not as characters of the active layout.

use lightbopomofo_core::bopomofo::FIRST_TONE;
use lightbopomofo_core::{dachen, Key};

#[allow(dead_code)]
pub mod vk {
    pub const BACK: u16 = 0x08;
    pub const TAB: u16 = 0x09;
    pub const RETURN: u16 = 0x0D;
    pub const SHIFT: u16 = 0x10;
    pub const CONTROL: u16 = 0x11;
    pub const MENU: u16 = 0x12;
    pub const CAPITAL: u16 = 0x14;
    pub const ESCAPE: u16 = 0x1B;
    pub const SPACE: u16 = 0x20;
    pub const PRIOR: u16 = 0x21; // PageUp
    pub const NEXT: u16 = 0x22; // PageDown
    pub const LEFT: u16 = 0x25;
    pub const UP: u16 = 0x26;
    pub const RIGHT: u16 = 0x27;
    pub const DOWN: u16 = 0x28;
    pub const LWIN: u16 = 0x5B;
    pub const RWIN: u16 = 0x5C;
    pub const NUMPAD0: u16 = 0x60;
    pub const DIVIDE: u16 = 0x6F;
    pub const LSHIFT: u16 = 0xA0;
    pub const RMENU: u16 = 0xA5;
    pub const OEM_1: u16 = 0xBA; // ; :
    pub const OEM_COMMA: u16 = 0xBC; // , <
    pub const OEM_MINUS: u16 = 0xBD; // - _
    pub const OEM_PERIOD: u16 = 0xBE; // . >
    pub const OEM_2: u16 = 0xBF; // / ?
    pub const OEM_4: u16 = 0xDB; // [ {
    pub const OEM_6: u16 = 0xDD; // ] }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub win: bool,
}

/// Modifier keys themselves: never ours, and never a reason to commit
/// (Shift is pressed on the way to Shift+, for example).
pub fn is_modifier(vk: u16) -> bool {
    matches!(
        vk,
        vk::SHIFT | vk::CONTROL | vk::MENU | vk::CAPITAL | vk::LWIN | vk::RWIN
    ) || (vk::LSHIFT..=vk::RMENU).contains(&vk)
}

/// The unshifted US-layout character on the key, for 大千 lookup.
fn us_char(vk: u16) -> Option<char> {
    match vk {
        0x30..=0x39 => Some(vk as u8 as char), // 0–9
        0x41..=0x5A => Some((vk as u8 as char).to_ascii_lowercase()), // A–Z
        vk::OEM_COMMA => Some(','),
        vk::OEM_PERIOD => Some('.'),
        vk::OEM_2 => Some('/'),
        vk::OEM_1 => Some(';'),
        vk::OEM_MINUS => Some('-'),
        _ => None,
    }
}

/// Full-width punctuation typed with Shift: , . / 1 ; → ，。？！：
fn shifted_punctuation(vk: u16) -> Option<&'static str> {
    match vk {
        vk::OEM_COMMA => Some("，"),
        vk::OEM_PERIOD => Some("。"),
        vk::OEM_2 => Some("？"),
        0x31 => Some("！"),
        vk::OEM_1 => Some("："),
        _ => None,
    }
}

/// Translate a key press. `None` for bare modifier keys, which must not
/// affect the composition at all. While `composing`, Shift+1–9 choose a
/// candidate (otherwise Shift+1 is ！).
pub fn translate(vk: u16, m: Modifiers, composing: bool) -> Option<Key> {
    if is_modifier(vk) {
        return None;
    }
    if m.ctrl || m.alt || m.win {
        return Some(Key::Other); // shortcuts: commit, then the app's
    }
    let key = match vk {
        vk::SPACE => Key::Symbol(FIRST_TONE),
        vk::RETURN => Key::Commit,
        vk::BACK => Key::Backspace,
        vk::ESCAPE => Key::Escape,
        vk::LEFT | vk::UP => Key::Previous,
        vk::RIGHT | vk::DOWN => Key::Next,
        vk::TAB if m.shift => Key::Previous,
        vk::TAB => Key::Next,
        vk::PRIOR => Key::PagePrevious,
        vk::NEXT => Key::PageNext,
        vk::OEM_4 if !m.shift => Key::PagePrevious,
        vk::OEM_6 if !m.shift => Key::PageNext,
        0x31..=0x39 if m.shift && composing => Key::Select((vk - 0x30) as usize),
        vk::NUMPAD0..=vk::DIVIDE => Key::Other, // keypad numbers stay numbers
        _ if m.shift => match shifted_punctuation(vk) {
            Some(p) => Key::Punctuation(p.into()),
            None if (0x41..=0x5A).contains(&vk) => Key::Letter(vk as u8 as char), // starts English
            None => Key::Other,
        },
        _ => match us_char(vk).and_then(dachen::symbol) {
            Some(symbol) => Key::Symbol(symbol),
            None => Key::Other,
        },
    };
    Some(key)
}

/// Whether the key needs the input method when nothing is being composed:
/// a bopomofo symbol (a tone alone is not), full-width punctuation or an
/// uppercase letter (English).
pub fn starts_composition(key: &Key) -> bool {
    match key {
        Key::Symbol(s) => !lightbopomofo_core::bopomofo::is_tone(*s),
        Key::Punctuation(_) | Key::Letter(_) => true,
        _ => false,
    }
}

/// Tracks a lone Shift tap (press and release with nothing in between),
/// which switches between Chinese and English.
#[derive(Default)]
pub struct ShiftTap {
    armed: bool,
}

impl ShiftTap {
    /// Any key down: Shift (or its auto-repeat) arms, anything else disarms.
    pub fn key_down(&mut self, vk: u16) {
        self.armed = matches!(vk, vk::SHIFT | 0xA0 | 0xA1);
    }

    /// Whether releasing `vk` now would complete a tap (does not change state).
    pub fn would_tap(&self, vk: u16) -> bool {
        self.armed && matches!(vk, vk::SHIFT | 0xA0 | 0xA1)
    }

    /// Returns true when this key-up completes a lone Shift tap.
    pub fn key_up(&mut self, vk: u16) -> bool {
        let tap = self.armed && matches!(vk, vk::SHIFT | 0xA0 | 0xA1);
        self.armed = false;
        tap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
        win: false,
    };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..NONE
    };

    #[test]
    fn dachen_by_position() {
        assert_eq!(translate(b'S' as u16, NONE, false), Some(Key::Symbol('ㄋ')));
        assert_eq!(translate(b'1' as u16, NONE, false), Some(Key::Symbol('ㄅ')));
        assert_eq!(translate(b'3' as u16, NONE, false), Some(Key::Symbol('ˇ')));
        assert_eq!(
            translate(vk::OEM_COMMA, NONE, false),
            Some(Key::Symbol('ㄝ'))
        );
        assert_eq!(translate(vk::OEM_2, NONE, false), Some(Key::Symbol('ㄥ')));
        assert_eq!(
            translate(vk::OEM_MINUS, NONE, false),
            Some(Key::Symbol('ㄦ'))
        );
    }

    #[test]
    fn editing_keys() {
        assert_eq!(
            translate(vk::SPACE, NONE, false),
            Some(Key::Symbol(FIRST_TONE))
        );
        assert_eq!(translate(vk::RETURN, NONE, false), Some(Key::Commit));
        assert_eq!(translate(vk::BACK, NONE, false), Some(Key::Backspace));
        assert_eq!(translate(vk::ESCAPE, NONE, false), Some(Key::Escape));
        assert_eq!(translate(vk::RIGHT, NONE, false), Some(Key::Next));
        assert_eq!(translate(vk::TAB, SHIFT, false), Some(Key::Previous));
    }

    #[test]
    fn shift_punctuation_and_letters() {
        assert_eq!(
            translate(vk::OEM_COMMA, SHIFT, false),
            Some(Key::Punctuation("，".into()))
        );
        assert_eq!(
            translate(b'1' as u16, SHIFT, false),
            Some(Key::Punctuation("！".into()))
        );
        assert_eq!(
            translate(vk::OEM_1, SHIFT, false),
            Some(Key::Punctuation("：".into()))
        );
        assert_eq!(translate(b'A' as u16, SHIFT, false), Some(Key::Letter('A')));
        assert_eq!(translate(b'2' as u16, SHIFT, false), Some(Key::Other)); // @
    }

    #[test]
    fn choosing_and_paging() {
        let shift_2 = translate(b'2' as u16, SHIFT, true);
        assert_eq!(shift_2, Some(Key::Select(2)));
        assert_eq!(translate(b'9' as u16, SHIFT, true), Some(Key::Select(9)));
        // Not composing: Shift+1 is still ！, the others go to the app.
        assert_eq!(
            translate(b'1' as u16, SHIFT, false),
            Some(Key::Punctuation("！".into()))
        );
        assert_eq!(translate(b'2' as u16, SHIFT, false), Some(Key::Other));
        assert_eq!(translate(b'0' as u16, SHIFT, true), Some(Key::Other));
        assert_eq!(translate(vk::OEM_4, NONE, true), Some(Key::PagePrevious));
        assert_eq!(translate(vk::OEM_6, NONE, false), Some(Key::PageNext));
        assert_eq!(translate(vk::PRIOR, NONE, true), Some(Key::PagePrevious));
        assert_eq!(translate(vk::NEXT, NONE, true), Some(Key::PageNext));
        assert_eq!(translate(vk::OEM_4, SHIFT, true), Some(Key::Other)); // {
        assert!(!starts_composition(&Key::PageNext));
    }

    #[test]
    fn modifiers_and_keypad() {
        assert_eq!(translate(vk::SHIFT, SHIFT, false), None);
        assert_eq!(translate(vk::CONTROL, NONE, false), None);
        let ctrl = Modifiers { ctrl: true, ..NONE };
        assert_eq!(translate(b'C' as u16, ctrl, false), Some(Key::Other));
        assert_eq!(translate(vk::NUMPAD0 + 5, NONE, false), Some(Key::Other));
    }

    #[test]
    fn what_starts_a_composition() {
        assert!(starts_composition(&Key::Symbol('ㄋ')));
        assert!(!starts_composition(&Key::Symbol('ˇ')));
        assert!(!starts_composition(&Key::Symbol(FIRST_TONE))); // Space
        assert!(starts_composition(&Key::Punctuation("，".into())));
        assert!(starts_composition(&Key::Letter('A')));
        assert!(!starts_composition(&Key::Commit));
        assert!(!starts_composition(&Key::Other));
    }

    #[test]
    fn lone_shift_tap() {
        let mut t = ShiftTap::default();
        t.key_down(vk::SHIFT);
        assert!(t.would_tap(vk::SHIFT));
        assert!(t.key_up(vk::SHIFT));
        assert!(!t.would_tap(vk::SHIFT));
        // Shift used with another key is not a tap.
        t.key_down(vk::SHIFT);
        t.key_down(vk::OEM_COMMA);
        assert!(!t.key_up(vk::SHIFT));
        // Auto-repeat of Shift keeps it armed.
        t.key_down(vk::SHIFT);
        t.key_down(vk::SHIFT);
        assert!(t.key_up(vk::SHIFT));
    }
}
