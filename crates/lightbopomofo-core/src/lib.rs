//! 輕注音 engine: 簡拼 bopomofo input → candidates. No platform APIs here.

pub mod bopomofo;
pub mod composer;
pub mod dachen;
pub mod engine;
pub mod history;
pub mod lexicon;

pub use composer::{Composer, Key, KeyResult};
pub use engine::{Candidate, Engine};
pub use history::History;
pub use lexicon::Lexicon;
