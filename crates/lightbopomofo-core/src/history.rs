//! User selection history: which word the user chose for which typed
//! symbols. Append-only file, one `unix_seconds<TAB>typed<TAB>word` per line,
//! aggregated into counts at load time. Stays on this machine.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct History {
    path: Option<PathBuf>,
    /// word → [(typed symbols, count)]
    counts: HashMap<String, Vec<(String, u32)>>,
}

impl History {
    /// In-memory only; nothing is written.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Load from `path`. A missing or unreadable file starts empty.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut counts = HashMap::new();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                let mut fields = line.split('\t');
                if let (Some(_), Some(typed), Some(word)) =
                    (fields.next(), fields.next(), fields.next())
                {
                    bump(&mut counts, typed, word);
                }
            }
        }
        History {
            path: Some(path),
            counts,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Times the user chose `word` after typing exactly `typed`.
    pub fn count(&self, typed: &[char], word: &str) -> u32 {
        let Some(list) = self.counts.get(word) else {
            return 0;
        };
        list.iter()
            .find(|(t, _)| t.chars().eq(typed.iter().copied()))
            .map_or(0, |&(_, n)| n)
    }

    /// Record (typed, word) pairs the user committed. Write failures are
    /// ignored so typing never breaks because of the history file.
    pub fn record(&mut self, choices: &[(&str, &str)]) {
        let choices: Vec<(&str, &str)> = choices
            .iter()
            .copied()
            .filter(|(t, w)| valid(t) && valid(w))
            .collect();
        for &(typed, word) in &choices {
            bump(&mut self.counts, typed, word);
        }
        if let Some(path) = &self.path {
            let _ = append(path, &choices);
        }
    }
}

fn valid(field: &str) -> bool {
    !field.is_empty() && !field.contains(['\t', '\n'])
}

fn bump(counts: &mut HashMap<String, Vec<(String, u32)>>, typed: &str, word: &str) {
    if !valid(typed) || !valid(word) {
        return;
    }
    let list = counts.entry(word.to_string()).or_default();
    match list.iter_mut().find(|(t, _)| t == typed) {
        Some((_, n)) => *n += 1,
        None => list.push((typed.to_string(), 1)),
    }
}

fn append(path: &Path, choices: &[(&str, &str)]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut buf = String::new();
    for (typed, word) in choices {
        buf.push_str(&format!("{now}\t{typed}\t{word}\n"));
    }
    file.write_all(buf.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_counts() {
        let dir = std::env::temp_dir().join(format!("lb-history-{}", std::process::id()));
        let path = dir.join("sub/history.tsv");
        let _ = std::fs::remove_dir_all(&dir);
        let gz: Vec<char> = "ㄍㄗ".chars().collect();
        let mut h = History::load(&path);
        assert_eq!(h.count(&gz, "工作"), 0);
        h.record(&[("ㄍㄗ", "工作"), ("ㄈㄈ", "方法")]);
        h.record(&[("ㄍㄗ", "工作"), ("ㄍㄨㄥㄗ", "工作")]);
        assert_eq!(h.count(&gz, "工作"), 2);
        let reloaded = History::load(&path);
        assert_eq!(reloaded.count(&gz, "工作"), 2);
        assert_eq!(
            reloaded.count(&"ㄍㄨㄥㄗ".chars().collect::<Vec<_>>(), "工作"),
            1
        );
        assert_eq!(
            reloaded.count(&"ㄈㄈ".chars().collect::<Vec<_>>(), "方法"),
            1
        );
        assert_eq!(reloaded.count(&gz, "方法"), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
