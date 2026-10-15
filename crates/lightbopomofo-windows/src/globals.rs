//! Process-wide state of the DLL: its module handle, identifiers, the shared
//! engine, file locations and the panic guard every COM entry point uses.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use lightbopomofo_core::{Engine, History, Lexicon};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_core::GUID;

use crate::log;

/// COM class of the text service.
pub const CLSID_TEXT_SERVICE: GUID = GUID::from_u128(0x0d5e0d6e_3056_4888_9151_8701b588ad0c);
/// The TSF language profile (one per language; ours is zh-TW).
pub const GUID_PROFILE: GUID = GUID::from_u128(0x31638591_41d6_4dc9_b7d8_c2f29953f24b);
/// Display attribute of text being composed (underlined).
pub const GUID_DISPLAY_ATTRIBUTE_INPUT: GUID =
    GUID::from_u128(0xc1d7017d_5769_45f6_9e99_065c4369956f);
/// Traditional Chinese (Taiwan).
pub const LANGID_ZH_TW: u16 = 0x0404;
pub const DISPLAY_NAME: &str = "輕注音";

static MODULE: AtomicIsize = AtomicIsize::new(0);
/// Live COM objects plus server locks; the DLL may unload at zero.
static REFS: AtomicUsize = AtomicUsize::new(0);

pub fn set_module(module: HMODULE) {
    MODULE.store(module.0 as isize, Ordering::Relaxed);
}

pub fn module() -> HMODULE {
    HMODULE(MODULE.load(Ordering::Relaxed) as *mut _)
}

pub fn add_ref() {
    REFS.fetch_add(1, Ordering::SeqCst);
}

pub fn release() {
    REFS.fetch_sub(1, Ordering::SeqCst);
}

pub fn can_unload() -> bool {
    REFS.load(Ordering::SeqCst) == 0
}

/// Full path of this DLL.
pub fn dll_path() -> Option<PathBuf> {
    let mut buf = vec![0u16; 1024];
    let len = unsafe { GetModuleFileNameW(Some(module()), &mut buf) } as usize;
    (len > 0 && len < buf.len()).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len])))
}

/// The install folder: DLLs live in per-architecture subfolders
/// (`x64\`, `x86\`, `arm64\`), shared data one level up.
pub fn install_dir() -> Option<PathBuf> {
    let dll_dir = dll_path()?.parent()?.to_path_buf();
    if dll_dir.join("lexicon.bin").exists() {
        return Some(dll_dir);
    }
    dll_dir.parent().map(|p| p.to_path_buf())
}

/// `%APPDATA%\LightBopomofo` (roaming: history follows the user).
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("LightBopomofo"))
}

/// The engine, shared by every input context in this process; loaded on
/// first use. `None` when the lexicon is missing or broken, in which case
/// the input method stays out of the way.
pub fn engine() -> Option<MutexGuard<'static, Engine>> {
    static ENGINE: OnceLock<Option<Mutex<Engine>>> = OnceLock::new();
    let engine = ENGINE.get_or_init(|| {
        let lexicon_path = install_dir()?.join("lexicon.bin");
        let lexicon = match Lexicon::load(&lexicon_path) {
            Ok(lexicon) => lexicon,
            Err(e) => {
                log::error(&format!("cannot load {}: {e}", lexicon_path.display()));
                return None;
            }
        };
        let history = data_dir()
            .map(|d| History::load(d.join("history.tsv")))
            .unwrap_or_else(History::in_memory);
        log::info(&format!("engine loaded ({} entries)", lexicon.len()));
        Some(Mutex::new(Engine::new(lexicon, history)))
    });
    // A panic while holding the lock poisons it; the engine itself is still
    // consistent enough to keep typing.
    engine
        .as_ref()
        .map(|m| m.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
}

/// Run `f`, turning a panic into `fallback` and a log line. Every method
/// Windows calls goes through this: a panic must never unwind into the
/// application that loaded us.
pub fn guard<T>(what: &str, fallback: T, f: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            log::error(&format!("panic in {what}: {message}"));
            fallback
        }
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

pub fn wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
