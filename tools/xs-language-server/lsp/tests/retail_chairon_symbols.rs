//! Retail parity test: chairon.xs symbol count.
//!
//! `chairon.xs` is a tiny real vanilla file. The typed-AST builder must
//! produce the same number of top-level symbols as the old tree-sitter
//! path. The old path counted two functions (`preInit` and `postInit`);
//! there are no `extract_error_*` workaround shapes in this file, so the
//! expected count stays 2.

use std::path::PathBuf;

use xs_language_server::symbols;

#[test]
fn symbols_retail_chairon_count() {
    let home = std::env::var("HOME").expect("HOME is required to locate the retail sample");
    let chairon = PathBuf::from(home)
        .join(".steam/steam/steamapps/common/Age of Mythology Retold/game/ai/chairon.xs");

    // If the retail installation is not present (e.g. CI), skip rather than fail.
    if !chairon.exists() {
        eprintln!("skipping: retail sample not found at {}", chairon.display());
        return;
    }

    let source = std::fs::read_to_string(&chairon)
        .unwrap_or_else(|e| panic!("could not read {}: {}", chairon.display(), e));
    let table = symbols::build_symbol_table(&source);

    assert_eq!(
        table.symbols.len(),
        2,
        "chairon.xs should yield exactly two symbols (preInit and postInit); got {:?}",
        table.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    assert!(table.find("preInit").is_some());
    assert!(table.find("postInit").is_some());
}
