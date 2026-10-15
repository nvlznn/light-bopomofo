//! A small log at `%LOCALAPPDATA%\LightBopomofo\logs\ime.log`, for sending
//! along with bug reports. Events and errors only — never what was typed.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 1 << 20;

fn path() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("LightBopomofo\\logs");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("ime.log"))
}

fn write(level: &str, message: &str) {
    let Some(path) = path() else { return };
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(&path, path.with_extension("old.log"));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let line = format!("{now} {level} [{} {exe}] {message}\n", std::process::id());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Key-level tracing, only when the LIGHTBOPOMOFO_DEBUG environment
/// variable is set (tests); never on users' machines by default.
pub fn debug(message: impl FnOnce() -> String) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("LIGHTBOPOMOFO_DEBUG").is_some()) {
        write("DEBUG", &message());
    }
}

pub fn info(message: &str) {
    write("INFO", message);
}

pub fn error(message: &str) {
    write("ERROR", message);
}
