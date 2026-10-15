//! User selection history: which text the user chose for which typed
//! symbols — single words as well as whole compositions. Stays on this
//! machine.
//!
//! File: one `unix_seconds<TAB>typed<TAB>text[<TAB>count]` line per entry
//! (count defaults to 1). Choices are appended; forgetting an entry rewrites
//! the file with one line per pair, as does compaction once appends pile up.
//!
//! Several processes may share the file (on Windows every application loads
//! the input method): each appends its own choices and re-reads the file when
//! its modification time or size changes, see [`History::reload_if_changed`].

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Rewrite the file once it has this many more lines than distinct entries.
const COMPACT_SLACK: usize = 2000;

/// What a file looked like when last read: modification time and size.
type Signature = (SystemTime, u64);

fn signature(path: &Path) -> Option<Signature> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Learned {
    pub text: String,
    pub count: u32,
    /// Unix seconds of the most recent choice.
    pub last_used: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub typed: String,
    pub text: String,
    pub count: u32,
    pub last_used: u64,
}

#[derive(Default)]
pub struct History {
    path: Option<PathBuf>,
    by_typed: HashMap<String, Vec<Learned>>,
    seen: Option<Signature>,
}

impl History {
    /// In-memory only; nothing is written.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Load from `path`. A missing or unreadable file starts empty.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let mut history = History {
            path: Some(path.as_ref().to_path_buf()),
            ..Self::default()
        };
        history.reload();
        history
    }

    /// Re-read the file if another process changed it since it was last
    /// read. Cheap when nothing changed (one metadata call). Returns whether
    /// it re-read.
    pub fn reload_if_changed(&mut self) -> bool {
        let Some(path) = &self.path else { return false };
        if signature(path) == self.seen {
            return false;
        }
        self.reload();
        true
    }

    /// Re-read the file (another process may have changed it).
    pub fn reload(&mut self) {
        self.by_typed.clear();
        let Some(path) = self.path.clone() else {
            return;
        };
        // Taken before reading: a write during the read shows up as a change.
        self.seen = signature(&path);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let mut lines = 0;
        for line in text.lines() {
            lines += 1;
            let mut fields = line.split('\t');
            let (Some(ts), Some(typed), Some(text)) = (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let count = fields.next().and_then(|c| c.parse().ok()).unwrap_or(1);
            self.bump(typed, text, count, ts.parse().unwrap_or(0));
        }
        let distinct: usize = self.by_typed.values().map(Vec::len).sum();
        if lines > distinct + COMPACT_SLACK {
            // Best effort: another process may hold the file just now.
            let _ = self.save();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_typed.is_empty()
    }

    /// Texts chosen for exactly `typed`.
    pub fn learned(&self, typed: &str) -> &[Learned] {
        self.by_typed.get(typed).map_or(&[], Vec::as_slice)
    }

    pub fn count(&self, typed: &str, text: &str) -> u32 {
        self.learned(typed)
            .iter()
            .find(|l| l.text == text)
            .map_or(0, |l| l.count)
    }

    /// Every entry, most recently used first.
    pub fn entries(&self) -> Vec<HistoryEntry> {
        let mut out: Vec<HistoryEntry> = self
            .by_typed
            .iter()
            .flat_map(|(typed, list)| {
                list.iter().map(move |l| HistoryEntry {
                    typed: typed.clone(),
                    text: l.text.clone(),
                    count: l.count,
                    last_used: l.last_used,
                })
            })
            .collect();
        out.sort_by(|a, b| b.last_used.cmp(&a.last_used).then(a.typed.cmp(&b.typed)));
        out
    }

    /// Record (typed, text) pairs the user committed. Write failures are
    /// ignored so typing never breaks because of the history file.
    pub fn record(&mut self, choices: &[(&str, &str)]) {
        let choices: Vec<(&str, &str)> = choices
            .iter()
            .copied()
            .filter(|(t, w)| valid(t) && valid(w))
            .collect();
        let now = now();
        for &(typed, text) in &choices {
            self.bump(typed, text, 1, now);
        }
        if let Some(path) = self.path.clone() {
            let lines: String = choices
                .iter()
                .map(|(typed, text)| format!("{now}\t{typed}\t{text}\n"))
                .collect();
            let before = signature(&path);
            if append(&path, lines.as_bytes()).is_ok() && before == self.seen {
                // Nobody else wrote since we last read: the file now holds
                // exactly what we have in memory, so no need to re-read it.
                let after = signature(&path);
                let expected = before.map_or(0, |(_, len)| len) + lines.len() as u64;
                if after.is_some_and(|(_, len)| len == expected) {
                    self.seen = after;
                }
            }
        }
    }

    /// Forget one entry; returns whether it existed.
    pub fn forget(&mut self, typed: &str, text: &str) -> io::Result<bool> {
        let Some(list) = self.by_typed.get_mut(typed) else {
            return Ok(false);
        };
        let before = list.len();
        list.retain(|l| l.text != text);
        if list.len() == before {
            return Ok(false);
        }
        if list.is_empty() {
            self.by_typed.remove(typed);
        }
        self.save()?;
        Ok(true)
    }

    /// Forget everything.
    pub fn clear(&mut self) -> io::Result<()> {
        self.by_typed.clear();
        self.save()
    }

    fn save(&mut self) -> io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let body: String = self
            .entries()
            .iter()
            .rev()
            .map(|e| format!("{}\t{}\t{}\t{}\n", e.last_used, e.typed, e.text, e.count))
            .collect();
        write_atomically(&path, body.as_bytes())?;
        self.seen = signature(&path);
        Ok(())
    }

    fn bump(&mut self, typed: &str, text: &str, count: u32, when: u64) {
        if !valid(typed) || !valid(text) {
            return;
        }
        let list = self.by_typed.entry(typed.to_string()).or_default();
        match list.iter_mut().find(|l| l.text == text) {
            Some(l) => {
                l.count += count;
                l.last_used = l.last_used.max(when);
            }
            None => list.push(Learned {
                text: text.to_string(),
                count,
                last_used: when,
            }),
        }
    }
}

