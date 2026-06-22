//! Snapshot regression tests for the pandoc DOCX corpus.
//!
//! Runs every `.docx` file in `tests/fixtures/docx/pandoc/` through
//! `DocxReader` and compares the event stream against committed snapshots.
//!
//! See TESTING.md § Snapshot Review for first-run and CI-mode workflow.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::panic,
    clippy::redundant_test_prefix,
    clippy::std_instead_of_core,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    dead_code
)]

use std::io::Cursor;

use docspec_docx_reader::DocxReader;
use docspec_test_utils::capture;

#[test]
fn pandoc_corpus() {
    insta::with_settings!({
        snapshot_path => "../../../tests/snapshots/docx/pandoc",
    }, {
        insta::glob!("../../../tests/fixtures/docx/pandoc", "*.docx", |path| {
            let snapshot = capture(path, |bytes| DocxReader::from_reader(Cursor::new(bytes)));
            insta::assert_debug_snapshot!(snapshot);
        });
    });
}
