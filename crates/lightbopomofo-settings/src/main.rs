//! 輕注音 for Windows — the app (LightBopomofo.exe).
//!
//!   LightBopomofo.exe            status, key guide and the history manager
//!   LightBopomofo.exe --enable   add 輕注音 to the user's input methods (installer)
//!   LightBopomofo.exe --disable  remove it (uninstaller)
//!   LightBopomofo.exe --status   exit code 0 ready, 1 not in the list, 2 not installed
//!   LightBopomofo.exe --check-update [--now]
//!                                notify of a newer release (scheduled task)
//!   LightBopomofo.exe lightbopomofo:update
//!                                download and install it (the notification)
//!
//! History edits take effect in every application right away: the input
//! method re-reads the history file when it changes.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod update;

#[cfg(windows)]
fn main() {
    app::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("LightBopomofo.exe is the Windows app; on macOS use 輕注音.app.");
}
