//! User selection history: which text the user chose for which typed
//! symbols — single words as well as whole compositions. Stays on this
//! machine.
//!
//! File: one `unix_seconds<TAB>typed<TAB>text[<TAB>count]` line per entry
//! (count defaults to 1). Choices are appended; forgetting an entry rewrites
//! the file with one line per pair.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
            by_typed: HashMap::new(),
        };
        history.reload();
        history
    }

    /// Re-read the file (another process may have changed it).
    pub fn reload(&mut self) {
        self.by_typed.clear();
        let Some(path) = &self.path else { return };
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        for line in text.lines() {
            let mut fields = line.split('\t');
            let (Some(ts), Some(typed), Some(text)) = (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let count = fields.next().and_then(|c| c.parse().ok()).unwrap_or(1);
            self.bump(typed, text, count, ts.parse().unwrap_or(0));
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
        if let Some(path) = &self.path {
            let lines: String = choices
                .iter()
                .map(|(typed, text)| format!("{now}\t{typed}\t{text}\n"))
                .collect();
            let _ = append(path, lines.as_bytes());
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

    fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let body: String = self
            .entries()
            .iter()
            .rev()
            .map(|e| format!("{}\t{}\t{}\t{}\n", e.last_used, e.typed, e.text, e.count))
            .collect();
        write_atomically(path, body.as_bytes())
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
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
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