fn valid(field: &str) -> bool {
    !field.is_empty() && !field.contains(['\t', '\n', '\r'])
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn append(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(bytes)
}

/// Write via a temporary file and rename, so readers never see half a file.
/// On Windows the rename fails while another program holds the file without
/// delete sharing (a virus scanner, say), so retry briefly.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    let mut attempt = 0;
    loop {
        match std::fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(_) if attempt < 10 => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("lb-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (dir.clone(), dir.join("sub/history.tsv"))
    }

    #[test]
    fn persists_counts() {
        let (dir, path) = temp_path("persist");
        let mut h = History::load(&path);
        assert_eq!(h.count("ㄍㄗ", "工作"), 0);
        h.record(&[("ㄍㄗ", "工作"), ("ㄈㄈ", "方法")]);
        h.record(&[("ㄍㄗ", "工作"), ("ㄍㄨㄥㄗ", "工作"), ("bad\tfield", "x")]);
        assert_eq!(h.count("ㄍㄗ", "工作"), 2);
        let reloaded = History::load(&path);
        assert_eq!(reloaded.count("ㄍㄗ", "工作"), 2);
        assert_eq!(reloaded.count("ㄍㄨㄥㄗ", "工作"), 1);
        assert_eq!(reloaded.count("ㄈㄈ", "方法"), 1);
        assert_eq!(reloaded.count("ㄍㄗ", "方法"), 0);
        assert_eq!(reloaded.learned("ㄍㄗ").len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_old_format_and_counts() {
        let (dir, path) = temp_path("format");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "100\tㄍㄗ\t工作\n200\tㄍㄗ\t工作\n300\tㄈㄈ\t方法\t5\n",
        )
        .unwrap();
        let h = History::load(&path);
        assert_eq!(h.count("ㄍㄗ", "工作"), 2);
        assert_eq!(h.count("ㄈㄈ", "方法"), 5);
        let entries = h.entries();
        assert_eq!(entries[0].text, "方法"); // most recent first
        assert_eq!(entries[1].last_used, 200);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn forget_and_clear_rewrite_the_file() {
        let (dir, path) = temp_path("forget");
        let mut h = History::load(&path);
        h.record(&[("ㄍㄗ", "工作"), ("ㄍㄗ", "各自"), ("ㄈㄈ", "方法")]);
        h.record(&[("ㄍㄗ", "工作")]);
        assert!(h.forget("ㄍㄗ", "各自").unwrap());
        assert!(!h.forget("ㄍㄗ", "各自").unwrap());
        let reloaded = History::load(&path);
        assert_eq!(reloaded.count("ㄍㄗ", "工作"), 2); // counts survive the rewrite
        assert_eq!(reloaded.count("ㄍㄗ", "各自"), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
        h.clear().unwrap();
        assert!(History::load(&path).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_if_changed_follows_other_processes() {
        let (dir, path) = temp_path("shared");
        let mut a = History::load(&path);
        let mut b = History::load(&path);
        assert!(!a.reload_if_changed());

        a.record(&[("ㄋㄏㄇ", "你好嗎")]);
        assert!(!a.reload_if_changed(), "own write needs no re-read");
        assert!(b.reload_if_changed());
        assert_eq!(b.count("ㄋㄏㄇ", "你好嗎"), 1);

        b.record(&[("ㄋㄏㄇ", "你好嗎")]);
        assert!(a.reload_if_changed());
        assert_eq!(a.count("ㄋㄏㄇ", "你好嗎"), 2);

        assert!(b.forget("ㄋㄏㄇ", "你好嗎").unwrap());
        assert!(a.reload_if_changed());
        assert_eq!(a.count("ㄋㄏㄇ", "你好嗎"), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compacts_when_appends_pile_up() {
        let (dir, path) = temp_path("compact");
        let mut h = History::load(&path);
        for _ in 0..COMPACT_SLACK + 10 {
            h.record(&[("ㄋ", "你")]);
        }
        let reloaded = History::load(&path); // triggers compaction
        assert_eq!(reloaded.count("ㄋ", "你"), (COMPACT_SLACK + 10) as u32);
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_sees_other_writers() {
        let (dir, path) = temp_path("reload");
        let mut a = History::load(&path);
        let mut b = History::load(&path);
        b.record(&[("ㄋㄏㄇ", "你好嗎")]);
        assert_eq!(a.count("ㄋㄏㄇ", "你好嗎"), 0);
        a.reload();
        assert_eq!(a.count("ㄋㄏㄇ", "你好嗎"), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
