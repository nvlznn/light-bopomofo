//! 輕注音 for Windows — the app (LightBopomofo.exe).
//!
//!   LightBopomofo.exe            status, key guide and the history manager
//!   LightBopomofo.exe --enable   add 輕注音 to the user's input methods (installer)
//!   LightBopomofo.exe --disable  remove it (uninstaller)
//!
//! History edits take effect in every application right away: the input
//! method re-reads the history file when it changes.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() {
    app::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("LightBopomofo.exe is the Windows app; on macOS use 輕注音.app.");
}
