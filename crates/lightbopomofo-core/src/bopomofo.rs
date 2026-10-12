//! Bopomofo symbols and their categories.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 聲母 ㄅ–ㄙ
    Initial,
    /// 介音 ㄧㄨㄩ
    Medial,
    /// 韻母 ㄚ–ㄦ
    Final,
    /// 聲調 ˊˇˋ˙ (first tone has no mark)
    Tone,
}

pub const INITIALS: &str = "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ";
pub const MEDIALS: &str = "ㄧㄨㄩ";
pub const FINALS: &str = "ㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ";
pub const TONES: &str = "ˊˇˋ˙";

pub fn kind(c: char) -> Option<Kind> {
    if INITIALS.contains(c) {
        Some(Kind::Initial)
    } else if MEDIALS.contains(c) {
        Some(Kind::Medial)
    } else if FINALS.contains(c) {
        Some(Kind::Final)
    } else if TONES.contains(c) {
        Some(Kind::Tone)
    } else {
        None
    }
}

pub fn is_tone(c: char) -> bool {
    kind(c) == Some(Kind::Tone)
}

/// Any bopomofo symbol or tone mark.
pub fn is_symbol(c: char) -> bool {
    kind(c).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories() {
        assert_eq!(kind('ㄅ'), Some(Kind::Initial));
        assert_eq!(kind('ㄙ'), Some(Kind::Initial));
        assert_eq!(kind('ㄩ'), Some(Kind::Medial));
        assert_eq!(kind('ㄦ'), Some(Kind::Final));
        assert_eq!(kind('˙'), Some(Kind::Tone));
        assert_eq!(kind('a'), None);
        assert_eq!(INITIALS.chars().count(), 21);
        assert_eq!(FINALS.chars().count(), 13);
    }
}
