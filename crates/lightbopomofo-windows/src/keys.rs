//! Windows virtual-key codes → platform-independent [`Key`]s.
//!
//! Pure functions with no Windows API, so they are unit-tested everywhere.
//! 大千 uses physical positions, so keys are read as virtual-key codes (the
//! US-layout key at that position), not as characters of the active layout.

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
/// affect the composition at all.
pub fn translate(vk: u16, m: Modifiers) -> Option<Key> {
    if is_modifier(vk) {
        return None;
    }
    if m.ctrl || m.alt || m.win {
        return Some(Key::Other); // shortcuts: commit, then the app's
    }
    let key = match vk {
        vk::SPACE | vk::RETURN => Key::Commit,
        vk::BACK => Key::Backspace,
        vk::ESCAPE => Key::Escape,
        vk::LEFT | vk::UP => Key::Previous,
        vk::RIGHT | vk::DOWN => Key::Next,
        vk::TAB if m.shift => Key::Previous,
        vk::TAB => Key::Next,
        vk::NUMPAD0..=vk::DIVIDE => Key::Other, // keypad numbers stay numbers
        _ if m.shift => match shifted_punctuation(vk) {
            Some(p) => Key::Punctuation(p.into()),
            None => Key::Other, // Shift+letter: an uppercase letter
        },
        _ => match us_char(vk).and_then(dachen::symbol) {
            Some(symbol) => Key::Symbol(symbol),
            None => Key::Other,
        },
    };
    Some(key)
}

/// Whether the key needs the input method when nothing is being composed:
/// a bopomofo symbol (a tone alone is not) or full-width punctuation.
pub fn starts_composition(key: &Key) -> bool {
    match key {
        Key::Symbol(s) => !lightbopomofo_core::bopomofo::is_tone(*s),
        Key::Punctuation(_) => true,
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
        assert_eq!(translate(b'S' as u16, NONE), Some(Key::Symbol('ㄋ')));
        assert_eq!(translate(b'1' as u16, NONE), Some(Key::Symbol('ㄅ')));
        assert_eq!(translate(b'3' as u16, NONE), Some(Key::Symbol('ˇ')));
        assert_eq!(translate(vk::OEM_COMMA, NONE), Some(Key::Symbol('ㄝ')));
        assert_eq!(translate(vk::OEM_2, NONE), Some(Key::Symbol('ㄥ')));
        assert_eq!(translate(vk::OEM_MINUS, NONE), Some(Key::Symbol('ㄦ')));
    }

    #[test]
    fn editing_keys() {
        assert_eq!(translate(vk::SPACE, NONE), Some(Key::Commit));
        assert_eq!(translate(vk::RETURN, NONE), Some(Key::Commit));
        assert_eq!(translate(vk::BACK, NONE), Some(Key::Backspace));
        assert_eq!(translate(vk::ESCAPE, NONE), Some(Key::Escape));
        assert_eq!(translate(vk::RIGHT, NONE), Some(Key::Next));
        assert_eq!(translate(vk::TAB, SHIFT), Some(Key::Previous));
    }

    #[test]
    fn shift_punctuation_and_letters() {
        assert_eq!(
            translate(vk::OEM_COMMA, SHIFT),
            Some(Key::Punctuation("，".into()))
        );
        assert_eq!(
            translate(b'1' as u16, SHIFT),
            Some(Key::Punctuation("！".into()))
        );
        assert_eq!(
            translate(vk::OEM_1, SHIFT),
            Some(Key::Punctuation("：".into()))
        );
        assert_eq!(translate(b'A' as u16, SHIFT), Some(Key::Other));
    }

    #[test]
    fn modifiers_and_keypad() {
        assert_eq!(translate(vk::SHIFT, SHIFT), None);
        assert_eq!(translate(vk::CONTROL, NONE), None);
        let ctrl = Modifiers { ctrl: true, ..NONE };
        assert_eq!(translate(b'C' as u16, ctrl), Some(Key::Other));
        assert_eq!(translate(vk::NUMPAD0 + 5, NONE), Some(Key::Other));
    }

    #[test]
    fn what_starts_a_composition() {
        assert!(starts_composition(&Key::Symbol('ㄋ')));
        assert!(!starts_composition(&Key::Symbol('ˇ')));
        assert!(starts_composition(&Key::Punctuation("，".into())));
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
