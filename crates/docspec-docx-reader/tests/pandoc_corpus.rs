//! Snapshot regression tests for the pandoc DOCX corpus.
//!
//! Runs every `.docx` file in `tests/fixtures/docx/pandoc/` through both
//! `DocxReader::from_reader` and `DocxReader::from_path`, asserts that the two
//! constructors emit identical event streams, and compares against a single
//! committed snapshot per fixture. Exercising both constructors from the
//! corpus is the canonical way to keep the `StreamingArchive` (`from_path`)
//! and in-memory (`from_reader`) paths in lockstep — a divergence anywhere in
//! the 80-fixture corpus fails fast.
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
use std::path::PathBuf;

use docspec_docx_reader::DocxReader;
use docspec_test_utils::capture;

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/docx/pandoc")
        .join(file_name)
}

fn assert_fixture_snapshot(file_name: &str) {
    let path = fixture_path(file_name);

    let from_reader_snapshot = capture(&path, |bytes| DocxReader::from_reader(Cursor::new(bytes)));
    let from_path_snapshot = capture(&path, |_bytes| DocxReader::from_path(&path));

    assert_eq!(
        from_reader_snapshot, from_path_snapshot,
        "from_reader and from_path produced divergent event streams for {file_name}"
    );

    insta::with_settings!({
        snapshot_path => "../../../tests/snapshots/docx/pandoc",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_debug_snapshot!(file_name, from_reader_snapshot);
    });
}

macro_rules! pandoc_fixture_test {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_fixture_snapshot($file_name);
        }
    };
}

include!(concat!(env!("OUT_DIR"), "/pandoc_corpus_tests.rs"));
