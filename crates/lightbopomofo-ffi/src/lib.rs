//! C ABI for the macOS shell.
//!
//! Rules: every string is UTF-8; strings returned by the engine are freed
//! with `lb_string_free`, never with `free`. No panic crosses this boundary:
//! each function catches unwinds and reports failure as NULL / no-op.
//! An engine is used from one thread only.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use lightbopomofo_core::{dachen, Engine, History, Lexicon};

pub struct LbEngine(Engine);

/// Just the history file, for the settings app (no lexicon loaded).
pub struct LbHistory(History);

/// Run `f`, turning a panic into `fallback`.
fn guard<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}

/// # Safety
/// `p` must be NULL or a NUL-terminated string.
unsafe fn str_arg<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    CStr::from_ptr(p).to_str().ok()
}

/// Create an engine. `history_path` may be NULL (history kept in memory).
/// Returns NULL if the lexicon cannot be read.
///
/// # Safety
/// Arguments must be NULL or NUL-terminated strings.
#[no_mangle]
pub unsafe extern "C" fn lb_engine_new(
    lexicon_path: *const c_char,
    history_path: *const c_char,
) -> *mut LbEngine {
    guard(ptr::null_mut(), || {
        let Some(lexicon_path) = str_arg(lexicon_path) else {
            return ptr::null_mut();
        };
        let Ok(lexicon) = Lexicon::load(lexicon_path) else {
            return ptr::null_mut();
        };
        let history = str_arg(history_path).map_or_else(History::in_memory, History::load);
        Box::into_raw(Box::new(LbEngine(Engine::new(lexicon, history))))
    })
}

/// # Safety
/// `engine` must be NULL or from `lb_engine_new`, and not used afterwards.
#[no_mangle]
pub unsafe extern "C" fn lb_engine_free(engine: *mut LbEngine) {
    if !engine.is_null() {
        guard((), || drop(Box::from_raw(engine)));
    }
}

/// Candidates for `symbols` (bopomofo) as JSON:
/// `[{"text":"你好嗎","consumed":3}, ...]`. NULL on bad arguments.
///
/// # Safety
/// `engine` from `lb_engine_new`; `symbols` NULL or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn lb_query(engine: *mut LbEngine, symbols: *const c_char) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let (Some(engine), Some(symbols)) = (engine.as_ref(), str_arg(symbols)) else {
            return ptr::null_mut();
        };
        let candidates = engine.0.query_str(symbols);
        let mut json = String::from("[");
        for (i, c) in candidates.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str("{\"text\":");
            push_json_string(&mut json, &c.text);
            json.push_str(&format!(",\"consumed\":{}}}", c.consumed));
        }
        json.push(']');
        CString::new(json).map_or(ptr::null_mut(), CString::into_raw)
    })
}

/// Record that the user committed `text` for the start of `symbols`.
///
/// # Safety
/// `engine` from `lb_engine_new`; strings NULL or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn lb_learn(
    engine: *mut LbEngine,
    symbols: *const c_char,
    text: *const c_char,
) {
    guard((), || {
        if let (Some(engine), Some(symbols), Some(text)) =
            (engine.as_mut(), str_arg(symbols), str_arg(text))
        {
            engine.0.learn_str(symbols, text);
        }
    })
}

/// Re-read the history file after another process (the settings app)
/// changed it.
///
/// # Safety
/// `engine` must be NULL or from `lb_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn lb_reload_history(engine: *mut LbEngine) {
    guard((), || {
        if let Some(engine) = engine.as_mut() {
            engine.0.reload_history();
        }
    })
}

/// Open a history file for viewing and editing. NULL on bad arguments.
///
/// # Safety
/// `path` must be NULL or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn lb_history_open(path: *const c_char) -> *mut LbHistory {
    guard(ptr::null_mut(), || match str_arg(path) {
        Some(path) => Box::into_raw(Box::new(LbHistory(History::load(path)))),
        None => ptr::null_mut(),
    })
}

