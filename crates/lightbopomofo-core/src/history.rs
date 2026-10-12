//! User selection history: which text the user chose for which typed
//! symbols — single words as well as whole compositions. Append-only file,
//! one `unix_seconds<TAB>typed<TAB>text` per line, aggregated into counts at
//! load time. Stays on this machine.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct History {
    path: Option<PathBuf>,
    /// typed symbols → [(text, count)]
    by_typed: HashMap<String, Vec<(String, u32)>>,
}

impl History {
    /// In-memory only; nothing is written.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Load from `path`. A missing or unreadable file starts empty.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut history = History {
            path: Some(path.clone()),
            by_typed: HashMap::new(),
        };
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                let mut fields = line.split('\t');
                if let (Some(_), Some(typed), Some(text)) =
                    (fields.next(), fields.next(), fields.next())
                {
                    history.bump(typed, text);
                }
            }
        }
        history
    }

    pub fn is_empty(&self) -> bool {
        self.by_typed.is_empty()
    }

    /// Texts chosen for exactly `typed`, with counts.
    pub fn learned(&self, typed: &str) -> &[(String, u32)] {
        self.by_typed.get(typed).map_or(&[], Vec::as_slice)
    }

    pub fn count(&self, typed: &str, text: &str) -> u32 {
        self.learned(typed)
            .iter()
            .find(|(t, _)| t == text)
            .map_or(0, |&(_, n)| n)
    }

    /// Record (typed, text) pairs the user committed. Write failures are
    /// ignored so typing never breaks because of the history file.
    pub fn record(&mut self, choices: &[(&str, &str)]) {
        let choices: Vec<(&str, &str)> = choices
            .iter()
            .copied()
            .filter(|(t, w)| valid(t) && valid(w))
            .collect();
        for &(typed, text) in &choices {
            self.bump(typed, text);
        }
        if let Some(path) = &self.path {
            let _ = append(path, &choices);
        }
    }

    fn bump(&mut self, typed: &str, text: &str) {
        if !valid(typed) || !valid(text) {
            return;
        }
        let list = self.by_typed.entry(typed.to_string()).or_default();
        match list.iter_mut().find(|(t, _)| t == text) {
            Some((_, n)) => *n += 1,
            None => list.push((text.to_string(), 1)),
        }
    }
}

fn valid(field: &str) -> bool {
    !field.is_empty() && !field.contains(['\t', '\n'])
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
    for (typed, text) in choices {
        buf.push_str(&format!("{now}\t{typed}\t{text}\n"));
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
        assert_eq!(reloaded.learned("ㄍㄗ"), [("工作".to_string(), 2)]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
