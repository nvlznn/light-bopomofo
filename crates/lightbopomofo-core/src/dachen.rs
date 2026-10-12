//! 大千（standard）keyboard layout: physical key → bopomofo symbol.

/// Map an unshifted US-layout key character to its bopomofo symbol.
pub fn symbol(key: char) -> Option<char> {
    let s = match key.to_ascii_lowercase() {
        '1' => 'ㄅ',
        'q' => 'ㄆ',
        'a' => 'ㄇ',
        'z' => 'ㄈ',
        '2' => 'ㄉ',
        'w' => 'ㄊ',
        's' => 'ㄋ',
        'x' => 'ㄌ',
        '3' => 'ˇ',
        'e' => 'ㄍ',
        'd' => 'ㄎ',
        'c' => 'ㄏ',
        '4' => 'ˋ',
        'r' => 'ㄐ',
        'f' => 'ㄑ',
        'v' => 'ㄒ',
        '5' => 'ㄓ',
        't' => 'ㄔ',
        'g' => 'ㄕ',
        'b' => 'ㄖ',
        '6' => 'ˊ',
        'y' => 'ㄗ',
        'h' => 'ㄘ',
        'n' => 'ㄙ',
        '7' => '˙',
        'u' => 'ㄧ',
        'j' => 'ㄨ',
        'm' => 'ㄩ',
        '8' => 'ㄚ',
        'i' => 'ㄛ',
        'k' => 'ㄜ',
        ',' => 'ㄝ',
        '9' => 'ㄞ',
        'o' => 'ㄟ',
        'l' => 'ㄠ',
        '.' => 'ㄡ',
        '0' => 'ㄢ',
        'p' => 'ㄣ',
        ';' => 'ㄤ',
        '/' => 'ㄥ',
        '-' => 'ㄦ',
        _ => return None,
    };
    Some(s)
}

/// Convert a string of 大千 keys to bopomofo. Unknown keys are dropped.
pub fn convert(keys: &str) -> String {
    keys.chars().filter_map(symbol).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bopomofo::{kind, Kind};

    #[test]
    fn layout() {
        assert_eq!(symbol('1'), Some('ㄅ'));
        assert_eq!(symbol('q'), Some('ㄆ'));
        assert_eq!(symbol('6'), Some('ˊ'));
        assert_eq!(symbol('3'), Some('ˇ'));
        assert_eq!(symbol('4'), Some('ˋ'));
        assert_eq!(symbol('7'), Some('˙'));
        assert_eq!(symbol('-'), Some('ㄦ'));
        assert_eq!(symbol(' '), None);
        assert_eq!(convert("sca"), "ㄋㄏㄇ");
        assert_eq!(convert("su3cl3"), "ㄋㄧˇㄏㄠˇ");
    }

    #[test]
    fn covers_every_symbol_once() {
        let keys = "1qaz2wsx3edc4rfv5tgb6yhn7ujm8ik,9ol.0p;/-";
        let mut seen: Vec<char> = keys.chars().map(|k| symbol(k).unwrap()).collect();
        assert!(seen.iter().all(|&c| kind(c).is_some()));
        assert_eq!(
            seen.iter()
                .filter(|&&c| kind(c) == Some(Kind::Tone))
                .count(),
            4
        );
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 41);
    }
}