/// # Safety
/// `history` must be NULL or from `lb_history_open`, and not used afterwards.
#[no_mangle]
pub unsafe extern "C" fn lb_history_free(history: *mut LbHistory) {
    if !history.is_null() {
        guard((), || drop(Box::from_raw(history)));
    }
}

/// Re-read the file, then list every entry, most recent first, as JSON:
/// `[{"typed":"ㄋㄏㄇ","text":"你好嗎","count":3,"last_used":1760000000}, ...]`.
///
/// # Safety
/// `history` must be from `lb_history_open`.
#[no_mangle]
pub unsafe extern "C" fn lb_history_json(history: *mut LbHistory) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let Some(history) = history.as_mut() else {
            return ptr::null_mut();
        };
        history.0.reload();
        entries_json(&history.0.entries())
    })
}

/// Learned choices whose text the lexicon lacks, as `lb_history_json`.
///
/// # Safety
/// `engine` from `lb_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn lb_new_words(engine: *mut LbEngine) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let Some(engine) = engine.as_ref() else {
            return ptr::null_mut();
        };
        entries_json(&engine.0.new_words())
    })
}

fn entries_json(entries: &[lightbopomofo_core::history::HistoryEntry]) -> *mut c_char {
    let mut json = String::from("[");
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"typed\":");
        push_json_string(&mut json, &e.typed);
        json.push_str(",\"text\":");
        push_json_string(&mut json, &e.text);
        json.push_str(&format!(
            ",\"count\":{},\"last_used\":{}}}",
            e.count, e.last_used
        ));
    }
    json.push(']');
    CString::new(json).map_or(ptr::null_mut(), CString::into_raw)
}

/// Forget one entry. Returns 1 if removed, 0 if not found, -1 on error.
///
/// # Safety
/// `history` from `lb_history_open`; strings NULL or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn lb_history_forget(
    history: *mut LbHistory,
    typed: *const c_char,
    text: *const c_char,
) -> i32 {
    guard(-1, || {
        let (Some(history), Some(typed), Some(text)) =
            (history.as_mut(), str_arg(typed), str_arg(text))
        else {
            return -1;
        };
        history.0.reload();
        match history.0.forget(typed, text) {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(_) => -1,
        }
    })
}

/// Forget everything. Returns 0 on success, -1 on error.
///
/// # Safety
/// `history` must be from `lb_history_open`.
#[no_mangle]
pub unsafe extern "C" fn lb_history_clear(history: *mut LbHistory) -> i32 {
    guard(-1, || match history.as_mut().map(|h| h.0.clear()) {
        Some(Ok(())) => 0,
        _ => -1,
    })
}

/// 大千 key (unshifted US-layout character) → bopomofo code point, or 0.
#[no_mangle]
pub extern "C" fn lb_dachen(key: u32) -> u32 {
    char::from_u32(key)
        .and_then(dachen::symbol)
        .map_or(0, |c| c as u32)
}

/// Free a string returned by the engine. NULL is ignored.
///
/// # Safety
/// `s` must be NULL or from this library, freed once.
#[no_mangle]
pub unsafe extern "C" fn lb_string_free(s: *mut c_char) {
    if !s.is_null() {
        guard((), || drop(CString::from_raw(s)));
    }
}

fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = "你好嗎,75,ㄋㄧˇ ㄏㄠˇ ㄇㄚ\n你,89941,ㄋㄧˇ\n";

    /// A lexicon file of its own for each test (tests run in parallel).
    fn lexicon_file() -> CString {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("lb-ffi-{}-{n}.csv", std::process::id()));
        std::fs::write(&path, MINI).unwrap();
        CString::new(path.to_str().unwrap()).unwrap()
    }

    fn take(s: *mut c_char) -> String {
        assert!(!s.is_null());
        let out = unsafe { CStr::from_ptr(s) }.to_str().unwrap().to_string();
        unsafe { lb_string_free(s) };
        out
    }

    #[test]
    fn query_returns_json() {
        let lex = lexicon_file();
        unsafe {
            let e = lb_engine_new(lex.as_ptr(), ptr::null());
            assert!(!e.is_null());
            let input = CString::new("ㄋㄏㄇ").unwrap();
            let json = take(lb_query(e, input.as_ptr()));
            assert!(
                json.starts_with("[{\"text\":\"你好嗎\",\"consumed\":3}"),
                "{json}"
            );
            let text = CString::new("你好嗎").unwrap();
            lb_learn(e, input.as_ptr(), text.as_ptr());
            lb_engine_free(e);
        }
    }

    #[test]
    fn bad_arguments_do_not_crash() {
        unsafe {
            assert!(lb_engine_new(ptr::null(), ptr::null()).is_null());
            let missing = CString::new("/nonexistent/lexicon.csv").unwrap();
            assert!(lb_engine_new(missing.as_ptr(), ptr::null()).is_null());
            assert!(lb_query(ptr::null_mut(), ptr::null()).is_null());
            lb_learn(ptr::null_mut(), ptr::null(), ptr::null());
            lb_engine_free(ptr::null_mut());
            lb_string_free(ptr::null_mut());

            let lex = lexicon_file();
            let e = lb_engine_new(lex.as_ptr(), ptr::null());
            let invalid_utf8 = [0xffu8, 0xfe, 0];
            assert!(lb_query(e, invalid_utf8.as_ptr() as *const c_char).is_null());
            assert_eq!(take(lb_query(e, c"".as_ptr())), "[]");
            lb_engine_free(e);
        }
    }

    #[test]
    fn repeated_create_and_free() {
        let lex = lexicon_file();
        for _ in 0..200 {
            unsafe {
                let e = lb_engine_new(lex.as_ptr(), ptr::null());
                let s = lb_query(e, c"ㄋ".as_ptr());
                lb_string_free(s);
                lb_engine_free(e);
            }
        }
    }

    #[test]
    fn dachen_keys() {
        assert_eq!(lb_dachen('1' as u32), 'ㄅ' as u32);
        assert_eq!(lb_dachen('3' as u32), 'ˇ' as u32);
        assert_eq!(lb_dachen(' ' as u32), 0);
        assert_eq!(lb_dachen(0xD800), 0);
    }

    #[test]
    fn history_handle_lists_forgets_and_clears() {
        let dir = std::env::temp_dir().join(format!("lb-ffi-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = CString::new(dir.join("history.tsv").to_str().unwrap()).unwrap();
        let lex = lexicon_file();
        unsafe {
            // The input method learns; the settings app sees and edits it.
            let e = lb_engine_new(lex.as_ptr(), path.as_ptr());
            lb_learn(e, c"ㄋㄏㄇ".as_ptr(), c"你好嗎".as_ptr());
            let h = lb_history_open(path.as_ptr());
            let json = take(lb_history_json(h));
            assert!(
                json.contains("\"typed\":\"ㄋㄏㄇ\",\"text\":\"你好嗎\",\"count\":1"),
                "{json}"
            );
            assert_eq!(
                lb_history_forget(h, c"ㄋㄏㄇ".as_ptr(), c"你好嗎".as_ptr()),
                1
            );
            assert_eq!(
                lb_history_forget(h, c"ㄋㄏㄇ".as_ptr(), c"你好嗎".as_ptr()),
                0
            );
            assert_eq!(take(lb_history_json(h)), "[]");

            // The engine forgets after reloading.
            lb_reload_history(e);
            lb_learn(e, c"ㄋ".as_ptr(), c"你".as_ptr());
            assert_eq!(lb_history_clear(h), 0);
            assert_eq!(take(lb_history_json(h)), "[]");

            assert!(lb_history_open(ptr::null()).is_null());
            assert!(lb_history_json(ptr::null_mut()).is_null());
            assert_eq!(
                lb_history_forget(ptr::null_mut(), ptr::null(), ptr::null()),
                -1
            );
            assert_eq!(lb_history_clear(ptr::null_mut()), -1);
            lb_reload_history(ptr::null_mut());
            lb_history_free(ptr::null_mut());
            lb_history_free(h);
            lb_engine_free(e);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_escaping() {
        let mut s = String::new();
        push_json_string(&mut s, "a\"b\\c\n");
        assert_eq!(s, "\"a\\\"b\\\\c\\u000a\"");
    }
}
