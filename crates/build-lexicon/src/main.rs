//! build-lexicon: compile chewing's tsi.csv into the binary image the
//! engine memory-maps (see lightbopomofo_core::lexicon).
//!
//!   build-lexicon data/tsi.csv path/to/lexicon.bin

use std::time::Instant;

use lightbopomofo_core::lexicon::{compile, Lexicon};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output] = args.as_slice() else {
        eprintln!("usage: build-lexicon TSI_CSV OUTPUT_BIN");
        std::process::exit(2);
    };
    let source = std::fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("cannot read {input}: {e}");
        std::process::exit(1);
    });
    let t = Instant::now();
    let image = compile(&source);
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
        "{output}: {} entries, {} KB; compiled in {compiled:.0?}, opens in {:.1?}",
        lexicon.len(),
        image.len() / 1024,
        t.elapsed()
    );
}
