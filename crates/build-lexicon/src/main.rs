//! build-lexicon: compile chewing's tsi.csv into the binary image the
//! engine memory-maps (see lightbopomofo_core::lexicon), with the word pairs
//! of crates/lightbopomofo-core/word-pairs.tsv.
//!
//!   build-lexicon data/tsi.csv path/to/lexicon.bin [WORD_PAIRS]
//!
//! WORD_PAIRS overrides the pairs file; an empty one compiles without pairs.

use std::time::Instant;

use lightbopomofo_core::lexicon::{compile_with_pairs, Lexicon};

/// The word pairs in the repository, next to the lexicon adjustments.
const WORD_PAIRS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../lightbopomofo-core/word-pairs.tsv"
);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (input, output, pairs_path) = match args.as_slice() {
        [input, output] => (input, output, WORD_PAIRS),
        [input, output, pairs] => (input, output, pairs.as_str()),
        _ => {
            eprintln!("usage: build-lexicon TSI_CSV OUTPUT_BIN [WORD_PAIRS]");
            std::process::exit(2);
        }
    };
    let read = |path: &str| {
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("cannot read {path}: {e}");
            std::process::exit(1);
        })
    };
    let source = read(input);
    let pairs = if pairs_path.is_empty() {
        String::new()
    } else {
        read(pairs_path)
    };
    let t = Instant::now();
    let image = compile_with_pairs(&source, &pairs);
    let compiled = t.elapsed();
    if let Some(dir) = std::path::Path::new(output).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(output, &image).unwrap_or_else(|e| {
        eprintln!("cannot write {output}: {e}");
        std::process::exit(1);
    });
    // Open what was written, so a bad image never ships.
    let t = Instant::now();
    let lexicon = Lexicon::load(output).unwrap_or_else(|e| {
        eprintln!("written image does not open: {e}");
        std::process::exit(1);
    });
    eprintln!(
        "{output}: {} entries, {} KB{}; compiled in {compiled:.0?}, opens in {:.1?}",
        lexicon.len(),
        image.len() / 1024,
        if lexicon.has_pairs() {
            ", with word pairs"
        } else {
            ""
        },
        t.elapsed()
    );
}
